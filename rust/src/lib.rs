//! FFI Facade: The main entry point for Python.
//! This file uses `pyo3` to define the `_core` Python
//! module and expose Rust structs and functions as Python objects.

use pyo3::prelude::*;

// --- Module Imports ---
// This brings the core logic into scope.
mod graph;
// This brings the Python bindings into scope.
mod graph_ffi;

// --- Placeholder function to test the bridge ---
/// A simple function to confirm the Rust core is callable from Python.
#[pyfunction]
fn rust_core_version() -> &'static str {
    "0.1.0-alpha"
}

// --- Module Definition ---
/// This function defines the `prism_finance._core` Python module.
/// The name `_core` is chosen to indicate it's an internal, compiled component.
#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // 1. Add the placeholder version function.
    m.add_function(wrap_pyfunction!(rust_core_version, m)?)?;

    // 2. Add the wrapped ComputationGraph class.
    m.add_class::<graph_ffi::PyComputationGraph>()?;

    Ok(())
}