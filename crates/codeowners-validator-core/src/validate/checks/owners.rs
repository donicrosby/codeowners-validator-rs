//! Owners validation check.
//!
//! This check verifies that owners specified in CODEOWNERS actually exist on GitHub.

use super::{AsyncCheck, AsyncCheckContext};
use crate::parse::{LineKind, Owner, Span};
use crate::validate::github_client::{TeamExistsResult, UserExistsResult};
use crate::validate::{ValidationError, ValidationResult};
use async_trait::async_trait;
use futures::future::join_all;
use log::{debug, trace, warn};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Represents the kind of validation failure for an owner, without span information.
/// This allows us to validate once and create errors for multiple occurrences.
#[derive(Debug, Clone)]
enum OwnerValidationFailure {
    /// Owner not found on GitHub.
    NotFound { owner: String, reason: String },
    /// Insufficient authorization to verify owner.
    Unauthorized { owner: String, reason: String },
    /// Owner must be a team but is not.
    MustBeTeam { owner: String },
}

impl OwnerValidationFailure {
    /// Creates a ValidationError from this failure with the given span.
    fn to_error(&self, span: Span) -> ValidationError {
        match self {
            OwnerValidationFailure::NotFound { owner, reason } => {
                ValidationError::owner_not_found(owner, reason, span)
            }
            OwnerValidationFailure::Unauthorized { owner, reason } => {
                ValidationError::insufficient_authorization(owner, reason, span)
            }
            OwnerValidationFailure::MustBeTeam { owner } => {
                ValidationError::owner_must_be_team(owner, span)
            }
        }
    }
}

/// Maximum number of concurrent GitHub API requests.
/// Conservative limit to leave headroom for other application requests.
const MAX_CONCURRENT_REQUESTS: usize = 10;

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

    /// Validates a single owner and returns a failure description (without span).
    /// This allows us to validate once per unique owner and apply the result to all occurrences.
    async fn validate_owner_inner(
        &self,
        owner: &Owner,
        ctx: &AsyncCheckContext<'_>,
    ) -> Option<OwnerValidationFailure> {
        // Check if owner is in the ignored list
        let owner_str = owner.as_str();
        if ctx.config.ignored_owners.contains(owner_str.as_ref()) {
            trace!("Skipping ignored owner: {}", owner_str);
            return None;
        }

        match owner {
            Owner::User { name, .. } => {
                // Check if owners must be teams
                if ctx.config.owners_must_be_teams {
                    debug!("User @{} rejected: owners_must_be_teams is enabled", name);
                    return Some(OwnerValidationFailure::MustBeTeam {
                        owner: format!("@{}", name),
                    });
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
                        Some(OwnerValidationFailure::NotFound {
                            owner: format!("@{}", name),
                            reason: "user does not exist".to_string(),
                        })
                    }
                    Ok(UserExistsResult::Unauthorized) => {
                        warn!("Unauthorized to check user @{}", name);
                        Some(OwnerValidationFailure::Unauthorized {
                            owner: format!("@{}", name),
                            reason: "may need additional token scopes".to_string(),
                        })
                    }
                    Err(e) => {
                        warn!("API error checking user @{}: {}", name, e);
                        Some(OwnerValidationFailure::NotFound {
                            owner: format!("@{}", name),
                            reason: format!("API error: {}", e),
                        })
                    }
                }
            }
            Owner::Team { org, team, .. } => {
                // Verify team exists in organization using the GitHub client trait
                trace!("Checking if team @{}/{} exists", org, team);
                match ctx.github_client.team_exists(org, team).await {
                    Ok(TeamExistsResult::Exists) => {
                        trace!("Team @{}/{} exists", org, team);
                        None
                    }
                    Ok(TeamExistsResult::NotFound) => {
                        debug!("Team @{}/{} not found", org, team);
                        Some(OwnerValidationFailure::NotFound {
                            owner: format!("@{}/{}", org, team),
                            reason: "team does not exist in organization".to_string(),
                        })
                    }
                    Ok(TeamExistsResult::Unauthorized) => {
                        warn!("Unauthorized to check team @{}/{}", org, team);
                        Some(OwnerValidationFailure::Unauthorized {
                            owner: format!("@{}/{}", org, team),
                            reason: "may need read:org scope or team membership".to_string(),
                        })
                    }
                    Err(e) => {
                        warn!("API error checking team @{}/{}: {}", org, team, e);
                        Some(OwnerValidationFailure::NotFound {
                            owner: format!("@{}/{}", org, team),
                            reason: format!("API error: {}", e),
                        })
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
                    return Some(OwnerValidationFailure::MustBeTeam {
                        owner: owner.as_str().into_owned(),
                    });
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

        // Collect all owners grouped by their string representation.
        // This allows us to make one API call per unique owner while tracking
        // all occurrences so we can report errors for each line.
        let mut owners_by_str: HashMap<String, Vec<&Owner>> = HashMap::new();

        for line in &ctx.file.lines {
            if let LineKind::Rule { owners, .. } = &line.kind {
                for owner in owners {
                    let owner_str = owner.as_str().into_owned();
                    owners_by_str.entry(owner_str).or_default().push(owner);
                }
            }
        }

        debug!(
            "Collected {} unique owners to validate ({} total occurrences)",
            owners_by_str.len(),
            owners_by_str.values().map(|v| v.len()).sum::<usize>()
        );

        if owners_by_str.is_empty() {
            return result;
        }

        // Use bounded concurrency to avoid rate limiting
        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS));

        // Create futures for all unique owner validations.
        // We validate using the first occurrence of each owner, but we'll
        // create errors for all occurrences if validation fails.
        let futures: Vec<_> = owners_by_str
            .iter()
            .map(|(owner_str, occurrences)| {
                let permit = semaphore.clone();
                let first_occurrence = occurrences[0];
                async move {
                    // Acquire semaphore permit before making API call
                    let _permit = permit.acquire().await.ok()?;
                    let failure = self.validate_owner_inner(first_occurrence, ctx).await?;
                    Some((owner_str.clone(), failure))
                }
            })
            .collect();

        // Run all validations concurrently (bounded by semaphore)
        let validation_results = join_all(futures).await;

        // Collect validation failures
        let failures: HashMap<String, OwnerValidationFailure> =
            validation_results.into_iter().flatten().collect();

        // Create errors for ALL occurrences of each failed owner
        for (owner_str, failure) in &failures {
            // Check if it's an authorization error and log a warning (once per owner)
            if matches!(failure, OwnerValidationFailure::Unauthorized { .. }) {
                warn!(
                    "GitHub API authorization issue encountered for {}",
                    owner_str
                );
            }

            // Get all occurrences of this owner and create an error for each
            if let Some(occurrences) = owners_by_str.get(owner_str) {
                for owner in occurrences {
                    let error = failure.to_error(*owner.span());
                    result.add_error(error);
                }
            }
        }

        debug!(
            "Owners check complete: {} errors found",
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

    #[tokio::test]
    async fn duplicate_invalid_owner_reports_all_lines() {
        // When the same invalid owner appears on multiple lines,
        // we should report errors for ALL lines, not just the first one.
        let client = MockGithubClient::new(); // No users registered - ghostuser will be not found
        let file = parse_codeowners("*.rs @ghostuser\n*.md @ghostuser\n*.txt @ghostuser\n").ast;
        let path = PathBuf::from("/repo");
        let config = CheckConfig::new();
        let ctx = AsyncCheckContext::new(&file, &path, &config, &client);

        let result = OwnersCheck::new().run(&ctx).await;
        assert!(result.has_errors());

        // Should have 3 errors, one for each line
        assert_eq!(
            result.errors.len(),
            3,
            "Expected 3 errors (one per line), got {}",
            result.errors.len()
        );

        // Verify each error is for the correct line
        let mut lines: Vec<usize> = result.errors.iter().map(|e| e.line()).collect();
        lines.sort();
        assert_eq!(
            lines,
            vec![1, 2, 3],
            "Errors should be on lines 1, 2, and 3"
        );

        // Verify all errors are OwnerNotFound for @ghostuser
        for error in &result.errors {
            match error {
                ValidationError::OwnerNotFound { owner, .. } => {
                    assert_eq!(owner, "@ghostuser");
                }
                _ => panic!("Expected OwnerNotFound error, got {:?}", error),
            }
        }

        // Verify API was still only called once (efficiency preserved)
        assert_eq!(client.user_calls(), 1);
    }

    #[tokio::test]
    async fn duplicate_invalid_team_reports_all_lines() {
        // Same test but for teams
        let client = MockGithubClient::new(); // No teams registered
        let file = parse_codeowners("*.rs @myorg/ghost\n*.md @myorg/ghost\n").ast;
        let path = PathBuf::from("/repo");
        let config = CheckConfig::new();
        let ctx = AsyncCheckContext::new(&file, &path, &config, &client);

        let result = OwnersCheck::new().run(&ctx).await;
        assert!(result.has_errors());

        // Should have 2 errors, one for each line
        assert_eq!(result.errors.len(), 2);

        let mut lines: Vec<usize> = result.errors.iter().map(|e| e.line()).collect();
        lines.sort();
        assert_eq!(lines, vec![1, 2]);
    }

    #[tokio::test]
    async fn duplicate_must_be_team_reports_all_lines() {
        // When owners_must_be_teams is enabled and a user appears on multiple lines,
        // all lines should be reported.
        let client = MockGithubClient::new().with_user("someuser");
        let file = parse_codeowners("*.rs @someuser\n*.md @someuser\n").ast;
        let path = PathBuf::from("/repo");
        let config = CheckConfig::new().with_owners_must_be_teams(true);
        let ctx = AsyncCheckContext::new(&file, &path, &config, &client);

        let result = OwnersCheck::new().run(&ctx).await;
        assert!(result.has_errors());

        // Should have 2 errors, one for each line
        assert_eq!(result.errors.len(), 2);

        for error in &result.errors {
            match error {
                ValidationError::OwnerMustBeTeam { owner, .. } => {
                    assert_eq!(owner, "@someuser");
                }
                _ => panic!("Expected OwnerMustBeTeam error"),
            }
        }
    }

    #[tokio::test]
    async fn mixed_valid_and_invalid_owners() {
        // Test that valid owners don't generate errors but invalid ones do for all occurrences
        let client = MockGithubClient::new().with_user("validuser");
        let file =
            parse_codeowners("*.rs @validuser @ghostuser\n*.md @ghostuser\n*.txt @validuser\n").ast;
        let path = PathBuf::from("/repo");
        let config = CheckConfig::new();
        let ctx = AsyncCheckContext::new(&file, &path, &config, &client);

        let result = OwnersCheck::new().run(&ctx).await;
        assert!(result.has_errors());

        // ghostuser appears on lines 1 and 2, so 2 errors
        assert_eq!(result.errors.len(), 2);

        let mut lines: Vec<usize> = result.errors.iter().map(|e| e.line()).collect();
        lines.sort();
        assert_eq!(lines, vec![1, 2]);
    }
}
