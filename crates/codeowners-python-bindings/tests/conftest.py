"""Pytest configuration and shared fixtures for benchmarks."""

from functools import cache
from pathlib import Path

import pytest
from codeowners_validator import generate_codeowners_fixture


def pytest_configure(config):
    """Configure pytest."""
    config.addinivalue_line("markers", "asyncio: mark test as async")
    config.addinivalue_line("markers", "benchmark: mark test as benchmark")


# Single source of truth for fixture configurations (rules, comments)
FIXTURE_CONFIGS: dict[str, tuple[int, int]] = {
    "small": (10, 5),
    "medium": (100, 20),
    "large": (1_000, 100),
    "xlarge": (10_000, 500),
}

# Extensions and directories matching the Rust generator vocabulary
EXTENSIONS = ["rs", "py", "js", "ts", "go", "md", "yaml", "json", "toml"]
DIRECTORIES = ["src", "lib", "tests", "docs", "config", "scripts", "api", "core"]


@cache
def _generate_fixture(name: str) -> str:
    """Generate and cache a fixture by name."""
    rules, comments = FIXTURE_CONFIGS[name]
    return generate_codeowners_fixture(num_rules=rules, num_comments=comments)


def create_benchmark_repo(root: Path) -> None:
    """Create a file structure matching generated CODEOWNERS patterns.

    This creates files that match patterns used by the generator:
    - Various extensions: .rs, .py, .js, .ts, .go, .md, .yaml, .json, .toml
    - Directories: src, lib, tests, docs, config, scripts, api, core
    - Nested files for ** glob patterns
    - Test files for test_* patterns
    """
    # Create root-level files for each extension (matches *.{ext})
    for ext in EXTENSIONS:
        (root / f"file.{ext}").touch()

    # Create directory structure with files
    for dir_name in DIRECTORIES:
        dir_path = root / dir_name
        dir_path.mkdir(parents=True, exist_ok=True)

        # Files in directory (matches /{dir}/*.{ext})
        for ext in EXTENSIONS:
            (dir_path / f"file.{ext}").touch()

        # Nested subdirectory (matches /{dir}/**)
        sub_dir = dir_path / "sub"
        sub_dir.mkdir(exist_ok=True)
        for ext in EXTENSIONS:
            (sub_dir / f"nested.{ext}").touch()
            # Test files (matches /{dir}/**/test_*.{ext})
            (sub_dir / f"test_example.{ext}").touch()

    # Create /src/{dir}/ structure
    for dir_name in DIRECTORIES:
        dir_path = root / "src" / dir_name
        dir_path.mkdir(parents=True, exist_ok=True)
        (dir_path / "mod.rs").touch()

    # Create docs/**/*.md structure
    docs_sub = root / "docs" / "guide"
    docs_sub.mkdir(parents=True, exist_ok=True)
    (docs_sub / "README.md").touch()
    (docs_sub / "guide.md").touch()

    # Create vendor directory (for !vendor/ negation patterns)
    vendor = root / "vendor"
    vendor.mkdir(exist_ok=True)
    (vendor / "external.rs").touch()


@pytest.fixture(scope="session")
def fixture_small() -> str:
    """Small CODEOWNERS fixture (~10 rules)."""
    return _generate_fixture("small")


@pytest.fixture(scope="session")
def fixture_medium() -> str:
    """Medium CODEOWNERS fixture (~100 rules)."""
    return _generate_fixture("medium")


@pytest.fixture(scope="session")
def fixture_large() -> str:
    """Large CODEOWNERS fixture (~1000 rules)."""
    return _generate_fixture("large")


@pytest.fixture(scope="session")
def fixture_xlarge() -> str:
    """Extra large CODEOWNERS fixture (~10k rules)."""
    return _generate_fixture("xlarge")


@pytest.fixture(scope="session", params=["small", "medium", "large"])
def fixture_all(request) -> tuple[str, str]:
    """Parameterized fixture returning (name, content) for standard sizes."""
    return request.param, _generate_fixture(request.param)


@pytest.fixture(scope="module")
def repo_with_codeowners(fixture_medium: str, tmp_path_factory) -> Path:
    """Create a temporary repo with medium CODEOWNERS file (module-scoped).

    The repo includes files matching the generated CODEOWNERS patterns.
    """
    tmp = tmp_path_factory.mktemp("repo")
    # Create file structure matching generated patterns
    create_benchmark_repo(tmp)
    # Add CODEOWNERS file
    codeowners = tmp / ".github" / "CODEOWNERS"
    codeowners.parent.mkdir(parents=True, exist_ok=True)
    codeowners.write_text(fixture_medium)
    return tmp


@pytest.fixture(scope="module")
def repo_with_codeowners_large(fixture_large: str, tmp_path_factory) -> Path:
    """Create a temporary repo with large CODEOWNERS file (module-scoped).

    The repo includes files matching the generated CODEOWNERS patterns.
    """
    tmp = tmp_path_factory.mktemp("repo_large")
    # Create file structure matching generated patterns
    create_benchmark_repo(tmp)
    # Add CODEOWNERS file
    codeowners = tmp / ".github" / "CODEOWNERS"
    codeowners.parent.mkdir(parents=True, exist_ok=True)
    codeowners.write_text(fixture_large)
    return tmp
