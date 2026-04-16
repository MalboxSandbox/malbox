extern crate malbox_plugin_sdk as malbox;

use malbox::prelude::*;
use sha2::{Digest, Sha256};

#[malbox::host_plugin]
#[malbox(state = "persistent", execution = "parallel")]
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
        info!("FileInfo plugin ready — waiting for tasks");
        Ok(())
    }

    #[malbox::on_task]
    fn process(&self, task: Task, ctx: &Context) -> Result<()> {
        let sample = task.sample_bytes()?;

        ctx.emit_progress(0.5, "computing hash")?;

        let hash = hex_sha256(&sample);
        let size = sample.len();

        info!(task_id = task.id(), %hash, size, "Computed file info");

        ctx.push_result(PluginResult::json(
            "file_info",
            &FileInfo { hash, size },
        )?)?;
        Ok(())
    }
}

fn hex_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}
