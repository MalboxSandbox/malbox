//! IPC receivers for task request and result channels.

use super::IpcTaskResultHeader;
use crate::error::{Result, TransportError};

use core::time::Duration;
use iceoryx2::port::listener::Listener;
use iceoryx2::port::subscriber::Subscriber;
use iceoryx2::prelude::*;

/// Subscribes to serialized task request bytes on a per-plugin IPC channel.
///
/// Used by host plugins to receive `TaskRequest` payloads from the daemon.
pub struct TaskRequestReceiver {
    subscriber: Subscriber<iceoryx2::service::ipc_threadsafe::Service, [u8], ()>,
}

impl TaskRequestReceiver {
    pub fn new(
        node: &Node<iceoryx2::service::ipc_threadsafe::Service>,
        plugin_id: &str,
    ) -> Result<Self> {
        let service_name = super::request_service_name(plugin_id);
        let service = node
            .service_builder(
                &service_name
                    .as_str()
                    .try_into()
                    .map_err(|e| TransportError::Ipc(Box::new(e)))?,
            )
            .publish_subscribe::<[u8]>()
            .open_or_create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        let subscriber = service
            .subscriber_builder()
            .create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        Ok(Self { subscriber })
    }

    /// Try to receive a pending task request without blocking.
    ///
    /// Returns the raw bytes (protobuf-serialized `TaskRequest`) or `None`
    /// if no request is pending.
    pub fn try_recv(&self) -> Result<Option<Vec<u8>>> {
        if let Some(sample) = self
            .subscriber
            .receive()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?
        {
            Ok(Some(sample.payload().to_vec()))
        } else {
            Ok(None)
        }
    }
}

/// Waits for and receives task result messages from a per-plugin IPC channel.
///
/// Used by the daemon to collect results during `execute_host_task`. Waits on
/// a listener for notifications, then reads the data sample containing the
/// `IpcTaskResultHeader` + protobuf payload.
pub struct TaskResultReceiver {
    subscriber: Subscriber<iceoryx2::service::ipc_threadsafe::Service, [u8], ()>,
    listener: Listener<iceoryx2::service::ipc_threadsafe::Service>,
}

impl TaskResultReceiver {
    pub fn new(
        node: &Node<iceoryx2::service::ipc_threadsafe::Service>,
        plugin_id: &str,
    ) -> Result<Self> {
        let data_name = super::result_service_name(plugin_id);
        let notify_name = super::result_notify_service_name(plugin_id);

        let data_service = node
            .service_builder(
                &data_name
                    .as_str()
                    .try_into()
                    .map_err(|e| TransportError::Ipc(Box::new(e)))?,
            )
            .publish_subscribe::<[u8]>()
            .open_or_create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        let subscriber = data_service
            .subscriber_builder()
            .create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        let event_service = node
            .service_builder(
                &notify_name
                    .as_str()
                    .try_into()
                    .map_err(|e| TransportError::Ipc(Box::new(e)))?,
            )
            .event()
            .open_or_create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        let listener = event_service
            .listener_builder()
            .create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        Ok(Self {
            subscriber,
            listener,
        })
    }

    /// Wait for the next result message with a timeout.
    ///
    /// Returns the parsed header and the raw protobuf payload bytes,
    /// or `None` if the timeout expires without a notification.
    pub fn wait(&self, timeout: Duration) -> Result<Option<(IpcTaskResultHeader, Vec<u8>)>> {
        if self
            .listener
            .timed_wait_one(timeout)
            .map_err(|e| TransportError::Ipc(Box::new(e)))?
            .is_some()
            && let Some(sample) = self
                .subscriber
                .receive()
                .map_err(|e| TransportError::Ipc(Box::new(e)))?
        {
            let buf = sample.payload();
            let header = IpcTaskResultHeader::read_from(buf).ok_or_else(|| {
                TransportError::Ipc(
                    format!(
                        "result message too short: {} bytes, need {}",
                        buf.len(),
                        IpcTaskResultHeader::SIZE
                    )
                    .into(),
                )
            })?;
            let payload = if buf.len() > IpcTaskResultHeader::SIZE {
                buf[IpcTaskResultHeader::SIZE..].to_vec()
            } else {
                Vec::new()
            };
            return Ok(Some((header, payload)));
        }
        Ok(None)
    }
}
