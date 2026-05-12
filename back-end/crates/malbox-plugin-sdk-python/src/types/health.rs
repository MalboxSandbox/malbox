use malbox_plugin_sdk::health::HealthStatus;
use pyo3::prelude::*;

#[pyclass(name = "HealthStatus", module = "malbox_plugin_sdk")]
#[derive(Clone)]
pub struct PyHealthStatus {
    pub(crate) inner: HealthStatus,
}

#[pymethods]
impl PyHealthStatus {
    #[staticmethod]
    #[pyo3(name = "Healthy")]
    fn healthy() -> Self {
        Self {
            inner: HealthStatus::ready(),
        }
    }

    #[staticmethod]
    #[pyo3(name = "Degraded")]
    fn degraded(reason: String) -> Self {
        Self {
            inner: HealthStatus::not_ready(reason),
        }
    }

    #[staticmethod]
    #[pyo3(name = "Unhealthy")]
    fn unhealthy(reason: String) -> Self {
        Self {
            inner: HealthStatus::not_ready(reason),
        }
    }

    #[getter]
    fn is_ready(&self) -> bool {
        self.inner.is_ready()
    }

    #[getter]
    fn reason(&self) -> &str {
        self.inner.reason()
    }

    fn __repr__(&self) -> String {
        if self.inner.is_ready() {
            "HealthStatus.Healthy".to_string()
        } else {
            format!("HealthStatus.Degraded('{}')", self.inner.reason())
        }
    }
}

impl PyHealthStatus {
    pub(crate) fn default_healthy() -> Self {
        Self {
            inner: HealthStatus::ready(),
        }
    }
}

impl From<PyHealthStatus> for HealthStatus {
    fn from(py: PyHealthStatus) -> Self {
        py.inner
    }
}
