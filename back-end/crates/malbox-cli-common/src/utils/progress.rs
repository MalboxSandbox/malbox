//! Animated terminal spinner for long-running CLI operations.

#![expect(
    clippy::print_stderr,
    reason = "spinner renders user-facing progress to the terminal's stderr"
)]

use console::Term;
use rattles::presets::prelude as presets;
use std::io::Write;
use std::sync::{
    Arc, Mutex, PoisonError,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::utils::format::Brand;

/// Spinner drawn on stderr by a background thread until dropped.
///
/// Draws nothing when stderr is not a terminal.
#[derive(Debug)]
pub struct Spinner {
    running: Arc<AtomicBool>,
    message: Arc<Mutex<String>>,
    handle: Option<JoinHandle<()>>,
}

impl Spinner {
    /// Starts a spinner showing `message` next to the animation.
    pub fn start(message: impl Into<String>) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let msg = Arc::new(Mutex::new(message.into()));

        let running_clone = Arc::clone(&running);
        let msg_clone = Arc::clone(&msg);

        let handle = if Term::stderr().is_term() {
            Some(thread::spawn(move || {
                let rattle = presets::diagswipe();
                let style = Brand::accent();
                while running_clone.load(Ordering::Relaxed) {
                    let text = msg_clone
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .clone();
                    eprint!(
                        "\r\x1b[2K  {} {}",
                        style.apply_to(rattle.current_frame()),
                        text
                    );
                    std::io::stderr().flush().ok();
                    thread::sleep(Duration::from_millis(60));
                }
            }))
        } else {
            None
        };

        Self {
            running,
            message: msg,
            handle,
        }
    }

    /// Replaces the text shown next to the animation.
    pub fn set_message(&self, message: impl Into<String>) {
        *self.message.lock().unwrap_or_else(PoisonError::into_inner) = message.into();
    }

    fn shutdown(&mut self) {
        if self.handle.is_none() {
            return;
        }
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            handle.join().ok();
        }
        eprint!("\r\x1b[2K");
        std::io::stderr().flush().ok();
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.shutdown();
    }
}
