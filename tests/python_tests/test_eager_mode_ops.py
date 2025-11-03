# tests/python_tests/test_eager_mode_ops.py

# Change this line:
# import prism
# to this:
import prism_finance

def test_rust_bridge_is_accessible():
    """
    A placeholder test to confirm the compiled Rust core
    is importable and callable via the Python wrapper.
    """
    version = prism_finance.rust_core_version()
    assert version == "0.1.0-alpha"
    assert isinstance(version, str)

def test_ci_pipeline_is_working():
    """
    A trivial test that will always pass, used to validate
    that the test runner is discovering and executing tests.
    """
    assert True