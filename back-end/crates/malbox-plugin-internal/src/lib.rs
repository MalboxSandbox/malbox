pub mod error;
pub mod manager;
pub mod plugin_types;
pub mod registry;

// Re-export main types for external use
pub use plugin_types::{ExecutionContext, ExecutionPolicy, GuestPlatform, HostIpc};
pub use registry::PluginRegistry;
