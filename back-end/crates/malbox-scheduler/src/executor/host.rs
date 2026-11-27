//! Host job executor: claims ready host jobs up to `host_concurrency`, one
//! per host plugin at a time.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use malbox_database::repositories::jobs::{self, Job};
use malbox_plugin_transport::job_config;
use tokio::task::JoinSet;
use tracing::{debug, warn};

use super::{ErrorClass, fail_job, finish_job, load_job_inputs};
use crate::Shared;
use crate::jobctx::build_job_config;
use crate::runner::{HostJobRequest, HostJobRunner, JobRunError, PluginOutputs};

const POLL_INTERVAL: Duration = Duration::from_secs(2);

type BusyPlugins = Arc<Mutex<HashSet<String>>>;

fn busy_lock(busy: &BusyPlugins) -> std::sync::MutexGuard<'_, HashSet<String>> {
    busy.lock().unwrap_or_else(|e| e.into_inner())
}

pub(crate) async fn run(shared: Arc<Shared>, runner: Arc<dyn HostJobRunner>) {
    let busy: BusyPlugins = Arc::default();
    let mut inflight = JoinSet::new();
    let mut interval = tokio::time::interval(POLL_INTERVAL);
    let notify = shared.executors.host_notify().clone();
    let concurrency = shared.core.settings.host_concurrency.max(1) as usize;
    let per_task = i64::from(shared.core.settings.max_running_jobs_per_task.max(1));

    loop {
        tokio::select! {
            _ = shared.token.cancelled() => break,
            _ = notify.notified() => {}
            _ = interval.tick() => {}
            Some(_) = inflight.join_next() => {}
        }
        while inflight.try_join_next().is_some() {}

        while inflight.len() < concurrency && !shared.token.is_cancelled() {
            let busy_list: Vec<String> = busy_lock(&busy).iter().cloned().collect();
            match jobs::claim_host_job(&shared.core.db, &busy_list, per_task).await {
                Ok(Some(job)) => {
                    debug!(job_id = job.id, analyzer = %job.analyzer, "claimed host job");
                    busy_lock(&busy).insert(job.analyzer.clone());
                    inflight.spawn(run_one(shared.clone(), runner.clone(), job, busy.clone()));
                }
                Ok(None) => break,
                Err(e) => {
                    warn!(error = %e, "failed to claim host job");
                    break;
                }
            }
        }
    }
    while inflight.join_next().await.is_some() {}
}

async fn run_one(shared: Arc<Shared>, runner: Arc<dyn HostJobRunner>, job: Job, busy: BusyPlugins) {
    let job_id = job.id;
    let analyzer = job.analyzer.clone();
    execute(&shared, &*runner, job).await;
    shared.running.remove(job_id);
    busy_lock(&busy).remove(&analyzer);
    shared.executors.host_notify().notify_one();
}

async fn execute(shared: &Shared, runner: &dyn HostJobRunner, job: Job) {
    let Some(spec) = shared.routing.analyzer(&job.analyzer).cloned() else {
        let message = format!("analyzer {} is not registered", job.analyzer);
        fail_job(shared, &job, ErrorClass::Nonrecoverable, message).await;
        return;
    };
    let (task, artifact) = match load_job_inputs(shared, &job).await {
        Ok(v) => v,
        Err(e) => {
            fail_job(shared, &job, ErrorClass::Recoverable, e.to_string()).await;
            return;
        }
    };
    let config = match build_job_config(&shared.core, &task, &job, &artifact, Some(&spec)).await {
        Ok(c) => c,
        Err(e) => {
            fail_job(shared, &job, ErrorClass::Recoverable, e.to_string()).await;
            return;
        }
    };
    // `malbox.max_artifacts` is capped by the task's remaining artifact
    // budget. Output that reached the cap depends on this task.
    let artifact_cap = config
        .get(job_config::MAX_ARTIFACTS)
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    let sample_path = match shared.core.sample_store.path(&artifact.sha256) {
        Ok(p) => p,
        Err(e) => {
            fail_job(shared, &job, ErrorClass::Nonrecoverable, e.to_string()).await;
            return;
        }
    };

    let token = shared.running.register(job.id);
    let result = if token.is_cancelled() {
        Err(JobRunError::Canceled)
    } else {
        let req = HostJobRequest {
            task_id: task.id,
            job: job.clone(),
            spec: spec.clone(),
            sample_path,
            config,
        };
        runner.run(req, token).await
    };
    if shared.token.is_cancelled() {
        // Shutdown: leave the job running; startup recovery re-queues it.
        return;
    }
    let cap_reached = result.as_ref().is_ok_and(|outputs| {
        let analysed = outputs
            .iter()
            .filter(|o| o.artifact.is_some_and(|m| m.analyze))
            .count();
        analysed >= artifact_cap
    });
    let result = result.map(|outputs| {
        vec![PluginOutputs {
            plugin: spec.name.clone(),
            result: Ok(outputs),
        }]
    });
    let cache_spec = (!cap_reached).then_some(&spec);
    finish_job(shared, task, job, artifact, cache_spec, result).await;
}
