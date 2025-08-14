use super::WorkerId;
use crate::error::{Result, WorkerError};
use crate::task::TaskResult;
use tokio::time::Duration;

#[derive(Debug)]
pub enum ShutdownReason {
    Requested,
    IdleTimeout,
    Error(String),
}

/// Events that workers send back to the pool for coordination.
#[derive(Debug)]
pub enum WorkerEvent {
    /// Worker has completed a job and is now idle.
    JobCompleted {
        worker_id: WorkerId,
        job_result: Result<TaskResult>,
        duration: Duration,
    },
    /// Worker has processed a batch and is now idle.
    BatchCompleted {
        worker_id: WorkerId,
        batch_size: usize,
        successful_count: usize,
        duration: Duration,
    },
    /// Worker is shutting down.
    WorkerShutdown {
        worker_id: WorkerId,
        reason: ShutdownReason,
    },
    /// Worker encountered an error.
    WorkerError {
        worker_id: WorkerId,
        error: WorkerError,
    },
}
