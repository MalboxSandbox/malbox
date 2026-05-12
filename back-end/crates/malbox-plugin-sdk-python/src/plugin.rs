use malbox_plugin_sdk::context::Context;
use malbox_plugin_sdk::error::{Result, SdkError};
use malbox_plugin_sdk::health::HealthStatus;
use malbox_plugin_sdk::plugin::{HostPlugin, Plugin};
use malbox_plugin_transport::messages::events::Event;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::collections::HashMap;

use crate::types::context::PyContext;
use crate::types::event::PyEvent;
use crate::types::health::PyHealthStatus;

/// Python base class for plugins. Users subclass this.
/// All handler methods have default no-op implementations.
#[pyclass(name = "Plugin", module = "malbox_plugin_sdk", subclass)]
pub struct PyPlugin;

#[pymethods]
impl PyPlugin {
    #[new]
    fn new() -> Self {
        Self
    }

    fn on_task(&self, _ctx: &PyContext) -> PyResult<()> {
        Ok(())
    }

    fn on_start(&self, _config: &Bound<'_, PyDict>) -> PyResult<()> {
        Ok(())
    }

    fn on_stop(&self) -> PyResult<()> {
        Ok(())
    }

    fn health_check(&self) -> PyHealthStatus {
        PyHealthStatus::default_healthy()
    }

    fn on_event(&self, _event: &PyEvent) -> PyResult<()> {
        Ok(())
    }
}

/// Adapter that wraps a Python Plugin subclass and implements the Rust Plugin trait.
pub struct PythonPlugin {
    py_plugin: Py<PyAny>,
}

impl PythonPlugin {
    pub fn new(py_plugin: Py<PyAny>) -> Self {
        Self { py_plugin }
    }

    /// If the result is a coroutine, drive it to completion with asyncio.
    fn maybe_await(&self, py: Python<'_>, result: &Bound<'_, PyAny>) -> PyResult<()> {
        // Check if it's a coroutine (has send method = coroutine protocol)
        if result.hasattr("send")? {
            let asyncio = py.import("asyncio")?;

            // Try to get or create an event loop
            let event_loop = match asyncio.call_method0("get_event_loop") {
                Ok(loop_obj) => {
                    // Check if the loop is closed
                    let is_closed: bool = loop_obj.call_method0("is_closed")?.extract()?;
                    if is_closed {
                        let new_loop = asyncio.call_method0("new_event_loop")?;
                        asyncio.call_method1("set_event_loop", (&new_loop,))?;
                        new_loop
                    } else {
                        loop_obj
                    }
                }
                Err(_) => {
                    let new_loop = asyncio.call_method0("new_event_loop")?;
                    asyncio.call_method1("set_event_loop", (&new_loop,))?;
                    new_loop
                }
            };

            event_loop.call_method1("run_until_complete", (result,))?;
        }
        Ok(())
    }
}

// SAFETY: Py<PyAny> is Send. All access to the Python object goes through
// Python::with_gil, which acquires the GIL before touching any Python state.
unsafe impl Send for PythonPlugin {}
unsafe impl Sync for PythonPlugin {}

impl Plugin for PythonPlugin {
    fn health_check(&self) -> HealthStatus {
        Python::with_gil(|py| {
            let result = self.py_plugin.call_method0(py, "health_check");
            match result {
                Ok(obj) => match obj.extract::<PyHealthStatus>(py) {
                    Ok(status) => status.inner,
                    Err(_) => HealthStatus::ready(),
                },
                Err(_) => HealthStatus::ready(),
            }
        })
    }
}

impl HostPlugin for PythonPlugin {
    fn on_task(&self, ctx: &Context) -> Result<()> {
        Python::with_gil(|py| {
            let py_ctx = unsafe { PyContext::from_ref(ctx) };

            let result = self.py_plugin.call_method1(py, "on_task", (py_ctx,))?;

            self.maybe_await(py, result.bind(py))?;
            Ok(())
        })
        .map_err(|e: PyErr| SdkError::Plugin(Box::new(PythonError(e.to_string()))))
    }

    fn on_start(&self, config: HashMap<String, String>) -> Result<()> {
        Python::with_gil(|py| {
            let dict = PyDict::new(py);
            for (k, v) in &config {
                dict.set_item(k, v)?;
            }

            let result = self.py_plugin.call_method1(py, "on_start", (dict,))?;
            self.maybe_await(py, result.bind(py))?;
            Ok(())
        })
        .map_err(|e: PyErr| SdkError::Plugin(Box::new(PythonError(e.to_string()))))
    }

    fn on_stop(&self) -> Result<()> {
        Python::with_gil(|py| {
            let result = self.py_plugin.call_method0(py, "on_stop")?;
            self.maybe_await(py, result.bind(py))?;
            Ok(())
        })
        .map_err(|e: PyErr| SdkError::Plugin(Box::new(PythonError(e.to_string()))))
    }

    fn on_event(&self, event: Event) -> Result<()> {
        Python::with_gil(|py| {
            let py_event = PyEvent::from(event);

            let result = self.py_plugin.call_method1(py, "on_event", (py_event,))?;

            self.maybe_await(py, result.bind(py))?;
            Ok(())
        })
        .map_err(|e: PyErr| SdkError::Plugin(Box::new(PythonError(e.to_string()))))
    }
}

/// Wraps a Python exception string for use as a std::error::Error.
#[derive(Debug)]
struct PythonError(String);

impl std::fmt::Display for PythonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Python plugin error: {}", self.0)
    }
}

impl std::error::Error for PythonError {}
