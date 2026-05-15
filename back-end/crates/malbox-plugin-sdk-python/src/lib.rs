use pyo3::prelude::*;

mod log;
mod plugin;
mod runtime;
mod types;

/// The native extension module for the malbox plugin SDK.
#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<plugin::PyPlugin>()?;
    m.add_function(wrap_pyfunction!(runtime::run, m)?)?;
    log::register(m)?;
    types::register(m)?;
    Ok(())
}
