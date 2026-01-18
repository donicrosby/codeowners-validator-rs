//! Check traits and runner for CODEOWNERS validation.
//!
//! This module provides a trait-based system for implementing validation checks
//! that can be composed and run together.

mod duppatterns;
mod files;
mod notowned;
mod owners;
mod shadowing;
mod syntax;

pub use duppatterns::DupPatternsCheck;
pub use files::FilesCheck;
pub use notowned::NotOwnedCheck;
pub use owners::OwnersCheck;
pub use shadowing::AvoidShadowingCheck;
pub use syntax::SyntaxCheck;

use crate::parse::CodeownersFile;
use crate::validate::ValidationResult;
use crate::validate::github_client::GithubClient;
use async_trait::async_trait;
use log::{debug, info};
use std::collections::HashSet;
use std::path::Path;

/// Configuration options for validation checks.
#[derive(Debug, Clone, Default)]
pub struct CheckConfig {
    /// Owners that should be skipped during validation.
    pub ignored_owners: HashSet<String>,
    /// If true, only team owners (@org/team) are allowed, not individual users.
    pub owners_must_be_teams: bool,
    /// If true, patterns without owners are allowed.
    pub allow_unowned_patterns: bool,
    /// Patterns to skip when checking for unowned files.
    pub skip_patterns: Vec<String>,
    /// The repository in "owner/repo" format, used for owner validation.
    pub repository: Option<String>,
}

impl CheckConfig {
    /// Creates a new configuration with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the ignored owners.
    pub fn with_ignored_owners(mut self, owners: HashSet<String>) -> Self {
        self.ignored_owners = owners;
        self
    }

    /// Sets whether owners must be teams.
    pub fn with_owners_must_be_teams(mut self, value: bool) -> Self {
        self.owners_must_be_teams = value;
        self
    }

    /// Sets whether unowned patterns are allowed.
    pub fn with_allow_unowned_patterns(mut self, value: bool) -> Self {
        self.allow_unowned_patterns = value;
        self
    }

    /// Sets the patterns to skip for the not-owned check.
    pub fn with_skip_patterns(mut self, patterns: Vec<String>) -> Self {
        self.skip_patterns = patterns;
        self
    }

    /// Sets the repository for owner validation.
    pub fn with_repository(mut self, repo: impl Into<String>) -> Self {
        self.repository = Some(repo.into());
        self
    }
}

/// Context provided to synchronous checks.
#[derive(Debug)]
pub struct CheckContext<'a> {
    /// The parsed CODEOWNERS file.
    pub file: &'a CodeownersFile,
    /// Path to the repository root.
    pub repo_path: &'a Path,
    /// Configuration options.
    pub config: &'a CheckConfig,
}

impl<'a> CheckContext<'a> {
    /// Creates a new check context.
    pub fn new(file: &'a CodeownersFile, repo_path: &'a Path, config: &'a CheckConfig) -> Self {
        Self {
            file,
            repo_path,
            config,
        }
    }
}

/// Context provided to asynchronous checks that need GitHub API access.
pub struct AsyncCheckContext<'a> {
    /// The parsed CODEOWNERS file.
    pub file: &'a CodeownersFile,
    /// Path to the repository root.
    pub repo_path: &'a Path,
    /// Configuration options.
    pub config: &'a CheckConfig,
    /// The GitHub client for API calls.
    pub github_client: &'a dyn GithubClient,
}

impl<'a> AsyncCheckContext<'a> {
    /// Creates a new async check context.
    pub fn new(
        file: &'a CodeownersFile,
        repo_path: &'a Path,
        config: &'a CheckConfig,
        github_client: &'a dyn GithubClient,
    ) -> Self {
        Self {
            file,
            repo_path,
            config,
            github_client,
        }
    }
}

impl std::fmt::Debug for AsyncCheckContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsyncCheckContext")
            .field("file", &self.file)
            .field("repo_path", &self.repo_path)
            .field("config", &self.config)
            .field("github_client", &"<dyn GithubClient>")
            .finish()
    }
}

/// A synchronous validation check.
pub trait Check: Send + Sync {
    /// Returns the name of this check.
    fn name(&self) -> &'static str;

    /// Runs the check and returns validation results.
    fn run(&self, ctx: &CheckContext) -> ValidationResult;
}

/// An asynchronous validation check that requires GitHub API access.
#[async_trait]
pub trait AsyncCheck: Send + Sync {
    /// Returns the name of this check.
    fn name(&self) -> &'static str;

    /// Runs the check asynchronously and returns validation results.
    async fn run(&self, ctx: &AsyncCheckContext<'_>) -> ValidationResult;
}

/// Runs multiple validation checks and collects results.
#[derive(Default)]
pub struct CheckRunner {
    checks: Vec<Box<dyn Check>>,
    async_checks: Vec<Box<dyn AsyncCheck>>,
}

impl CheckRunner {
    /// Creates a new check runner.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a check runner with all built-in checks.
    pub fn with_all_checks() -> Self {
        let mut runner = Self::new();
        runner.add_check(SyntaxCheck::new());
        runner.add_check(DupPatternsCheck::new());
        runner.add_check(FilesCheck::new());
        runner.add_check(NotOwnedCheck::new());
        runner.add_check(AvoidShadowingCheck::new());
        runner.add_async_check(OwnersCheck::new());
        runner
    }

    /// Adds a synchronous check.
    pub fn add_check<C: Check + 'static>(&mut self, check: C) {
        self.checks.push(Box::new(check));
    }

    /// Adds an asynchronous check.
    pub fn add_async_check<C: AsyncCheck + 'static>(&mut self, check: C) {
        self.async_checks.push(Box::new(check));
    }

    /// Runs all synchronous checks and returns combined results.
    pub fn run_sync(
        &self,
        file: &CodeownersFile,
        repo_path: &Path,
        config: &CheckConfig,
    ) -> ValidationResult {
        info!("Running {} synchronous checks", self.checks.len());
        let ctx = CheckContext::new(file, repo_path, config);
        let mut result = ValidationResult::new();

        for check in &self.checks {
            debug!("Running check: {}", check.name());
            let check_result = check.run(&ctx);
            debug!(
                "Check '{}' found {} issues",
                check.name(),
                check_result.errors.len()
            );
            result.merge(check_result);
        }

        info!(
            "Synchronous checks complete: {} total issues",
            result.errors.len()
        );
        result
    }

    /// Runs all checks (both sync and async) and returns combined results.
    pub async fn run_all(
        &self,
        file: &CodeownersFile,
        repo_path: &Path,
        config: &CheckConfig,
        github_client: Option<&dyn GithubClient>,
    ) -> ValidationResult {
        info!(
            "Running all checks ({} sync, {} async)",
            self.checks.len(),
            self.async_checks.len()
        );
        let ctx = CheckContext::new(file, repo_path, config);
        let mut result = ValidationResult::new();

        // Run synchronous checks
        for check in &self.checks {
            debug!("Running sync check: {}", check.name());
            let check_result = check.run(&ctx);
            debug!(
                "Check '{}' found {} issues",
                check.name(),
                check_result.errors.len()
            );
            result.merge(check_result);
        }

        // Run asynchronous checks if github_client is provided
        if let Some(client) = github_client {
            let async_ctx = AsyncCheckContext::new(file, repo_path, config, client);
            for check in &self.async_checks {
                debug!("Running async check: {}", check.name());
                let check_result = check.run(&async_ctx).await;
                debug!(
                    "Check '{}' found {} issues",
                    check.name(),
                    check_result.errors.len()
                );
                result.merge(check_result);
            }
        } else {
            debug!(
                "No GitHub client provided, skipping {} async checks",
                self.async_checks.len()
            );
        }

        info!("All checks complete: {} total issues", result.errors.len());
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_codeowners;
    use std::path::PathBuf;

    #[test]
    fn check_config_builder() {
        let config = CheckConfig::new()
            .with_owners_must_be_teams(true)
            .with_repository("owner/repo");

        assert!(config.owners_must_be_teams);
        assert_eq!(config.repository, Some("owner/repo".to_string()));
    }

    #[test]
    fn check_context_creation() {
        let file = parse_codeowners("*.rs @owner\n").ast;
        let path = PathBuf::from("/repo");
        let config = CheckConfig::new();

        let ctx = CheckContext::new(&file, &path, &config);
        assert_eq!(ctx.repo_path, Path::new("/repo"));
    }

    #[test]
    fn check_runner_creation() {
        let runner = CheckRunner::new();
        assert!(runner.checks.is_empty());
        assert!(runner.async_checks.is_empty());
    }

    #[test]
    fn check_runner_with_all_checks() {
        let runner = CheckRunner::with_all_checks();
        assert_eq!(runner.checks.len(), 5); // syntax, dup, files, notowned, shadowing
        assert_eq!(runner.async_checks.len(), 1); // owners
    }
}
