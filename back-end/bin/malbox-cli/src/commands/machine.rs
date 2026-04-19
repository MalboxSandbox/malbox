use crate::api::ApiClient;
use crate::api::machines::ProvisionRequest;
use crate::commands::{Command, Context};
use crate::error::Result;
use crate::utils::format::{self, Detail, Table, display_json};
use crate::utils::progress::Spinner;
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
    /// Manage snapshots for a machine
    Snapshot(SnapshotCommand),
    /// Run a provisioning step against a machine
    Provision(ProvisionArgs),
    /// List provision run history for a machine
    ProvisionHistory(ProvisionHistoryArgs),
}

#[derive(Parser)]
struct GetArgs {
    /// Machine ID
    id: i32,
}

// -- Snapshot subcommand group --

#[derive(Parser)]
#[command(about = "Manage machine snapshots")]
struct SnapshotCommand {
    #[command(subcommand)]
    command: SnapshotCommands,
}

#[derive(Subcommand)]
enum SnapshotCommands {
    /// List snapshots for a machine
    List(SnapshotListArgs),
    /// Delete a snapshot (and its associated provision runs)
    Delete(SnapshotDeleteArgs),
}

#[derive(Parser)]
struct SnapshotListArgs {
    /// Machine ID
    machine_id: i32,
}

#[derive(Parser)]
struct SnapshotDeleteArgs {
    /// Machine ID
    machine_id: i32,
    /// Snapshot name (e.g., "with-yara")
    snapshot_name: String,
}

// -- Provision args --

#[derive(Parser)]
struct ProvisionHistoryArgs {
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
            MachineCommands::Snapshot(cmd) => match cmd.command {
                SnapshotCommands::List(args) => snapshot_list(&ctx.api, args).await,
                SnapshotCommands::Delete(args) => snapshot_delete(&ctx.api, args).await,
            },
            MachineCommands::Provision(args) => provision(&ctx.api, args).await,
            MachineCommands::ProvisionHistory(args) => provision_history(&ctx.api, args).await,
        }
    }
}

async fn list(api: &ApiClient) -> Result<()> {
    let machines = api.list_machines().await?;

    if machines.is_empty() {
        format::empty("No machines found.");
        return Ok(());
    }

    let mut table = Table::new(&[
        ("ID", 6),
        ("NAME", 20),
        ("PLATFORM", 10),
        ("ARCH", 8),
        ("STATUS", 15),
        ("IP", 15),
    ]);

    for m in &machines {
        table.add_row(vec![
            m.id.map(|id| id.to_string()).unwrap_or_default(),
            m.name.clone(),
            display_json(&m.platform),
            display_json(&m.arch),
            display_json(&m.status),
            m.ip.as_deref().unwrap_or("-").to_string(),
        ]);
    }

    table.print();
    format::total(machines.len(), "machine");
    Ok(())
}

async fn get(api: &ApiClient, args: GetArgs) -> Result<()> {
    let machine = api.get_machine(args.id).await?;
    print_machine(&machine);
    Ok(())
}

async fn snapshot_list(api: &ApiClient, args: SnapshotListArgs) -> Result<()> {
    let snaps = api.list_snapshots(args.machine_id).await?;

    if snaps.is_empty() {
        format::empty(format!("No snapshots for machine {}.", args.machine_id));
        return Ok(());
    }

    let mut table = Table::new(&[("ACTIVE", 8), ("NAME", 20), ("TAGS", 8), ("CREATED", 20)]);

    for s in &snaps {
        let active = if s.is_active {
            "\u{25cf}".to_string()
        } else {
            String::new()
        };
        let tags = s
            .tags
            .as_ref()
            .map(|t| t.join(", "))
            .unwrap_or_else(|| "-".to_string());
        let created = s
            .created_at
            .as_ref()
            .map(display_json)
            .unwrap_or_else(|| "-".to_string());

        table.add_row(vec![active, s.name.clone(), tags, created]);
    }

    table.print();
    format::total(snaps.len(), "snapshot");
    Ok(())
}

async fn snapshot_delete(api: &ApiClient, args: SnapshotDeleteArgs) -> Result<()> {
    let spinner = Spinner::start(format!("Deleting snapshot '{}'...", args.snapshot_name));
    api.delete_snapshot(args.machine_id, &args.snapshot_name)
        .await?;
    drop(spinner);
    format::success(format!("Snapshot '{}' deleted.", args.snapshot_name));
    Ok(())
}

fn print_machine(m: &crate::api::machines::Machine) {
    let tags_display = m
        .tags
        .as_ref()
        .filter(|t| !t.is_empty())
        .map(|t| t.join(", "));

    let mut detail = Detail::new();
    detail
        .field("ID", m.id.map(|id| id.to_string()).unwrap_or_default())
        .field("Name", &m.name)
        .field("Platform", display_json(&m.platform))
        .field("Arch", display_json(&m.arch))
        .field("Status", display_json(&m.status))
        .field("IP", m.ip.as_deref().unwrap_or("-"))
        .field_opt("Provider", m.provider.as_deref())
        .field_opt("Provider ID", m.provider_id.as_deref())
        .field_opt("Task", m.current_task_id)
        .field_opt("Error", m.error_message.as_deref())
        .field_opt("Tags", tags_display)
        .field_opt("Created", m.created_at.as_ref().map(display_json))
        .field_opt("Updated", m.updated_at.as_ref().map(display_json));
    detail.print();
}

async fn provision(api: &ApiClient, args: ProvisionArgs) -> Result<()> {
    let config = args
        .config
        .as_deref()
        .map(serde_json::from_str)
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

    let spinner = Spinner::start(format!("Running provisioning on machine {}...", args.id));
    let result = api.provision_machine(args.id, request).await?;
    drop(spinner);

    let mut detail = Detail::new();
    detail
        .field("Status", &result.status)
        .field_opt("Error", result.error_message.as_deref())
        .field_opt("Snapshot", result.snapshot_id.as_ref().map(display_json));

    if result.status == "completed" || result.status == "success" {
        format::success(format!("Provisioning completed on machine {}.", args.id));
    } else {
        println!("Provisioning finished on machine {}.", args.id);
    }
    detail.print();

    Ok(())
}

async fn provision_history(api: &ApiClient, args: ProvisionHistoryArgs) -> Result<()> {
    let runs = api.list_provision_runs(args.id).await?;

    if runs.is_empty() {
        format::empty(format!("No provision runs for machine {}.", args.id));
        return Ok(());
    }

    let mut table = Table::new(&[
        ("STATUS", 10),
        ("PROVISIONER", 15),
        ("SNAPSHOT", 10),
        ("CREATED", 20),
    ]);

    for r in &runs {
        let snapshot = r
            .snapshot_id
            .as_ref()
            .map(display_json)
            .unwrap_or_else(|| "-".to_string());
        let created = r
            .created_at
            .as_ref()
            .map(display_json)
            .unwrap_or_else(|| "-".to_string());

        table.add_row(vec![
            r.status.clone(),
            r.provisioner.clone(),
            snapshot,
            created,
        ]);
    }

    table.print();
    format::total(runs.len(), "run");
    Ok(())
}
