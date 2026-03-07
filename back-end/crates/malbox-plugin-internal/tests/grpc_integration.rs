use malbox_plugin_internal::transport::daemon::GrpcClient;
use malbox_plugin_internal::transport::grpc::proto;
use malbox_plugin_internal::transport::plugin::{GrpcServer, GuestPluginHandler};
use malbox_plugin_internal::transport::messages::events::*;

use std::collections::HashMap;
use tokio::sync::mpsc;
use tonic::Status;

struct TestHandler;

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
        _sample_path: String,
        _config: HashMap<String, String>,
        result_tx: mpsc::Sender<Result<proto::TaskResult, Status>>,
    ) {
        // Stream two results, second is final
        let _ = result_tx
            .send(Ok(proto::TaskResult {
                task_id,
                result_name: "partial".to_string(),
                data: b"partial data".to_vec(),
                format: proto::ResultFormat::Json.into(),
                is_final: false,
            }))
            .await;

        let _ = result_tx
            .send(Ok(proto::TaskResult {
                task_id,
                result_name: "final".to_string(),
                data: b"final data".to_vec(),
                format: proto::ResultFormat::Bytes.into(),
                is_final: true,
            }))
            .await;
        // tx is dropped here, closing the stream
    }

    async fn on_event(&self, _event: Event, _payload: Payload) -> Result<(), String> {
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
        })
    }
}

async fn setup() -> GrpcClient {
    let addr: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound_addr = listener.local_addr().unwrap();

    // Drop the listener so the port is free for the server
    drop(listener);

    let server = GrpcServer::new(TestHandler);
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

    let event = Event::Task(TaskEvent::TaskCreated);
    let payload = Payload::Task(TaskEventPayload { task_id: 42 });

    let resp = client
        .notify_event(&event, &payload)
        .await
        .expect("notify_event RPC should succeed");

    assert!(resp.success, "expected success=true");
    assert!(
        resp.error_message.is_empty(),
        "expected empty error_message"
    );
}
