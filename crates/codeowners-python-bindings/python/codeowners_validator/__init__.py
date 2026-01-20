"""
CODEOWNERS Validator - A fast CODEOWNERS file validator with Python bindings.

This package provides functions to parse and validate GitHub CODEOWNERS files.
It's built on a Rust core for speed and reliability.

Example:
    >>> from codeowners_validator import parse_codeowners, validate_codeowners
    >>>
    >>> # Parse a CODEOWNERS file
    >>> result = parse_codeowners("*.rs @rustacean\\n/docs/ @docs-team\\n")
    >>> print(f"Parsed {len(result['ast']['lines'])} lines")
    Parsed 2 lines
    >>>
    >>> # Validate a CODEOWNERS file
    >>> result = validate_codeowners("*.rs @rustacean\\n", "/path/to/repo")
    >>> if not result["syntax"]:
    ...     print("No syntax errors!")
    No syntax errors!

Types:
    The following types are available for type annotations:

    - ``GithubClientProtocol``: Protocol for implementing custom GitHub clients
    - ``CheckConfigDict``: Configuration options for validation
    - ``ParseResultDict``: Return type of ``parse_codeowners()``
    - ``ValidationResultDict``: Return type of ``validate_codeowners()``
    - ``IssueDict``: Individual validation issue
    - ``SpanDict``: Location information in source
    - ``AstDict``: Full AST structure
    - ``LineDict``, ``LineKindDict``: AST line types
    - ``OwnerDict``, ``PatternDict``: AST component types

Example with type annotations:
    >>> from codeowners_validator import GithubClientProtocol, CheckConfigDict
    >>>
    >>> class MyClient(GithubClientProtocol):
    ...     async def user_exists(self, username: str) -> bool:
    ...         return True
    ...     async def team_exists(self, org: str, team: str) -> str:
    ...         return "exists"
"""

from codeowners_validator._codeowners_validator import (
    __version__,
    parse_codeowners,
    validate_codeowners,
)
from codeowners_validator._types import (
    AstDict,
    CheckConfigDict,
    GithubClientProtocol,
    IssueDict,
    LineDict,
    LineKindDict,
    OwnerDict,
    ParseResultDict,
    PatternDict,
    SpanDict,
    ValidationResultDict,
)

# The generate function is available when built with the 'generate' feature (default)
try:
    from codeowners_validator._codeowners_validator import generate_codeowners_fixture
except ImportError:
    generate_codeowners_fixture = None  # type: ignore[assignment,misc]

__all__ = [
    # Version
    "__version__",
    # Functions
    "parse_codeowners",
    "validate_codeowners",
    "generate_codeowners_fixture",
    # Types (available at runtime)
    "AstDict",
    "CheckConfigDict",
    "GithubClientProtocol",
    "IssueDict",
    "LineDict",
    "LineKindDict",
    "OwnerDict",
    "ParseResultDict",
    "PatternDict",
    "SpanDict",
    "ValidationResultDict",
]
