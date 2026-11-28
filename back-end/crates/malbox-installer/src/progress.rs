use crate::error::Step;

/// Receives install progress events so a front end can render them.
pub trait ProgressObserver: Send + Sync {
    /// A step with the given label began.
    fn step_started(&self, step: &str);
    /// A step finished; `detail` summarizes the result.
    fn step_completed(&self, step: &str, detail: &str);
    /// A step failed with the given error message.
    fn step_failed(&self, step: &str, error: &str);
    /// A source build compiled `compiled` of `total` units, most recently `crate_name`.
    fn build_progress(&self, compiled: usize, total: usize, crate_name: &str);
    /// A raw line of build tool output.
    fn build_output(&self, line: &str);
    /// `done` bytes of a download labelled `label` arrived; `total` when known.
    fn download_progress(&self, done: u64, total: Option<u64>, label: &str);
}

/// Observer that discards every progress event.
#[derive(Debug)]
pub struct NullObserver;

impl ProgressObserver for NullObserver {
    fn step_started(&self, _step: &str) {}
    fn step_completed(&self, _step: &str, _detail: &str) {}
    fn step_failed(&self, _step: &str, _error: &str) {}
    fn build_progress(&self, _compiled: usize, _total: usize, _crate_name: &str) {}
    fn build_output(&self, _line: &str) {}
    fn download_progress(&self, _done: u64, _total: Option<u64>, _label: &str) {}
}

/// Human-readable label shown while `step` runs.
#[must_use]
pub fn step_label(step: Step) -> &'static str {
    match step {
        Step::Daemon => "Installing malbox binaries",
        Step::Frontend => "Installing front-end assets",
        Step::Postgres => "Setting up PostgreSQL",
        Step::Config => "Generating default configuration",
        Step::Systemd => "Configuring systemd user services",
    }
}

pub(crate) fn observe<T>(
    step: Step,
    observer: &dyn ProgressObserver,
    result: crate::Result<T>,
) -> crate::Result<T> {
    if let Err(error) = &result {
        observer.step_failed(step_label(step), &error.to_string());
    }
    result
}
