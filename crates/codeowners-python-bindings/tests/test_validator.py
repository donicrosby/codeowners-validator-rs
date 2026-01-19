"""Tests for the codeowners_validator package."""

import tempfile
from collections.abc import Generator
from pathlib import Path
from typing import TYPE_CHECKING, Literal

import pytest

if TYPE_CHECKING:
    from codeowners_validator._codeowners_validator import CheckConfigDict


class MockGithubClient:
    """A mock GitHub client for testing."""

    def __init__(
        self,
        existing_users: set[str] | None = None,
        existing_teams: set[tuple[str, str]] | None = None,
        unauthorized_teams: set[tuple[str, str]] | None = None,
    ):
        self.existing_users = existing_users or set()
        self.existing_teams = existing_teams or set()
        self.unauthorized_teams = unauthorized_teams or set()
        self.user_calls: list[str] = []
        self.team_calls: list[tuple[str, str]] = []

    def user_exists(self, username: str) -> bool:
        self.user_calls.append(username)
        return username in self.existing_users

    def team_exists(self, org: str, team: str) -> Literal["exists", "not_found", "unauthorized"]:
        self.team_calls.append((org, team))
        if (org, team) in self.unauthorized_teams:
            return "unauthorized"
        if (org, team) in self.existing_teams:
            return "exists"
        return "not_found"


class AsyncMockGithubClient:
    """An async mock GitHub client for testing."""

    def __init__(
        self,
        existing_users: set[str] | None = None,
        existing_teams: set[tuple[str, str]] | None = None,
    ):
        self.existing_users = existing_users or set()
        self.existing_teams = existing_teams or set()

    async def user_exists(self, username: str) -> bool:
        return username in self.existing_users

    async def team_exists(self, org: str, team: str) -> Literal["exists", "not_found", "unauthorized"]:
        if (org, team) in self.existing_teams:
            return "exists"
        return "not_found"


@pytest.fixture
def temp_repo() -> Generator[str, None, None]:
    """Create a temporary repository with some files for testing."""
    with tempfile.TemporaryDirectory() as tmpdir:
        # Create some files
        Path(tmpdir, "README.md").touch()
        Path(tmpdir, "src").mkdir()
        Path(tmpdir, "src/main.rs").touch()
        Path(tmpdir, "src/lib.rs").touch()
        Path(tmpdir, "docs").mkdir()
        Path(tmpdir, "docs/README.md").touch()
        # Create .github directory for CODEOWNERS
        Path(tmpdir, ".github").mkdir()
        yield tmpdir


def write_codeowners(repo_path: str, content: str) -> None:
    """Helper to write a CODEOWNERS file in a repository."""
    codeowners_path = Path(repo_path, ".github", "CODEOWNERS")
    codeowners_path.write_text(content)


class TestParseCodeowners:
    """Tests for parse_codeowners function."""

    def test_parse_simple_rule(self):
        """Test parsing a simple CODEOWNERS rule."""
        from codeowners_validator import parse_codeowners

        result = parse_codeowners("*.rs @rustacean\n")

        assert result["is_ok"] is True
        assert len(result["errors"]) == 0
        assert len(result["ast"]["lines"]) == 1

        line = result["ast"]["lines"][0]
        assert line["kind"].get("type") == "rule"
        assert line["kind"].get("pattern", {}).get("text") == "*.rs"
        assert len(line["kind"].get("owners", [])) == 1
        assert line["kind"].get("owners", [])[0].get("type") == "user"
        assert line["kind"].get("owners", [])[0].get("name") == "rustacean"

    def test_parse_team_owner(self):
        """Test parsing a rule with a team owner."""
        from codeowners_validator import parse_codeowners

        result = parse_codeowners("/docs/ @github/docs-team\n")

        assert result["is_ok"] is True
        line = result["ast"]["lines"][0]
        assert line["kind"].get("type") == "rule"
        owner = line["kind"].get("owners", [])[0]
        assert owner["type"] == "team"
        assert owner["org"] == "github"
        assert owner["team"] == "docs-team"

    def test_parse_email_owner(self):
        """Test parsing a rule with an email owner."""
        from codeowners_validator import parse_codeowners

        result = parse_codeowners("*.md user@example.com\n")

        assert result["is_ok"] is True
        line = result["ast"]["lines"][0]
        owner = line["kind"].get("owners", [])[0]
        assert owner["type"] == "email"
        assert owner["email"] == "user@example.com"

    def test_parse_multiple_owners(self):
        """Test parsing a rule with multiple owners."""
        from codeowners_validator import parse_codeowners

        result = parse_codeowners("*.rs @user1 @org/team @user2\n")

        assert result["is_ok"] is True
        line = result["ast"]["lines"][0]
        assert len(line["kind"].get("owners", [])) == 3

    def test_parse_comment(self):
        """Test parsing a comment line."""
        from codeowners_validator import parse_codeowners

        result = parse_codeowners("# This is a comment\n")

        assert result["is_ok"] is True
        line = result["ast"]["lines"][0]
        assert line["kind"].get("type") == "comment"
        assert line["kind"].get("content") == " This is a comment"

    def test_parse_blank_line(self):
        """Test parsing a blank line."""
        from codeowners_validator import parse_codeowners

        result = parse_codeowners("\n")

        assert result["is_ok"] is True
        line = result["ast"]["lines"][0]
        assert line["kind"].get("type") == "blank"

    def test_parse_mixed_content(self):
        """Test parsing a file with mixed content."""
        from codeowners_validator import parse_codeowners

        content = """# CODEOWNERS file
*.rs @rustacean

/docs/ @github/docs-team
"""
        result = parse_codeowners(content)

        assert result["is_ok"] is True
        assert len(result["ast"]["lines"]) == 4  # comment, rule, blank, rule


class TestValidateCodeowners:
    """Tests for validate_codeowners function."""

    @pytest.mark.asyncio
    async def test_validate_valid_file(self, temp_repo: str) -> None:
        """Test validating a valid CODEOWNERS file."""
        from codeowners_validator import validate_codeowners

        write_codeowners(temp_repo, "*.rs @rustacean\n")
        result = await validate_codeowners(temp_repo)

        # Check that we got results for each check
        assert "syntax" in result
        assert "files" in result
        assert "duppatterns" in result
        assert "owners" in result  # Empty since no GitHub client

        # Should have no syntax errors
        assert len(result["syntax"]) == 0

    @pytest.mark.asyncio
    async def test_validate_duplicate_patterns(self, temp_repo: str) -> None:
        """Test detecting duplicate patterns."""
        from codeowners_validator import validate_codeowners

        write_codeowners(
            temp_repo,
            """*.rs @user1
*.rs @user2
""",
        )
        result = await validate_codeowners(temp_repo)

        # Should detect duplicate pattern
        assert len(result["duppatterns"]) > 0
        assert any("duplicate" in issue["message"].lower() for issue in result["duppatterns"])

    @pytest.mark.asyncio
    async def test_validate_with_config(self, temp_repo: str) -> None:
        """Test validation with custom configuration."""
        from codeowners_validator import validate_codeowners

        write_codeowners(temp_repo, "*.rs @ignored-user\n")
        config: CheckConfigDict = {
            "ignored_owners": ["@ignored-user"],
        }
        result = await validate_codeowners(temp_repo, config=config)

        # Should have no errors (owner is ignored)
        assert len(result["syntax"]) == 0

    @pytest.mark.asyncio
    async def test_validate_specific_checks(self, temp_repo: str) -> None:
        """Test running only specific checks."""
        from codeowners_validator import validate_codeowners

        write_codeowners(temp_repo, "*.rs @rustacean\n")
        result = await validate_codeowners(temp_repo, checks=["syntax"])

        # Should have run syntax check
        assert "syntax" in result
        assert len(result["syntax"]) == 0

    @pytest.mark.asyncio
    async def test_validate_notowned_check(self, temp_repo: str) -> None:
        """Test the not-owned experimental check."""
        from codeowners_validator import validate_codeowners

        # Only cover .rs files, leaving other files unowned
        write_codeowners(temp_repo, "*.rs @rustacean\n")
        result = await validate_codeowners(temp_repo, checks=["notowned"])

        # Should detect files not covered by any rule
        # Note: The actual result depends on what files exist in temp_repo
        assert "notowned" in result

    @pytest.mark.asyncio
    async def test_validate_file_not_found(self) -> None:
        """Test that FileNotFoundError is raised when CODEOWNERS is missing."""
        from codeowners_validator import validate_codeowners

        with (
            tempfile.TemporaryDirectory() as tmpdir,
            pytest.raises(FileNotFoundError, match="CODEOWNERS file not found"),
        ):
            # Don't create a CODEOWNERS file
            await validate_codeowners(tmpdir)


class TestValidateWithGithub:
    """Tests for validate_codeowners with github_client."""

    @pytest.mark.asyncio
    async def test_validate_user_exists(self, temp_repo: str) -> None:
        """Test validation with a user that exists."""
        from codeowners_validator import validate_codeowners

        write_codeowners(temp_repo, "*.rs @validuser\n")
        client = MockGithubClient(existing_users={"validuser"})

        result = await validate_codeowners(temp_repo, github_client=client)

        assert "owners" in result
        assert len(result["owners"]) == 0  # No errors
        assert "validuser" in client.user_calls

    @pytest.mark.asyncio
    async def test_validate_user_not_found(self, temp_repo: str) -> None:
        """Test validation with a user that doesn't exist."""
        from codeowners_validator import validate_codeowners

        write_codeowners(temp_repo, "*.rs @ghostuser\n")
        client = MockGithubClient()  # No users

        result = await validate_codeowners(temp_repo, github_client=client)

        assert "owners" in result
        assert len(result["owners"]) > 0
        assert any("ghostuser" in issue["message"] for issue in result["owners"])

    @pytest.mark.asyncio
    async def test_validate_team_exists(self, temp_repo: str) -> None:
        """Test validation with a team that exists."""
        from codeowners_validator import validate_codeowners

        write_codeowners(temp_repo, "*.rs @myorg/myteam\n")
        client = MockGithubClient(existing_teams={("myorg", "myteam")})

        result = await validate_codeowners(temp_repo, github_client=client)

        assert "owners" in result
        assert len(result["owners"]) == 0  # No errors
        assert ("myorg", "myteam") in client.team_calls

    @pytest.mark.asyncio
    async def test_validate_team_unauthorized(self, temp_repo: str) -> None:
        """Test validation with a team that returns unauthorized."""
        from codeowners_validator import validate_codeowners

        write_codeowners(temp_repo, "*.rs @privateorg/privateteam\n")
        client = MockGithubClient(unauthorized_teams={("privateorg", "privateteam")})

        result = await validate_codeowners(temp_repo, github_client=client)

        assert "owners" in result
        assert len(result["owners"]) > 0
        # Should be an authorization error, not a not-found error
        assert any("authorization" in issue["message"].lower() for issue in result["owners"])

    @pytest.mark.asyncio
    async def test_validate_with_async_client(self, temp_repo: str) -> None:
        """Test validation with an async GitHub client."""
        from codeowners_validator import validate_codeowners

        write_codeowners(temp_repo, "*.rs @asyncuser\n")
        client = AsyncMockGithubClient(existing_users={"asyncuser"})

        result = await validate_codeowners(temp_repo, github_client=client)

        assert "owners" in result
        assert len(result["owners"]) == 0

    @pytest.mark.asyncio
    async def test_validate_with_owners_must_be_teams(self, temp_repo: str) -> None:
        """Test validation requiring team owners."""
        from codeowners_validator import validate_codeowners

        write_codeowners(temp_repo, "*.rs @individual-user\n")
        client = MockGithubClient(existing_users={"individual-user"})
        config: CheckConfigDict = {"owners_must_be_teams": True}

        result = await validate_codeowners(temp_repo, config=config, github_client=client)

        assert "owners" in result
        assert len(result["owners"]) > 0
        # Should fail because individual users are not allowed
        assert any("team" in issue["message"].lower() for issue in result["owners"])

    @pytest.mark.asyncio
    async def test_validate_with_ignored_owners(self, temp_repo: str) -> None:
        """Test validation with ignored owners."""
        from codeowners_validator import validate_codeowners

        write_codeowners(temp_repo, "*.rs @ignored-owner\n")
        client = MockGithubClient()  # User doesn't exist
        config: CheckConfigDict = {"ignored_owners": ["@ignored-owner"]}

        result = await validate_codeowners(temp_repo, config=config, github_client=client)

        assert "owners" in result
        assert len(result["owners"]) == 0  # Should be ignored


class TestIssueFormat:
    """Tests for the format of validation issues."""

    @pytest.mark.asyncio
    async def test_issue_has_required_fields(self, temp_repo: str) -> None:
        """Test that issues have all required fields."""
        from codeowners_validator import validate_codeowners

        # Create a file with a syntax issue
        write_codeowners(temp_repo, "*.rs @invalid--owner\n")  # Double hyphen might be invalid
        result = await validate_codeowners(temp_repo)

        # Check any issues have the expected format
        all_issues = result.get("syntax", []) + result.get("files", []) + result.get("duppatterns", [])

        for issue in all_issues:
            # All issues should have these fields
            assert "message" in issue
            assert "severity" in issue
            assert issue["severity"] in ("error", "warning")
            # span can be None for issues without location (e.g., FileNotOwned)
            assert "span" in issue
            if issue["span"] is not None:
                assert "offset" in issue["span"]
                assert "line" in issue["span"]
                assert "column" in issue["span"]
                assert "length" in issue["span"]


class TestSpanFormat:
    """Tests for the format of span information."""

    def test_span_has_required_fields(self):
        """Test that spans have all required fields."""
        from codeowners_validator import parse_codeowners

        result = parse_codeowners("*.rs @owner\n")
        line = result["ast"]["lines"][0]

        span = line["span"]
        assert "offset" in span
        assert "line" in span
        assert "column" in span
        assert "length" in span

        # Values should be non-negative integers
        assert isinstance(span["offset"], int) and span["offset"] >= 0
        assert isinstance(span["line"], int) and span["line"] >= 1
        assert isinstance(span["column"], int) and span["column"] >= 1
        assert isinstance(span["length"], int) and span["length"] >= 0
