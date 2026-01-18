"""Pytest configuration and fixtures for codeowners_validator tests."""


def pytest_configure(config):
    """Configure pytest."""
    config.addinivalue_line("markers", "asyncio: mark test as async")
