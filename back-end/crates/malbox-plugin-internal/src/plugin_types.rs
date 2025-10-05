//! Plugin type definitions (stubs for missing malbox_plugin_utils).

use serde::{Deserialize, Serialize};

/// Guest platform type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GuestPlatform {
    Windows,
    Linux,
}

/// Plugin execution context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionContext {
    Host,
    Guest { platform: GuestPlatform },
}

impl std::fmt::Display for ExecutionContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionContext::Host => write!(f, "host"),
            ExecutionContext::Guest { .. } => write!(f, "guest"),
        }
    }
}

/// Plugin execution policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionPolicy {
    Exclusive,
    Sequential,
    Parallel,
    Unrestricted,
}

/// Host IPC communication stub.
pub struct HostIpc {
    // TODO: Implement proper IPC
}

impl HostIpc {
    pub fn new() -> Result<Self, crate::error::InternalError> {
        Ok(Self {})
    }
}
