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

impl Command for TaskCommand {
    async fn execute(self, ctx: &Context) -> Result<()> {
        match self.command {
            TaskCommands::Submit(args) => submit(&ctx.api, args).await,
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
