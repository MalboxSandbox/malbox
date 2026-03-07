//! Event services for IPC communication between daemon and plugins.

pub mod emit;
pub mod receive;

pub use emit::EventEmitter;
pub use receive::EventReceiver;

/// Service names for the daemon → plugin direction.
pub mod daemon_channel {
    /// Event notifications sent by the daemon.
    pub const EVENTS: &str = "malbox/events/daemon/signals";
    /// Event payloads sent by the daemon.
    pub const PAYLOADS: &str = "malbox/events/daemon/payloads";
}

/// Service names for the plugin → daemon direction.
pub mod plugin_channel {
    /// Event notifications sent by plugins.
    pub const EVENTS: &str = "malbox/events/plugin/signals";
    /// Event payloads sent by plugins.
    pub const PAYLOADS: &str = "malbox/events/plugin/payloads";
}
