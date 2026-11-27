//! One actor per active task. It is the only component that changes
//! artifact state, task state, and job states other than claim / retry /
//! finish (which executors write).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use malbox_database::repositories::artifacts::{self, ArtifactEdge, NewArtifact, TaskArtifact};
use malbox_database::repositories::jobs::{self, Job, NewJob};
use malbox_database::repositories::plugin_reports;
use malbox_database::repositories::result_cache;
use malbox_database::repositories::tasks::{self, Task, TaskFinalization, TaskState};
use malbox_plugin_manifest::Stage;
use malbox_plugin_transport::messages::events::Event;
use time::OffsetDateTime;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info, warn};

use crate::admission::{Admission, AdmissionInput, admit};
use crate::cache::{cache_key, is_cacheable};
use crate::config::{EffectiveLimits, to_i64};
use crate::coordinator::Command;
use crate::error::{Result, SchedulerError};
use crate::ingest::IngestSummary;
use crate::interest::{Candidate, order_candidates};
use crate::params::{ChildDetonation, SubmissionParams, resolve_plugin_params};
use crate::plan::{JobKindPlan, PlanInput, TaskSelection, plan_artifact};
use crate::recursion::{DYNAMIC, ParentContext, forbidden_for_child};
use crate::submission::{ensure_sample_for_artifact, explicit_detonation, task_params};
use crate::verdict::{recompute_sample_verdict, task_verdict};
use crate::{ControlError, DescendantPolicy, Shared};

const TICK: Duration = Duration::from_secs(5);
/// Back-off before retrying a task whose state could not be loaded.
const LOAD_RETRY: Duration = Duration::from_secs(5);
pub(crate) const BUDGET_MESSAGE: &str = "detonation budget reached";
/// `canceled_by` of artifacts canceled with their task.
const TASK_CANCELED: &str = "task_canceled";

pub(crate) enum ActorEvent {
    JobFinished(JobFinished),
    JobRetrying { job_id: i64 },
    Control(ControlCmd, oneshot::Sender<Result<(), ControlError>>),
    Tick,
}

pub(crate) struct JobFinished {
    pub job_id: i64,
    pub outcome: JobOutcome,
}

pub(crate) enum JobOutcome {
    /// Outputs were ingested. The job is `completed`, or `failed` when the
    /// report was invalid or the sample could not be launched.
    Completed(IngestSummary),
    Failed {
        class: ErrorClass,
        message: String,
    },
    Canceled,
}

/// `job_error_class`, ordered by severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ErrorClass {
    Nonrecoverable,
    Recoverable,
    Timeout,
    MachineLost,
}

impl ErrorClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Nonrecoverable => "nonrecoverable",
            Self::Recoverable => "recoverable",
            Self::Timeout => "timeout",
            Self::MachineLost => "machine_lost",
        }
    }
}

pub(crate) enum ControlCmd {
    CancelTask,
    CancelArtifact {
        artifact_id: i64,
        descendants: DescendantPolicy,
    },
    CancelJob {
        job_id: i64,
    },
    Rescan {
        artifact_id: i64,
        plugins: Vec<String>,
        ignore_cache: bool,
    },
    SetPriority {
        artifact_id: i64,
        priority: i64,
        subtree: bool,
    },
}

fn stage_of(job: &Job) -> Stage {
    Stage::ALL
        .into_iter()
        .find(|s| s.as_str() == job.stage)
        .unwrap_or(Stage::Review)
}

fn internal(e: SchedulerError) -> ControlError {
    ControlError::Internal(e)
}

fn is_open_state(state: &str) -> bool {
    matches!(state, "pending" | "running")
}

/// Stored, asked for analysis, admission not decided yet.
fn awaiting_admission(a: &TaskArtifact) -> bool {
    a.analyze_requested && !a.analyze && a.state == "stored"
}

struct TaskActor {
    shared: Arc<Shared>,
    task: Task,
    params: SubmissionParams,
    limits: EffectiveLimits,
    artifacts: BTreeMap<i64, TaskArtifact>,
    /// child → incoming edges, oldest first.
    incoming: HashMap<i64, Vec<ArtifactEdge>>,
    /// parent → children.
    outgoing: HashMap<i64, BTreeSet<i64>>,
    jobs: BTreeMap<i64, Job>,
    by_artifact: HashMap<i64, BTreeSet<i64>>,
    detonations_used: i64,
    candidates: Vec<(i64, Instant)>,
    last_event: Instant,
    user_canceled: bool,
    deadline_hit: bool,
    dirty: BTreeSet<i64>,
    admissions: VecDeque<(i64, i32)>,
}

pub(crate) async fn run_actor(
    shared: Arc<Shared>,
    task_id: i32,
    generation: u64,
    mut rx: mpsc::UnboundedReceiver<ActorEvent>,
) {
    let mut actor = loop {
        match TaskActor::load(shared.clone(), task_id).await {
            Ok(Some(a)) => break a,
            Ok(None) => {
                warn!(task_id, "task disappeared, actor exiting");
                exit(&shared, task_id, generation, rx);
                return;
            }
            Err(e) => {
                error!(task_id, error = %e, "failed to load task state, retrying");
                tokio::select! {
                    _ = shared.token.cancelled() => return,
                    _ = tokio::time::sleep(LOAD_RETRY) => {}
                }
            }
        }
    };
    if let Err(e) = actor.start().await {
        error!(task_id, error = %e, "task start failed");
        actor.recover().await;
    }

    let mut tick = tokio::time::interval(TICK);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        let event = tokio::select! {
            biased;
            _ = shared.token.cancelled() => return,
            ev = rx.recv() => match ev {
                Some(ev) => ev,
                None => return,
            },
            _ = tick.tick() => ActorEvent::Tick,
        };
        if let Err(e) = actor.handle(event).await {
            error!(task_id, error = %e, "task event failed, reloading state");
            actor.recover().await;
        }
        match actor.try_finish().await {
            Ok(true) => break,
            Ok(false) => {}
            Err(e) => error!(task_id, error = %e, "task finalization failed"),
        }
    }
    exit(&shared, task_id, generation, rx);
}

/// Hand undelivered events back to the coordinator and deregister.
fn exit(
    shared: &Shared,
    task_id: i32,
    generation: u64,
    mut rx: mpsc::UnboundedReceiver<ActorEvent>,
) {
    rx.close();
    let mut leftover = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        leftover.push(ev);
    }
    let _ = shared.commands.send(Command::ActorExited {
        task_id,
        generation,
        leftover,
    });
}

impl TaskActor {
    async fn load(shared: Arc<Shared>, task_id: i32) -> Result<Option<Self>> {
        let db = &shared.core.db;
        let Some(task) = tasks::fetch_task(db, task_id).await? else {
            return Ok(None);
        };
        let params = task_params(&task);
        let limits = EffectiveLimits::resolve(&shared.core.settings, &params);
        let mut actor = Self {
            shared,
            task,
            params,
            limits,
            artifacts: BTreeMap::new(),
            incoming: HashMap::new(),
            outgoing: HashMap::new(),
            jobs: BTreeMap::new(),
            by_artifact: HashMap::new(),
            detonations_used: 0,
            candidates: Vec::new(),
            last_event: Instant::now(),
            user_canceled: false,
            deadline_hit: false,
            dirty: BTreeSet::new(),
            admissions: VecDeque::new(),
        };
        actor.reload().await?;
        // The cancel flag only lives in memory; after a restart, derive it
        // from the artifacts the cancel closed.
        actor.user_canceled = actor
            .artifacts
            .values()
            .any(|a| a.canceled_by.as_deref() == Some(TASK_CANCELED));
        Ok(Some(actor))
    }

    /// Replace the in-memory view with the database's.
    async fn reload(&mut self) -> Result<()> {
        let db = &self.shared.core.db;
        let id = self.task.id;
        let task = tasks::fetch_task(db, id)
            .await?
            .ok_or_else(|| SchedulerError::Internal(format!("task {id} disappeared")))?;
        let all_artifacts = artifacts::fetch_task_artifacts(db, id).await?;
        let edges = artifacts::fetch_task_edges(db, id).await?;
        let all_jobs = jobs::fetch_task_jobs(db, id).await?;

        self.task = task;
        self.artifacts = all_artifacts.into_iter().map(|a| (a.id, a)).collect();
        self.incoming.clear();
        self.outgoing.clear();
        for e in edges {
            self.add_edge(e);
        }
        self.jobs.clear();
        self.by_artifact.clear();
        for j in all_jobs {
            self.put_job(j);
        }
        self.detonations_used = self
            .jobs
            .values()
            .filter(|j| {
                j.kind == "detonation" && !matches!(j.state.as_str(), "blocked" | "skipped")
            })
            .count() as i64;
        self.candidates.clear();
        self.dirty.clear();
        self.admissions.clear();
        Ok(())
    }

    /// After a failed event: reload and resume from the database's state.
    async fn recover(&mut self) {
        let task_id = self.task.id;
        if let Err(e) = self.reload().await {
            error!(task_id, error = %e, "failed to reload task state");
            return;
        }
        if let Err(e) = self.resume().await {
            error!(task_id, error = %e, "failed to progress task after reload");
        }
    }

    fn emit(&self, event: Event) {
        if let Err(e) = self.shared.core.events.emit(event) {
            warn!(task_id = self.task.id, error = %e, "failed to emit task event");
        }
    }

    async fn start(&mut self) -> Result<()> {
        let task_id = self.task.id;
        self.emit(Event::TaskStarting { task_id });
        tasks::set_task_running(&self.shared.core.db, task_id).await?;
        self.resume().await
    }

    /// Rebuild the work derived from persisted state, which `reload` drops:
    /// plan analysed artifacts that have no jobs yet (`pending`), queue the
    /// ones still awaiting admission, then progress everything.
    async fn resume(&mut self) -> Result<()> {
        let pending: Vec<i64> = self
            .artifacts
            .values()
            .filter(|a| a.analyze && a.state == "pending")
            .map(|a| a.id)
            .collect();
        for id in pending {
            self.plan_jobs(id, None).await?;
        }
        let waiting: Vec<(i64, i32)> = self
            .artifacts
            .values()
            .filter(|a| awaiting_admission(a))
            .map(|a| (a.id, (a.depth - 1).max(0)))
            .collect();
        self.admissions.extend(waiting);
        // Startup recovery requeues jobs left running by a stopped daemon,
        // including those of artifacts canceled meanwhile.
        let stale: Vec<i64> = self
            .jobs
            .values()
            .filter(|j| !j.is_terminal())
            .filter(|j| {
                self.artifacts
                    .get(&j.artifact_id)
                    .is_some_and(|a| a.state == "canceled")
            })
            .map(|j| j.id)
            .collect();
        self.cancel_jobs(&stale).await?;
        self.progress_all().await
    }

    // ----- in-memory bookkeeping -------------------------------------------

    fn add_edge(&mut self, e: ArtifactEdge) {
        let list = self.incoming.entry(e.child_artifact_id).or_default();
        let exists = list
            .iter()
            .any(|x| x.parent_artifact_id == e.parent_artifact_id && x.relation == e.relation);
        if !exists {
            self.outgoing
                .entry(e.parent_artifact_id)
                .or_default()
                .insert(e.child_artifact_id);
            list.push(e);
            list.sort_by_key(|x| x.created_on);
        }
    }

    fn put_job(&mut self, j: Job) {
        self.by_artifact
            .entry(j.artifact_id)
            .or_default()
            .insert(j.id);
        self.jobs.insert(j.id, j);
    }

    fn jobs_of(&self, artifact_id: i64) -> impl Iterator<Item = &Job> {
        self.by_artifact
            .get(&artifact_id)
            .into_iter()
            .flatten()
            .filter_map(|id| self.jobs.get(id))
    }

    /// Distinct parents, self-loops excluded.
    fn parents_of(&self, id: i64) -> BTreeSet<i64> {
        self.incoming
            .get(&id)
            .into_iter()
            .flatten()
            .map(|e| e.parent_artifact_id)
            .filter(|p| *p != id)
            .collect()
    }

    fn children_of(&self, id: i64) -> Vec<i64> {
        self.outgoing
            .get(&id)
            .into_iter()
            .flatten()
            .copied()
            .filter(|c| *c != id)
            .collect()
    }

    async fn refresh_job(&mut self, job_id: i64) -> Result<Option<i64>> {
        match jobs::fetch_job(&self.shared.core.db, job_id).await? {
            Some(j) if j.task_id == self.task.id => {
                let artifact_id = j.artifact_id;
                self.put_job(j);
                Ok(Some(artifact_id))
            }
            _ => Ok(None),
        }
    }

    /// Pull a job's edges and their child artifacts into memory.
    async fn absorb_job_edges(&mut self, job_id: i64) -> Result<()> {
        for (edge, child) in artifacts::fetch_edges_from_job(&self.shared.core.db, job_id).await? {
            if child.task_id == self.task.id {
                self.artifacts.insert(child.id, child);
                self.add_edge(edge);
            }
        }
        Ok(())
    }

    async fn set_artifact_state(
        &mut self,
        id: i64,
        state: &str,
        drop_reason: Option<&str>,
        canceled_by: Option<&str>,
    ) -> Result<()> {
        artifacts::update_artifact_state(&self.shared.core.db, id, state, drop_reason, canceled_by)
            .await?;
        if let Some(a) = self.artifacts.get_mut(&id) {
            a.state = state.to_string();
            a.drop_reason = drop_reason.map(str::to_string);
            a.canceled_by = canceled_by.map(str::to_string);
        }
        Ok(())
    }

    fn artifact_deadline(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
            + time::Duration::seconds(to_i64(self.limits.max_artifact_time_secs))
    }

    /// Re-open a finished artifact (rerun / rescan) with a fresh deadline.
    async fn reopen_artifact(&mut self, id: i64) -> Result<()> {
        let deadline = self.artifact_deadline();
        artifacts::set_artifact_deadline(&self.shared.core.db, id, Some(deadline)).await?;
        if let Some(a) = self.artifacts.get_mut(&id) {
            a.deadline = Some(deadline);
        }
        self.set_artifact_state(id, "running", None, None).await
    }

    async fn release(&mut self, ids: &[i64], kind: JobKindPlan) -> Result<()> {
        jobs::release_jobs(&self.shared.core.db, ids).await?;
        for id in ids {
            if let Some(j) = self.jobs.get_mut(id) {
                j.state = "ready".to_string();
            }
        }
        self.shared.executors.notify(kind);
        Ok(())
    }

    async fn skip_job(&mut self, id: i64, message: &str) -> Result<()> {
        jobs::set_job_state(&self.shared.core.db, id, "skipped", None, Some(message)).await?;
        if let Some(j) = self.jobs.get_mut(&id) {
            j.state = "skipped".to_string();
            j.error_message = Some(message.to_string());
        }
        Ok(())
    }

    async fn finish_artifact(&mut self, id: i64) -> Result<()> {
        self.set_artifact_state(id, "done", None, None).await?;
        if let Some(sample_id) = self.artifacts.get(&id).and_then(|a| a.sample_id) {
            recompute_sample_verdict(&self.shared.core, sample_id, self.task.id).await;
        }
        Ok(())
    }

    // ----- recursion prevention ---------------------------------------------

    fn prevention_from_jobs(&self, parent: i64) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for j in self.jobs_of(parent) {
            if j.kind == "detonation" {
                if j.state != "skipped"
                    && self.params.detonate_children == ChildDetonation::Extracted
                {
                    out.insert(DYNAMIC.to_string());
                }
            } else if let Some(spec) = self.shared.routing.analyzer(&j.analyzer) {
                out.extend(spec.recursion_prevention.iter().cloned());
            }
        }
        out
    }

    fn forbidden_for(
        &self,
        id: i64,
        memo: &mut HashMap<i64, BTreeSet<String>>,
        visiting: &mut HashSet<i64>,
    ) -> BTreeSet<String> {
        if let Some(f) = memo.get(&id) {
            return f.clone();
        }
        let Some(art) = self.artifacts.get(&id) else {
            return BTreeSet::new();
        };
        if art.depth == 0 || !visiting.insert(id) {
            return BTreeSet::new();
        }
        let parents: Vec<ParentContext> = self
            .parents_of(id)
            .into_iter()
            .map(|p| ParentContext {
                forbidden: self.forbidden_for(p, memo, visiting),
                prevention_from_jobs: self.prevention_from_jobs(p),
            })
            .collect();
        let f = if parents.is_empty() {
            BTreeSet::new()
        } else {
            forbidden_for_child(
                &parents,
                self.params.detonate_children,
                self.params.ignore_recursion_prevention,
                art.dynamic_recursion_bypass,
            )
        };
        visiting.remove(&id);
        memo.insert(id, f.clone());
        f
    }

    fn source_analyzers(&self, id: i64) -> BTreeSet<String> {
        self.incoming
            .get(&id)
            .into_iter()
            .flatten()
            .filter(|e| e.parent_artifact_id != id)
            .filter_map(|e| match e.source_job_id.and_then(|j| self.jobs.get(&j)) {
                Some(job) if job.kind == "host" => Some(job.analyzer.clone()),
                _ => e.source_plugin.clone(),
            })
            .collect()
    }

    // ----- planning, admission, progression ---------------------------------

    /// Insert the planned jobs of an analysed artifact (skipping analyzers
    /// that already have one) and mark it running, or done without jobs.
    async fn plan_jobs(&mut self, id: i64, selected: Option<Vec<String>>) -> Result<()> {
        let Some(art) = self.artifacts.get(&id).cloned() else {
            return Ok(());
        };
        let is_root = art.depth == 0;
        let forbidden = self.forbidden_for(id, &mut HashMap::new(), &mut HashSet::new());
        let sources = self.source_analyzers(id);
        let selection = TaskSelection {
            selected: selected.unwrap_or_else(|| self.task.plugins.clone()),
            excluded: self.task.excluded_plugins.clone(),
        };
        let routing = self.shared.routing.clone();
        let explicit = if is_root {
            explicit_detonation(&self.task, &self.params, &routing)
        } else {
            None
        };
        let planned = plan_artifact(&PlanInput {
            artifact_type: &art.artifact_type,
            is_root,
            task: &selection,
            params: &self.params,
            forbidden: &forbidden,
            source_analyzers: &sources,
            routing: &routing,
            explicit: explicit.as_ref(),
        });
        let existing: HashSet<String> = self.jobs_of(id).map(|j| j.analyzer.clone()).collect();
        let settings = &self.shared.core.settings;
        let max_attempts = i16::try_from(settings.max_attempts).unwrap_or(i16::MAX);

        let mut tx = self.shared.core.db.begin().await?;
        let mut inserted = Vec::new();
        for p in planned.iter().filter(|p| !existing.contains(&p.analyzer)) {
            let key = match (p.kind, routing.analyzer(&p.analyzer)) {
                (JobKindPlan::Host, Some(spec)) if is_cacheable(spec, &self.params) => {
                    Some(cache_key(
                        &art.sha256,
                        &spec.name,
                        &spec.version,
                        &resolve_plugin_params(spec, &self.params),
                        self.params.deep_scan,
                        self.params.ignore_filtering,
                    ))
                }
                _ => None,
            };
            let detonation = p
                .detonation
                .as_ref()
                .map(serde_json::to_value)
                .transpose()
                .map_err(|e| {
                    SchedulerError::Internal(format!("failed to encode detonation: {e}"))
                })?;
            let job = jobs::insert_job(
                &mut *tx,
                &NewJob {
                    task_id: self.task.id,
                    artifact_id: id,
                    kind: p.kind.as_str(),
                    analyzer: &p.analyzer,
                    stage: p.stage.as_str(),
                    priority: art.priority,
                    state: "blocked",
                    max_attempts,
                    ignore_cache: self.params.ignore_cache,
                    cache_key: key.as_deref(),
                    snapshot_id: p.detonation.as_ref().and_then(|d| d.snapshot_id),
                    guest_plugins: None,
                    detonation,
                },
            )
            .await?;
            inserted.push(job);
        }
        let has_jobs = !existing.is_empty() || !inserted.is_empty();
        let state = if has_jobs { "running" } else { "done" };
        let deadline = self.artifact_deadline();
        artifacts::set_artifact_deadline(&mut *tx, id, Some(deadline)).await?;
        artifacts::update_artifact_state(&mut *tx, id, state, None, None).await?;
        tx.commit().await?;

        debug!(
            task_id = self.task.id,
            artifact_id = id,
            jobs = inserted.len(),
            "planned artifact"
        );
        for j in inserted {
            self.put_job(j);
        }
        if let Some(a) = self.artifacts.get_mut(&id) {
            a.state = state.to_string();
            a.deadline = Some(deadline);
            a.drop_reason = None;
            a.canceled_by = None;
        }
        if has_jobs {
            self.dirty.insert(id);
        } else if let Some(sample_id) = art.sample_id {
            recompute_sample_verdict(&self.shared.core, sample_id, self.task.id).await;
        }
        Ok(())
    }

    async fn admit_artifact(&mut self, id: i64, parent_depth: i32) -> Result<()> {
        let Some(art) = self.artifacts.get(&id).cloned() else {
            return Ok(());
        };
        if !awaiting_admission(&art) {
            return Ok(());
        }
        let parents = self.parents_of(id);
        if !parents.is_empty()
            && parents
                .iter()
                .all(|p| self.artifacts.get(p).is_some_and(|a| a.state == "canceled"))
        {
            return self
                .set_artifact_state(id, "canceled", None, Some("parent_canceled"))
                .await;
        }
        let analysed = self.artifacts.values().filter(|a| a.analyze).count();
        let decision = admit(&AdmissionInput {
            size_bytes: art.size_bytes,
            parent_depth,
            analysed_count: i32::try_from(analysed).unwrap_or(i32::MAX),
            max_depth: self.task.max_depth,
            max_artifacts: self.task.max_artifacts,
            max_file_size: self.shared.core.settings.max_file_size,
            ignore_size: self.params.ignore_size,
        });
        match decision {
            Admission::Drop(reason) => {
                artifacts::set_artifact_admission_dropped(&self.shared.core.db, id, reason).await?;
                if let Some(a) = self.artifacts.get_mut(&id) {
                    a.state = "dropped".to_string();
                    a.drop_reason = Some(reason.to_string());
                }
                Ok(())
            }
            Admission::Analyse { depth } => {
                let sample = ensure_sample_for_artifact(&self.shared.core, &art).await?;
                self.upgrade(id, sample.id, depth).await?;
                self.plan_jobs(id, None).await
            }
        }
    }

    async fn upgrade(&mut self, id: i64, sample_id: i64, depth: i32) -> Result<()> {
        artifacts::upgrade_artifact_to_analysed(&self.shared.core.db, id, sample_id, depth).await?;
        if let Some(a) = self.artifacts.get_mut(&id) {
            a.analyze = true;
            a.sample_id = Some(sample_id);
            a.state = "pending".to_string();
            a.drop_reason = None;
            a.depth = a.depth.min(depth);
        }
        Ok(())
    }

    /// Bookkeeping after a job's outputs were ingested (by an executor or a
    /// cache replay): queue admissions and monitored-key re-runs.
    async fn after_outputs(
        &mut self,
        job_id: i64,
        new_artifacts: &[i64],
        changed_keys: &[String],
    ) -> Result<()> {
        let Some(job) = self.jobs.get(&job_id).cloned() else {
            return Ok(());
        };
        let parent_depth = self.artifacts.get(&job.artifact_id).map_or(0, |a| a.depth);
        for id in new_artifacts {
            if self.artifacts.get(id).is_some_and(awaiting_admission) {
                self.admissions.push_back((*id, parent_depth));
            }
        }

        if !changed_keys.is_empty() {
            let max_reruns =
                i16::try_from(self.shared.core.settings.max_reruns).unwrap_or(i16::MAX);
            let rerun: Vec<Job> =
                self.jobs
                    .values()
                    .filter(|j| j.id != job_id && j.kind == "host")
                    .filter(|j| {
                        matches!(j.state.as_str(), "completed" | "failed") && j.reruns < max_reruns
                    })
                    .filter(|j| {
                        self.shared.routing.analyzer(&j.analyzer).is_some_and(|s| {
                            s.monitored_keys.iter().any(|k| changed_keys.contains(k))
                        })
                    })
                    .filter(|j| {
                        self.artifacts
                            .get(&j.artifact_id)
                            .is_some_and(|a| a.state != "canceled")
                    })
                    .cloned()
                    .collect();
            for j in rerun {
                info!(task_id = self.task.id, job_id = j.id, analyzer = %j.analyzer, "monitored key changed, re-running");
                jobs::reset_job_for_rerun(&self.shared.core.db, j.id, true).await?;
                self.refresh_job(j.id).await?;
                if self
                    .artifacts
                    .get(&j.artifact_id)
                    .is_some_and(|a| a.state == "done")
                {
                    self.reopen_artifact(j.artifact_id).await?;
                }
                self.shared.executors.notify(JobKindPlan::Host);
            }
        }
        self.dirty.insert(job.artifact_id);
        Ok(())
    }

    /// Replay a cached host job. `Ok(false)` when there is no usable entry.
    async fn try_replay(&mut self, job: &Job, key: &str) -> Result<bool> {
        let core = self.shared.core.clone();
        let db = &core.db;
        let Some((source_id, _empty)) = result_cache::lookup(db, key).await? else {
            return Ok(false);
        };
        if source_id == job.id {
            return Ok(false);
        }
        let Some(source) = jobs::fetch_job(db, source_id).await? else {
            return Ok(false);
        };
        let report = plugin_reports::fetch_plugin_report(db, source.artifact_id, &source.analyzer)
            .await?
            .filter(|r| r.job_id == Some(source.id));
        if let Some(r) = &report
            && !tokio::fs::try_exists(core.data_dir.join(&r.report_path))
                .await
                .unwrap_or(false)
        {
            return Ok(false);
        }
        let children = artifacts::fetch_edges_from_job(db, source.id).await?;
        let mut new_bytes: i64 = 0;
        for (_, child) in &children {
            // A blobless child means the source run was cut short by its
            // task's byte budget; its result does not apply to this task.
            if child.storage_path.is_none() || !core.sample_store.exists(&child.sha256).await? {
                return Ok(false);
            }
            if !self.artifacts.values().any(|a| a.sha256 == child.sha256) {
                new_bytes = new_bytes.saturating_add(child.size_bytes);
            }
        }
        let Some(art) = self.artifacts.get(&job.artifact_id).cloned() else {
            return Ok(false);
        };

        let mut tx = db.begin().await?;
        if new_bytes > 0 {
            // Locked like ingest, so concurrent executor ingests can't race it.
            let used = artifacts::lock_task_artifact_bytes(&mut tx, self.task.id).await?;
            if used.saturating_add(new_bytes) > self.task.max_total_bytes {
                // Run the job instead: ingest applies this task's budget.
                return Ok(false);
            }
        }
        if report.is_some() {
            plugin_reports::copy_plugin_report(&mut tx, source.id, art.id, job.id, self.task.id)
                .await?;
        }
        let mut replayed = Vec::new();
        for (edge, child) in &children {
            let (row, created) = artifacts::insert_or_get_artifact(
                &mut tx,
                &NewArtifact {
                    task_id: self.task.id,
                    sha256: &child.sha256,
                    size_bytes: child.size_bytes,
                    storage_path: child.storage_path.as_deref(),
                    analyze: false,
                    sample_id: None,
                    depth: art.depth + 1,
                    artifact_type: &child.artifact_type,
                    mime: &child.mime,
                    state: "stored",
                    drop_reason: None,
                    priority: art.priority,
                    analyze_requested: child.analyze_requested,
                    dynamic_recursion_bypass: child.dynamic_recursion_bypass,
                },
            )
            .await?;
            let newly_requested = !created && child.analyze_requested && !row.analyze_requested;
            if !created && child.analyze_requested {
                artifacts::set_analyze_requested(&mut *tx, row.id, child.dynamic_recursion_bypass)
                    .await?;
            }
            artifacts::insert_edge(
                &mut *tx,
                self.task.id,
                art.id,
                row.id,
                &edge.relation,
                Some(job.id),
                edge.source_plugin.as_deref(),
                edge.name.as_deref(),
            )
            .await?;
            if (created || newly_requested) && !replayed.contains(&row.id) {
                replayed.push(row.id);
            }
        }
        jobs::mark_cached(&mut *tx, job.id, source.id).await?;
        tx.commit().await?;

        info!(
            task_id = self.task.id,
            job_id = job.id,
            source_job_id = source.id,
            "cache hit"
        );
        self.refresh_job(job.id).await?;
        self.absorb_job_edges(job.id).await?;
        self.after_outputs(job.id, &replayed, &[]).await?;
        Ok(true)
    }

    /// Release the next stage of an artifact once the previous one is done.
    async fn progress_one(&mut self, id: i64) -> Result<()> {
        loop {
            let Some(art) = self.artifacts.get(&id) else {
                return Ok(());
            };
            if !is_open_state(&art.state) {
                return Ok(());
            }
            let (state, depth) = (art.state.clone(), art.depth);
            let all: Vec<Job> = self.jobs_of(id).cloned().collect();
            let open: Vec<&Job> = all.iter().filter(|j| !j.is_terminal()).collect();
            if open.is_empty() {
                if all.is_empty() && state == "pending" {
                    // Not planned yet; planning will progress it.
                    return Ok(());
                }
                return self.finish_artifact(id).await;
            }
            let current = open
                .iter()
                .map(|j| stage_of(j))
                .min()
                .unwrap_or(Stage::Review);
            let stage_jobs: Vec<&Job> = open
                .iter()
                .copied()
                .filter(|j| stage_of(j) == current)
                .collect();
            if stage_jobs.iter().any(|j| j.state != "blocked") {
                return Ok(());
            }

            if !self.params.ignore_filtering
                && let Some(dropper) = all.iter().find(|j| j.is_terminal() && j.drop_requested)
            {
                let message = format!("skipped: dropped by {}", dropper.analyzer);
                for j in open.iter().filter(|j| j.state == "blocked") {
                    self.skip_job(j.id, &message).await?;
                }
                continue;
            }

            if current == Stage::Dynamic {
                if depth > 0 {
                    if !self.candidates.iter().any(|(c, _)| *c == id) {
                        self.candidates.push((id, Instant::now()));
                    }
                    return Ok(());
                }
                let mut released = false;
                for j in &stage_jobs {
                    if self.detonations_used < i64::from(self.task.max_detonations) {
                        self.release(&[j.id], JobKindPlan::Detonation).await?;
                        self.detonations_used += 1;
                        released = true;
                    } else {
                        self.skip_job(j.id, BUDGET_MESSAGE).await?;
                    }
                }
                if released {
                    return Ok(());
                }
                continue;
            }

            let mut to_release = Vec::new();
            for j in &stage_jobs {
                if !j.ignore_cache
                    && let Some(key) = &j.cache_key
                    && self.try_replay(j, key).await?
                {
                    continue;
                }
                to_release.push(j.id);
            }
            if to_release.is_empty() {
                continue;
            }
            return self.release(&to_release, JobKindPlan::Host).await;
        }
    }

    /// Run queued admissions and progressions until nothing is left.
    async fn drain(&mut self) -> Result<()> {
        loop {
            if let Some((id, parent_depth)) = self.admissions.pop_front() {
                self.admit_artifact(id, parent_depth).await?;
                continue;
            }
            if let Some(id) = self.dirty.pop_first() {
                self.progress_one(id).await?;
                continue;
            }
            return Ok(());
        }
    }

    async fn progress_all(&mut self) -> Result<()> {
        let open: Vec<i64> = self
            .artifacts
            .values()
            .filter(|a| is_open_state(&a.state))
            .map(|a| a.id)
            .collect();
        self.dirty.extend(open);
        self.drain().await?;
        self.decide_detonations().await
    }

    fn blocked_detonations(&self, artifact_id: i64) -> Vec<i64> {
        self.jobs_of(artifact_id)
            .filter(|j| j.kind == "detonation" && j.state == "blocked")
            .map(|j| j.id)
            .collect()
    }

    /// Spend the detonation budget on the most interesting waiting children
    /// once static analysis settled or the decision window elapsed.
    async fn decide_detonations(&mut self) -> Result<()> {
        loop {
            let candidates = std::mem::take(&mut self.candidates);
            self.candidates = candidates
                .into_iter()
                .filter(|(id, _)| {
                    self.artifacts
                        .get(id)
                        .is_some_and(|a| is_open_state(&a.state))
                        && !self.blocked_detonations(*id).is_empty()
                })
                .collect();
            let Some(oldest) = self.candidates.iter().map(|(_, t)| *t).min() else {
                return Ok(());
            };
            let settled = !self.artifacts.values().any(|a| {
                a.analyze
                    && self
                        .jobs_of(a.id)
                        .any(|j| !j.is_terminal() && stage_of(j) < Stage::Dynamic)
            });
            let window =
                Duration::from_secs(self.shared.core.settings.detonation_decision_window_secs);
            if !settled && oldest.elapsed() < window {
                return Ok(());
            }

            let mut list = Vec::with_capacity(self.candidates.len());
            for (id, _) in std::mem::take(&mut self.candidates) {
                let Some(a) = self.artifacts.get(&id) else {
                    continue;
                };
                let (artifact_type, depth) = (a.artifact_type.clone(), a.depth);
                let max_score =
                    plugin_reports::fetch_plugin_reports_for_artifact(&self.shared.core.db, id)
                        .await?
                        .iter()
                        .filter_map(|r| r.score)
                        .max();
                list.push(Candidate {
                    artifact_id: id,
                    artifact_type,
                    max_score,
                    depth,
                });
            }
            order_candidates(&mut list);
            for c in list {
                for job_id in self.blocked_detonations(c.artifact_id) {
                    if self.detonations_used < i64::from(self.task.max_detonations) {
                        self.release(&[job_id], JobKindPlan::Detonation).await?;
                        self.detonations_used += 1;
                    } else {
                        self.skip_job(job_id, BUDGET_MESSAGE).await?;
                    }
                }
                self.dirty.insert(c.artifact_id);
            }
            self.drain().await?;
        }
    }

    // ----- events ------------------------------------------------------------

    async fn handle(&mut self, event: ActorEvent) -> Result<()> {
        match event {
            ActorEvent::JobFinished(f) => {
                self.last_event = Instant::now();
                self.job_finished(f).await?;
                self.drain().await?;
                self.decide_detonations().await
            }
            ActorEvent::JobRetrying { job_id } => {
                debug!(task_id = self.task.id, job_id, "job retrying");
                self.last_event = Instant::now();
                Ok(())
            }
            ActorEvent::Control(cmd, reply) => {
                self.last_event = Instant::now();
                let result = self.control(cmd).await;
                if matches!(result, Err(ControlError::Internal(_))) {
                    // The command may have stopped half-way: rebuild the
                    // in-memory view from the database before replying.
                    self.recover().await;
                    let _ = reply.send(result);
                    return Ok(());
                }
                let progressed = async {
                    self.drain().await?;
                    self.decide_detonations().await
                }
                .await;
                let _ = reply.send(result);
                progressed
            }
            ActorEvent::Tick => self.tick().await,
        }
    }

    async fn job_finished(&mut self, f: JobFinished) -> Result<()> {
        // The job is done: drop a cancel that raced its completion, so a
        // later rescan or rerun of the same job id doesn't start canceled.
        self.shared.running.remove(f.job_id);
        let Some(artifact_id) = self.refresh_job(f.job_id).await? else {
            debug!(
                task_id = self.task.id,
                job_id = f.job_id,
                "finished job not in task"
            );
            return Ok(());
        };
        match f.outcome {
            JobOutcome::Completed(summary) => {
                self.absorb_job_edges(f.job_id).await?;
                if !summary.changed_shared_keys.is_empty()
                    && let Some(task) =
                        tasks::fetch_task(&self.shared.core.db, self.task.id).await?
                {
                    self.task.shared_data = task.shared_data;
                }
                self.after_outputs(
                    f.job_id,
                    &summary.new_artifacts,
                    &summary.changed_shared_keys,
                )
                .await?;
            }
            JobOutcome::Failed { class, message } => {
                debug!(task_id = self.task.id, job_id = f.job_id, class = class.as_str(), %message, "job failed");
                self.dirty.insert(artifact_id);
            }
            JobOutcome::Canceled => {
                self.dirty.insert(artifact_id);
            }
        }
        Ok(())
    }

    async fn tick(&mut self) -> Result<()> {
        let now = OffsetDateTime::now_utc();
        if !self.deadline_hit && now >= self.task.deadline {
            warn!(
                task_id = self.task.id,
                "task deadline reached, canceling remaining work"
            );
            self.deadline_hit = true;
            self.cancel_all("task_deadline").await?;
        }
        let expired: Vec<i64> = self
            .artifacts
            .values()
            .filter(|a| is_open_state(&a.state) && a.deadline.is_some_and(|d| d <= now))
            .map(|a| a.id)
            .collect();
        for id in expired {
            warn!(
                task_id = self.task.id,
                artifact_id = id,
                "artifact deadline reached"
            );
            self.cancel_artifact(id, DescendantPolicy::Keep, "artifact_deadline")
                .await?;
        }

        let stall = Duration::from_secs(self.shared.core.settings.stall_check_secs);
        if self.last_event.elapsed() > stall && self.jobs.values().any(|j| !j.is_terminal()) {
            warn!("task {}: stall check reloaded state", self.task.id);
            self.last_event = Instant::now();
            self.reload().await?;
            self.dirty.extend(
                self.artifacts
                    .values()
                    .filter(|a| is_open_state(&a.state))
                    .map(|a| a.id),
            );
        }
        self.drain().await?;
        self.decide_detonations().await
    }

    // ----- control -------------------------------------------------------------

    async fn control(&mut self, cmd: ControlCmd) -> Result<(), ControlError> {
        match cmd {
            ControlCmd::CancelTask => {
                self.user_canceled = true;
                self.cancel_all(TASK_CANCELED).await.map_err(internal)
            }
            ControlCmd::CancelArtifact {
                artifact_id,
                descendants,
            } => {
                let art = self
                    .artifacts
                    .get(&artifact_id)
                    .ok_or(ControlError::ArtifactNotFound)?;
                if !is_open_state(&art.state) {
                    return Err(ControlError::Conflict("artifact already finished".into()));
                }
                self.cancel_artifact(artifact_id, descendants, "user")
                    .await
                    .map_err(internal)
            }
            ControlCmd::CancelJob { job_id } => self.cancel_job(job_id).await,
            ControlCmd::Rescan {
                artifact_id,
                plugins,
                ignore_cache,
            } => self.rescan(artifact_id, plugins, ignore_cache).await,
            ControlCmd::SetPriority {
                artifact_id,
                priority,
                subtree,
            } => self.set_priority(artifact_id, priority, subtree).await,
        }
    }

    /// Cancel an artifact's unfinished jobs and mark it canceled; with
    /// [`DescendantPolicy::Cancel`], recurse into children whose every
    /// parent is canceled.
    ///
    /// Jobs are canceled before the artifact is closed: a closed artifact is
    /// never progressed again, so a failure in between must leave it open
    /// for the retry (recovery or the next deadline sweep).
    async fn cancel_artifact(&mut self, id: i64, policy: DescendantPolicy, by: &str) -> Result<()> {
        let mut stack = vec![(id, by.to_string())];
        while let Some((id, by)) = stack.pop() {
            let Some(art) = self.artifacts.get(&id) else {
                continue;
            };
            if !(is_open_state(&art.state) || awaiting_admission(art)) {
                continue;
            }
            let drop_reason = art.drop_reason.clone();
            let open: Vec<i64> = self
                .jobs_of(id)
                .filter(|j| !j.is_terminal())
                .map(|j| j.id)
                .collect();
            self.cancel_jobs(&open).await?;
            self.set_artifact_state(id, "canceled", drop_reason.as_deref(), Some(&by))
                .await?;
            self.candidates.retain(|(c, _)| *c != id);

            if policy == DescendantPolicy::Cancel {
                for child in self.children_of(id) {
                    let all_parents_canceled = self
                        .parents_of(child)
                        .iter()
                        .all(|p| self.artifacts.get(p).is_some_and(|a| a.state == "canceled"));
                    if all_parents_canceled {
                        stack.push((child, "parent_canceled".to_string()));
                    }
                }
            }
        }
        Ok(())
    }

    /// Cancel unstarted jobs directly; running ones through their tokens
    /// (their executors report `Canceled`).
    async fn cancel_jobs(&mut self, ids: &[i64]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let canceled = jobs::cancel_unstarted_jobs(&self.shared.core.db, ids).await?;
        for id in ids {
            if canceled.contains(id) {
                if let Some(j) = self.jobs.get_mut(id) {
                    j.state = "canceled".to_string();
                }
            } else {
                self.shared.running.cancel(*id);
            }
        }
        Ok(())
    }

    async fn cancel_all(&mut self, by: &str) -> Result<()> {
        let root = self.artifacts.values().find(|a| a.depth == 0).map(|a| a.id);
        if let Some(root) = root {
            self.cancel_artifact(root, DescendantPolicy::Cancel, by)
                .await?;
        }
        let rest: Vec<i64> = self
            .artifacts
            .values()
            .filter(|a| is_open_state(&a.state) || awaiting_admission(a))
            .map(|a| a.id)
            .collect();
        for id in rest {
            self.cancel_artifact(id, DescendantPolicy::Cancel, by)
                .await?;
        }
        Ok(())
    }

    async fn cancel_job(&mut self, job_id: i64) -> Result<(), ControlError> {
        if !self.jobs.contains_key(&job_id) {
            self.refresh_job(job_id).await.map_err(internal)?;
        }
        let job = self.jobs.get(&job_id).ok_or(ControlError::JobNotFound)?;
        if job.is_terminal() {
            return Err(ControlError::Conflict("job already finished".into()));
        }
        let artifact_id = job.artifact_id;
        self.cancel_jobs(&[job_id]).await.map_err(internal)?;
        self.dirty.insert(artifact_id);
        Ok(())
    }

    async fn rescan(
        &mut self,
        id: i64,
        plugins: Vec<String>,
        ignore_cache: bool,
    ) -> Result<(), ControlError> {
        let art = self
            .artifacts
            .get(&id)
            .cloned()
            .ok_or(ControlError::ArtifactNotFound)?;
        if art.storage_path.is_none() {
            return Err(ControlError::Conflict(
                "artifact has no stored content".into(),
            ));
        }
        let db = self.shared.core.db.clone();

        if !art.analyze {
            self.user_canceled = false;
            let sample = ensure_sample_for_artifact(&self.shared.core, &art)
                .await
                .map_err(internal)?;
            self.upgrade(id, sample.id, art.depth)
                .await
                .map_err(internal)?;
            let selected = (!plugins.is_empty()).then_some(plugins);
            return self.plan_jobs(id, selected).await.map_err(internal);
        }

        let targets: Vec<Job> = self
            .jobs_of(id)
            .filter(|j| plugins.is_empty() || plugins.contains(&j.analyzer))
            .filter(|j| j.is_terminal())
            .cloned()
            .collect();
        if targets.is_empty() {
            return Err(ControlError::Conflict(
                "no finished jobs match the rescan request".into(),
            ));
        }
        // Rescanning reopens the task; it no longer counts as canceled.
        self.user_canceled = false;
        // Reopen first: if a reset below fails, the artifact is open and
        // progression (or recovery) still handles the reset jobs.
        self.reopen_artifact(id).await.map_err(internal)?;
        for j in &targets {
            jobs::reset_job_for_rescan(&db, j.id, ignore_cache)
                .await
                .map_err(|e| internal(e.into()))?;
            if j.kind == "detonation" && !matches!(j.state.as_str(), "blocked" | "skipped") {
                self.detonations_used = (self.detonations_used - 1).max(0);
            }
            self.refresh_job(j.id).await.map_err(internal)?;
        }
        self.dirty.insert(id);
        Ok(())
    }

    async fn set_priority(
        &mut self,
        id: i64,
        priority: i64,
        subtree: bool,
    ) -> Result<(), ControlError> {
        if !self.artifacts.contains_key(&id) {
            return Err(ControlError::ArtifactNotFound);
        }
        let mut ids = vec![id];
        if subtree {
            let mut seen = HashSet::from([id]);
            let mut i = 0;
            while i < ids.len() {
                for c in self.children_of(ids[i]) {
                    if seen.insert(c) {
                        ids.push(c);
                    }
                }
                i += 1;
            }
        }
        let db = self.shared.core.db.clone();
        for a in ids {
            artifacts::set_artifact_priority(&db, a, priority)
                .await
                .map_err(|e| internal(e.into()))?;
            jobs::set_job_priority_for_artifact(&db, a, priority)
                .await
                .map_err(|e| internal(e.into()))?;
            if let Some(art) = self.artifacts.get_mut(&a) {
                art.priority = priority;
            }
            let job_ids: Vec<i64> = self
                .by_artifact
                .get(&a)
                .into_iter()
                .flatten()
                .copied()
                .collect();
            for j in job_ids {
                if let Some(job) = self.jobs.get_mut(&j)
                    && matches!(job.state.as_str(), "blocked" | "ready")
                {
                    job.priority = priority;
                }
            }
        }
        Ok(())
    }

    // ----- completion -----------------------------------------------------------

    /// Finalize the task when no work is left. Returns `true` when finished.
    async fn try_finish(&mut self) -> Result<bool> {
        let busy = self
            .jobs
            .values()
            .any(|j| matches!(j.state.as_str(), "blocked" | "ready" | "running"));
        if busy || !self.candidates.is_empty() || self.artifacts.values().any(awaiting_admission) {
            return Ok(false);
        }

        let task_id = self.task.id;
        let root_jobs: Vec<&Job> = self
            .artifacts
            .values()
            .find(|a| a.depth == 0)
            .map(|root| self.jobs_of(root.id).collect())
            .unwrap_or_default();
        let status = if self.user_canceled {
            TaskState::Canceled
        } else if self.deadline_hit {
            TaskState::Completed
        } else if !root_jobs.is_empty() && root_jobs.iter().all(|j| j.state == "failed") {
            TaskState::Failed
        } else {
            TaskState::Completed
        };
        let error_count = self.jobs.values().filter(|j| j.state == "failed").count();
        let artifact_count = self.artifacts.values().filter(|a| a.analyze).count();
        let partial = self.deadline_hit
            || self
                .artifacts
                .values()
                .any(|a| matches!(a.state.as_str(), "canceled" | "dropped"))
            || self.jobs.values().any(|j| {
                j.state == "skipped" && j.error_message.as_deref() == Some(BUDGET_MESSAGE)
            });
        let reports =
            plugin_reports::fetch_plugin_reports_for_task(&self.shared.core.db, task_id).await?;
        let verdict = task_verdict(&reports);

        tasks::finalize_task(
            &self.shared.core.db,
            task_id,
            &TaskFinalization {
                status,
                error_count: i32::try_from(error_count).unwrap_or(i32::MAX),
                artifact_count: i32::try_from(artifact_count).unwrap_or(i32::MAX),
                partial,
                verdict_classification: verdict.and_then(|r| r.classification),
                verdict_score: verdict.and_then(|r| r.score),
                verdict_artifact_id: verdict.map(|r| r.artifact_id),
            },
        )
        .await?;
        info!(
            task_id,
            status = status.as_str(),
            error_count,
            artifact_count,
            partial,
            "task finished"
        );
        self.emit(match status {
            TaskState::Canceled => Event::TaskCanceled { task_id },
            TaskState::Failed => Event::TaskFailed { task_id },
            _ => Event::TaskCompleted { task_id },
        });
        Ok(true)
    }
}
