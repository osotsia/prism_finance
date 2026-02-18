import pytest

def pytest_addoption(parser):
    """Register the --run-perf command line option."""
    parser.addoption(
        "--run-perf", action="store_true", default=False, help="run performance benchmark tests"
    )

def pytest_configure(config):
    """Register custom markers."""
    config.addinivalue_line("markers", "benchmark: mark test as a performance benchmark")

def pytest_collection_modifyitems(config, items):
    """
    Skip tests marked with @pytest.mark.benchmark unless --run-perf is provided.
    """
    if config.getoption("--run-perf"):
        # If the flag is present, run everything (including benchmarks)
        return

    skip_benchmark = pytest.mark.skip(reason="need --run-perf option to run")
    for item in items:
        if "benchmark" in item.keywords:
            item.add_marker(skip_benchmark)