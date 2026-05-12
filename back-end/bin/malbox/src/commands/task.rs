use crate::commands::Command;
use clap::{Parser, Subcommand};
use malbox_cli_common::api::ApiClient;
use malbox_cli_common::api::tasks::SubmitTaskRequest;
use malbox_cli_common::context::Context;
use malbox_cli_common::error::{CliError, Result};
use malbox_cli_common::utils::format::{self, Detail, Table, styled_status};
use malbox_cli_common::utils::progress::Spinner;
use std::io::{IsTerminal, Write};

#[derive(Parser)]
#[command(
    about = "Manage analysis tasks",
    long_about = "Submit samples for analysis, inspect task status, and retrieve results.",
    after_help = "Examples:\n  \
                  malbox task list\n  \
                  malbox task get 7\n  \
                  malbox task get 7 --result 2\n  \
                  malbox task submit sample.exe --package default --timeout 120"
)]
pub struct TaskCommand {
    #[command(subcommand)]
    command: TaskCommands,
}

#[derive(Subcommand)]
enum TaskCommands {
    /// List all tasks
    List,
    /// Get details and results of a specific task
    #[command(after_help = "Examples:\n  \
                            malbox task get 7\n  \
                            malbox task get 7 --result 2\n  \
                            malbox task get 7 --result 2 > output.json")]
    Get(GetArgs),
    /// Submit a file for analysis
    #[command(after_help = "Examples:\n  \
                            malbox task submit malware.exe\n  \
                            malbox task submit sample.dll --package dll --timeout 300\n  \
                            malbox task submit doc.pdf --tags phishing campaign-2024 --priority 5")]
    Submit(SubmitArgs),
}

#[derive(Parser)]
struct GetArgs {
    /// Task ID (interactive selection if omitted)
    id: Option<i32>,
    /// Fetch and print the content of a specific result by its ID.
    /// JSON results are pretty-printed to a terminal (raw when piped);
    /// binary results are written raw when piped and refused on a TTY.
    #[arg(long)]
    result: Option<i32>,
}

#[derive(Parser)]
struct SubmitArgs {
    /// Path to the file to analyze
    file: String,
    /// Analysis package to use
    #[arg(long)]
    package: Option<String>,
    /// Analysis module to use
    #[arg(long)]
    module: Option<String>,
    /// Analysis timeout in seconds
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
    /// Enable memory dump
    #[arg(long)]
    memory: bool,
    /// Only analyze if sample is unique
    #[arg(long)]
    unique: bool,
    /// Enforce timeout strictly
    #[arg(long)]
    enforce_timeout: bool,
}

impl Command for TaskCommand {
    async fn execute(self, ctx: &Context) -> Result<()> {
        match self.command {
            TaskCommands::List => list(&ctx.api).await,
            TaskCommands::Get(args) => get(ctx, args).await,
            TaskCommands::Submit(args) => submit(&ctx.api, args).await,
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
        "ID", "STATUS", "TARGET", "PLATFORM", "PRIORITY", "MACHINE", "CREATED",
    ]);

    for t in &tasks {
        table.add_row(vec![
            t.id.to_string(),
            styled_status(&t.status),
            t.target.clone(),
            t.platform.clone(),
            t.priority.to_string(),
            t.machine_id
                .map(|m| m.to_string())
                .unwrap_or_else(|| "-".to_string()),
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

    if let Some(result_id) = args.result {
        return print_result_content(&ctx.api, task_id, result_id).await;
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
        .field("Created", format::format_time(&task.created_on))
        .field_opt(
            "Completed",
            task.completed_on.as_deref().map(format::format_time),
        );
    detail.print();

    let results = ctx.api.get_task_results(task_id).await?;
    format::section_header(&format!("Results ({})", results.len()));

    if !results.is_empty() {
        let mut table = Table::new(&["ID", "PLUGIN", "RESULT", "SIZE"]);
        table.set_indent(2);

        for r in &results {
            table.add_row(vec![
                r.id.to_string(),
                r.plugin_name.clone(),
                r.result_name.clone(),
                format::bytes(r.size_bytes),
            ]);
        }

        table.print();
    }

    Ok(())
}

async fn print_result_content(api: &ApiClient, task_id: i32, result_id: i32) -> Result<()> {
    let results = api.get_task_results(task_id).await?;
    let result = results.iter().find(|r| r.id == result_id).ok_or_else(|| {
        CliError::InvalidArgument(format!(
            "result {} not found for task {}",
            result_id, task_id
        ))
    })?;

    let content = api.get_task_result_content(task_id, result_id).await?;
    let stdout = std::io::stdout();
    let is_tty = stdout.is_terminal();

    match result.format.as_str() {
        "json" if is_tty => match serde_json::from_slice::<serde_json::Value>(&content) {
            Ok(v) => {
                let pretty = serde_json::to_string_pretty(&v)?;
                println!("{}", pretty);
            }
            Err(e) => {
                eprintln!("warning: result labeled json but failed to parse ({e}); writing raw");
                stdout.lock().write_all(&content)?;
            }
        },
        "json" => {
            stdout.lock().write_all(&content)?;
        }
        _ => {
            if is_tty {
                eprintln!(
                    "Result {} ({}/{}): {} of binary data.",
                    result.id,
                    result.plugin_name,
                    result.result_name,
                    format::bytes(result.size_bytes),
                );
                eprintln!("Redirect to a file to save it, e.g.:");
                eprintln!(
                    "  malbox task get {} --result {} > {}.bin",
                    task_id, result_id, result.result_name,
                );
            } else {
                stdout.lock().write_all(&content)?;
            }
        }
    }

    Ok(())
}

async fn submit(api: &ApiClient, args: SubmitArgs) -> Result<()> {
    let file_display = args.file.clone();
    let spinner = Spinner::start(format!("Submitting '{}'...", file_display));

    let response = api
        .submit_task(SubmitTaskRequest {
            file_path: args.file,
            package: args.package,
            module: args.module,
            timeout: args.timeout,
            priority: args.priority,
            tags: args.tags,
            owner: args.owner,
            memory: args.memory,
            unique: args.unique,
            enforce_timeout: args.enforce_timeout,
        })
        .await?;

    drop(spinner);
    format::success(format!("Task submitted (ID: {})", response.task_id));
    Ok(())
}
