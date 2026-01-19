# codeowners-validator

A fast CODEOWNERS file validator with Python bindings, powered by Rust.

## Features

- **Fast parsing** - Built on a Rust core for speed and reliability
- **Comprehensive validation** - Check syntax, file patterns, duplicate patterns, and more
- **GitHub integration** - Verify that owners actually exist on GitHub
- **Fully typed** - Complete type stubs for excellent IDE support
- **Async support** - Works with both sync and async GitHub clients

## Installation

This package requires [maturin](https://github.com/PyO3/maturin) to build from source.

### Using uv (Recommended)

```bash
# Install directly from git
uv add "codeowners-validator @ git+https://github.com/donicrosby/codeowners-validator-rs#subdirectory=crates/codeowners-python-bindings"

# With optional GitHub client dependencies
uv add "codeowners-validator[githubkit] @ git+https://github.com/donicrosby/codeowners-validator-rs#subdirectory=crates/codeowners-python-bindings"

# Or with PyGithub
uv add "codeowners-validator[pygithub] @ git+https://github.com/donicrosby/codeowners-validator-rs#subdirectory=crates/codeowners-python-bindings"
```

### Using pip

```bash
# Install directly from git
pip install "codeowners-validator @ git+https://github.com/donicrosby/codeowners-validator-rs#subdirectory=crates/codeowners-python-bindings"

# With optional GitHub client dependencies
pip install "codeowners-validator[githubkit] @ git+https://github.com/donicrosby/codeowners-validator-rs#subdirectory=crates/codeowners-python-bindings"
```

### From Local Clone

```bash
git clone https://github.com/donicrosby/codeowners-validator-rs.git
cd codeowners-validator-rs/crates/codeowners-python-bindings

# Using uv
uv add -e .

# Or using pip with maturin
pip install maturin
maturin develop
```

## Quick Start

### Parsing CODEOWNERS Files

```python
from codeowners_validator import parse_codeowners

content = """
# Frontend team owns all JS/TS files
*.js @frontend-team
*.ts @frontend-team

# Documentation
/docs/ @org/docs-team
"""

result = parse_codeowners(content)

if result["is_ok"]:
    for line in result["ast"]["lines"]:
        if line["kind"]["type"] == "rule":
            pattern = line["kind"]["pattern"]["text"]
            owners = [o["text"] for o in line["kind"]["owners"]]
            print(f"{pattern} -> {owners}")
else:
    for error in result["errors"]:
        print(f"Parse error: {error}")
```

### Validating CODEOWNERS Files

```python
from codeowners_validator import validate_codeowners

content = "*.rs @rustacean\n/docs/ @docs-team\n"
repo_path = "/path/to/your/repo"

result = validate_codeowners(content, repo_path)

# Check for issues in each category
for check_name in ["syntax", "files", "duppatterns"]:
    issues = result[check_name]
    for issue in issues:
        print(f"[{issue['severity']}] Line {issue['line']}: {issue['message']}")
```

### Validation with GitHub Owner Verification

Verify that owners (users and teams) actually exist on GitHub:

```python
import asyncio
from codeowners_validator import validate_with_github

# Option 1: Using githubkit
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

# Option 2: Using PyGithub
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

async def main():
    content = "*.rs @rustacean\n/docs/ @myorg/docs-team\n"
    repo_path = "/path/to/repo"
    
    client = GithubKitClient("your-github-token")
    # Or: client = PyGithubClient("your-github-token")
    
    result = await validate_with_github(content, repo_path, client)
    
    for issue in result["owners"]:
        print(f"Owner issue: {issue['message']}")

asyncio.run(main())
```

## Configuration Options

Pass a configuration dictionary to customize validation behavior:

```python
config = {
    # Owners to skip during validation
    "ignored_owners": ["@bot-user", "@legacy-team"],
    
    # Require all owners to be teams (no individual users)
    "owners_must_be_teams": True,
    
    # Allow patterns without any owners
    "allow_unowned_patterns": False,
    
    # Skip these patterns for the not-owned check
    "skip_patterns": ["vendor/*", "generated/*"],
    
    # Repository in "owner/repo" format (for owner validation context)
    "repository": "myorg/myrepo",
}

result = validate_codeowners(content, repo_path, config=config)
```

## Available Checks

Specify which checks to run with the `checks` parameter:

```python
result = validate_codeowners(
    content, 
    repo_path, 
    checks=["syntax", "files", "duppatterns"]
)
```

| Check | Description |
|-------|-------------|
| `syntax` | Validates CODEOWNERS syntax |
| `files` | Checks that patterns match actual files in the repo |
| `duppatterns` | Detects duplicate patterns |
| `owners` | Verifies owners exist on GitHub (requires GitHub client) |
| `notowned` | Finds files not covered by any rule (experimental) |
| `avoid-shadowing` | Detects patterns that shadow earlier rules (experimental) |

## API Reference

### `parse_codeowners(content: str) -> ParseResultDict`

Parses CODEOWNERS content and returns the AST.

**Returns:**
- `is_ok`: Whether parsing succeeded
- `ast`: The parsed abstract syntax tree
- `errors`: List of parse errors (if any)

### `validate_codeowners(content, repo_path, config=None, checks=None) -> ValidationResultDict`

Validates CODEOWNERS content without GitHub API calls.

**Parameters:**
- `content`: CODEOWNERS file content
- `repo_path`: Path to the repository root
- `config`: Optional configuration dictionary
- `checks`: Optional list of checks to run

**Returns:** Dictionary with issues grouped by check name.

### `validate_with_github(content, repo_path, github_client, config=None, checks=None) -> ValidationResultDict`

Validates CODEOWNERS content with GitHub owner verification.

**Parameters:**
- `content`: CODEOWNERS file content
- `repo_path`: Path to the repository root
- `github_client`: GitHub client implementing `user_exists()` and `team_exists()`
- `config`: Optional configuration dictionary
- `checks`: Optional list of checks to run

**Returns:** Dictionary with issues grouped by check name, including `owners` check results.

## Type Annotations

This package exports type definitions for excellent IDE support and type checking. All types are available for import:

```python
from codeowners_validator import (
    # Protocol for implementing GitHub clients
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

### `GithubClientProtocol`

Protocol class for implementing custom GitHub clients. Implement this protocol to enable owner verification:

```python
from typing import Literal
from codeowners_validator import GithubClientProtocol, validate_codeowners

class MyGithubClient(GithubClientProtocol):
    """Custom GitHub client implementing the protocol."""
    
    def __init__(self, token: str):
        self.token = token
    
    async def user_exists(self, username: str) -> bool:
        """Check if a GitHub user exists.
        
        Args:
            username: GitHub username (without leading '@')
        
        Returns:
            True if user exists, False otherwise
        """
        # Your implementation here
        ...
    
    async def team_exists(
        self, org: str, team: str
    ) -> Literal["exists", "not_found", "unauthorized"]:
        """Check if a GitHub team exists.
        
        Args:
            org: Organization name
            team: Team slug
        
        Returns:
            "exists" if team exists,
            "not_found" if team doesn't exist,
            "unauthorized" if insufficient permissions
        """
        # Your implementation here
        ...

# Usage with type annotations
async def validate_with_types() -> None:
    client = MyGithubClient("ghp_token")
    result = await validate_codeowners(
        "*.rs @rustacean\n",
        "/path/to/repo",
        github_client=client
    )
    for issue in result["owners"]:
        print(issue["message"])
```

### Type Definitions

| Type | Description |
|------|-------------|
| `GithubClientProtocol` | Protocol for GitHub client implementations |
| `CheckConfigDict` | Configuration options (`ignored_owners`, `owners_must_be_teams`, etc.) |
| `ParseResultDict` | Return type of `parse_codeowners()` |
| `ValidationResultDict` | Return type of `validate_codeowners()` |
| `IssueDict` | Validation issue with `span`, `message`, `severity` |
| `SpanDict` | Source location with `offset`, `line`, `column`, `length` |
| `AstDict` | Parsed AST containing `lines` |
| `LineDict` | Single line with `kind` and `span` |
| `LineKindDict` | Line content: `blank`, `comment`, `rule`, or `invalid` |
| `OwnerDict` | Owner entry: `user`, `team`, or `email` type |
| `PatternDict` | File pattern with `text` and `span` |

## Development

This project uses [uv](https://docs.astral.sh/uv/) for Python dependency management and [maturin](https://www.maturin.rs/) for building the Rust extension.

### Setup

```bash
cd crates/codeowners-python-bindings

# Install dependencies and create virtual environment
uv sync

# Build and install the extension in development mode
uv run maturin develop

# Run tests
uv run pytest

# Lint
uv run ruff check .

# Type check
uv run mypy python/
```

### Building

```bash
# Build a wheel
uv run maturin build --release

# Build and install locally
uv run maturin develop --release
```

## License

MIT
