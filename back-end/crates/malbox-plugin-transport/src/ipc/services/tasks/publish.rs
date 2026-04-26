//! IPC publishers for task request and result channels.

use super::{IpcTaskResultHeader, result_notify_service_name, result_service_name};
use crate::error::{Result, TransportError};

use iceoryx2::port::notifier::Notifier;
use iceoryx2::port::publisher::Publisher;
use iceoryx2::prelude::*;

const MAX_REQUEST_BYTES: usize = 64 * 1024;
const MAX_RESULT_BYTES: usize = 1024 * 1024;

/// Publishes serialized task request bytes to a per-plugin IPC channel.
///
/// Used by the daemon to send `TaskRequest` payloads to host plugins.
pub struct TaskRequestPublisher {
    publisher: Publisher<iceoryx2::service::ipc_threadsafe::Service, [u8], ()>,
}

impl TaskRequestPublisher {
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

        let publisher = service
            .publisher_builder()
            .initial_max_slice_len(MAX_REQUEST_BYTES)
            .create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        Ok(Self { publisher })
    }

    /// Publish serialized task request bytes.
    pub fn publish(&self, data: &[u8]) -> Result<()> {
        let sample = self
            .publisher
            .loan_slice_uninit(data.len())
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        let sample = sample.write_from_fn(|idx| data[idx]);
        sample
            .send()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        Ok(())
    }
}

/// Publishes task result messages to a per-plugin IPC channel and notifies.
///
/// Used by host plugins to stream results back to the daemon. Each message is
/// an `IpcTaskResultHeader` followed by protobuf payload bytes. A notification
/// is sent after each publish so the daemon can wake from its listener.
pub struct TaskResultPublisher {
    publisher: Publisher<iceoryx2::service::ipc_threadsafe::Service, [u8], ()>,
    notifier: Notifier<iceoryx2::service::ipc_threadsafe::Service>,
}

impl TaskResultPublisher {
    pub fn new(
        node: &Node<iceoryx2::service::ipc_threadsafe::Service>,
        plugin_id: &str,
    ) -> Result<Self> {
        let data_name = result_service_name(plugin_id);
        let notify_name = result_notify_service_name(plugin_id);

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

        let publisher = data_service
            .publisher_builder()
            .initial_max_slice_len(MAX_RESULT_BYTES)
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

        let notifier = event_service
            .notifier_builder()
            .create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        Ok(Self {
            publisher,
            notifier,
        })
    }

    /// Publish a result header + protobuf payload and notify the daemon.
    pub fn publish(&self, header: &IpcTaskResultHeader, payload: &[u8]) -> Result<()> {
        let total_len = IpcTaskResultHeader::SIZE + payload.len();

        let mut header_bytes = [0u8; IpcTaskResultHeader::SIZE];
        header.write_to(&mut header_bytes);

        let sample = self
            .publisher
            .loan_slice_uninit(total_len)
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        let sample = sample.write_from_fn(|idx| {
            if idx < IpcTaskResultHeader::SIZE {
                header_bytes[idx]
            } else {
                payload[idx - IpcTaskResultHeader::SIZE]
            }
        });

        sample
            .send()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        self.notifier
            .notify()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        Ok(())
    }

    /// Publish a final marker (header-only, no payload) and notify.
    pub fn publish_final(&self, task_id: i32) -> Result<()> {
        self.publish(&IpcTaskResultHeader::final_marker(task_id), &[])
    }
}
