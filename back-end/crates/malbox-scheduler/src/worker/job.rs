use crate::error::Result;
use crate::task::{ResourceAllocation, TaskResult};
use malbox_database::repositories::tasks::Task;
use tokio::sync::oneshot;

pub struct Job {
    pub task: Task,
    pub resources: ResourceAllocation,
    pub result_tx: oneshot::Sender<Result<TaskResult>>,
}
