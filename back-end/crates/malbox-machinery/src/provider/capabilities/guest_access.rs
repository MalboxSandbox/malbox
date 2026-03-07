//! Guest access capability: file transfer, command execution, and health checks on guest machines.
//!
//! This module defines:
//! - [`GuestSession`] — the uniform interface for interacting with guest VMs
//! - [`GuestAccess`] — the provider capability trait for opening native sessions
//! - Supporting types: [`ExecOptions`], [`ExecResult`], [`GuestStatus`]

use crate::machine::Machine;
use async_trait::async_trait;
use std::error::Error;
use std::path::Path;
use std::time::Duration;

// Re-export toml::Value for provider implementations
pub use toml::Value as TomlValue;

/// Options for guest command execution.
#[derive(Debug, Clone, Default)]
pub struct ExecOptions {
    /// Working directory inside the guest.
    pub cwd: Option<String>,
    /// Environment variables to set for the command.
    pub env: Vec<(String, String)>,
    /// Maximum time to wait for the command to complete.
    pub timeout: Option<Duration>,
    /// If `true`, start the command in the background and return immediately.
    pub background: bool,
}

/// Result of a guest command execution.
#[derive(Debug, Clone)]
pub struct ExecResult {
    /// Exit code of the process, or `None` if the process was killed / background.
    pub exit_code: Option<i32>,
    /// Standard output captured from the command.
    pub stdout: Vec<u8>,
    /// Standard error captured from the command.
    pub stderr: Vec<u8>,
}

/// Health status reported by a guest session.
#[derive(Debug, Clone)]
pub enum GuestStatus {
    /// The guest agent / transport is reachable and ready.
    Ready,
    /// The guest is not yet reachable.
    NotReady {
        /// Human-readable explanation of why the guest is not ready.
        reason: String,
    },
}

/// A live session to a guest machine.
///
/// Obtained from either a provider-native transport (via [`GuestAccess`]) or a
/// protocol-level transport (e.g., gRPC client). All methods are async and
/// the session is `Send + Sync` so it can be used across tasks.
#[async_trait]
pub trait GuestSession: Send + Sync {
    /// Push a file from the host filesystem into the guest.
    async fn push_file(
        &self,
        source: &Path,
        dest: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Pull a file from the guest into the host filesystem.
    async fn pull_file(
        &self,
        source: &str,
        dest: &Path,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Execute a command inside the guest.
    async fn execute(
        &self,
        command: &str,
        args: &[String],
        opts: ExecOptions,
    ) -> Result<ExecResult, Box<dyn Error + Send + Sync>>;

    /// Check whether the guest transport is reachable.
    async fn health_check(&self) -> Result<GuestStatus, Box<dyn Error + Send + Sync>>;

    /// Close the session and release any held resources.
    ///
    /// Takes ownership via `Box<Self>` so the session cannot be reused after closing.
    async fn close(self: Box<Self>) -> Result<(), Box<dyn Error + Send + Sync>>;
}

/// Provider-native guest access capability.
///
/// Providers that implement this can open guest sessions using transport
/// mechanisms specific to the provider. The `transport` parameter selects
/// which mechanism to use (e.g., "virtio-serial", "qemu-guest-agent").
///
/// A single provider may support multiple native transports. The user
/// selects which one via configuration.
#[async_trait]
pub trait GuestAccess: Send + Sync {
    /// List the transport methods this provider supports natively.
    ///
    /// Returns names like `["virtio-serial", "qemu-guest-agent"]`.
    fn supported_transports(&self) -> Vec<&str>;

    /// Open a guest session using the named transport.
    async fn open_session(
        &self,
        transport: &str,
        machine: &Machine,
        config: &TomlValue,
    ) -> Result<Box<dyn GuestSession>, Box<dyn Error + Send + Sync>>;
}
