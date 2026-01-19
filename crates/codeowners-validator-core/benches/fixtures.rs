//! Benchmark fixtures - generated at runtime using AST serialization.
//!
//! Fixtures are generated lazily on first access and cached for the
//! duration of the benchmark run. All generation is deterministic.
//!
//! For file-dependent checks (FilesCheck, NotOwnedCheck), a temporary
//! directory is created with files matching the generated patterns.

use codeowners_validator_core::generate::{GeneratorConfig, generate};
use std::path::PathBuf;
use std::sync::LazyLock;
use tempfile::TempDir;

// Lazily generated fixtures (deterministic via default seed)
static SMALL: LazyLock<String> = LazyLock::new(|| generate(&GeneratorConfig::small()));
static MEDIUM: LazyLock<String> = LazyLock::new(|| generate(&GeneratorConfig::medium()));
static LARGE: LazyLock<String> = LazyLock::new(|| generate(&GeneratorConfig::large()));
static XLARGE: LazyLock<String> = LazyLock::new(|| generate(&GeneratorConfig::xlarge()));
static MAX_SIZE: LazyLock<String> =
    LazyLock::new(|| generate(&GeneratorConfig::target_bytes(3_000_000)));

/// Standard fixtures for regular benchmarks.
pub fn fixtures() -> &'static [(&'static str, &'static str)] {
    // Return static slice to avoid allocation on each call
    static FIXTURES: LazyLock<Vec<(&'static str, &'static str)>> = LazyLock::new(|| {
        vec![
            ("small", SMALL.as_str()),
            ("medium", MEDIUM.as_str()),
            ("large", LARGE.as_str()),
        ]
    });
    FIXTURES.as_slice()
}

/// Extended fixtures including stress tests up to GitHub's 3MB limit.
pub fn fixtures_extended() -> &'static [(&'static str, &'static str)] {
    static FIXTURES: LazyLock<Vec<(&'static str, &'static str)>> = LazyLock::new(|| {
        vec![
            ("small", SMALL.as_str()),
            ("medium", MEDIUM.as_str()),
            ("large", LARGE.as_str()),
            ("xlarge", XLARGE.as_str()),
            ("max_size", MAX_SIZE.as_str()),
        ]
    });
    FIXTURES.as_slice()
}

/// A benchmark repository with files matching generated CODEOWNERS patterns.
///
/// The temp directory is kept alive as long as this struct exists.
pub struct BenchmarkRepo {
    #[allow(dead_code)] // Kept to maintain temp directory lifetime
    temp_dir: TempDir,
    /// Path to the repository root.
    pub path: PathBuf,
}

impl BenchmarkRepo {
    /// Creates a new benchmark repository with files matching generated patterns.
    ///
    /// The file structure matches patterns used by the generator:
    /// - Various extensions: .rs, .py, .js, .ts, .go, .md, .yaml, .json, .toml
    /// - Directories: src, lib, tests, docs, config, scripts, api, core
    /// - Nested files for ** glob patterns
    /// - Test files for test_* patterns
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let path = temp_dir.path().to_path_buf();
        let root = &path;

        // Extensions used by the generator
        let extensions = ["rs", "py", "js", "ts", "go", "md", "yaml", "json", "toml"];
        // Directories used by the generator
        let directories = [
            "src", "lib", "tests", "docs", "config", "scripts", "api", "core",
        ];

        // Create root-level files for each extension (matches *.{ext})
        for ext in &extensions {
            std::fs::write(root.join(format!("file.{}", ext)), "").ok();
        }

        // Create directory structure with files
        for dir in &directories {
            let dir_path = root.join(dir);
            std::fs::create_dir_all(&dir_path).ok();

            // Files in directory (matches /{dir}/*.{ext})
            for ext in &extensions {
                std::fs::write(dir_path.join(format!("file.{}", ext)), "").ok();
            }

            // Nested subdirectory (matches /{dir}/**)
            let sub_dir = dir_path.join("sub");
            std::fs::create_dir_all(&sub_dir).ok();
            for ext in &extensions {
                std::fs::write(sub_dir.join(format!("nested.{}", ext)), "").ok();
                // Test files (matches /{dir}/**/test_*.{ext})
                std::fs::write(sub_dir.join(format!("test_example.{}", ext)), "").ok();
            }
        }

        // Create /src/{dir}/ structure
        for dir in &directories {
            let dir_path = root.join("src").join(dir);
            std::fs::create_dir_all(&dir_path).ok();
            std::fs::write(dir_path.join("mod.rs"), "").ok();
        }

        // Create docs/**/*.md structure
        let docs_sub = root.join("docs").join("guide");
        std::fs::create_dir_all(&docs_sub).ok();
        std::fs::write(docs_sub.join("README.md"), "").ok();
        std::fs::write(docs_sub.join("guide.md"), "").ok();

        // Create vendor directory (for !vendor/ negation patterns)
        let vendor = root.join("vendor");
        std::fs::create_dir_all(&vendor).ok();
        std::fs::write(vendor.join("external.rs"), "").ok();

        BenchmarkRepo { temp_dir, path }
    }
}

/// Lazily created benchmark repository for file-dependent checks.
///
/// This is created once and reused across all benchmarks.
static BENCHMARK_REPO: LazyLock<BenchmarkRepo> = LazyLock::new(BenchmarkRepo::new);

/// Returns the path to the benchmark repository.
///
/// Use this for checks that need actual files (FilesCheck, NotOwnedCheck).
pub fn repo_path() -> &'static PathBuf {
    &BENCHMARK_REPO.path
}
