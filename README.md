# codeowners-validator-rs

A fast, feature-rich validator for GitHub CODEOWNERS files, written in Rust with Python bindings.

## Features

- **Comprehensive Parsing**: Full parser for CODEOWNERS files with detailed error messages and location information
- **Multiple Validation Checks**: Syntax, file existence, duplicate patterns, owner verification, and more
- **Python Bindings**: Native Python module with full type hints and async support
- **CLI Tool**: Command-line interface with JSON output support
- **GitHub Integration**: Verify that owners (users/teams) actually exist on GitHub
- **Flexible Authentication**: Support for Personal Access Tokens and GitHub App authentication

## Validation Checks

| Check | Description | Requires GitHub API |
|-------|-------------|---------------------|
| `syntax` | Validates CODEOWNERS syntax | No |
| `files` | Checks that patterns match existing files in the repository | No |
| `duppatterns` | Detects duplicate patterns | No |
| `owners` | Verifies owners exist on GitHub | Yes |
| `notowned` | Finds files not covered by any rule *(experimental)* | No |
| `avoid-shadowing` | Detects patterns that shadow earlier patterns *(experimental)* | No |

---

## Installation

### Rust Crate

Add to your `Cargo.toml`:

```toml
[dependencies]
codeowners-validator-core = { git = "https://github.com/donicrosby/codeowners-validator-rs" }
```

### CLI Tool

```bash
cargo install --git https://github.com/donicrosby/codeowners-validator-rs codeowners-cli
```

### Python Bindings

The Python package requires [maturin](https://github.com/PyO3/maturin) to build from source.

#### Using uv (Recommended)

```bash
# Install directly from git
uv add "codeowners-validator @ git+https://github.com/donicrosby/codeowners-validator-rs#subdirectory=crates/codeowners-python-bindings"

# With optional GitHub client dependencies
uv add "codeowners-validator[githubkit] @ git+https://github.com/donicrosby/codeowners-validator-rs#subdirectory=crates/codeowners-python-bindings"

# Or with PyGithub
uv add "codeowners-validator[pygithub] @ git+https://github.com/donicrosby/codeowners-validator-rs#subdirectory=crates/codeowners-python-bindings"
```

#### Using pip

```bash
# Install directly from git
pip install "codeowners-validator @ git+https://github.com/donicrosby/codeowners-validator-rs#subdirectory=crates/codeowners-python-bindings"

# With optional GitHub client dependencies
pip install "codeowners-validator[githubkit] @ git+https://github.com/donicrosby/codeowners-validator-rs#subdirectory=crates/codeowners-python-bindings"
```

#### From Local Clone

```bash
git clone https://github.com/donicrosby/codeowners-validator-rs.git
cd codeowners-validator-rs/crates/codeowners-python-bindings

# Using uv
uv add -e .

# Or using pip with maturin
pip install maturin
maturin develop
```

---

## Usage

### Python

#### Basic Parsing

```python
from codeowners_validator import parse_codeowners

content = """
# CODEOWNERS file
*.rs @rustacean
/docs/ @github/docs-team
"""

result = parse_codeowners(content)

if result["is_ok"]:
    print(f"Parsed {len(result['ast']['lines'])} lines")
    for line in result["ast"]["lines"]:
        if line["kind"]["type"] == "rule":
            pattern = line["kind"]["pattern"]["text"]
            owners = [o["text"] for o in line["kind"]["owners"]]
            print(f"  {pattern} -> {owners}")
else:
    for error in result["errors"]:
        print(f"Parse error: {error}")
```

#### Validation (Without GitHub)

```python
from codeowners_validator import validate_codeowners

content = "*.rs @rustacean\n/docs/ @docs-team\n"
repo_path = "/path/to/your/repo"

# Run default checks (syntax, files, duppatterns)
result = validate_codeowners(content, repo_path)

# Check results
for check_name, issues in result.items():
    if issues:
        print(f"{check_name}:")
        for issue in issues:
            print(f"  Line {issue['line']}: {issue['message']} ({issue['severity']})")

# Run specific checks only
result = validate_codeowners(
    content, 
    repo_path, 
    checks=["syntax", "duppatterns"]
)

# With configuration
result = validate_codeowners(
    content,
    repo_path,
    config={
        "ignored_owners": ["@bot", "@ghost"],
        "allow_unowned_patterns": False,
    },
    checks=["syntax", "files", "duppatterns", "notowned"]
)
```

#### Validation with GitHub Owner Verification

To verify that owners exist on GitHub, you need to provide a GitHub client. The client must implement the `GithubClientProtocol`:

```python
import asyncio
from codeowners_validator import validate_with_github

# Example with githubkit
from githubkit import GitHub

class GithubKitClient:
    def __init__(self, token: str):
        self.client = GitHub(token)

    async def user_exists(self, username: str) -> bool:
        try:
            await self.client.rest.users.async_get_by_username(username)
            return True
        except Exception:
            return False

    async def team_exists(self, org: str, team: str) -> str:
        try:
            await self.client.rest.teams.async_get_by_name(org, team)
            return "exists"
        except Exception as e:
            if "404" in str(e):
                return "not_found"
            return "unauthorized"

async def main():
    client = GithubKitClient("your-github-token")
    
    content = "*.rs @rustacean\n/docs/ @myorg/docs-team\n"
    repo_path = "/path/to/your/repo"
    
    result = await validate_with_github(
        content,
        repo_path,
        client,
        config={"repository": "myorg/myrepo"}
    )
    
    if result["owners"]:
        print("Owner issues found:")
        for issue in result["owners"]:
            print(f"  Line {issue['line']}: {issue['message']}")

asyncio.run(main())
```

<details>
<summary>Example with PyGithub (synchronous)</summary>

```python
from github import Github
from github.GithubException import UnknownObjectException, BadCredentialsException

class PyGithubClient:
    def __init__(self, token: str):
        self.client = Github(token)

    def user_exists(self, username: str) -> bool:
        try:
            self.client.get_user(username)
            return True
        except UnknownObjectException:
            return False

    def team_exists(self, org: str, team: str) -> str:
        try:
            self.client.get_organization(org).get_team_by_slug(team)
            return "exists"
        except UnknownObjectException:
            return "not_found"
        except BadCredentialsException:
            return "unauthorized"

# Can be used with validate_with_github - sync methods work too!
```

</details>

### Rust

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
codeowners-validator-core = { git = "https://github.com/donicrosby/codeowners-validator-rs" }
```

#### Basic Usage

```rust
use codeowners_validator_core::parse::parse_codeowners;
use codeowners_validator_core::validate::validate_syntax;

let input = r#"
# CODEOWNERS file
*.rs @rustacean
/docs/ @github/docs-team
"#;

// Parse the file
let parse_result = parse_codeowners(input);

if parse_result.is_ok() {
    // Validate syntax
    let validation = validate_syntax(&parse_result.ast);
    
    if validation.is_ok() {
        println!("CODEOWNERS file is valid!");
        
        // Extract rules for further processing
        for (pattern, owners) in parse_result.ast.extract_rules() {
            println!("Pattern: {} -> {:?}", pattern.text, owners);
        }
    } else {
        for error in &validation.errors {
            eprintln!("Validation error: {}", error);
        }
    }
} else {
    for error in &parse_result.errors {
        eprintln!("Parse error: {}", error);
    }
}
```

#### Running Multiple Checks

```rust
use codeowners_validator_core::parse::parse_codeowners;
use codeowners_validator_core::validate::checks::{
    Check, CheckConfig, CheckContext, CheckRunner,
    SyntaxCheck, FilesCheck, DupPatternsCheck,
};
use std::path::Path;

let content = std::fs::read_to_string("CODEOWNERS").unwrap();
let parse_result = parse_codeowners(&content);

let config = CheckConfig::new()
    .with_allow_unowned_patterns(false)
    .with_owners_must_be_teams(true);

let repo_path = Path::new(".");
let ctx = CheckContext::new(&parse_result.ast, repo_path, &config);

// Run individual checks
let syntax_result = SyntaxCheck::new().run(&ctx);
let files_result = FilesCheck::new().run(&ctx);
let dup_result = DupPatternsCheck::new().run(&ctx);

// Or use the CheckRunner
let runner = CheckRunner::with_all_checks();
let result = runner.run_sync(&parse_result.ast, repo_path, &config);

for error in result.errors {
    eprintln!("{}", error);
}
```

### CLI

```bash
# Basic usage (runs all checks except owners by default)
codeowners-validator --repository-path /path/to/repo

# With GitHub token for owner verification
export GITHUB_ACCESS_TOKEN=ghp_your_token
codeowners-validator --repository-path /path/to/repo

# Run specific checks only
codeowners-validator --checks syntax,files,duppatterns

# Run experimental checks
codeowners-validator --experimental-checks notowned,avoid-shadowing

# JSON output
codeowners-validator --json

# Verbose output
codeowners-validator -v    # Debug level
codeowners-validator -vv   # Trace level
```

#### CLI Options

| Option | Environment Variable | Description |
|--------|---------------------|-------------|
| `--repository-path` | `REPOSITORY_PATH` | Path to the repository root (default: `.`) |
| `--github-access-token` | `GITHUB_ACCESS_TOKEN` | GitHub PAT for owner validation |
| `--github-base-url` | `GITHUB_BASE_URL` | GitHub API URL (for Enterprise) |
| `--github-app-id` | `GITHUB_APP_ID` | GitHub App ID |
| `--github-app-installation-id` | `GITHUB_APP_INSTALLATION_ID` | GitHub App Installation ID |
| `--github-app-private-key` | `GITHUB_APP_PRIVATE_KEY` | GitHub App private key (PEM) |
| `--checks` | `CHECKS` | Comma-separated list of checks |
| `--experimental-checks` | `EXPERIMENTAL_CHECKS` | Comma-separated experimental checks |
| `--check-failure-level` | `CHECK_FAILURE_LEVEL` | `warning` or `error` |
| `--owner-checker-repository` | `OWNER_CHECKER_REPOSITORY` | Repository in `owner/repo` format |
| `--owner-checker-ignored-owners` | `OWNER_CHECKER_IGNORED_OWNERS` | Owners to ignore |
| `--owner-checker-allow-unowned-patterns` | `OWNER_CHECKER_ALLOW_UNOWNED_PATTERNS` | Allow patterns without owners |
| `--owner-checker-owners-must-be-teams` | `OWNER_CHECKER_OWNERS_MUST_BE_TEAMS` | Require team owners |
| `--not-owned-checker-skip-patterns` | `NOT_OWNED_CHECKER_SKIP_PATTERNS` | Patterns to skip for notowned check |
| `--json`, `-j` | - | Output as JSON |
| `--verbose`, `-v` | - | Increase verbosity |

#### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success, no issues found |
| 1 | Startup failure (config error, file not found, etc.) |
| 2 | Errors found in validation |
| 3 | Warnings found (when `--check-failure-level=warning`) |
| 130 | Terminated by signal |

---

## API Reference

### Python Types

The Python bindings include full type stubs for excellent IDE support. All types are exported and available for import:

```python
from codeowners_validator import (
    # Functions
    parse_codeowners,      # Parse CODEOWNERS content
    validate_codeowners,   # Validate (with optional GitHub client)
    # Protocol for custom GitHub clients
    GithubClientProtocol,
    # Configuration and result types
    CheckConfigDict,
    ParseResultDict,
    ValidationResultDict,
    IssueDict,
    # AST types
    AstDict,
    LineDict,
    LineKindDict,
    OwnerDict,
    PatternDict,
    SpanDict,
)
```

#### Configuration Dictionary

```python
from codeowners_validator import CheckConfigDict

config: CheckConfigDict = {
    "ignored_owners": ["@bot", "@ghost"],  # Owners to skip validation
    "owners_must_be_teams": False,         # Require @org/team format
    "allow_unowned_patterns": True,        # Allow patterns without owners
    "skip_patterns": ["*.generated.*"],    # Skip for notowned check
    "repository": "owner/repo",            # For owner validation
}
```

#### GitHub Client Protocol

Implement `GithubClientProtocol` to create custom GitHub clients for owner verification:

```python
from typing import Literal
from codeowners_validator import GithubClientProtocol, validate_codeowners

class MyGithubClient(GithubClientProtocol):
    """Custom GitHub client implementing the protocol."""
    
    async def user_exists(self, username: str) -> bool:
        # Your implementation
        ...
    
    async def team_exists(
        self, org: str, team: str
    ) -> Literal["exists", "not_found", "unauthorized"]:
        # Your implementation
        ...

# Usage
client = MyGithubClient()
result = await validate_codeowners(content, repo_path, github_client=client)
```

---

## Requirements

- **Rust**: 2024 edition (1.85+)
- **Python**: 3.10+ (for bindings)

## License

MIT License - see [LICENSE](LICENSE) for details.
