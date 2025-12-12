"""
Prism Finance: A Verifiable Calculation Engine.
"""
# Expose the primary user-facing classes from the graph module.
from .graph import Canvas, Var

# Define package metadata
__version__ = "0.3.1"

__all__ = ["Canvas", "Var"]