/// Whether the plugin runs on the daemon host or inside a guest VM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginKind {
    /// IPC-based plugin that runs directly on the daemon host.
    Host,
    /// gRPC-based plugin that runs inside a guest VM/container.
    Guest,
}
