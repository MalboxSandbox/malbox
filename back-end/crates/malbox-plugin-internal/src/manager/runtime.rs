//! Host (IPC) and guest (gRPC) runtime backends behind a plugin instance.

use std::collections::HashMap;
use std::time::Duration;

use tokio::sync::oneshot;
use tracing::{debug, info};

use crate::manager::error::{ManagerError, Result};
use crate::manager::handle::{OutputFormat, PluginOutput};
use crate::manager::ipc_reactor::IpcReactorHandle;
use crate::registry::types::{PluginEntry, PluginId};
use crate::transport::daemon::GrpcClient;
use crate::transport::grpc::conversions::artifact_meta_from_proto;
use crate::transport::grpc::proto;

/// Polymorphic operations shared by both host and guest runtimes.
///
/// Not used as `dyn PluginExecution` - the `PluginRuntime` enum delegates
/// via match arms. The trait enforces signature parity between impls and
/// documents the contract.
pub(crate) trait PluginExecution: Send {
    /// Run one task to completion and return every output the plugin produced.
    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors the full task dispatch context; both runtimes need each piece"
    )]
    async fn execute_task(
        &mut self,
        plugin_id: &PluginId,
        entry: &PluginEntry,
        task_id: i32,
        sample_path: &str,
        config: HashMap<String, String>,
        timeout: Duration,
        ready: Option<oneshot::Sender<()>>,
    ) -> Result<Vec<PluginOutput>>;

    /// Whether the plugin is alive and responsive.
    async fn check_health(&mut self) -> bool;
}

/// Host plugin runtime: child process + a handle to the IPC reactor.
pub(crate) struct HostRuntime {
    /// Spawned plugin child process.
    pub(crate) process: tokio::process::Child,
    /// Handle used to dispatch tasks through the daemon IPC reactor.
    pub(crate) ipc: IpcReactorHandle,
}

/// Guest plugin runtime: gRPC client connection used for the task stream,
/// health checks and result pulls. Control RPCs (`StopTask`) go through the
/// second connection held by the [`PluginHandle`](crate::manager::handle::PluginHandle).
pub(crate) struct GuestRuntime {
    pub(crate) grpc_client: GrpcClient,
}

/// Type-discriminated runtime state for a plugin instance.
pub(crate) enum PluginRuntime {
    /// Native process on the daemon host, driven over IPC.
    Host(HostRuntime),
    /// Plugin inside a VM, driven over gRPC.
    Guest(GuestRuntime),
}

impl PluginExecution for HostRuntime {
    async fn execute_task(
        &mut self,
        plugin_id: &PluginId,
        _entry: &PluginEntry,
        task_id: i32,
        sample_path: &str,
        config: HashMap<String, String>,
        timeout: Duration,
        _ready: Option<oneshot::Sender<()>>,
    ) -> Result<Vec<PluginOutput>> {
        debug!(plugin = %plugin_id, task_id, "executing task on host plugin via IPC reactor");

        self.ipc
            .execute_task(plugin_id, task_id, sample_path.to_string(), config, timeout)
            .await
    }

    async fn check_health(&mut self) -> bool {
        matches!(self.process.try_wait(), Ok(None))
    }
}

fn output_format(format: i32) -> OutputFormat {
    match proto::ResultFormat::try_from(format) {
        Ok(proto::ResultFormat::Json) => OutputFormat::Json,
        _ => OutputFormat::Bytes,
    }
}

impl GuestRuntime {
    /// Consume the task stream until the final marker, collecting inline
    /// outputs and result refs to pull afterwards.
    async fn collect_stream(
        plugin_id: &PluginId,
        stream: &mut tonic::Streaming<proto::TaskResult>,
        mut ready: Option<oneshot::Sender<()>>,
    ) -> Result<(Vec<PluginOutput>, Vec<proto::ResultRef>)> {
        use crate::transport::grpc::proto::ResultKind;
        use prost::Message;

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
                            if let Some(tx) = ready.take() {
                                let _ = tx.send(());
                            }
                        }
                        ResultKind::Event => {
                            debug!(plugin = %plugin_id, task_id = result.task_id, "dropping event message from guest stream");
                        }
                        ResultKind::Result => {
                            info!(
                                plugin = %plugin_id,
                                task_id = result.task_id,
                                result_name = %result.result_name,
                                data_len = result.data.len(),
                                is_final,
                                "received inline task result from guest plugin"
                            );

                            if !result.result_name.is_empty() {
                                inline_outputs.push(PluginOutput {
                                    format: output_format(result.format),
                                    artifact: result
                                        .artifact
                                        .as_ref()
                                        .and_then(artifact_meta_from_proto),
                                    result_name: result.result_name,
                                    data: result.data,
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
                        return Ok((inline_outputs, pending_refs));
                    }
                }
                Ok(None) => {
                    debug!(plugin = %plugin_id, "guest plugin task stream ended without final marker");
                    return Err(ManagerError::Crashed(plugin_id.clone()));
                }
                Err(status) => {
                    return Err(ManagerError::ExecutionFailed(
                        plugin_id.clone(),
                        format!("gRPC stream error: {status}"),
                    ));
                }
            }
        }
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
        timeout: Duration,
        ready: Option<oneshot::Sender<()>>,
    ) -> Result<Vec<PluginOutput>> {
        debug!(plugin = %plugin_id, task_id, "executing task on guest plugin via gRPC");

        let collected = tokio::time::timeout(timeout, async {
            let mut stream = self
                .grpc_client
                .execute_task(task_id, sample_path.to_string(), config)
                .await
                .map_err(|e| ManagerError::ExecutionFailed(plugin_id.clone(), e.to_string()))?;
            Self::collect_stream(plugin_id, &mut stream, ready).await
        })
        .await
        .map_err(|_| ManagerError::Timeout(plugin_id.clone()))?;

        let (mut outputs, pending_refs) = collected?;
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
    /// Dispatch a task to whichever runtime backs this instance.
    #[expect(
        clippy::too_many_arguments,
        reason = "forwards the full PluginExecution::execute_task context"
    )]
    pub(crate) async fn execute_task(
        &mut self,
        plugin_id: &PluginId,
        entry: &PluginEntry,
        task_id: i32,
        sample_path: &str,
        config: HashMap<String, String>,
        timeout: Duration,
        ready: Option<oneshot::Sender<()>>,
    ) -> Result<Vec<PluginOutput>> {
        match self {
            Self::Host(h) => {
                h.execute_task(
                    plugin_id,
                    entry,
                    task_id,
                    sample_path,
                    config,
                    timeout,
                    ready,
                )
                .await
            }
            Self::Guest(g) => {
                g.execute_task(
                    plugin_id,
                    entry,
                    task_id,
                    sample_path,
                    config,
                    timeout,
                    ready,
                )
                .await
            }
        }
    }

    /// Whether the plugin is alive and responsive.
    pub(crate) async fn check_health(&mut self) -> bool {
        match self {
            Self::Host(h) => h.check_health().await,
            Self::Guest(g) => g.check_health().await,
        }
    }
}

async fn pull_result_chunks(
    client: &mut GrpcClient,
    plugin_id: &PluginId,
    ref_msg: &proto::ResultRef,
) -> Result<PluginOutput> {
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

    // The size is only a capacity hint; skip preallocation if it does not fit.
    let mut buf: Vec<u8> = Vec::with_capacity(usize::try_from(ref_msg.size_bytes).unwrap_or(0));
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

    info!(
        plugin = %plugin_id,
        result_name = %ref_msg.result_name,
        bytes = buf.len(),
        "pulled large result from guest plugin"
    );

    Ok(PluginOutput {
        result_name: ref_msg.result_name.clone(),
        data: buf,
        format: output_format(ref_msg.format),
        artifact: ref_msg.artifact.as_ref().and_then(artifact_meta_from_proto),
    })
}
