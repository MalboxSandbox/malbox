use console::Term;
use rattles::presets::prelude as presets;
use std::io::Write;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// A terminal spinner using the rattles `diagswipe` braille glyph.
///
/// Renders on stderr so it does not interfere with piped stdout.
/// Automatically cleans up on drop.
pub struct Spinner {
    running: Arc<AtomicBool>,
    message: Arc<Mutex<String>>,
    handle: Option<JoinHandle<()>>,
}

impl Spinner {
    /// Start a spinner with the given message displayed beside the glyph.
    pub fn start(message: impl Into<String>) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let msg = Arc::new(Mutex::new(message.into()));

        let running_clone = Arc::clone(&running);
        let msg_clone = Arc::clone(&msg);

        let handle = if Term::stderr().is_term() {
            Some(thread::spawn(move || {
                let rattle = presets::diagswipe();
                while running_clone.load(Ordering::Relaxed) {
                    let text = msg_clone.lock().unwrap().clone();
                    eprint!("\r\x1b[2K{} {}", rattle.current_frame(), text);
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

    /// Update the message displayed beside the spinner.
    pub fn set_message(&self, message: impl Into<String>) {
        *self.message.lock().unwrap() = message.into();
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
