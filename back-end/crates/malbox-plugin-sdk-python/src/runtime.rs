use crate::plugin::PythonPlugin;
use malbox_plugin_sdk::meta::{ExecutionContext, PluginMeta, PluginState};
use malbox_plugin_sdk::runtime::host::HostRuntime;
use pyo3::prelude::*;

/// Start the plugin runtime. Blocks until the daemon signals shutdown.
///
/// This is the main entry point for Python plugins. Call at the end of
/// your plugin script with your Plugin subclass instance.
#[pyfunction]
#[pyo3(signature = (plugin, name=None, version=None))]
pub fn run(
    py: Python<'_>,
    plugin: Py<PyAny>,
    name: Option<String>,
    version: Option<String>,
) -> PyResult<()> {
    let plugin_name = name.unwrap_or_else(|| {
        std::env::var("MALBOX_PLUGIN_NAME").unwrap_or_else(|_| "python-plugin".into())
    });
    let plugin_version = version.unwrap_or_else(|| "0.1.0".into());

    let meta = PluginMeta::new(plugin_name, plugin_version)
        .with_state(PluginState::Persistent)
        .with_execution(ExecutionContext::Parallel);

    malbox_plugin_sdk::internal::init_tracing("info", None);

    let python_plugin = PythonPlugin::new(plugin);

    // Detach from the interpreter (releasing the GIL) while the Rust runtime
    // runs the blocking event loop. PythonPlugin's trait methods re-attach
    // when dispatching back into Python handler code.
    py.detach(|| {
        let runtime = HostRuntime::new(python_plugin, meta, &[]).map_err(|e| {
            pyo3::exceptions::PyRuntimeError::new_err(format!("Failed to initialize runtime: {e}"))
        })?;

        runtime.run().map_err(|e| {
            pyo3::exceptions::PyRuntimeError::new_err(format!("Runtime error: {e}"))
        })?;

        Ok::<_, PyErr>(())
    })?;

    Ok(())
}
