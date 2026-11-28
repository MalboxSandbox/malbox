//! Cancellation tokens of running jobs.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use tokio_util::sync::CancellationToken;

#[derive(Default)]
struct RunningInner {
    tokens: HashMap<i64, CancellationToken>,
    /// Cancels that arrived between a job's claim and its registration.
    pending_cancel: HashSet<i64>,
}

pub(crate) struct RunningJobs {
    parent: CancellationToken,
    inner: Mutex<RunningInner>,
}

impl RunningJobs {
    pub(crate) fn new(parent: CancellationToken) -> Self {
        Self {
            parent,
            inner: Mutex::new(RunningInner::default()),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, RunningInner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Token for a job that is about to run. Already cancelled when a cancel
    /// for it arrived first.
    pub(crate) fn register(&self, job_id: i64) -> CancellationToken {
        let token = self.parent.child_token();
        let mut inner = self.lock();
        if inner.pending_cancel.remove(&job_id) {
            token.cancel();
        }
        inner.tokens.insert(job_id, token.clone());
        token
    }

    /// Cancel a job's token, or remember the cancel until it registers.
    /// Returns `true` when a registered token was cancelled.
    pub(crate) fn cancel(&self, job_id: i64) -> bool {
        let mut inner = self.lock();
        if let Some(token) = inner.tokens.get(&job_id) {
            token.cancel();
            true
        } else {
            inner.pending_cancel.insert(job_id);
            false
        }
    }

    pub(crate) fn remove(&self, job_id: i64) {
        let mut inner = self.lock();
        inner.tokens.remove(&job_id);
        inner.pending_cancel.remove(&job_id);
    }

    pub(crate) fn cancel_all(&self) {
        for token in self.lock().tokens.values() {
            token.cancel();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_before_register_is_remembered() {
        let r = RunningJobs::new(CancellationToken::new());
        assert!(!r.cancel(7));
        assert!(r.register(7).is_cancelled());
        r.remove(7);
        assert!(!r.register(7).is_cancelled());
    }

    #[test]
    fn cancel_registered_token() {
        let r = RunningJobs::new(CancellationToken::new());
        let t = r.register(1);
        assert!(r.cancel(1));
        assert!(t.is_cancelled());
    }

    #[test]
    fn parent_cancels_children() {
        let parent = CancellationToken::new();
        let r = RunningJobs::new(parent.clone());
        let t = r.register(1);
        parent.cancel();
        assert!(t.is_cancelled());
    }
}
