//! CLI module for the CODEOWNERS validator.
//!
//! This module provides command-line argument parsing using Clap with
//! environment variable support, matching the configuration options
//! from the Go version of the codeowners-validator.

pub mod config;
pub mod output;

use clap::{Parser, ValueEnum};
use std::path::PathBuf;

/// CODEOWNERS file validator - validates GitHub CODEOWNERS files.
///
/// Ensures the correctness of your CODEOWNERS file by running various
/// checks against it. Supports both human-readable and JSON output formats.
#[derive(Parser, Debug)]
#[command(name = "codeowners-validator")]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Path to the repository root.
    #[arg(long, env = "REPOSITORY_PATH", default_value = ".")]
    pub repository_path: PathBuf,

    /// GitHub personal access token for owner validation.
    /// Required if the 'owners' check is enabled.
    #[arg(long, env = "GITHUB_ACCESS_TOKEN")]
    pub github_access_token: Option<String>,

    /// GitHub base URL for API requests (for GitHub Enterprise).
    #[arg(long, env = "GITHUB_BASE_URL", default_value = "https://api.github.com/")]
    pub github_base_url: String,

    /// GitHub upload URL (defaults to base URL if not specified).
    #[arg(long, env = "GITHUB_UPLOAD_URL")]
    pub github_upload_url: Option<String>,

    /// GitHub App ID for authentication (alternative to access token).
    #[arg(long, env = "GITHUB_APP_ID")]
    pub github_app_id: Option<u64>,

    /// GitHub App Installation ID (required when using App authentication).
    #[arg(long, env = "GITHUB_APP_INSTALLATION_ID")]
    pub github_app_installation_id: Option<u64>,

    /// GitHub App private key in PEM format (required when using App authentication).
    #[arg(long, env = "GITHUB_APP_PRIVATE_KEY")]
    pub github_app_private_key: Option<String>,

    /// Comma-separated list of checks to run.
    /// Possible values: files, owners, duppatterns, syntax
    #[arg(long, env = "CHECKS", value_delimiter = ',')]
    pub checks: Option<Vec<CheckKind>>,

    /// Comma-separated list of experimental checks to run.
    /// Possible values: notowned, avoid-shadowing
    #[arg(long, env = "EXPERIMENTAL_CHECKS", value_delimiter = ',')]
    pub experimental_checks: Option<Vec<ExperimentalCheckKind>>,

    /// Failure level for validation issues.
    /// 'warning' treats both errors and warnings as failures.
    /// 'error' only treats errors as failures.
    #[arg(long, env = "CHECK_FAILURE_LEVEL", default_value = "warning")]
    pub check_failure_level: FailureLevel,

    /// Repository in 'owner/repo' format for owner validation.
    #[arg(long, env = "OWNER_CHECKER_REPOSITORY")]
    pub owner_checker_repository: Option<String>,

    /// Comma-separated list of owners to ignore during validation.
    #[arg(long, env = "OWNER_CHECKER_IGNORED_OWNERS", value_delimiter = ',')]
    pub owner_checker_ignored_owners: Option<Vec<String>>,

    /// Allow patterns without owners in the CODEOWNERS file.
    #[arg(long, env = "OWNER_CHECKER_ALLOW_UNOWNED_PATTERNS", default_value = "true")]
    pub owner_checker_allow_unowned_patterns: bool,

    /// Require all owners to be teams (@org/team), not individual users.
    #[arg(long, env = "OWNER_CHECKER_OWNERS_MUST_BE_TEAMS", default_value = "false")]
    pub owner_checker_owners_must_be_teams: bool,

    /// Comma-separated patterns to skip in the not-owned checker.
    #[arg(long, env = "NOT_OWNED_CHECKER_SKIP_PATTERNS", value_delimiter = ',')]
    pub not_owned_checker_skip_patterns: Option<Vec<String>>,

    /// Output validation results as JSON instead of human-readable format.
    #[arg(long, short = 'j')]
    pub json: bool,

    /// Increase verbosity level (-v for debug, -vv for trace).
    #[arg(long, short = 'v', action = clap::ArgAction::Count)]
    pub verbose: u8,
}

/// Standard validation checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "lowercase")]
pub enum CheckKind {
    /// Check that patterns match existing files.
    Files,
    /// Check that owners exist on GitHub.
    Owners,
    /// Check for duplicate patterns.
    Duppatterns,
    /// Check for syntax errors.
    Syntax,
}

impl CheckKind {
    /// Returns all standard checks.
    pub fn all() -> Vec<Self> {
        vec![Self::Files, Self::Owners, Self::Duppatterns, Self::Syntax]
    }
}

/// Experimental validation checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum ExperimentalCheckKind {
    /// Check for files not covered by any CODEOWNERS rule.
    Notowned,
    /// Check for patterns that shadow earlier patterns.
    AvoidShadowing,
}

/// Failure level for validation issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
#[value(rename_all = "lowercase")]
pub enum FailureLevel {
    /// Treat both warnings and errors as failures (exit code 3).
    #[default]
    Warning,
    /// Only treat errors as failures.
    Error,
}

impl Args {
    /// Returns the checks to run, defaulting to all standard checks.
    pub fn effective_checks(&self) -> Vec<CheckKind> {
        self.checks.clone().unwrap_or_else(CheckKind::all)
    }

    /// Returns the experimental checks to run (empty by default).
    pub fn effective_experimental_checks(&self) -> Vec<ExperimentalCheckKind> {
        self.experimental_checks.clone().unwrap_or_default()
    }

    /// Returns true if a specific check should be run.
    pub fn should_run_check(&self, check: CheckKind) -> bool {
        self.effective_checks().contains(&check)
    }

    /// Returns true if a specific experimental check should be run.
    pub fn should_run_experimental_check(&self, check: ExperimentalCheckKind) -> bool {
        self.effective_experimental_checks().contains(&check)
    }

    /// Returns true if GitHub authentication is configured.
    pub fn has_github_auth(&self) -> bool {
        self.github_access_token.is_some()
            || (self.github_app_id.is_some()
                && self.github_app_installation_id.is_some()
                && self.github_app_private_key.is_some())
    }

    /// Returns true if GitHub App authentication is configured.
    pub fn has_github_app_auth(&self) -> bool {
        self.github_app_id.is_some()
            && self.github_app_installation_id.is_some()
            && self.github_app_private_key.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_checks() {
        let args = Args::parse_from(["codeowners-validator"]);
        let checks = args.effective_checks();
        assert_eq!(checks.len(), 4);
        assert!(checks.contains(&CheckKind::Files));
        assert!(checks.contains(&CheckKind::Owners));
        assert!(checks.contains(&CheckKind::Duppatterns));
        assert!(checks.contains(&CheckKind::Syntax));
    }

    #[test]
    fn test_specific_checks() {
        let args = Args::parse_from(["codeowners-validator", "--checks", "syntax,files"]);
        let checks = args.effective_checks();
        assert_eq!(checks.len(), 2);
        assert!(checks.contains(&CheckKind::Syntax));
        assert!(checks.contains(&CheckKind::Files));
    }

    #[test]
    fn test_experimental_checks() {
        let args = Args::parse_from([
            "codeowners-validator",
            "--experimental-checks",
            "notowned,avoid-shadowing",
        ]);
        let checks = args.effective_experimental_checks();
        assert_eq!(checks.len(), 2);
        assert!(checks.contains(&ExperimentalCheckKind::Notowned));
        assert!(checks.contains(&ExperimentalCheckKind::AvoidShadowing));
    }

    #[test]
    fn test_default_failure_level() {
        let args = Args::parse_from(["codeowners-validator"]);
        assert_eq!(args.check_failure_level, FailureLevel::Warning);
    }

    #[test]
    fn test_error_failure_level() {
        let args = Args::parse_from(["codeowners-validator", "--check-failure-level", "error"]);
        assert_eq!(args.check_failure_level, FailureLevel::Error);
    }

    #[test]
    fn test_json_output_flag() {
        let args = Args::parse_from(["codeowners-validator", "--json"]);
        assert!(args.json);

        let args = Args::parse_from(["codeowners-validator", "-j"]);
        assert!(args.json);
    }

    #[test]
    fn test_verbose_flag() {
        let args = Args::parse_from(["codeowners-validator"]);
        assert_eq!(args.verbose, 0);

        let args = Args::parse_from(["codeowners-validator", "-v"]);
        assert_eq!(args.verbose, 1);

        let args = Args::parse_from(["codeowners-validator", "-vv"]);
        assert_eq!(args.verbose, 2);
    }

    #[test]
    fn test_default_paths() {
        let args = Args::parse_from(["codeowners-validator"]);
        assert_eq!(args.repository_path, PathBuf::from("."));
        assert_eq!(args.github_base_url, "https://api.github.com/");
    }

    #[test]
    fn test_github_auth_detection() {
        // No auth
        let args = Args::parse_from(["codeowners-validator"]);
        assert!(!args.has_github_auth());

        // Token auth
        let args = Args::parse_from([
            "codeowners-validator",
            "--github-access-token",
            "ghp_test",
        ]);
        assert!(args.has_github_auth());
        assert!(!args.has_github_app_auth());
    }

    #[test]
    fn test_ignored_owners() {
        let args = Args::parse_from([
            "codeowners-validator",
            "--owner-checker-ignored-owners",
            "@ghost,@bot",
        ]);
        let ignored = args.owner_checker_ignored_owners.unwrap();
        assert_eq!(ignored.len(), 2);
        assert!(ignored.contains(&"@ghost".to_string()));
        assert!(ignored.contains(&"@bot".to_string()));
    }

    #[test]
    fn test_should_run_check() {
        let args = Args::parse_from(["codeowners-validator", "--checks", "syntax"]);
        assert!(args.should_run_check(CheckKind::Syntax));
        assert!(!args.should_run_check(CheckKind::Files));
    }
}
