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
#[derive(Debug)]
pub(crate) struct PyPlugin;

#[pymethods]
#[expect(
    clippy::unused_self,
    clippy::unnecessary_wraps,
    reason = "default Python handler methods: instance methods returning None/raising, overridden by subclasses"
)]
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
pub(crate) struct PythonPlugin {
    py_plugin: Py<PyAny>,
}

impl PythonPlugin {
    pub(crate) fn new(py_plugin: Py<PyAny>) -> Self {
        Self { py_plugin }
    }
}

/// Drive `result` to completion on a fresh event loop if the handler returned a coroutine.
fn maybe_await(py: Python<'_>, result: &Bound<'_, PyAny>) -> PyResult<()> {
    let asyncio = py.import("asyncio")?;
    let is_coro: bool = asyncio.call_method1("iscoroutine", (result,))?.extract()?;
    if is_coro {
        let event_loop = asyncio.call_method0("new_event_loop")?;
        let run_result = event_loop.call_method1("run_until_complete", (result,));
        event_loop.call_method0("close")?;
        run_result?;
    }
    Ok(())
}

impl Plugin for PythonPlugin {
    fn health_check(&self) -> HealthStatus {
        Python::attach(|py| {
            let result = self.py_plugin.call_method0(py, "health_check");
            match result {
                Ok(obj) => match obj.extract::<PyHealthStatus>(py) {
                    Ok(status) => status.inner,
                    Err(e) => {
                        tracing::warn!("health_check returned non-HealthStatus type: {e}");
                        HealthStatus::not_ready(format!("health_check type error: {e}"))
                    }
                },
                Err(e) => {
                    tracing::error!("health_check raised an exception: {e}");
                    HealthStatus::not_ready(format!("health_check exception: {e}"))
                }
            }
        })
    }
}

impl HostPlugin for PythonPlugin {
    fn on_task(&self, ctx: &Context) -> Result<()> {
        Python::attach(|py| {
            // SAFETY: `valid` is cleared right after the handler call returns,
            // while `ctx` is still borrowed, so the PyContext never dereferences
            // the pointer after `ctx` is gone.
            let (py_ctx, valid) = unsafe { PyContext::from_ref(ctx) };

            let result = self.py_plugin.call_method1(py, "on_task", (py_ctx,));
            valid.store(false, std::sync::atomic::Ordering::Release);

            let result = result?;
            maybe_await(py, result.bind(py))?;
            Ok(())
        })
        .map_err(|e: PyErr| SdkError::Plugin(Box::new(PythonError(e.to_string()))))
    }

    fn on_start(&self, config: HashMap<String, String>) -> Result<()> {
        Python::attach(|py| {
            let dict = PyDict::new(py);
            for (k, v) in &config {
                dict.set_item(k, v)?;
            }

            let result = self.py_plugin.call_method1(py, "on_start", (dict,))?;
            maybe_await(py, result.bind(py))?;
            Ok(())
        })
        .map_err(|e: PyErr| SdkError::Plugin(Box::new(PythonError(e.to_string()))))
    }

    fn on_stop(&self) -> Result<()> {
        Python::attach(|py| {
            let result = self.py_plugin.call_method0(py, "on_stop")?;
            maybe_await(py, result.bind(py))?;
            Ok(())
        })
        .map_err(|e: PyErr| SdkError::Plugin(Box::new(PythonError(e.to_string()))))
    }

    fn on_event(&self, event: Event) -> Result<()> {
        Python::attach(|py| {
            let py_event = PyEvent::from(event);

            let result = self.py_plugin.call_method1(py, "on_event", (py_event,))?;

            maybe_await(py, result.bind(py))?;
            Ok(())
        })
        .map_err(|e: PyErr| SdkError::Plugin(Box::new(PythonError(e.to_string()))))
    }
}

/// Wraps a Python exception string for use as a `std::error::Error`.
#[derive(Debug)]
struct PythonError(String);

impl std::fmt::Display for PythonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Python plugin error: {}", self.0)
    }
}

impl std::error::Error for PythonError {}
