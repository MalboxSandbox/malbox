//! C-ABI event type definitions.
//!
//! All types here are `#[repr(C)]` and mirror the flat `Event` enum from
//! `malbox_plugin_transport::messages::events` so they can cross the C ABI
//! boundary.

/// Discriminant for `MalboxEvent`, matching the flat `Event` enum variants.
///
/// Value 7 is retired (formerly the result-chaining event); the remaining
/// values keep their numbers.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MalboxEventTag {
    /// A task was created; `id` is the task ID.
    TaskCreated = 0,
    /// A task is about to start running; `id` is the task ID.
    TaskStarting = 1,
    /// A task finished successfully; `id` is the task ID.
    TaskCompleted = 2,
    /// A task finished with an error; `id` is the task ID.
    TaskFailed = 3,
    /// A task was canceled before completion; `id` is the task ID.
    TaskCanceled = 4,
    /// A plugin process came up; `id` is the plugin ID.
    PluginStarted = 5,
    /// A plugin process shut down; `id` is the plugin ID.
    PluginStopped = 6,
    /// Analysis of a sample began; `id` is the sample ID.
    SampleStarted = 8,
    /// Analysis of a sample ended; `id` is the sample ID.
    SampleStopped = 9,
    /// A result was produced for a sample; `id` is the sample ID.
    SampleResultProduced = 10,
    /// The daemon is shutting down; `id` is unused.
    DaemonShutdown = 11,
    /// The daemon reloaded its configuration; `id` is unused.
    ConfigReloaded = 12,
}

/// A C-ABI representation of a system-wide event.
///
/// Each event carries at most one integer identifier (`id`):
/// - Task events: `id` is the `task_id`.
/// - Plugin events: `id` is the `plugin_id`.
/// - Sample events: `id` is the `sample_id`.
/// - Daemon events: `id` is unused (set to `0`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MalboxEvent {
    /// Which kind of event this is.
    pub tag: MalboxEventTag,
    /// Task, plugin or sample ID selected by `tag`; `0` for daemon events.
    pub id: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[expect(
        clippy::as_conversions,
        reason = "reading a #[repr(i32)] discriminant requires `as`"
    )]
    fn event_tag_values() {
        assert_eq!(MalboxEventTag::TaskCreated as i32, 0);
        assert_eq!(MalboxEventTag::TaskStarting as i32, 1);
        assert_eq!(MalboxEventTag::TaskCompleted as i32, 2);
        assert_eq!(MalboxEventTag::TaskFailed as i32, 3);
        assert_eq!(MalboxEventTag::TaskCanceled as i32, 4);
        assert_eq!(MalboxEventTag::PluginStarted as i32, 5);
        assert_eq!(MalboxEventTag::PluginStopped as i32, 6);
        assert_eq!(MalboxEventTag::SampleStarted as i32, 8);
        assert_eq!(MalboxEventTag::SampleStopped as i32, 9);
        assert_eq!(MalboxEventTag::SampleResultProduced as i32, 10);
        assert_eq!(MalboxEventTag::DaemonShutdown as i32, 11);
        assert_eq!(MalboxEventTag::ConfigReloaded as i32, 12);
    }

    #[test]
    fn event_struct_size() {
        // tag (i32) + id (i32)
        assert_eq!(std::mem::size_of::<MalboxEvent>(), 8);
    }
}
