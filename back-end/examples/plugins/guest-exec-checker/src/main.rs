extern crate malbox_plugin_sdk as malbox;

use malbox::prelude::*;
use std::process::Command;

/// Snapshot of all running processes at the time of the check.
#[derive(Debug, Serialize)]
struct ExecStatus {
    /// The sample target name for reference.
    target: String,
    /// Whether a process referencing the sample was found.
    sample_running: bool,
    /// Process listing lines that reference the sample.
    sample_processes: Vec<String>,
    /// Total number of processes found.
    process_count: usize,
    /// Full process listing output lines.
    processes: Vec<String>,
}

#[malbox::guest_plugin]
struct ExecChecker;

#[malbox::handlers]
impl ExecChecker {
    #[malbox::on_start]
    fn init(&self) -> Result<()> {
        info!("Exec checker ready");
        Ok(())
    }

    #[malbox::on_stop]
    fn collect(&self, ctx: &Context) -> Result<()> {
        let target = ctx
            .task()
            .sample_path()
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        ctx.progress(0.5, "listing running processes")?;

        let processes = list_processes();
        let sample_processes = matching_processes(&target, &processes);

        info!(
            task_id = ctx.task().id(),
            target = target.as_str(),
            process_count = processes.len(),
            sample_running = !sample_processes.is_empty(),
            "Process listing captured"
        );

        let status = ExecStatus {
            target,
            sample_running: !sample_processes.is_empty(),
            sample_processes,
            process_count: processes.len(),
            processes,
        };

        ctx.results()
            .push(build_report(&status).into_plugin_result()?)?;
        ctx.results()
            .push(PluginResult::json("exec_status", &status)?)?;
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

/// Process listing lines that mention the sample's file name.
///
/// `tasklist` truncates image names to 25 characters, so on Windows samples
/// with longer names are not matched.
fn matching_processes(target: &str, processes: &[String]) -> Vec<String> {
    if target.is_empty() || target == "unknown" {
        return Vec::new();
    }
    processes
        .iter()
        .filter(|line| line.contains(target))
        .cloned()
        .collect()
}

/// Report envelope for the UI. Whether the sample is still running says
/// nothing about maliciousness, so the verdict is `Unknown`; the finding is
/// carried by the summary, a label and the overview section.
fn build_report(status: &ExecStatus) -> Report {
    let matched = status.sample_processes.len();
    let summary = if status.sample_running {
        format!(
            "Sample `{}` was running at the end of the analysis ({} matching process{}).",
            status.target,
            matched,
            if matched == 1 { "" } else { "es" }
        )
    } else {
        format!(
            "No process of sample `{}` was running at the end of the analysis: it exited, crashed, or never started.",
            status.target
        )
    };

    ReportBuilder::new("guest-exec-checker", env!("CARGO_PKG_VERSION"))
        .display_name("Execution Checker")
        .summary(summary.clone())
        .verdict(Classification::Unknown, None, None)
        .labels([if status.sample_running {
            "sample-running"
        } else {
            "sample-not-running"
        }])
        .artifact(
            ArtifactRef::new("exec_status", "process-list")
                .description("Full process listing as JSON"),
        )
        .section("overview", "Overview", |s| {
            let s = s.kv([
                KvPair {
                    key: "Sample".into(),
                    value: status.target.clone(),
                    mono: true,
                },
                KvPair {
                    key: "Running at end of analysis".into(),
                    value: if status.sample_running { "yes" } else { "no" }.into(),
                    mono: false,
                },
                KvPair {
                    key: "Matching processes".into(),
                    value: matched.to_string(),
                    mono: false,
                },
                KvPair {
                    key: "Total processes".into(),
                    value: status.process_count.to_string(),
                    mono: false,
                },
            ]);
            let level = if status.sample_running {
                CalloutLevel::Success
            } else {
                CalloutLevel::Warn
            };
            s.callout(level, summary)
        })
        .section("processes", "Processes", |s| {
            let s = if status.sample_running {
                s.heading(3, "Sample processes")
                    .code("text", status.sample_processes.join("\n"))
            } else {
                s
            };
            s.heading(3, "All processes")
                .code("text", status.processes.join("\n"))
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn status(target: &str, processes: &[&str]) -> ExecStatus {
        let processes = lines(processes);
        let sample_processes = matching_processes(target, &processes);
        ExecStatus {
            target: target.into(),
            sample_running: !sample_processes.is_empty(),
            sample_processes,
            process_count: processes.len(),
            processes,
        }
    }

    #[test]
    fn running_sample_is_reported_as_running() {
        let s = status(
            "sample.sh",
            &[
                "PID   USER     TIME  COMMAND",
                "    3 sandbox   0:00 /opt/malbox/plugins/guest-exec-checker/guest-exec-checker",
                "    6 sandbox   0:00 /bin/sh /tmp/malbox/samples/sample.sh",
            ],
        );
        assert!(s.sample_running);
        assert_eq!(s.sample_processes.len(), 1);

        let report = build_report(&s);
        let verdict = report.verdict.as_ref().unwrap();
        assert_eq!(verdict.classification, Classification::Unknown);
        assert_eq!(verdict.labels, vec!["sample-running"]);
        assert!(report.summary.unwrap().contains("was running"));
    }

    #[test]
    fn exited_sample_is_reported_as_not_running() {
        let s = status(
            "sample.sh",
            &[
                "PID   USER     TIME  COMMAND",
                "    1 root      0:00 tail -f /dev/null",
            ],
        );
        assert!(!s.sample_running);

        let report = build_report(&s);
        assert_eq!(report.verdict.unwrap().labels, vec!["sample-not-running"]);
    }

    #[test]
    fn unknown_target_matches_nothing() {
        assert!(matching_processes("unknown", &lines(&["unknown process"])).is_empty());
    }
}
