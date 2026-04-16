//! Plugin metadata types: name, version, type, state, execution context.

/// Metadata about a plugin.
///
/// Generic fields (`name`, `version`, `description`, `authors`) are sourced
/// automatically from the crate's `Cargo.toml` via `env!()` macros so that
/// plugin authors don't have to duplicate them in the `#[malbox(…)]` attribute.
#[derive(Debug, Clone)]
pub struct PluginMeta {
    pub name: &'static str,
    pub version: &'static str,
    pub description: Option<&'static str>,
    pub authors: &'static str,
    pub plugin_type: PluginType,
    pub state: PluginState,
    pub execution: ExecutionContext,
}

/// How the plugin communicates with the daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginType {
    /// Runs on the daemon host, communicating over IPC.
    Host,
    /// Runs inside a guest VM/container, communicating over gRPC.
    Guest,
}

/// Lifecycle behavior of the plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginState {
    /// Stays running between tasks.
    Persistent,
    /// Spun up per task and torn down immediately after.
    Ephemeral,
    /// Lives for the duration of an analysis scope (e.g. a batch).
    Scoped,
}

/// Concurrency model for task execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionContext {
    /// Only one instance runs at a time across the entire daemon.
    Exclusive,
    /// Tasks are dispatched one at a time in order.
    Sequential,
    /// Multiple tasks may run concurrently.
    Parallel,
    /// No constraints on concurrency or ordering.
    Unrestricted,
}
