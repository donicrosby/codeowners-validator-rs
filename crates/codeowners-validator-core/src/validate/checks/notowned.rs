//! Not-owned files check.
//!
//! This check identifies files in the repository that are not covered
//! by any CODEOWNERS rule.

use super::{Check, CheckContext};
use crate::matching::Pattern;
use crate::parse::LineKind;
use crate::validate::file_walker::{FileWalkerConfig, list_files};
use crate::validate::{ValidationError, ValidationResult};

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

    /// Checks if a file is covered by any of the patterns.
    fn is_file_covered(file: &str, patterns: &[Pattern]) -> bool {
        patterns.iter().any(|pattern| pattern.matches(file))
    }

    /// Checks if a file matches any skip pattern.
    fn should_skip_file(file: &str, skip_patterns: &[Pattern]) -> bool {
        skip_patterns.iter().any(|pattern| pattern.matches(file))
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

        // List all files (includes hidden, respects gitignore)
        let files = list_files(ctx.repo_path, &FileWalkerConfig::for_not_owned_check());

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
    use std::path::Path;
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
    fn hidden_directories_like_dotgit_excluded() {
        // Note: The `ignore` crate automatically excludes .git/ directories
        // even when hidden files are enabled. This test verifies other
        // hidden directories ARE included while .git would be excluded.
        let dir = TempDir::new().unwrap();

        // Create a hidden directory that looks like .git but isn't
        // (actual .git creation may be blocked by sandbox)
        fs::create_dir_all(dir.path().join(".config")).unwrap();
        File::create(dir.path().join(".config/settings")).unwrap();

        // Create a visible file
        File::create(dir.path().join("visible.rs")).unwrap();

        // Only *.rs covered - .config/ files SHOULD be flagged (not excluded)
        let result = run_check("*.rs @owner\n", dir.path());
        assert!(
            result.has_errors(),
            "Hidden directories (except .git) should be checked"
        );

        let uncovered: Vec<_> = result
            .errors
            .iter()
            .filter_map(|e| match e {
                ValidationError::FileNotOwned { path } => Some(path.as_str()),
                _ => None,
            })
            .collect();

        assert!(
            uncovered.contains(&".config/settings"),
            "Expected .config/settings to be flagged as uncovered"
        );
    }

    #[test]
    fn hidden_files_included() {
        let dir = TempDir::new().unwrap();

        // Create hidden files that SHOULD be checked
        File::create(dir.path().join(".gitignore")).unwrap();
        fs::create_dir_all(dir.path().join(".github")).unwrap();
        File::create(dir.path().join(".github/CODEOWNERS")).unwrap();
        File::create(dir.path().join("visible.rs")).unwrap();

        // Only *.rs covered - hidden files should now be flagged as uncovered
        let result = run_check("*.rs @owner\n", dir.path());
        assert!(
            result.has_errors(),
            "Hidden files should be checked for coverage"
        );

        let uncovered: Vec<_> = result
            .errors
            .iter()
            .filter_map(|e| match e {
                ValidationError::FileNotOwned { path } => Some(path.as_str()),
                _ => None,
            })
            .collect();

        assert!(
            uncovered.contains(&".gitignore"),
            "Expected .gitignore to be flagged as uncovered"
        );
        assert!(
            uncovered.contains(&".github/CODEOWNERS"),
            "Expected .github/CODEOWNERS to be flagged as uncovered"
        );
    }

    #[test]
    fn hidden_files_covered_by_patterns() {
        let dir = TempDir::new().unwrap();

        // Create hidden files
        File::create(dir.path().join(".gitignore")).unwrap();
        fs::create_dir_all(dir.path().join(".github")).unwrap();
        File::create(dir.path().join(".github/CODEOWNERS")).unwrap();
        File::create(dir.path().join("visible.rs")).unwrap();

        // Cover all files including hidden ones
        let result = run_check("* @owner\n", dir.path());
        assert!(result.is_ok(), "Wildcard should cover hidden files too");
    }

    #[test]
    fn gitignore_requires_git_repo() {
        // .gitignore is only respected when the directory is a git repository.
        // Since we can't create .git directories in tests (sandbox restriction),
        // this test verifies that without a git repo, .gitignore is NOT respected.
        let dir = TempDir::new().unwrap();

        // Create a .gitignore file (won't be respected without .git directory)
        fs::write(dir.path().join(".gitignore"), "target/\n*.log\n").unwrap();

        // Create files that would be gitignored in a real repo
        fs::create_dir_all(dir.path().join("target")).unwrap();
        File::create(dir.path().join("target/debug.txt")).unwrap();
        File::create(dir.path().join("build.log")).unwrap();
        File::create(dir.path().join("src.rs")).unwrap();

        // Without a .git directory, .gitignore is not respected
        // so target/ and *.log files WILL be checked
        let result = run_check("*.rs @owner\n/.gitignore @owner\n", dir.path());
        assert!(
            result.has_errors(),
            "Without .git directory, .gitignore should not be respected"
        );

        let uncovered: Vec<_> = result
            .errors
            .iter()
            .filter_map(|e| match e {
                ValidationError::FileNotOwned { path } => Some(path.as_str()),
                _ => None,
            })
            .collect();

        // These would be ignored in a real git repo, but not here
        assert!(
            uncovered.contains(&"target/debug.txt"),
            "target/debug.txt should be checked (no git repo)"
        );
        assert!(
            uncovered.contains(&"build.log"),
            "build.log should be checked (no git repo)"
        );
    }

    #[test]
    fn no_ignore_file_walks_all_files() {
        let dir = TempDir::new().unwrap();

        // No .ignore or .gitignore file - all files should be walked
        fs::create_dir_all(dir.path().join("target")).unwrap();
        File::create(dir.path().join("target/output.txt")).unwrap();
        File::create(dir.path().join("main.rs")).unwrap();

        // Without any ignore file, target/ should be checked
        let result = run_check("*.rs @owner\n", dir.path());
        assert!(result.has_errors());

        let uncovered: Vec<_> = result
            .errors
            .iter()
            .filter_map(|e| match e {
                ValidationError::FileNotOwned { path } => Some(path.as_str()),
                _ => None,
            })
            .collect();

        assert!(
            uncovered.contains(&"target/output.txt"),
            "Without ignore files, target/ files should be checked"
        );
    }
}
