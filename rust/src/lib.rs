use pyo3::prelude::*;

pub mod store;
pub mod analysis;
pub mod compute;
pub mod solver;
pub mod bindings;
pub mod display;

#[pyfunction]
fn rust_core_version() -> &'static str {
    "0.3.1"
}

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(rust_core_version, m)?)?;
    // Register the benchmark function
    m.add_function(wrap_pyfunction!(bindings::python::benchmark_pure_rust, m)?)?;
    
    m.add_class::<bindings::python::PyComputationGraph>()?;
    m.add_class::<bindings::python::PyLedger>()?;
    Ok(())
}