use pyo3::prelude::*;

/// Register logging functions into the Python module.
///
/// These call directly into the `tracing` macros used by the Rust SDK.
/// For host plugins, tracing is wired to the same subscriber the host
/// runtime sets up, so Python log calls appear identically to Rust ones.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(trace, m)?)?;
    m.add_function(wrap_pyfunction!(debug, m)?)?;
    m.add_function(wrap_pyfunction!(info, m)?)?;
    m.add_function(wrap_pyfunction!(warn, m)?)?;
    m.add_function(wrap_pyfunction!(error, m)?)?;
    Ok(())
}

#[pyfunction]
fn trace(message: &str) {
    tracing::trace!(target: "python", "{}", message);
}

#[pyfunction]
fn debug(message: &str) {
    tracing::debug!(target: "python", "{}", message);
}

#[pyfunction]
fn info(message: &str) {
    tracing::info!(target: "python", "{}", message);
}

#[pyfunction]
fn warn(message: &str) {
    tracing::warn!(target: "python", "{}", message);
}

#[pyfunction]
fn error(message: &str) {
    tracing::error!(target: "python", "{}", message);
}
