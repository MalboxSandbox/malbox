//! Per-plugin result pub/sub channel for chained/self-initiated work.
//!
//! Service name: `malbox/plugin/{plugin_id}/results`
//! Pattern: publish_subscribe::<[u8]>() with user_header::<ResultHeader>()
//!
//! A plugin publishes results here. The daemon and any chaining plugins subscribe.
//! This channel is separate from the request/response task service - it carries
//! results from self-initiated work (event-driven chaining).

use crate::error::{Result, TransportError};

use super::headers::ResultHeader;

use iceoryx2::port::publisher::Publisher;
use iceoryx2::port::subscriber::Subscriber;
use iceoryx2::prelude::*;

type IpcServiceType = iceoryx2::service::ipc_threadsafe::Service;

const MAX_RESULT_SLICE_LEN: usize = 1024 * 1024; // 1 MB (chunk size)
const MAX_PUBLISHERS: usize = 1; // one plugin owns the channel
const MAX_SUBSCRIBERS: usize = 65; // daemon + up to 64 subscribing plugins
const SUBSCRIBER_BUFFER_SIZE: usize = 16;

fn result_service_name(plugin_id: &str) -> String {
    format!("malbox/plugin/{plugin_id}/results")
}

/// Publisher side: a plugin publishes result data on its own channel.
pub struct ResultPublisher {
    publisher: Publisher<IpcServiceType, [u8], ResultHeader>,
}

impl ResultPublisher {
    pub fn new(node: &Node<IpcServiceType>, plugin_id: &str) -> Result<Self> {
        let name = result_service_name(plugin_id);
        let service = node
            .service_builder(
                &name
                    .as_str()
                    .try_into()
                    .map_err(|e| TransportError::Ipc(Box::new(e)))?,
            )
            .publish_subscribe::<[u8]>()
            .user_header::<ResultHeader>()
            .max_publishers(MAX_PUBLISHERS)
            .max_subscribers(MAX_SUBSCRIBERS)
            .subscriber_max_buffer_size(SUBSCRIBER_BUFFER_SIZE)
            .enable_safe_overflow(false)
            .open_or_create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        let publisher = service
            .publisher_builder()
            .initial_max_slice_len(MAX_RESULT_SLICE_LEN)
            .allocation_strategy(AllocationStrategy::PowerOfTwo)
            .create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        Ok(Self { publisher })
    }

    /// Publish result data with a header. Retries on transient OutOfMemory
    /// with a bounded retry window.
    pub fn publish(&self, header: &ResultHeader, payload: &[u8]) -> Result<()> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);

        loop {
            match self.publisher.loan_slice_uninit(payload.len()) {
                Ok(mut sample) => {
                    let user_header = sample.user_header_mut();
                    *user_header = *header;
                    let sample = sample.write_from_fn(|i| payload[i]);
                    sample
                        .send()
                        .map_err(|e| TransportError::Ipc(Box::new(e)))?;
                    return Ok(());
                }
                Err(iceoryx2::port::LoanError::OutOfMemory) => {
                    if std::time::Instant::now() > deadline {
                        return Err(TransportError::BufferFull(format!(
                            "result publish buffer full for 30s (payload {} bytes)",
                            payload.len()
                        )));
                    }
                    std::thread::yield_now();
                }
                Err(e) => {
                    return Err(TransportError::Ipc(Box::new(e)));
                }
            }
        }
    }
}

/// Subscriber side: daemon or chaining plugin reads results from a plugin.
pub struct ResultSubscriber {
    subscriber: Subscriber<IpcServiceType, [u8], ResultHeader>,
}

impl ResultSubscriber {
    pub fn new(node: &Node<IpcServiceType>, plugin_id: &str) -> Result<Self> {
        let name = result_service_name(plugin_id);
        let service = node
            .service_builder(
                &name
                    .as_str()
                    .try_into()
                    .map_err(|e| TransportError::Ipc(Box::new(e)))?,
            )
            .publish_subscribe::<[u8]>()
            .user_header::<ResultHeader>()
            .max_publishers(MAX_PUBLISHERS)
            .max_subscribers(MAX_SUBSCRIBERS)
            .subscriber_max_buffer_size(SUBSCRIBER_BUFFER_SIZE)
            .enable_safe_overflow(false)
            .open_or_create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        let subscriber = service
            .subscriber_builder()
            .create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        Ok(Self { subscriber })
    }

    /// Non-blocking receive. Returns None if no result is pending.
    pub fn try_recv(&self) -> Result<Option<ReceivedResult>> {
        match self
            .subscriber
            .receive()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?
        {
            Some(sample) => {
                let header = *sample.user_header();
                let payload = sample.payload().to_vec();
                Ok(Some(ReceivedResult { header, payload }))
            }
            None => Ok(None),
        }
    }

    /// Drain all pending results.
    pub fn drain(&self) -> Result<Vec<ReceivedResult>> {
        let mut results = Vec::new();
        while let Some(r) = self.try_recv()? {
            results.push(r);
        }
        Ok(results)
    }
}

/// A received result with its header and payload.
pub struct ReceivedResult {
    pub header: ResultHeader,
    pub payload: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::headers::{ResultFormat, ResultPayloadKind};

    #[test]
    fn result_publish_subscribe_roundtrip() {
        let node = NodeBuilder::new().create::<IpcServiceType>().expect("node");

        let publisher = ResultPublisher::new(&node, "test-results").expect("publisher");
        let subscriber = ResultSubscriber::new(&node, "test-results").expect("subscriber");

        let header = ResultHeader {
            task_id: 7,
            payload_kind: ResultPayloadKind::Inline as u8,
            format: ResultFormat::Json as u8,
            flags: 0,
            _reserved: 0,
            chunk_index: 0,
            total_size: 11,
            name_len: 6,
            _reserved2: [0; 2],
        };
        let payload = b"report:hello";

        publisher.publish(&header, payload).expect("publish");

        let received = subscriber
            .try_recv()
            .expect("recv")
            .expect("should have result");
        assert_eq!(received.header.task_id, 7);
        assert_eq!(received.header.name_len, 6);
        assert_eq!(received.payload, payload);
    }

    #[test]
    fn drain_multiple() {
        let node = NodeBuilder::new().create::<IpcServiceType>().expect("node");

        let publisher = ResultPublisher::new(&node, "test-drain").expect("publisher");
        let subscriber = ResultSubscriber::new(&node, "test-drain").expect("subscriber");

        for i in 0..3i32 {
            let header = ResultHeader {
                task_id: i,
                ..Default::default()
            };
            publisher.publish(&header, &[i as u8]).expect("publish");
        }

        let results = subscriber.drain().expect("drain");
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].header.task_id, 0);
        assert_eq!(results[2].header.task_id, 2);
    }

    #[test]
    fn try_recv_empty() {
        let node = NodeBuilder::new().create::<IpcServiceType>().expect("node");

        let subscriber = ResultSubscriber::new(&node, "test-empty-res").expect("subscriber");
        assert!(subscriber.try_recv().expect("recv").is_none());
    }
}
