//! GitHub client trait abstraction for owner validation.
//!
//! This module provides a trait-based abstraction for GitHub API calls,
//! allowing different implementations (e.g., octocrab, Python bindings).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// The result of checking if a team exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeamExistsResult {
    /// The team exists and is accessible.
    Exists,
    /// The team was not found.
    NotFound,
    /// Insufficient authorization to check the team.
    Unauthorized,
}

impl fmt::Display for TeamExistsResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TeamExistsResult::Exists => write!(f, "exists"),
            TeamExistsResult::NotFound => write!(f, "not_found"),
            TeamExistsResult::Unauthorized => write!(f, "unauthorized"),
        }
    }
}

/// The result of checking if a user exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserExistsResult {
    /// The user exists.
    Exists,
    /// The user was not found.
    NotFound,
    /// Insufficient authorization to check the user.
    Unauthorized,
}

impl fmt::Display for UserExistsResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserExistsResult::Exists => write!(f, "exists"),
            UserExistsResult::NotFound => write!(f, "not_found"),
            UserExistsResult::Unauthorized => write!(f, "unauthorized"),
        }
    }
}

/// Errors that can occur when interacting with the GitHub client.
#[derive(Debug, Error)]
pub enum GithubClientError {
    /// An API error occurred.
    #[error("GitHub API error: {0}")]
    ApiError(String),

    /// A network error occurred.
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Authentication failed.
    #[error("Authentication error: {0}")]
    AuthError(String),

    /// Rate limit exceeded.
    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    /// Other error.
    #[error("{0}")]
    Other(String),
}

/// Trait for GitHub API client implementations.
///
/// This trait abstracts the GitHub API calls needed for owner validation,
/// allowing different implementations such as:
/// - `octocrab` for native Rust usage
/// - Python-based clients (githubkit, pygithub) via PyO3 bindings
///
/// # Example
///
/// ```rust,ignore
/// use codeowners_validator_core::validate::github_client::{GithubClient, UserExistsResult, TeamExistsResult};
///
/// struct MyGithubClient { /* ... */ }
///
/// #[async_trait::async_trait]
/// impl GithubClient for MyGithubClient {
///     async fn user_exists(&self, username: &str) -> Result<UserExistsResult, GithubClientError> {
///         // Check if user exists via your preferred method
///         Ok(UserExistsResult::Exists)
///     }
///
///     async fn team_exists(&self, org: &str, team: &str) -> Result<TeamExistsResult, GithubClientError> {
///         // Check if team exists via your preferred method
///         Ok(TeamExistsResult::Exists)
///     }
/// }
/// ```
#[async_trait]
pub trait GithubClient: Send + Sync {
    /// Checks if a GitHub user exists.
    ///
    /// # Arguments
    ///
    /// * `username` - The GitHub username (without the leading '@')
    ///
    /// # Returns
    ///
    /// * `Ok(UserExistsResult::Exists)` - The user exists
    /// * `Ok(UserExistsResult::NotFound)` - The user was not found
    /// * `Ok(UserExistsResult::Unauthorized)` - Insufficient permissions to check
    /// * `Err(GithubClientError)` - An error occurred
    async fn user_exists(&self, username: &str) -> Result<UserExistsResult, GithubClientError>;

    /// Checks if a GitHub team exists within an organization.
    ///
    /// # Arguments
    ///
    /// * `org` - The organization name
    /// * `team` - The team slug (name)
    ///
    /// # Returns
    ///
    /// * `Ok(TeamExistsResult::Exists)` - The team exists
    /// * `Ok(TeamExistsResult::NotFound)` - The team was not found
    /// * `Ok(TeamExistsResult::Unauthorized)` - Insufficient permissions to check
    /// * `Err(GithubClientError)` - An error occurred
    async fn team_exists(
        &self,
        org: &str,
        team: &str,
    ) -> Result<TeamExistsResult, GithubClientError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn team_exists_result_display() {
        assert_eq!(TeamExistsResult::Exists.to_string(), "exists");
        assert_eq!(TeamExistsResult::NotFound.to_string(), "not_found");
        assert_eq!(TeamExistsResult::Unauthorized.to_string(), "unauthorized");
    }

    #[test]
    fn user_exists_result_display() {
        assert_eq!(UserExistsResult::Exists.to_string(), "exists");
        assert_eq!(UserExistsResult::NotFound.to_string(), "not_found");
        assert_eq!(UserExistsResult::Unauthorized.to_string(), "unauthorized");
    }

    #[test]
    fn github_client_error_display() {
        let err = GithubClientError::ApiError("test error".to_string());
        assert!(err.to_string().contains("test error"));
    }
}
