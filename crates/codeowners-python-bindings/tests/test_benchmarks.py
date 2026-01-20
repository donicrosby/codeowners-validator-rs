"""Benchmark tests for codeowners-validator Python bindings.

Run with: uv run pytest tests/test_benchmarks.py --benchmark-only
Compare: uv run pytest tests/test_benchmarks.py --benchmark-compare

Filter by group:
  uv run pytest tests/test_benchmarks.py --benchmark-only -k "parse"
  uv run pytest tests/test_benchmarks.py --benchmark-only -k "experimental"
"""

import asyncio
import threading

import pytest
from codeowners_validator import parse_codeowners, validate_codeowners

# Check profiles
STANDARD_CHECKS = ["syntax", "duppatterns", "files"]
EXPERIMENTAL_CHECKS = ["notowned", "avoid-shadowing"]
ALL_CHECKS = STANDARD_CHECKS + EXPERIMENTAL_CHECKS


# Thread-local event loop to avoid issues with parallel pytest
_thread_local = threading.local()


def _get_loop() -> asyncio.AbstractEventLoop:
    """Get or create a thread-local reusable event loop."""
    if not hasattr(_thread_local, "loop") or _thread_local.loop.is_closed():
        _thread_local.loop = asyncio.new_event_loop()
    return _thread_local.loop


def _run_async(coro):
    """Run async function using thread-local event loop."""
    return _get_loop().run_until_complete(coro)


class TestParseBenchmarks:
    """Benchmarks for parse_codeowners function."""

    @pytest.mark.benchmark(group="parse")
    @pytest.mark.parametrize("fixture_name", ["small", "medium", "large"])
    def test_parse(self, benchmark, fixture_name, request):
        """Benchmark parsing across fixture sizes."""
        fixture = request.getfixturevalue(f"fixture_{fixture_name}")
        result = benchmark(parse_codeowners, fixture)
        assert result["is_ok"]


class TestStandardChecksBenchmarks:
    """Benchmarks for standard validation checks."""

    @pytest.mark.benchmark(group="checks/standard")
    @pytest.mark.parametrize("check", STANDARD_CHECKS)
    def test_individual_check(self, benchmark, check, repo_with_codeowners):
        """Benchmark each standard check individually."""

        async def run():
            return await validate_codeowners(str(repo_with_codeowners), checks=[check])

        result = benchmark(lambda: _run_async(run()))
        assert check in result

    @pytest.mark.benchmark(group="checks/standard")
    def test_all_standard_checks(self, benchmark, repo_with_codeowners):
        """Benchmark all standard checks combined."""

        async def run():
            return await validate_codeowners(str(repo_with_codeowners), checks=STANDARD_CHECKS)

        result = benchmark(lambda: _run_async(run()))
        for check in STANDARD_CHECKS:
            assert check in result


class TestExperimentalChecksBenchmarks:
    """Benchmarks for experimental validation checks."""

    @pytest.mark.benchmark(group="checks/experimental")
    @pytest.mark.parametrize("check", EXPERIMENTAL_CHECKS)
    def test_individual_check(self, benchmark, check, repo_with_codeowners):
        """Benchmark each experimental check individually."""

        async def run():
            return await validate_codeowners(str(repo_with_codeowners), checks=[check])

        result = benchmark(lambda: _run_async(run()))
        assert check in result

    @pytest.mark.benchmark(group="checks/experimental")
    def test_all_experimental_checks(self, benchmark, repo_with_codeowners):
        """Benchmark all experimental checks combined."""

        async def run():
            return await validate_codeowners(str(repo_with_codeowners), checks=EXPERIMENTAL_CHECKS)

        result = benchmark(lambda: _run_async(run()))
        for check in EXPERIMENTAL_CHECKS:
            assert check in result


class TestCombinedBenchmarks:
    """Benchmarks for combined workloads."""

    @pytest.mark.benchmark(group="combined")
    def test_all_checks_large(self, benchmark, repo_with_codeowners_large):
        """Benchmark all checks on large file - worst case scenario."""

        async def run():
            return await validate_codeowners(str(repo_with_codeowners_large), checks=ALL_CHECKS)

        result = benchmark(lambda: _run_async(run()))
        for check in ALL_CHECKS:
            assert check in result
