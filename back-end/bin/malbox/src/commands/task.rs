use crate::commands::Command;
use clap::{Parser, Subcommand};
use malbox_cli_common::api::ApiClient;
use malbox_cli_common::api::tasks::{ArtifactInfo, SubmitTaskRequest, TaskTree};
use malbox_cli_common::context::Context;
use malbox_cli_common::error::{CliError, Result};
use malbox_cli_common::utils::format::{self, Detail, Table, styled_status};
use malbox_cli_common::utils::progress::Spinner;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{IsTerminal, Write};

#[derive(Parser)]
#[command(
    about = "Manage analysis tasks",
    long_about = "Submit samples for analysis, inspect task status, and retrieve artifacts.",
    after_help = "Examples:\n  \
                  malbox task list\n  \
                  malbox task get 7\n  \
                  malbox task get 7 --artifact 12\n  \
                  malbox task tree 7\n  \
                  malbox task cancel 7\n  \
                  malbox task submit sample.exe --timeout 120"
)]
pub struct TaskCommand {
    #[command(subcommand)]
    command: TaskCommands,
}

#[derive(Subcommand)]
enum TaskCommands {
    /// List all tasks
    List,
    /// Get details and artifacts of a specific task
    #[command(after_help = "Examples:\n  \
                            malbox task get 7\n  \
                            malbox task get 7 --artifact 12\n  \
                            malbox task get 7 --artifact 12 > output.bin")]
    Get(GetArgs),
    /// Submit a file for analysis
    #[command(after_help = "Examples:\n  \
                            malbox task submit malware.exe\n  \
                            malbox task submit sample.bin --platform linux --snapshot-id <uuid> --timeout 300\n  \
                            malbox task submit archive.zip --password infected --param host-archive-extractor.passwords='[\"x\"]'\n  \
                            malbox task submit doc.pdf --tags phishing campaign-2024 --priority 5")]
    Submit(Box<SubmitArgs>),
    /// Cancel a task
    Cancel {
        /// Task ID
        id: i32,
    },
    /// Show a task's artifact tree with its jobs
    Tree {
        /// Task ID
        id: i32,
    },
}

#[derive(Parser)]
struct GetArgs {
    /// Task ID (interactive selection if omitted)
    id: Option<i32>,
    /// Fetch and print the content of an artifact by its ID.
    /// JSON artifacts are pretty-printed to a terminal (raw when piped);
    /// binary artifacts are written raw when piped and refused on a TTY.
    #[arg(long)]
    artifact: Option<i64>,
}

#[derive(Parser)]
struct SubmitArgs {
    /// Path to the file to analyze
    file: String,
    /// Detonation timeout in seconds
    #[arg(long)]
    timeout: Option<i64>,
    /// Task priority (higher = more urgent)
    #[arg(long)]
    priority: Option<i64>,
    /// Tags for the task
    #[arg(long, num_args = 1..)]
    tags: Option<Vec<String>>,
    /// Task owner
    #[arg(long)]
    owner: Option<String>,
    /// Guest platform; with --snapshot-id the task runs on a machine
    #[arg(long, value_parser = ["linux", "windows"])]
    platform: Option<String>,
    /// Machine snapshot ID to analyse on (see `malbox machine snapshot list`)
    #[arg(long)]
    snapshot_id: Option<uuid::Uuid>,
    /// Machine ID to detonate on
    #[arg(long)]
    machine_id: Option<i32>,
    /// Plugins to run
    #[arg(long, num_args = 1..)]
    plugins: Option<Vec<String>>,
    /// Plugin to skip (repeatable)
    #[arg(long = "exclude-plugin")]
    exclude_plugin: Vec<String>,
    /// Sample file name inside the guest (defaults to the local file name)
    #[arg(long)]
    target_filename: Option<String>,
    /// Never detonate (host analysis only)
    #[arg(long)]
    no_detonate: bool,
    /// Which child artifacts may be detonated
    #[arg(long, value_parser = ["extracted", "none", "all"])]
    detonate_children: Option<String>,
    /// Maximum number of detonations for this task
    #[arg(long)]
    max_detonations: Option<u32>,
    /// Ask plugins for a deeper (slower) scan
    #[arg(long)]
    deep_scan: bool,
    /// Do not reuse cached host plugin results
    #[arg(long)]
    ignore_cache: bool,
    /// Password to try on encrypted content (repeatable)
    #[arg(long = "password")]
    passwords: Vec<String>,
    /// Plugin parameter as plugin.key=value; value is JSON or a plain string (repeatable)
    #[arg(long = "param")]
    params: Vec<String>,
}

impl Command for TaskCommand {
    async fn execute(self, ctx: &Context) -> Result<()> {
        match self.command {
            TaskCommands::List => list(&ctx.api).await,
            TaskCommands::Get(args) => get(ctx, args).await,
            TaskCommands::Submit(args) => submit(&ctx.api, *args).await,
            TaskCommands::Cancel { id } => cancel(&ctx.api, id).await,
            TaskCommands::Tree { id } => tree(&ctx.api, id).await,
        }
    }
}

async fn select_task(api: &ApiClient) -> Result<i32> {
    let tasks = api.list_tasks().await?;
    if tasks.is_empty() {
        return Err(CliError::InvalidArgument("no tasks available".to_string()));
    }
    let items: Vec<String> = tasks
        .iter()
        .map(|t| format!("#{} {} [{}] ({})", t.id, t.target, t.status, t.created_on))
        .collect();

    let selection = format::fuzzy_select("Select task", &items).ok_or_else(|| {
        CliError::InvalidArgument("task selection required (not a terminal?)".to_string())
    })?;

    Ok(tasks[selection].id)
}

async fn list(api: &ApiClient) -> Result<()> {
    let tasks = api.list_tasks().await?;

    if tasks.is_empty() {
        format::empty_with_hint(
            "No tasks found.",
            "Run 'malbox task submit <file>' to submit a sample for analysis.",
        );
        return Ok(());
    }

    let mut table = Table::new(&[
        "ID",
        "STATUS",
        "TARGET",
        "PLATFORM",
        "PRIORITY",
        "ARTIFACTS",
        "ERRORS",
        "CREATED",
    ]);

    for t in &tasks {
        let status = if t.partial {
            format!("{} (partial)", styled_status(&t.status))
        } else {
            styled_status(&t.status)
        };
        table.add_row(vec![
            t.id.to_string(),
            status,
            t.target.clone(),
            t.platform.clone(),
            t.priority.to_string(),
            t.artifact_count.to_string(),
            t.error_count.to_string(),
            format::format_time(&t.created_on),
        ]);
    }

    table.print();
    format::total(tasks.len(), "task");
    Ok(())
}

async fn get(ctx: &Context, args: GetArgs) -> Result<()> {
    let task_id = match args.id {
        Some(id) => id,
        None => select_task(&ctx.api).await?,
    };

    if let Some(artifact_id) = args.artifact {
        return print_artifact_content(&ctx.api, task_id, artifact_id).await;
    }

    let task = ctx.api.get_task(task_id).await?;

    let tags_display = task
        .tags
        .as_ref()
        .filter(|t| !t.is_empty())
        .map(|t| t.join(", "));

    let mut detail = Detail::new();
    detail
        .field("ID", task.id)
        .field_status("Status", &task.status)
        .field("Target", &task.target)
        .field("Platform", &task.platform)
        .field("Timeout", format!("{}s", task.timeout))
        .field("Priority", task.priority)
        .field_opt("Owner", task.owner.as_deref())
        .field_opt("Machine", task.machine_id)
        .field_opt("Tags", tags_display)
        .field("Artifacts", task.artifact_count)
        .field("Errors", task.error_count)
        .field("Partial", task.partial)
        .field("Created", format::format_time(&task.created_on))
        .field_opt(
            "Completed",
            task.completed_on.as_deref().map(format::format_time),
        );
    detail.print();

    let tree = ctx.api.get_task_tree(task_id).await?;
    format::section_header(&format!("Artifacts ({})", tree.artifacts.len()));

    if !tree.artifacts.is_empty() {
        let mut table = Table::new(&["ID", "NAME", "TYPE", "STATE", "SIZE"]);
        table.set_indent(2);

        for a in &tree.artifacts {
            table.add_row(vec![
                a.id.to_string(),
                sanitize_name(&a.name),
                a.artifact_type.clone(),
                styled_status(&a.state),
                format::bytes(a.size_bytes),
            ]);
        }

        table.print();
    }

    Ok(())
}

async fn print_artifact_content(api: &ApiClient, task_id: i32, artifact_id: i64) -> Result<()> {
    let tree = api.get_task_tree(task_id).await?;
    let artifact = tree
        .artifacts
        .iter()
        .find(|a| a.id == artifact_id)
        .ok_or_else(|| {
            CliError::InvalidArgument(format!(
                "artifact {} not found for task {}",
                artifact_id, task_id
            ))
        })?;
    if artifact.content_url.is_none() {
        return Err(CliError::InvalidArgument(format!(
            "artifact {} has no stored content",
            artifact_id
        )));
    }

    let content = api.get_artifact_content(task_id, artifact_id).await?;
    let stdout = std::io::stdout();
    let is_tty = stdout.is_terminal();
    let is_json = artifact.mime == "application/json";

    if is_json && is_tty {
        match serde_json::from_slice::<serde_json::Value>(&content) {
            Ok(v) => println!("{}", serde_json::to_string_pretty(&v)?),
            Err(e) => {
                eprintln!("warning: artifact labeled json but failed to parse ({e}); writing raw");
                stdout.lock().write_all(&content)?;
            }
        }
    } else if is_json || !is_tty {
        stdout.lock().write_all(&content)?;
    } else {
        eprintln!(
            "Artifact {} ({}): {} of binary data.",
            artifact.id,
            sanitize_name(&artifact.name),
            format::bytes(artifact.size_bytes),
        );
        eprintln!("Redirect to a file to save it, e.g.:");
        eprintln!(
            "  malbox task get {} --artifact {} > {}",
            task_id, artifact_id, artifact.sha256,
        );
    }

    Ok(())
}

/// Artifact names come from archive entries; control characters are replaced
/// so they cannot drive the terminal.
fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .collect()
}

async fn cancel(api: &ApiClient, id: i32) -> Result<()> {
    api.cancel_task(id).await?;
    format::success(format!("Task {id} canceled"));
    Ok(())
}

async fn tree(api: &ApiClient, id: i32) -> Result<()> {
    let tree = api.get_task_tree(id).await?;
    for line in render_tree(&tree) {
        println!("{line}");
    }
    Ok(())
}

/// Depth-first rendering: each artifact appears once, under its earliest
/// parent edge (edges arrive ordered by creation), siblings ordered by id.
fn render_tree(tree: &TaskTree) -> Vec<String> {
    let artifacts: HashMap<i64, &ArtifactInfo> = tree.artifacts.iter().map(|a| (a.id, a)).collect();
    let mut placed: HashSet<i64> = HashSet::new();
    let mut children: BTreeMap<i64, Vec<i64>> = BTreeMap::new();
    if let Some(root) = tree.root_artifact_id {
        placed.insert(root);
    }
    for e in &tree.edges {
        if placed.insert(e.child_artifact_id) {
            children
                .entry(e.parent_artifact_id)
                .or_default()
                .push(e.child_artifact_id);
        }
    }
    for list in children.values_mut() {
        list.sort_unstable();
    }

    let mut lines = Vec::new();
    let Some(root) = tree.root_artifact_id else {
        return lines;
    };
    let mut stack = vec![(root, 0usize)];
    let mut visited = HashSet::new();
    while let Some((id, level)) = stack.pop() {
        if !visited.insert(id) {
            continue;
        }
        let Some(a) = artifacts.get(&id) else {
            continue;
        };
        let indent = "  ".repeat(level);
        let mut line = format!(
            "{indent}{} {} [{}] {}",
            a.relation,
            sanitize_name(&a.name),
            a.artifact_type,
            a.state
        );
        if let Some(v) = &a.verdict {
            line.push(' ');
            line.push_str(&v.classification);
        }
        if let Some(r) = &a.drop_reason {
            line.push_str(&format!(" ({r})"));
        }
        lines.push(line);
        for j in tree.jobs.iter().filter(|j| j.artifact_id == id) {
            let mut job_line = format!("{indent}  - {} {}", j.analyzer, j.state);
            if let Some(m) = &j.error_message {
                job_line.push(' ');
                job_line.push_str(m);
            }
            lines.push(job_line);
        }
        if let Some(kids) = children.get(&id) {
            for kid in kids.iter().rev() {
                stack.push((*kid, level + 1));
            }
        }
    }
    lines
}

/// Builds the `params` JSON from the submit flags; only given keys are set.
fn build_params(args: &SubmitArgs) -> Result<Option<serde_json::Value>> {
    let mut params = serde_json::Map::new();
    if args.no_detonate {
        params.insert("detonate".into(), "never".into());
    }
    if let Some(mode) = &args.detonate_children {
        params.insert("detonate_children".into(), mode.clone().into());
    }
    if let Some(n) = args.max_detonations {
        params.insert("max_detonations".into(), n.into());
    }
    if args.deep_scan {
        params.insert("deep_scan".into(), true.into());
    }
    if args.ignore_cache {
        params.insert("ignore_cache".into(), true.into());
    }
    if !args.passwords.is_empty() {
        params.insert(
            "initial_data".into(),
            serde_json::json!({ "passwords": args.passwords }),
        );
    }
    if !args.params.is_empty() {
        let mut plugin_params: BTreeMap<String, serde_json::Map<String, serde_json::Value>> =
            BTreeMap::new();
        for raw in &args.params {
            let (plugin, key, value) = parse_param(raw)?;
            plugin_params.entry(plugin).or_default().insert(key, value);
        }
        params.insert("plugin_params".into(), serde_json::to_value(plugin_params)?);
    }
    Ok((!params.is_empty()).then_some(serde_json::Value::Object(params)))
}

fn parse_param(raw: &str) -> Result<(String, String, serde_json::Value)> {
    let invalid = || CliError::InvalidArgument("invalid --param, expected plugin.key=value".into());
    let (path, value) = raw.split_once('=').ok_or_else(invalid)?;
    let (plugin, key) = path.split_once('.').ok_or_else(invalid)?;
    if plugin.is_empty() || key.is_empty() {
        return Err(invalid());
    }
    let value = serde_json::from_str(value)
        .unwrap_or_else(|_| serde_json::Value::String(value.to_string()));
    Ok((plugin.to_string(), key.to_string(), value))
}

async fn submit(api: &ApiClient, args: SubmitArgs) -> Result<()> {
    let params = build_params(&args)?;
    let file_display = args.file.clone();
    let spinner = Spinner::start(format!("Submitting '{}'...", file_display));

    let response = api
        .submit_task(SubmitTaskRequest {
            file_path: args.file,
            timeout: args.timeout,
            priority: args.priority,
            tags: args.tags,
            owner: args.owner,
            platform: args.platform,
            snapshot_id: args.snapshot_id.map(|id| id.to_string()),
            machine_id: args.machine_id,
            plugins: args.plugins,
            excluded_plugins: (!args.exclude_plugin.is_empty()).then_some(args.exclude_plugin),
            target_filename: args.target_filename,
            params,
        })
        .await?;

    drop(spinner);
    format::success(format!("Task submitted (ID: {})", response.task_id));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use malbox_cli_common::api::tasks::{EdgeInfo, JobInfo};

    fn artifact(id: i64, relation: &str, name: &str, state: &str) -> ArtifactInfo {
        ArtifactInfo {
            id,
            sha256: format!("{id:064}"),
            size_bytes: 1,
            analyze: true,
            sample_id: None,
            depth: 0,
            artifact_type: "text/plain".into(),
            mime: "text/plain".into(),
            state: state.into(),
            drop_reason: None,
            priority: 1,
            canceled_by: None,
            promoted_task_id: None,
            name: name.into(),
            relation: relation.into(),
            verdict: None,
            content_url: None,
        }
    }

    fn job(artifact_id: i64, analyzer: &str, state: &str) -> JobInfo {
        JobInfo {
            id: artifact_id * 10,
            artifact_id,
            kind: "host".into(),
            analyzer: analyzer.into(),
            stage: "static".into(),
            state: state.into(),
            attempts: 1,
            max_attempts: 3,
            reruns: 0,
            error_class: None,
            error_message: None,
            cached_from_job_id: None,
            machine_id: None,
            started_on: None,
            completed_on: None,
        }
    }

    #[test]
    fn renders_tree_depth_first() {
        let tree = TaskTree {
            task_id: 1,
            root_artifact_id: Some(1),
            artifacts: vec![
                artifact(1, "root", "sample.zip", "done"),
                artifact(2, "extracted", "test-sample.sh", "done"),
            ],
            edges: vec![EdgeInfo {
                parent_artifact_id: 1,
                child_artifact_id: 2,
                relation: "extracted".into(),
                name: Some("test-sample.sh".into()),
                source_job_id: Some(10),
                source_plugin: Some("x".into()),
            }],
            jobs: vec![job(1, "x", "completed"), job(2, "linux-dev", "completed")],
        };
        assert_eq!(
            render_tree(&tree),
            vec![
                "root sample.zip [text/plain] done",
                "  - x completed",
                "  extracted test-sample.sh [text/plain] done",
                "    - linux-dev completed",
            ]
        );
    }

    #[test]
    fn parses_params() {
        assert_eq!(
            parse_param("p.k=[\"a\"]").unwrap(),
            ("p".into(), "k".into(), serde_json::json!(["a"]))
        );
        assert_eq!(
            parse_param("p.k=plain").unwrap().2,
            serde_json::json!("plain")
        );
        assert!(parse_param("nokey=1").is_err());
        assert!(parse_param("p.k").is_err());
    }

    #[test]
    fn sanitizes_control_characters() {
        assert_eq!(sanitize_name("a\x1b[31mb\n.exe"), "a?[31mb?.exe");
        assert_eq!(sanitize_name("plain.txt"), "plain.txt");
    }
}
