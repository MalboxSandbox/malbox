use crate::api::ApiClient;
use crate::api::tasks::SubmitTaskRequest;
use crate::commands::{Command, Context};
use crate::error::Result;
use crate::utils::format::{self, Detail, Table};
use crate::utils::progress::Spinner;
use clap::{Parser, Subcommand};
use console::Style;

#[derive(Parser)]
#[command(about = "Manage analysis tasks")]
pub struct TaskCommand {
    #[command(subcommand)]
    command: TaskCommands,
}

#[derive(Subcommand)]
enum TaskCommands {
    /// List all tasks
    List,
    /// Get details and results of a specific task
    Get(GetArgs),
    /// Submit a file for analysis
    Submit(SubmitArgs),
}

#[derive(Parser)]
struct GetArgs {
    /// Task ID
    id: i32,
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
            TaskCommands::Get(args) => get(&ctx.api, args).await,
            TaskCommands::Submit(args) => submit(&ctx.api, args).await,
        }
    }
}

async fn list(api: &ApiClient) -> Result<()> {
    let tasks = api.list_tasks().await?;

    if tasks.is_empty() {
        format::empty("No tasks found.");
        return Ok(());
    }

    let mut table = Table::new(&[
        ("ID", 6),
        ("STATUS", 12),
        ("TARGET", 25),
        ("PLATFORM", 10),
        ("PRIORITY", 10),
        ("MACHINE", 10),
        ("CREATED", 20),
    ]);

    for t in &tasks {
        table.add_row(vec![
            t.id.to_string(),
            t.status.clone(),
            t.target.clone(),
            t.platform.clone(),
            t.priority.to_string(),
            t.machine_id
                .map(|m| m.to_string())
                .unwrap_or_else(|| "-".to_string()),
            t.created_on.clone(),
        ]);
    }

    table.print();
    format::total(tasks.len(), "task");
    Ok(())
}

async fn get(api: &ApiClient, args: GetArgs) -> Result<()> {
    let task = api.get_task(args.id).await?;

    let tags_display = task
        .tags
        .as_ref()
        .filter(|t| !t.is_empty())
        .map(|t| t.join(", "));

    let mut detail = Detail::new();
    detail
        .field("ID", task.id)
        .field("Status", &task.status)
        .field("Target", &task.target)
        .field("Platform", &task.platform)
        .field("Timeout", format!("{}s", task.timeout))
        .field("Priority", task.priority)
        .field_opt("Owner", task.owner.as_deref())
        .field_opt("Machine", task.machine_id)
        .field_opt("Tags", tags_display)
        .field("Created", &task.created_on)
        .field_opt("Completed", task.completed_on.as_deref());
    detail.print();

    let results = api.get_task_results(args.id).await?;
    if !results.is_empty() {
        let bold = Style::new().bold();
        println!("\n  {}", bold.apply_to("Results:"));

        let mut table = Table::new(&[
            ("ID", 6),
            ("PLUGIN", 25),
            ("RESULT", 20),
            ("FORMAT", 8),
            ("SIZE", 10),
            ("PATH", 30),
        ]);
        table.set_indent(2);

        for r in &results {
            table.add_row(vec![
                r.id.to_string(),
                r.plugin_name.clone(),
                r.result_name.clone(),
                r.format.clone(),
                r.size_bytes.to_string(),
                r.file_path.clone(),
            ]);
        }

        table.print();
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
