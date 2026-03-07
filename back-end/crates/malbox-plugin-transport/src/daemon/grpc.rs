//! gRPC client for connecting to guest plugins running inside VMs.
//!
//! The daemon uses `GrpcClient` to call RPC methods on the plugin's gRPC server.
//! All methods are async.

use crate::grpc::conversions;
use crate::grpc::proto;
use crate::grpc::proto::guest_plugin_service_client::GuestPluginServiceClient;
use crate::error::{Result, TransportError};
use crate::messages::events::{Event, Payload};
use std::collections::HashMap;

/// Daemon-side gRPC client wrapping the generated tonic stub.
pub struct GrpcClient {
    inner: GuestPluginServiceClient<tonic::transport::Channel>,
}

impl GrpcClient {
    /// Connect to a guest plugin gRPC server at the given address.
    pub async fn connect(addr: impl Into<String>) -> Result<Self> {
        let inner = GuestPluginServiceClient::connect(addr.into())
            .await
            .map_err(TransportError::GrpcTransport)?;
        Ok(Self { inner })
    }

    /// Send an `InitializeRequest` and return the response.
    pub async fn initialize(
        &mut self,
        plugin_id: i32,
        config: HashMap<String, String>,
    ) -> Result<proto::InitializeResponse> {
        let request = proto::InitializeRequest { plugin_id, config };
        let response = self
            .inner
            .initialize(request)
            .await
            .map_err(TransportError::GrpcStatus)?;
        Ok(response.into_inner())
    }

    /// Send a `TaskRequest` and return the server-streaming response.
    pub async fn execute_task(
        &mut self,
        task_id: i32,
        sample_path: String,
        config: HashMap<String, String>,
    ) -> Result<tonic::Streaming<proto::TaskResult>> {
        let request = proto::TaskRequest {
            task_id,
            sample_path,
            config,
        };
        let response = self
            .inner
            .execute_task(request)
            .await
            .map_err(TransportError::GrpcStatus)?;
        Ok(response.into_inner())
    }

    /// Perform a health check against the guest plugin.
    pub async fn health_check(&mut self) -> Result<proto::HealthCheckResponse> {
        let response = self
            .inner
            .health_check(proto::HealthCheckRequest {})
            .await
            .map_err(TransportError::GrpcStatus)?;
        Ok(response.into_inner())
    }

    /// Request the guest plugin to shut down.
    pub async fn shutdown(&mut self, graceful: bool) -> Result<proto::ShutdownResponse> {
        let request = proto::ShutdownRequest { graceful };
        let response = self
            .inner
            .shutdown(request)
            .await
            .map_err(TransportError::GrpcStatus)?;
        Ok(response.into_inner())
    }

    /// Notify the guest plugin of a system-wide event.
    pub async fn notify_event(
        &mut self,
        event: &Event,
        payload: &Payload,
    ) -> Result<proto::EventAck> {
        let notification = conversions::event_to_proto(event, payload)?;
        let response = self
            .inner
            .notify_event(notification)
            .await
            .map_err(TransportError::GrpcStatus)?;
        Ok(response.into_inner())
    }

    /// Push a file to the guest via streaming chunks.
    pub async fn push_file(
        &mut self,
        dest: &str,
        data: Vec<u8>,
    ) -> Result<proto::FileTransferResponse> {
        let dest = dest.to_string();
        let chunks = data
            .chunks(64 * 1024)
            .enumerate()
            .map(|(i, chunk)| {
                let remaining = data.len() - (i * 64 * 1024) - chunk.len();
                proto::FileChunk {
                    path: dest.clone(),
                    data: chunk.to_vec(),
                    is_last: remaining == 0,
                }
            })
            .collect::<Vec<_>>();

        let response = self
            .inner
            .push_file(tokio_stream::iter(chunks))
            .await
            .map_err(TransportError::GrpcStatus)?;
        Ok(response.into_inner())
    }

    /// Pull a file from the guest, collecting streamed chunks.
    pub async fn pull_file(&mut self, source: &str) -> Result<Vec<u8>> {
        let request = proto::PullFileRequest {
            path: source.to_string(),
        };
        let mut stream = self
            .inner
            .pull_file(request)
            .await
            .map_err(TransportError::GrpcStatus)?
            .into_inner();

        let mut buf = Vec::new();
        while let Some(chunk) = stream
            .message()
            .await
            .map_err(TransportError::GrpcStatus)?
        {
            buf.extend_from_slice(&chunk.data);
        }
        Ok(buf)
    }

    /// Execute a command on the guest.
    pub async fn execute_command(
        &mut self,
        command: &str,
        args: &[String],
        cwd: Option<&str>,
        env: &[(String, String)],
        timeout_ms: Option<u64>,
        background: bool,
    ) -> Result<proto::ExecResponse> {
        let request = proto::ExecRequest {
            command: command.to_string(),
            args: args.to_vec(),
            cwd: cwd.map(|s| s.to_string()),
            env: env.iter().cloned().collect(),
            timeout_ms,
            background,
        };
        let response = self
            .inner
            .execute_command(request)
            .await
            .map_err(TransportError::GrpcStatus)?;
        Ok(response.into_inner())
    }
}
