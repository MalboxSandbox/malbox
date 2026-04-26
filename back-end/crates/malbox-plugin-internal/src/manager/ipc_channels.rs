//! Daemon-side IPC task channels for a single host plugin.

use malbox_plugin_transport::ipc::tasks::{TaskRequestPublisher, TaskResultReceiver};
use malbox_plugin_transport::ipc::{IpcService, Node};

use crate::manager::error::Result;

/// Bundles the daemon-side IPC channels for task execution with a host plugin.
pub struct HostTaskChannels {
    pub request_publisher: TaskRequestPublisher,
    pub result_receiver: TaskResultReceiver,
}

impl HostTaskChannels {
    /// Open or create the per-plugin task channels.
    pub fn new(node: &Node<IpcService>, plugin_id: &str) -> Result<Self> {
        let request_publisher = TaskRequestPublisher::new(node, plugin_id)
            .map_err(crate::manager::error::ManagerError::Transport)?;

        let result_receiver = TaskResultReceiver::new(node, plugin_id)
            .map_err(crate::manager::error::ManagerError::Transport)?;

        Ok(Self {
            request_publisher,
            result_receiver,
        })
    }
}
