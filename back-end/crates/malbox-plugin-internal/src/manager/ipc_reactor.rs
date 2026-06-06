//! Daemon-side IPC reactor.
//!
//! A dedicated OS thread owns every daemon-side iceoryx2 port (single-threaded
//! `ipc::Service` variant, `!Send`) and bridges them to the async world
//! through a command mpsc plus a threadsafe wakeup notifier. Nothing `!Send`
//! ever crosses a thread boundary.
//!
//! The reactor blocks in `WaitSet::wait_and_process` with three attachments:
//! a command wakeup listener, the shared plugin -> daemon notify listener, and
//! a 100 ms interval for deadline sweeps and crashed-plugin disconnect
//! detection. On any wakeup it drains commands, pumps all active tasks, and
//! checks deadlines.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info, warn};

use malbox_plugin_transport::error::TransportError;
use malbox_plugin_transport::ipc::headers::{
    EventKind, FLAG_HAS_MORE_CHUNKS, FLAG_IS_FINAL, ResponseKind, ResultFormat, TaskRequestHeader,
};
use malbox_plugin_transport::ipc::notify::NotifyKind;
use malbox_plugin_transport::ipc::{
    CallbackProgression, DaemonEventPublisher, DaemonNotifyListener, IpcService, Node, NodeBuilder,
    PluginEventSubscriber, PluginNotifier, ReactorWaker, ReactorWakeupListener, SignalHandlingMode,
    TaskClient, TaskPendingResponse, WaitSetBuilder,
};
use malbox_plugin_transport::messages::events::Event;
use malbox_plugin_transport::traits::TransportEmitter;

use crate::manager::error::{ManagerError, Result};
use crate::manager::handle::{OutputFormat, PluginOutput};
use crate::registry::types::PluginId;

/// Tick driving deadline sweeps and disconnect detection. Data-path latency
/// does not depend on this: every send is followed by a notify wake.
const TICK: Duration = Duration::from_millis(100);

/// Postcard-serialized task request payload (daemon side).
#[derive(serde::Serialize)]
struct TaskRequestPayload<'a> {
    pub sample_path: &'a str,
    pub config: Vec<(&'a str, &'a str)>,
}

/// Commands accepted by the reactor thread.
pub enum IpcCommand {
    ExecuteTask {
        plugin_id: PluginId,
        task_id: i32,
        sample_path: String,
        config: HashMap<String, String>,
        timeout: Duration,
        reply: oneshot::Sender<Result<Vec<PluginOutput>>>,
    },
    EmitEvent(Event),
    /// Clean up iceoryx2 resources left by dead plugin processes. Awaited by
    /// the caller so the cleanup is ordered before an ephemeral respawn.
    CleanupDeadNodes {
        reply: oneshot::Sender<()>,
    },
    /// Drop a plugin's channels (instance removed from the registry or the
    /// manager is shutting down). Fails any in-flight task for that plugin.
    RemoveChannels(PluginId),
    Shutdown,
}

/// Cloneable, `Send + Sync` handle used by the async world.
#[derive(Clone)]
pub struct IpcReactorHandle {
    tx: mpsc::UnboundedSender<IpcCommand>,
    waker: Arc<ReactorWaker>,
}

impl IpcReactorHandle {
    /// Queue a command and wake the reactor thread.
    fn send(&self, cmd: IpcCommand) -> std::result::Result<(), ()> {
        if self.tx.send(cmd).is_err() {
            return Err(());
        }
        // A failed wake is non-fatal: the reactor's interval tick also
        // drains the command queue.
        if let Err(e) = self.waker.wake() {
            warn!(error = %e, "reactor wake failed, command picked up on next tick");
        }
        Ok(())
    }

    /// Dispatch a task to a host plugin and await its streamed outputs.
    pub async fn execute_task(
        &self,
        plugin_id: &PluginId,
        task_id: i32,
        sample_path: String,
        config: HashMap<String, String>,
        timeout: Duration,
    ) -> Result<Vec<PluginOutput>> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.send(IpcCommand::ExecuteTask {
            plugin_id: plugin_id.clone(),
            task_id,
            sample_path,
            config,
            timeout,
            reply: reply_tx,
        })
        .map_err(|_| reactor_gone(plugin_id))?;

        reply_rx.await.map_err(|_| reactor_gone(plugin_id))?
    }

    /// Clean up stale iceoryx2 resources from dead plugin processes.
    /// Completes once the reactor has performed the cleanup.
    pub async fn cleanup_dead_nodes(&self) {
        let (reply_tx, reply_rx) = oneshot::channel();
        if self
            .send(IpcCommand::CleanupDeadNodes { reply: reply_tx })
            .is_ok()
        {
            let _ = reply_rx.await;
        }
    }

    /// Drop the channels for a plugin whose instance went away.
    pub fn remove_channels(&self, plugin_id: PluginId) {
        let _ = self.send(IpcCommand::RemoveChannels(plugin_id));
    }

    /// Ask the reactor thread to exit. Join the handle returned by
    /// [`IpcReactor::spawn`] afterwards.
    pub fn shutdown(&self) {
        let _ = self.send(IpcCommand::Shutdown);
    }
}

impl TransportEmitter for IpcReactorHandle {
    fn emit(&self, event: Event) -> malbox_plugin_transport::error::Result<()> {
        self.send(IpcCommand::EmitEvent(event))
            .map_err(|_| TransportError::NotConnected("IPC reactor unavailable".into()))
    }
}

fn reactor_gone(plugin_id: &PluginId) -> ManagerError {
    ManagerError::ExecutionFailed(plugin_id.clone(), "IPC reactor unavailable".into())
}

/// Spawns and owns the reactor thread.
pub struct IpcReactor;

impl IpcReactor {
    /// Spawn the reactor thread. Returns once the thread has created its
    /// node, services, and WaitSet (or propagates the startup error).
    pub fn spawn() -> Result<(IpcReactorHandle, std::thread::JoinHandle<()>)> {
        let (tx, rx) = mpsc::unbounded_channel();
        let (ready_tx, ready_rx) =
            std::sync::mpsc::sync_channel::<std::result::Result<(), String>>(1);

        let join = std::thread::Builder::new()
            .name("malbox-ipc-reactor".into())
            .spawn(move || reactor_thread(rx, ready_tx))
            .map_err(ManagerError::Io)?;

        ready_rx
            .recv()
            .map_err(|_| ManagerError::Reactor("reactor thread died during startup".into()))?
            .map_err(ManagerError::Reactor)?;

        // Opens the wakeup service the reactor just created.
        let waker = Arc::new(ReactorWaker::new().map_err(ManagerError::Transport)?);

        Ok((IpcReactorHandle { tx, waker }, join))
    }
}

/// Thread body: build all `!Send` state, signal readiness, run the loop.
fn reactor_thread(
    command_rx: mpsc::UnboundedReceiver<IpcCommand>,
    ready_tx: std::sync::mpsc::SyncSender<std::result::Result<(), String>>,
) {
    macro_rules! try_init {
        ($expr:expr, $what:literal) => {
            match $expr {
                Ok(v) => v,
                Err(e) => {
                    let _ = ready_tx.send(Err(format!(concat!($what, ": {}"), e)));
                    return;
                }
            }
        };
    }

    let node = try_init!(
        NodeBuilder::new()
            .signal_handling_mode(SignalHandlingMode::Disabled)
            .create::<IpcService>(),
        "failed to create IPC node"
    );

    let emitter = try_init!(
        DaemonEventPublisher::new(&node),
        "failed to create event publisher"
    );
    let wakeup_listener = try_init!(
        ReactorWakeupListener::new(&node),
        "failed to create wakeup listener"
    );
    let notify_listener = try_init!(
        DaemonNotifyListener::new(&node),
        "failed to create daemon notify listener"
    );
    let waitset = try_init!(
        WaitSetBuilder::new().create::<IpcService>(),
        "failed to create WaitSet"
    );
    let wakeup_guard = try_init!(
        waitset.attach_notification(wakeup_listener.listener()),
        "failed to attach wakeup listener"
    );
    let notify_guard = try_init!(
        waitset.attach_notification(notify_listener.listener()),
        "failed to attach notify listener"
    );
    let _tick_guard = try_init!(waitset.attach_interval(TICK), "failed to attach interval");

    let _ = ready_tx.send(Ok(()));

    let mut reactor = Reactor {
        node,
        emitter,
        command_rx,
        channels: HashMap::new(),
        active: Vec::new(),
        shutdown: false,
    };

    info!("IPC reactor running");

    let result = waitset.wait_and_process(|id| {
        if id.has_event_from(&wakeup_guard) {
            let _ = wakeup_listener.drain();
        } else if id.has_event_from(&notify_guard) {
            let _ = notify_listener.drain();
        }

        // Uniform handling regardless of wake source: drain commands, pump
        // every active task, sweep deadlines.
        reactor.drain_commands();
        reactor.pump_tasks();

        if reactor.shutdown {
            CallbackProgression::Stop
        } else {
            CallbackProgression::Continue
        }
    });

    if let Err(e) = result {
        error!(error = ?e, "reactor WaitSet error");
    }

    reactor.fail_all_inflight();
    info!("IPC reactor exited");
}

/// Daemon-side ports for one plugin. All single-threaded; reactor-confined.
struct PluginChannels {
    task_client: TaskClient,
    plugin_notifier: PluginNotifier,
    event_sub: PluginEventSubscriber,
}

impl PluginChannels {
    fn new(node: &Node<IpcService>, plugin_id: &str) -> Result<Self> {
        Ok(Self {
            task_client: TaskClient::new(node, plugin_id).map_err(ManagerError::Transport)?,
            plugin_notifier: PluginNotifier::new(node, plugin_id)
                .map_err(ManagerError::Transport)?,
            event_sub: PluginEventSubscriber::new(node, plugin_id)
                .map_err(ManagerError::Transport)?,
        })
    }
}

/// Accumulator for streamed responses, ported from the old collect_responses.
struct ChunkAccum {
    outputs: Vec<PluginOutput>,
    chunked_name: Option<String>,
    chunked_data: Vec<u8>,
    chunked_format: OutputFormat,
}

impl ChunkAccum {
    fn new() -> Self {
        Self {
            outputs: Vec::new(),
            chunked_name: None,
            chunked_data: Vec::new(),
            chunked_format: OutputFormat::Bytes,
        }
    }

    /// Finish any in-progress chunked transfer and move it into `outputs`.
    fn flush_chunked(&mut self) {
        if let Some(name) = self.chunked_name.take() {
            self.outputs.push(PluginOutput {
                result_name: name,
                data: std::mem::take(&mut self.chunked_data),
                format: self.chunked_format,
            });
        }
    }
}

enum TaskState {
    /// Request not yet delivered: the (ephemeral) plugin's task server was
    /// not connected at send time. Resent when PluginStarted arrives.
    WaitingReady {
        header: TaskRequestHeader,
        payload: Vec<u8>,
    },
    /// Request delivered; pumping streamed responses.
    Collecting {
        pending: TaskPendingResponse,
        accum: ChunkAccum,
        server_connected: bool,
    },
}

struct ActiveTask {
    plugin_id: PluginId,
    task_id: i32,
    deadline: Instant,
    reply: oneshot::Sender<Result<Vec<PluginOutput>>>,
    state: TaskState,
}

enum TaskOutcome {
    Pending,
    Finished(Result<Vec<PluginOutput>>),
}

/// Reactor state. Lives entirely on the reactor thread.
struct Reactor {
    node: Node<IpcService>,
    emitter: DaemonEventPublisher,
    command_rx: mpsc::UnboundedReceiver<IpcCommand>,
    channels: HashMap<PluginId, PluginChannels>,
    active: Vec<ActiveTask>,
    shutdown: bool,
}

impl Reactor {
    fn drain_commands(&mut self) {
        loop {
            match self.command_rx.try_recv() {
                Ok(cmd) => self.handle_command(cmd),
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    // Every handle is gone: nothing can reach us anymore.
                    self.shutdown = true;
                    break;
                }
            }
        }
    }

    fn handle_command(&mut self, cmd: IpcCommand) {
        match cmd {
            IpcCommand::ExecuteTask {
                plugin_id,
                task_id,
                sample_path,
                config,
                timeout,
                reply,
            } => self.handle_execute(plugin_id, task_id, sample_path, config, timeout, reply),
            IpcCommand::EmitEvent(event) => {
                if let Err(e) = self.emitter.emit(event) {
                    error!(error = %e, "failed to emit daemon event");
                }
            }
            IpcCommand::CleanupDeadNodes { reply } => {
                Node::<IpcService>::try_cleanup_dead_nodes(self.node.config());
                let _ = reply.send(());
            }
            IpcCommand::RemoveChannels(plugin_id) => {
                // Fail any in-flight task for this plugin before dropping ports.
                let mut i = 0;
                while i < self.active.len() {
                    if self.active[i].plugin_id == plugin_id {
                        let task = self.active.swap_remove(i);
                        let _ = task.reply.send(Err(ManagerError::ExecutionFailed(
                            plugin_id.clone(),
                            "plugin stopped while task in flight".into(),
                        )));
                    } else {
                        i += 1;
                    }
                }
                self.channels.remove(&plugin_id);
            }
            IpcCommand::Shutdown => self.shutdown = true,
        }
    }

    fn handle_execute(
        &mut self,
        plugin_id: PluginId,
        task_id: i32,
        sample_path: String,
        config: HashMap<String, String>,
        timeout: Duration,
        reply: oneshot::Sender<Result<Vec<PluginOutput>>>,
    ) {
        if !self.channels.contains_key(&plugin_id) {
            match PluginChannels::new(&self.node, plugin_id.as_str()) {
                Ok(channels) => {
                    self.channels.insert(plugin_id.clone(), channels);
                }
                Err(e) => {
                    let _ = reply.send(Err(e));
                    return;
                }
            }
        }
        let channels = self.channels.get(&plugin_id).expect("just inserted");

        // Discard stale events (e.g. PluginStarted from a previous ephemeral
        // generation) so WaitingReady only reacts to fresh announcements.
        // Replaces the old fresh-subscriber-per-instance semantics. Recv
        // errors are non-fatal here; a dead subscriber surfaces as a task
        // timeout.
        while let Ok(Some(_)) = channels.event_sub.try_recv() {}

        let config_vec: Vec<(&str, &str)> = config
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let payload = match postcard::to_allocvec(&TaskRequestPayload {
            sample_path: &sample_path,
            config: config_vec,
        }) {
            Ok(p) => p,
            Err(e) => {
                let _ = reply.send(Err(ManagerError::ExecutionFailed(
                    plugin_id.clone(),
                    format!("failed to serialize task request: {e}"),
                )));
                return;
            }
        };

        let header = TaskRequestHeader {
            task_id,
            request_type: 0,
            _reserved: [0; 3],
        };
        let deadline = Instant::now() + timeout;

        // In iceoryx2 request-response, a request sent while no server is
        // connected is delivered to zero servers and silently lost. Detect
        // that (ephemeral plugin still booting) and park until PluginStarted.
        let state = match channels.task_client.send(&header, &payload) {
            Ok(pending) if pending.is_connected() => {
                let _ = channels.plugin_notifier.wake(NotifyKind::TaskAvailable);
                TaskState::Collecting {
                    pending,
                    accum: ChunkAccum::new(),
                    // Only flipped once a real response arrives; a connected
                    // send alone must not arm the crashed-mid-stream flush.
                    server_connected: false,
                }
            }
            Ok(_) => {
                debug!(
                    plugin = %plugin_id,
                    task_id,
                    "task server not connected, waiting for plugin to start"
                );
                TaskState::WaitingReady { header, payload }
            }
            Err(e) => {
                let _ = reply.send(Err(ManagerError::ExecutionFailed(
                    plugin_id.clone(),
                    format!("failed to send task request: {e}"),
                )));
                return;
            }
        };

        self.active.push(ActiveTask {
            plugin_id,
            task_id,
            deadline,
            reply,
            state,
        });
    }

    fn pump_tasks(&mut self) {
        let mut i = 0;
        while i < self.active.len() {
            match self.pump_one(i) {
                TaskOutcome::Pending => i += 1,
                TaskOutcome::Finished(result) => {
                    let task = self.active.swap_remove(i);
                    let _ = task.reply.send(result);
                }
            }
        }
    }

    fn pump_one(&mut self, idx: usize) -> TaskOutcome {
        let task = &mut self.active[idx];

        if Instant::now() > task.deadline {
            let msg = match task.state {
                TaskState::WaitingReady { .. } => format!(
                    "task {}: plugin task server did not become ready within timeout",
                    task.task_id
                ),
                TaskState::Collecting { .. } => format!("task {} timed out", task.task_id),
            };
            return TaskOutcome::Finished(Err(ManagerError::ExecutionFailed(
                task.plugin_id.clone(),
                msg,
            )));
        }

        let Some(channels) = self.channels.get(&task.plugin_id) else {
            return TaskOutcome::Finished(Err(ManagerError::ExecutionFailed(
                task.plugin_id.clone(),
                "plugin channels removed while task in flight".into(),
            )));
        };

        match &mut task.state {
            TaskState::WaitingReady { header, payload } => {
                let mut started = false;
                while let Ok(Some(event)) = channels.event_sub.try_recv() {
                    if EventKind::try_from(event.header.event_type) == Ok(EventKind::PluginStarted)
                    {
                        started = true;
                    }
                }
                if !started {
                    return TaskOutcome::Pending;
                }

                debug!(
                    plugin = %task.plugin_id,
                    task_id = task.task_id,
                    "plugin ready, sending task request"
                );
                match channels.task_client.send(header, payload) {
                    Ok(pending) if pending.is_connected() => {
                        let _ = channels.plugin_notifier.wake(NotifyKind::TaskAvailable);
                        task.state = TaskState::Collecting {
                            pending,
                            accum: ChunkAccum::new(),
                            // Only flipped once a real response arrives; see
                            // handle_execute.
                            server_connected: false,
                        };
                        // Responses cannot exist yet; the plugin's sends will
                        // notify us.
                        TaskOutcome::Pending
                    }
                    // Stale or premature PluginStarted: server still absent.
                    // The undelivered pending is dropped; stay parked.
                    Ok(_) => TaskOutcome::Pending,
                    Err(e) => TaskOutcome::Finished(Err(ManagerError::ExecutionFailed(
                        task.plugin_id.clone(),
                        format!("failed to send task request after readiness wait: {e}"),
                    ))),
                }
            }
            TaskState::Collecting {
                pending,
                accum,
                server_connected,
            } => Self::pump_collecting(
                &task.plugin_id,
                task.task_id,
                pending,
                accum,
                server_connected,
            ),
        }
    }

    /// Drain all currently available responses for one task. Ported from the
    /// old `collect_responses` loop body (minus the sleep-polling).
    fn pump_collecting(
        plugin_id: &PluginId,
        task_id: i32,
        pending: &TaskPendingResponse,
        accum: &mut ChunkAccum,
        server_connected: &mut bool,
    ) -> TaskOutcome {
        loop {
            match pending.try_recv() {
                Ok(Some(response)) => {
                    *server_connected = true;
                    let h = &response.header;

                    if h.flags & FLAG_IS_FINAL != 0 {
                        accum.flush_chunked();
                        return TaskOutcome::Finished(Ok(std::mem::take(&mut accum.outputs)));
                    }

                    let kind = ResponseKind::try_from(h.kind).unwrap_or(ResponseKind::Result);

                    match kind {
                        ResponseKind::Result => {
                            let format = match ResultFormat::try_from(h.format) {
                                Ok(ResultFormat::Json) => OutputFormat::Json,
                                _ => OutputFormat::Bytes,
                            };

                            let has_more = h.flags & FLAG_HAS_MORE_CHUNKS != 0;

                            let name_len = h.name_len as usize;
                            let (result_name, data) = if name_len > 0
                                && response.payload.len() >= name_len
                            {
                                let name = String::from_utf8_lossy(&response.payload[..name_len])
                                    .into_owned();
                                let data = response.payload[name_len..].to_vec();
                                (name, data)
                            } else {
                                (String::new(), response.payload)
                            };

                            if has_more {
                                if h.chunk_index == 0 {
                                    accum.flush_chunked();
                                    accum.chunked_name = Some(result_name);
                                    accum.chunked_data = data;
                                    accum.chunked_format = format;
                                    if h.total_size > 0 {
                                        accum.chunked_data.reserve(h.total_size as usize);
                                    }
                                } else {
                                    accum.chunked_data.extend_from_slice(&data);
                                }
                            } else if accum.chunked_name.is_some() {
                                // Final chunk of an in-progress transfer.
                                accum.chunked_data.extend_from_slice(&data);
                                accum.flush_chunked();
                            } else {
                                debug!(
                                    plugin = %plugin_id,
                                    task_id,
                                    %result_name,
                                    data_len = data.len(),
                                    "received result"
                                );
                                accum.outputs.push(PluginOutput {
                                    result_name,
                                    data,
                                    format,
                                });
                            }
                        }
                        ResponseKind::FileRef => {
                            let path = String::from_utf8_lossy(&response.payload).to_string();
                            match std::fs::read(&path) {
                                Ok(data) => {
                                    let format = match ResultFormat::try_from(h.format) {
                                        Ok(ResultFormat::Json) => OutputFormat::Json,
                                        _ => OutputFormat::Bytes,
                                    };
                                    accum.outputs.push(PluginOutput {
                                        result_name: path
                                            .rsplit('/')
                                            .next()
                                            .unwrap_or(&path)
                                            .to_string(),
                                        data,
                                        format,
                                    });
                                }
                                Err(e) => {
                                    warn!(
                                        plugin = %plugin_id,
                                        task_id,
                                        path = %path,
                                        error = %e,
                                        "failed to read file ref"
                                    );
                                }
                            }
                        }
                        ResponseKind::Progress => {
                            debug!(plugin = %plugin_id, task_id, "progress update");
                        }
                        ResponseKind::Ready => {
                            debug!(plugin = %plugin_id, task_id, "plugin ready");
                        }
                        ResponseKind::Error => {
                            let msg = String::from_utf8_lossy(&response.payload);
                            return TaskOutcome::Finished(Err(ManagerError::ExecutionFailed(
                                plugin_id.clone(),
                                format!("plugin task error: {msg}"),
                            )));
                        }
                    }
                }
                Ok(None) => {
                    if pending.is_connected() {
                        *server_connected = true;
                        return TaskOutcome::Pending;
                    }
                    if *server_connected {
                        // Server finished or crashed without a final marker:
                        // flush whatever arrived.
                        accum.flush_chunked();
                        return TaskOutcome::Finished(Ok(std::mem::take(&mut accum.outputs)));
                    }
                    return TaskOutcome::Pending;
                }
                Err(e) => {
                    return TaskOutcome::Finished(Err(ManagerError::ExecutionFailed(
                        plugin_id.clone(),
                        format!("response receive error: {e}"),
                    )));
                }
            }
        }
    }

    /// Fail every in-flight task (used on reactor exit).
    fn fail_all_inflight(&mut self) {
        for task in self.active.drain(..) {
            let _ = task.reply.send(Err(ManagerError::ExecutionFailed(
                task.plugin_id,
                "daemon shutting down".into(),
            )));
        }
    }
}
