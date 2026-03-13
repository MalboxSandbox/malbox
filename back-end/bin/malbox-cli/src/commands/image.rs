use crate::api::ApiClient;
use crate::api::images::RegisterImageRequest;
use crate::commands::{Command, Context};
use crate::error::Result;
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

    println!("Image registered successfully:");
    print_image(&image);
    Ok(())
}

async fn list(api: &ApiClient) -> Result<()> {
    let images = api.list_images().await?;

    if images.is_empty() {
        println!("No images found.");
        return Ok(());
    }

    println!(
        "{:<38} {:<20} {:<10} {:<8} {:<10} {:<10}",
        "ID", "NAME", "PLATFORM", "ARCH", "FORMAT", "AVAILABLE"
    );
    println!("{}", "-".repeat(96));

    for img in &images {
        println!(
            "{:<38} {:<20} {:<10} {:<8} {:<10} {:<10}",
            display_value(&img.id),
            img.name,
            display_value(&img.platform),
            display_value(&img.arch),
            img.format.as_deref().unwrap_or("-"),
            img.available
                .map(|a| if a { "yes" } else { "no" })
                .unwrap_or("-"),
        );
    }

    println!("\nTotal: {} image(s)", images.len());
    Ok(())
}

async fn get(api: &ApiClient, args: GetArgs) -> Result<()> {
    let image = api.get_image(&args.name).await?;
    print_image(&image);
    Ok(())
}

async fn delete(api: &ApiClient, args: DeleteArgs) -> Result<()> {
    api.delete_image(&args.name).await?;
    println!("Image '{}' deleted.", args.name);
    Ok(())
}

fn print_image(img: &crate::api::images::Image) {
    if let Some(ref id) = img.id {
        println!("  ID:          {}", display_json(id));
    }
    println!("  Name:        {}", img.name);
    if let Some(ref platform) = img.platform {
        println!("  Platform:    {}", display_json(platform));
    }
    if let Some(ref arch) = img.arch {
        println!("  Arch:        {}", display_json(arch));
    }
    if let Some(ref format) = img.format {
        println!("  Format:      {}", format);
    }
    if let Some(ref desc) = img.description {
        println!("  Description: {}", desc);
    }
    if let Some(ref path) = img.path {
        println!("  Path:        {}", path);
    }
    if let Some(available) = img.available {
        println!("  Available:   {}", if available { "yes" } else { "no" });
    }
}

fn display_value(val: &Option<serde_json::Value>) -> String {
    match val {
        Some(v) => display_json(v),
        None => "-".to_string(),
    }
}

fn display_json(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}
