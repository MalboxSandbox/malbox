//! System-wide events that plugins can subscribe to.
//!
//! These are data-carrying events dispatched by the daemon. Plugins declare
//! subscriptions at initialization.

/// A flat enumeration of all system-wide events.
///
/// Each variant carries its relevant data inline.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::exhaustive_enums,
    reason = "matched exhaustively by malbox-plugin-sdk-cpp and malbox-plugin-sdk-python FFI conversions"
)]
pub enum Event {
    /// A task has been created and queued.
    TaskCreated {
        /// Database id of the task.
        task_id: i32,
    },
    /// A task is about to begin processing.
    TaskStarting {
        /// Database id of the task.
        task_id: i32,
    },
    /// A task has finished processing.
    TaskCompleted {
        /// Database id of the task.
        task_id: i32,
    },
    /// A task has failed processing.
    TaskFailed {
        /// Database id of the task.
        task_id: i32,
    },
    /// A task has been canceled (e.g. due to worker shutdown).
    TaskCanceled {
        /// Database id of the task.
        task_id: i32,
    },

    /// A plugin has started.
    PluginStarted {
        /// Database id of the plugin.
        plugin_id: i32,
    },
    /// A plugin has stopped.
    PluginStopped {
        /// Database id of the plugin.
        plugin_id: i32,
    },

    /// A sample has started.
    SampleStarted {
        /// Database id of the sample.
        sample_id: i32,
    },
    /// A sample has stopped.
    SampleStopped {
        /// Database id of the sample.
        sample_id: i32,
    },
    /// A sample has produced a result.
    SampleResultProduced {
        /// Database id of the sample.
        sample_id: i32,
    },

    /// The daemon is shutting down.
    DaemonShutdown,
    /// Configuration has been reloaded.
    ConfigReloaded,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_format() {
        let event = Event::TaskCreated { task_id: 42 };
        let dbg = format!("{event:?}");
        assert!(dbg.contains("TaskCreated"));
        assert!(dbg.contains("42"));
    }

    #[test]
    fn clone_and_eq() {
        let started = Event::PluginStarted { plugin_id: 7 };
        let cloned = started.clone();
        assert_eq!(started, cloned);

        let stopped = Event::PluginStopped { plugin_id: 7 };
        assert_ne!(started, stopped);

        let shutdown_a = Event::DaemonShutdown;
        let shutdown_b = Event::DaemonShutdown;
        assert_eq!(shutdown_a, shutdown_b);
    }
}
