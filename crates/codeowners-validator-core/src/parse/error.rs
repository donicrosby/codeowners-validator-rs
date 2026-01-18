//! Error types for CODEOWNERS file parsing.
//!
//! This module defines error types that capture parse failures
//! along with their source locations.

use super::span::Span;
use thiserror::Error;

/// An error that occurred during parsing.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ParseError {
    /// A line could not be parsed.
    #[error("line {line}: {message}")]
    InvalidLine {
        /// The line number where the error occurred (1-based).
        line: usize,
        /// Description of the error.
        message: String,
        /// Location in the source.
        span: Span,
    },

    /// Expected an owner but found something else.
    #[error("line {line}, column {column}: expected owner (e.g., @user, @org/team, or email)")]
    ExpectedOwner {
        /// The line number (1-based).
        line: usize,
        /// The column number (1-based).
        column: usize,
        /// Location in the source.
        span: Span,
    },

    /// Expected a pattern but found something else.
    #[error("line {line}, column {column}: expected file pattern")]
    ExpectedPattern {
        /// The line number (1-based).
        line: usize,
        /// The column number (1-based).
        column: usize,
        /// Location in the source.
        span: Span,
    },

    /// A rule line has no owners.
    #[error("line {line}: rule has no owners")]
    MissingOwners {
        /// The line number (1-based).
        line: usize,
        /// Location of the pattern without owners.
        span: Span,
    },

    /// Unexpected content at end of line.
    #[error("line {line}, column {column}: unexpected content")]
    UnexpectedContent {
        /// The line number (1-based).
        line: usize,
        /// The column number (1-based).
        column: usize,
        /// Location of the unexpected content.
        span: Span,
    },
}

impl ParseError {
    /// Creates an invalid line error.
    pub fn invalid_line(message: impl Into<String>, span: Span) -> Self {
        Self::InvalidLine {
            line: span.line,
            message: message.into(),
            span,
        }
    }

    /// Creates an expected owner error.
    pub fn expected_owner(span: Span) -> Self {
        Self::ExpectedOwner {
            line: span.line,
            column: span.column,
            span,
        }
    }

    /// Creates an expected pattern error.
    pub fn expected_pattern(span: Span) -> Self {
        Self::ExpectedPattern {
            line: span.line,
            column: span.column,
            span,
        }
    }

    /// Creates a missing owners error.
    pub fn missing_owners(span: Span) -> Self {
        Self::MissingOwners {
            line: span.line,
            span,
        }
    }

    /// Creates an unexpected content error.
    pub fn unexpected_content(span: Span) -> Self {
        Self::UnexpectedContent {
            line: span.line,
            column: span.column,
            span,
        }
    }

    /// Returns the span associated with this error.
    pub fn span(&self) -> &Span {
        match self {
            ParseError::InvalidLine { span, .. } => span,
            ParseError::ExpectedOwner { span, .. } => span,
            ParseError::ExpectedPattern { span, .. } => span,
            ParseError::MissingOwners { span, .. } => span,
            ParseError::UnexpectedContent { span, .. } => span,
        }
    }

    /// Returns the line number where this error occurred.
    pub fn line(&self) -> usize {
        match self {
            ParseError::InvalidLine { line, .. } => *line,
            ParseError::ExpectedOwner { line, .. } => *line,
            ParseError::ExpectedPattern { line, .. } => *line,
            ParseError::MissingOwners { line, .. } => *line,
            ParseError::UnexpectedContent { line, .. } => *line,
        }
    }
}

/// The result of parsing a CODEOWNERS file.
#[derive(Debug, Clone)]
pub struct ParseResult {
    /// The parsed AST (may be partial if there were errors in lenient mode).
    pub ast: super::ast::CodeownersFile,
    /// Any errors encountered during parsing.
    pub errors: Vec<ParseError>,
}

impl ParseResult {
    /// Creates a successful parse result with no errors.
    pub fn ok(ast: super::ast::CodeownersFile) -> Self {
        Self {
            ast,
            errors: Vec::new(),
        }
    }

    /// Creates a parse result with errors.
    pub fn with_errors(ast: super::ast::CodeownersFile, errors: Vec<ParseError>) -> Self {
        Self { ast, errors }
    }

    /// Returns true if parsing succeeded without errors.
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    /// Returns true if there were parse errors.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::ast::CodeownersFile;

    fn test_span() -> Span {
        Span::new(10, 2, 5, 15)
    }

    #[test]
    fn parse_error_invalid_line() {
        let error = ParseError::invalid_line("bad syntax", test_span());
        assert!(matches!(error, ParseError::InvalidLine { line: 2, .. }));
        assert_eq!(error.line(), 2);
        assert!(error.to_string().contains("bad syntax"));
    }

    #[test]
    fn parse_error_expected_owner() {
        let error = ParseError::expected_owner(test_span());
        assert!(matches!(
            error,
            ParseError::ExpectedOwner {
                line: 2,
                column: 5,
                ..
            }
        ));
        assert!(error.to_string().contains("expected owner"));
    }

    #[test]
    fn parse_error_expected_pattern() {
        let error = ParseError::expected_pattern(test_span());
        assert!(matches!(
            error,
            ParseError::ExpectedPattern {
                line: 2,
                column: 5,
                ..
            }
        ));
        assert!(error.to_string().contains("expected file pattern"));
    }

    #[test]
    fn parse_error_missing_owners() {
        let error = ParseError::missing_owners(test_span());
        assert!(matches!(error, ParseError::MissingOwners { line: 2, .. }));
        assert!(error.to_string().contains("no owners"));
    }

    #[test]
    fn parse_error_span() {
        let span = test_span();
        let error = ParseError::invalid_line("test", span);
        assert_eq!(error.span(), &span);
    }

    #[test]
    fn parse_result_ok() {
        let result = ParseResult::ok(CodeownersFile::default());
        assert!(result.is_ok());
        assert!(!result.has_errors());
    }

    #[test]
    fn parse_result_with_errors() {
        let errors = vec![ParseError::invalid_line("error", test_span())];
        let result = ParseResult::with_errors(CodeownersFile::default(), errors);
        assert!(!result.is_ok());
        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 1);
    }
}
