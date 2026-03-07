/// How the plugin can be executed relative to other plugins
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionContext {
    /// Only one instance at a time, blocks other plugins
    Exclusive,
    /// One at a time, ordered execution
    Sequential,
    /// Multiple instances can run concurrently
    Parallel,
    /// No constraints
    Unrestricted,
}
