/// Marker trait for state type.
pub trait StateType {}

pub enum Persistent {}
impl StateType for Persistent {}

pub enum Ephemeral {}
impl StateType for Ephemeral {}

pub enum Scoped {}
impl StateType for Scoped {}

/// Configuration for scoped plugins.
///
// NOTE: Might remove scoped plugin in favor of being able to
// configure plugin concurrency and task acquisition in general.
pub struct ScopeConfig {
    pub plugins: Vec<String>,    // TODO: Replace with actual plugin ID.
    pub task_types: Vec<String>, // Temporary!
}
