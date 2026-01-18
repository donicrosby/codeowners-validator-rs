//! Parser module for CODEOWNERS files.
//!
//! This module provides functionality to parse GitHub CODEOWNERS files
//! into an AST with rich span metadata for error reporting.
//!
//! # Example
//!
//! ```rust
//! use codeowners_validator_core::parse::{parse_codeowners, ParserConfig};
//!
//! let input = r#"
//! # CODEOWNERS file
//! *.rs @rustacean
//! /docs/ @docs-team
//! "#;
//!
//! let result = parse_codeowners(input);
//! if result.is_ok() {
//!     for line in &result.ast.lines {
//!         println!("{:?}", line);
//!     }
//! }
//! ```

mod ast;
mod error;
mod lexer;
mod parser;
pub mod span;

// Re-export public types
pub use ast::{CodeownersFile, Line, LineKind, Owner, Pattern};
pub use error::{ParseError, ParseResult};
pub use parser::{parse_codeowners, parse_codeowners_strict, parse_codeowners_with_config, ParserConfig};
pub use span::Span;

// Re-export lexer utilities that may be useful for custom parsing
pub use lexer::{classify_owner, OwnerKind};
