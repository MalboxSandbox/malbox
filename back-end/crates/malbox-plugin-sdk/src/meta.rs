//! Plugin metadata: identity, lifecycle behavior, and concurrency model.
//!
//! These types describe a plugin to the daemon. [`PluginMeta`] carries
//! the identity fields (name, version, etc.) while [`PluginState`] and
//! [`ExecutionContext`] control how the daemon manages the plugin's lifetime
//! and task scheduling.

/// Metadata about a plugin.
///
/// Constructed via the builder pattern: [`PluginMeta::new`] followed by
/// optional chained setters. Generic fields (`name`, `version`, `description`,
/// `authors`) are sourced automatically from `Cargo.toml` via `env!()` macros
/// so that plugin authors don't have to duplicate them.
#[derive(Debug, Clone, PartialEq)]
pub struct PluginMeta {
    name: String,
    version: String,
    description: String,
    authors: String,
    state: PluginState,
    execution: ExecutionContext,
}

impl PluginMeta {
    /// Create metadata with a plugin name and version. All other fields
    /// start at sensible defaults (ephemeral state, exclusive execution).
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

    /// Set a human-readable description of what the plugin does.
    pub fn with_description(mut self, d: impl Into<String>) -> Self {
        self.description = d.into();
        self
    }

    /// Set the plugin author(s), typically sourced from `Cargo.toml`.
    pub fn with_authors(mut self, a: impl Into<String>) -> Self {
        self.authors = a.into();
        self
    }

    /// Set the plugin's lifecycle behavior (persistent, ephemeral, or scoped).
    pub fn with_state(mut self, s: PluginState) -> Self {
        self.state = s;
        self
    }

    /// Set the concurrency model the daemon should use when dispatching tasks.
    pub fn with_execution(mut self, e: ExecutionContext) -> Self {
        self.execution = e;
        self
    }

    /// The plugin's unique identifier (typically the crate name).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The plugin's version string (typically from `Cargo.toml`).
    pub fn version(&self) -> &str {
        &self.version
    }

    /// A short description of what the plugin does. May be empty.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// The plugin author(s). May be empty.
    pub fn authors(&self) -> &str {
        &self.authors
    }

    /// How the daemon manages this plugin's lifetime between tasks.
    pub fn state(&self) -> PluginState {
        self.state
    }

    /// How the daemon schedules concurrent tasks for this plugin.
    pub fn execution(&self) -> ExecutionContext {
        self.execution
    }
}

/// Lifecycle behavior of the plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PluginState {
    /// Stays running between tasks.
    Persistent,
    /// Spun up per task and torn down immediately after.
    Ephemeral,
    /// Lives for the duration of an analysis scope (e.g. a batch).
    Scoped,
}

/// Concurrency model for task execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
