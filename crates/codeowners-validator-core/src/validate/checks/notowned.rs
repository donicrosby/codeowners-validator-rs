//! Not-owned files check.
//!
//! This check identifies files in the repository that are not covered
//! by any CODEOWNERS rule.

use super::{Check, CheckContext};
use crate::matching::Pattern;
use crate::parse::LineKind;
use crate::validate::{ValidationError, ValidationResult};
use std::path::Path;
use walkdir::WalkDir;

/// A check that identifies files without CODEOWNERS coverage.
///
/// This experimental check helps ensure that all files in the repository
/// have designated owners, which is important for code review workflows.
#[derive(Debug, Clone, Default)]
pub struct NotOwnedCheck;

impl NotOwnedCheck {
    /// Creates a new not-owned check.
    pub fn new() -> Self {
        Self
    }

    /// Lists all files in the repository, relative to the root.
    fn list_files(repo_path: &Path) -> Vec<String> {
        let mut files = Vec::new();

        for entry in WalkDir::new(repo_path)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                // Filter out hidden files and directories at the iterator level
                // This prevents descending into hidden directories
                // But always allow the root directory itself
                if e.depth() == 0 {
                    return true;
                }
                e.file_name()
                    .to_str()
                    .map(|s| !s.starts_with('.'))
                    .unwrap_or(false)
            })
            .filter_map(|e| e.ok())
        {
            // Skip the root directory itself
            if entry.path() == repo_path {
                continue;
            }

            // Only include files, not directories
            if !entry.file_type().is_file() {
                continue;
            }

            // Get path relative to repo root
            if let Ok(relative) = entry.path().strip_prefix(repo_path)
                && let Some(path_str) = relative.to_str()
            {
                // Normalize to forward slashes
                let normalized = path_str.replace('\\', "/");
                files.push(normalized);
            }
        }

        files
    }

    /// Checks if a file is covered by any of the patterns.
    fn is_file_covered(file: &str, patterns: &[Pattern]) -> bool {
        for pattern in patterns {
            if pattern.matches(file) {
                return true;
            }
        }
        false
    }

    /// Checks if a file matches any skip pattern.
    fn should_skip_file(file: &str, skip_patterns: &[Pattern]) -> bool {
        for pattern in skip_patterns {
            if pattern.matches(file) {
                return true;
            }
        }
        false
    }
}

impl Check for NotOwnedCheck {
    fn name(&self) -> &'static str {
        "notowned"
    }

    fn run(&self, ctx: &CheckContext) -> ValidationResult {
        let mut result = ValidationResult::new();

        // Compile all patterns from CODEOWNERS
        let mut patterns: Vec<Pattern> = Vec::new();
        for line in &ctx.file.lines {
            if let LineKind::Rule { pattern, .. } = &line.kind
                && let Some(compiled) = Pattern::new(&pattern.text)
            {
                patterns.push(compiled);
            }
        }

        // Compile skip patterns from config
        let skip_patterns: Vec<Pattern> = ctx
            .config
            .skip_patterns
            .iter()
            .filter_map(|p| Pattern::new(p))
            .collect();

        // List all files in the repository
        let files = Self::list_files(ctx.repo_path);

        // Check each file
        for file in files {
            // Skip files matching skip patterns
            if Self::should_skip_file(&file, &skip_patterns) {
                continue;
            }

            // Check if file is covered
            if !Self::is_file_covered(&file, &patterns) {
                result.add_error(ValidationError::file_not_owned(&file));
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
    use std::fs::{self, File};
    use tempfile::TempDir;

    fn setup_test_dir() -> TempDir {
        let dir = TempDir::new().unwrap();

        // Create some test files
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::create_dir_all(dir.path().join("docs")).unwrap();
        fs::create_dir_all(dir.path().join("tests")).unwrap();

        File::create(dir.path().join("src/main.rs")).unwrap();
        File::create(dir.path().join("src/lib.rs")).unwrap();
        File::create(dir.path().join("docs/README.md")).unwrap();
        File::create(dir.path().join("tests/test.rs")).unwrap();
        File::create(dir.path().join("Cargo.toml")).unwrap();

        dir
    }

    fn run_check(input: &str, repo_path: &Path) -> ValidationResult {
        let file = parse_codeowners(input).ast;
        let config = CheckConfig::new();
        let ctx = CheckContext::new(&file, repo_path, &config);
        NotOwnedCheck::new().run(&ctx)
    }

    fn run_check_with_config(
        input: &str,
        repo_path: &Path,
        config: CheckConfig,
    ) -> ValidationResult {
        let file = parse_codeowners(input).ast;
        let ctx = CheckContext::new(&file, repo_path, &config);
        NotOwnedCheck::new().run(&ctx)
    }

    #[test]
    fn all_files_covered() {
        let dir = setup_test_dir();
        let result = run_check("* @owner\n", dir.path());
        assert!(result.is_ok(), "Wildcard * should cover all files");
    }

    #[test]
    fn some_files_not_covered() {
        let dir = setup_test_dir();
        let result = run_check("*.rs @rust\n", dir.path());

        // README.md and Cargo.toml are not covered
        assert!(result.has_errors());

        let uncovered: Vec<_> = result
            .errors
            .iter()
            .filter_map(|e| match e {
                ValidationError::FileNotOwned { path } => Some(path.as_str()),
                _ => None,
            })
            .collect();

        assert!(uncovered.contains(&"docs/README.md"));
        assert!(uncovered.contains(&"Cargo.toml"));
    }

    #[test]
    fn directory_pattern_covers_contents() {
        let dir = setup_test_dir();
        let result = run_check(
            "/src/ @dev\n/docs/ @docs\n/tests/ @qa\n/Cargo.toml @dev\n",
            dir.path(),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn empty_codeowners_all_uncovered() {
        let dir = setup_test_dir();
        let result = run_check("", dir.path());

        // All files should be uncovered
        assert!(result.has_errors());
        assert!(result.errors.len() >= 5); // At least 5 files
    }

    #[test]
    fn skip_patterns_respected() {
        let dir = setup_test_dir();
        let config = CheckConfig::new()
            .with_skip_patterns(vec!["*.md".to_string(), "Cargo.toml".to_string()]);

        let result = run_check_with_config("*.rs @rust\n", dir.path(), config);

        // Only Cargo.toml and README.md should have been skipped
        // But wait, *.rs covers .rs files, so nothing should be uncovered
        // Actually tests/test.rs is covered by *.rs
        // Only docs/README.md and Cargo.toml would be uncovered but we skipped them
        assert!(result.is_ok());
    }

    #[test]
    fn specific_patterns() {
        let dir = setup_test_dir();
        let result = run_check(
            "/src/*.rs @dev\n/docs/*.md @docs\n/tests/*.rs @qa\n/Cargo.toml @dev\n",
            dir.path(),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn partial_coverage() {
        let dir = setup_test_dir();
        let result = run_check("/src/ @dev\n", dir.path());

        assert!(result.has_errors());

        // docs/, tests/, and Cargo.toml should be uncovered
        let uncovered_count = result
            .errors
            .iter()
            .filter(|e| matches!(e, ValidationError::FileNotOwned { .. }))
            .count();

        assert!(uncovered_count >= 3);
    }

    #[test]
    fn empty_directory() {
        let dir = TempDir::new().unwrap();
        let result = run_check("* @owner\n", dir.path());
        // No files means nothing to check
        assert!(result.is_ok());
    }

    #[test]
    fn multiple_patterns_combine() {
        let dir = setup_test_dir();
        let result = run_check("*.rs @rust\n*.md @docs\n*.toml @config\n", dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn hidden_files_skipped() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join(".git")).unwrap();
        File::create(dir.path().join(".git/config")).unwrap();
        File::create(dir.path().join(".gitignore")).unwrap();
        File::create(dir.path().join("visible.rs")).unwrap();

        // Only *.rs covered, but hidden files should not be checked
        let result = run_check("*.rs @owner\n", dir.path());
        assert!(result.is_ok());
    }
}
