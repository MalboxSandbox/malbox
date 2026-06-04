use crate::error::{InstallError, Step};

pub trait InstallProgress: Send + Sync {
    fn started(&self, step: Step, message: &str);
    fn progress(&self, step: Step, percent: u8, message: &str);
    fn completed(&self, step: Step);
    fn failed(&self, step: Step, error: &InstallError);
}

pub struct NoopProgress;

impl InstallProgress for NoopProgress {
    fn started(&self, _step: Step, _message: &str) {}
    fn progress(&self, _step: Step, _percent: u8, _message: &str) {}
    fn completed(&self, _step: Step) {}
    fn failed(&self, _step: Step, _error: &InstallError) {}
}

/// Report a step failure to the progress sink without disturbing the error
/// flow. Keeps the orchestrators free of repeated `if let Err` blocks.
pub(crate) fn observe<T>(
    step: Step,
    progress: &dyn InstallProgress,
    result: crate::Result<T>,
) -> crate::Result<T> {
    if let Err(error) = &result {
        progress.failed(step, error);
    }
    result
}
