//! System-wide events that plugins can subscribe to.
//!
//! These are data-carrying events dispatched by the daemon. Plugins declare
//! subscriptions at initialization.

use crate::error::{Result, TransportError};

#[cfg(feature = "ipc")]
use iceoryx2::prelude::ZeroCopySend;

/// A wrapper around all system-wide events.
/// This is the event type that is used in our transport event services.
#[repr(C)]
#[derive(Debug)]
#[cfg_attr(feature = "ipc", derive(ZeroCopySend))]
pub enum Event {
    Task(TaskEvent),
    Plugin(PluginEvent),
    Sample(SampleEvent),
    Daemon(DaemonEvent),
}

/// Maximum number of variants per event category.
/// Must be >= the largest variant count across all categories.
#[cfg(feature = "ipc")]
const VARIANTS_PER_CATEGORY: usize = 4;

#[cfg(feature = "ipc")]
impl Event {
    /// Convert to a unique event ID for the iceoryx2 notifier.
    ///
    /// Encoding: `category * VARIANTS_PER_CATEGORY + variant`.
    /// Max value with 4 categories and 4 variants = 15.
    pub fn event_id(&self) -> usize {
        match self {
            Event::Task(t) => (0 * VARIANTS_PER_CATEGORY) + (*t as usize),
            Event::Plugin(p) => (1 * VARIANTS_PER_CATEGORY) + (*p as usize),
            Event::Sample(s) => (2 * VARIANTS_PER_CATEGORY) + (*s as usize),
            Event::Daemon(d) => (3 * VARIANTS_PER_CATEGORY) + (*d as usize),
        }
    }

    /// Convert from an event ID back to an Event.
    pub fn from_id(id: usize) -> Result<Self> {
        let category = id / VARIANTS_PER_CATEGORY;
        let variant = (id % VARIANTS_PER_CATEGORY) as u8;

        match category {
            0 => TaskEvent::from_u8(variant).map(Event::Task),
            1 => PluginEvent::from_u8(variant).map(Event::Plugin),
            2 => SampleEvent::from_u8(variant).map(Event::Sample),
            3 => DaemonEvent::from_u8(variant).map(Event::Daemon),
            _ => Err(TransportError::InvalidEventId(id)),
        }
    }
}

/// A wrapper around all system-wide event payloads.
/// This is the payload type that is used in our transport event services.
#[repr(C)]
#[derive(Debug, Clone)]
#[cfg_attr(feature = "ipc", derive(ZeroCopySend))]
pub enum Payload {
    Task(TaskEventPayload),
    Plugin(PluginEventPayload),
    Sample(SampleEventPayload),
}

/// An enumeration of all avaiable task-related system-wide events,
/// used for internal IPC event payloads.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "ipc", derive(ZeroCopySend))]
pub enum TaskEvent {
    /// A task has been created and queued.
    TaskCreated = 0,
    /// A task is about to begin processing.
    TaskStarting = 1,
    /// A task has finished processing.
    TaskCompleted = 2,
    /// A task has failed processing.
    TaskFailed = 3,
}

/// A payload representing the data that a plugin receives
/// after it has been signaled with a task event.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "ipc", derive(ZeroCopySend))]
pub struct TaskEventPayload {
    pub task_id: i32,
}

/// An enumeration of all avaiable plugin-related system-wide events,
/// used for internal IPC event payloads.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "ipc", derive(ZeroCopySend))]
pub enum PluginEvent {
    /// A plugin has started.
    PluginStarted = 0,
    /// A plugin has stopped.
    PluginStopped = 1,
    /// A plugin has produced a result.
    PluginResultProduced = 2,
}

/// A payload representing the data that a plugin receives
/// after it has been signaled with a plugin event.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "ipc", derive(ZeroCopySend))]
pub struct PluginEventPayload {
    pub plugin_id: i32,
}

/// An enumeration of all avaiable sample-related system-wide events,
/// used for internal IPC event payloads.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "ipc", derive(ZeroCopySend))]
pub enum SampleEvent {
    /// A plugin has started.
    PluginStarted = 0,
    /// A plugin has stopped.
    PluginStopped = 1,
    /// A plugin has produced a result.
    PluginResultProduced = 2,
}

/// A payload representing the data that a plugin receives
/// after it has been signaled with a sample event.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "ipc", derive(ZeroCopySend))]
pub struct SampleEventPayload {
    pub sample_id: i32,
}

/// An enumeration of all avaiable daemon-related system-wide events,
/// used for internal IPC event payloads.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "ipc", derive(ZeroCopySend))]
pub enum DaemonEvent {
    DaemonShutdown = 0,
    ConfigReloaded = 1,
}

// Conversion implementations for deserializing event IDs

impl TaskEvent {
    pub fn from_u8(v: u8) -> Result<Self> {
        match v {
            0 => Ok(Self::TaskCreated),
            1 => Ok(Self::TaskStarting),
            2 => Ok(Self::TaskCompleted),
            3 => Ok(Self::TaskFailed),
            _ => Err(TransportError::InvalidEventId(v as usize)),
        }
    }
}

impl PluginEvent {
    pub fn from_u8(v: u8) -> Result<Self> {
        match v {
            0 => Ok(Self::PluginStarted),
            1 => Ok(Self::PluginStopped),
            2 => Ok(Self::PluginResultProduced),
            _ => Err(TransportError::InvalidEventId(v as usize)),
        }
    }
}

impl SampleEvent {
    pub fn from_u8(v: u8) -> Result<Self> {
        match v {
            0 => Ok(Self::PluginStarted),
            1 => Ok(Self::PluginStopped),
            2 => Ok(Self::PluginResultProduced),
            _ => Err(TransportError::InvalidEventId(v as usize)),
        }
    }
}

impl DaemonEvent {
    pub fn from_u8(v: u8) -> Result<Self> {
        match v {
            0 => Ok(Self::DaemonShutdown),
            1 => Ok(Self::ConfigReloaded),
            _ => Err(TransportError::InvalidEventId(v as usize)),
        }
    }
}
