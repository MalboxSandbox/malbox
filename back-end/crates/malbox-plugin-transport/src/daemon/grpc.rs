//! gRPC client for connecting to guest plugins running inside VMs.
//!
//! The daemon uses `GrpcClient` to call RPC methods on the plugin's gRPC server.
//! All methods are async.

use crate::error::{Result, TransportError};
use crate::grpc::proto;
use crate::grpc::proto::guest_plugin_service_client::GuestPluginServiceClient;
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
    ///
    /// Rejects files larger than 256 MB to prevent unbounded memory growth.
    pub async fn pull_file(&mut self, source: &str) -> Result<Vec<u8>> {
        const MAX_FILE_SIZE: usize = 256 * 1024 * 1024;

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
        while let Some(chunk) = stream.message().await.map_err(TransportError::GrpcStatus)? {
            if buf.len() + chunk.data.len() > MAX_FILE_SIZE {
                return Err(TransportError::Grpc(format!(
                    "pull_file exceeded {MAX_FILE_SIZE} byte limit for '{source}'"
                )));
            }
            buf.extend_from_slice(&chunk.data);
        }
        Ok(buf)
    }

    /// Start streaming logs from the guest plugin.
    ///
    /// Returns a streaming response of `LogEntry` messages. The stream stays
    /// open for the plugin's lifetime.
    pub async fn stream_logs(
        &mut self,
        include_buffered: bool,
    ) -> Result<tonic::Streaming<proto::LogEntry>> {
        let request = proto::LogStreamRequest { include_buffered };
        let response = self
            .inner
            .stream_logs(request)
            .await
            .map_err(TransportError::GrpcStatus)?;
        Ok(response.into_inner())
    }

    /// Pull a stashed large result from the guest plugin by handle.
    ///
    /// Returns a streaming response of `ResultChunk` messages. The caller
    /// concatenates `chunk.data` in `index` order until a chunk with
    /// `is_last == true` is received.
    pub async fn pull_result(
        &mut self,
        handle: String,
    ) -> Result<tonic::Streaming<proto::ResultChunk>> {
        let request = proto::PullResultRequest { handle };
        let response = self
            .inner
            .pull_result(request)
            .await
            .map_err(TransportError::GrpcStatus)?;
        Ok(response.into_inner())
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

#[cfg(test)]
mod tests {
    use super::*;

    // Compile-time check: pull_result exists with the expected signature.
    #[allow(dead_code)]
    fn _pull_result_signature_compiles(client: &mut GrpcClient) {
        let _fut: std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<tonic::Streaming<proto::ResultChunk>>>>,
        > = Box::pin(client.pull_result("handle".to_string()));
    }
}
