extern crate malbox_plugin_sdk as malbox;

use malbox::prelude::*;
use std::process::Command;

/// Snapshot of all running processes at the time of the check.
#[derive(Debug, Serialize)]
struct ExecStatus {
    /// The sample target name for reference.
    target: String,
    /// Total number of processes found.
    process_count: usize,
    /// Full process listing output lines.
    processes: Vec<String>,
}

#[malbox::guest_plugin]
#[malbox(state = "ephemeral", execution = "exclusive")]
struct ExecChecker;

#[malbox::handlers]
impl ExecChecker {
    #[malbox::on_start]
    fn init(&self) -> Result<()> {
        info!("Exec checker ready");
        Ok(())
    }

    #[malbox::on_stop]
    fn shutdown(&self) -> Result<()> {
        info!("Exec checker shutting down");
        Ok(())
    }

    #[malbox::on_task]
    fn check(&self, task: Task, ctx: &Context) -> Result<()> {
        let target = task
            .sample_path()
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        ctx.emit_progress(0.5, "listing running processes")?;

        let processes = list_processes();

        info!(
            task_id = task.id(),
            target = target.as_str(),
            process_count = processes.len(),
            "Process listing captured"
        );

        let status = ExecStatus {
            target,
            process_count: processes.len(),
            processes,
        };

        ctx.push_result(PluginResult::json("exec_status", &status)?)?;
        Ok(())
    }
}

/// Get a list of all running processes.
///
/// On Windows uses `tasklist`, on Linux uses `ps aux`.
fn list_processes() -> Vec<String> {
    let output = if cfg!(target_os = "windows") {
        Command::new("tasklist").output()
    } else {
        Command::new("ps").args(["aux"]).output()
    };

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.lines().map(|s| s.to_string()).collect()
        }
        Err(e) => {
            eprintln!("Failed to run process listing command: {}", e);
            vec![]
        }
    }
}
