//! Pattern shadowing detection check.
//!
//! This check detects when earlier, less-specific patterns shadow
//! later, more-specific patterns in a CODEOWNERS file.

use super::{Check, CheckContext};
use crate::parse::LineKind;
use crate::matching::Pattern;
use crate::validate::{ValidationError, ValidationResult};

/// A check that detects pattern shadowing.
///
/// In CODEOWNERS, later patterns take precedence over earlier ones for the same file.
/// However, if a more general pattern appears before a more specific one, the specific
/// pattern effectively shadows (overrides) the general one, which may indicate:
/// - Incorrect pattern ordering
/// - Redundant patterns
/// - Potential confusion about ownership
///
/// Example of shadowing:
/// ```text
/// *           @default-owner    # General catch-all
/// /src/*.rs   @rust-team        # More specific - this works correctly
/// ```
///
/// Example of problematic ordering:
/// ```text
/// /src/       @backend-team     # More general
/// /src/api/   @api-team         # More specific - this is fine (later wins)
/// ```
///
/// This check warns when a more specific pattern appears AFTER a less specific one
/// that would have matched the same files, as the earlier pattern gets shadowed.
#[derive(Debug, Clone, Default)]
pub struct AvoidShadowingCheck;

impl AvoidShadowingCheck {
    /// Creates a new shadowing detection check.
    pub fn new() -> Self {
        Self
    }

    /// Checks if pattern `a` could shadow pattern `b`.
    ///
    /// Returns true if `a` is less specific than `b` but would match
    /// some of the same paths.
    fn could_shadow(general: &CompiledPattern, specific: &CompiledPattern) -> bool {
        // A pattern shadows another if:
        // 1. The general pattern is less specific (lower specificity score)
        // 2. The general pattern would match paths that the specific pattern matches

        if general.specificity >= specific.specificity {
            // General pattern is not less specific
            return false;
        }

        // Check if the general pattern could match paths the specific one matches
        // We use heuristics here since exact matching is complex
        Self::patterns_overlap(&general.text, &specific.text)
    }

    /// Heuristically checks if two patterns could match overlapping paths.
    fn patterns_overlap(general: &str, specific: &str) -> bool {
        let general_trimmed = general.trim_matches('/');
        let specific_trimmed = specific.trim_matches('/');

        // Wildcard patterns overlap with everything
        if general_trimmed == "*" || general_trimmed == "**" {
            return true;
        }

        // Check if specific path starts with general path
        // e.g., /src/ overlaps with /src/api/
        if specific_trimmed.starts_with(general_trimmed) {
            return true;
        }

        // Check for glob pattern overlap
        // e.g., *.rs overlaps with src/*.rs
        if general_trimmed.contains('*') {
            // Extract the non-wildcard suffix
            if let Some(suffix) = general_trimmed.strip_prefix('*')
                && specific_trimmed.ends_with(suffix) {
                    return true;
                }
            // Check if specific contains the same extension pattern
            if general_trimmed.starts_with("*.") && specific_trimmed.contains(&general_trimmed[1..]) {
                return true;
            }
        }

        // Check if patterns share a common path prefix
        let general_parts: Vec<&str> = general_trimmed.split('/').collect();
        let specific_parts: Vec<&str> = specific_trimmed.split('/').collect();

        if !general_parts.is_empty() && !specific_parts.is_empty() {
            // If first non-wildcard parts match, they might overlap
            let general_first = general_parts.iter().find(|p| !p.contains('*'));
            let specific_first = specific_parts.iter().find(|p| !p.contains('*'));

            if let (Some(g), Some(s)) = (general_first, specific_first)
                && g == s {
                    return true;
                }
        }

        false
    }
}

/// A compiled pattern with metadata for shadowing analysis.
struct CompiledPattern {
    text: String,
    specificity: u32,
    line: usize,
    span: crate::parse::Span,
}

impl Check for AvoidShadowingCheck {
    fn name(&self) -> &'static str {
        "avoid-shadowing"
    }

    fn run(&self, ctx: &CheckContext) -> ValidationResult {
        let mut result = ValidationResult::new();

        // Collect all patterns with their metadata
        let mut patterns: Vec<CompiledPattern> = Vec::new();

        for line in &ctx.file.lines {
            if let LineKind::Rule { pattern, .. } = &line.kind
                && let Some(compiled) = Pattern::new(&pattern.text) {
                    patterns.push(CompiledPattern {
                        text: pattern.text.clone(),
                        specificity: compiled.specificity(),
                        line: line.span.line,
                        span: pattern.span,
                    });
                }
        }

        // Check for shadowing: compare each pattern with all subsequent patterns
        for i in 0..patterns.len() {
            for j in (i + 1)..patterns.len() {
                let earlier = &patterns[i];
                let later = &patterns[j];

                // Check if the earlier (more general) pattern shadows the later (more specific) one
                // Note: In CODEOWNERS, later patterns win, so if a general pattern comes first
                // and a specific one comes later, the specific one will override for matching files.
                // This is actually correct behavior, but we warn about potential confusion.

                // We warn when:
                // - Earlier pattern is more general (lower specificity)
                // - Patterns could match overlapping files
                // This indicates the earlier pattern's owners will be shadowed for specific files
                if Self::could_shadow(earlier, later) {
                    result.add_error(ValidationError::pattern_shadowed(
                        &earlier.text,
                        earlier.span,
                        &later.text,
                        later.line,
                    ));
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
        AvoidShadowingCheck::new().run(&ctx)
    }

    #[test]
    fn no_shadowing_specific_first() {
        // When specific patterns come before general ones, no shadowing warning is needed
        // because the ownership is clear (later patterns win, so general pattern takes over
        // for non-matching files)
        // However, our check warns when ANY general pattern precedes a specific one
        // In this case: /src/api/ is specific, /src/ is more general but comes later (OK)
        // Then * is most general and comes last
        // But /src/ before * means /src/ gets shadowed by *... but actually * comes after /src/
        // so * would shadow /src/, and /src/ shadows /src/api/
        // This test should pass with the corrected logic
        let result = run_check("/src/api/ @api-team\n/src/ @backend-team\n* @default\n");
        // Here /src/api/ < /src/ < * in order
        // /src/api/ is more specific than /src/, and /src/ is before /src/api/ in our check... no wait
        // The check iterates i < j, so earlier vs later
        // /src/api/ (i=0) vs /src/ (j=1): /src/api/ specificity > /src/ specificity, so no shadow
        // /src/api/ (i=0) vs * (j=2): /src/api/ specificity > * specificity, so no shadow
        // /src/ (i=1) vs * (j=2): /src/ specificity > * specificity, so no shadow
        // No errors expected when going specific -> general order
        assert!(result.is_ok());
    }

    #[test]
    fn shadowing_general_first() {
        // General pattern first shadows later specific patterns
        let result = run_check("* @default\n/src/ @src-team\n");
        assert!(result.has_errors());

        match &result.errors[0] {
            ValidationError::PatternShadowed {
                pattern,
                shadowing_pattern,
                ..
            } => {
                assert_eq!(pattern, "*");
                assert_eq!(shadowing_pattern, "/src/");
            }
            _ => panic!("Expected PatternShadowed error"),
        }
    }

    #[test]
    fn no_overlap_no_shadowing() {
        let result = run_check("*.rs @rust\n*.md @docs\n");
        assert!(result.is_ok());
    }

    #[test]
    fn directory_shadowing() {
        let result = run_check("/src/ @general\n/src/api/ @specific\n");
        assert!(result.has_errors());
    }

    #[test]
    fn extension_shadowing() {
        let result = run_check("*.rs @all-rust\n/src/*.rs @src-rust\n");
        assert!(result.has_errors());
    }

    #[test]
    fn same_specificity_no_shadowing() {
        // Same specificity patterns don't shadow each other
        let result = run_check("/src/ @team-a\n/docs/ @team-b\n");
        assert!(result.is_ok());
    }

    #[test]
    fn single_pattern() {
        let result = run_check("* @owner\n");
        assert!(result.is_ok());
    }

    #[test]
    fn empty_file() {
        let result = run_check("");
        assert!(result.is_ok());
    }

    #[test]
    fn comments_ignored() {
        let result = run_check("# Comment\n* @owner\n# Another comment\n");
        assert!(result.is_ok());
    }

    #[test]
    fn wildcard_shadows_everything() {
        let result = run_check("* @default\n/specific/path/ @team\n*.rs @rust\n");
        // * shadows both later patterns
        assert_eq!(result.errors.len(), 2);
    }

    #[test]
    fn double_wildcard_shadows() {
        let result = run_check("** @default\n/src/ @team\n");
        assert!(result.has_errors());
    }

    #[test]
    fn patterns_overlap_function() {
        assert!(AvoidShadowingCheck::patterns_overlap("*", "src/main.rs"));
        assert!(AvoidShadowingCheck::patterns_overlap("src/", "src/api/"));
        assert!(AvoidShadowingCheck::patterns_overlap("*.rs", "src/*.rs"));
        assert!(!AvoidShadowingCheck::patterns_overlap("src/", "docs/"));
        assert!(!AvoidShadowingCheck::patterns_overlap("*.rs", "*.md"));
    }

    #[test]
    fn check_name() {
        let check = AvoidShadowingCheck::new();
        assert_eq!(check.name(), "avoid-shadowing");
    }
}
