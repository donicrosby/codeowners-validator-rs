//! Duplicate pattern detection check.
//!
//! This check detects when the same pattern appears multiple times in a CODEOWNERS file.

use super::{Check, CheckContext};
use crate::parse::LineKind;
use crate::validate::{ValidationError, ValidationResult};
use std::collections::HashMap;

/// A check that detects duplicate patterns in CODEOWNERS files.
///
/// Duplicate patterns can lead to confusion about ownership and
/// may indicate copy-paste errors.
#[derive(Debug, Clone, Default)]
pub struct DupPatternsCheck;

impl DupPatternsCheck {
    /// Creates a new duplicate patterns check.
    pub fn new() -> Self {
        Self
    }
}

impl Check for DupPatternsCheck {
    fn name(&self) -> &'static str {
        "duppatterns"
    }

    fn run(&self, ctx: &CheckContext) -> ValidationResult {
        let mut result = ValidationResult::new();
        
        // Track patterns we've seen: pattern text -> (first line number, first span)
        let mut seen: HashMap<&str, (usize, crate::parse::Span)> = HashMap::new();
        
        for line in &ctx.file.lines {
            if let LineKind::Rule { pattern, .. } = &line.kind {
                let pattern_text = pattern.text.as_str();
                
                if let Some(&(first_line, _)) = seen.get(pattern_text) {
                    // Found a duplicate
                    result.add_error(ValidationError::duplicate_pattern(
                        pattern_text,
                        pattern.span,
                        first_line,
                    ));
                } else {
                    // First occurrence
                    seen.insert(pattern_text, (line.span.line, pattern.span));
                }
            }
        }
        
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_codeowners;
    use crate::validate::checks::CheckConfig;
    use std::path::PathBuf;

    fn run_check(input: &str) -> ValidationResult {
        let file = parse_codeowners(input).ast;
        let path = PathBuf::from("/repo");
        let config = CheckConfig::new();
        let ctx = CheckContext::new(&file, &path, &config);
        DupPatternsCheck::new().run(&ctx)
    }

    #[test]
    fn no_duplicates() {
        let result = run_check("*.rs @rust\n*.md @docs\n/src/ @dev\n");
        assert!(result.is_ok());
    }

    #[test]
    fn exact_duplicate() {
        let result = run_check("*.rs @owner1\n*.rs @owner2\n");
        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 1);
        
        match &result.errors[0] {
            ValidationError::DuplicatePattern { pattern, line, first_line, .. } => {
                assert_eq!(pattern, "*.rs");
                assert_eq!(*line, 2);
                assert_eq!(*first_line, 1);
            }
            _ => panic!("Expected DuplicatePattern error"),
        }
    }

    #[test]
    fn multiple_duplicates() {
        let result = run_check("*.rs @a\n*.md @b\n*.rs @c\n*.md @d\n");
        assert_eq!(result.errors.len(), 2);
    }

    #[test]
    fn same_pattern_different_case_not_duplicate() {
        // Patterns are case-sensitive
        let result = run_check("*.RS @a\n*.rs @b\n");
        assert!(result.is_ok());
    }

    #[test]
    fn three_occurrences() {
        let result = run_check("*.rs @a\n*.rs @b\n*.rs @c\n");
        // Should report two duplicates (lines 2 and 3)
        assert_eq!(result.errors.len(), 2);
    }

    #[test]
    fn different_patterns_no_duplicate() {
        let result = run_check("/src/*.rs @a\nsrc/*.rs @b\n");
        // These are different patterns (anchored vs not)
        assert!(result.is_ok());
    }

    #[test]
    fn empty_file() {
        let result = run_check("");
        assert!(result.is_ok());
    }

    #[test]
    fn only_comments() {
        let result = run_check("# Comment 1\n# Comment 2\n");
        assert!(result.is_ok());
    }
}
