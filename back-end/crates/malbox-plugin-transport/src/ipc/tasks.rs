//! Request/response task service for daemon -> plugin dispatch.
//!
//! Service name: `malbox/plugin/{plugin_id}/tasks`
//! Pattern: request_response::<[u8], [u8]>() with user headers on both sides.
//!
//! The daemon is the client, the plugin is the server. Supports streaming
//! multiple responses per request (chunked results, progress, final marker).

use crate::error::{Result, TransportError};

use super::daemon_notify::{DaemonNotifier, DaemonNotifyKind};
use super::headers::{TaskRequestHeader, TaskResponseHeader};

use iceoryx2::port::server::Server;
use iceoryx2::prelude::*;
use iceoryx2::service::port_factory::request_response::PortFactory;
use std::rc::Rc;

type IpcServiceType = iceoryx2::service::ipc::Service;

const MAX_REQUEST_SLICE_LEN: usize = 64 * 1024; // 64 KB
const CHUNK_SIZE: usize = 1024 * 1024; // 1 MB
const MAX_RESPONSE_BUFFER_SIZE: usize = 4;

fn task_service_name(plugin_id: &str) -> String {
    format!("malbox/plugin/{plugin_id}/tasks")
}

/// Daemon-side client for dispatching tasks to a plugin.
pub struct TaskClient {
    client: iceoryx2::port::client::Client<
        IpcServiceType,
        [u8],
        TaskRequestHeader,
        [u8],
        TaskResponseHeader,
    >,
}

impl TaskClient {
    pub fn new(node: &Node<IpcServiceType>, plugin_id: &str) -> Result<Self> {
        let service = open_or_create_task_service(node, plugin_id)?;

        let client = service
            .client_builder()
            .initial_max_slice_len(MAX_REQUEST_SLICE_LEN)
            .allocation_strategy(AllocationStrategy::PowerOfTwo)
            .create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        Ok(Self { client })
    }

    /// Send a task request. Returns a handle to receive streamed responses.
    ///
    /// `header` contains task_id and request_type.
    /// `payload` is postcard-serialized TaskRequestPayload.
    pub fn send(&self, header: &TaskRequestHeader, payload: &[u8]) -> Result<TaskPendingResponse> {
        let mut request = self
            .client
            .loan_slice_uninit(payload.len())
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        let user_header = request.user_header_mut();
        *user_header = *header;

        let request = request.write_from_fn(|i| payload[i]);
        let pending = request
            .send()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        Ok(TaskPendingResponse { pending })
    }
}

/// Handle for receiving streamed responses from a dispatched task.
pub struct TaskPendingResponse {
    pending: iceoryx2::pending_response::PendingResponse<
        IpcServiceType,
        [u8],
        TaskRequestHeader,
        [u8],
        TaskResponseHeader,
    >,
}

impl TaskPendingResponse {
    /// Non-blocking receive of the next response chunk.
    /// Returns None if no response is available yet.
    pub fn try_recv(&self) -> Result<Option<TaskResponse>> {
        match self
            .pending
            .receive()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?
        {
            Some(response) => {
                let header = *response.user_header();
                let payload = response.payload().to_vec();
                Ok(Some(TaskResponse { header, payload }))
            }
            None => Ok(None),
        }
    }

    /// Check if the server (plugin) is still connected.
    pub fn is_connected(&self) -> bool {
        self.pending.is_connected()
    }
}

/// A received response with header and payload data.
pub struct TaskResponse {
    pub header: TaskResponseHeader,
    pub payload: Vec<u8>,
}

/// Plugin-side server for receiving task requests.
pub struct TaskServer {
    server: Server<IpcServiceType, [u8], TaskRequestHeader, [u8], TaskResponseHeader>,
    /// Shared with every [`ActiveTaskRequest`] so each response chunk can
    /// wake the daemon (mandatory notify contract).
    daemon_notifier: Rc<DaemonNotifier>,
}

impl TaskServer {
    pub fn new(node: &Node<IpcServiceType>, plugin_id: &str) -> Result<Self> {
        let service = open_or_create_task_service(node, plugin_id)?;

        let server = service
            .server_builder()
            .initial_max_slice_len(CHUNK_SIZE)
            .allocation_strategy(AllocationStrategy::PowerOfTwo)
            .create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        let daemon_notifier = Rc::new(DaemonNotifier::new(node)?);

        Ok(Self {
            server,
            daemon_notifier,
        })
    }

    /// Non-blocking receive of a pending task request.
    pub fn receive(&self) -> Result<Option<ActiveTaskRequest>> {
        match self
            .server
            .receive()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?
        {
            Some(active_request) => Ok(Some(ActiveTaskRequest {
                active_request,
                daemon_notifier: Rc::clone(&self.daemon_notifier),
            })),
            None => Ok(None),
        }
    }
}

/// An active request the plugin is handling. Dropping this signals
/// to the client that no more responses will be sent.
pub struct ActiveTaskRequest {
    active_request: iceoryx2::active_request::ActiveRequest<
        IpcServiceType,
        [u8],
        TaskRequestHeader,
        [u8],
        TaskResponseHeader,
    >,
    daemon_notifier: Rc<DaemonNotifier>,
}

impl ActiveTaskRequest {
    /// Access the request header (task_id, request_type).
    pub fn header(&self) -> &TaskRequestHeader {
        self.active_request.user_header()
    }

    /// Access the request payload bytes (postcard-serialized).
    pub fn payload(&self) -> &[u8] {
        self.active_request.payload()
    }

    /// Check if the client is still interested in responses.
    pub fn is_connected(&self) -> bool {
        self.active_request.is_connected()
    }

    /// Send a response, automatically chunking payloads that exceed the
    /// IPC buffer capacity. Retries on transient `OutOfMemory` (buffer
    /// backpressure) with a bounded retry window.
    pub fn send_response(&self, header: &TaskResponseHeader, payload: &[u8]) -> Result<()> {
        if payload.len() <= CHUNK_SIZE {
            return self.send_response_single(header, payload);
        }

        let total_size = payload.len() as u64;
        let mut offset = 0usize;
        let mut chunk_index = 0u32;

        while offset < payload.len() {
            let end = (offset + CHUNK_SIZE).min(payload.len());
            let is_last_chunk = end == payload.len();

            let mut chunk_header = *header;
            chunk_header.chunk_index = chunk_index;
            chunk_header.total_size = if chunk_index == 0 { total_size } else { 0 };
            if is_last_chunk {
                chunk_header.flags &= !super::headers::FLAG_HAS_MORE_CHUNKS;
            } else {
                chunk_header.flags |= super::headers::FLAG_HAS_MORE_CHUNKS;
            }
            if chunk_index > 0 {
                chunk_header.name_len = 0;
            }

            self.send_response_single(&chunk_header, &payload[offset..end])?;

            offset = end;
            chunk_index += 1;
        }

        Ok(())
    }

    fn send_response_single(&self, header: &TaskResponseHeader, payload: &[u8]) -> Result<()> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);

        loop {
            match self.active_request.loan_slice_uninit(payload.len()) {
                Ok(mut response) => {
                    let user_header = response.user_header_mut();
                    *user_header = *header;
                    let response = response.write_from_fn(|i| payload[i]);
                    response
                        .send()
                        .map_err(|e| TransportError::Ipc(Box::new(e)))?;
                    // Mandatory notify contract: wake the daemon for every
                    // chunk so it keeps draining the bounded response buffer.
                    let _ = self.daemon_notifier.notify(DaemonNotifyKind::TaskResponse);
                    return Ok(());
                }
                Err(iceoryx2::port::LoanError::OutOfMemory) => {
                    if std::time::Instant::now() > deadline {
                        return Err(TransportError::BufferFull(format!(
                            "IPC response buffer full for 30s (payload {} bytes)",
                            payload.len()
                        )));
                    }
                    // Kick the daemon before backing off: the buffer only
                    // frees up once it drains.
                    let _ = self.daemon_notifier.notify(DaemonNotifyKind::TaskResponse);
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                Err(e) => {
                    return Err(TransportError::Ipc(Box::new(e)));
                }
            }
        }
    }
}

type TaskPortFactory =
    PortFactory<IpcServiceType, [u8], TaskRequestHeader, [u8], TaskResponseHeader>;

fn open_or_create_task_service(
    node: &Node<IpcServiceType>,
    plugin_id: &str,
) -> Result<TaskPortFactory> {
    let name = task_service_name(plugin_id);
    let service = node
        .service_builder(
            &name
                .as_str()
                .try_into()
                .map_err(|e| TransportError::Ipc(Box::new(e)))?,
        )
        .request_response::<[u8], [u8]>()
        .request_user_header::<TaskRequestHeader>()
        .response_user_header::<TaskResponseHeader>()
        .max_servers(1)
        .max_clients(1)
        .max_response_buffer_size(MAX_RESPONSE_BUFFER_SIZE)
        .enable_safe_overflow_for_responses(false)
        .open_or_create()
        .map_err(|e| TransportError::Ipc(Box::new(e)))?;

    Ok(service)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::headers::FLAG_IS_FINAL;

    #[test]
    fn task_request_response_roundtrip() {
        let _shared_service = crate::ipc::shared_notify_service_lock();
        let node = NodeBuilder::new().create::<IpcServiceType>().expect("node");

        let server = TaskServer::new(&node, "test-task-rt").expect("server");
        let client = TaskClient::new(&node, "test-task-rt").expect("client");

        let req_header = TaskRequestHeader {
            task_id: 42,
            request_type: 0,
            _reserved: [0; 3],
        };
        let payload = b"hello-request";

        let pending = client.send(&req_header, payload).expect("send");

        let active = server
            .receive()
            .expect("receive")
            .expect("should have request");
        assert_eq!(active.header().task_id, 42);
        assert_eq!(active.payload(), payload);

        let resp_header = TaskResponseHeader {
            task_id: 42,
            kind: 0,
            format: 1,
            flags: FLAG_IS_FINAL,
            name_len: 0,
            chunk_index: 0,
            total_size: 5,
        };
        active
            .send_response(&resp_header, b"world")
            .expect("respond");
        drop(active);

        let response = pending
            .try_recv()
            .expect("recv")
            .expect("should have response");
        assert_eq!(response.header.task_id, 42);
        assert_eq!(response.header.flags & FLAG_IS_FINAL, FLAG_IS_FINAL);
        assert_eq!(response.payload, b"world");
    }

    #[test]
    fn multi_response_streaming() {
        let _shared_service = crate::ipc::shared_notify_service_lock();
        let node = NodeBuilder::new().create::<IpcServiceType>().expect("node");

        let server = TaskServer::new(&node, "test-stream").expect("server");
        let client = TaskClient::new(&node, "test-stream").expect("client");

        let req_header = TaskRequestHeader {
            task_id: 1,
            request_type: 0,
            _reserved: [0; 3],
        };
        let pending = client.send(&req_header, b"go").expect("send");

        let active = server.receive().expect("receive").expect("request");

        for i in 0..3u32 {
            let h = TaskResponseHeader {
                task_id: 1,
                kind: 0,
                format: 0,
                flags: 0,
                name_len: 0,
                chunk_index: i,
                total_size: 0,
            };
            active.send_response(&h, &[i as u8; 4]).expect("chunk");
        }

        let final_h = TaskResponseHeader {
            task_id: 1,
            flags: FLAG_IS_FINAL,
            ..Default::default()
        };
        active.send_response(&final_h, &[]).expect("final");
        drop(active);

        let mut received = Vec::new();
        loop {
            if let Some(resp) = pending.try_recv().expect("recv") {
                if resp.header.flags & FLAG_IS_FINAL != 0 {
                    break;
                }
                received.push(resp);
            } else {
                std::thread::yield_now();
            }
        }
        assert_eq!(received.len(), 3);
        assert_eq!(received[0].header.chunk_index, 0);
        assert_eq!(received[2].header.chunk_index, 2);
    }
}
