//! Example guest plugin — YARA-like signature scanner.
//!
//! Runs inside a guest VM, communicating with the daemon over gRPC.
//! When a task starts the daemon pushes the sample file into the guest's
//! working directory. This plugin scans it against a set of built-in
//! signature rules and emits match results.
//!
//! ```sh
//! cargo run --manifest-path examples/plugins/guest-yara-scanner/Cargo.toml
//! ```

use malbox_plugin_sdk::prelude::*;
use tracing::info;

/// A trivial signature rule for demonstration purposes.
struct Rule {
    name: &'static str,
    pattern: &'static [u8],
}

/// Built-in demo rules. A real plugin would load compiled YARA rules from disk.
const RULES: &[Rule] = &[
    Rule {
        name: "MZ_Header",
        pattern: b"MZ",
    },
    Rule {
        name: "ELF_Header",
        pattern: b"\x7fELF",
    },
    Rule {
        name: "PDF_Header",
        pattern: b"%PDF",
    },
    Rule {
        name: "PK_Archive",
        pattern: b"PK\x03\x04",
    },
];

struct YaraScannerPlugin;

impl Plugin for YaraScannerPlugin {
    fn name(&self) -> &str {
        "guest-yara-scanner"
    }

    fn plugin_id(&self) -> i32 {
        200
    }

    fn on_start(&self, _ctx: &EventContext) -> Result<()> {
        info!(rule_count = RULES.len(), "YARA scanner ready");
        Ok(())
    }

    fn on_stop(&self, _ctx: &EventContext) -> Result<()> {
        info!("YARA scanner shutting down");
        Ok(())
    }

    fn on_task_event(
        &self,
        event: TaskEvent,
        payload: TaskEventPayload,
        ctx: &EventContext,
    ) -> Result<()> {
        let task_id = payload.task_id;

        match event {
            TaskEvent::TaskStarting => {
                info!(task_id, "Scanning sample");
                ctx.emit_plugin_started(self.plugin_id())?;

                // In a real guest plugin the daemon would have already pushed
                // the sample via `push_file` into our work_dir. We simulate
                // scanning by checking demo bytes.
                let sample_bytes: &[u8] = b"MZ\x90\x00PE\x00\x00";
                let matches = scan(sample_bytes);

                if matches.is_empty() {
                    info!(task_id, "No rules matched");
                } else {
                    info!(task_id, matched = ?matches, "Rules matched");
                }

                ctx.emit_result_produced(self.plugin_id())?;
                ctx.emit_plugin_stopped(self.plugin_id())?;
            }
            other => {
                info!(task_id, event = ?other, "Ignoring event");
            }
        }

        Ok(())
    }
}

/// Scan `data` against all built-in rules and return matched rule names.
fn scan(data: &[u8]) -> Vec<&'static str> {
    RULES
        .iter()
        .filter(|rule| {
            data.windows(rule.pattern.len())
                .any(|window| window == rule.pattern)
        })
        .map(|rule| rule.name)
        .collect()
}

fn main() {
    malbox_tracing::init_tracing("debug");
    info!("Starting guest-yara-scanner plugin on 0.0.0.0:50051");

    let config = GuestRuntimeConfig {
        work_dir: std::path::PathBuf::from("/tmp/malbox-yara"),
        ..Default::default()
    };

    let runtime = GuestPluginRuntime::with_config(YaraScannerPlugin, config);
    runtime.run_blocking().expect("Guest plugin runtime error");
}
