//! Syntax validation for CODEOWNERS files.
//!
//! This module implements validation rules for owner formats
//! and pattern syntax.

use super::error::{ValidationError, ValidationResult};
use crate::parse::span::Span;
use crate::parse::{CodeownersFile, LineKind, Owner, Pattern};

/// Validates owner syntax according to GitHub CODEOWNERS rules.
///
/// Valid owners are:
/// - `@username` - alphanumeric, hyphens, underscores (max 39 chars)
/// - `@org/team` - organization and team names follow same rules
/// - `email@domain.com` - basic email format validation
pub fn validate_owner_syntax(owner: &Owner) -> Option<ValidationError> {
    match owner {
        Owner::User { name, span } => validate_username(name, span),
        Owner::Team { org, team, span } => validate_team(org, team, span),
        Owner::Email { email, span } => validate_email(email, span),
    }
}

/// Validates a GitHub username.
fn validate_username(name: &str, span: &Span) -> Option<ValidationError> {
    if name.is_empty() {
        return Some(ValidationError::invalid_owner_format(
            format!("@{}", name),
            "username cannot be empty",
            *span,
        ));
    }

    if name.len() > 39 {
        return Some(ValidationError::invalid_owner_format(
            format!("@{}", name),
            "username cannot exceed 39 characters",
            *span,
        ));
    }

    // GitHub usernames: alphanumeric and hyphens, cannot start/end with hyphen
    if name.starts_with('-') || name.ends_with('-') {
        return Some(ValidationError::invalid_owner_format(
            format!("@{}", name),
            "username cannot start or end with a hyphen",
            *span,
        ));
    }

    if !name.chars().all(|c| c.is_alphanumeric() || c == '-') {
        return Some(ValidationError::invalid_owner_format(
            format!("@{}", name),
            "username can only contain alphanumeric characters and hyphens",
            *span,
        ));
    }

    // Check for consecutive hyphens
    if name.contains("--") {
        return Some(ValidationError::invalid_owner_format(
            format!("@{}", name),
            "username cannot contain consecutive hyphens",
            *span,
        ));
    }

    None
}

/// Validates a GitHub team reference.
fn validate_team(org: &str, team: &str, span: &Span) -> Option<ValidationError> {
    let full = format!("@{}/{}", org, team);

    if org.is_empty() {
        return Some(ValidationError::invalid_owner_format(
            &full,
            "organization name cannot be empty",
            *span,
        ));
    }

    if team.is_empty() {
        return Some(ValidationError::invalid_owner_format(
            &full,
            "team name cannot be empty",
            *span,
        ));
    }

    // Validate org name
    if let Some(err) = validate_github_name(org, "organization") {
        return Some(ValidationError::invalid_owner_format(&full, err, *span));
    }

    // Validate team name (similar rules but can include underscores)
    if let Some(err) = validate_github_name(team, "team") {
        return Some(ValidationError::invalid_owner_format(&full, err, *span));
    }

    None
}

/// Validates a GitHub organization or team name component.
fn validate_github_name(name: &str, kind: &str) -> Option<String> {
    if name.len() > 39 {
        return Some(format!("{} name cannot exceed 39 characters", kind));
    }

    if name.starts_with('-') || name.ends_with('-') {
        return Some(format!("{} name cannot start or end with a hyphen", kind));
    }

    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Some(format!(
            "{} name can only contain alphanumeric characters, hyphens, and underscores",
            kind
        ));
    }

    None
}

/// Validates an email address with basic checks.
fn validate_email(email: &str, span: &Span) -> Option<ValidationError> {
    // Basic email validation - must have exactly one @ with content on both sides
    let parts: Vec<&str> = email.split('@').collect();

    if parts.len() != 2 {
        return Some(ValidationError::invalid_owner_format(
            email,
            "email must contain exactly one @ symbol",
            *span,
        ));
    }

    let local = parts[0];
    let domain = parts[1];

    if local.is_empty() {
        return Some(ValidationError::invalid_owner_format(
            email,
            "email local part cannot be empty",
            *span,
        ));
    }

    if domain.is_empty() {
        return Some(ValidationError::invalid_owner_format(
            email,
            "email domain cannot be empty",
            *span,
        ));
    }

    // Domain must contain at least one dot
    if !domain.contains('.') {
        return Some(ValidationError::invalid_owner_format(
            email,
            "email domain must contain a dot",
            *span,
        ));
    }

    // Domain parts must not be empty
    for part in domain.split('.') {
        if part.is_empty() {
            return Some(ValidationError::invalid_owner_format(
                email,
                "email domain parts cannot be empty",
                *span,
            ));
        }
    }

    None
}

/// Validates pattern syntax according to GitHub CODEOWNERS rules.
///
/// CODEOWNERS patterns follow a subset of gitignore syntax:
/// - `*` matches any file name
/// - `**` matches any directory path
/// - `/` at start anchors to repo root
/// - `/` at end matches only directories
///
/// Not supported (will produce warnings):
/// - `!` negation patterns
/// - `[abc]` character classes
/// - `\` escape sequences
pub fn validate_pattern_syntax(pattern: &Pattern) -> Option<ValidationError> {
    let text = &pattern.text;

    // Check for negation (not supported)
    if text.starts_with('!') {
        return Some(ValidationError::unsupported_pattern_syntax(
            text,
            "negation patterns (!) are not supported in CODEOWNERS",
            pattern.span,
        ));
    }

    // Check for character classes (not supported)
    if text.contains('[') || text.contains(']') {
        return Some(ValidationError::unsupported_pattern_syntax(
            text,
            "character classes ([abc]) are not supported in CODEOWNERS",
            pattern.span,
        ));
    }

    // Check for escape sequences (not supported in the same way as gitignore)
    if text.contains('\\') {
        return Some(ValidationError::unsupported_pattern_syntax(
            text,
            "escape sequences (\\) are not supported in CODEOWNERS",
            pattern.span,
        ));
    }

    // Pattern cannot be empty
    if text.is_empty() {
        return Some(ValidationError::invalid_pattern_syntax(
            text,
            "pattern cannot be empty",
            pattern.span,
        ));
    }

    // Pattern cannot be just whitespace
    if text.trim().is_empty() {
        return Some(ValidationError::invalid_pattern_syntax(
            text,
            "pattern cannot be only whitespace",
            pattern.span,
        ));
    }

    None
}

/// Validates all owners in a CODEOWNERS file.
pub fn validate_all_owners(file: &CodeownersFile) -> ValidationResult {
    let mut result = ValidationResult::new();

    for line in &file.lines {
        if let LineKind::Rule { owners, .. } = &line.kind {
            for owner in owners {
                if let Some(error) = validate_owner_syntax(owner) {
                    result.add_error(error);
                }
            }
        }
    }

    result
}

/// Validates all patterns in a CODEOWNERS file.
pub fn validate_all_patterns(file: &CodeownersFile) -> ValidationResult {
    let mut result = ValidationResult::new();

    for line in &file.lines {
        if let LineKind::Rule { pattern, .. } = &line.kind
            && let Some(error) = validate_pattern_syntax(pattern)
        {
            result.add_error(error);
        }
    }

    result
}

/// Performs all syntax validations on a CODEOWNERS file.
pub fn validate_syntax(file: &CodeownersFile) -> ValidationResult {
    let mut result = validate_all_owners(file);
    result.merge(validate_all_patterns(file));
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::Span;

    fn test_span() -> Span {
        Span::new(0, 1, 1, 10)
    }

    // Username validation tests

    #[test]
    fn valid_username() {
        let owner = Owner::user("octocat", test_span());
        assert!(validate_owner_syntax(&owner).is_none());
    }

    #[test]
    fn valid_username_with_hyphen() {
        let owner = Owner::user("octo-cat", test_span());
        assert!(validate_owner_syntax(&owner).is_none());
    }

    #[test]
    fn valid_username_with_numbers() {
        let owner = Owner::user("user123", test_span());
        assert!(validate_owner_syntax(&owner).is_none());
    }

    #[test]
    fn invalid_username_empty() {
        let owner = Owner::user("", test_span());
        let err = validate_owner_syntax(&owner);
        assert!(err.is_some());
        assert!(err.unwrap().to_string().contains("empty"));
    }

    #[test]
    fn invalid_username_starts_with_hyphen() {
        let owner = Owner::user("-user", test_span());
        let err = validate_owner_syntax(&owner);
        assert!(err.is_some());
    }

    #[test]
    fn invalid_username_ends_with_hyphen() {
        let owner = Owner::user("user-", test_span());
        let err = validate_owner_syntax(&owner);
        assert!(err.is_some());
    }

    #[test]
    fn invalid_username_consecutive_hyphens() {
        let owner = Owner::user("user--name", test_span());
        let err = validate_owner_syntax(&owner);
        assert!(err.is_some());
    }

    #[test]
    fn invalid_username_special_chars() {
        let owner = Owner::user("user@name", test_span());
        let err = validate_owner_syntax(&owner);
        assert!(err.is_some());
    }

    #[test]
    fn invalid_username_too_long() {
        let long_name = "a".repeat(40);
        let owner = Owner::user(&long_name, test_span());
        let err = validate_owner_syntax(&owner);
        assert!(err.is_some());
        assert!(err.unwrap().to_string().contains("39 characters"));
    }

    // Team validation tests

    #[test]
    fn valid_team() {
        let owner = Owner::team("github", "core", test_span());
        assert!(validate_owner_syntax(&owner).is_none());
    }

    #[test]
    fn valid_team_with_hyphens() {
        let owner = Owner::team("my-org", "my-team", test_span());
        assert!(validate_owner_syntax(&owner).is_none());
    }

    #[test]
    fn valid_team_with_underscores() {
        let owner = Owner::team("my_org", "my_team", test_span());
        assert!(validate_owner_syntax(&owner).is_none());
    }

    #[test]
    fn invalid_team_empty_org() {
        let owner = Owner::team("", "team", test_span());
        let err = validate_owner_syntax(&owner);
        assert!(err.is_some());
    }

    #[test]
    fn invalid_team_empty_team() {
        let owner = Owner::team("org", "", test_span());
        let err = validate_owner_syntax(&owner);
        assert!(err.is_some());
    }

    // Email validation tests

    #[test]
    fn valid_email_simple() {
        let owner = Owner::email("user@example.com", test_span());
        assert!(validate_owner_syntax(&owner).is_none());
    }

    #[test]
    fn valid_email_subdomain() {
        let owner = Owner::email("user@mail.example.com", test_span());
        assert!(validate_owner_syntax(&owner).is_none());
    }

    #[test]
    fn invalid_email_no_at() {
        let owner = Owner::email("userexample.com", test_span());
        let err = validate_owner_syntax(&owner);
        assert!(err.is_some());
    }

    #[test]
    fn invalid_email_multiple_at() {
        let owner = Owner::email("user@@example.com", test_span());
        let err = validate_owner_syntax(&owner);
        assert!(err.is_some());
    }

    #[test]
    fn invalid_email_no_domain_dot() {
        let owner = Owner::email("user@example", test_span());
        let err = validate_owner_syntax(&owner);
        assert!(err.is_some());
    }

    #[test]
    fn invalid_email_empty_local() {
        let owner = Owner::email("@example.com", test_span());
        let err = validate_owner_syntax(&owner);
        assert!(err.is_some());
    }

    // Pattern validation tests

    #[test]
    fn valid_pattern_simple() {
        let pattern = Pattern::new("*.rs", test_span());
        assert!(validate_pattern_syntax(&pattern).is_none());
    }

    #[test]
    fn valid_pattern_glob() {
        let pattern = Pattern::new("**/*.js", test_span());
        assert!(validate_pattern_syntax(&pattern).is_none());
    }

    #[test]
    fn valid_pattern_rooted() {
        let pattern = Pattern::new("/src/", test_span());
        assert!(validate_pattern_syntax(&pattern).is_none());
    }

    #[test]
    fn valid_pattern_directory() {
        let pattern = Pattern::new("docs/", test_span());
        assert!(validate_pattern_syntax(&pattern).is_none());
    }

    #[test]
    fn invalid_pattern_negation() {
        let pattern = Pattern::new("!*.log", test_span());
        let err = validate_pattern_syntax(&pattern);
        assert!(err.is_some());
        assert!(err.unwrap().to_string().contains("negation"));
    }

    #[test]
    fn invalid_pattern_character_class() {
        let pattern = Pattern::new("*.[ch]", test_span());
        let err = validate_pattern_syntax(&pattern);
        assert!(err.is_some());
        assert!(err.unwrap().to_string().contains("character class"));
    }

    #[test]
    fn invalid_pattern_escape() {
        let pattern = Pattern::new("\\#file", test_span());
        let err = validate_pattern_syntax(&pattern);
        assert!(err.is_some());
        assert!(err.unwrap().to_string().contains("escape"));
    }

    // Full file validation tests

    #[test]
    fn validate_file_all_valid() {
        use crate::parse::parse_codeowners;

        let input = r#"
# Comment
*.rs @rust-dev
/docs/ @github/docs-team user@example.com
"#;
        let result = parse_codeowners(input);
        let validation = validate_syntax(&result.ast);

        assert!(validation.is_ok());
    }

    #[test]
    fn validate_file_with_invalid_owner() {
        use crate::parse::parse_codeowners;

        let input = "*.rs @-invalid-user\n";
        let result = parse_codeowners(input);
        let validation = validate_syntax(&result.ast);

        assert!(validation.has_errors());
        assert_eq!(validation.errors.len(), 1);
    }

    #[test]
    fn validate_file_with_invalid_pattern() {
        use crate::parse::parse_codeowners;

        let input = "!*.log @dev\n";
        let result = parse_codeowners(input);
        let validation = validate_syntax(&result.ast);

        assert!(validation.has_errors());
    }

    #[test]
    fn validate_file_multiple_errors() {
        use crate::parse::parse_codeowners;

        let input = r#"
!*.log @dev
*.rs @-bad-name
[abc] @team
"#;
        let result = parse_codeowners(input);
        let validation = validate_syntax(&result.ast);

        // Should have multiple errors
        assert!(validation.errors.len() >= 2);
    }
}
