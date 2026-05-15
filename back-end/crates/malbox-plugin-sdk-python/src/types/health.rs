use malbox_plugin_sdk::health::HealthStatus;
use pyo3::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Severity {
    Healthy,
    Degraded,
    Unhealthy,
}

#[pyclass(name = "HealthStatus", module = "malbox_plugin_sdk")]
#[derive(Clone)]
pub struct PyHealthStatus {
    pub(crate) inner: HealthStatus,
    severity: Severity,
}

#[pymethods]
impl PyHealthStatus {
    #[staticmethod]
    #[pyo3(name = "Healthy")]
    fn healthy() -> Self {
        Self {
            inner: HealthStatus::ready(),
            severity: Severity::Healthy,
        }
    }

    #[staticmethod]
    #[pyo3(name = "Degraded")]
    fn degraded(reason: String) -> Self {
        Self {
            inner: HealthStatus::not_ready(reason),
            severity: Severity::Degraded,
        }
    }

    #[staticmethod]
    #[pyo3(name = "Unhealthy")]
    fn unhealthy(reason: String) -> Self {
        Self {
            inner: HealthStatus::not_ready(reason),
            severity: Severity::Unhealthy,
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
        match self.severity {
            Severity::Healthy => "HealthStatus.Healthy".to_string(),
            Severity::Degraded => {
                format!("HealthStatus.Degraded('{}')", self.inner.reason())
            }
            Severity::Unhealthy => {
                format!("HealthStatus.Unhealthy('{}')", self.inner.reason())
            }
        }
    }
}

impl PyHealthStatus {
    pub(crate) fn default_healthy() -> Self {
        Self {
            inner: HealthStatus::ready(),
            severity: Severity::Healthy,
        }
    }
}

impl From<PyHealthStatus> for HealthStatus {
    fn from(py: PyHealthStatus) -> Self {
        py.inner
    }
}
