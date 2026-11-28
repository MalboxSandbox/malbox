//! Detonation executor: matches ready detonation jobs to machines and runs
//! one session per acquired machine.

use std::sync::Arc;
use std::time::Duration;

use malbox_database::repositories::jobs::{self, Job};
use tokio::sync::Notify;
use tokio::task::JoinSet;
use tracing::{info, warn};

use super::{ErrorClass, fail_job, finish_job, load_job_inputs};
use crate::Shared;
use crate::jobctx::build_job_config;
use crate::plan::{DetonationRequest, UNMATCHED_PROFILE};
use crate::runner::{
    AcquireResult, DetonationJobRequest, DetonationRunner, JobRunError, LAUNCH_FAILURE_PLUGIN,
    Lease,
};

const POLL_INTERVAL: Duration = Duration::from_secs(2);
const SCAN_LIMIT: i64 = 20;

pub(crate) async fn run(
    shared: Arc<Shared>,
    runner: Arc<dyn DetonationRunner>,
    machine_available: Option<Arc<Notify>>,
) {
    let mut inflight = JoinSet::new();
    let mut interval = tokio::time::interval(POLL_INTERVAL);
    let notify = Arc::clone(shared.executors.detonation_notify());

    loop {
        let machine_freed = async {
            match &machine_available {
                Some(n) => n.notified().await,
                None => std::future::pending().await,
            }
        };
        tokio::select! {
            () = shared.token.cancelled() => break,
            () = notify.notified() => {}
            () = machine_freed => {}
            _ = interval.tick() => {}
            Some(_) = inflight.join_next() => {}
        }
        while inflight.try_join_next().is_some() {}

        let ready = match jobs::fetch_ready_detonation_jobs(&shared.core.db, SCAN_LIMIT).await {
            Ok(j) => j,
            Err(e) => {
                warn!(error = %e, "failed to fetch ready detonation jobs");
                continue;
            }
        };
        for job in ready {
            if shared.token.is_cancelled() {
                break;
            }
            let request = job
                .detonation
                .clone()
                .and_then(|v| serde_json::from_value::<DetonationRequest>(v).ok());
            let request = match request {
                Some(r) if r.profile != UNMATCHED_PROFILE => r,
                other => {
                    let message = match other {
                        Some(_) => "no detonation profile matches this artifact".to_string(),
                        None => "job has no valid detonation request".to_string(),
                    };
                    fail_unserviceable(&shared, &job, message).await;
                    continue;
                }
            };
            match runner.acquire(&request, job.task_id).await {
                AcquireResult::Busy => {}
                AcquireResult::Unserviceable(m) => fail_unserviceable(&shared, &job, m).await,
                AcquireResult::Acquired(lease) => {
                    match jobs::claim_job_by_id(&shared.core.db, job.id).await {
                        Ok(Some(job)) => {
                            if let Err(e) = jobs::set_job_machine(
                                &shared.core.db,
                                job.id,
                                lease.machine_id,
                                lease.snapshot_id,
                            )
                            .await
                            {
                                warn!(job_id = job.id, error = %e, "failed to record job machine");
                            }
                            info!(
                                job_id = job.id,
                                machine_id = lease.machine_id,
                                "detonation started"
                            );
                            inflight.spawn(run_one(
                                Arc::clone(&shared),
                                Arc::clone(&runner),
                                lease,
                                job,
                                request,
                            ));
                        }
                        Ok(None) => runner.abandon(lease).await,
                        Err(e) => {
                            warn!(job_id = job.id, error = %e, "failed to claim detonation job");
                            runner.abandon(lease).await;
                        }
                    }
                }
            }
        }
    }
    while inflight.join_next().await.is_some() {}
}

/// Claim the job (it may have been cancelled meanwhile) and fail it.
async fn fail_unserviceable(shared: &Shared, job: &Job, message: String) {
    match jobs::claim_job_by_id(&shared.core.db, job.id).await {
        Ok(Some(job)) => fail_job(shared, &job, ErrorClass::Nonrecoverable, message).await,
        Ok(None) => {}
        Err(e) => warn!(job_id = job.id, error = %e, "failed to claim detonation job"),
    }
}

async fn run_one(
    shared: Arc<Shared>,
    runner: Arc<dyn DetonationRunner>,
    lease: Lease,
    job: Job,
    request: DetonationRequest,
) {
    let job_id = job.id;
    execute(&shared, &*runner, lease, job, request).await;
    shared.running.remove(job_id);
    shared.executors.detonation_notify().notify_one();
}

async fn execute(
    shared: &Shared,
    runner: &dyn DetonationRunner,
    lease: Lease,
    job: Job,
    request: DetonationRequest,
) {
    let prepared = async {
        let (task, artifact) = load_job_inputs(shared, &job).await?;
        let base_config = build_job_config(&shared.core, &task, &job, &artifact, None).await?;
        let sample_path = shared.core.sample_store.path(&artifact.sha256)?;
        Ok::<_, crate::SchedulerError>((task, artifact, base_config, sample_path))
    }
    .await;
    let (task, artifact, base_config, sample_path) = match prepared {
        Ok(v) => v,
        Err(e) => {
            runner.abandon(lease).await;
            fail_job(shared, &job, ErrorClass::Recoverable, e.to_string()).await;
            return;
        }
    };

    let token = shared.running.register(job.id);
    let result = if token.is_cancelled() {
        runner.abandon(lease).await;
        Err(JobRunError::Canceled)
    } else {
        let req = DetonationJobRequest {
            task: task.clone(),
            job: job.clone(),
            artifact: artifact.clone(),
            request,
            sample_path,
            base_config,
        };
        runner.run(lease, req, token).await
    };
    if shared.token.is_cancelled() {
        // Shutdown: leave the job running; startup recovery re-queues it.
        return;
    }
    if let Ok(outputs) = &result {
        let names: Vec<String> = outputs
            .iter()
            .filter(|p| p.plugin != LAUNCH_FAILURE_PLUGIN)
            .map(|p| p.plugin.clone())
            .collect();
        if let Err(e) = jobs::set_job_guest_plugins(&shared.core.db, job.id, &names).await {
            warn!(job_id = job.id, error = %e, "failed to record guest plugins");
        }
    }
    finish_job(shared, task, job, artifact, None, result).await;
}
