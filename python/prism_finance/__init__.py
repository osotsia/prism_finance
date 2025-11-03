# The Public-Facing API: This is the "front door" to the library.

# Here, we import the compiled objects from our Rust module (`_core`)
# and expose them directly under the `prism` namespace. This creates a
# seamless experience for the user, who doesn't need to know about the
# internal `_core` module.
# from ._core import Var, Canvas, Test

# For now, we just expose the test function.
from ._core import rust_core_version

# We can also define pure Python classes here if needed.
__all__ = [
    # "Var",
    # "Canvas",
    # "Test",
    "rust_core_version",
]
