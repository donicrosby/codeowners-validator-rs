//! Random CODEOWNERS file generation for benchmarking and testing.
//!
//! Uses the AST types directly to guarantee valid output.
//!
//! Note: Generated ASTs have placeholder spans (all zeros). Do not use
//! for operations that depend on accurate span information.

use crate::parse::{CodeownersFile, Line, Owner, Pattern, Span};
use rand::prelude::*;
use rand::rngs::StdRng;

/// Configuration for generating CODEOWNERS files.
#[derive(Debug, Clone)]
pub struct GeneratorConfig {
    /// Number of rule lines to generate.
    pub num_rules: usize,
    /// Number of comment lines to generate.
    pub num_comments: usize,
    /// Maximum owners per rule (1-4 typical).
    pub max_owners_per_rule: usize,
    /// Seed for deterministic generation.
    pub seed: u64,
}

impl Default for GeneratorConfig {
    fn default() -> Self {
        Self {
            num_rules: 100,
            num_comments: 20,
            max_owners_per_rule: 4,
            seed: 42,
        }
    }
}

impl GeneratorConfig {
    /// Create a new config with specified rules and proportional comments.
    ///
    /// Comments are set to ~20% of rules (minimum 0).
    pub fn new(num_rules: usize) -> Self {
        Self {
            num_rules,
            num_comments: num_rules / 5,
            ..Default::default()
        }
    }

    /// Small fixture (~10 rules).
    pub fn small() -> Self {
        Self::new(10)
    }

    /// Medium fixture (~100 rules).
    pub fn medium() -> Self {
        Self::new(100)
    }

    /// Large fixture (~1000 rules).
    pub fn large() -> Self {
        Self::new(1_000)
    }

    /// Extra large fixture (~10k rules).
    pub fn xlarge() -> Self {
        Self::new(10_000)
    }

    /// Generate a file targeting approximately the given byte size.
    /// GitHub's limit is 3MB (~3_000_000 bytes).
    ///
    /// Note: Actual size varies based on pattern/owner complexity.
    pub fn target_bytes(bytes: usize) -> Self {
        // Average line is ~50 bytes
        Self::new(bytes.saturating_div(50).max(1))
    }

    /// Set the random seed for deterministic generation.
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Set the number of comments explicitly.
    pub fn with_comments(mut self, num_comments: usize) -> Self {
        self.num_comments = num_comments;
        self
    }

    /// Set the maximum owners per rule.
    pub fn with_max_owners(mut self, max: usize) -> Self {
        self.max_owners_per_rule = max.max(1); // At least 1 owner
        self
    }
}

/// Vocabulary for generating realistic patterns and owners.
mod vocabulary {
    pub const PATTERN_TEMPLATES: &[&str] = &[
        "*.{ext}",
        "**/*.{ext}",
        "/{dir}/",
        "/{dir}/**",
        "/{dir}/*.{ext}",
        "/src/{dir}/",
        "/src/**/*.{ext}",
        "/{dir}/**/test_*.{ext}",
        "docs/**/*.md",
        "!vendor/",
    ];

    pub const EXTENSIONS: &[&str] = &["rs", "py", "js", "ts", "go", "md", "yaml", "json", "toml"];
    pub const DIRECTORIES: &[&str] = &[
        "src", "lib", "tests", "docs", "config", "scripts", "api", "core",
    ];
    pub const USERNAMES: &[&str] = &["alice", "bob", "charlie", "dev", "maintainer", "reviewer"];
    pub const ORGS: &[&str] = &["acme", "github", "myorg"];
    pub const TEAMS: &[&str] = &["core", "platform", "frontend", "backend", "infra", "docs"];
    pub const SECTION_NAMES: &[&str] = &["Frontend", "Backend", "Infrastructure", "Documentation"];
}

/// Owner type distribution weights (must sum to 100).
const WEIGHT_USER: u32 = 50;
const WEIGHT_TEAM: u32 = 30;
// Remaining weight (20) goes to email

/// Probability of inserting a comment section header (percentage).
const COMMENT_PROBABILITY: u32 = 20;

/// Placeholder span for generated AST nodes.
///
/// Generated content doesn't have meaningful source positions.
fn placeholder_span() -> Span {
    Span::new(0, 0, 0, 0)
}

/// Generates a random CODEOWNERS AST based on configuration.
pub fn generate_ast(config: &GeneratorConfig) -> CodeownersFile {
    use vocabulary::*;

    let mut rng = StdRng::seed_from_u64(config.seed);
    let capacity = config.num_rules + config.num_comments + 10;
    let mut lines = Vec::with_capacity(capacity);

    // Header comment
    lines.push(Line::comment(
        " Auto-generated CODEOWNERS for benchmarking",
        placeholder_span(),
    ));
    lines.push(Line::blank(placeholder_span()));

    let mut rules_added = 0;
    let mut comments_added = 0;

    // Generate rules, interspersing comments
    while rules_added < config.num_rules {
        // Maybe add a section comment
        if comments_added < config.num_comments
            && rules_added > 0
            && rng.random_ratio(COMMENT_PROBABILITY, 100)
        {
            let section = SECTION_NAMES[rng.random_range(0..SECTION_NAMES.len())];
            lines.push(Line::blank(placeholder_span()));
            lines.push(Line::comment(
                format!(" {} section", section),
                placeholder_span(),
            ));
            comments_added += 1;
        }

        // Generate pattern
        let template = PATTERN_TEMPLATES[rng.random_range(0..PATTERN_TEMPLATES.len())];
        let ext = EXTENSIONS[rng.random_range(0..EXTENSIONS.len())];
        let dir = DIRECTORIES[rng.random_range(0..DIRECTORIES.len())];
        let pattern_text = template.replace("{ext}", ext).replace("{dir}", dir);
        let pattern = Pattern::new(pattern_text, placeholder_span());

        // Generate owners (1 to max_owners_per_rule)
        let num_owners = rng.random_range(1..=config.max_owners_per_rule);
        let owners: Vec<Owner> = (0..num_owners).map(|_| generate_owner(&mut rng)).collect();

        lines.push(Line::rule(pattern, owners, placeholder_span()));
        rules_added += 1;
    }

    CodeownersFile::new(lines)
}

/// Generate a random owner based on weighted distribution.
fn generate_owner(rng: &mut StdRng) -> Owner {
    use vocabulary::*;

    let roll = rng.random_range(0..100);

    if roll < WEIGHT_USER {
        Owner::user(
            USERNAMES[rng.random_range(0..USERNAMES.len())],
            placeholder_span(),
        )
    } else if roll < WEIGHT_USER + WEIGHT_TEAM {
        Owner::team(
            ORGS[rng.random_range(0..ORGS.len())],
            TEAMS[rng.random_range(0..TEAMS.len())],
            placeholder_span(),
        )
    } else {
        Owner::email(
            format!(
                "{}@example.com",
                USERNAMES[rng.random_range(0..USERNAMES.len())]
            ),
            placeholder_span(),
        )
    }
}

/// Generates a CODEOWNERS file as a string.
pub fn generate(config: &GeneratorConfig) -> String {
    generate_ast(config).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_codeowners;

    #[test]
    fn round_trip_small() {
        let content = generate(&GeneratorConfig::small());
        let parsed = parse_codeowners(&content);
        assert!(
            parsed.is_ok(),
            "Generated content should parse: {:?}",
            parsed.errors
        );
    }

    #[test]
    fn round_trip_large() {
        let content = generate(&GeneratorConfig::large());
        let parsed = parse_codeowners(&content);
        assert!(parsed.is_ok());
        assert!(parsed.ast.rules().count() >= 900);
    }

    #[test]
    fn deterministic_generation() {
        let config = GeneratorConfig::medium();
        let content1 = generate(&config);
        let content2 = generate(&config);
        assert_eq!(content1, content2, "Same seed should produce same output");
    }

    #[test]
    fn different_seeds_differ() {
        let content1 = generate(&GeneratorConfig::medium().with_seed(1));
        let content2 = generate(&GeneratorConfig::medium().with_seed(2));
        assert_ne!(content1, content2);
    }

    #[test]
    fn target_bytes_approximate() {
        let config = GeneratorConfig::target_bytes(100_000);
        let content = generate(&config);
        // Should be within 2x of target
        assert!(
            content.len() > 50_000 && content.len() < 200_000,
            "Got {} bytes",
            content.len()
        );
    }

    #[test]
    fn zero_rules_produces_header_only() {
        let config = GeneratorConfig::new(0);
        let content = generate(&config);
        let parsed = parse_codeowners(&content);
        assert!(parsed.is_ok());
        // Should have header comment + blank line only
        assert_eq!(parsed.ast.rules().count(), 0);
    }

    #[test]
    fn single_rule_works() {
        let config = GeneratorConfig::new(1);
        let content = generate(&config);
        let parsed = parse_codeowners(&content);
        assert!(parsed.is_ok());
        assert_eq!(parsed.ast.rules().count(), 1);
    }

    #[test]
    fn with_comments_override() {
        let config = GeneratorConfig::new(100).with_comments(50);
        assert_eq!(config.num_comments, 50);
    }

    #[test]
    fn with_max_owners_minimum() {
        let config = GeneratorConfig::default().with_max_owners(0);
        assert_eq!(config.max_owners_per_rule, 1); // Should be at least 1
    }
}
