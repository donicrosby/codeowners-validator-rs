//! CODEOWNERS Validator
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
//! use codeowners_validator::parse::parse_codeowners;
//! use codeowners_validator::validate::validate_syntax;
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

pub mod matching;
pub mod parse;
pub mod validate;

// Re-export commonly used types at the crate root
pub use parse::{parse_codeowners, CodeownersFile, ParseResult};
pub use validate::{validate_syntax, ValidationResult};
pub use validate::checks::{CheckConfig, CheckContext, AsyncCheckContext, Check, AsyncCheck, CheckRunner};
