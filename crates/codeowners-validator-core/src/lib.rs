//! CODEOWNERS Validator Core
//!
//! A library for parsing and validating GitHub CODEOWNERS files.
//!
//! # Features
//!
//! - **Parser**: Parse CODEOWNERS files into an AST with rich span metadata
//! - **Validation**: Check for syntax errors and invalid owner/pattern formats
//! - **Lenient Mode**: Continue parsing after errors to collect all issues
//! - **Strict Mode**: Stop at first error for fail-fast behavior
//!
//! # Quick Start
//!
//! ```rust
//! use codeowners_validator_core::parse::parse_codeowners;
//! use codeowners_validator_core::validate::validate_syntax;
//!
//! let input = r#"
//! # CODEOWNERS file
//! *.rs @rustacean
//! /docs/ @github/docs-team
//! "#;
//!
//! // Parse the file
//! let parse_result = parse_codeowners(input);
//!
//! if parse_result.is_ok() {
//!     // Validate syntax
//!     let validation = validate_syntax(&parse_result.ast);
//!     
//!     if validation.is_ok() {
//!         println!("CODEOWNERS file is valid!");
//!         
//!         // Extract rules for further processing
//!         for (pattern, owners) in parse_result.ast.extract_rules() {
//!             println!("Pattern: {} -> {:?}", pattern.text, owners);
//!         }
//!     } else {
//!         for error in &validation.errors {
//!             eprintln!("Validation error: {}", error);
//!         }
//!     }
//! } else {
//!     for error in &parse_result.errors {
//!         eprintln!("Parse error: {}", error);
//!     }
//! }
//! ```
//!
//! # Modules
//!
//! - [`parse`]: Parser for CODEOWNERS files
//! - [`validate`]: Validation rules for parsed files
//! - [`matching`]: Pattern matching for CODEOWNERS files

use std::path::{Path, PathBuf};

pub mod matching;
pub mod parse;
pub mod validate;

// Re-export commonly used types at the crate root
pub use parse::{CodeownersFile, ParseResult, parse_codeowners};
pub use validate::checks::{
    AsyncCheck, AsyncCheckContext, Check, CheckConfig, CheckContext, CheckRunner,
};
pub use validate::{ValidationResult, validate_syntax};

/// Finds the CODEOWNERS file in a repository.
///
/// Searches in the following locations (in order):
/// 1. `.github/CODEOWNERS`
/// 2. `CODEOWNERS`
/// 3. `docs/CODEOWNERS`
///
/// Returns `Some(path)` if found, `None` otherwise.
///
/// # Example
///
/// ```no_run
/// use std::path::Path;
/// use codeowners_validator_core::find_codeowners_file;
///
/// let repo_path = Path::new("/path/to/repo");
/// if let Some(codeowners_path) = find_codeowners_file(repo_path) {
///     println!("Found CODEOWNERS at: {}", codeowners_path.display());
/// } else {
///     eprintln!("CODEOWNERS file not found");
/// }
/// ```
pub fn find_codeowners_file(repo_path: &Path) -> Option<PathBuf> {
    let locations = [
        repo_path.join(".github/CODEOWNERS"),
        repo_path.join("CODEOWNERS"),
        repo_path.join("docs/CODEOWNERS"),
    ];
    locations.into_iter().find(|p| p.exists())
}
