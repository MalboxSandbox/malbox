use crate::api::ApiClient;
use crate::api::machines::CreateMachineRequest;
use crate::commands::{Command, Context};
use crate::error::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(about = "Manage analysis machines")]
pub struct MachineCommand {
    #[command(subcommand)]
    command: MachineCommands,
}

#[derive(Subcommand)]
enum MachineCommands {
    /// Create a new analysis machine
    Create(CreateArgs),
    /// List all machines
    List,
    /// Get details of a specific machine
    Get(GetArgs),
    /// Delete a machine
    Delete(DeleteArgs),
    /// Retry a failed machine
    Retry(RetryArgs),
}

#[derive(Parser)]
struct CreateArgs {
    /// Machine name
    #[arg(long)]
    name: String,
    /// Image name to use
    #[arg(long)]
    image: String,
    /// Platform (windows or linux)
    #[arg(long)]
    platform: String,
    /// Architecture (x64 or x86)
    #[arg(long, default_value = "x64")]
    arch: String,
    /// Number of CPUs
    #[arg(long)]
    cpus: Option<u32>,
    /// Memory in MB
    #[arg(long)]
    memory_mb: Option<u64>,
}

#[derive(Parser)]
struct GetArgs {
    /// Machine ID
    id: i32,
}

#[derive(Parser)]
struct DeleteArgs {
    /// Machine ID
    id: i32,
}

#[derive(Parser)]
struct RetryArgs {
    /// Machine ID
    id: i32,
}

impl Command for MachineCommand {
    async fn execute(self, ctx: &Context) -> Result<()> {
        match self.command {
            MachineCommands::Create(args) => create(&ctx.api, args).await,
            MachineCommands::List => list(&ctx.api).await,
            MachineCommands::Get(args) => get(&ctx.api, args).await,
            MachineCommands::Delete(args) => delete(&ctx.api, args).await,
            MachineCommands::Retry(args) => retry(&ctx.api, args).await,
        }
    }
}

async fn create(api: &ApiClient, args: CreateArgs) -> Result<()> {
    let machine = api
        .create_machine(CreateMachineRequest {
            name: args.name,
            image: args.image,
            platform: args.platform,
            arch: args.arch,
            cpus: args.cpus,
            memory_mb: args.memory_mb,
        })
        .await?;

    println!("Machine created successfully:");
    print_machine(&machine);
    Ok(())
}

async fn list(api: &ApiClient) -> Result<()> {
    let machines = api.list_machines().await?;

    if machines.is_empty() {
        println!("No machines found.");
        return Ok(());
    }

    println!(
        "{:<6} {:<20} {:<10} {:<8} {:<15} {:<15}",
        "ID", "NAME", "PLATFORM", "ARCH", "STATUS", "IP"
    );
    println!("{}", "-".repeat(74));

    for m in &machines {
        println!(
            "{:<6} {:<20} {:<10} {:<8} {:<15} {:<15}",
            m.id.map(|id| id.to_string()).unwrap_or_default(),
            m.name,
            display_json(&m.platform),
            display_json(&m.arch),
            display_json(&m.status),
            m.ip.as_deref().unwrap_or("-"),
        );
    }

    println!("\nTotal: {} machine(s)", machines.len());
    Ok(())
}

async fn get(api: &ApiClient, args: GetArgs) -> Result<()> {
    let machine = api.get_machine(args.id).await?;
    print_machine(&machine);
    Ok(())
}

async fn delete(api: &ApiClient, args: DeleteArgs) -> Result<()> {
    api.delete_machine(args.id).await?;
    println!("Machine {} deleted.", args.id);
    Ok(())
}

async fn retry(api: &ApiClient, args: RetryArgs) -> Result<()> {
    let machine = api.retry_machine(args.id).await?;
    println!("Machine retry initiated:");
    print_machine(&machine);
    Ok(())
}

fn print_machine(m: &crate::api::machines::Machine) {
    println!(
        "  ID:        {}",
        m.id.map(|id| id.to_string()).unwrap_or_default()
    );
    println!("  Name:      {}", m.name);
    println!("  Platform:  {}", display_json(&m.platform));
    println!("  Arch:      {}", display_json(&m.arch));
    println!("  Status:    {}", display_json(&m.status));
    println!("  IP:        {}", m.ip.as_deref().unwrap_or("-"));
    if let Some(ref provider) = m.provider {
        println!("  Provider:  {}", provider);
    }
    if let Some(task_id) = m.current_task_id {
        println!("  Task:      {}", task_id);
    }
    if let Some(ref err) = m.error_message {
        println!("  Error:     {}", err);
    }
    if let Some(ref tags) = m.tags {
        if !tags.is_empty() {
            println!("  Tags:      {}", tags.join(", "));
        }
    }
}

fn display_json(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}
