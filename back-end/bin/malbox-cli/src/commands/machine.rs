use crate::api::ApiClient;
use crate::api::machines::ProvisionRequest;
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
    /// List all machines
    List,
    /// Get details of a specific machine
    Get(GetArgs),
    /// List snapshots for a machine
    Snapshots(SnapshotsArgs),
    /// Run a provisioning step against a machine
    Provision(ProvisionArgs),
    /// List provision run history for a machine
    Provisions(ProvisionsArgs),
}

#[derive(Parser)]
struct GetArgs {
    /// Machine ID
    id: i32,
}

#[derive(Parser)]
struct SnapshotsArgs {
    /// Machine ID
    id: i32,
}

#[derive(Parser)]
struct ProvisionsArgs {
    /// Machine ID
    id: i32,
}

#[derive(Parser)]
struct ProvisionArgs {
    /// Machine ID
    id: i32,
    /// Provisioner type (e.g., "ansible", "native")
    #[arg(long)]
    provisioner: String,
    /// Snapshot name to create after provisioning
    #[arg(long)]
    snapshot: Option<String>,
    /// Snapshot to revert to before provisioning (defaults to "base")
    #[arg(long)]
    revert_to: Option<String>,
    /// Provisioner config as JSON (e.g., '{"playbook": "setup.yml"}')
    #[arg(long)]
    config: Option<String>,
    /// Guest plugin names to deploy (resolved from daemon's plugin registry)
    #[arg(long, num_args = 1..)]
    plugins: Option<Vec<String>>,
}

impl Command for MachineCommand {
    async fn execute(self, ctx: &Context) -> Result<()> {
        match self.command {
            MachineCommands::List => list(&ctx.api).await,
            MachineCommands::Get(args) => get(&ctx.api, args).await,
            MachineCommands::Snapshots(args) => snapshots(&ctx.api, args).await,
            MachineCommands::Provision(args) => provision(&ctx.api, args).await,
            MachineCommands::Provisions(args) => provisions(&ctx.api, args).await,
        }
    }
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

async fn snapshots(api: &ApiClient, args: SnapshotsArgs) -> Result<()> {
    let snaps = api.list_snapshots(args.id).await?;

    if snaps.is_empty() {
        println!("No snapshots for machine {}.", args.id);
        return Ok(());
    }

    println!(
        "{:<8} {:<20} {:<8} {:<20}",
        "ACTIVE", "NAME", "TAGS", "CREATED"
    );
    println!("{}", "-".repeat(56));

    for s in &snaps {
        let active = if s.is_active { "  ●" } else { "" };
        let tags = s
            .tags
            .as_ref()
            .map(|t| t.join(", "))
            .unwrap_or_else(|| "-".to_string());
        let created = s
            .created_at
            .as_ref()
            .map(|v| display_json(v))
            .unwrap_or_else(|| "-".to_string());

        println!("{:<8} {:<20} {:<8} {:<20}", active, s.name, tags, created);
    }

    println!("\nTotal: {} snapshot(s)", snaps.len());
    Ok(())
}

fn print_machine(m: &crate::api::machines::Machine) {
    println!(
        "  ID:          {}",
        m.id.map(|id| id.to_string()).unwrap_or_default()
    );
    println!("  Name:        {}", m.name);
    println!("  Platform:    {}", display_json(&m.platform));
    println!("  Arch:        {}", display_json(&m.arch));
    println!("  Status:      {}", display_json(&m.status));
    println!("  IP:          {}", m.ip.as_deref().unwrap_or("-"));
    if let Some(ref provider) = m.provider {
        println!("  Provider:    {}", provider);
    }
    if let Some(ref provider_id) = m.provider_id {
        println!("  Provider ID: {}", provider_id);
    }
    if let Some(task_id) = m.current_task_id {
        println!("  Task:        {}", task_id);
    }
    if let Some(ref err) = m.error_message {
        println!("  Error:       {}", err);
    }
    if let Some(ref tags) = m.tags {
        if !tags.is_empty() {
            println!("  Tags:        {}", tags.join(", "));
        }
    }
    if let Some(ref ts) = m.created_at {
        println!("  Created:     {}", display_json(ts));
    }
    if let Some(ref ts) = m.updated_at {
        println!("  Updated:     {}", display_json(ts));
    }
}

async fn provision(api: &ApiClient, args: ProvisionArgs) -> Result<()> {
    let config = args
        .config
        .as_deref()
        .map(|s| serde_json::from_str(s))
        .transpose()
        .map_err(|e| {
            crate::error::CliError::InvalidArgument(format!("invalid --config JSON: {}", e))
        })?;

    let request = ProvisionRequest {
        provisioner: args.provisioner,
        config,
        plugins: args.plugins,
        snapshot: args.snapshot,
        revert_to: args.revert_to,
    };

    println!("Running provisioning step on machine {}...", args.id);

    let result = api.provision_machine(args.id, request).await?;
    println!("Status: {}", result.status);
    if let Some(ref err) = result.error_message {
        println!("Error: {}", err);
    }
    if let Some(ref snap_id) = result.snapshot_id {
        println!("Snapshot: {}", display_json(snap_id));
    }

    Ok(())
}

async fn provisions(api: &ApiClient, args: ProvisionsArgs) -> Result<()> {
    let runs = api.list_provision_runs(args.id).await?;

    if runs.is_empty() {
        println!("No provision runs for machine {}.", args.id);
        return Ok(());
    }

    println!(
        "{:<10} {:<15} {:<10} {:<20}",
        "STATUS", "PROVISIONER", "SNAPSHOT", "CREATED"
    );
    println!("{}", "-".repeat(55));

    for r in &runs {
        let snapshot = r
            .snapshot_id
            .as_ref()
            .map(|v| display_json(v))
            .unwrap_or_else(|| "-".to_string());
        let created = r
            .created_at
            .as_ref()
            .map(|v| display_json(v))
            .unwrap_or_else(|| "-".to_string());

        println!(
            "{:<10} {:<15} {:<10} {:<20}",
            r.status, r.provisioner, snapshot, created
        );
    }

    println!("\nTotal: {} run(s)", runs.len());
    Ok(())
}

fn display_json(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}
