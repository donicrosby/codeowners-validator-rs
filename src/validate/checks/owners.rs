//! Owners validation check.
//!
//! This check verifies that owners specified in CODEOWNERS actually exist on GitHub.

use super::{AsyncCheck, AsyncCheckContext};
use crate::parse::{LineKind, Owner};
use crate::validate::{ValidationError, ValidationResult};
use async_trait::async_trait;
use http::StatusCode;

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
        if ctx.config.ignored_owners.contains(&owner_str) {
            return None;
        }

        match owner {
            Owner::User { name, span } => {
                // Check if owners must be teams
                if ctx.config.owners_must_be_teams {
                    return Some(ValidationError::owner_must_be_team(
                        format!("@{}", name),
                        *span,
                    ));
                }

                // Verify user exists
                match ctx.octocrab.users(name).profile().await {
                    Ok(_) => None,
                    Err(e) => {
                        if let Some(status) = extract_status_code(&e) {
                            match status {
                                StatusCode::NOT_FOUND => {
                                    Some(ValidationError::owner_not_found(
                                        format!("@{}", name),
                                        "user does not exist",
                                        *span,
                                    ))
                                }
                                StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                                    Some(ValidationError::insufficient_authorization(
                                        format!("@{}", name),
                                        format!("API returned {}: may need additional token scopes", status),
                                        *span,
                                    ))
                                }
                                _ => {
                                    Some(ValidationError::owner_not_found(
                                        format!("@{}", name),
                                        format!("API error: {}", e),
                                        *span,
                                    ))
                                }
                            }
                        } else {
                            Some(ValidationError::owner_not_found(
                                format!("@{}", name),
                                format!("API error: {}", e),
                                *span,
                            ))
                        }
                    }
                }
            }
            Owner::Team { org, team, span } => {
                // Verify team exists in organization
                match ctx.octocrab.teams(org).get(team).await {
                    Ok(_) => None,
                    Err(e) => {
                        if let Some(status) = extract_status_code(&e) {
                            match status {
                                StatusCode::NOT_FOUND => {
                                    Some(ValidationError::owner_not_found(
                                        format!("@{}/{}", org, team),
                                        "team does not exist in organization",
                                        *span,
                                    ))
                                }
                                StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                                    Some(ValidationError::insufficient_authorization(
                                        format!("@{}/{}", org, team),
                                        format!(
                                            "API returned {}: may need read:org scope or team membership",
                                            status
                                        ),
                                        *span,
                                    ))
                                }
                                _ => {
                                    Some(ValidationError::owner_not_found(
                                        format!("@{}/{}", org, team),
                                        format!("API error: {}", e),
                                        *span,
                                    ))
                                }
                            }
                        } else {
                            Some(ValidationError::owner_not_found(
                                format!("@{}/{}", org, team),
                                format!("API error: {}", e),
                                *span,
                            ))
                        }
                    }
                }
            }
            Owner::Email { .. } => {
                // Cannot validate emails via GitHub API
                // Check if owners must be teams
                if ctx.config.owners_must_be_teams {
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

/// Extracts the HTTP status code from an octocrab error.
fn extract_status_code(error: &octocrab::Error) -> Option<StatusCode> {
    match error {
        octocrab::Error::GitHub { source, .. } => Some(source.status_code),
        _ => None,
    }
}

#[async_trait]
impl AsyncCheck for OwnersCheck {
    fn name(&self) -> &'static str {
        "owners"
    }

    async fn run(&self, ctx: &AsyncCheckContext<'_>) -> ValidationResult {
        let mut result = ValidationResult::new();

        // Collect all unique owners to avoid duplicate API calls
        let mut checked_owners = std::collections::HashSet::new();

        for line in &ctx.file.lines {
            if let LineKind::Rule { owners, .. } = &line.kind {
                for owner in owners {
                    let owner_str = owner.as_str();
                    
                    // Skip if already checked
                    if checked_owners.contains(&owner_str) {
                        continue;
                    }
                    checked_owners.insert(owner_str);

                    if let Some(error) = self.validate_owner(owner, ctx).await {
                        result.add_error(error);
                    }
                }
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_codeowners;
    use crate::validate::checks::CheckConfig;
    use std::collections::HashSet;
    use std::path::PathBuf;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn setup_mock_server() -> MockServer {
        MockServer::start().await
    }

    fn create_octocrab(base_uri: &str) -> octocrab::Octocrab {
        octocrab::Octocrab::builder()
            .base_uri(base_uri)
            .unwrap()
            .build()
            .unwrap()
    }

    fn mock_user_response(username: &str) -> serde_json::Value {
        serde_json::json!({
            "login": username,
            "id": 12345,
            "node_id": "MDQ6VXNlcjEyMzQ1",
            "avatar_url": format!("https://avatars.githubusercontent.com/u/12345?v=4"),
            "gravatar_id": "",
            "url": format!("https://api.github.com/users/{}", username),
            "html_url": format!("https://github.com/{}", username),
            "followers_url": format!("https://api.github.com/users/{}/followers", username),
            "following_url": format!("https://api.github.com/users/{}/following{{/other_user}}", username),
            "gists_url": format!("https://api.github.com/users/{}/gists{{/gist_id}}", username),
            "starred_url": format!("https://api.github.com/users/{}/starred{{/owner}}{{/repo}}", username),
            "subscriptions_url": format!("https://api.github.com/users/{}/subscriptions", username),
            "organizations_url": format!("https://api.github.com/users/{}/orgs", username),
            "repos_url": format!("https://api.github.com/users/{}/repos", username),
            "events_url": format!("https://api.github.com/users/{}/events{{/privacy}}", username),
            "received_events_url": format!("https://api.github.com/users/{}/received_events", username),
            "type": "User",
            "site_admin": false,
            "name": username,
            "company": null,
            "blog": "",
            "location": null,
            "email": null,
            "hireable": null,
            "bio": null,
            "twitter_username": null,
            "public_repos": 10,
            "public_gists": 0,
            "followers": 5,
            "following": 3,
            "created_at": "2020-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        })
    }

    #[tokio::test]
    async fn user_exists() {
        let mock_server = setup_mock_server().await;
        
        Mock::given(method("GET"))
            .and(path("/users/validuser"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_user_response("validuser")))
            .mount(&mock_server)
            .await;

        let octocrab = create_octocrab(&mock_server.uri());
        let file = parse_codeowners("*.rs @validuser\n").ast;
        let path = PathBuf::from("/repo");
        let config = CheckConfig::new();
        let ctx = AsyncCheckContext::new(&file, &path, &config, &octocrab);
        
        let result = OwnersCheck::new().run(&ctx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn user_not_found() {
        let mock_server = setup_mock_server().await;
        
        Mock::given(method("GET"))
            .and(path("/users/ghostuser"))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "message": "Not Found",
                "documentation_url": "https://docs.github.com/rest"
            })))
            .mount(&mock_server)
            .await;

        let octocrab = create_octocrab(&mock_server.uri());
        let file = parse_codeowners("*.rs @ghostuser\n").ast;
        let path = PathBuf::from("/repo");
        let config = CheckConfig::new();
        let ctx = AsyncCheckContext::new(&file, &path, &config, &octocrab);
        
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
        let mock_server = setup_mock_server().await;
        
        Mock::given(method("GET"))
            .and(path("/orgs/myorg/teams/myteam"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "name": "myteam",
                "id": 12345,
                "node_id": "MDQ6VGVhbTEyMzQ1",
                "slug": "myteam",
                "description": "Test team",
                "privacy": "closed",
                "permission": "pull",
                "url": "https://api.github.com/orgs/myorg/teams/myteam",
                "html_url": "https://github.com/orgs/myorg/teams/myteam",
                "members_url": "https://api.github.com/orgs/myorg/teams/myteam/members{/member}",
                "repositories_url": "https://api.github.com/orgs/myorg/teams/myteam/repos"
            })))
            .mount(&mock_server)
            .await;

        let octocrab = create_octocrab(&mock_server.uri());
        let file = parse_codeowners("*.rs @myorg/myteam\n").ast;
        let path = PathBuf::from("/repo");
        let config = CheckConfig::new();
        let ctx = AsyncCheckContext::new(&file, &path, &config, &octocrab);
        
        let result = OwnersCheck::new().run(&ctx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn team_not_found() {
        let mock_server = setup_mock_server().await;
        
        Mock::given(method("GET"))
            .and(path("/orgs/myorg/teams/nonexistent"))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "message": "Not Found",
                "documentation_url": "https://docs.github.com/rest"
            })))
            .mount(&mock_server)
            .await;

        let octocrab = create_octocrab(&mock_server.uri());
        let file = parse_codeowners("*.rs @myorg/nonexistent\n").ast;
        let path = PathBuf::from("/repo");
        let config = CheckConfig::new();
        let ctx = AsyncCheckContext::new(&file, &path, &config, &octocrab);
        
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
        let mock_server = setup_mock_server().await;
        
        Mock::given(method("GET"))
            .and(path("/orgs/privateorg/teams/privateteam"))
            .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
                "message": "Must have admin rights to Repository",
                "documentation_url": "https://docs.github.com/rest"
            })))
            .mount(&mock_server)
            .await;

        let octocrab = create_octocrab(&mock_server.uri());
        let file = parse_codeowners("*.rs @privateorg/privateteam\n").ast;
        let path = PathBuf::from("/repo");
        let config = CheckConfig::new();
        let ctx = AsyncCheckContext::new(&file, &path, &config, &octocrab);
        
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
        let mock_server = setup_mock_server().await;
        // No mock needed - the owner should be skipped

        let octocrab = create_octocrab(&mock_server.uri());
        let file = parse_codeowners("*.rs @ignored\n").ast;
        let path = PathBuf::from("/repo");
        let mut ignored = HashSet::new();
        ignored.insert("@ignored".to_string());
        let config = CheckConfig::new().with_ignored_owners(ignored);
        let ctx = AsyncCheckContext::new(&file, &path, &config, &octocrab);
        
        let result = OwnersCheck::new().run(&ctx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn owners_must_be_teams_rejects_user() {
        let mock_server = setup_mock_server().await;
        // No mock needed - should fail before API call

        let octocrab = create_octocrab(&mock_server.uri());
        let file = parse_codeowners("*.rs @someuser\n").ast;
        let path = PathBuf::from("/repo");
        let config = CheckConfig::new().with_owners_must_be_teams(true);
        let ctx = AsyncCheckContext::new(&file, &path, &config, &octocrab);
        
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
        let mock_server = setup_mock_server().await;
        // No mock needed - emails are skipped

        let octocrab = create_octocrab(&mock_server.uri());
        let file = parse_codeowners("*.rs user@example.com\n").ast;
        let path = PathBuf::from("/repo");
        let config = CheckConfig::new();
        let ctx = AsyncCheckContext::new(&file, &path, &config, &octocrab);
        
        let result = OwnersCheck::new().run(&ctx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn email_rejected_when_must_be_teams() {
        let mock_server = setup_mock_server().await;

        let octocrab = create_octocrab(&mock_server.uri());
        let file = parse_codeowners("*.rs user@example.com\n").ast;
        let path = PathBuf::from("/repo");
        let config = CheckConfig::new().with_owners_must_be_teams(true);
        let ctx = AsyncCheckContext::new(&file, &path, &config, &octocrab);
        
        let result = OwnersCheck::new().run(&ctx).await;
        assert!(result.has_errors());
        
        match &result.errors[0] {
            ValidationError::OwnerMustBeTeam { .. } => {}
            _ => panic!("Expected OwnerMustBeTeam error"),
        }
    }

    #[tokio::test]
    async fn duplicate_owners_checked_once() {
        let mock_server = setup_mock_server().await;
        
        // Should only be called once even though owner appears twice
        Mock::given(method("GET"))
            .and(path("/users/sameuser"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_user_response("sameuser")))
            .expect(1) // Exactly one call
            .mount(&mock_server)
            .await;

        let octocrab = create_octocrab(&mock_server.uri());
        let file = parse_codeowners("*.rs @sameuser\n*.md @sameuser\n").ast;
        let path = PathBuf::from("/repo");
        let config = CheckConfig::new();
        let ctx = AsyncCheckContext::new(&file, &path, &config, &octocrab);
        
        let result = OwnersCheck::new().run(&ctx).await;
        assert!(result.is_ok());
    }
}
