//! Shared file walking utilities for validation checks.
//!
//! This module provides a configurable file walker that can be used by different
//! validation checks with varying requirements.

use ignore::WalkBuilder;
use log::{debug, trace};
use std::path::Path;

/// Configuration for file walking behavior.
#[derive(Debug, Clone)]
#[derive(Default)]
pub struct FileWalkerConfig {
    /// Whether to include hidden files and directories (starting with `.`).
    /// Default: false
    pub include_hidden: bool,
    /// Whether to respect `.gitignore` rules (only works in git repos).
    /// Default: false
    pub respect_gitignore: bool,
    /// Whether to include directories in the output.
    /// Default: false (files only)
    pub include_directories: bool,
}


impl FileWalkerConfig {
    /// Creates a new config with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets whether to include hidden files.
    pub fn with_hidden(mut self, include: bool) -> Self {
        self.include_hidden = include;
        self
    }

    /// Sets whether to respect .gitignore rules.
    pub fn with_gitignore(mut self, respect: bool) -> Self {
        self.respect_gitignore = respect;
        self
    }

    /// Sets whether to include directories.
    pub fn with_directories(mut self, include: bool) -> Self {
        self.include_directories = include;
        self
    }

    /// Configuration for FilesCheck: excludes hidden, includes dirs.
    pub fn for_files_check() -> Self {
        Self {
            include_hidden: false,
            respect_gitignore: false,
            include_directories: true,
        }
    }

    /// Configuration for NotOwnedCheck: includes hidden, respects gitignore, files only.
    pub fn for_not_owned_check() -> Self {
        Self {
            include_hidden: true,
            respect_gitignore: true,
            include_directories: false,
        }
    }
}

/// Lists files (and optionally directories) in a repository.
///
/// Returns paths relative to `repo_path` with forward slashes.
pub fn list_files(repo_path: &Path, config: &FileWalkerConfig) -> Vec<String> {
    debug!(
        "Listing files in {:?} (hidden={}, gitignore={}, dirs={})",
        repo_path, config.include_hidden, config.respect_gitignore, config.include_directories
    );

    let mut files = Vec::new();

    // Use WalkBuilder from the `ignore` crate which:
    // - Can respect .gitignore by default (when in a git repo)
    // - Skips .git directory by default
    // - Can be configured to include/exclude hidden files
    let walker = WalkBuilder::new(repo_path)
        .hidden(!config.include_hidden) // hidden(true) = skip hidden files
        .ignore(false) // Don't respect .ignore files (not a git standard)
        .git_ignore(config.respect_gitignore)
        .git_global(config.respect_gitignore)
        .git_exclude(config.respect_gitignore)
        .follow_links(false)
        .build();

    for entry in walker.filter_map(|e| e.ok()) {
        // Skip the root directory itself
        if entry.path() == repo_path {
            continue;
        }

        // Check file type
        let is_file = entry.file_type().is_some_and(|ft| ft.is_file());
        let is_dir = entry.file_type().is_some_and(|ft| ft.is_dir());

        // Skip based on config
        if !is_file && !(config.include_directories && is_dir) {
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

    debug!("Found {} entries", files.len());
    trace!("Entries: {:?}", files);
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use tempfile::TempDir;

    fn setup_test_dir() -> TempDir {
        let dir = TempDir::new().unwrap();

        // Create visible files and dirs
        fs::create_dir_all(dir.path().join("src")).unwrap();
        File::create(dir.path().join("src/main.rs")).unwrap();
        File::create(dir.path().join("visible.txt")).unwrap();

        // Create hidden files and dirs
        fs::create_dir_all(dir.path().join(".hidden_dir")).unwrap();
        File::create(dir.path().join(".hidden_dir/config")).unwrap();
        File::create(dir.path().join(".hidden_file")).unwrap();

        dir
    }

    #[test]
    fn default_excludes_hidden() {
        let dir = setup_test_dir();
        let config = FileWalkerConfig::default();
        let files = list_files(dir.path(), &config);

        assert!(files.contains(&"src/main.rs".to_string()));
        assert!(files.contains(&"visible.txt".to_string()));
        assert!(!files.iter().any(|f| f.contains(".hidden")));
    }

    #[test]
    fn include_hidden_files() {
        let dir = setup_test_dir();
        let config = FileWalkerConfig::new().with_hidden(true);
        let files = list_files(dir.path(), &config);

        assert!(files.contains(&"src/main.rs".to_string()));
        assert!(files.contains(&".hidden_file".to_string()));
        assert!(files.contains(&".hidden_dir/config".to_string()));
    }

    #[test]
    fn include_directories() {
        let dir = setup_test_dir();
        let config = FileWalkerConfig::new().with_directories(true);
        let files = list_files(dir.path(), &config);

        assert!(files.contains(&"src".to_string()));
        assert!(files.contains(&"src/main.rs".to_string()));
    }

    #[test]
    fn for_files_check_config() {
        let config = FileWalkerConfig::for_files_check();
        assert!(!config.include_hidden);
        assert!(!config.respect_gitignore);
        assert!(config.include_directories);
    }

    #[test]
    fn for_not_owned_check_config() {
        let config = FileWalkerConfig::for_not_owned_check();
        assert!(config.include_hidden);
        assert!(config.respect_gitignore);
        assert!(!config.include_directories);
    }
}
