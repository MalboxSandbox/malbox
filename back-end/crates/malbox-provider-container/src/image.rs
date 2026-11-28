//! Loading `docker save` / OCI image archives into the engine.

use std::time::UNIX_EPOCH;

use bollard::query_parameters::{ImportImageOptions, TagImageOptions};
use futures::TryStreamExt;
use sha2::{Digest, Sha256};
use tracing::info;

use crate::error::tolerate;
use crate::{BASE_REPO, ContainerError, ContainerProvider};

impl ContainerProvider {
    /// Ensure the archive at `path` is loaded and return its image reference.
    ///
    /// The reference tag is derived from the archive's canonical path, size
    /// and mtime, so an unchanged archive is loaded once and a replaced one
    /// is loaded again.
    pub(crate) async fn ensure_image(&self, path: &str) -> Result<String, ContainerError> {
        let _guard = self.image_lock.lock().await;
        let docker = self.docker().await?;

        let metadata = tokio::fs::metadata(path)
            .await
            .map_err(|_| ContainerError::Image(format!("image archive not found: {path}")))?;
        let canonical = tokio::fs::canonicalize(path).await?;
        let mtime = metadata
            .modified()?
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();

        let mut hasher = Sha256::new();
        hasher.update(canonical.as_os_str().as_encoded_bytes());
        hasher.update(metadata.len().to_le_bytes());
        hasher.update(mtime.as_secs().to_le_bytes());
        hasher.update(mtime.subsec_nanos().to_le_bytes());
        let tag = hex::encode(hasher.finalize())[..16].to_string();
        let reference = format!("{BASE_REPO}:{tag}");

        if tolerate(docker.inspect_image(&reference).await, &[404])?.is_some() {
            return Ok(reference);
        }

        info!(archive = %path, %reference, "Loading image archive");
        let file = tokio::fs::File::open(path).await?;
        let body = bollard::body_try_stream(tokio_util::io::ReaderStream::new(file));
        let mut events = docker.import_image(ImportImageOptions::default(), body, None);

        let mut lines = Vec::new();
        while let Some(event) = events.try_next().await? {
            if let Some(detail) = event.error_detail {
                return Err(ContainerError::Image(
                    detail
                        .message
                        .unwrap_or_else(|| "image load failed".to_string()),
                ));
            }
            if let Some(line) = event.stream {
                lines.push(line);
            }
        }

        let loaded = parse_loaded(&lines)?;
        docker
            .tag_image(
                &loaded,
                Some(TagImageOptions {
                    repo: Some(BASE_REPO.to_string()),
                    tag: Some(tag),
                }),
            )
            .await?;
        info!(%loaded, %reference, "Image archive loaded");
        Ok(reference)
    }
}

/// Extract the single loaded image from `docker load` output lines.
pub(crate) fn parse_loaded(lines: &[String]) -> Result<String, ContainerError> {
    let loaded: Vec<&str> = lines
        .iter()
        .map(|line| line.trim())
        .filter_map(|line| {
            line.strip_prefix("Loaded image: ")
                .or_else(|| line.strip_prefix("Loaded image ID: "))
        })
        .map(str::trim)
        .collect();

    match loaded.as_slice() {
        [image] => Ok((*image).to_string()),
        _ => Err(ContainerError::Image(
            "archive must contain exactly one image".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(items: &[&str]) -> Vec<String> {
        items.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn parses_tagged_image() {
        assert_eq!(
            parse_loaded(&lines(&["Loaded image: alpine:3.20\n"])).unwrap(),
            "alpine:3.20"
        );
    }

    #[test]
    fn parses_image_id() {
        assert_eq!(
            parse_loaded(&lines(&["progress", "Loaded image ID: sha256:ab\n"])).unwrap(),
            "sha256:ab"
        );
    }

    #[test]
    fn rejects_zero_or_multiple_images() {
        assert!(parse_loaded(&lines(&["nothing here"])).is_err());
        assert!(parse_loaded(&lines(&["Loaded image: a:1\n", "Loaded image: b:2\n"])).is_err());
    }
}
