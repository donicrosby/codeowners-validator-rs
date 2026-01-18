//! Pattern matching for CODEOWNERS files.
//!
//! This module implements gitignore-style pattern matching used by GitHub CODEOWNERS files.
//! Patterns follow these rules:
//!
//! - `*` matches any sequence of non-slash characters
//! - `**` matches any sequence including slashes (any path)
//! - `/` at the start anchors to the repository root
//! - `/` at the end matches only directories
//! - Patterns without a leading `/` match anywhere in the path

use globset::{GlobBuilder, GlobMatcher, GlobSet, GlobSetBuilder};

/// A compiled CODEOWNERS pattern that can match file paths.
#[derive(Debug, Clone)]
pub struct Pattern {
    /// The original pattern string.
    original: String,
    /// The compiled glob matcher.
    matcher: GlobMatcher,
    /// Whether this pattern is anchored to the root.
    anchored: bool,
    /// Whether this pattern matches only directories.
    directory_only: bool,
}

impl Pattern {
    /// Compiles a CODEOWNERS pattern for matching.
    ///
    /// Returns `None` if the pattern is invalid.
    pub fn new(pattern: &str) -> Option<Self> {
        let original = pattern.to_string();
        let (glob_pattern, anchored, directory_only) = normalize_pattern(pattern);

        // Use literal_separator to ensure * doesn't match /
        let glob = GlobBuilder::new(&glob_pattern)
            .literal_separator(true)
            .build()
            .ok()?;
        let matcher = glob.compile_matcher();

        Some(Self {
            original,
            matcher,
            anchored,
            directory_only,
        })
    }

    /// Returns the original pattern string.
    pub fn as_str(&self) -> &str {
        &self.original
    }

    /// Returns true if this pattern is anchored to the repository root.
    pub fn is_anchored(&self) -> bool {
        self.anchored
    }

    /// Returns true if this pattern matches only directories.
    pub fn is_directory_only(&self) -> bool {
        self.directory_only
    }

    /// Checks if this pattern matches the given path.
    ///
    /// The path should be relative to the repository root and use forward slashes.
    pub fn matches(&self, path: &str) -> bool {
        // Normalize path to not have leading slash
        let path = path.strip_prefix('/').unwrap_or(path);
        self.matcher.is_match(path)
    }

    /// Checks if this pattern matches the given path, considering directory status.
    ///
    /// If the pattern is directory-only, `is_dir` must be true for a match.
    pub fn matches_path(&self, path: &str, is_dir: bool) -> bool {
        if self.directory_only && !is_dir {
            return false;
        }
        self.matches(path)
    }

    /// Calculates a specificity score for this pattern.
    ///
    /// Higher scores indicate more specific patterns.
    /// Used for detecting shadowing.
    pub fn specificity(&self) -> u32 {
        calculate_specificity(&self.original)
    }
}

/// A set of compiled patterns for efficient matching.
#[derive(Debug, Clone)]
pub struct PatternSet {
    /// The glob set for batch matching.
    glob_set: GlobSet,
    /// Individual patterns for detailed matching.
    patterns: Vec<Pattern>,
}

impl PatternSet {
    /// Creates a new pattern set from a list of pattern strings.
    pub fn new(patterns: &[&str]) -> Option<Self> {
        let mut builder = GlobSetBuilder::new();
        let mut compiled_patterns = Vec::new();

        for pattern_str in patterns {
            let pattern = Pattern::new(pattern_str)?;
            let (glob_pattern, _, _) = normalize_pattern(pattern_str);
            let glob = GlobBuilder::new(&glob_pattern)
                .literal_separator(true)
                .build()
                .ok()?;
            builder.add(glob);
            compiled_patterns.push(pattern);
        }

        let glob_set = builder.build().ok()?;

        Some(Self {
            glob_set,
            patterns: compiled_patterns,
        })
    }

    /// Returns indices of all patterns that match the given path.
    pub fn matches(&self, path: &str) -> Vec<usize> {
        let path = path.strip_prefix('/').unwrap_or(path);
        self.glob_set.matches(path)
    }

    /// Returns the last (most recent) pattern that matches the path.
    ///
    /// In CODEOWNERS, later patterns take precedence.
    pub fn last_match(&self, path: &str) -> Option<&Pattern> {
        let matches = self.matches(path);
        matches.last().map(|&idx| &self.patterns[idx])
    }

    /// Returns true if any pattern matches the path.
    pub fn is_match(&self, path: &str) -> bool {
        let path = path.strip_prefix('/').unwrap_or(path);
        self.glob_set.is_match(path)
    }

    /// Returns the number of patterns in the set.
    pub fn len(&self) -> usize {
        self.patterns.len()
    }

    /// Returns true if the set contains no patterns.
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }
}

/// Normalizes a CODEOWNERS pattern to a glob pattern.
///
/// Returns (glob_pattern, is_anchored, is_directory_only).
fn normalize_pattern(pattern: &str) -> (String, bool, bool) {
    let mut pattern = pattern.to_string();
    let mut anchored = false;
    let mut directory_only = false;

    // Check for directory-only suffix
    if pattern.ends_with('/') {
        directory_only = true;
        pattern = pattern.trim_end_matches('/').to_string();
    }

    // Check for anchored pattern (starts with /)
    if pattern.starts_with('/') {
        anchored = true;
        pattern = pattern[1..].to_string();
        // Anchored patterns are already relative to root, don't add **/
    } else if !pattern.contains('/') {
        // Pattern without slash matches anywhere in the tree
        // Convert to **/ prefix for glob matching
        pattern = format!("**/{}", pattern);
    }
    // Patterns with / but not starting with / are relative to root already

    // For directory patterns, we need to match everything inside
    // e.g., /docs/ should become docs/** to match docs/anything
    if directory_only {
        pattern = format!("{}/**", pattern);
    }

    (pattern, anchored, directory_only)
}

/// Calculates a specificity score for a pattern.
///
/// Higher scores mean more specific patterns.
/// Scoring rules:
/// - Each literal path segment: +10
/// - Each wildcard (*): +1
/// - Each double wildcard (**): +0 (matches anything)
/// - Anchored patterns: +5
/// - File extension patterns: +3
fn calculate_specificity(pattern: &str) -> u32 {
    let mut score = 0u32;

    // Anchored patterns are more specific
    if pattern.starts_with('/') {
        score += 5;
    }

    // Count path segments
    let clean_pattern = pattern.trim_matches('/');
    let segments: Vec<&str> = clean_pattern.split('/').collect();

    for segment in segments {
        if segment == "**" {
            // Double wildcard matches anything - low specificity
            score += 0;
        } else if segment == "*" {
            // Single wildcard
            score += 1;
        } else if segment.contains('*') {
            // Partial wildcard like "*.rs"
            score += 3;
        } else {
            // Literal segment
            score += 10;
        }
    }

    // Patterns with file extensions are more specific
    if pattern.contains('.') && !pattern.ends_with('/') {
        score += 3;
    }

    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_simple_wildcard() {
        let pattern = Pattern::new("*.rs").unwrap();
        assert!(pattern.matches("main.rs"));
        assert!(pattern.matches("src/lib.rs"));
        assert!(pattern.matches("src/parse/mod.rs"));
        assert!(!pattern.matches("main.txt"));
    }

    #[test]
    fn pattern_anchored() {
        let pattern = Pattern::new("/*.rs").unwrap();
        assert!(pattern.is_anchored());
        assert!(pattern.matches("main.rs"));
        assert!(!pattern.matches("src/main.rs"));
    }

    #[test]
    fn pattern_directory() {
        let pattern = Pattern::new("src/").unwrap();
        assert!(pattern.is_directory_only());
        // Directory patterns match files inside the directory
        assert!(pattern.matches("src/main.rs"));
        assert!(pattern.matches("src/lib/mod.rs"));
        // But not files outside
        assert!(!pattern.matches("main.rs"));
        assert!(!pattern.matches("other/main.rs"));
    }

    #[test]
    fn pattern_double_wildcard() {
        let pattern = Pattern::new("**/test/").unwrap();
        // Directory patterns match files inside the directory
        assert!(pattern.matches("test/file.rs"));
        assert!(pattern.matches("a/test/file.rs"));
        assert!(pattern.matches("a/b/test/file.rs"));
        assert!(pattern.matches("a/b/c/test/file.rs"));
        // But not files outside
        assert!(!pattern.matches("test.rs"));
        assert!(!pattern.matches("a/test.rs"));
    }

    #[test]
    fn pattern_specific_path() {
        let pattern = Pattern::new("/docs/*.md").unwrap();
        assert!(pattern.matches("docs/README.md"));
        assert!(!pattern.matches("docs/api/index.md"));
        assert!(!pattern.matches("other/docs/README.md"));
    }

    #[test]
    fn pattern_unanchored_with_slash() {
        let pattern = Pattern::new("docs/*.md").unwrap();
        assert!(pattern.matches("docs/README.md"));
        // Without leading /, it matches from root
        assert!(!pattern.matches("other/docs/README.md"));
    }

    #[test]
    fn pattern_all_files() {
        let pattern = Pattern::new("*").unwrap();
        assert!(pattern.matches("main.rs"));
        assert!(pattern.matches("src/main.rs"));
        assert!(pattern.matches("a/b/c/d.txt"));
    }

    #[test]
    fn pattern_specificity() {
        // More specific patterns should have higher scores
        let p1 = Pattern::new("*").unwrap();
        let p2 = Pattern::new("*.rs").unwrap();
        let p3 = Pattern::new("/src/").unwrap();
        let p4 = Pattern::new("/src/main.rs").unwrap();

        assert!(p1.specificity() < p2.specificity());
        assert!(p2.specificity() < p3.specificity());
        assert!(p3.specificity() < p4.specificity());
    }

    #[test]
    fn pattern_set_basic() {
        let set = PatternSet::new(&["*.rs", "*.md", "/docs/"]).unwrap();
        assert_eq!(set.len(), 3);
        assert!(set.is_match("main.rs"));
        assert!(set.is_match("README.md"));
        assert!(set.is_match("docs/index.html"));
        assert!(!set.is_match("main.txt"));
    }

    #[test]
    fn pattern_set_last_match() {
        let set = PatternSet::new(&["*", "*.rs", "/src/*.rs"]).unwrap();

        // For src/main.rs, the last matching pattern should be /src/*.rs
        let last = set.last_match("src/main.rs");
        assert!(last.is_some());
        assert_eq!(last.unwrap().as_str(), "/src/*.rs");

        // For README.md, only * matches
        let last = set.last_match("README.md");
        assert!(last.is_some());
        assert_eq!(last.unwrap().as_str(), "*");
    }

    #[test]
    fn pattern_matches_with_leading_slash() {
        let pattern = Pattern::new("*.rs").unwrap();
        // Should handle paths with or without leading slash
        assert!(pattern.matches("main.rs"));
        assert!(pattern.matches("/main.rs"));
        assert!(pattern.matches("src/main.rs"));
        assert!(pattern.matches("/src/main.rs"));
    }

    #[test]
    fn normalize_pattern_cases() {
        // Test various normalization cases
        let (p, anchored, dir) = normalize_pattern("/src/");
        assert!(anchored);
        assert!(dir);
        assert_eq!(p, "src/**");

        let (p, anchored, dir) = normalize_pattern("*.rs");
        assert!(!anchored);
        assert!(!dir);
        assert_eq!(p, "**/*.rs");

        let (p, anchored, dir) = normalize_pattern("src/lib/");
        assert!(!anchored);
        assert!(dir);
        assert_eq!(p, "src/lib/**");
    }
}
