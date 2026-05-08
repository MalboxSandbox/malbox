use crate::api::ApiClient;
use crate::api::machines::ProvisionRequest;
use crate::commands::{Command, Context};
use crate::error::{CliError, Result};
use crate::utils::format::{self, Detail, Table, display_json, styled_status};
use crate::utils::progress::Spinner;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    about = "Manage analysis machines",
    long_about = "Manage analysis machines - list, inspect, provision, and manage snapshots.\n\n\
                  Machines can be referenced by name or numeric ID.",
    after_help = "Examples:\n  \
                  malbox machine list\n  \
                  malbox machine get threatforge-test-2\n  \
                  malbox machine provision win10-dev --provisioner ansible --snapshot with-yara\n  \
                  malbox machine snapshot list win10-dev\n  \
                  malbox machine snapshot delete win10-dev with-yara"
)]
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
    #[command(subcommand)]
    Snapshot(SnapshotCommands),
    /// Run a provisioning step against a machine
    Provision(ProvisionArgs),
    /// List provision run history for a machine
    ProvisionHistory(ProvisionHistoryArgs),
}

#[derive(Parser)]
#[command(
    after_help = "Examples:\n  malbox machine get threatforge-test-2\n  malbox machine get 3"
)]
struct GetArgs {
    /// Machine name or ID (interactive selection if omitted)
    machine: Option<String>,
}

#[derive(Subcommand)]
enum SnapshotCommands {
    /// List snapshots for a machine
    #[command(after_help = "Examples:\n  malbox machine snapshot list win10-dev")]
    List(SnapshotListArgs),
    /// Delete a snapshot (and its associated provision runs)
    #[command(after_help = "Examples:\n  malbox machine snapshot delete win10-dev with-yara")]
    Delete(SnapshotDeleteArgs),
}

#[derive(Parser)]
struct SnapshotListArgs {
    /// Machine name or ID (interactive selection if omitted)
    machine: Option<String>,
}

#[derive(Parser)]
struct SnapshotDeleteArgs {
    /// Machine name or ID
    machine: Option<String>,
    /// Snapshot name (interactive selection if omitted)
    snapshot_name: Option<String>,
}

#[derive(Parser)]
struct ProvisionHistoryArgs {
    /// Machine name or ID (interactive selection if omitted)
    machine: Option<String>,
}

#[derive(Parser)]
#[command(after_help = "Examples:\n  \
                  malbox machine provision win10-dev --provisioner ansible --snapshot with-yara\n  \
                  malbox machine provision win10-dev --provisioner ansible --config '{\"playbook\": \"setup.yml\"}'\n  \
                  malbox machine provision win10-dev --provisioner native --plugins yara capa")]
struct ProvisionArgs {
    /// Machine name or ID
    machine: Option<String>,
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
            MachineCommands::Get(args) => get(ctx, args).await,
            MachineCommands::Snapshot(cmd) => match cmd {
                SnapshotCommands::List(args) => snapshot_list(ctx, args).await,
                SnapshotCommands::Delete(args) => snapshot_delete(ctx, args).await,
            },
            MachineCommands::Provision(args) => provision(ctx, args).await,
            MachineCommands::ProvisionHistory(args) => provision_history(ctx, args).await,
        }
    }
}

async fn select_machine(api: &ApiClient) -> Result<i32> {
    let machines = api.list_machines().await?;
    if machines.is_empty() {
        return Err(CliError::InvalidArgument(
            "no machines available".to_string(),
        ));
    }
    let items: Vec<String> = machines
        .iter()
        .map(|m| {
            let status = display_json(&m.status);
            let id = m.id.map(|i| i.to_string()).unwrap_or_default();
            format!("{} ({}) [{}]", m.name, id, status)
        })
        .collect();

    let selection = format::fuzzy_select("Select machine", &items).ok_or_else(|| {
        CliError::InvalidArgument("machine selection required (not a terminal?)".to_string())
    })?;

    machines[selection]
        .id
        .ok_or_else(|| CliError::InvalidArgument("selected machine has no ID".to_string()))
}

async fn resolve_machine(ctx: &Context, machine: Option<String>) -> Result<i32> {
    match machine {
        Some(ref name_or_id) => ctx.api.resolve_machine_id(name_or_id).await,
        None => select_machine(&ctx.api).await,
    }
}

async fn list(api: &ApiClient) -> Result<()> {
    let machines = api.list_machines().await?;

    if machines.is_empty() {
        format::empty_with_hint(
            "No machines found.",
            "Run 'malbox provider install' to set up a provider first.",
        );
        return Ok(());
    }

    let mut table = Table::new(&["ID", "NAME", "PLATFORM", "ARCH", "STATUS", "IP"]);

    for m in &machines {
        let status = display_json(&m.status);
        table.add_row(vec![
            m.id.map(|id| id.to_string()).unwrap_or_default(),
            m.name.clone(),
            display_json(&m.platform),
            display_json(&m.arch),
            styled_status(&status),
            m.ip.as_deref().unwrap_or("-").to_string(),
        ]);
    }

    table.print();
    format::total(machines.len(), "machine");
    Ok(())
}

async fn get(ctx: &Context, args: GetArgs) -> Result<()> {
    let id = resolve_machine(ctx, args.machine).await?;
    let machine = ctx.api.get_machine(id).await?;
    print_machine(&machine);
    Ok(())
}

async fn snapshot_list(ctx: &Context, args: SnapshotListArgs) -> Result<()> {
    let machine_id = resolve_machine(ctx, args.machine).await?;
    let snaps = ctx.api.list_snapshots(machine_id).await?;

    if snaps.is_empty() {
        format::empty("No snapshots for this machine.");
        return Ok(());
    }

    let mut table = Table::new(&["ACTIVE", "NAME", "TAGS", "CREATED"]);

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
            .map(|v| format::format_time(&display_json(v)))
            .unwrap_or_else(|| "-".to_string());

        table.add_row(vec![active, s.name.clone(), tags, created]);
    }

    table.print();
    format::total(snaps.len(), "snapshot");
    Ok(())
}

async fn snapshot_delete(ctx: &Context, args: SnapshotDeleteArgs) -> Result<()> {
    let machine_id = resolve_machine(ctx, args.machine).await?;

    let snapshot_name = match args.snapshot_name {
        Some(name) => name,
        None => {
            let snaps = ctx.api.list_snapshots(machine_id).await?;
            if snaps.is_empty() {
                return Err(CliError::InvalidArgument(
                    "no snapshots available on this machine".to_string(),
                ));
            }
            let items: Vec<String> = snaps.iter().map(|s| s.name.clone()).collect();
            let selection =
                format::fuzzy_select("Select snapshot to delete", &items).ok_or_else(|| {
                    CliError::InvalidArgument(
                        "snapshot selection required (not a terminal?)".to_string(),
                    )
                })?;
            items[selection].clone()
        }
    };

    let prompt = format!("Delete snapshot '{}'?", snapshot_name);
    if !format::confirm(&prompt, ctx.yes) {
        format::empty("Cancelled.");
        return Ok(());
    }

    let spinner = Spinner::start(format!("Deleting snapshot '{}'...", snapshot_name));
    ctx.api.delete_snapshot(machine_id, &snapshot_name).await?;
    drop(spinner);
    format::success(format!("Snapshot '{}' deleted.", snapshot_name));
    Ok(())
}

fn print_machine(m: &crate::api::machines::Machine) {
    let status = display_json(&m.status);
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
        .field_status("Status", &status)
        .field("IP", m.ip.as_deref().unwrap_or("-"))
        .field_opt("Provider", m.provider.as_deref())
        .field_opt("Provider ID", m.provider_id.as_deref())
        .field_opt("Task", m.current_task_id)
        .field_opt("Error", m.error_message.as_deref())
        .field_opt("Tags", tags_display)
        .field_opt(
            "Created",
            m.created_at
                .as_ref()
                .map(|v| format::format_time(&display_json(v))),
        )
        .field_opt(
            "Updated",
            m.updated_at
                .as_ref()
                .map(|v| format::format_time(&display_json(v))),
        );

    detail.print();
}

async fn provision(ctx: &Context, args: ProvisionArgs) -> Result<()> {
    let machine_id = resolve_machine(ctx, args.machine).await?;

    let config = args
        .config
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|e| CliError::InvalidArgument(format!("invalid --config JSON: {}", e)))?;

    let request = ProvisionRequest {
        provisioner: args.provisioner,
        config,
        plugins: args.plugins,
        snapshot: args.snapshot,
        revert_to: args.revert_to,
    };

    let spinner = Spinner::start("Running provisioning...");
    let result = ctx.api.provision_machine(machine_id, request).await?;
    drop(spinner);

    let mut detail = Detail::new();
    detail
        .field_status("Status", &result.status)
        .field_opt("Error", result.error_message.as_deref())
        .field_opt("Snapshot", result.snapshot_id.as_ref().map(display_json));

    if result.status == "completed" || result.status == "success" {
        format::success("Provisioning completed.");
    } else {
        println!(
            "Provisioning finished with status: {}",
            styled_status(&result.status)
        );
    }
    detail.print();

    Ok(())
}

async fn provision_history(ctx: &Context, args: ProvisionHistoryArgs) -> Result<()> {
    let machine_id = resolve_machine(ctx, args.machine).await?;
    let runs = ctx.api.list_provision_runs(machine_id).await?;

    if runs.is_empty() {
        format::empty("No provision runs for this machine.");
        return Ok(());
    }

    let mut table = Table::new(&["STATUS", "PROVISIONER", "SNAPSHOT", "CREATED"]);

    for r in &runs {
        let snapshot = r
            .snapshot_id
            .as_ref()
            .map(display_json)
            .unwrap_or_else(|| "-".to_string());
        let created = r
            .created_at
            .as_ref()
            .map(|v| format::format_time(&display_json(v)))
            .unwrap_or_else(|| "-".to_string());

        table.add_row(vec![
            styled_status(&r.status),
            r.provisioner.clone(),
            snapshot,
            created,
        ]);
    }

    table.print();
    format::total(runs.len(), "run");
    Ok(())
}
