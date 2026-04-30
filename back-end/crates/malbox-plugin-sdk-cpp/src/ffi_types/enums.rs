/// Lifetime policy of a plugin instance.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MalboxPluginState {
    /// The plugin instance is kept alive across tasks.
    Persistent = 0,
    /// A new plugin instance is created for each task and discarded afterwards.
    Ephemeral = 1,
    /// The plugin instance lives for a user-defined scope.
    Scoped = 2,
}

/// Concurrency policy controlling how the scheduler dispatches tasks to a plugin.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MalboxExecutionContext {
    /// Only one task may run in the plugin at a time and no other plugins may
    /// run concurrently.
    Exclusive = 0,
    /// Tasks are dispatched to this plugin one at a time, sequentially.
    Sequential = 1,
    /// Multiple tasks may be dispatched to this plugin concurrently.
    Parallel = 2,
    /// No scheduling restrictions are applied.
    Unrestricted = 3,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem;

    #[test]
    fn test_enums_are_repr_u8() {
        assert_eq!(mem::size_of::<MalboxPluginState>(), 1);
        assert_eq!(mem::size_of::<MalboxExecutionContext>(), 1);
    }
}
