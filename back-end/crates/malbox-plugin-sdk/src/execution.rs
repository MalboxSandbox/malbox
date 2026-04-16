//! Execution notification channel for coordinating between on_task and
//! on_execute_command handlers.

use crate::error::{Result, SdkError};
use crate::types::ExecutionInfo;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

/// Sender half — held by the guest runtime, signaled after on_execute_command.
#[derive(Clone)]
pub struct ExecutionNotifier {
    inner: Arc<Inner>,
}

/// Receiver half — held by the Context, used by on_task via wait_for_execution.
#[derive(Clone)]
pub struct ExecutionWaiter {
    inner: Arc<Inner>,
}

struct Inner {
    state: Mutex<Option<ExecutionInfo>>,
    condvar: Condvar,
}

/// Create a paired (notifier, waiter).
pub fn execution_channel() -> (ExecutionNotifier, ExecutionWaiter) {
    let inner = Arc::new(Inner {
        state: Mutex::new(None),
        condvar: Condvar::new(),
    });
    (
        ExecutionNotifier {
            inner: inner.clone(),
        },
        ExecutionWaiter { inner },
    )
}

impl ExecutionNotifier {
    /// Signal that execution has started, unblocking any waiting on_task.
    pub fn notify(&self, info: ExecutionInfo) {
        let mut state = self.inner.state.lock().unwrap();
        *state = Some(info);
        self.inner.condvar.notify_all();
    }
}

impl ExecutionWaiter {
    /// Block until execution info is available or timeout expires.
    pub fn wait(&self, timeout: Duration) -> Result<ExecutionInfo> {
        let mut state = self.inner.state.lock().unwrap();
        let deadline = std::time::Instant::now() + timeout;

        while state.is_none() {
            let remaining = deadline
                .checked_duration_since(std::time::Instant::now())
                .unwrap_or(Duration::ZERO);
            if remaining.is_zero() {
                return Err(SdkError::Timeout {
                    operation: "wait_for_execution",
                    elapsed: timeout,
                });
            }
            let (new_state, timeout_result) =
                self.inner.condvar.wait_timeout(state, remaining).unwrap();
            state = new_state;
            if timeout_result.timed_out() && state.is_none() {
                return Err(SdkError::Timeout {
                    operation: "wait_for_execution",
                    elapsed: timeout,
                });
            }
        }

        Ok(state.take().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notify_before_wait_returns_immediately() {
        let (notifier, waiter) = execution_channel();
        notifier.notify(ExecutionInfo::new(Some(42), "test".into(), vec![]));
        let info = waiter.wait(Duration::from_secs(1)).unwrap();
        assert_eq!(info.pid(), Some(42));
    }

    #[test]
    fn wait_blocks_until_notify() {
        let (notifier, waiter) = execution_channel();
        let handle = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            notifier.notify(ExecutionInfo::new(
                Some(99),
                "cmd".into(),
                vec!["/c".into()],
            ));
        });
        let info = waiter.wait(Duration::from_secs(5)).unwrap();
        assert_eq!(info.pid(), Some(99));
        assert_eq!(info.command(), "cmd");
        handle.join().unwrap();
    }

    #[test]
    fn wait_times_out() {
        let (_notifier, waiter) = execution_channel();
        let result = waiter.wait(Duration::from_millis(50));
        assert!(matches!(
            result,
            Err(crate::error::SdkError::Timeout {
                operation: "wait_for_execution",
                ..
            })
        ));
    }
}
