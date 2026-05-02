use std::collections::HashMap;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

pub struct TaskCancellationRegistry {
    tokens: RwLock<HashMap<i32, CancellationToken>>,
}

impl Default for TaskCancellationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskCancellationRegistry {
    pub fn new() -> Self {
        Self {
            tokens: RwLock::new(HashMap::new()),
        }
    }

    pub async fn register(&self, task_id: i32, token: CancellationToken) {
        self.tokens.write().await.insert(task_id, token);
    }

    /// Cancel a task by ID. Returns true if the task was found and cancelled.
    pub async fn cancel(&self, task_id: i32) -> bool {
        let tokens = self.tokens.read().await;
        if let Some(token) = tokens.get(&task_id) {
            token.cancel();
            true
        } else {
            false
        }
    }

    pub async fn remove(&self, task_id: i32) {
        self.tokens.write().await.remove(&task_id);
    }
}
