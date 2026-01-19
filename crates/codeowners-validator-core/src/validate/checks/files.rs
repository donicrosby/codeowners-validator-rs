//! Files existence check.
//!
//! This check verifies that patterns in CODEOWNERS actually match files in the repository.

use super::{Check, CheckContext};
use crate::matching::Pattern;
use crate::parse::LineKind;
use crate::validate::{ValidationError, ValidationResult};
use log::{debug, trace};
use std::path::Path;
use walkdir::WalkDir;

/// A check that verifies patterns match existing files.
///
/// Patterns that don't match any files in the repository may indicate:
/// - Typos in the pattern
/// - Files that have been deleted
/// - Incorrect path assumptions
#[derive(Debug, Clone, Default)]
pub struct FilesCheck;

impl FilesCheck {
    /// Creates a new files existence check.
    pub fn new() -> Self {
        Self
    }

    /// Lists all files in the repository, relative to the root.
    fn list_files(repo_path: &Path) -> Vec<String> {
        debug!("Listing files in repository: {:?}", repo_path);
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

            // Get path relative to repo root
            if let Ok(relative) = entry.path().strip_prefix(repo_path)
                && let Some(path_str) = relative.to_str()
            {
                // Normalize to forward slashes
                let normalized = path_str.replace('\\', "/");
                files.push(normalized);
            }
        }

        debug!("Found {} files in repository", files.len());
        trace!("Files: {:?}", files);
        files
    }

    /// Checks if a pattern matches any file in the given list.
    fn pattern_matches_any(pattern: &Pattern, files: &[String]) -> bool {
        for file in files {
            if pattern.matches(file) {
                return true;
            }
        }
        false
    }
}

impl Check for FilesCheck {
    fn name(&self) -> &'static str {
        "files"
    }

    fn run(&self, ctx: &CheckContext) -> ValidationResult {
        debug!("Running files check");
        let mut result = ValidationResult::new();

        // List all files in the repository
        let files = Self::list_files(ctx.repo_path);

        // Check each pattern
        for line in &ctx.file.lines {
            if let LineKind::Rule { pattern, .. } = &line.kind {
                trace!("Checking pattern: {}", pattern.text);
                // Compile the pattern
                if let Some(compiled) = Pattern::new(&pattern.text)
                    && !Self::pattern_matches_any(&compiled, &files)
                {
                    debug!("Pattern '{}' does not match any files", pattern.text);
                    result.add_error(ValidationError::pattern_not_matching(
                        &pattern.text,
                        pattern.span,
                    ));
                }
                // If pattern compilation fails, that's a syntax error handled elsewhere
            }
        }

        debug!("Files check complete: {} errors found", result.errors.len());
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

        File::create(dir.path().join("src/main.rs")).unwrap();
        File::create(dir.path().join("src/lib.rs")).unwrap();
        File::create(dir.path().join("docs/README.md")).unwrap();
        File::create(dir.path().join("Cargo.toml")).unwrap();

        dir
    }

    fn run_check(input: &str, repo_path: &Path) -> ValidationResult {
        let file = parse_codeowners(input).ast;
        let config = CheckConfig::new();
        let ctx = CheckContext::new(&file, repo_path, &config);
        FilesCheck::new().run(&ctx)
    }

    #[test]
    fn pattern_matches_existing_files() {
        let dir = setup_test_dir();
        let result = run_check("*.rs @owner\n", dir.path());
        assert!(
            result.is_ok(),
            "Pattern *.rs should match src/main.rs and src/lib.rs"
        );
    }

    #[test]
    fn pattern_matches_specific_directory() {
        let dir = setup_test_dir();
        let result = run_check("/src/ @owner\n", dir.path());
        assert!(result.is_ok(), "Pattern /src/ should match files in src/");
    }

    #[test]
    fn pattern_not_matching_reports_error() {
        let dir = setup_test_dir();
        let result = run_check("/nonexistent/ @owner\n", dir.path());
        assert!(result.has_errors());

        match &result.errors[0] {
            ValidationError::PatternNotMatching { pattern, .. } => {
                assert_eq!(pattern, "/nonexistent/");
            }
            _ => panic!("Expected PatternNotMatching error"),
        }
    }

    #[test]
    fn wildcard_matches_all() {
        let dir = setup_test_dir();
        let result = run_check("* @owner\n", dir.path());
        assert!(result.is_ok(), "Pattern * should match everything");
    }

    #[test]
    fn specific_file_pattern() {
        let dir = setup_test_dir();
        let result = run_check("/Cargo.toml @owner\n", dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn specific_file_not_found() {
        let dir = setup_test_dir();
        let result = run_check("/package.json @owner\n", dir.path());
        assert!(result.has_errors());
    }

    #[test]
    fn markdown_files_in_docs() {
        let dir = setup_test_dir();
        let result = run_check("/docs/*.md @owner\n", dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn multiple_patterns_mixed() {
        let dir = setup_test_dir();
        let result = run_check(
            "*.rs @rust\n/nonexistent/ @nobody\n/docs/ @docs\n",
            dir.path(),
        );
        assert_eq!(result.errors.len(), 1);

        match &result.errors[0] {
            ValidationError::PatternNotMatching { pattern, .. } => {
                assert_eq!(pattern, "/nonexistent/");
            }
            _ => panic!("Expected PatternNotMatching error"),
        }
    }

    #[test]
    fn empty_directory() {
        let dir = TempDir::new().unwrap();
        let result = run_check("*.rs @owner\n", dir.path());
        // Should report that the pattern doesn't match anything
        assert!(result.has_errors());
    }

    #[test]
    fn list_files_basic() {
        let dir = setup_test_dir();
        let files = FilesCheck::list_files(dir.path());

        assert!(files.contains(&"src/main.rs".to_string()));
        assert!(files.contains(&"src/lib.rs".to_string()));
        assert!(files.contains(&"docs/README.md".to_string()));
        assert!(files.contains(&"Cargo.toml".to_string()));
    }

    #[test]
    fn list_files_skips_hidden() {
        let dir = TempDir::new().unwrap();

        // Create hidden files and directories
        fs::create_dir_all(dir.path().join(".hidden_dir")).unwrap();
        File::create(dir.path().join(".hidden_dir/config")).unwrap();
        File::create(dir.path().join(".hidden_file")).unwrap();
        File::create(dir.path().join("visible.rs")).unwrap();

        let files = FilesCheck::list_files(dir.path());

        // Currently FilesCheck still filters hidden files
        assert!(files.contains(&"visible.rs".to_string()));
        assert!(!files.iter().any(|f| f.contains(".hidden")));
    }
}
