use malbox_plugin_transport::messages::events::Event;
use pyo3::prelude::*;

#[pyclass(name = "Event", module = "malbox_plugin_sdk")]
#[derive(Clone)]
pub struct PyEvent {
    pub(crate) inner: Event,
}

#[pymethods]
impl PyEvent {
    #[getter]
    fn kind(&self) -> &str {
        match &self.inner {
            Event::TaskCreated { .. } => "TaskCreated",
            Event::TaskStarting { .. } => "TaskStarting",
            Event::TaskCompleted { .. } => "TaskCompleted",
            Event::TaskFailed { .. } => "TaskFailed",
            Event::TaskCanceled { .. } => "TaskCanceled",
            Event::PluginStarted { .. } => "PluginStarted",
            Event::PluginStopped { .. } => "PluginStopped",
            Event::PluginResultAvailable { .. } => "PluginResultAvailable",
            Event::SampleStarted { .. } => "SampleStarted",
            Event::SampleStopped { .. } => "SampleStopped",
            Event::SampleResultProduced { .. } => "SampleResultProduced",
            Event::DaemonShutdown => "DaemonShutdown",
            Event::ConfigReloaded => "ConfigReloaded",
        }
    }

    #[getter]
    fn id(&self) -> Option<i32> {
        match &self.inner {
            Event::TaskCreated { task_id }
            | Event::TaskStarting { task_id }
            | Event::TaskCompleted { task_id }
            | Event::TaskFailed { task_id }
            | Event::TaskCanceled { task_id } => Some(*task_id),
            Event::PluginStarted { plugin_id } | Event::PluginStopped { plugin_id } => {
                Some(*plugin_id)
            }
            Event::PluginResultAvailable { .. } => None,
            Event::SampleStarted { sample_id }
            | Event::SampleStopped { sample_id }
            | Event::SampleResultProduced { sample_id } => Some(*sample_id),
            Event::DaemonShutdown | Event::ConfigReloaded => None,
        }
    }

    #[getter]
    fn source(&self) -> Option<&str> {
        match &self.inner {
            Event::PluginResultAvailable { source, .. } => Some(source.as_str()),
            _ => None,
        }
    }

    #[getter]
    fn result_name(&self) -> Option<&str> {
        match &self.inner {
            Event::PluginResultAvailable { result_name, .. } => Some(result_name.as_str()),
            _ => None,
        }
    }

    #[staticmethod]
    fn task_created(task_id: i32) -> Self {
        Self {
            inner: Event::TaskCreated { task_id },
        }
    }

    #[staticmethod]
    fn task_starting(task_id: i32) -> Self {
        Self {
            inner: Event::TaskStarting { task_id },
        }
    }

    #[staticmethod]
    fn task_completed(task_id: i32) -> Self {
        Self {
            inner: Event::TaskCompleted { task_id },
        }
    }

    #[staticmethod]
    fn task_failed(task_id: i32) -> Self {
        Self {
            inner: Event::TaskFailed { task_id },
        }
    }

    #[staticmethod]
    fn task_canceled(task_id: i32) -> Self {
        Self {
            inner: Event::TaskCanceled { task_id },
        }
    }

    #[staticmethod]
    fn plugin_started(plugin_id: i32) -> Self {
        Self {
            inner: Event::PluginStarted { plugin_id },
        }
    }

    #[staticmethod]
    fn plugin_stopped(plugin_id: i32) -> Self {
        Self {
            inner: Event::PluginStopped { plugin_id },
        }
    }

    #[staticmethod]
    fn daemon_shutdown() -> Self {
        Self {
            inner: Event::DaemonShutdown,
        }
    }

    #[staticmethod]
    fn config_reloaded() -> Self {
        Self {
            inner: Event::ConfigReloaded,
        }
    }

    #[staticmethod]
    fn plugin_result_available(source: String, result_name: String) -> Self {
        Self {
            inner: Event::PluginResultAvailable {
                source,
                result_name,
            },
        }
    }

    #[staticmethod]
    fn sample_started(sample_id: i32) -> Self {
        Self {
            inner: Event::SampleStarted { sample_id },
        }
    }

    #[staticmethod]
    fn sample_stopped(sample_id: i32) -> Self {
        Self {
            inner: Event::SampleStopped { sample_id },
        }
    }

    #[staticmethod]
    fn sample_result_produced(sample_id: i32) -> Self {
        Self {
            inner: Event::SampleResultProduced { sample_id },
        }
    }

    fn __repr__(&self) -> String {
        match self.id() {
            Some(id) => format!("Event(kind='{}', id={})", self.kind(), id),
            None => format!("Event(kind='{}')", self.kind()),
        }
    }
}

impl From<Event> for PyEvent {
    fn from(event: Event) -> Self {
        Self { inner: event }
    }
}

impl From<PyEvent> for Event {
    fn from(py: PyEvent) -> Self {
        py.inner
    }
}
