extern crate malbox_plugin_sdk as malbox;

use malbox::prelude::*;
use sha2::{Digest, Sha256};

#[malbox::host_plugin]
struct FileInfoPlugin;

#[derive(Serialize)]
struct FileInfo {
    hash: String,
    size: usize,
}

#[malbox::handlers]
impl FileInfoPlugin {
    #[malbox::on_start]
    fn init(&self) -> Result<()> {
        info!("FileInfo plugin ready");
        Ok(())
    }

    #[malbox::on_task]
    fn process(&self, ctx: &Context) -> Result<()> {
        let sample = ctx.task().sample_bytes()?;

        ctx.progress(0.5, "computing hash")?;

        let hash = hex_sha256(&sample);
        let size = sample.len();

        info!(task_id = ctx.task().id(), %hash, size, "Computed file info");

        ctx.results()
            .push(PluginResult::json("file_info", &FileInfo { hash, size })?)?;
        Ok(())
    }
}

fn hex_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}
