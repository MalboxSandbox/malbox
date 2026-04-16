//! System-wide events that plugins can subscribe to.
//!
//! These are data-carrying events dispatched by the daemon. Plugins declare
//! subscriptions at initialization.

#[cfg(feature = "ipc")]
use crate::error::{Result, TransportError};

#[cfg(feature = "ipc")]
use iceoryx2::prelude::ZeroCopySend;

/// A flat enumeration of all system-wide events.
///
/// Each variant carries its relevant data inline. This replaces the previous
/// nested `Event` + category enum + `Payload` design with a single type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    // ── Task events ──────────────────────────────────────────────────
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

    // ── Plugin events ────────────────────────────────────────────────
    /// A plugin has started.
    PluginStarted { plugin_id: i32 },
    /// A plugin has stopped.
    PluginStopped { plugin_id: i32 },
    /// A plugin has produced a result.
    PluginResultProduced { plugin_id: i32 },

    // ── Sample events ────────────────────────────────────────────────
    /// A sample has started.
    SampleStarted { sample_id: i32 },
    /// A sample has stopped.
    SampleStopped { sample_id: i32 },
    /// A sample has produced a result.
    SampleResultProduced { sample_id: i32 },

    // ── Daemon events ────────────────────────────────────────────────
    /// The daemon is shutting down.
    DaemonShutdown,
    /// Configuration has been reloaded.
    ConfigReloaded,
}

/// IPC wire-format payload for shared-memory transport.
///
/// This is the data sent alongside the event notification ID over iceoryx2.
/// It carries only the integer identifier relevant to the event category.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(feature = "ipc", derive(ZeroCopySend))]
pub struct IpcPayload {
    pub id: i32,
}

#[cfg(feature = "ipc")]
impl IpcPayload {
    /// Extract the relevant ID from an [`Event`] into an IPC payload.
    pub fn from_event(event: &Event) -> Self {
        let id = match event {
            Event::TaskCreated { task_id }
            | Event::TaskStarting { task_id }
            | Event::TaskCompleted { task_id }
            | Event::TaskFailed { task_id }
            | Event::TaskCanceled { task_id } => *task_id,

            Event::PluginStarted { plugin_id }
            | Event::PluginStopped { plugin_id }
            | Event::PluginResultProduced { plugin_id } => *plugin_id,

            Event::SampleStarted { sample_id }
            | Event::SampleStopped { sample_id }
            | Event::SampleResultProduced { sample_id } => *sample_id,

            Event::DaemonShutdown | Event::ConfigReloaded => 0,
        };
        Self { id }
    }
}

#[cfg(feature = "ipc")]
impl Event {
    /// Convert to a unique sequential event ID (0..=12) for the iceoryx2 notifier.
    pub fn event_id(&self) -> usize {
        match self {
            Event::TaskCreated { .. } => 0,
            Event::TaskStarting { .. } => 1,
            Event::TaskCompleted { .. } => 2,
            Event::TaskFailed { .. } => 3,
            Event::TaskCanceled { .. } => 4,
            Event::PluginStarted { .. } => 5,
            Event::PluginStopped { .. } => 6,
            Event::PluginResultProduced { .. } => 7,
            Event::SampleStarted { .. } => 8,
            Event::SampleStopped { .. } => 9,
            Event::SampleResultProduced { .. } => 10,
            Event::DaemonShutdown => 11,
            Event::ConfigReloaded => 12,
        }
    }

    /// Reconstruct an [`Event`] from its IPC wire representation.
    ///
    /// `id` is the sequential event ID (0..=12) and `payload` carries the
    /// associated integer identifier.
    pub fn from_id_and_payload(id: usize, payload: &IpcPayload) -> Result<Self> {
        match id {
            0 => Ok(Event::TaskCreated {
                task_id: payload.id,
            }),
            1 => Ok(Event::TaskStarting {
                task_id: payload.id,
            }),
            2 => Ok(Event::TaskCompleted {
                task_id: payload.id,
            }),
            3 => Ok(Event::TaskFailed {
                task_id: payload.id,
            }),
            4 => Ok(Event::TaskCanceled {
                task_id: payload.id,
            }),
            5 => Ok(Event::PluginStarted {
                plugin_id: payload.id,
            }),
            6 => Ok(Event::PluginStopped {
                plugin_id: payload.id,
            }),
            7 => Ok(Event::PluginResultProduced {
                plugin_id: payload.id,
            }),
            8 => Ok(Event::SampleStarted {
                sample_id: payload.id,
            }),
            9 => Ok(Event::SampleStopped {
                sample_id: payload.id,
            }),
            10 => Ok(Event::SampleResultProduced {
                sample_id: payload.id,
            }),
            11 => Ok(Event::DaemonShutdown),
            12 => Ok(Event::ConfigReloaded),
            _ => Err(TransportError::InvalidEventId(id)),
        }
    }

    /// Backward-compatible helper: reconstruct an [`Event`] from just its ID,
    /// using a default (zero) payload.
    pub fn from_id(id: usize) -> Result<Self> {
        Self::from_id_and_payload(id, &IpcPayload::default())
    }
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

    #[test]
    #[cfg(feature = "ipc")]
    fn ipc_roundtrip() {
        let events = [
            Event::TaskCreated { task_id: 1 },
            Event::TaskStarting { task_id: 2 },
            Event::TaskCompleted { task_id: 3 },
            Event::TaskFailed { task_id: 4 },
            Event::TaskCanceled { task_id: 5 },
            Event::PluginStarted { plugin_id: 10 },
            Event::PluginStopped { plugin_id: 11 },
            Event::PluginResultProduced { plugin_id: 12 },
            Event::SampleStarted { sample_id: 20 },
            Event::SampleStopped { sample_id: 21 },
            Event::SampleResultProduced { sample_id: 22 },
            Event::DaemonShutdown,
            Event::ConfigReloaded,
        ];

        for (expected_id, event) in events.iter().enumerate() {
            let id = event.event_id();
            assert_eq!(id, expected_id, "event_id mismatch for {:?}", event);

            let payload = IpcPayload::from_event(event);
            let roundtripped = Event::from_id_and_payload(id, &payload).unwrap();
            assert_eq!(*event, roundtripped, "roundtrip mismatch for id {}", id);
        }
    }

    #[test]
    #[cfg(feature = "ipc")]
    fn invalid_event_id() {
        assert!(Event::from_id(13).is_err());
        assert!(Event::from_id(100).is_err());
        assert!(Event::from_id(usize::MAX).is_err());
    }

    #[test]
    #[cfg(feature = "ipc")]
    fn from_id_uses_zero_payload() {
        let event = Event::from_id(0).unwrap();
        assert_eq!(event, Event::TaskCreated { task_id: 0 });

        let event = Event::from_id(11).unwrap();
        assert_eq!(event, Event::DaemonShutdown);
    }

    #[test]
    fn ipc_payload_default() {
        let p = IpcPayload::default();
        assert_eq!(p.id, 0);
    }
}
