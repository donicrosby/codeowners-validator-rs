//! Line and file-level parsers for CODEOWNERS files.
//!
//! This module combines the lexer components to parse complete lines
//! and entire CODEOWNERS files.

use super::ast::{CodeownersFile, Line, Owner};
use super::error::{ParseError, ParseResult};
use super::lexer::{
    is_blank_line, make_owner, make_pattern, parse_comment_line, parse_pattern_only,
    parse_rule_components,
};
use super::span::Span;
use log::{debug, trace};

/// Configuration options for the parser.
#[derive(Debug, Clone, Default)]
pub struct ParserConfig {
    /// If true, parsing stops at the first error (strict mode).
    /// If false, errors are collected and parsing continues (lenient mode).
    pub strict: bool,
    /// If true, patterns without owners are allowed (creates rules with empty owner list).
    /// If false, patterns without owners are parse errors.
    pub allow_unowned_patterns: bool,
}

impl ParserConfig {
    /// Creates a new parser config with default settings (lenient mode).
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a strict mode parser config.
    pub fn strict() -> Self {
        Self {
            strict: true,
            ..Default::default()
        }
    }

    /// Creates a lenient mode parser config.
    pub fn lenient() -> Self {
        Self {
            strict: false,
            ..Default::default()
        }
    }

    /// Sets whether unowned patterns are allowed.
    pub fn with_allow_unowned_patterns(mut self, value: bool) -> Self {
        self.allow_unowned_patterns = value;
        self
    }
}

/// Parses a single line of a CODEOWNERS file.
///
/// Returns the parsed Line AST node, or an error if the line is invalid.
fn parse_line(
    line_text: &str,
    line_num: usize,
    line_offset: usize,
    config: &ParserConfig,
) -> Result<Line, ParseError> {
    let line_span = Span::new(line_offset, line_num, 1, line_text.len());

    // Check for blank line
    if is_blank_line(line_text) {
        return Ok(Line::blank(line_span));
    }

    // Check for comment line
    if let Ok((_, comment_content)) = parse_comment_line(line_text) {
        return Ok(Line::comment(comment_content, line_span));
    }

    // Try to parse as a rule line (pattern + owners)
    match parse_rule_components(line_text) {
        Ok((_remaining, components)) => {
            // Create pattern span
            let pattern_span = Span::new(
                line_offset + components.pattern_offset,
                line_num,
                components.pattern_offset + 1,
                components.pattern.len(),
            );
            let pattern = make_pattern(components.pattern, pattern_span);

            // Create owner nodes with spans
            let owners: Vec<Owner> = components
                .owners
                .iter()
                .zip(components.owner_offsets.iter())
                .map(|(owner_text, &offset)| {
                    let owner_span =
                        Span::new(line_offset + offset, line_num, offset + 1, owner_text.len());
                    make_owner(owner_text, owner_span)
                })
                .collect();

            Ok(Line::rule(pattern, owners, line_span))
        }
        Err(_) => {
            // Check if it looks like a pattern with no owners
            let trimmed = line_text.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                // It has content but no owners
                if config.allow_unowned_patterns {
                    // Parse just the pattern and create a rule with empty owners
                    if let Ok((_, pattern_only)) = parse_pattern_only(line_text) {
                        let pattern_span = Span::new(
                            line_offset + pattern_only.pattern_offset,
                            line_num,
                            pattern_only.pattern_offset + 1,
                            pattern_only.pattern.len(),
                        );
                        let pattern = make_pattern(pattern_only.pattern, pattern_span);
                        return Ok(Line::rule(pattern, Vec::new(), line_span));
                    }
                }
                // Not allowed or couldn't parse pattern - error
                let error_span = Span::new(line_offset, line_num, 1, line_text.len());
                Err(ParseError::missing_owners(error_span))
            } else {
                let error_span = Span::new(line_offset, line_num, 1, line_text.len());
                Err(ParseError::invalid_line("could not parse line", error_span))
            }
        }
    }
}

/// Parses a CODEOWNERS file with the given configuration.
pub fn parse_codeowners_with_config(input: &str, config: &ParserConfig) -> ParseResult {
    debug!(
        "Parsing CODEOWNERS file ({} bytes, strict={})",
        input.len(),
        config.strict
    );
    let mut lines = Vec::new();
    let mut errors = Vec::new();
    let mut offset = 0;
    let mut remaining = input;

    for (line_idx, line_text) in input.lines().enumerate() {
        let line_num = line_idx + 1; // 1-based line numbers

        match parse_line(line_text, line_num, offset, config) {
            Ok(line) => {
                trace!("Line {}: parsed successfully", line_num);
                lines.push(line);
            }
            Err(error) => {
                debug!("Line {}: parse error - {}", line_num, error);
                if config.strict {
                    // In strict mode, return immediately on first error
                    debug!("Strict mode: stopping at first error");
                    return ParseResult::with_errors(CodeownersFile::new(lines), vec![error]);
                } else {
                    // In lenient mode, record the error and add an Invalid line
                    let line_span = Span::new(offset, line_num, 1, line_text.len());
                    lines.push(Line::invalid(line_text, error.to_string(), line_span));
                    errors.push(error);
                }
            }
        }

        // Calculate actual byte offset for next line by examining the original input.
        // This correctly handles both Unix (\n) and Windows (\r\n) line endings.
        let line_with_ending_len = if remaining.len() > line_text.len() {
            let after_content = &remaining[line_text.len()..];
            if after_content.starts_with("\r\n") {
                line_text.len() + 2 // CRLF
            } else if after_content.starts_with('\n') {
                line_text.len() + 1 // LF
            } else {
                // Last line without trailing newline
                line_text.len()
            }
        } else {
            // Last line, no more content
            line_text.len()
        };

        offset += line_with_ending_len;
        remaining = &remaining[line_with_ending_len..];
    }

    let ast = CodeownersFile::new(lines);

    debug!(
        "Parsing complete: {} lines, {} errors",
        ast.lines.len(),
        errors.len()
    );
    if errors.is_empty() {
        ParseResult::ok(ast)
    } else {
        ParseResult::with_errors(ast, errors)
    }
}

/// Parses a CODEOWNERS file using default (lenient) configuration.
pub fn parse_codeowners(input: &str) -> ParseResult {
    parse_codeowners_with_config(input, &ParserConfig::default())
}

/// Parses a CODEOWNERS file in strict mode, stopping at first error.
pub fn parse_codeowners_strict(input: &str) -> ParseResult {
    parse_codeowners_with_config(input, &ParserConfig::strict())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{LineKind, Owner};

    #[test]
    fn parse_empty_file() {
        let result = parse_codeowners("");
        assert!(result.is_ok());
        assert_eq!(result.ast.lines.len(), 0);
    }

    #[test]
    fn parse_blank_lines() {
        let input = "\n   \n\t\n";
        let result = parse_codeowners(input);
        assert!(result.is_ok());

        for line in &result.ast.lines {
            assert!(line.is_blank());
        }
    }

    #[test]
    fn parse_comment_line() {
        let input = "# This is a comment\n";
        let result = parse_codeowners(input);
        assert!(result.is_ok());
        assert_eq!(result.ast.lines.len(), 1);
        assert!(result.ast.lines[0].is_comment());

        if let LineKind::Comment { content } = &result.ast.lines[0].kind {
            assert_eq!(content, " This is a comment");
        } else {
            panic!("Expected comment");
        }
    }

    #[test]
    fn parse_comment_with_leading_whitespace() {
        let input = "   # Indented comment\n";
        let result = parse_codeowners(input);
        assert!(result.is_ok());
        assert!(result.ast.lines[0].is_comment());
    }

    #[test]
    fn parse_simple_rule() {
        let input = "*.rs @rustacean\n";
        let result = parse_codeowners(input);
        assert!(result.is_ok());
        assert_eq!(result.ast.lines.len(), 1);

        let line = &result.ast.lines[0];
        assert!(line.is_rule());

        if let LineKind::Rule { pattern, owners } = &line.kind {
            assert_eq!(pattern.text, "*.rs");
            assert_eq!(owners.len(), 1);
            assert!(matches!(&owners[0], Owner::User { name, .. } if name == "rustacean"));
        } else {
            panic!("Expected rule");
        }
    }

    #[test]
    fn parse_rule_with_multiple_owners() {
        let input = "/src/ @dev @github/core dev@example.com\n";
        let result = parse_codeowners(input);
        assert!(result.is_ok());

        let line = &result.ast.lines[0];
        if let LineKind::Rule { pattern, owners } = &line.kind {
            assert_eq!(pattern.text, "/src/");
            assert_eq!(owners.len(), 3);
            assert!(matches!(&owners[0], Owner::User { name, .. } if name == "dev"));
            assert!(
                matches!(&owners[1], Owner::Team { org, team, .. } if org == "github" && team == "core")
            );
            assert!(matches!(&owners[2], Owner::Email { email, .. } if email == "dev@example.com"));
        } else {
            panic!("Expected rule");
        }
    }

    #[test]
    fn parse_rule_with_glob_pattern() {
        let input = "**/*.js @frontend\n";
        let result = parse_codeowners(input);
        assert!(result.is_ok());

        let line = &result.ast.lines[0];
        if let LineKind::Rule { pattern, .. } = &line.kind {
            assert_eq!(pattern.text, "**/*.js");
        } else {
            panic!("Expected rule");
        }
    }

    #[test]
    fn parse_mixed_content() {
        let input = r#"# CODEOWNERS file

*.rs @rustacean
/docs/ @docs-team

# Frontend
*.js @frontend @github/web-team"#;
        let result = parse_codeowners(input);
        assert!(result.is_ok());
        assert_eq!(result.ast.lines.len(), 7);

        // Line 1: comment
        assert!(result.ast.lines[0].is_comment());
        // Line 2: blank
        assert!(result.ast.lines[1].is_blank());
        // Line 3: rule
        assert!(result.ast.lines[2].is_rule());
        // Line 4: rule
        assert!(result.ast.lines[3].is_rule());
        // Line 5: blank
        assert!(result.ast.lines[4].is_blank());
        // Line 6: comment
        assert!(result.ast.lines[5].is_comment());
        // Line 7: rule
        assert!(result.ast.lines[6].is_rule());
    }

    #[test]
    fn parse_rule_missing_owner_lenient() {
        let input = "*.rs\n*.js @frontend\n";
        let result = parse_codeowners(input);

        // Lenient mode should continue and collect error
        assert!(result.has_errors());
        assert_eq!(result.ast.lines.len(), 2);
        assert!(result.ast.lines[0].is_invalid());
        assert!(result.ast.lines[1].is_rule());
    }

    #[test]
    fn parse_rule_missing_owner_strict() {
        let input = "*.rs\n*.js @frontend\n";
        let result = parse_codeowners_strict(input);

        // Strict mode should stop at first error
        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 1);
    }

    #[test]
    fn span_positions_are_correct() {
        let input = "*.rs @owner\n/docs/ @team\n";
        let result = parse_codeowners(input);
        assert!(result.is_ok());

        // First line
        let line1 = &result.ast.lines[0];
        assert_eq!(line1.span.line, 1);
        assert_eq!(line1.span.offset, 0);

        // Second line
        let line2 = &result.ast.lines[1];
        assert_eq!(line2.span.line, 2);
        assert_eq!(line2.span.offset, 12); // After "*.rs @owner\n"
    }

    #[test]
    fn pattern_span_is_correct() {
        let input = "  *.rs @owner\n";
        let result = parse_codeowners(input);
        assert!(result.is_ok());

        if let LineKind::Rule { pattern, .. } = &result.ast.lines[0].kind {
            assert_eq!(pattern.span.column, 3); // After 2 spaces
            assert_eq!(pattern.span.length, 4); // "*.rs"
        } else {
            panic!("Expected rule");
        }
    }

    #[test]
    fn owner_spans_are_correct() {
        let input = "*.rs @alice @bob\n";
        let result = parse_codeowners(input);
        assert!(result.is_ok());

        if let LineKind::Rule { owners, .. } = &result.ast.lines[0].kind {
            assert_eq!(owners.len(), 2);

            // @alice starts at column 6 (after "*.rs ")
            assert_eq!(owners[0].span().column, 6);
            assert_eq!(owners[0].span().length, 6); // "@alice"

            // @bob starts at column 13 (after "*.rs @alice ")
            assert_eq!(owners[1].span().column, 13);
            assert_eq!(owners[1].span().length, 4); // "@bob"
        } else {
            panic!("Expected rule");
        }
    }

    #[test]
    fn parse_result_extract_rules() {
        let input = "# comment\n*.rs @rust\n*.js @js\n";
        let result = parse_codeowners(input);
        assert!(result.is_ok());

        let rules = result.ast.extract_rules();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].0.text, "*.rs");
        assert_eq!(rules[1].0.text, "*.js");
    }

    #[test]
    fn config_default_is_lenient() {
        let config = ParserConfig::default();
        assert!(!config.strict);
        assert!(!config.allow_unowned_patterns);
    }

    #[test]
    fn config_strict_mode() {
        let config = ParserConfig::strict();
        assert!(config.strict);
        assert!(!config.allow_unowned_patterns);
    }

    #[test]
    fn config_lenient_mode() {
        let config = ParserConfig::lenient();
        assert!(!config.strict);
        assert!(!config.allow_unowned_patterns);
    }

    #[test]
    fn config_allow_unowned_patterns() {
        let config = ParserConfig::new().with_allow_unowned_patterns(true);
        assert!(config.allow_unowned_patterns);
    }

    #[test]
    fn parse_unowned_pattern_when_allowed() {
        let config = ParserConfig::new().with_allow_unowned_patterns(true);
        let input = "*.rs\n*.js @frontend\n";
        let result = parse_codeowners_with_config(input, &config);

        // Should succeed without errors
        assert!(result.is_ok());
        assert_eq!(result.ast.lines.len(), 2);

        // First line should be a rule with empty owners
        if let LineKind::Rule { pattern, owners } = &result.ast.lines[0].kind {
            assert_eq!(pattern.text, "*.rs");
            assert!(owners.is_empty());
        } else {
            panic!("Expected rule with empty owners");
        }

        // Second line should be a rule with owner
        if let LineKind::Rule { pattern, owners } = &result.ast.lines[1].kind {
            assert_eq!(pattern.text, "*.js");
            assert_eq!(owners.len(), 1);
        } else {
            panic!("Expected rule with owner");
        }
    }

    #[test]
    fn parse_unowned_pattern_when_not_allowed() {
        let config = ParserConfig::new().with_allow_unowned_patterns(false);
        let input = "*.rs\n*.js @frontend\n";
        let result = parse_codeowners_with_config(input, &config);

        // Should have errors
        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 1);
    }

    #[test]
    fn parse_multiple_unowned_patterns() {
        let config = ParserConfig::new().with_allow_unowned_patterns(true);
        let input = "*.rs\n*.js\n*.md\n";
        let result = parse_codeowners_with_config(input, &config);

        assert!(result.is_ok());
        assert_eq!(result.ast.lines.len(), 3);

        for line in &result.ast.lines {
            if let LineKind::Rule { owners, .. } = &line.kind {
                assert!(owners.is_empty());
            } else {
                panic!("Expected rule");
            }
        }
    }

    #[test]
    fn unowned_pattern_span_is_correct() {
        let config = ParserConfig::new().with_allow_unowned_patterns(true);
        let input = "  /src/\n";
        let result = parse_codeowners_with_config(input, &config);

        assert!(result.is_ok());
        if let LineKind::Rule { pattern, .. } = &result.ast.lines[0].kind {
            assert_eq!(pattern.text, "/src/");
            assert_eq!(pattern.span.column, 3); // After 2 spaces
            assert_eq!(pattern.span.length, 5); // "/src/"
        } else {
            panic!("Expected rule");
        }
    }
}
