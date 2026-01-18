//! Validation module for CODEOWNERS files.
//!
//! This module provides validation rules that can be applied to parsed
//! CODEOWNERS files to detect semantic issues.
//!
//! # Example
//!
//! ```rust
//! use codeowners_validator_core::parse::parse_codeowners;
//! use codeowners_validator_core::validate::{validate_syntax, ValidationResult};
//!
//! let input = "*.rs @rustacean\n";
//! let parse_result = parse_codeowners(input);
//!
//! if parse_result.is_ok() {
//!     let validation = validate_syntax(&parse_result.ast);
//!     if validation.is_ok() {
//!         println!("CODEOWNERS file is valid!");
//!     } else {
//!         for error in &validation.errors {
//!             eprintln!("{}", error);
//!         }
//!     }
//! }
//! ```

pub mod checks;
mod error;
pub mod github_client;
mod syntax;

// Re-export public types
pub use error::{Severity, ValidationError, ValidationResult};
pub use syntax::{
    validate_all_owners, validate_all_patterns, validate_owner_syntax, validate_pattern_syntax,
    validate_syntax,
};
