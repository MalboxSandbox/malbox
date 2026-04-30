//! Plugin metadata types: name, version, state, execution context.

/// Metadata about a plugin.
///
/// Generic fields (`name`, `version`, `description`, `authors`) are sourced
/// automatically from the crate's `Cargo.toml` via `env!()` macros so that
/// plugin authors don't have to duplicate them in the `#[malbox(…)]` attribute.
#[derive(Debug, Clone)]
pub struct PluginMeta {
    pub name: String,
    pub version: String,
    pub description: String,
    pub authors: String,
    pub state: PluginState,
    pub execution: ExecutionContext,
}

impl PluginMeta {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            description: String::new(),
            authors: String::new(),
            state: PluginState::Ephemeral,
            execution: ExecutionContext::Exclusive,
        }
    }

    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = d.into();
        self
    }

    pub fn authors(mut self, a: impl Into<String>) -> Self {
        self.authors = a.into();
        self
    }

    pub fn state(mut self, s: PluginState) -> Self {
        self.state = s;
        self
    }

    pub fn execution(mut self, e: ExecutionContext) -> Self {
        self.execution = e;
        self
    }
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
