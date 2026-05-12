//! System-wide events that plugins can subscribe to.
//!
//! These are data-carrying events dispatched by the daemon. Plugins declare
//! subscriptions at initialization.

/// A flat enumeration of all system-wide events.
///
/// Each variant carries its relevant data inline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A task has been created and queued.
    TaskCreated { task_id: i32 },
    /// A task is about to begin processing.
    TaskStarting { task_id: i32 },
    /// A task has finished processing.
    TaskCompleted { task_id: i32 },
    /// A task has failed processing.
    TaskFailed { task_id: i32 },
    /// A task has been canceled (e.g. due to worker shutdown).
    TaskCanceled { task_id: i32 },

    /// A plugin has started.
    PluginStarted { plugin_id: i32 },
    /// A plugin has stopped.
    PluginStopped { plugin_id: i32 },
    /// A plugin has produced a result. This is a lightweight signal only -
    /// actual result data is accessed lazily via the result pub/sub channel
    /// through the macro-generated handler's event wrapper.
    PluginResultAvailable { source: String, result_name: String },

    /// A sample has started.
    SampleStarted { sample_id: i32 },
    /// A sample has stopped.
    SampleStopped { sample_id: i32 },
    /// A sample has produced a result.
    SampleResultProduced { sample_id: i32 },

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
        let dbg = format!("{:?}", event);
        assert!(dbg.contains("TaskCreated"));
        assert!(dbg.contains("42"));
    }

    #[test]
    fn clone_and_eq() {
        let a = Event::PluginStarted { plugin_id: 7 };
        let b = a.clone();
        assert_eq!(a, b);

        let c = Event::PluginStopped { plugin_id: 7 };
        assert_ne!(a, c);

        let d = Event::DaemonShutdown;
        let e = Event::DaemonShutdown;
        assert_eq!(d, e);
    }
}
