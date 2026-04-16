//! C-ABI event type definitions.
//!
//! All types here are `#[repr(C)]` and mirror the flat `Event` enum from
//! `malbox_plugin_transport::messages::events` so they can cross the C ABI
//! boundary.

/// Discriminant for `MalboxEvent`, matching the flat `Event` enum variants.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MalboxEventTag {
    // Task events
    TaskCreated = 0,
    TaskStarting = 1,
    TaskCompleted = 2,
    TaskFailed = 3,
    TaskCanceled = 4,
    // Plugin events
    PluginStarted = 5,
    PluginStopped = 6,
    PluginResultProduced = 7,
    // Sample events
    SampleStarted = 8,
    SampleStopped = 9,
    SampleResultProduced = 10,
    // Daemon events
    DaemonShutdown = 11,
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
    pub tag: MalboxEventTag,
    pub id: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_tag_values() {
        assert_eq!(MalboxEventTag::TaskCreated as i32, 0);
        assert_eq!(MalboxEventTag::TaskStarting as i32, 1);
        assert_eq!(MalboxEventTag::TaskCompleted as i32, 2);
        assert_eq!(MalboxEventTag::TaskFailed as i32, 3);
        assert_eq!(MalboxEventTag::TaskCanceled as i32, 4);
        assert_eq!(MalboxEventTag::PluginStarted as i32, 5);
        assert_eq!(MalboxEventTag::PluginStopped as i32, 6);
        assert_eq!(MalboxEventTag::PluginResultProduced as i32, 7);
        assert_eq!(MalboxEventTag::SampleStarted as i32, 8);
        assert_eq!(MalboxEventTag::SampleStopped as i32, 9);
        assert_eq!(MalboxEventTag::SampleResultProduced as i32, 10);
        assert_eq!(MalboxEventTag::DaemonShutdown as i32, 11);
        assert_eq!(MalboxEventTag::ConfigReloaded as i32, 12);
    }

    #[test]
    fn event_struct_size() {
        // tag (i32) + id (i32) = 8 bytes, no padding needed
        assert_eq!(std::mem::size_of::<MalboxEvent>(), 8);
    }
}
