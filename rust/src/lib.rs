// FFI Facade: The main entry point for Python.
// This file uses `pyo3` to define the `_core` Python
// module and expose Rust structs and functions as Python objects.

use pyo3::prelude::*;

// --- Placeholder function to test the bridge ---
/// A simple function to confirm the Rust core is callable from Python.
#[pyfunction]
fn rust_core_version() -> &'static str {
    "0.1.0-alpha"
}

// --- Module Definition ---
/// This function defines the `prism._core` Python module.
/// The name `_core` is chosen to indicate it's an internal, compiled component.
#[pymodule]
fn _core(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // 1. Create the wrapped function object.
    let version_fn = wrap_pyfunction!(rust_core_version, m)?;
    // 2. Add the function object to the module.
    m.add_function(version_fn)?;
    Ok(())
}
