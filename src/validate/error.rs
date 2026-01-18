//! Error types for CODEOWNERS validation.
//!
//! This module defines validation error types that describe
//! semantic issues found after parsing.

use crate::parse::span::Span;
use thiserror::Error;

/// The severity of a validation issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// A warning that doesn't prevent the file from working.
    Warning,
    /// An error that may cause unexpected behavior.
    Error,
}

/// A validation error found in a CODEOWNERS file.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ValidationError {
    /// Invalid owner format.
    #[error("line {line}: invalid owner format '{owner}' - {reason}")]
    InvalidOwnerFormat {
        /// The line number (1-based).
        line: usize,
        /// The invalid owner text.
        owner: String,
        /// Why the format is invalid.
        reason: String,
        /// Location of the invalid owner.
        span: Span,
    },

    /// Invalid pattern syntax.
    #[error("line {line}: invalid pattern '{pattern}' - {reason}")]
    InvalidPatternSyntax {
        /// The line number (1-based).
        line: usize,
        /// The invalid pattern text.
        pattern: String,
        /// Why the pattern is invalid.
        reason: String,
        /// Location of the invalid pattern.
        span: Span,
    },

    /// Pattern uses unsupported gitignore syntax.
    #[error("line {line}: pattern '{pattern}' uses unsupported syntax - {reason}")]
    UnsupportedPatternSyntax {
        /// The line number (1-based).
        line: usize,
        /// The pattern with unsupported syntax.
        pattern: String,
        /// What syntax is not supported.
        reason: String,
        /// Location of the pattern.
        span: Span,
    },
}

impl ValidationError {
    /// Creates an invalid owner format error.
    pub fn invalid_owner_format(
        owner: impl Into<String>,
        reason: impl Into<String>,
        span: Span,
    ) -> Self {
        Self::InvalidOwnerFormat {
            line: span.line,
            owner: owner.into(),
            reason: reason.into(),
            span,
        }
    }

    /// Creates an invalid pattern syntax error.
    pub fn invalid_pattern_syntax(
        pattern: impl Into<String>,
        reason: impl Into<String>,
        span: Span,
    ) -> Self {
        Self::InvalidPatternSyntax {
            line: span.line,
            pattern: pattern.into(),
            reason: reason.into(),
            span,
        }
    }

    /// Creates an unsupported pattern syntax error.
    pub fn unsupported_pattern_syntax(
        pattern: impl Into<String>,
        reason: impl Into<String>,
        span: Span,
    ) -> Self {
        Self::UnsupportedPatternSyntax {
            line: span.line,
            pattern: pattern.into(),
            reason: reason.into(),
            span,
        }
    }

    /// Returns the span associated with this error.
    pub fn span(&self) -> &Span {
        match self {
            ValidationError::InvalidOwnerFormat { span, .. } => span,
            ValidationError::InvalidPatternSyntax { span, .. } => span,
            ValidationError::UnsupportedPatternSyntax { span, .. } => span,
        }
    }

    /// Returns the line number where this error occurred.
    pub fn line(&self) -> usize {
        match self {
            ValidationError::InvalidOwnerFormat { line, .. } => *line,
            ValidationError::InvalidPatternSyntax { line, .. } => *line,
            ValidationError::UnsupportedPatternSyntax { line, .. } => *line,
        }
    }

    /// Returns the severity of this error.
    pub fn severity(&self) -> Severity {
        match self {
            ValidationError::InvalidOwnerFormat { .. } => Severity::Error,
            ValidationError::InvalidPatternSyntax { .. } => Severity::Error,
            ValidationError::UnsupportedPatternSyntax { .. } => Severity::Warning,
        }
    }
}

/// The result of validating a CODEOWNERS file.
#[derive(Debug, Clone, Default)]
pub struct ValidationResult {
    /// All validation errors found.
    pub errors: Vec<ValidationError>,
}

impl ValidationResult {
    /// Creates a new empty validation result.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a validation result with the given errors.
    pub fn with_errors(errors: Vec<ValidationError>) -> Self {
        Self { errors }
    }

    /// Returns true if validation passed with no errors.
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    /// Returns true if there are validation errors.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Returns only errors (not warnings).
    pub fn errors_only(&self) -> impl Iterator<Item = &ValidationError> {
        self.errors
            .iter()
            .filter(|e| e.severity() == Severity::Error)
    }

    /// Returns only warnings.
    pub fn warnings_only(&self) -> impl Iterator<Item = &ValidationError> {
        self.errors
            .iter()
            .filter(|e| e.severity() == Severity::Warning)
    }

    /// Adds an error to the result.
    pub fn add_error(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    /// Merges another validation result into this one.
    pub fn merge(&mut self, other: ValidationResult) {
        self.errors.extend(other.errors);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_span() -> Span {
        Span::new(10, 2, 5, 15)
    }

    #[test]
    fn validation_error_invalid_owner_format() {
        let error = ValidationError::invalid_owner_format("badowner", "must start with @", test_span());
        assert!(matches!(error, ValidationError::InvalidOwnerFormat { line: 2, .. }));
        assert_eq!(error.severity(), Severity::Error);
        assert!(error.to_string().contains("badowner"));
        assert!(error.to_string().contains("must start with @"));
    }

    #[test]
    fn validation_error_invalid_pattern_syntax() {
        let error = ValidationError::invalid_pattern_syntax("[abc]", "character classes not supported", test_span());
        assert!(matches!(error, ValidationError::InvalidPatternSyntax { line: 2, .. }));
        assert_eq!(error.severity(), Severity::Error);
    }

    #[test]
    fn validation_error_unsupported_pattern_syntax() {
        let error = ValidationError::unsupported_pattern_syntax("!ignore", "negation not supported", test_span());
        assert!(matches!(error, ValidationError::UnsupportedPatternSyntax { line: 2, .. }));
        assert_eq!(error.severity(), Severity::Warning);
    }

    #[test]
    fn severity_ordering() {
        assert!(Severity::Warning < Severity::Error);
    }

    #[test]
    fn validation_result_empty() {
        let result = ValidationResult::new();
        assert!(result.is_ok());
        assert!(!result.has_errors());
    }

    #[test]
    fn validation_result_with_errors() {
        let errors = vec![
            ValidationError::invalid_owner_format("bad", "reason", test_span()),
        ];
        let result = ValidationResult::with_errors(errors);
        assert!(!result.is_ok());
        assert!(result.has_errors());
    }

    #[test]
    fn validation_result_filter_by_severity() {
        let errors = vec![
            ValidationError::invalid_owner_format("bad", "reason", test_span()),
            ValidationError::unsupported_pattern_syntax("!x", "negation", test_span()),
        ];
        let result = ValidationResult::with_errors(errors);
        
        let errors_only: Vec<_> = result.errors_only().collect();
        let warnings_only: Vec<_> = result.warnings_only().collect();
        
        assert_eq!(errors_only.len(), 1);
        assert_eq!(warnings_only.len(), 1);
    }

    #[test]
    fn validation_result_add_and_merge() {
        let mut result1 = ValidationResult::new();
        result1.add_error(ValidationError::invalid_owner_format("a", "r", test_span()));
        
        let mut result2 = ValidationResult::new();
        result2.add_error(ValidationError::invalid_owner_format("b", "r", test_span()));
        
        result1.merge(result2);
        assert_eq!(result1.errors.len(), 2);
    }
}
