//! gRPC server — implements the GuestPluginService for guest plugins.
//!
//! The `GuestPluginHandler` trait defines the interface that a concrete guest
//! plugin must implement. `GrpcServer<H>` wraps a handler and exposes it as a
//! tonic service, handling the translation between protobuf messages and the
//! higher-level trait methods.

use crate::grpc::conversions;
use crate::grpc::proto;
use crate::grpc::proto::guest_plugin_service_server::{
    GuestPluginService, GuestPluginServiceServer,
};
use crate::messages::events::{Event, Payload};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::pin::Pin;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

// ---------------------------------------------------------------------------
// Stream type alias
// ---------------------------------------------------------------------------

/// The stream type returned from the `execute_task` RPC method.
pub type TaskResultStream =
    Pin<Box<dyn tokio_stream::Stream<Item = Result<proto::TaskResult, Status>> + Send>>;

/// The stream type returned from the `pull_file` RPC method.
pub type FileChunkStream =
    Pin<Box<dyn tokio_stream::Stream<Item = Result<proto::FileChunk, Status>> + Send>>;

// ---------------------------------------------------------------------------
// GuestPluginHandler trait
// ---------------------------------------------------------------------------

/// Async trait that a guest plugin implementation must satisfy.
///
/// The daemon calls these methods in response to incoming gRPC requests. The
/// future plugin SDK will provide a concrete implementation of this trait.
#[tonic::async_trait]
pub trait GuestPluginHandler: Send + Sync + 'static {
    /// Called when the daemon initializes the plugin.
    ///
    /// Returns a list of capability strings on success, or an error message on
    /// failure.
    async fn on_initialize(
        &self,
        plugin_id: i32,
        config: HashMap<String, String>,
    ) -> Result<Vec<String>, String>;

    /// Called periodically to check whether the plugin is ready to accept work.
    ///
    /// Returns `(ready, reason)` where `reason` is a human-readable string that
    /// is meaningful when `ready` is `false`.
    async fn on_health_check(&self) -> (bool, String);

    /// Called when the daemon requests a clean or forceful shutdown.
    async fn on_shutdown(&self, graceful: bool);

    /// Called when the daemon wants the plugin to analyse a sample.
    ///
    /// Results are sent incrementally over `result_tx`. The handler is
    /// responsible for closing the channel (by dropping it) when all results
    /// have been produced.
    async fn on_execute_task(
        &self,
        task_id: i32,
        sample_path: String,
        config: HashMap<String, String>,
        result_tx: mpsc::Sender<Result<proto::TaskResult, Status>>,
    );

    /// Called when the daemon broadcasts a system event to the plugin.
    ///
    /// Returns `Ok(())` on success or an error message on failure.
    async fn on_event(&self, event: Event, payload: Payload) -> Result<(), String>;

    /// Handle an incoming file push from the daemon.
    async fn on_push_file(&self, dest: &str, data: Vec<u8>) -> Result<(), String>;

    /// Handle a file pull request — return the file contents.
    async fn on_pull_file(&self, source: &str) -> Result<Vec<u8>, String>;

    /// Execute a command on the guest OS.
    async fn on_execute_command(
        &self,
        command: &str,
        args: &[String],
        cwd: Option<&str>,
        env: HashMap<String, String>,
        timeout_ms: Option<u64>,
        background: bool,
    ) -> Result<proto::ExecResponse, String>;
}

// ---------------------------------------------------------------------------
// GrpcServer
// ---------------------------------------------------------------------------

/// Wraps a `GuestPluginHandler` and exposes it as a tonic gRPC service.
pub struct GrpcServer<H: GuestPluginHandler> {
    handler: std::sync::Arc<H>,
}

impl<H: GuestPluginHandler> GrpcServer<H> {
    /// Create a new `GrpcServer` wrapping `handler`.
    pub fn new(handler: H) -> Self {
        Self {
            handler: std::sync::Arc::new(handler),
        }
    }

    /// Start listening on `addr` and serve requests until the process exits.
    pub async fn serve(self, addr: SocketAddr) -> Result<(), tonic::transport::Error> {
        tonic::transport::Server::builder()
            .add_service(GuestPluginServiceServer::new(self))
            .serve(addr)
            .await
    }

    /// Consume this server and return the tonic `GuestPluginServiceServer`
    /// wrapper so it can be embedded in a larger tonic server.
    pub fn into_service(self) -> GuestPluginServiceServer<Self> {
        GuestPluginServiceServer::new(self)
    }
}

// ---------------------------------------------------------------------------
// GuestPluginService tonic trait implementation
// ---------------------------------------------------------------------------

#[tonic::async_trait]
impl<H: GuestPluginHandler> GuestPluginService for GrpcServer<H> {
    type ExecuteTaskStream = TaskResultStream;
    type PullFileStream = FileChunkStream;

    // --- Initialize ---

    async fn initialize(
        &self,
        request: Request<proto::InitializeRequest>,
    ) -> Result<Response<proto::InitializeResponse>, Status> {
        let req = request.into_inner();

        match self.handler.on_initialize(req.plugin_id, req.config).await {
            Ok(capabilities) => Ok(Response::new(proto::InitializeResponse {
                success: true,
                error_message: String::new(),
                capabilities,
            })),
            Err(error_message) => Ok(Response::new(proto::InitializeResponse {
                success: false,
                error_message,
                capabilities: vec![],
            })),
        }
    }

    // --- HealthCheck ---

    async fn health_check(
        &self,
        _request: Request<proto::HealthCheckRequest>,
    ) -> Result<Response<proto::HealthCheckResponse>, Status> {
        let (ready, reason) = self.handler.on_health_check().await;

        Ok(Response::new(proto::HealthCheckResponse { ready, reason }))
    }

    // --- Shutdown ---

    async fn shutdown(
        &self,
        request: Request<proto::ShutdownRequest>,
    ) -> Result<Response<proto::ShutdownResponse>, Status> {
        let req = request.into_inner();

        self.handler.on_shutdown(req.graceful).await;

        Ok(Response::new(proto::ShutdownResponse {
            acknowledged: true,
        }))
    }

    // --- ExecuteTask (server-streaming) ---

    async fn execute_task(
        &self,
        request: Request<proto::TaskRequest>,
    ) -> Result<Response<Self::ExecuteTaskStream>, Status> {
        let req = request.into_inner();

        let (tx, rx) = mpsc::channel::<Result<proto::TaskResult, Status>>(32);

        let handler = self.handler.clone();
        tokio::spawn(async move {
            handler
                .on_execute_task(req.task_id, req.sample_path, req.config, tx)
                .await;
        });

        let stream: TaskResultStream = Box::pin(ReceiverStream::new(rx));
        Ok(Response::new(stream))
    }

    // --- NotifyEvent ---

    async fn notify_event(
        &self,
        request: Request<proto::EventNotification>,
    ) -> Result<Response<proto::EventAck>, Status> {
        let notification = request.into_inner();

        match conversions::proto_to_event(notification) {
            Ok((event, payload)) => match self.handler.on_event(event, payload).await {
                Ok(()) => Ok(Response::new(proto::EventAck {
                    success: true,
                    error_message: String::new(),
                })),
                Err(error_message) => Ok(Response::new(proto::EventAck {
                    success: false,
                    error_message,
                })),
            },
            Err(transport_err) => Ok(Response::new(proto::EventAck {
                success: false,
                error_message: transport_err.to_string(),
            })),
        }
    }

    // --- PushFile (client-streaming) ---

    async fn push_file(
        &self,
        request: Request<tonic::Streaming<proto::FileChunk>>,
    ) -> Result<Response<proto::FileTransferResponse>, Status> {
        let mut stream = request.into_inner();
        let mut dest = String::new();
        let mut data = Vec::new();

        while let Some(chunk) = stream
            .message()
            .await
            .map_err(|e| Status::internal(format!("stream error: {}", e)))?
        {
            if dest.is_empty() {
                dest.clone_from(&chunk.path);
            }
            data.extend_from_slice(&chunk.data);
        }

        match self.handler.on_push_file(&dest, data).await {
            Ok(()) => Ok(Response::new(proto::FileTransferResponse {
                success: true,
                error_message: String::new(),
            })),
            Err(error_message) => Ok(Response::new(proto::FileTransferResponse {
                success: false,
                error_message,
            })),
        }
    }

    // --- PullFile (server-streaming) ---

    async fn pull_file(
        &self,
        request: Request<proto::PullFileRequest>,
    ) -> Result<Response<Self::PullFileStream>, Status> {
        let req = request.into_inner();

        match self.handler.on_pull_file(&req.path).await {
            Ok(data) => {
                let path = req.path;
                let chunks: Vec<Result<proto::FileChunk, Status>> = data
                    .chunks(64 * 1024)
                    .enumerate()
                    .map(|(i, chunk)| {
                        let remaining = data.len() - (i * 64 * 1024) - chunk.len();
                        Ok(proto::FileChunk {
                            path: path.clone(),
                            data: chunk.to_vec(),
                            is_last: remaining == 0,
                        })
                    })
                    .collect();
                let stream: Self::PullFileStream = Box::pin(tokio_stream::iter(chunks));
                Ok(Response::new(stream))
            }
            Err(e) => Err(Status::internal(e)),
        }
    }

    // --- ExecuteCommand ---

    async fn execute_command(
        &self,
        request: Request<proto::ExecRequest>,
    ) -> Result<Response<proto::ExecResponse>, Status> {
        let req = request.into_inner();

        match self
            .handler
            .on_execute_command(
                &req.command,
                &req.args,
                req.cwd.as_deref(),
                req.env,
                req.timeout_ms,
                req.background,
            )
            .await
        {
            Ok(response) => Ok(Response::new(response)),
            Err(e) => Err(Status::internal(e)),
        }
    }
}
