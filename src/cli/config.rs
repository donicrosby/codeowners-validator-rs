//! Configuration handling for the CLI.
//!
//! This module converts CLI arguments into the library's configuration types
//! and handles GitHub authentication setup.

use crate::cli::{Args, CheckKind, ExperimentalCheckKind, FailureLevel};
use crate::validate::checks::CheckConfig;
use crate::validate::Severity;
use jsonwebtoken::EncodingKey;
use octocrab::models::{AppId, InstallationId};
use octocrab::Octocrab;
use std::path::Path;
use thiserror::Error;

/// Errors that can occur during configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Missing required configuration.
    #[error("missing required configuration: {0}")]
    MissingRequired(String),

    /// Invalid configuration value.
    #[error("invalid configuration: {0}")]
    Invalid(String),

    /// GitHub authentication error.
    #[error("GitHub authentication error: {0}")]
    GitHubAuth(String),

    /// Failed to read CODEOWNERS file.
    #[error("failed to read CODEOWNERS file: {0}")]
    ReadCodeowners(String),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Application exit codes matching the Go version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    /// Validation passed successfully.
    Success = 0,
    /// Application startup failed (wrong configuration or internal error).
    StartupFailure = 1,
    /// Application terminated by signal (SIGINT/SIGTERM).
    Terminated = 2,
    /// Validation failed (checks found issues).
    ValidationFailed = 3,
}

impl From<ExitCode> for i32 {
    fn from(code: ExitCode) -> Self {
        code as i32
    }
}

/// Validated and processed configuration for running the validator.
#[derive(Debug)]
pub struct ValidatedConfig {
    /// Path to the repository root.
    pub repo_path: std::path::PathBuf,
    /// Path to the CODEOWNERS file.
    pub codeowners_path: std::path::PathBuf,
    /// Configuration for the check runner.
    pub check_config: CheckConfig,
    /// Which standard checks to run.
    pub checks: Vec<CheckKind>,
    /// Which experimental checks to run.
    pub experimental_checks: Vec<ExperimentalCheckKind>,
    /// Failure level for determining exit code.
    pub failure_level: FailureLevel,
    /// Whether to output JSON.
    pub json_output: bool,
}

impl ValidatedConfig {
    /// Creates a validated configuration from CLI arguments.
    pub fn from_args(args: &Args) -> Result<Self, ConfigError> {
        let repo_path = args.repository_path.canonicalize().map_err(|e| {
            ConfigError::Invalid(format!(
                "repository path '{}' is invalid: {}",
                args.repository_path.display(),
                e
            ))
        })?;

        // Find the CODEOWNERS file
        let codeowners_path = find_codeowners_file(&repo_path)?;

        // Validate that owners check has required config
        let checks = args.effective_checks();
        if checks.contains(&CheckKind::Owners) && args.owner_checker_repository.is_none() {
            return Err(ConfigError::MissingRequired(
                "OWNER_CHECKER_REPOSITORY is required when 'owners' check is enabled".to_string(),
            ));
        }

        // Build check config
        let mut check_config = CheckConfig::new();

        if let Some(ref ignored) = args.owner_checker_ignored_owners {
            check_config = check_config.with_ignored_owners(ignored.iter().cloned().collect());
        }

        check_config = check_config
            .with_owners_must_be_teams(args.owner_checker_owners_must_be_teams)
            .with_allow_unowned_patterns(args.owner_checker_allow_unowned_patterns);

        if let Some(ref patterns) = args.not_owned_checker_skip_patterns {
            check_config = check_config.with_skip_patterns(patterns.clone());
        }

        if let Some(ref repo) = args.owner_checker_repository {
            check_config = check_config.with_repository(repo.clone());
        }

        Ok(Self {
            repo_path,
            codeowners_path,
            check_config,
            checks,
            experimental_checks: args.effective_experimental_checks(),
            failure_level: args.check_failure_level,
            json_output: args.json,
        })
    }

    /// Determines the exit code based on validation results.
    pub fn exit_code_for_results(
        &self,
        has_errors: bool,
        has_warnings: bool,
    ) -> ExitCode {
        if has_errors {
            return ExitCode::ValidationFailed;
        }

        match self.failure_level {
            FailureLevel::Warning if has_warnings => ExitCode::ValidationFailed,
            _ => ExitCode::Success,
        }
    }
}

/// Finds the CODEOWNERS file in the repository.
///
/// Searches in the following locations (in order):
/// 1. `.github/CODEOWNERS`
/// 2. `CODEOWNERS`
/// 3. `docs/CODEOWNERS`
pub fn find_codeowners_file(repo_path: &Path) -> Result<std::path::PathBuf, ConfigError> {
    let locations = [
        repo_path.join(".github/CODEOWNERS"),
        repo_path.join("CODEOWNERS"),
        repo_path.join("docs/CODEOWNERS"),
    ];

    for path in &locations {
        if path.exists() {
            return Ok(path.clone());
        }
    }

    Err(ConfigError::ReadCodeowners(format!(
        "CODEOWNERS file not found in repository '{}'. Searched in: .github/CODEOWNERS, CODEOWNERS, docs/CODEOWNERS",
        repo_path.display()
    )))
}

/// Creates an authenticated Octocrab client from CLI arguments.
pub async fn create_octocrab(args: &Args) -> Result<Option<Octocrab>, ConfigError> {
    // Check if any auth is configured
    if !args.has_github_auth() {
        return Ok(None);
    }

    let base_url = if args.github_base_url != "https://api.github.com/" {
        Some(args.github_base_url.as_str())
    } else {
        None
    };

    // Configure authentication
    if args.has_github_app_auth() {
        // GitHub App authentication
        let app_id = AppId(args.github_app_id.unwrap());
        let installation_id = InstallationId(args.github_app_installation_id.unwrap());
        let private_key = args.github_app_private_key.as_ref().unwrap();

        // Create encoding key from PEM
        let key = EncodingKey::from_rsa_pem(private_key.as_bytes())
            .map_err(|e| ConfigError::GitHubAuth(format!("invalid private key: {}", e)))?;

        // Build app-authenticated client
        let mut app_builder = Octocrab::builder().app(app_id, key);
        if let Some(url) = base_url {
            app_builder = app_builder
                .base_uri(url)
                .map_err(|e| ConfigError::GitHubAuth(format!("invalid base URL: {}", e)))?;
        }
        let app_client = app_builder
            .build()
            .map_err(|e| ConfigError::GitHubAuth(format!("failed to create app client: {}", e)))?;

        // Get installation-specific client
        let client = app_client
            .installation(installation_id)
            .map_err(|e| ConfigError::GitHubAuth(format!("failed to get installation client: {}", e)))?;

        Ok(Some(client))
    } else if let Some(ref token) = args.github_access_token {
        // Personal access token authentication
        let mut builder = Octocrab::builder();
        if let Some(url) = base_url {
            builder = builder
                .base_uri(url)
                .map_err(|e| ConfigError::GitHubAuth(format!("invalid base URL: {}", e)))?;
        }
        let client = builder
            .personal_token(token.clone())
            .build()
            .map_err(|e| ConfigError::GitHubAuth(format!("failed to build client: {}", e)))?;

        Ok(Some(client))
    } else {
        Ok(None)
    }
}

/// Determines if validation failed based on results and failure level.
pub fn has_failures(
    errors: impl Iterator<Item = Severity>,
    failure_level: FailureLevel,
) -> bool {
    for severity in errors {
        match (severity, failure_level) {
            (Severity::Error, _) => return true,
            (Severity::Warning, FailureLevel::Warning) => return true,
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join(".github")).unwrap();
        fs::write(dir.path().join(".github/CODEOWNERS"), "* @owner\n").unwrap();
        dir
    }

    #[test]
    fn test_find_codeowners_github_dir() {
        let dir = create_test_repo();
        let path = find_codeowners_file(dir.path()).unwrap();
        assert!(path.ends_with(".github/CODEOWNERS"));
    }

    #[test]
    fn test_find_codeowners_root() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("CODEOWNERS"), "* @owner\n").unwrap();
        let path = find_codeowners_file(dir.path()).unwrap();
        assert!(path.ends_with("CODEOWNERS"));
        assert!(!path.to_string_lossy().contains(".github"));
    }

    #[test]
    fn test_find_codeowners_docs() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("docs")).unwrap();
        fs::write(dir.path().join("docs/CODEOWNERS"), "* @owner\n").unwrap();
        let path = find_codeowners_file(dir.path()).unwrap();
        assert!(path.ends_with("docs/CODEOWNERS"));
    }

    #[test]
    fn test_find_codeowners_not_found() {
        let dir = TempDir::new().unwrap();
        let result = find_codeowners_file(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_exit_codes() {
        assert_eq!(i32::from(ExitCode::Success), 0);
        assert_eq!(i32::from(ExitCode::StartupFailure), 1);
        assert_eq!(i32::from(ExitCode::Terminated), 2);
        assert_eq!(i32::from(ExitCode::ValidationFailed), 3);
    }

    #[test]
    fn test_has_failures_error() {
        let errors = vec![Severity::Error];
        assert!(has_failures(errors.into_iter(), FailureLevel::Warning));
        
        let errors = vec![Severity::Error];
        assert!(has_failures(errors.into_iter(), FailureLevel::Error));
    }

    #[test]
    fn test_has_failures_warning() {
        let warnings = vec![Severity::Warning];
        assert!(has_failures(warnings.into_iter(), FailureLevel::Warning));
        
        let warnings = vec![Severity::Warning];
        assert!(!has_failures(warnings.into_iter(), FailureLevel::Error));
    }

    #[test]
    fn test_has_failures_empty() {
        let empty: Vec<Severity> = vec![];
        assert!(!has_failures(empty.into_iter(), FailureLevel::Warning));
    }

    #[test]
    fn test_exit_code_for_results() {
        use crate::cli::Args;
        use clap::Parser;

        let dir = create_test_repo();
        let args = Args::parse_from([
            "codeowners-validator",
            "--repository-path",
            dir.path().to_str().unwrap(),
            "--owner-checker-repository",
            "owner/repo",
        ]);
        let config = ValidatedConfig::from_args(&args).unwrap();

        // No issues
        assert_eq!(
            config.exit_code_for_results(false, false),
            ExitCode::Success
        );

        // Errors always fail
        assert_eq!(
            config.exit_code_for_results(true, false),
            ExitCode::ValidationFailed
        );

        // Warnings fail with warning level
        assert_eq!(
            config.exit_code_for_results(false, true),
            ExitCode::ValidationFailed
        );
    }

    #[test]
    fn test_exit_code_error_level() {
        use crate::cli::Args;
        use clap::Parser;

        let dir = create_test_repo();
        let args = Args::parse_from([
            "codeowners-validator",
            "--repository-path",
            dir.path().to_str().unwrap(),
            "--owner-checker-repository",
            "owner/repo",
            "--check-failure-level",
            "error",
        ]);
        let config = ValidatedConfig::from_args(&args).unwrap();

        // Warnings don't fail with error level
        assert_eq!(
            config.exit_code_for_results(false, true),
            ExitCode::Success
        );

        // Errors still fail
        assert_eq!(
            config.exit_code_for_results(true, true),
            ExitCode::ValidationFailed
        );
    }

    #[test]
    fn test_validated_config_missing_owner_repo() {
        use crate::cli::Args;
        use clap::Parser;

        let dir = create_test_repo();
        let args = Args::parse_from([
            "codeowners-validator",
            "--repository-path",
            dir.path().to_str().unwrap(),
            "--checks",
            "owners",
        ]);
        let result = ValidatedConfig::from_args(&args);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("OWNER_CHECKER_REPOSITORY"));
    }

    #[test]
    fn test_validated_config_without_owners_check() {
        use crate::cli::Args;
        use clap::Parser;

        let dir = create_test_repo();
        let args = Args::parse_from([
            "codeowners-validator",
            "--repository-path",
            dir.path().to_str().unwrap(),
            "--checks",
            "syntax,files",
        ]);
        // Should succeed without owner checker repository
        let result = ValidatedConfig::from_args(&args);
        assert!(result.is_ok());
    }
}
