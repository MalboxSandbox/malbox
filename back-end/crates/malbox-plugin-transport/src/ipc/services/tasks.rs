//! Task data channels for IPC communication between daemon and host plugins.
//!
//! Each host plugin gets a dedicated set of iceoryx2 services for exchanging
//! task request and result payloads. Service names are derived deterministically
//! from the plugin ID.

pub mod publish;
pub mod receive;

pub use publish::{TaskRequestPublisher, TaskResultPublisher};
pub use receive::{TaskRequestReceiver, TaskResultReceiver};

/// Derive the request channel service name for a given plugin.
pub fn request_service_name(plugin_id: &str) -> String {
    format!("malbox/tasks/{plugin_id}/requests")
}

/// Derive the result data channel service name for a given plugin.
pub fn result_service_name(plugin_id: &str) -> String {
    format!("malbox/tasks/{plugin_id}/results")
}

/// Derive the result notification channel service name for a given plugin.
pub fn result_notify_service_name(plugin_id: &str) -> String {
    format!("malbox/tasks/{plugin_id}/results/notify")
}

/// Fixed-size header prepended to every result message on the IPC result channel.
///
/// The daemon reads these fields directly to determine how to handle the
/// trailing protobuf payload without deserializing the entire message.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IpcTaskResultHeader {
    pub task_id: i32,
    pub format: i32,
    pub kind: i32,
    pub is_final: u8,
    pub _padding: [u8; 3],
    pub payload_len: u32,
}

impl IpcTaskResultHeader {
    pub const SIZE: usize = 20;

    /// Serialize into the first [`SIZE`](Self::SIZE) bytes of `buf`.
    ///
    /// # Panics
    ///
    /// Panics if `buf.len() < Self::SIZE`.
    pub fn write_to(&self, buf: &mut [u8]) {
        buf[0..4].copy_from_slice(&self.task_id.to_ne_bytes());
        buf[4..8].copy_from_slice(&self.format.to_ne_bytes());
        buf[8..12].copy_from_slice(&self.kind.to_ne_bytes());
        buf[12] = self.is_final;
        buf[13..16].copy_from_slice(&self._padding);
        buf[16..20].copy_from_slice(&self.payload_len.to_ne_bytes());
    }

    /// Deserialize from the first [`SIZE`](Self::SIZE) bytes of `buf`.
    ///
    /// Returns `None` if `buf` is too short.
    pub fn read_from(buf: &[u8]) -> Option<Self> {
        if buf.len() < Self::SIZE {
            return None;
        }
        Some(Self {
            task_id: i32::from_ne_bytes(buf[0..4].try_into().unwrap()),
            format: i32::from_ne_bytes(buf[4..8].try_into().unwrap()),
            kind: i32::from_ne_bytes(buf[8..12].try_into().unwrap()),
            is_final: buf[12],
            _padding: [buf[13], buf[14], buf[15]],
            payload_len: u32::from_ne_bytes(buf[16..20].try_into().unwrap()),
        })
    }

    /// Build a final marker header with no payload.
    pub fn final_marker(task_id: i32) -> Self {
        Self {
            task_id,
            format: 0,
            kind: 0,
            is_final: 1,
            _padding: [0; 3],
            payload_len: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_size_matches_const() {
        assert_eq!(
            std::mem::size_of::<IpcTaskResultHeader>(),
            IpcTaskResultHeader::SIZE
        );
    }

    #[test]
    fn header_roundtrip() {
        let header = IpcTaskResultHeader {
            task_id: 42,
            format: 1,
            kind: 0,
            is_final: 0,
            _padding: [0; 3],
            payload_len: 128,
        };
        let mut buf = [0u8; IpcTaskResultHeader::SIZE];
        header.write_to(&mut buf);
        let decoded = IpcTaskResultHeader::read_from(&buf).unwrap();
        assert_eq!(header, decoded);
    }

    #[test]
    fn header_read_from_short_buffer_returns_none() {
        let buf = [0u8; 10];
        assert!(IpcTaskResultHeader::read_from(&buf).is_none());
    }

    #[test]
    fn header_final_marker() {
        let marker = IpcTaskResultHeader::final_marker(7);
        assert_eq!(marker.task_id, 7);
        assert_eq!(marker.is_final, 1);
        assert_eq!(marker.payload_len, 0);
    }

    #[test]
    fn service_names_contain_plugin_id() {
        assert_eq!(request_service_name("yara"), "malbox/tasks/yara/requests");
        assert_eq!(result_service_name("yara"), "malbox/tasks/yara/results");
        assert_eq!(
            result_notify_service_name("yara"),
            "malbox/tasks/yara/results/notify"
        );
    }

    #[test]
    fn request_publish_receive_roundtrip() {
        use super::publish::TaskRequestPublisher;
        use super::receive::TaskRequestReceiver;
        use iceoryx2::prelude::*;

        let node = NodeBuilder::new()
            .create::<iceoryx2::service::ipc_threadsafe::Service>()
            .expect("node creation");

        let publisher =
            TaskRequestPublisher::new(&node, "test-roundtrip").expect("publisher creation");
        let receiver =
            TaskRequestReceiver::new(&node, "test-roundtrip").expect("receiver creation");

        let data = b"hello task request";
        publisher.publish(data).expect("publish");

        let received = receiver.try_recv().expect("try_recv");
        assert_eq!(received, Some(data.to_vec()));

        assert_eq!(receiver.try_recv().expect("try_recv empty"), None);
    }

    #[test]
    fn result_publish_receive_roundtrip() {
        use super::publish::TaskResultPublisher;
        use super::receive::TaskResultReceiver;
        use iceoryx2::prelude::*;
        use std::time::Duration;

        let node = NodeBuilder::new()
            .create::<iceoryx2::service::ipc_threadsafe::Service>()
            .expect("node creation");

        let publisher =
            TaskResultPublisher::new(&node, "test-result-rt").expect("publisher creation");
        let receiver = TaskResultReceiver::new(&node, "test-result-rt").expect("receiver creation");

        let header = IpcTaskResultHeader {
            task_id: 42,
            format: 1,
            kind: 0,
            is_final: 0,
            _padding: [0; 3],
            payload_len: 5,
        };
        let payload = b"hello";

        publisher.publish(&header, payload).expect("publish");

        let (recv_header, recv_payload) = receiver
            .wait(Duration::from_secs(1))
            .expect("wait")
            .expect("should have message");

        assert_eq!(recv_header, header);
        assert_eq!(recv_payload, payload.to_vec());
    }

    #[test]
    fn result_final_marker_roundtrip() {
        use super::publish::TaskResultPublisher;
        use super::receive::TaskResultReceiver;
        use iceoryx2::prelude::*;
        use std::time::Duration;

        let node = NodeBuilder::new()
            .create::<iceoryx2::service::ipc_threadsafe::Service>()
            .expect("node creation");

        let publisher =
            TaskResultPublisher::new(&node, "test-final-rt").expect("publisher creation");
        let receiver = TaskResultReceiver::new(&node, "test-final-rt").expect("receiver creation");

        publisher.publish_final(7).expect("publish_final");

        let (header, payload) = receiver
            .wait(Duration::from_secs(1))
            .expect("wait")
            .expect("should have message");

        assert_eq!(header.task_id, 7);
        assert_eq!(header.is_final, 1);
        assert_eq!(header.payload_len, 0);
        assert!(payload.is_empty());
    }
}
