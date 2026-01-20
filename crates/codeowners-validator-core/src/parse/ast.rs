//! AST data structures for CODEOWNERS files.
//!
//! This module defines the abstract syntax tree nodes that represent
//! parsed CODEOWNERS file content.

use super::span::Span;
use std::borrow::Cow;
use std::fmt::{self, Display};

/// Represents a pattern in a CODEOWNERS rule.
///
/// Patterns follow a subset of gitignore syntax for matching file paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    /// The raw pattern text (e.g., "*.rs", "/src/**", "docs/").
    pub text: String,
    /// Location of the pattern in the source file.
    pub span: Span,
}

impl Pattern {
    /// Creates a new pattern with the given text and span.
    pub fn new(text: impl Into<String>, span: Span) -> Self {
        Self {
            text: text.into(),
            span,
        }
    }
}

impl Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

/// Represents an owner in a CODEOWNERS rule.
///
/// Owners can be GitHub users, teams, or email addresses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Owner {
    /// A GitHub user (e.g., "@username").
    User {
        /// The username without the leading '@'.
        name: String,
        /// Location in the source file.
        span: Span,
    },
    /// A GitHub team (e.g., "@org/team-name").
    Team {
        /// The organization name.
        org: String,
        /// The team name within the organization.
        team: String,
        /// Location in the source file.
        span: Span,
    },
    /// An email address owner.
    Email {
        /// The email address.
        email: String,
        /// Location in the source file.
        span: Span,
    },
}

impl Owner {
    /// Creates a new user owner.
    pub fn user(name: impl Into<String>, span: Span) -> Self {
        Self::User {
            name: name.into(),
            span,
        }
    }

    /// Creates a new team owner.
    pub fn team(org: impl Into<String>, team: impl Into<String>, span: Span) -> Self {
        Self::Team {
            org: org.into(),
            team: team.into(),
            span,
        }
    }

    /// Creates a new email owner.
    pub fn email(email: impl Into<String>, span: Span) -> Self {
        Self::Email {
            email: email.into(),
            span,
        }
    }

    /// Returns the span of this owner.
    pub fn span(&self) -> &Span {
        match self {
            Owner::User { span, .. } => span,
            Owner::Team { span, .. } => span,
            Owner::Email { span, .. } => span,
        }
    }

    /// Returns the raw text representation of this owner.
    ///
    /// Returns a `Cow<str>` to avoid allocations when possible:
    /// - For emails, returns a borrowed reference
    /// - For users and teams, returns an owned formatted string
    pub fn as_str(&self) -> Cow<'_, str> {
        match self {
            Owner::User { name, .. } => Cow::Owned(format!("@{}", name)),
            Owner::Team { org, team, .. } => Cow::Owned(format!("@{}/{}", org, team)),
            Owner::Email { email, .. } => Cow::Borrowed(email),
        }
    }
}

impl Display for Owner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Owner::User { name, .. } => write!(f, "@{}", name),
            Owner::Team { org, team, .. } => write!(f, "@{}/{}", org, team),
            Owner::Email { email, .. } => f.write_str(email),
        }
    }
}

/// Represents the kind of line in a CODEOWNERS file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineKind {
    /// A blank line (may contain only whitespace).
    Blank,
    /// A comment line (starts with '#').
    Comment {
        /// The comment content (without the leading '#').
        content: String,
    },
    /// A rule line with a pattern and one or more owners.
    Rule {
        /// The file path pattern.
        pattern: Pattern,
        /// The list of owners for files matching the pattern.
        owners: Vec<Owner>,
    },
    /// An invalid line that couldn't be parsed.
    Invalid {
        /// The raw line content.
        raw: String,
        /// Description of what went wrong.
        error: String,
    },
}

/// Represents a single line in a CODEOWNERS file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// The kind/content of this line.
    pub kind: LineKind,
    /// Location of the entire line in the source file.
    pub span: Span,
}

impl Line {
    /// Creates a new line with the given kind and span.
    pub fn new(kind: LineKind, span: Span) -> Self {
        Self { kind, span }
    }

    /// Creates a blank line.
    pub fn blank(span: Span) -> Self {
        Self::new(LineKind::Blank, span)
    }

    /// Creates a comment line.
    pub fn comment(content: impl Into<String>, span: Span) -> Self {
        Self::new(
            LineKind::Comment {
                content: content.into(),
            },
            span,
        )
    }

    /// Creates a rule line.
    pub fn rule(pattern: Pattern, owners: Vec<Owner>, span: Span) -> Self {
        Self::new(LineKind::Rule { pattern, owners }, span)
    }

    /// Creates an invalid line.
    pub fn invalid(raw: impl Into<String>, error: impl Into<String>, span: Span) -> Self {
        Self::new(
            LineKind::Invalid {
                raw: raw.into(),
                error: error.into(),
            },
            span,
        )
    }

    /// Returns true if this is a rule line.
    pub fn is_rule(&self) -> bool {
        matches!(self.kind, LineKind::Rule { .. })
    }

    /// Returns true if this is a comment line.
    pub fn is_comment(&self) -> bool {
        matches!(self.kind, LineKind::Comment { .. })
    }

    /// Returns true if this is a blank line.
    pub fn is_blank(&self) -> bool {
        matches!(self.kind, LineKind::Blank)
    }

    /// Returns true if this is an invalid line.
    pub fn is_invalid(&self) -> bool {
        matches!(self.kind, LineKind::Invalid { .. })
    }
}

impl Display for Line {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            LineKind::Blank => Ok(()),
            LineKind::Comment { content } => write!(f, "#{}", content),
            LineKind::Rule { pattern, owners } => {
                write!(f, "{}", pattern)?;
                for owner in owners {
                    write!(f, " {}", owner)?;
                }
                Ok(())
            }
            LineKind::Invalid { raw, .. } => f.write_str(raw),
        }
    }
}

/// The complete AST for a CODEOWNERS file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeownersFile {
    /// All lines in the file, in order.
    pub lines: Vec<Line>,
}

impl CodeownersFile {
    /// Creates a new CODEOWNERS file AST from the given lines.
    pub fn new(lines: Vec<Line>) -> Self {
        Self { lines }
    }

    /// Returns an iterator over all rule lines.
    pub fn rules(&self) -> impl Iterator<Item = &Line> {
        self.lines.iter().filter(|line| line.is_rule())
    }

    /// Returns an iterator over all invalid lines.
    pub fn invalid_lines(&self) -> impl Iterator<Item = &Line> {
        self.lines.iter().filter(|line| line.is_invalid())
    }

    /// Returns true if there are any invalid lines.
    pub fn has_errors(&self) -> bool {
        self.lines.iter().any(|line| line.is_invalid())
    }

    /// Extracts all rules as (pattern, owners) pairs.
    pub fn extract_rules(&self) -> Vec<(&Pattern, &[Owner])> {
        self.lines
            .iter()
            .filter_map(|line| match &line.kind {
                LineKind::Rule { pattern, owners } => Some((pattern, owners.as_slice())),
                _ => None,
            })
            .collect()
    }
}

impl Default for CodeownersFile {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl Display for CodeownersFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for line in &self.lines {
            if !first {
                writeln!(f)?;
            }
            first = false;
            write!(f, "{}", line)?;
        }
        // Trailing newline for POSIX compatibility
        if !self.lines.is_empty() {
            writeln!(f)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_span() -> Span {
        Span::new(0, 1, 1, 10)
    }

    #[test]
    fn pattern_creation() {
        let pattern = Pattern::new("*.rs", test_span());
        assert_eq!(pattern.text, "*.rs");
    }

    #[test]
    fn owner_user_creation() {
        let owner = Owner::user("octocat", test_span());
        assert!(matches!(&owner, Owner::User { name, .. } if name == "octocat"));
        assert_eq!(owner.as_str(), "@octocat");
    }

    #[test]
    fn owner_team_creation() {
        let owner = Owner::team("github", "core", test_span());
        assert!(
            matches!(&owner, Owner::Team { org, team, .. } if org == "github" && team == "core")
        );
        assert_eq!(owner.as_str(), "@github/core");
    }

    #[test]
    fn owner_email_creation() {
        let owner = Owner::email("dev@example.com", test_span());
        assert!(matches!(&owner, Owner::Email { email, .. } if email == "dev@example.com"));
        assert_eq!(owner.as_str(), "dev@example.com");
    }

    #[test]
    fn line_blank() {
        let line = Line::blank(test_span());
        assert!(line.is_blank());
        assert!(!line.is_rule());
        assert!(!line.is_comment());
        assert!(!line.is_invalid());
    }

    #[test]
    fn line_comment() {
        let line = Line::comment(" This is a comment", test_span());
        assert!(line.is_comment());
        assert!(
            matches!(line.kind, LineKind::Comment { content } if content == " This is a comment")
        );
    }

    #[test]
    fn line_rule() {
        let pattern = Pattern::new("*.rs", test_span());
        let owners = vec![Owner::user("rustacean", test_span())];
        let line = Line::rule(pattern, owners, test_span());

        assert!(line.is_rule());
        if let LineKind::Rule { pattern, owners } = &line.kind {
            assert_eq!(pattern.text, "*.rs");
            assert_eq!(owners.len(), 1);
        } else {
            panic!("Expected Rule");
        }
    }

    #[test]
    fn line_invalid() {
        let line = Line::invalid("bad line", "missing owner", test_span());
        assert!(line.is_invalid());
        if let LineKind::Invalid { raw, error } = &line.kind {
            assert_eq!(raw, "bad line");
            assert_eq!(error, "missing owner");
        } else {
            panic!("Expected Invalid");
        }
    }

    #[test]
    fn codeowners_file_rules_iterator() {
        let lines = vec![
            Line::comment("comment", test_span()),
            Line::rule(
                Pattern::new("*.rs", test_span()),
                vec![Owner::user("owner", test_span())],
                test_span(),
            ),
            Line::blank(test_span()),
            Line::rule(
                Pattern::new("*.md", test_span()),
                vec![Owner::user("docs", test_span())],
                test_span(),
            ),
        ];

        let file = CodeownersFile::new(lines);
        let rules: Vec<_> = file.rules().collect();

        assert_eq!(rules.len(), 2);
        assert!(rules[0].is_rule());
        assert!(rules[1].is_rule());
    }

    #[test]
    fn codeowners_file_has_errors() {
        let lines_ok = vec![Line::rule(
            Pattern::new("*", test_span()),
            vec![Owner::user("owner", test_span())],
            test_span(),
        )];

        let lines_with_error = vec![Line::invalid("bad", "error", test_span())];

        assert!(!CodeownersFile::new(lines_ok).has_errors());
        assert!(CodeownersFile::new(lines_with_error).has_errors());
    }

    #[test]
    fn codeowners_file_extract_rules() {
        let lines = vec![
            Line::comment("header", test_span()),
            Line::rule(
                Pattern::new("/src/", test_span()),
                vec![
                    Owner::user("dev", test_span()),
                    Owner::team("org", "team", test_span()),
                ],
                test_span(),
            ),
        ];

        let file = CodeownersFile::new(lines);
        let rules = file.extract_rules();

        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].0.text, "/src/");
        assert_eq!(rules[0].1.len(), 2);
    }

    // Display trait tests
    #[test]
    fn pattern_display() {
        let pattern = Pattern::new("*.rs", test_span());
        assert_eq!(pattern.to_string(), "*.rs");
    }

    #[test]
    fn owner_display_user() {
        let owner = Owner::user("octocat", test_span());
        assert_eq!(owner.to_string(), "@octocat");
    }

    #[test]
    fn owner_display_team() {
        let owner = Owner::team("github", "core", test_span());
        assert_eq!(owner.to_string(), "@github/core");
    }

    #[test]
    fn owner_display_email() {
        let owner = Owner::email("dev@example.com", test_span());
        assert_eq!(owner.to_string(), "dev@example.com");
    }

    #[test]
    fn line_display_blank() {
        let line = Line::blank(test_span());
        assert_eq!(line.to_string(), "");
    }

    #[test]
    fn line_display_comment() {
        let line = Line::comment(" This is a comment", test_span());
        assert_eq!(line.to_string(), "# This is a comment");
    }

    #[test]
    fn line_display_rule() {
        let pattern = Pattern::new("*.rs", test_span());
        let owners = vec![
            Owner::user("alice", test_span()),
            Owner::team("org", "team", test_span()),
        ];
        let line = Line::rule(pattern, owners, test_span());
        assert_eq!(line.to_string(), "*.rs @alice @org/team");
    }

    #[test]
    fn line_display_invalid() {
        let line = Line::invalid("bad line", "error", test_span());
        assert_eq!(line.to_string(), "bad line");
    }

    #[test]
    fn codeowners_file_display() {
        let lines = vec![
            Line::comment(" CODEOWNERS", test_span()),
            Line::blank(test_span()),
            Line::rule(
                Pattern::new("*.rs", test_span()),
                vec![Owner::user("rustacean", test_span())],
                test_span(),
            ),
        ];
        let file = CodeownersFile::new(lines);
        assert_eq!(file.to_string(), "# CODEOWNERS\n\n*.rs @rustacean\n");
    }

    #[test]
    fn codeowners_file_display_empty() {
        let file = CodeownersFile::new(vec![]);
        assert_eq!(file.to_string(), "");
    }
}
