//! Owners validation check.
//!
//! This check verifies that owners specified in CODEOWNERS actually exist on GitHub.

use super::{AsyncCheck, AsyncCheckContext};
use crate::parse::{LineKind, Owner};
use crate::validate::github_client::{TeamExistsResult, UserExistsResult};
use crate::validate::{ValidationError, ValidationResult};
use async_trait::async_trait;
use log::{debug, trace, warn};

/// A check that validates owners exist on GitHub.
///
/// For each owner in the CODEOWNERS file:
/// - `@username`: Verifies the user exists
/// - `@org/team`: Verifies the team exists in the organization
/// - `email@domain.com`: Skips validation (cannot verify via API)
///
/// Reports authorization errors if the token lacks required permissions.
#[derive(Debug, Clone, Default)]
pub struct OwnersCheck;

impl OwnersCheck {
    /// Creates a new owners check.
    pub fn new() -> Self {
        Self
    }

    /// Validates a single owner.
    async fn validate_owner(
        &self,
        owner: &Owner,
        ctx: &AsyncCheckContext<'_>,
    ) -> Option<ValidationError> {
        // Check if owner is in the ignored list
        let owner_str = owner.as_str();
        if ctx.config.ignored_owners.contains(owner_str.as_ref()) {
            trace!("Skipping ignored owner: {}", owner_str);
            return None;
        }

        match owner {
            Owner::User { name, span } => {
                // Check if owners must be teams
                if ctx.config.owners_must_be_teams {
                    debug!("User @{} rejected: owners_must_be_teams is enabled", name);
                    return Some(ValidationError::owner_must_be_team(
                        format!("@{}", name),
                        *span,
                    ));
                }

                // Verify user exists using the GitHub client trait
                trace!("Checking if user @{} exists", name);
                match ctx.github_client.user_exists(name).await {
                    Ok(UserExistsResult::Exists) => {
                        trace!("User @{} exists", name);
                        None
                    }
                    Ok(UserExistsResult::NotFound) => {
                        debug!("User @{} not found", name);
                        Some(ValidationError::owner_not_found(
                            format!("@{}", name),
                            "user does not exist",
                            *span,
                        ))
                    }
                    Ok(UserExistsResult::Unauthorized) => {
                        warn!("Unauthorized to check user @{}", name);
                        Some(ValidationError::insufficient_authorization(
                            format!("@{}", name),
                            "may need additional token scopes",
                            *span,
                        ))
                    }
                    Err(e) => {
                        warn!("API error checking user @{}: {}", name, e);
                        Some(ValidationError::owner_not_found(
                            format!("@{}", name),
                            format!("API error: {}", e),
                            *span,
                        ))
                    }
                }
            }
            Owner::Team { org, team, span } => {
                // Verify team exists in organization using the GitHub client trait
                trace!("Checking if team @{}/{} exists", org, team);
                match ctx.github_client.team_exists(org, team).await {
                    Ok(TeamExistsResult::Exists) => {
                        trace!("Team @{}/{} exists", org, team);
                        None
                    }
                    Ok(TeamExistsResult::NotFound) => {
                        debug!("Team @{}/{} not found", org, team);
                        Some(ValidationError::owner_not_found(
                            format!("@{}/{}", org, team),
                            "team does not exist in organization",
                            *span,
                        ))
                    }
                    Ok(TeamExistsResult::Unauthorized) => {
                        warn!("Unauthorized to check team @{}/{}", org, team);
                        Some(ValidationError::insufficient_authorization(
                            format!("@{}/{}", org, team),
                            "may need read:org scope or team membership",
                            *span,
                        ))
                    }
                    Err(e) => {
                        warn!("API error checking team @{}/{}: {}", org, team, e);
                        Some(ValidationError::owner_not_found(
                            format!("@{}/{}", org, team),
                            format!("API error: {}", e),
                            *span,
                        ))
                    }
                }
            }
            Owner::Email { .. } => {
                // Cannot validate emails via GitHub API
                trace!("Skipping email owner validation: {}", owner.as_str());
                // Check if owners must be teams
                if ctx.config.owners_must_be_teams {
                    debug!(
                        "Email {} rejected: owners_must_be_teams is enabled",
                        owner.as_str()
                    );
                    return Some(ValidationError::owner_must_be_team(
                        owner.as_str(),
                        *owner.span(),
                    ));
                }
                None
            }
        }
    }
}

#[async_trait]
impl AsyncCheck for OwnersCheck {
    fn name(&self) -> &'static str {
        "owners"
    }

    async fn run(&self, ctx: &AsyncCheckContext<'_>) -> ValidationResult {
        debug!("Running owners check");
        let mut result = ValidationResult::new();

        // Collect all unique owners to avoid duplicate API calls
        let mut checked_owners = std::collections::HashSet::new();

        for line in &ctx.file.lines {
            if let LineKind::Rule { owners, .. } = &line.kind {
                for owner in owners {
                    let owner_str = owner.as_str();

                    // Skip if already checked
                    if checked_owners.contains(owner_str.as_ref()) {
                        trace!("Skipping already-checked owner: {}", owner_str);
                        continue;
                    }
                    checked_owners.insert(owner_str.into_owned());

                    if let Some(error) = self.validate_owner(owner, ctx).await {
                        result.add_error(error);
                    }
                }
            }
        }

        debug!(
            "Owners check complete: {} unique owners checked, {} errors found",
            checked_owners.len(),
            result.errors.len()
        );
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_codeowners;
    use crate::validate::checks::CheckConfig;
    use crate::validate::github_client::{GithubClient, GithubClientError};
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A mock GitHub client for testing.
    struct MockGithubClient {
        users: HashSet<String>,
        teams: HashSet<(String, String)>,
        unauthorized_users: HashSet<String>,
        unauthorized_teams: HashSet<(String, String)>,
        user_call_count: AtomicUsize,
        team_call_count: AtomicUsize,
    }

    impl MockGithubClient {
        fn new() -> Self {
            Self {
                users: HashSet::new(),
                teams: HashSet::new(),
                unauthorized_users: HashSet::new(),
                unauthorized_teams: HashSet::new(),
                user_call_count: AtomicUsize::new(0),
                team_call_count: AtomicUsize::new(0),
            }
        }

        fn with_user(mut self, username: &str) -> Self {
            self.users.insert(username.to_string());
            self
        }

        fn with_team(mut self, org: &str, team: &str) -> Self {
            self.teams.insert((org.to_string(), team.to_string()));
            self
        }

        fn with_unauthorized_team(mut self, org: &str, team: &str) -> Self {
            self.unauthorized_teams
                .insert((org.to_string(), team.to_string()));
            self
        }

        fn user_calls(&self) -> usize {
            self.user_call_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl GithubClient for MockGithubClient {
        async fn user_exists(&self, username: &str) -> Result<UserExistsResult, GithubClientError> {
            self.user_call_count.fetch_add(1, Ordering::SeqCst);
            if self.unauthorized_users.contains(username) {
                Ok(UserExistsResult::Unauthorized)
            } else if self.users.contains(username) {
                Ok(UserExistsResult::Exists)
            } else {
                Ok(UserExistsResult::NotFound)
            }
        }

        async fn team_exists(
            &self,
            org: &str,
            team: &str,
        ) -> Result<TeamExistsResult, GithubClientError> {
            self.team_call_count.fetch_add(1, Ordering::SeqCst);
            let key = (org.to_string(), team.to_string());
            if self.unauthorized_teams.contains(&key) {
                Ok(TeamExistsResult::Unauthorized)
            } else if self.teams.contains(&key) {
                Ok(TeamExistsResult::Exists)
            } else {
                Ok(TeamExistsResult::NotFound)
            }
        }
    }

    #[tokio::test]
    async fn user_exists() {
        let client = MockGithubClient::new().with_user("validuser");
        let file = parse_codeowners("*.rs @validuser\n").ast;
        let path = PathBuf::from("/repo");
        let config = CheckConfig::new();
        let ctx = AsyncCheckContext::new(&file, &path, &config, &client);

        let result = OwnersCheck::new().run(&ctx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn user_not_found() {
        let client = MockGithubClient::new(); // No users registered
        let file = parse_codeowners("*.rs @ghostuser\n").ast;
        let path = PathBuf::from("/repo");
        let config = CheckConfig::new();
        let ctx = AsyncCheckContext::new(&file, &path, &config, &client);

        let result = OwnersCheck::new().run(&ctx).await;
        assert!(result.has_errors());

        match &result.errors[0] {
            ValidationError::OwnerNotFound { owner, .. } => {
                assert_eq!(owner, "@ghostuser");
            }
            _ => panic!("Expected OwnerNotFound error"),
        }
    }

    #[tokio::test]
    async fn team_exists() {
        let client = MockGithubClient::new().with_team("myorg", "myteam");
        let file = parse_codeowners("*.rs @myorg/myteam\n").ast;
        let path = PathBuf::from("/repo");
        let config = CheckConfig::new();
        let ctx = AsyncCheckContext::new(&file, &path, &config, &client);

        let result = OwnersCheck::new().run(&ctx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn team_not_found() {
        let client = MockGithubClient::new(); // No teams registered
        let file = parse_codeowners("*.rs @myorg/nonexistent\n").ast;
        let path = PathBuf::from("/repo");
        let config = CheckConfig::new();
        let ctx = AsyncCheckContext::new(&file, &path, &config, &client);

        let result = OwnersCheck::new().run(&ctx).await;
        assert!(result.has_errors());

        match &result.errors[0] {
            ValidationError::OwnerNotFound { owner, .. } => {
                assert_eq!(owner, "@myorg/nonexistent");
            }
            _ => panic!("Expected OwnerNotFound error"),
        }
    }

    #[tokio::test]
    async fn insufficient_authorization() {
        let client = MockGithubClient::new().with_unauthorized_team("privateorg", "privateteam");
        let file = parse_codeowners("*.rs @privateorg/privateteam\n").ast;
        let path = PathBuf::from("/repo");
        let config = CheckConfig::new();
        let ctx = AsyncCheckContext::new(&file, &path, &config, &client);

        let result = OwnersCheck::new().run(&ctx).await;
        assert!(result.has_errors());

        match &result.errors[0] {
            ValidationError::InsufficientAuthorization { owner, .. } => {
                assert_eq!(owner, "@privateorg/privateteam");
            }
            _ => panic!("Expected InsufficientAuthorization error"),
        }
    }

    #[tokio::test]
    async fn ignored_owner_skipped() {
        let client = MockGithubClient::new(); // No users - but should be skipped
        let file = parse_codeowners("*.rs @ignored\n").ast;
        let path = PathBuf::from("/repo");
        let mut ignored = HashSet::new();
        ignored.insert("@ignored".to_string());
        let config = CheckConfig::new().with_ignored_owners(ignored);
        let ctx = AsyncCheckContext::new(&file, &path, &config, &client);

        let result = OwnersCheck::new().run(&ctx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn owners_must_be_teams_rejects_user() {
        let client = MockGithubClient::new().with_user("someuser");
        let file = parse_codeowners("*.rs @someuser\n").ast;
        let path = PathBuf::from("/repo");
        let config = CheckConfig::new().with_owners_must_be_teams(true);
        let ctx = AsyncCheckContext::new(&file, &path, &config, &client);

        let result = OwnersCheck::new().run(&ctx).await;
        assert!(result.has_errors());

        match &result.errors[0] {
            ValidationError::OwnerMustBeTeam { owner, .. } => {
                assert_eq!(owner, "@someuser");
            }
            _ => panic!("Expected OwnerMustBeTeam error"),
        }
    }

    #[tokio::test]
    async fn email_skipped() {
        let client = MockGithubClient::new();
        let file = parse_codeowners("*.rs user@example.com\n").ast;
        let path = PathBuf::from("/repo");
        let config = CheckConfig::new();
        let ctx = AsyncCheckContext::new(&file, &path, &config, &client);

        let result = OwnersCheck::new().run(&ctx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn email_rejected_when_must_be_teams() {
        let client = MockGithubClient::new();
        let file = parse_codeowners("*.rs user@example.com\n").ast;
        let path = PathBuf::from("/repo");
        let config = CheckConfig::new().with_owners_must_be_teams(true);
        let ctx = AsyncCheckContext::new(&file, &path, &config, &client);

        let result = OwnersCheck::new().run(&ctx).await;
        assert!(result.has_errors());

        match &result.errors[0] {
            ValidationError::OwnerMustBeTeam { .. } => {}
            _ => panic!("Expected OwnerMustBeTeam error"),
        }
    }

    #[tokio::test]
    async fn duplicate_owners_checked_once() {
        let client = MockGithubClient::new().with_user("sameuser");
        let file = parse_codeowners("*.rs @sameuser\n*.md @sameuser\n").ast;
        let path = PathBuf::from("/repo");
        let config = CheckConfig::new();
        let ctx = AsyncCheckContext::new(&file, &path, &config, &client);

        let result = OwnersCheck::new().run(&ctx).await;
        assert!(result.is_ok());
        // Verify user_exists was only called once
        assert_eq!(client.user_calls(), 1);
    }
}
