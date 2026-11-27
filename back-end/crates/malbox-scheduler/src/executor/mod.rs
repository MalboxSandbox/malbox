//! Job executors: claim ready jobs, run them through a runner, ingest the
//! outputs and report the outcome to the task's actor.

pub(crate) mod detonation;
pub(crate) mod host;

use std::sync::Arc;

use malbox_database::repositories::artifacts::{self, TaskArtifact};
use malbox_database::repositories::jobs::{self, Job};
use malbox_database::repositories::result_cache;
use malbox_database::repositories::tasks::{self, Task};
use time::OffsetDateTime;
use tokio::sync::Notify;
use tracing::{error, warn};

use crate::Shared;
use crate::actor::{ErrorClass, JobFinished, JobOutcome};
use crate::cache::is_cacheable;
use crate::config::to_i64;
use crate::coordinator::Command;
use crate::error::{Result, SchedulerError};
use crate::ingest::{IngestContext, IngestSummary, ingest_outputs};
use crate::plan::JobKindPlan;
use crate::routing::AnalyzerSpec;
use crate::runner::{JobRunError, LAUNCH_FAILURE_PLUGIN, PluginOutputs};
use crate::submission::task_params;

/// Wake-ups for the executor loops.
#[derive(Default)]
pub(crate) struct Executors {
    host_notify: Arc<Notify>,
    detonation_notify: Arc<Notify>,
}

impl Executors {
    pub fn notify(&self, kind: JobKindPlan) {
        match kind {
            JobKindPlan::Host => self.host_notify.notify_one(),
            JobKindPlan::Detonation => self.detonation_notify.notify_one(),
        }
    }

    pub fn host_notify(&self) -> &Arc<Notify> {
        &self.host_notify
    }

    pub fn detonation_notify(&self) -> &Arc<Notify> {
        &self.detonation_notify
    }
}

fn class_of(e: &JobRunError) -> ErrorClass {
    match e {
        JobRunError::Timeout(_) => ErrorClass::Timeout,
        JobRunError::Recoverable(_) | JobRunError::Canceled => ErrorClass::Recoverable,
        JobRunError::Nonrecoverable(_) => ErrorClass::Nonrecoverable,
        JobRunError::MachineLost(_) => ErrorClass::MachineLost,
    }
}

/// Load the job's task and input artifact.
pub(crate) async fn load_job_inputs(shared: &Shared, job: &Job) -> Result<(Task, TaskArtifact)> {
    let db = &shared.core.db;
    let task = tasks::fetch_task(db, job.task_id)
        .await?
        .ok_or_else(|| SchedulerError::Internal(format!("task {} not found", job.task_id)))?;
    let artifact = artifacts::fetch_artifact(db, job.artifact_id)
        .await?
        .ok_or_else(|| {
            SchedulerError::Internal(format!("artifact {} not found", job.artifact_id))
        })?;
    Ok((task, artifact))
}

pub(crate) fn send_finished(shared: &Shared, task_id: i32, finished: JobFinished) {
    // The coordinator only stops on shutdown, when outcomes are no longer
    // needed (unfinished jobs are recovered on the next start).
    let _ = shared
        .commands
        .send(Command::JobFinished(task_id, finished));
}

/// Mark a job failed for good and report it.
pub(crate) async fn fail_job(shared: &Shared, job: &Job, class: ErrorClass, message: String) {
    if let Err(e) = jobs::set_job_state(
        &shared.core.db,
        job.id,
        "failed",
        Some(class.as_str()),
        Some(&message),
    )
    .await
    {
        error!(job_id = job.id, error = %e, "failed to mark job failed");
    }
    send_finished(
        shared,
        job.task_id,
        JobFinished {
            job_id: job.id,
            outcome: JobOutcome::Failed { class, message },
        },
    );
}

/// Record a run's result: ingest outputs, set the job state, store the
/// cache entry, schedule a retry or report the outcome.
///
/// `cache_spec` is the analyzer of a host job whose result may be cached;
/// `None` for detonations and for runs whose outputs depend on the task.
pub(crate) async fn finish_job(
    shared: &Shared,
    task: Task,
    job: Job,
    artifact: TaskArtifact,
    cache_spec: Option<&AnalyzerSpec>,
    result: Result<Vec<PluginOutputs>, JobRunError>,
) {
    let job_id = job.id;
    let task_id = job.task_id;
    let outcome = match finish_inner(shared, task, job, artifact, cache_spec, result).await {
        Ok(Some(outcome)) => outcome,
        Ok(None) => {
            let _ = shared.commands.send(Command::JobRetrying(task_id, job_id));
            return;
        }
        Err(e) => {
            error!(job_id, error = %e, "failed to record job result");
            let message = format!("failed to record result: {e}");
            if let Err(e) = jobs::set_job_state(
                &shared.core.db,
                job_id,
                "failed",
                Some(ErrorClass::Recoverable.as_str()),
                Some(&message),
            )
            .await
            {
                error!(job_id, error = %e, "failed to mark job failed");
            }
            JobOutcome::Failed {
                class: ErrorClass::Recoverable,
                message,
            }
        }
    };
    send_finished(shared, task_id, JobFinished { job_id, outcome });
}

/// `Ok(None)` means a retry was scheduled.
async fn finish_inner(
    shared: &Shared,
    task: Task,
    job: Job,
    artifact: TaskArtifact,
    cache_spec: Option<&AnalyzerSpec>,
    result: Result<Vec<PluginOutputs>, JobRunError>,
) -> Result<Option<JobOutcome>> {
    let core = &shared.core;
    let db = &core.db;

    let per_plugin = match result {
        Ok(per_plugin) => per_plugin,
        Err(JobRunError::Canceled) => {
            jobs::set_job_state(db, job.id, "canceled", None, None).await?;
            return Ok(Some(JobOutcome::Canceled));
        }
        Err(e) => return handle_error(shared, &job, e).await,
    };

    let (launch_failures, per_plugin): (Vec<_>, Vec<_>) = per_plugin
        .into_iter()
        .partition(|p| p.plugin == LAUNCH_FAILURE_PLUGIN);
    let launch_error = launch_failures
        .into_iter()
        .find_map(|p| p.result.err())
        .map(|e| e.to_string());

    let any_ok = per_plugin.is_empty() || per_plugin.iter().any(|p| p.result.is_ok());
    if !any_ok {
        if per_plugin
            .iter()
            .all(|p| matches!(p.result, Err(JobRunError::Canceled)))
        {
            jobs::set_job_state(db, job.id, "canceled", None, None).await?;
            return Ok(Some(JobOutcome::Canceled));
        }
        let worst = per_plugin
            .into_iter()
            .filter_map(|p| p.result.err())
            .filter(|e| !matches!(e, JobRunError::Canceled))
            .max_by_key(class_of)
            .unwrap_or(JobRunError::Recoverable(
                "no plugin produced a result".into(),
            ));
        return handle_error(shared, &job, worst).await;
    }

    let ctx = IngestContext {
        task,
        job,
        artifact,
    };
    let mut summary = IngestSummary::default();
    let mut messages: Vec<String> = Vec::new();
    for p in per_plugin {
        match p.result {
            Ok(outputs) => {
                let s = ingest_outputs(core, &ctx, &p.plugin, outputs).await?;
                summary.merge(s);
            }
            Err(e) => messages.push(format!("guest plugin {} failed: {e}", p.plugin)),
        }
    }
    if let Some(m) = summary.discard_message() {
        messages.insert(0, m);
    }

    let job = &ctx.job;
    let failure = if let Some(e) = &summary.invalid_report {
        Some(format!("invalid report: {e}"))
    } else {
        launch_error.map(|e| format!("launch failed: {e}"))
    };
    match failure {
        Some(f) => {
            messages.insert(0, f);
            jobs::set_job_state(
                db,
                job.id,
                "failed",
                Some(ErrorClass::Nonrecoverable.as_str()),
                Some(&messages.join("; ")),
            )
            .await?;
        }
        None => {
            let message = (!messages.is_empty()).then(|| messages.join("; "));
            jobs::set_job_state(db, job.id, "completed", None, message.as_deref()).await?;
            if let (Some(spec), Some(key)) = (cache_spec, &job.cache_key) {
                let params = task_params(&ctx.task);
                if is_cacheable(spec, &params)
                    && job.reruns == 0
                    && !job.ignore_cache
                    && summary.changed_shared_keys.is_empty()
                    && !summary.drop_requested
                    && !summary.budget_limited
                {
                    let empty = !summary.had_report && summary.new_artifacts.is_empty();
                    let ttl = if empty {
                        core.settings.empty_result_cache_ttl_secs
                    } else {
                        core.settings.cache_ttl_secs
                    };
                    if let Err(e) = result_cache::store(db, key, job.id, empty, to_i64(ttl)).await {
                        warn!(job_id = job.id, error = %e, "failed to store cache entry");
                    }
                }
            }
        }
    }
    Ok(Some(JobOutcome::Completed(summary)))
}

async fn handle_error(shared: &Shared, job: &Job, e: JobRunError) -> Result<Option<JobOutcome>> {
    let db = &shared.core.db;
    let class = class_of(&e);
    let message = e.to_string();
    if class != ErrorClass::Nonrecoverable && job.attempts < job.max_attempts {
        let backoff = shared
            .core
            .settings
            .retry_backoff_secs
            .saturating_mul(job.attempts.max(0) as u64);
        let not_before = OffsetDateTime::now_utc() + time::Duration::seconds(to_i64(backoff));
        jobs::schedule_retry(db, job.id, class.as_str(), &message, not_before).await?;
        warn!(job_id = job.id, attempts = job.attempts, error = %message, "job failed, retrying");
        return Ok(None);
    }
    jobs::set_job_state(db, job.id, "failed", Some(class.as_str()), Some(&message)).await?;
    Ok(Some(JobOutcome::Failed { class, message }))
}
