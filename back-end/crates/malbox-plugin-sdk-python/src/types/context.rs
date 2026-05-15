use malbox_plugin_sdk::context::Context;
use malbox_plugin_sdk::result::PluginResult;
use pyo3::prelude::*;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::event::PyEvent;
use super::result::PyPluginResult;

fn ctx_from_ptr(ptr: *const (), valid: &AtomicBool) -> PyResult<&'static Context> {
    if !valid.load(Ordering::Acquire) || ptr.is_null() {
        return Err(pyo3::exceptions::PyRuntimeError::new_err(
            "Context is no longer valid (used outside handler scope)",
        ));
    }
    // SAFETY: The pointer is valid for the duration of the handler call,
    // and we verified `valid` is still true above.
    Ok(unsafe { &*(ptr as *const Context) })
}

// ─── TaskInfo ──────────────────────────────────────────────────────────────

#[pyclass(name = "TaskInfo", module = "malbox_plugin_sdk")]
pub struct PyTaskInfo {
    ptr: *const (),
    valid: Arc<AtomicBool>,
}

unsafe impl Send for PyTaskInfo {}
unsafe impl Sync for PyTaskInfo {}

#[pymethods]
impl PyTaskInfo {
    #[getter]
    fn id(&self) -> PyResult<i32> {
        Ok(ctx_from_ptr(self.ptr, &self.valid)?.task().id())
    }

    #[getter]
    fn sample_path(&self) -> PyResult<PathBuf> {
        Ok(ctx_from_ptr(self.ptr, &self.valid)?
            .task()
            .sample_path()
            .to_path_buf())
    }

    #[getter]
    fn config(&self) -> PyResult<std::collections::HashMap<String, String>> {
        Ok(ctx_from_ptr(self.ptr, &self.valid)?.task().config().clone())
    }

    fn sample_bytes(&self) -> PyResult<Vec<u8>> {
        ctx_from_ptr(self.ptr, &self.valid)?
            .task()
            .sample_bytes()
            .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))
    }

    fn config_value(&self, key: &str) -> PyResult<Option<String>> {
        Ok(ctx_from_ptr(self.ptr, &self.valid)?
            .task()
            .config_value(key)
            .map(String::from))
    }

    fn __repr__(&self) -> PyResult<String> {
        let id = ctx_from_ptr(self.ptr, &self.valid)?.task().id();
        Ok(format!("TaskInfo(id={id})"))
    }
}

// ─── ResultSink ────────────────────────────────────────────────────────────

#[pyclass(name = "ResultSink", module = "malbox_plugin_sdk")]
pub struct PyResultSink {
    ptr: *const (),
    valid: Arc<AtomicBool>,
}

unsafe impl Send for PyResultSink {}
unsafe impl Sync for PyResultSink {}

#[pymethods]
impl PyResultSink {
    fn push(&self, result: Bound<'_, PyPluginResult>) -> PyResult<()> {
        let inner = std::mem::replace(
            &mut result.borrow_mut().inner,
            PluginResult::bytes("", vec![]),
        );
        ctx_from_ptr(self.ptr, &self.valid)?
            .results()
            .push(inner)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn push_json(&self, name: &str, data: &[u8]) -> PyResult<()> {
        ctx_from_ptr(self.ptr, &self.valid)?
            .results()
            .push_json(name, data)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn push_bytes(&self, name: &str, data: &[u8]) -> PyResult<()> {
        ctx_from_ptr(self.ptr, &self.valid)?
            .results()
            .push_bytes(name, data)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn push_file(&self, name: &str, path: PathBuf) -> PyResult<()> {
        ctx_from_ptr(self.ptr, &self.valid)?
            .results()
            .push_file(name, &path)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn push_all(&self, results: Vec<Bound<'_, PyPluginResult>>) -> PyResult<()> {
        let inner: Vec<PluginResult> = results
            .into_iter()
            .map(|r| std::mem::replace(&mut r.borrow_mut().inner, PluginResult::bytes("", vec![])))
            .collect();
        ctx_from_ptr(self.ptr, &self.valid)?
            .results()
            .push_all(inner)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }
}

// ─── Context ───────────────────────────────────────────────────────────────

#[pyclass(name = "Context", module = "malbox_plugin_sdk")]
pub struct PyContext {
    ptr: *const (),
    valid: Arc<AtomicBool>,
}

unsafe impl Send for PyContext {}
unsafe impl Sync for PyContext {}

impl PyContext {
    /// Create a `PyContext` from a reference to a Rust `Context`.
    ///
    /// Returns the context and a validity guard. The caller must call
    /// [`PyContext::invalidate`] (via the returned `Arc`) when the
    /// underlying `Context` is no longer valid.
    ///
    /// # Safety
    /// The caller must ensure the Context outlives this PyContext
    /// (i.e. `invalidate` is called before the Context is dropped).
    pub(crate) unsafe fn from_ref(ctx: &Context) -> (Self, Arc<AtomicBool>) {
        let valid = Arc::new(AtomicBool::new(true));
        let py_ctx = Self {
            ptr: ctx as *const Context as *const (),
            valid: Arc::clone(&valid),
        };
        (py_ctx, valid)
    }
}

#[pymethods]
impl PyContext {
    fn task(&self) -> PyTaskInfo {
        PyTaskInfo {
            ptr: self.ptr,
            valid: Arc::clone(&self.valid),
        }
    }

    fn results(&self) -> PyResultSink {
        PyResultSink {
            ptr: self.ptr,
            valid: Arc::clone(&self.valid),
        }
    }

    fn progress(&self, pct: f64, message: &str) -> PyResult<()> {
        ctx_from_ptr(self.ptr, &self.valid)?
            .progress(pct, message)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn emit_event(&self, event: PyEvent) -> PyResult<()> {
        ctx_from_ptr(self.ptr, &self.valid)?
            .emit_event(event.inner)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn warn(&self, message: &str) -> PyResult<()> {
        ctx_from_ptr(self.ptr, &self.valid)?
            .warn(message)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn mark_collected(&self, path: PathBuf) -> PyResult<()> {
        ctx_from_ptr(self.ptr, &self.valid)?.mark_collected(path);
        Ok(())
    }
}
