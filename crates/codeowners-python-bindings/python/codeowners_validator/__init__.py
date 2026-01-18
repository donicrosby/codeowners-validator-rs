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
"""

from codeowners_validator._codeowners_validator import (
    __version__,
    parse_codeowners,
    validate_codeowners,
    validate_with_github,
)

__all__ = [
    "__version__",
    "parse_codeowners",
    "validate_codeowners",
    "validate_with_github",
]
