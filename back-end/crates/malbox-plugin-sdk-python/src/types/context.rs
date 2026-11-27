use malbox_plugin_sdk::context::Context;
use malbox_plugin_sdk::result::PluginResult;
use malbox_plugin_sdk::{ArtifactOpts, ArtifactRelation, ArtifactSource, SdkError};
use pyo3::prelude::*;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::event::PyEvent;
use super::result::PyPluginResult;

pyo3::create_exception!(
    malbox_plugin_sdk,
    ArtifactLimitError,
    pyo3::exceptions::PyRuntimeError,
    "Raised when a job has pushed as many analysed artifacts as it may."
);

/// Convert a JSON value into a Python object via the stdlib `json` module.
fn json_to_py(py: Python<'_>, value: &serde_json::Value) -> PyResult<Py<PyAny>> {
    let text = serde_json::to_string(value)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    Ok(py.import("json")?.call_method1("loads", (text,))?.unbind())
}

/// Resolve an artifact relation label (`root` is reserved and rejected).
fn parse_relation(relation: &str) -> PyResult<ArtifactRelation> {
    relation
        .parse::<ArtifactRelation>()
        .ok()
        .filter(|r| *r != ArtifactRelation::Root)
        .ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err(format!(
                "unknown artifact relation: {relation}"
            ))
        })
}

/// Validate `ResultSink.push_artifact` arguments without a live context.
#[pyfunction]
#[pyo3(signature = (data, path, relation))]
pub fn _validate_artifact_args(
    data: Option<&[u8]>,
    path: Option<PathBuf>,
    relation: &str,
) -> PyResult<()> {
    if data.is_some() == path.is_some() {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "exactly one of data or path is required",
        ));
    }
    parse_relation(relation).map(|_| ())
}

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

    #[getter]
    fn job_id(&self) -> PyResult<Option<i64>> {
        Ok(ctx_from_ptr(self.ptr, &self.valid)?.task().job_id())
    }

    #[getter]
    fn artifact(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        let Some(a) = ctx_from_ptr(self.ptr, &self.valid)?.task().artifact() else {
            return Ok(None);
        };
        let value = serde_json::json!({
            "id": a.id,
            "sha256": a.sha256,
            "artifact_type": a.artifact_type,
            "name": a.name,
            "depth": a.depth,
        });
        json_to_py(py, &value).map(Some)
    }

    #[getter]
    fn deep_scan(&self) -> PyResult<bool> {
        Ok(ctx_from_ptr(self.ptr, &self.valid)?.task().deep_scan())
    }

    #[getter]
    fn ancestry(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let entries: Vec<serde_json::Value> = ctx_from_ptr(self.ptr, &self.valid)?
            .task()
            .ancestry()
            .into_iter()
            .map(|e| {
                serde_json::json!({
                    "sha256": e.sha256,
                    "artifact_type": e.artifact_type,
                    "relation": e.relation,
                })
            })
            .collect();
        json_to_py(py, &serde_json::Value::Array(entries))
    }

    #[getter]
    fn shared(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let map = ctx_from_ptr(self.ptr, &self.valid)?.task().shared();
        json_to_py(py, &serde_json::Value::Object(map))
    }

    #[getter]
    fn params(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let map = ctx_from_ptr(self.ptr, &self.valid)?.task().params();
        json_to_py(py, &serde_json::Value::Object(map))
    }

    #[getter]
    fn indicators(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let indicators = ctx_from_ptr(self.ptr, &self.valid)?.task().indicators();
        let value = serde_json::to_value(indicators)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        json_to_py(py, &value)
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

    #[pyo3(signature = (name, data=None, path=None, relation="extracted", analyze=None, allow_dynamic_recursion=false))]
    fn push_artifact(
        &self,
        name: &str,
        data: Option<Vec<u8>>,
        path: Option<PathBuf>,
        relation: &str,
        analyze: Option<bool>,
        allow_dynamic_recursion: bool,
    ) -> PyResult<()> {
        _validate_artifact_args(data.as_deref(), path.clone(), relation)?;
        let mut opts = ArtifactOpts::new(parse_relation(relation)?)
            .allow_dynamic_recursion(allow_dynamic_recursion);
        if let Some(v) = analyze {
            opts = opts.analyze(v);
        }
        let source = match (data, path) {
            (Some(bytes), None) => ArtifactSource::Bytes(bytes),
            (None, Some(p)) => ArtifactSource::File(p),
            _ => unreachable!("validated by _validate_artifact_args"),
        };
        ctx_from_ptr(self.ptr, &self.valid)?
            .results()
            .push_artifact(name, source, opts)
            .map_err(|e| match e {
                SdkError::ArtifactLimit { .. } => ArtifactLimitError::new_err(e.to_string()),
                other => pyo3::exceptions::PyRuntimeError::new_err(other.to_string()),
            })
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

    fn shared_put(&self, py: Python<'_>, key: &str, value: Bound<'_, PyAny>) -> PyResult<()> {
        let text: String = py
            .import("json")?
            .call_method1("dumps", (value,))?
            .extract()?;
        let parsed: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        ctx_from_ptr(self.ptr, &self.valid)?
            .shared_put(key, parsed)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn drop_artifact(&self) -> PyResult<()> {
        ctx_from_ptr(self.ptr, &self.valid)?
            .drop_artifact()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }
}
