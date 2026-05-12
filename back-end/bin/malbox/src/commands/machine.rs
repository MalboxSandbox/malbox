use crate::commands::Command;
use clap::{Parser, Subcommand};
use malbox_cli_common::api::ApiClient;
use malbox_cli_common::context::Context;
use malbox_cli_common::error::{CliError, Result};
use malbox_cli_common::utils::format::{self, Detail, Table, display_json, styled_status};

#[derive(Parser)]
#[command(
    about = "View analysis machines",
    long_about = "View analysis machines - list and inspect machine status.\n\n\
                  Machines can be referenced by name or numeric ID.",
    after_help = "Examples:\n  \
                  malbox machine list\n  \
                  malbox machine get threatforge-test-2"
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
}

#[derive(Parser)]
#[command(
    after_help = "Examples:\n  malbox machine get threatforge-test-2\n  malbox machine get 3"
)]
struct GetArgs {
    /// Machine name or ID (interactive selection if omitted)
    machine: Option<String>,
}

impl Command for MachineCommand {
    async fn execute(self, ctx: &Context) -> Result<()> {
        match self.command {
            MachineCommands::List => list(&ctx.api).await,
            MachineCommands::Get(args) => get(ctx, args).await,
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
            "Ask an administrator to set up machines with 'malboxctl'.",
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

fn print_machine(m: &malbox_cli_common::api::machines::Machine) {
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
