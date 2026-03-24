use crate::api::ApiClient;
use crate::api::tasks::SubmitTaskRequest;
use crate::commands::{Command, Context};
use crate::error::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(about = "Manage analysis tasks")]
pub struct TaskCommand {
    #[command(subcommand)]
    command: TaskCommands,
}

#[derive(Subcommand)]
enum TaskCommands {
    /// Submit a file for analysis
    Submit(SubmitArgs),
    /// Show results for a completed task
    Results(ResultsArgs),
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
    /// Comma-separated tags
    #[arg(long)]
    tags: Option<String>,
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

#[derive(Parser)]
struct ResultsArgs {
    /// Task ID
    id: i32,
}

impl Command for TaskCommand {
    async fn execute(self, ctx: &Context) -> Result<()> {
        match self.command {
            TaskCommands::Submit(args) => submit(&ctx.api, args).await,
            TaskCommands::Results(args) => results(&ctx.api, args).await,
        }
    }
}

async fn submit(api: &ApiClient, args: SubmitArgs) -> Result<()> {
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

    println!("Task submitted successfully.");
    println!("  Task ID: {}", response.task_id);
    Ok(())
}

async fn results(api: &ApiClient, args: ResultsArgs) -> Result<()> {
    let results = api.get_task_results(args.id).await?;

    if results.is_empty() {
        println!("No results for task {}.", args.id);
        return Ok(());
    }

    println!(
        "{:<6} {:<25} {:<20} {:<8} {:<10} {}",
        "ID", "PLUGIN", "RESULT", "FORMAT", "SIZE", "PATH"
    );
    println!("{}", "-".repeat(90));

    for r in &results {
        println!(
            "{:<6} {:<25} {:<20} {:<8} {:<10} {}",
            r.id, r.plugin_name, r.result_name, r.format, r.size_bytes, r.file_path,
        );
    }

    println!("\nTotal: {} result(s)", results.len());
    Ok(())
}
