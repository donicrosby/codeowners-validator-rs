"""Runtime type definitions for codeowners_validator.

This module contains TypedDict and Protocol definitions that can be imported
at runtime for use in type annotations and isinstance checks.
"""

from collections.abc import Awaitable
from typing import Literal, Protocol, TypedDict


class SpanDict(TypedDict):
    """Location information for a token in the source."""

    offset: int
    line: int
    column: int
    length: int


class OwnerDict(TypedDict):
    """An owner entry in a CODEOWNERS rule."""

    type: Literal["user", "team", "email"]
    text: str
    span: SpanDict
    # For user type
    name: str  # only present for user
    # For team type
    org: str  # only present for team
    team: str  # only present for team
    # For email type
    email: str  # only present for email


class PatternDict(TypedDict):
    """A file pattern in a CODEOWNERS rule."""

    text: str
    span: SpanDict


class LineKindDict(TypedDict, total=False):
    """The content of a line in a CODEOWNERS file."""

    type: Literal["blank", "comment", "rule", "invalid"]
    # For comment type
    content: str
    # For rule type
    pattern: PatternDict
    owners: list[OwnerDict]
    # For invalid type
    raw: str
    error: str


class LineDict(TypedDict):
    """A line in a CODEOWNERS file."""

    kind: LineKindDict
    span: SpanDict


class AstDict(TypedDict):
    """The parsed AST of a CODEOWNERS file."""

    lines: list[LineDict]


class ParseResultDict(TypedDict):
    """The result of parsing a CODEOWNERS file."""

    is_ok: bool
    ast: AstDict
    errors: list[str]


class IssueDict(TypedDict):
    """A validation issue."""

    span: SpanDict | None
    message: str
    severity: Literal["error", "warning"]


class ValidationResultDict(TypedDict):
    """The result of validating a CODEOWNERS file."""

    syntax: list[IssueDict]
    files: list[IssueDict]
    duppatterns: list[IssueDict]
    owners: list[IssueDict]
    notowned: list[IssueDict]


class CheckConfigDict(TypedDict, total=False):
    """Configuration for validation checks."""

    ignored_owners: list[str]
    owners_must_be_teams: bool
    allow_unowned_patterns: bool
    skip_patterns: list[str]
    repository: str


class GithubClientProtocol(Protocol):
    """Protocol for GitHub client implementations.

    Implement this protocol to provide a GitHub client for owner validation.
    The methods can be async or sync - the Rust code handles both.

    Example with githubkit:
        ```python
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

            async def team_exists(self, org: str, team: str) -> Literal["exists", "not_found", "unauthorized"]:
                try:
                    await self.client.rest.teams.async_get_by_name(org, team)
                    return "exists"
                except Exception as e:
                    if "404" in str(e):
                        return "not_found"
                    return "unauthorized"
        ```

    Example with PyGithub:
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

            def team_exists(self, org: str, team: str) -> Literal["exists", "not_found", "unauthorized"]:
                try:
                    self.client.get_organization(org).get_team_by_slug(team)
                    return "exists"
                except UnknownObjectException:
                    return "not_found"
                except BadCredentialsException:
                    return "unauthorized"
        ```
    """

    def user_exists(self, username: str) -> bool | Awaitable[bool]:
        """Check if a GitHub user exists.

        Args:
            username: The GitHub username (without the leading '@')

        Returns:
            True if the user exists, False otherwise.
            Can be sync (returns bool) or async (returns Awaitable[bool]).
        """
        ...

    def team_exists(
        self, org: str, team: str
    ) -> Literal["exists", "not_found", "unauthorized"] | Awaitable[Literal["exists", "not_found", "unauthorized"]]:
        """Check if a GitHub team exists in an organization.

        Args:
            org: The organization name
            team: The team slug (name)

        Returns:
            "exists" if the team exists
            "not_found" if the team doesn't exist
            "unauthorized" if the client doesn't have permission to check
            Can be sync or async.
        """
        ...


__all__ = [
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
