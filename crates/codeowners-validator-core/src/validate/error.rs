//! Error types for CODEOWNERS validation.
//!
//! This module defines validation error types that describe
//! semantic issues found after parsing.

use crate::parse::span::Span;
use serde::Serialize;
use thiserror::Error;

/// The severity of a validation issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// A warning that doesn't prevent the file from working.
    Warning,
    /// An error that may cause unexpected behavior.
    Error,
}

/// A validation error found in a CODEOWNERS file.
#[derive(Debug, Clone, Error, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
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

    /// Duplicate pattern found.
    #[error("line {line}: duplicate pattern '{pattern}' (first defined on line {first_line})")]
    DuplicatePattern {
        /// The line number where the duplicate was found (1-based).
        line: usize,
        /// The duplicate pattern text.
        pattern: String,
        /// The line where the pattern was first defined.
        first_line: usize,
        /// Location of the duplicate pattern.
        span: Span,
    },

    /// Pattern doesn't match any files in the repository.
    #[error("line {line}: pattern '{pattern}' does not match any files")]
    PatternNotMatching {
        /// The line number (1-based).
        line: usize,
        /// The pattern that doesn't match.
        pattern: String,
        /// Location of the pattern.
        span: Span,
    },

    /// Owner not found on GitHub.
    #[error("line {line}: owner '{owner}' not found on GitHub - {reason}")]
    OwnerNotFound {
        /// The line number (1-based).
        line: usize,
        /// The owner that wasn't found.
        owner: String,
        /// Additional details about why the owner wasn't found.
        reason: String,
        /// Location of the owner.
        span: Span,
    },

    /// GitHub API authorization insufficient for the check.
    #[error("line {line}: insufficient authorization to verify owner '{owner}' - {reason}")]
    InsufficientAuthorization {
        /// The line number (1-based).
        line: usize,
        /// The owner being checked.
        owner: String,
        /// Details about the authorization failure.
        reason: String,
        /// Location of the owner.
        span: Span,
    },

    /// File in repository has no CODEOWNERS coverage.
    #[error("file '{path}' is not covered by any CODEOWNERS rule")]
    FileNotOwned {
        /// The file path that isn't covered.
        path: String,
    },

    /// A pattern is shadowed by an earlier, less specific pattern.
    #[error("line {line}: pattern '{pattern}' is shadowed by pattern '{shadowing_pattern}' on line {shadowing_line}")]
    PatternShadowed {
        /// The line number of the shadowed pattern (1-based).
        line: usize,
        /// The pattern that is being shadowed.
        pattern: String,
        /// The line number of the shadowing pattern.
        shadowing_line: usize,
        /// The pattern that is doing the shadowing.
        shadowing_pattern: String,
        /// Location of the shadowed pattern.
        span: Span,
    },

    /// Owner must be a team but a user was specified.
    #[error("line {line}: owner '{owner}' must be a team (@org/team), not a user")]
    OwnerMustBeTeam {
        /// The line number (1-based).
        line: usize,
        /// The owner that should be a team.
        owner: String,
        /// Location of the owner.
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

    /// Creates a duplicate pattern error.
    pub fn duplicate_pattern(
        pattern: impl Into<String>,
        span: Span,
        first_line: usize,
    ) -> Self {
        Self::DuplicatePattern {
            line: span.line,
            pattern: pattern.into(),
            first_line,
            span,
        }
    }

    /// Creates a pattern not matching error.
    pub fn pattern_not_matching(pattern: impl Into<String>, span: Span) -> Self {
        Self::PatternNotMatching {
            line: span.line,
            pattern: pattern.into(),
            span,
        }
    }

    /// Creates an owner not found error.
    pub fn owner_not_found(
        owner: impl Into<String>,
        reason: impl Into<String>,
        span: Span,
    ) -> Self {
        Self::OwnerNotFound {
            line: span.line,
            owner: owner.into(),
            reason: reason.into(),
            span,
        }
    }

    /// Creates an insufficient authorization error.
    pub fn insufficient_authorization(
        owner: impl Into<String>,
        reason: impl Into<String>,
        span: Span,
    ) -> Self {
        Self::InsufficientAuthorization {
            line: span.line,
            owner: owner.into(),
            reason: reason.into(),
            span,
        }
    }

    /// Creates a file not owned error.
    pub fn file_not_owned(path: impl Into<String>) -> Self {
        Self::FileNotOwned { path: path.into() }
    }

    /// Creates a pattern shadowed error.
    pub fn pattern_shadowed(
        pattern: impl Into<String>,
        span: Span,
        shadowing_pattern: impl Into<String>,
        shadowing_line: usize,
    ) -> Self {
        Self::PatternShadowed {
            line: span.line,
            pattern: pattern.into(),
            shadowing_line,
            shadowing_pattern: shadowing_pattern.into(),
            span,
        }
    }

    /// Creates an owner must be team error.
    pub fn owner_must_be_team(owner: impl Into<String>, span: Span) -> Self {
        Self::OwnerMustBeTeam {
            line: span.line,
            owner: owner.into(),
            span,
        }
    }

    /// Returns the span associated with this error, if available.
    pub fn span(&self) -> Option<&Span> {
        match self {
            ValidationError::InvalidOwnerFormat { span, .. } => Some(span),
            ValidationError::InvalidPatternSyntax { span, .. } => Some(span),
            ValidationError::UnsupportedPatternSyntax { span, .. } => Some(span),
            ValidationError::DuplicatePattern { span, .. } => Some(span),
            ValidationError::PatternNotMatching { span, .. } => Some(span),
            ValidationError::OwnerNotFound { span, .. } => Some(span),
            ValidationError::InsufficientAuthorization { span, .. } => Some(span),
            ValidationError::FileNotOwned { .. } => None,
            ValidationError::PatternShadowed { span, .. } => Some(span),
            ValidationError::OwnerMustBeTeam { span, .. } => Some(span),
        }
    }

    /// Returns the line number where this error occurred, if available.
    pub fn line(&self) -> Option<usize> {
        match self {
            ValidationError::InvalidOwnerFormat { line, .. } => Some(*line),
            ValidationError::InvalidPatternSyntax { line, .. } => Some(*line),
            ValidationError::UnsupportedPatternSyntax { line, .. } => Some(*line),
            ValidationError::DuplicatePattern { line, .. } => Some(*line),
            ValidationError::PatternNotMatching { line, .. } => Some(*line),
            ValidationError::OwnerNotFound { line, .. } => Some(*line),
            ValidationError::InsufficientAuthorization { line, .. } => Some(*line),
            ValidationError::FileNotOwned { .. } => None,
            ValidationError::PatternShadowed { line, .. } => Some(*line),
            ValidationError::OwnerMustBeTeam { line, .. } => Some(*line),
        }
    }

    /// Returns the severity of this error.
    pub fn severity(&self) -> Severity {
        match self {
            ValidationError::InvalidOwnerFormat { .. } => Severity::Error,
            ValidationError::InvalidPatternSyntax { .. } => Severity::Error,
            ValidationError::UnsupportedPatternSyntax { .. } => Severity::Warning,
            ValidationError::DuplicatePattern { .. } => Severity::Warning,
            ValidationError::PatternNotMatching { .. } => Severity::Warning,
            ValidationError::OwnerNotFound { .. } => Severity::Error,
            ValidationError::InsufficientAuthorization { .. } => Severity::Error,
            ValidationError::FileNotOwned { .. } => Severity::Warning,
            ValidationError::PatternShadowed { .. } => Severity::Warning,
            ValidationError::OwnerMustBeTeam { .. } => Severity::Error,
        }
    }
}

/// The result of validating a CODEOWNERS file.
#[derive(Debug, Clone, Default, Serialize)]
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
    fn validation_error_duplicate_pattern() {
        let error = ValidationError::duplicate_pattern("*.rs", test_span(), 1);
        assert!(matches!(error, ValidationError::DuplicatePattern { line: 2, first_line: 1, .. }));
        assert_eq!(error.severity(), Severity::Warning);
        assert!(error.to_string().contains("duplicate"));
        assert!(error.to_string().contains("*.rs"));
    }

    #[test]
    fn validation_error_pattern_not_matching() {
        let error = ValidationError::pattern_not_matching("/nonexistent/", test_span());
        assert!(matches!(error, ValidationError::PatternNotMatching { line: 2, .. }));
        assert_eq!(error.severity(), Severity::Warning);
    }

    #[test]
    fn validation_error_owner_not_found() {
        let error = ValidationError::owner_not_found("@ghost", "user does not exist", test_span());
        assert!(matches!(error, ValidationError::OwnerNotFound { line: 2, .. }));
        assert_eq!(error.severity(), Severity::Error);
    }

    #[test]
    fn validation_error_insufficient_authorization() {
        let error = ValidationError::insufficient_authorization("@org/team", "requires read:org scope", test_span());
        assert!(matches!(error, ValidationError::InsufficientAuthorization { line: 2, .. }));
        assert_eq!(error.severity(), Severity::Error);
    }

    #[test]
    fn validation_error_file_not_owned() {
        let error = ValidationError::file_not_owned("src/main.rs");
        assert!(matches!(error, ValidationError::FileNotOwned { .. }));
        assert_eq!(error.severity(), Severity::Warning);
        assert!(error.line().is_none());
        assert!(error.span().is_none());
    }

    #[test]
    fn validation_error_pattern_shadowed() {
        let error = ValidationError::pattern_shadowed("src/*.rs", test_span(), "*", 1);
        assert!(matches!(error, ValidationError::PatternShadowed { line: 2, shadowing_line: 1, .. }));
        assert_eq!(error.severity(), Severity::Warning);
    }

    #[test]
    fn validation_error_owner_must_be_team() {
        let error = ValidationError::owner_must_be_team("@user", test_span());
        assert!(matches!(error, ValidationError::OwnerMustBeTeam { line: 2, .. }));
        assert_eq!(error.severity(), Severity::Error);
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
