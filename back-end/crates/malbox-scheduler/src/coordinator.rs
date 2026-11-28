//! Routes commands to task actors, spawning actors for active tasks, and
//! owns startup recovery and shutdown.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use malbox_database::repositories::{jobs, result_cache, tasks};
use malbox_plugin_transport::messages::events::Event;
use tokio::sync::{Notify, mpsc, oneshot};
use tokio::task::{JoinHandle, JoinSet};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::actor::{ActorEvent, ControlCmd, JobFinished, run_actor};
use crate::config::EffectiveLimits;
use crate::error::Result;
use crate::executor::{self, Executors};
use crate::runner::{DetonationRunner, HostJobRunner};
use crate::running::RunningJobs;
use crate::submission::task_params;
use crate::{ControlError, OrchestratorCore, OrchestratorHandle, RoutingTable, Shared};

const CACHE_PURGE_INTERVAL: Duration = Duration::from_hours(1);
const SHUTDOWN_GRACE: Duration = Duration::from_secs(20);

pub(crate) enum Command {
    Submit(i32),
    Control(i32, ControlCmd, oneshot::Sender<Result<(), ControlError>>),
    JobFinished(i32, JobFinished),
    JobRetrying(i32, i64),
    /// An actor finished its task. `leftover` holds events it never read.
    ActorExited {
        task_id: i32,
        generation: u64,
        leftover: Vec<ActorEvent>,
    },
}

struct Coordinator {
    shared: Arc<Shared>,
    actors: HashMap<i32, (u64, mpsc::UnboundedSender<ActorEvent>)>,
    next_generation: u64,
    actor_tasks: JoinSet<()>,
}

pub(crate) async fn launch(
    core: OrchestratorCore,
    routing: Arc<RoutingTable>,
    host: Arc<dyn HostJobRunner>,
    detonation: Arc<dyn DetonationRunner>,
    machine_available: Option<Arc<Notify>>,
    token: CancellationToken,
) -> Result<(OrchestratorHandle, JoinHandle<()>)> {
    let core = Arc::new(core);
    let (commands, rx) = mpsc::unbounded_channel();
    let shared = Arc::new(Shared {
        core: Arc::clone(&core),
        routing: Arc::clone(&routing),
        running: RunningJobs::new(token.clone()),
        executors: Executors::default(),
        commands: commands.clone(),
        token,
    });

    let purged = result_cache::purge_expired(&core.db).await?;
    let recovered = jobs::recover_running_jobs(&core.db).await?;
    info!(purged, recovered, "orchestrator recovered state");

    let executors = vec![
        tokio::spawn(executor::host::run(Arc::clone(&shared), host)),
        tokio::spawn(executor::detonation::run(
            Arc::clone(&shared),
            detonation,
            machine_available,
        )),
    ];
    let mut coordinator = Coordinator {
        shared,
        actors: HashMap::new(),
        next_generation: 0,
        actor_tasks: JoinSet::new(),
    };
    for task_id in tasks::fetch_active_task_ids(&core.db).await? {
        coordinator.spawn(task_id);
    }
    let join = tokio::spawn(coordinator.run(rx, executors));
    Ok((
        OrchestratorHandle {
            commands,
            routing,
            core,
        },
        join,
    ))
}

impl Coordinator {
    async fn run(
        mut self,
        mut rx: mpsc::UnboundedReceiver<Command>,
        executors: Vec<JoinHandle<()>>,
    ) {
        let token = self.shared.token.clone();
        let mut purge = tokio::time::interval(CACHE_PURGE_INTERVAL);
        purge.tick().await;
        loop {
            tokio::select! {
                () = token.cancelled() => break,
                cmd = rx.recv() => match cmd {
                    Some(cmd) => self.handle(cmd).await,
                    None => break,
                },
                _ = purge.tick() => {
                    if let Err(e) = result_cache::purge_expired(&self.shared.core.db).await {
                        warn!(error = %e, "failed to purge result cache");
                    }
                }
                Some(res) = self.actor_tasks.join_next() => {
                    if let Err(e) = res {
                        error!(error = %e, "task actor panicked");
                    }
                }
            }
        }

        info!("orchestrator shutting down");
        self.shared.running.cancel_all();
        let actor_tasks = &mut self.actor_tasks;
        let drained = tokio::time::timeout(SHUTDOWN_GRACE, async {
            for handle in executors {
                if let Err(e) = handle.await {
                    error!(error = %e, "executor panicked");
                }
            }
            while actor_tasks.join_next().await.is_some() {}
        })
        .await;
        if drained.is_err() {
            warn!("orchestrator shutdown grace elapsed; unfinished jobs are recovered on restart");
            self.actor_tasks.abort_all();
        }
    }

    fn spawn(&mut self, task_id: i32) -> mpsc::UnboundedSender<ActorEvent> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.next_generation += 1;
        let generation = self.next_generation;
        self.actor_tasks
            .spawn(run_actor(Arc::clone(&self.shared), task_id, generation, rx));
        self.actors.insert(task_id, (generation, tx.clone()));
        tx
    }

    async fn handle(&mut self, cmd: Command) {
        match cmd {
            Command::Submit(task_id) => {
                if self.actors.contains_key(&task_id) {
                    debug!(task_id, "task already active");
                    return;
                }
                if let Err(e) = self.shared.core.events.emit(Event::TaskCreated { task_id }) {
                    warn!(task_id, error = %e, "failed to emit task event");
                }
                self.spawn(task_id);
            }
            Command::Control(task_id, cmd, reply) => {
                self.dispatch(task_id, ActorEvent::Control(cmd, reply))
                    .await;
            }
            Command::JobFinished(task_id, finished) => {
                self.dispatch(task_id, ActorEvent::JobFinished(finished))
                    .await;
            }
            Command::JobRetrying(task_id, job_id) => {
                if let Some((_, tx)) = self.actors.get(&task_id) {
                    let _ = tx.send(ActorEvent::JobRetrying { job_id });
                }
            }
            Command::ActorExited {
                task_id,
                generation,
                leftover,
            } => {
                if self
                    .actors
                    .get(&task_id)
                    .is_some_and(|(g, _)| *g == generation)
                {
                    self.actors.remove(&task_id);
                }
                for event in leftover {
                    self.dispatch(task_id, event).await;
                }
            }
        }
    }

    /// Deliver to the task's actor, spawning one when the task is still
    /// active (or reopening it for a rescan).
    async fn dispatch(&mut self, task_id: i32, event: ActorEvent) {
        let event = match self.actors.get(&task_id) {
            Some((_, tx)) => match tx.send(event) {
                Ok(()) => return,
                Err(mpsc::error::SendError(event)) => {
                    self.actors.remove(&task_id);
                    event
                }
            },
            None => event,
        };

        let db = &self.shared.core.db;
        let task = match tasks::fetch_task(db, task_id).await {
            Ok(t) => t,
            Err(e) => {
                error!(task_id, error = %e, "failed to load task");
                if let ActorEvent::Control(_, reply) = event {
                    let _ = reply.send(Err(ControlError::Internal(e.into())));
                }
                return;
            }
        };
        let Some(task) = task else {
            if let ActorEvent::Control(_, reply) = event {
                let _ = reply.send(Err(ControlError::TaskNotFound));
            }
            return;
        };

        if task.status.is_terminal() {
            match event {
                ActorEvent::Control(cmd @ ControlCmd::Rescan { .. }, reply) => {
                    let limits =
                        EffectiveLimits::resolve(&self.shared.core.settings, &task_params(&task));
                    let deadline = limits.deadline_from_now();
                    if let Err(e) = tasks::reopen_task(db, task_id, deadline).await {
                        let _ = reply.send(Err(ControlError::Internal(e.into())));
                        return;
                    }
                    let tx = self.spawn(task_id);
                    let _ = tx.send(ActorEvent::Control(cmd, reply));
                }
                ActorEvent::Control(_, reply) => {
                    let _ = reply.send(Err(ControlError::Conflict("task already finished".into())));
                }
                ActorEvent::JobFinished(f) => {
                    debug!(
                        task_id,
                        job_id = f.job_id,
                        "late job result for finished task dropped"
                    );
                }
                ActorEvent::JobRetrying { .. } | ActorEvent::Tick => {}
            }
            return;
        }

        match event {
            ActorEvent::Control(..) | ActorEvent::JobFinished(_) => {
                let tx = self.spawn(task_id);
                let _ = tx.send(event);
            }
            ActorEvent::JobRetrying { .. } | ActorEvent::Tick => {}
        }
    }
}
