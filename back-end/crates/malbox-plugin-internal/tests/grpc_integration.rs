use malbox_plugin_internal::transport::daemon::GrpcClient;
use malbox_plugin_internal::transport::grpc::proto;
use malbox_plugin_internal::transport::messages::events::Event;
use malbox_plugin_internal::transport::plugin::{GrpcServer, GuestPluginHandler, LogEntryStream};

use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::mpsc;
use tonic::Status;

struct TestHandler {
    /// Simple in-memory stash for pull_result integration tests.
    stash: Mutex<HashMap<String, Vec<u8>>>,
}

impl TestHandler {
    fn new() -> Self {
        Self {
            stash: Mutex::new(HashMap::new()),
        }
    }
}

#[tonic::async_trait]
impl GuestPluginHandler for TestHandler {
    async fn on_initialize(
        &self,
        plugin_id: i32,
        _config: HashMap<String, String>,
    ) -> Result<Vec<String>, String> {
        if plugin_id > 0 {
            Ok(vec!["analyze".to_string(), "report".to_string()])
        } else {
            Err("invalid plugin ID".to_string())
        }
    }

    async fn on_health_check(&self) -> (bool, String) {
        (true, String::new())
    }

    async fn on_shutdown(&self, _graceful: bool) {}

    async fn on_execute_task(
        &self,
        task_id: i32,
        sample_path: String,
        _config: HashMap<String, String>,
        result_tx: mpsc::Sender<Result<proto::TaskResult, Status>>,
    ) {
        if sample_path == "/large" {
            use prost::Message;

            // Stash a 5 MB payload keyed by a known handle.
            let handle = format!("handle-{task_id}");
            let big_payload = vec![0xAAu8; 5 * 1024 * 1024];
            self.stash
                .lock()
                .unwrap()
                .insert(handle.clone(), big_payload);

            let ref_msg = proto::ResultRef {
                handle,
                result_name: "big".to_string(),
                format: proto::ResultFormat::Bytes.into(),
                size_bytes: (5 * 1024 * 1024) as u64,
            };

            // Emit a RESULT_REF and then a final marker.
            let _ = result_tx
                .send(Ok(proto::TaskResult {
                    task_id,
                    result_name: String::new(),
                    data: ref_msg.encode_to_vec(),
                    format: proto::ResultFormat::Unspecified.into(),
                    is_final: false,
                    kind: proto::ResultKind::ResultRef.into(),
                }))
                .await;

            let _ = result_tx
                .send(Ok(proto::TaskResult {
                    task_id,
                    result_name: String::new(),
                    data: vec![],
                    format: proto::ResultFormat::Unspecified.into(),
                    is_final: true,
                    kind: proto::ResultKind::Result.into(),
                }))
                .await;
            return;
        }

        // Existing small-result path — stream two inline results.
        let _ = result_tx
            .send(Ok(proto::TaskResult {
                task_id,
                result_name: "partial".to_string(),
                data: b"partial data".to_vec(),
                format: proto::ResultFormat::Json.into(),
                is_final: false,
                kind: proto::ResultKind::Result.into(),
            }))
            .await;

        let _ = result_tx
            .send(Ok(proto::TaskResult {
                task_id,
                result_name: "final".to_string(),
                data: b"final data".to_vec(),
                format: proto::ResultFormat::Bytes.into(),
                is_final: true,
                kind: proto::ResultKind::Result.into(),
            }))
            .await;
        // tx is dropped here, closing the stream
    }

    async fn on_event(&self, _event: Event) -> Result<(), String> {
        Ok(())
    }

    async fn on_push_file(&self, _dest: &str, _data: Vec<u8>) -> Result<(), String> {
        Ok(())
    }

    async fn on_pull_file(&self, _source: &str) -> Result<Vec<u8>, String> {
        Ok(b"test file contents".to_vec())
    }

    async fn on_execute_command(
        &self,
        command: &str,
        _args: &[String],
        _cwd: Option<&str>,
        _env: HashMap<String, String>,
        _timeout_ms: Option<u64>,
        _background: bool,
    ) -> Result<proto::ExecResponse, String> {
        Ok(proto::ExecResponse {
            exit_code: Some(0),
            stdout: format!("ran: {}", command).into_bytes(),
            stderr: vec![],
            pid: None,
        })
    }

    async fn on_stream_logs(&self, _include_buffered: bool) -> LogEntryStream {
        let stream = async_stream::stream! {
            for i in 0..10_000u64 {
                yield Ok(proto::LogEntry {
                    timestamp_ns: i,
                    level: proto::LogLevel::Info as i32,
                    target: "test".to_string(),
                    message: format!("msg-{i}"),
                    fields: HashMap::new(),
                });
            }
        };
        Box::pin(stream)
    }

    async fn on_pull_result(
        &self,
        handle: String,
    ) -> Result<malbox_plugin_internal::transport::plugin::ResultChunkStream, String> {
        let data = self.stash.lock().unwrap().remove(&handle);

        match data {
            None => Err(format!("unknown handle: {handle}")),
            Some(bytes) => {
                let stream = async_stream::stream! {
                    let chunk_size = 64 * 1024;
                    let mut index = 0u32;
                    let mut offset = 0usize;
                    while offset < bytes.len() {
                        let end = (offset + chunk_size).min(bytes.len());
                        let is_last = end == bytes.len();
                        yield Ok(proto::ResultChunk {
                            data: bytes[offset..end].to_vec(),
                            index,
                            is_last,
                        });
                        index += 1;
                        offset = end;
                    }
                };
                Ok(Box::pin(stream))
            }
        }
    }
}

async fn setup() -> GrpcClient {
    let addr: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound_addr = listener.local_addr().unwrap();

    // Drop the listener so the port is free for the server
    drop(listener);

    let server = GrpcServer::new(TestHandler::new());
    tokio::spawn(async move {
        server.serve(bound_addr).await.unwrap();
    });

    // Give the server time to start listening
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let url = format!("http://{}", bound_addr);
    GrpcClient::connect(url)
        .await
        .expect("client should connect")
}

#[tokio::test]
async fn test_initialize_success() {
    let mut client = setup().await;

    let resp = client
        .initialize(1, HashMap::new())
        .await
        .expect("initialize RPC should succeed");

    assert!(resp.success, "expected success=true");
    assert!(
        resp.error_message.is_empty(),
        "expected empty error_message"
    );
    assert_eq!(
        resp.capabilities,
        vec!["analyze".to_string(), "report".to_string()],
        "expected capabilities [analyze, report]"
    );
}

#[tokio::test]
async fn test_initialize_failure() {
    let mut client = setup().await;

    let resp = client
        .initialize(0, HashMap::new())
        .await
        .expect("initialize RPC should succeed (even on logical failure)");

    assert!(!resp.success, "expected success=false");
    assert_eq!(
        resp.error_message, "invalid plugin ID",
        "expected error_message='invalid plugin ID'"
    );
    assert!(
        resp.capabilities.is_empty(),
        "expected empty capabilities on failure"
    );
}

#[tokio::test]
async fn test_execute_task_streaming() {
    let mut client = setup().await;

    let mut stream = client
        .execute_task(42, "/samples/malware.exe".to_string(), HashMap::new())
        .await
        .expect("execute_task RPC should succeed");

    // First result: partial
    let first = stream
        .message()
        .await
        .expect("reading first message should not fail")
        .expect("first message should be Some");
    assert_eq!(first.task_id, 42);
    assert_eq!(first.result_name, "partial");
    assert_eq!(first.data, b"partial data");
    assert_eq!(first.format, i32::from(proto::ResultFormat::Json));
    assert!(!first.is_final, "first result should not be final");

    // Second result: final
    let second = stream
        .message()
        .await
        .expect("reading second message should not fail")
        .expect("second message should be Some");
    assert_eq!(second.task_id, 42);
    assert_eq!(second.result_name, "final");
    assert_eq!(second.data, b"final data");
    assert_eq!(second.format, i32::from(proto::ResultFormat::Bytes));
    assert!(second.is_final, "second result should be final");

    // Stream should end
    let end = stream
        .message()
        .await
        .expect("reading end-of-stream should not fail");
    assert!(
        end.is_none(),
        "stream should be exhausted after two results"
    );
}

#[tokio::test]
async fn test_health_check() {
    let mut client = setup().await;

    let resp = client
        .health_check()
        .await
        .expect("health_check RPC should succeed");

    assert!(resp.ready, "expected ready=true");
}

#[tokio::test]
async fn test_notify_event() {
    let mut client = setup().await;

    let event = Event::TaskCreated { task_id: 42 };

    let resp = client
        .notify_event(&event)
        .await
        .expect("notify_event RPC should succeed");

    assert!(resp.success, "expected success=true");
    assert!(
        resp.error_message.is_empty(),
        "expected empty error_message"
    );
}

#[tokio::test]
async fn test_pull_result_streams_large_payload() {
    let mut client = setup().await;

    // Trigger the large-result path.
    let mut stream = client
        .execute_task(99, "/large".to_string(), HashMap::new())
        .await
        .expect("execute_task RPC should succeed");

    // Read the RESULT_REF.
    let ref_result = stream
        .message()
        .await
        .expect("reading first message should succeed")
        .expect("first message should be Some");
    assert_eq!(ref_result.kind, proto::ResultKind::ResultRef as i32);

    use prost::Message;
    let ref_msg = proto::ResultRef::decode(ref_result.data.as_slice()).expect("decode ResultRef");
    assert_eq!(ref_msg.result_name, "big");
    assert_eq!(ref_msg.size_bytes, 5 * 1024 * 1024);

    // Read the final marker.
    let final_marker = stream
        .message()
        .await
        .expect("reading second message")
        .expect("second message should be Some");
    assert!(final_marker.is_final, "expected is_final=true");

    // Pull the big payload via the separate RPC.
    let mut chunk_stream = client
        .pull_result(ref_msg.handle.clone())
        .await
        .expect("pull_result RPC should succeed");

    let mut collected = Vec::new();
    let mut seen_last = false;
    while let Some(chunk) = chunk_stream.message().await.expect("chunk read") {
        collected.extend_from_slice(&chunk.data);
        if chunk.is_last {
            seen_last = true;
            break;
        }
    }
    assert!(seen_last);
    assert_eq!(collected.len(), 5 * 1024 * 1024);
    assert!(collected.iter().all(|&b| b == 0xAA));
}

#[tokio::test]
async fn test_stream_logs_receives_all_entries_in_order() {
    let mut client = setup().await;

    let mut stream = client
        .stream_logs(true)
        .await
        .expect("stream_logs RPC should succeed");

    let mut received: Vec<u64> = Vec::with_capacity(10_000);
    while let Some(entry) = stream.message().await.expect("stream read") {
        received.push(entry.timestamp_ns);
        if received.len() >= 10_000 {
            break;
        }
    }

    assert_eq!(received.len(), 10_000);
    for (i, ts) in received.iter().enumerate() {
        assert_eq!(*ts, i as u64, "entries must arrive in order with no gaps");
    }
}
