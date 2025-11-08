//! FFI Facade: The main entry point for Python.
//! This file uses `pyo3` to define the `_core` Python
//! module and expose Rust structs and functions as Python objects.

use pyo3::prelude::*;

// --- Module Imports ---
mod computation;
mod graph;
mod graph_ffi;
mod solver;
mod type_system;
mod display;

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

    // 2. Add the wrapped classes.
    m.add_class::<graph_ffi::PyComputationGraph>()?;
    m.add_class::<graph_ffi::PyLedger>()?;

    Ok(())
}