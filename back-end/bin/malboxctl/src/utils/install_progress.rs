use malbox_cli_common::utils::format::Brand;
use malbox_cli_common::utils::progress::Spinner;
use malbox_installer::error::{InstallError, Step};
use malbox_installer::progress::InstallProgress;
use std::sync::Mutex;

pub struct CliProgress {
    spinner: Mutex<Option<Spinner>>,
}

impl CliProgress {
    pub fn new() -> Self {
        Self {
            spinner: Mutex::new(None),
        }
    }
}

impl Default for CliProgress {
    fn default() -> Self {
        Self::new()
    }
}

impl InstallProgress for CliProgress {
    fn started(&self, _step: Step, message: &str) {
        let mut guard = self.spinner.lock().unwrap();
        *guard = Some(Spinner::start(message));
    }

    fn progress(&self, _step: Step, _percent: u8, message: &str) {
        let guard = self.spinner.lock().unwrap();
        if let Some(spinner) = guard.as_ref() {
            spinner.set_message(message);
        }
    }

    fn completed(&self, step: Step) {
        let mut guard = self.spinner.lock().unwrap();
        *guard = None;
        let success = Brand::success();
        println!("  {} {step} complete", success.apply_to("\u{2713}"));
    }

    fn failed(&self, step: Step, error: &InstallError) {
        let mut guard = self.spinner.lock().unwrap();
        *guard = None;
        let err_style = Brand::error();
        eprintln!(
            "  {} {step} failed: {error}",
            err_style.apply_to("\u{2717}")
        );
    }
}
