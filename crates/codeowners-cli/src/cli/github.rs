//! GitHub client implementation using octocrab.
//!
//! This module provides the octocrab-based implementation of the GithubClient trait
//! for use in the CLI.

use async_trait::async_trait;
use codeowners_validator_core::validate::github_client::{
    GithubClient, GithubClientError, TeamExistsResult, UserExistsResult,
};
use http::StatusCode;

/// A wrapper around `octocrab::Octocrab` that implements `GithubClient`.
///
/// This wrapper is necessary due to Rust's orphan rules, which prevent
/// implementing external traits on external types.
pub struct OctocrabClient(pub octocrab::Octocrab);

impl OctocrabClient {
    /// Creates a new OctocrabClient from an Octocrab instance.
    pub fn new(client: octocrab::Octocrab) -> Self {
        Self(client)
    }
}

impl std::ops::Deref for OctocrabClient {
    type Target = octocrab::Octocrab;

    fn deref(&self) -> &Self::Target {
        &self.0
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
impl GithubClient for OctocrabClient {
    async fn user_exists(&self, username: &str) -> Result<UserExistsResult, GithubClientError> {
        match self.0.users(username).profile().await {
            Ok(_) => Ok(UserExistsResult::Exists),
            Err(e) => {
                if let Some(status) = extract_status_code(&e) {
                    match status {
                        StatusCode::NOT_FOUND => Ok(UserExistsResult::NotFound),
                        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                            Ok(UserExistsResult::Unauthorized)
                        }
                        _ => Err(GithubClientError::ApiError(e.to_string())),
                    }
                } else {
                    Err(GithubClientError::ApiError(e.to_string()))
                }
            }
        }
    }

    async fn team_exists(
        &self,
        org: &str,
        team: &str,
    ) -> Result<TeamExistsResult, GithubClientError> {
        match self.0.teams(org).get(team).await {
            Ok(_) => Ok(TeamExistsResult::Exists),
            Err(e) => {
                if let Some(status) = extract_status_code(&e) {
                    match status {
                        StatusCode::NOT_FOUND => Ok(TeamExistsResult::NotFound),
                        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                            Ok(TeamExistsResult::Unauthorized)
                        }
                        _ => Err(GithubClientError::ApiError(e.to_string())),
                    }
                } else {
                    Err(GithubClientError::ApiError(e.to_string()))
                }
            }
        }
    }
}
