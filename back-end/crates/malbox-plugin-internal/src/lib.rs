//! Daemon-side plugin infrastructure: discovers plugins on disk, manages their
//! lifecycles, and talks to them over IPC (host) or gRPC (guest).

pub mod manager;
/// Plugin discovery, hot-reload watching, and immutable registry snapshots.
pub mod registry;
pub mod transport;
