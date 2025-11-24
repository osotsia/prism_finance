# The Receptionist (Bindings)

**Role**: External Interface.

This module handles the FFI (Foreign Function Interface) with Python via PyO3.

## Responsibilities
1.  **Data Marshaling**: Converting Python lists/floats into Rust `Vec<f64>`/`NodeId`.
2.  **API Surface**: Defines the `PyComputationGraph` class exposed to the `prism_finance` Python package.
3.  **Error Handling**: Translates internal Rust `ComputationError` or `ValidationError` types into Python `ValueError` or `RuntimeError` exceptions.