//! Syntax validation check.
//!
//! This check validates owner formats and pattern syntax.

use super::{Check, CheckContext};
use crate::validate::syntax::validate_syntax as validate_syntax_impl;
use crate::validate::ValidationResult;

/// A check that validates CODEOWNERS syntax.
///
/// This includes:
/// - Owner format validation (@user, @org/team, email)
/// - Pattern syntax validation (no unsupported gitignore features)
#[derive(Debug, Clone, Default)]
pub struct SyntaxCheck;

impl SyntaxCheck {
    /// Creates a new syntax check.
    pub fn new() -> Self {
        Self
    }
}

impl Check for SyntaxCheck {
    fn name(&self) -> &'static str {
        "syntax"
    }

    fn run(&self, ctx: &CheckContext) -> ValidationResult {
        validate_syntax_impl(ctx.file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_codeowners;
    use std::path::PathBuf;
    use crate::validate::checks::CheckConfig;

    fn run_check(input: &str) -> ValidationResult {
        let file = parse_codeowners(input).ast;
        let path = PathBuf::from("/repo");
        let config = CheckConfig::new();
        let ctx = CheckContext::new(&file, &path, &config);
        SyntaxCheck::new().run(&ctx)
    }

    #[test]
    fn valid_syntax() {
        let result = run_check("*.rs @owner\n/docs/ @github/team user@example.com\n");
        assert!(result.is_ok());
    }

    #[test]
    fn invalid_owner_format() {
        let result = run_check("*.rs @-invalid\n");
        assert!(result.has_errors());
    }

    #[test]
    fn unsupported_pattern() {
        let result = run_check("!*.log @owner\n");
        assert!(result.has_errors());
    }

    #[test]
    fn character_class_not_supported() {
        let result = run_check("*.[ch] @owner\n");
        assert!(result.has_errors());
    }
}
