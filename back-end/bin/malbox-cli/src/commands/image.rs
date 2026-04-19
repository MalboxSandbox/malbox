use crate::api::ApiClient;
use crate::api::images::RegisterImageRequest;
use crate::commands::{Command, Context};
use crate::error::Result;
use crate::utils::format::{self, Detail, Table, display_json, display_value};
use crate::utils::progress::Spinner;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(about = "Manage VM images")]
pub struct ImageCommand {
    #[command(subcommand)]
    command: ImageCommands,
}

#[derive(Subcommand)]
enum ImageCommands {
    /// Register a new VM image
    Register(RegisterArgs),
    /// List all registered images
    List,
    /// Get details of a specific image
    Get(GetArgs),
    /// Delete an image
    Delete(DeleteArgs),
}

#[derive(Parser)]
struct RegisterArgs {
    /// Image name
    #[arg(long)]
    name: String,
    /// Platform (windows or linux)
    #[arg(long)]
    platform: String,
    /// Architecture (x64 or x86)
    #[arg(long)]
    arch: String,
    /// Path to the image file
    #[arg(long)]
    path: String,
    /// Image format (e.g. qcow2, vmdk)
    #[arg(long)]
    format: Option<String>,
    /// Description
    #[arg(long)]
    description: Option<String>,
}

#[derive(Parser)]
struct GetArgs {
    /// Image name
    name: String,
}

#[derive(Parser)]
struct DeleteArgs {
    /// Image name
    name: String,
}

impl Command for ImageCommand {
    async fn execute(self, ctx: &Context) -> Result<()> {
        match self.command {
            ImageCommands::Register(args) => register(&ctx.api, args).await,
            ImageCommands::List => list(&ctx.api).await,
            ImageCommands::Get(args) => get(&ctx.api, args).await,
            ImageCommands::Delete(args) => delete(&ctx.api, args).await,
        }
    }
}

async fn register(api: &ApiClient, args: RegisterArgs) -> Result<()> {
    let spinner = Spinner::start("Registering image...");
    let image = api
        .register_image(RegisterImageRequest {
            name: args.name,
            platform: args.platform,
            arch: args.arch,
            path: args.path,
            format: args.format,
            description: args.description,
        })
        .await?;
    drop(spinner);

    format::success("Image registered.");
    print_image(&image);
    Ok(())
}

async fn list(api: &ApiClient) -> Result<()> {
    let images = api.list_images().await?;

    if images.is_empty() {
        format::empty("No images found.");
        return Ok(());
    }

    let mut table = Table::new(&[
        ("ID", 38),
        ("NAME", 20),
        ("PLATFORM", 10),
        ("ARCH", 8),
        ("FORMAT", 10),
        ("AVAILABLE", 10),
    ]);

    for img in &images {
        table.add_row(vec![
            display_value(&img.id),
            img.name.clone(),
            display_value(&img.platform),
            display_value(&img.arch),
            img.format.as_deref().unwrap_or("-").to_string(),
            img.available
                .map(|a| if a { "yes" } else { "no" })
                .unwrap_or("-")
                .to_string(),
        ]);
    }

    table.print();
    format::total(images.len(), "image");
    Ok(())
}

async fn get(api: &ApiClient, args: GetArgs) -> Result<()> {
    let image = api.get_image(&args.name).await?;
    print_image(&image);
    Ok(())
}

async fn delete(api: &ApiClient, args: DeleteArgs) -> Result<()> {
    let spinner = Spinner::start(format!("Deleting image '{}'...", args.name));
    api.delete_image(&args.name).await?;
    drop(spinner);
    format::success(format!("Image '{}' deleted.", args.name));
    Ok(())
}

fn print_image(img: &crate::api::images::Image) {
    let mut detail = Detail::new();
    detail
        .field_opt("ID", img.id.as_ref().map(display_json))
        .field("Name", &img.name)
        .field_opt("Platform", img.platform.as_ref().map(display_json))
        .field_opt("Arch", img.arch.as_ref().map(display_json))
        .field_opt("Format", img.format.as_deref())
        .field_opt("Description", img.description.as_deref())
        .field_opt("Path", img.path.as_deref())
        .field_opt(
            "Available",
            img.available.map(|a| if a { "yes" } else { "no" }),
        );
    detail.print();
}
