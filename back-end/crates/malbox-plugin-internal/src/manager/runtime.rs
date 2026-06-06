use std::collections::HashMap;
use std::time::Duration;

use tracing::debug;

use crate::manager::error::{ManagerError, Result};
use crate::manager::handle::{OutputFormat, PluginOutput};
use crate::manager::ipc_reactor::IpcReactorHandle;
use crate::registry::types::{PluginEntry, PluginId};
use crate::transport::daemon::GrpcClient;

/// Polymorphic operations shared by both host and guest runtimes.
///
/// Not used as `dyn PluginExecution` - the `PluginRuntime` enum delegates
/// via match arms. The trait enforces signature parity between impls and
/// documents the contract.
#[allow(async_fn_in_trait)]
pub trait PluginExecution: Send {
    async fn execute_task(
        &mut self,
        plugin_id: &PluginId,
        entry: &PluginEntry,
        task_id: i32,
        sample_path: &str,
        config: HashMap<String, String>,
    ) -> Result<Vec<PluginOutput>>;

    async fn check_health(&mut self) -> bool;
}

/// Host plugin runtime: child process + a handle to the IPC reactor.
pub struct HostRuntime {
    pub process: tokio::process::Child,
    pub ipc: IpcReactorHandle,
}

/// Guest plugin runtime: gRPC client connection.
pub struct GuestRuntime {
    pub grpc_client: GrpcClient,
}

/// Type-discriminated runtime state for a plugin instance.
pub enum PluginRuntime {
    Host(HostRuntime),
    Guest(GuestRuntime),
}

impl PluginExecution for HostRuntime {
    async fn execute_task(
        &mut self,
        plugin_id: &PluginId,
        entry: &PluginEntry,
        task_id: i32,
        sample_path: &str,
        config: HashMap<String, String>,
    ) -> Result<Vec<PluginOutput>> {
        debug!(plugin = %plugin_id, task_id, "executing task on host plugin via IPC reactor");

        let timeout_secs = entry
            .runtime_config
            .as_ref()
            .map(|c| c.analysis_timeout)
            .unwrap_or(300);

        self.ipc
            .execute_task(
                plugin_id,
                task_id,
                sample_path.to_string(),
                config,
                Duration::from_secs(timeout_secs),
            )
            .await
    }

    async fn check_health(&mut self) -> bool {
        matches!(self.process.try_wait(), Ok(None))
    }
}

impl PluginExecution for GuestRuntime {
    async fn execute_task(
        &mut self,
        plugin_id: &PluginId,
        _entry: &PluginEntry,
        task_id: i32,
        sample_path: &str,
        config: HashMap<String, String>,
    ) -> Result<Vec<PluginOutput>> {
        use crate::transport::grpc::proto::{self, ResultFormat as ProtoFormat, ResultKind};
        use prost::Message;

        debug!(plugin = %plugin_id, task_id, "executing task on guest plugin via gRPC");

        let mut stream = self
            .grpc_client
            .execute_task(task_id, sample_path.to_string(), config)
            .await
            .map_err(|e| ManagerError::ExecutionFailed(plugin_id.clone(), e.to_string()))?;

        let mut inline_outputs: Vec<PluginOutput> = Vec::new();
        let mut pending_refs: Vec<proto::ResultRef> = Vec::new();

        loop {
            match stream.message().await {
                Ok(Some(result)) => {
                    let kind = ResultKind::try_from(result.kind).unwrap_or(ResultKind::Result);
                    let is_final = result.is_final;

                    match kind {
                        ResultKind::Progress => {
                            debug!(plugin = %plugin_id, task_id = result.task_id, "received progress update");
                        }
                        ResultKind::Ready => {
                            debug!(plugin = %plugin_id, task_id = result.task_id, "plugin signaled ready");
                        }
                        ResultKind::Result => {
                            use tracing::info;
                            info!(
                                plugin = %plugin_id,
                                task_id = result.task_id,
                                result_name = %result.result_name,
                                data_len = result.data.len(),
                                is_final,
                                "received inline task result from guest plugin"
                            );

                            let format = match ProtoFormat::try_from(result.format) {
                                Ok(ProtoFormat::Json) => OutputFormat::Json,
                                _ => OutputFormat::Bytes,
                            };

                            if !result.result_name.is_empty() {
                                inline_outputs.push(PluginOutput {
                                    result_name: result.result_name,
                                    data: result.data,
                                    format,
                                });
                            }
                        }
                        ResultKind::ResultRef => {
                            let ref_msg = proto::ResultRef::decode(result.data.as_slice())
                                .map_err(|e| {
                                    ManagerError::ExecutionFailed(
                                        plugin_id.clone(),
                                        format!("failed to decode ResultRef: {e}"),
                                    )
                                })?;
                            use tracing::info;
                            info!(
                                plugin = %plugin_id,
                                task_id = result.task_id,
                                handle = %ref_msg.handle,
                                result_name = %ref_msg.result_name,
                                size_bytes = ref_msg.size_bytes,
                                "received result ref (will pull after stream completes)"
                            );
                            pending_refs.push(ref_msg);
                        }
                    }

                    if is_final {
                        break;
                    }
                }
                Ok(None) => {
                    debug!(plugin = %plugin_id, "guest plugin task stream ended");
                    break;
                }
                Err(status) => {
                    return Err(ManagerError::ExecutionFailed(
                        plugin_id.clone(),
                        format!("gRPC stream error: {}", status),
                    ));
                }
            }
        }

        let mut outputs = inline_outputs;
        for ref_msg in pending_refs {
            let pulled = pull_result_chunks(&mut self.grpc_client, plugin_id, &ref_msg).await?;
            outputs.push(pulled);
        }

        Ok(outputs)
    }

    async fn check_health(&mut self) -> bool {
        let timeout = Duration::from_secs(5);
        matches!(
            tokio::time::timeout(timeout, self.grpc_client.health_check()).await,
            Ok(Ok(_))
        )
    }
}

impl PluginRuntime {
    pub async fn execute_task(
        &mut self,
        plugin_id: &PluginId,
        entry: &PluginEntry,
        task_id: i32,
        sample_path: &str,
        config: HashMap<String, String>,
    ) -> Result<Vec<PluginOutput>> {
        match self {
            Self::Host(h) => {
                h.execute_task(plugin_id, entry, task_id, sample_path, config)
                    .await
            }
            Self::Guest(g) => {
                g.execute_task(plugin_id, entry, task_id, sample_path, config)
                    .await
            }
        }
    }

    pub async fn check_health(&mut self) -> bool {
        match self {
            Self::Host(h) => h.check_health().await,
            Self::Guest(g) => g.check_health().await,
        }
    }
}

async fn pull_result_chunks(
    client: &mut GrpcClient,
    plugin_id: &PluginId,
    ref_msg: &crate::transport::grpc::proto::ResultRef,
) -> Result<PluginOutput> {
    use crate::transport::grpc::proto::ResultFormat as ProtoFormat;
    use tracing::info;

    let mut stream = client
        .pull_result(ref_msg.handle.clone())
        .await
        .map_err(|e| {
            ManagerError::ExecutionFailed(
                plugin_id.clone(),
                format!(
                    "pull_result RPC for '{}' (handle {}) failed: {}",
                    ref_msg.result_name, ref_msg.handle, e
                ),
            )
        })?;

    let mut buf: Vec<u8> = Vec::with_capacity(ref_msg.size_bytes as usize);
    loop {
        match stream.message().await {
            Ok(Some(chunk)) => {
                buf.extend_from_slice(&chunk.data);
                if chunk.is_last {
                    break;
                }
            }
            Ok(None) => break,
            Err(status) => {
                return Err(ManagerError::ExecutionFailed(
                    plugin_id.clone(),
                    format!(
                        "pull_result stream error for '{}' (handle {}): {}",
                        ref_msg.result_name, ref_msg.handle, status
                    ),
                ));
            }
        }
    }

    let format = match ProtoFormat::try_from(ref_msg.format) {
        Ok(ProtoFormat::Json) => OutputFormat::Json,
        _ => OutputFormat::Bytes,
    };

    info!(
        plugin = %plugin_id,
        result_name = %ref_msg.result_name,
        bytes = buf.len(),
        "pulled large result from guest plugin"
    );

    Ok(PluginOutput {
        result_name: ref_msg.result_name.clone(),
        data: buf,
        format,
    })
}
