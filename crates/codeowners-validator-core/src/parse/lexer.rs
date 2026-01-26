//! Lexer and token parsers for CODEOWNERS files.
//!
//! This module contains nom-based parsers for individual tokens
//! like patterns, owners, and comments.

use nom::{
    IResult, Parser,
    bytes::complete::take_while1,
    character::complete::{char, space0, space1},
    combinator::rest,
};

use super::ast::{Owner, Pattern};
use super::span::Span;

/// Characters that can appear in a pattern (non-whitespace, non-comment).
fn is_pattern_char(c: char) -> bool {
    !c.is_whitespace() && c != '#'
}

/// Characters that can appear in an owner token.
fn is_owner_char(c: char) -> bool {
    !c.is_whitespace() && c != '#'
}

/// Parses a complete comment line (optional whitespace + # + content).
pub fn parse_comment_line(input: &str) -> IResult<&str, &str> {
    (space0, char('#'), rest)
        .map(|(_, _, content)| content)
        .parse(input)
}

/// Checks if a line is blank (empty or only whitespace).
pub fn is_blank_line(input: &str) -> bool {
    input.trim().is_empty()
}

/// Result of parsing a rule line's components.
#[derive(Debug, Clone)]
pub struct RuleComponents<'a> {
    /// The pattern text.
    pub pattern: &'a str,
    /// Byte offset of pattern start within the line.
    pub pattern_offset: usize,
    /// List of owner texts.
    pub owners: Vec<&'a str>,
    /// Byte offsets of each owner start within the line.
    pub owner_offsets: Vec<usize>,
}

/// Result of parsing just a pattern (for unowned patterns).
#[derive(Debug, Clone)]
pub struct PatternOnly<'a> {
    /// The pattern text.
    pub pattern: &'a str,
    /// Byte offset of pattern start within the line.
    pub pattern_offset: usize,
}

/// Parses just a pattern from a line (no owners required).
///
/// Used when `allow_unowned_patterns` is enabled.
pub fn parse_pattern_only(input: &str) -> IResult<&str, PatternOnly<'_>> {
    // Skip leading whitespace
    let (after_ws, leading_ws) = space0(input)?;
    let pattern_offset = leading_ws.len();

    // Parse pattern
    let (rest, pattern) = take_while1(is_pattern_char)(after_ws)?;

    Ok((
        rest,
        PatternOnly {
            pattern,
            pattern_offset,
        },
    ))
}

/// Parses the components of a rule line (pattern + owners).
///
/// This parser extracts the raw text and offsets without constructing
/// AST nodes, allowing the caller to add span information.
pub fn parse_rule_components(input: &str) -> IResult<&str, RuleComponents<'_>> {
    // Skip leading whitespace
    let (after_ws, leading_ws) = space0(input)?;
    let pattern_offset = leading_ws.len();

    // Parse pattern
    let (after_pattern, pattern) = take_while1(is_pattern_char)(after_ws)?;

    // Parse separator (whitespace between pattern and first owner)
    let (after_sep, separator) = space1(after_pattern)?;

    // Parse owners (one or more)
    let mut owners = Vec::new();
    let mut owner_offsets = Vec::new();
    let mut current = after_sep;
    // Calculate offset using string lengths instead of pointer arithmetic
    let mut current_offset = pattern_offset + pattern.len() + separator.len();

    loop {
        // Skip whitespace before owner
        let (after_ws, ws) = space0(current)?;
        current_offset += ws.len();

        // Check for comment or end of input
        if after_ws.is_empty() || after_ws.starts_with('#') {
            break;
        }

        // Parse owner
        let (after_owner, owner) = take_while1(is_owner_char)(after_ws)?;
        owner_offsets.push(current_offset);
        owners.push(owner);
        current_offset += owner.len();
        current = after_owner;
    }

    if owners.is_empty() {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Many1,
        )));
    }

    Ok((
        current,
        RuleComponents {
            pattern,
            pattern_offset,
            owners,
            owner_offsets,
        },
    ))
}

/// Classifies an owner string into its type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnerKind<'a> {
    /// A GitHub user (@username).
    User(&'a str),
    /// A GitHub team (@org/team).
    Team { org: &'a str, team: &'a str },
    /// An email address.
    Email(&'a str),
    /// Unknown/invalid format.
    Unknown(&'a str),
}

/// Classifies an owner text string into its type.
pub fn classify_owner(text: &str) -> OwnerKind<'_> {
    if let Some(stripped) = text.strip_prefix('@') {
        if let Some(slash_pos) = stripped.find('/') {
            let org = &stripped[..slash_pos];
            let team = &stripped[slash_pos + 1..];
            if !org.is_empty() && !team.is_empty() {
                return OwnerKind::Team { org, team };
            }
            // Empty org or team means invalid format
            return OwnerKind::Unknown(text);
        }
        if !stripped.is_empty() {
            return OwnerKind::User(stripped);
        }
        // Just "@" with nothing after
        return OwnerKind::Unknown(text);
    } else if text.contains('@') && !text.starts_with('@') {
        // Likely an email address
        return OwnerKind::Email(text);
    }

    OwnerKind::Unknown(text)
}

/// Creates an Owner AST node from text and span.
pub fn make_owner(text: &str, span: Span) -> Owner {
    match classify_owner(text) {
        OwnerKind::User(name) => Owner::user(name, span),
        OwnerKind::Team { org, team } => Owner::team(org, team, span),
        OwnerKind::Email(email) => Owner::email(email, span),
        OwnerKind::Unknown(raw) => {
            // Treat unknown as a user for now; validation will catch it
            Owner::user(raw, span)
        }
    }
}

/// Creates a Pattern AST node from text and span.
pub fn make_pattern(text: &str, span: Span) -> Pattern {
    Pattern::new(text, span)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_comment_line_with_leading_whitespace() {
        let (_rest, content) = parse_comment_line("   # comment").unwrap();
        assert_eq!(content, " comment");
    }

    #[test]
    fn parse_comment_line_no_whitespace() {
        let (_rest, content) = parse_comment_line("# This is a comment").unwrap();
        assert_eq!(content, " This is a comment");
    }

    #[test]
    fn is_blank_line_empty() {
        assert!(is_blank_line(""));
        assert!(is_blank_line("   "));
        assert!(is_blank_line("\t  \t"));
        assert!(!is_blank_line("*.rs @owner"));
        assert!(!is_blank_line("# comment"));
    }

    #[test]
    fn classify_owner_user() {
        assert_eq!(classify_owner("@octocat"), OwnerKind::User("octocat"));
        assert_eq!(classify_owner("@user-name"), OwnerKind::User("user-name"));
        assert_eq!(classify_owner("@user_123"), OwnerKind::User("user_123"));
    }

    #[test]
    fn classify_owner_team() {
        assert_eq!(
            classify_owner("@github/core"),
            OwnerKind::Team {
                org: "github",
                team: "core"
            }
        );
        assert_eq!(
            classify_owner("@my-org/team-name"),
            OwnerKind::Team {
                org: "my-org",
                team: "team-name"
            }
        );
    }

    #[test]
    fn classify_owner_email() {
        assert_eq!(
            classify_owner("dev@example.com"),
            OwnerKind::Email("dev@example.com")
        );
        assert_eq!(
            classify_owner("user.name@company.co.uk"),
            OwnerKind::Email("user.name@company.co.uk")
        );
    }

    #[test]
    fn classify_owner_unknown() {
        assert_eq!(classify_owner("noatsign"), OwnerKind::Unknown("noatsign"));
        assert_eq!(classify_owner("@"), OwnerKind::Unknown("@"));
        assert_eq!(classify_owner("@/team"), OwnerKind::Unknown("@/team"));
        assert_eq!(classify_owner("@org/"), OwnerKind::Unknown("@org/"));
    }

    #[test]
    fn parse_rule_components_single_owner() {
        let (_rest, components) = parse_rule_components("*.rs @owner").unwrap();
        assert_eq!(components.pattern, "*.rs");
        assert_eq!(components.owners, vec!["@owner"]);
        assert_eq!(components.pattern_offset, 0);
    }

    #[test]
    fn parse_rule_components_multiple_owners() {
        let (_rest, components) =
            parse_rule_components("/src/ @dev @github/core dev@example.com").unwrap();
        assert_eq!(components.pattern, "/src/");
        assert_eq!(
            components.owners,
            vec!["@dev", "@github/core", "dev@example.com"]
        );
    }

    #[test]
    fn parse_rule_components_with_leading_whitespace() {
        let (_rest, components) = parse_rule_components("  *.md @docs").unwrap();
        assert_eq!(components.pattern, "*.md");
        assert_eq!(components.pattern_offset, 2);
    }

    #[test]
    fn parse_rule_components_with_trailing_comment() {
        let (rest, components) =
            parse_rule_components("*.js @frontend # JavaScript files").unwrap();
        assert_eq!(components.pattern, "*.js");
        assert_eq!(components.owners, vec!["@frontend"]);
        // After parsing, rest should contain " # JavaScript files" (space before #)
        assert!(rest.contains('#'));
    }

    #[test]
    fn parse_rule_components_no_owner_fails() {
        let result = parse_rule_components("*.rs");
        assert!(result.is_err());
    }

    #[test]
    fn make_owner_user() {
        let span = Span::new(0, 1, 1, 8);
        let owner = make_owner("@octocat", span);
        assert!(matches!(owner, Owner::User { name, .. } if name == "octocat"));
    }

    #[test]
    fn make_owner_team() {
        let span = Span::new(0, 1, 1, 12);
        let owner = make_owner("@github/core", span);
        assert!(
            matches!(owner, Owner::Team { org, team, .. } if org == "github" && team == "core")
        );
    }

    #[test]
    fn make_owner_email() {
        let span = Span::new(0, 1, 1, 15);
        let owner = make_owner("dev@example.com", span);
        assert!(matches!(owner, Owner::Email { email, .. } if email == "dev@example.com"));
    }
}
