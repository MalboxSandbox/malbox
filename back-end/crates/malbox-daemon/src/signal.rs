// iceoryx2 unconditionally overwrites process-wide signal handlers via
// sigaction during shared memory initialization (SignalHandler::new() is
// triggered by call_and_fetch in shared_memory.rs). After that, SIGTERM
// writes to iceoryx2's LAST_SIGNAL atomic but never notifies tokio's
// signal pipe — the daemon becomes unkillable.
//
// We sidestep this entirely with signal-hook's Signals iterator, which
// (re-)installs its own sigaction handler and uses a self-pipe internally.
// The registration happens synchronously in spawn_signal_handlers(),
// called right after IpcReactor::spawn(), so it overwrites iceoryx2's
// handler. A dedicated OS thread blocks on the pipe — no tokio polling
// required, no signal-mask coordination across threads.

use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

pub(crate) fn spawn_signal_handlers(shutdown_token: &CancellationToken) {
    use signal_hook::consts::{SIGINT, SIGTERM};
    use signal_hook::iterator::Signals;

    let mut signals = Signals::new([SIGTERM, SIGINT]).expect("failed to register signal handlers");

    let graceful = shutdown_token.clone();

    std::thread::Builder::new()
        .name("malbox-signal".into())
        .spawn(move || {
            if let Some(sig) = signals.forever().next() {
                info!(
                    signal = sig,
                    "Received shutdown signal, starting graceful shutdown..."
                );
            }
            graceful.cancel();

            if let Some(sig) = signals.forever().next() {
                warn!(
                    signal = sig,
                    "Received second shutdown signal, forcing exit"
                );
            }
            std::process::exit(1);
        })
        .expect("failed to spawn signal handler thread");
}
