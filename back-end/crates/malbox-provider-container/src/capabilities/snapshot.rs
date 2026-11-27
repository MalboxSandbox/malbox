use std::collections::HashMap;
use std::error::Error;

use async_trait::async_trait;
use bollard::models::{ContainerConfig, ImageSummary};
use bollard::query_parameters::{
    CommitContainerOptions, CreateContainerOptions, ListImagesOptions, RemoveContainerOptions,
    RemoveImageOptions,
};
use malbox_machinery::{Machine, MachineId, Snapshot, SnapshotId, SnapshotInfo};
use tracing::info;

use crate::error::{status, tolerate};
use crate::spec::{create_body, read_spec};
use crate::{
    ContainerError, ContainerProvider, LABEL_MACHINE, LABEL_SNAPSHOT_NAME, SNAPSHOT_REPO,
    container_name,
};

impl ContainerProvider {
    /// Snapshot images of a machine, paired with their short
    /// `malbox-snapshot:<uuid>` tag (the form `create_snapshot` returns).
    pub(crate) async fn snapshot_images(
        &self,
        id: &MachineId,
    ) -> Result<Vec<(String, ImageSummary)>, ContainerError> {
        let docker = self.docker().await?;
        let images = docker
            .list_images(Some(ListImagesOptions {
                filters: Some(HashMap::from([(
                    "label".to_string(),
                    vec![format!("{LABEL_MACHINE}={id}")],
                )])),
                ..Default::default()
            }))
            .await?;

        Ok(images
            .into_iter()
            .filter_map(|image| {
                let tag = image.repo_tags.iter().find_map(|tag| snapshot_tag(tag))?;
                Some((tag.to_string(), image))
            })
            .collect())
    }

    async fn commit(&self, machine: &Machine, name: &str) -> Result<SnapshotId, ContainerError> {
        let docker = self.docker().await?;
        let cname = container_name(machine.id());

        // Carry the container labels into the image explicitly: engines differ
        // in whether a commit with a config body inherits them.
        let info = docker.inspect_container(&cname, None).await?;
        let mut labels = info.config.and_then(|c| c.labels).unwrap_or_default();
        labels.insert(LABEL_SNAPSHOT_NAME.to_string(), name.to_string());

        let tag = uuid::Uuid::new_v4().simple().to_string();
        docker
            .commit_container(
                CommitContainerOptions {
                    container: Some(cname.clone()),
                    repo: Some(SNAPSHOT_REPO.to_string()),
                    tag: Some(tag.clone()),
                    pause: false,
                    changes: Some(format!(
                        "LABEL {LABEL_SNAPSHOT_NAME}={}",
                        serde_json::to_string(name)?
                    )),
                    ..Default::default()
                },
                ContainerConfig {
                    labels: Some(labels),
                    ..Default::default()
                },
            )
            .await?;

        let id = SnapshotId(format!("{SNAPSHOT_REPO}:{tag}"));
        info!(container = %cname, snapshot = name, image = %id, "Snapshot committed");
        Ok(id)
    }

    async fn restore(
        &self,
        machine: &Machine,
        snapshot: &SnapshotId,
    ) -> Result<(), ContainerError> {
        let docker = self.docker().await?;
        if tolerate(docker.inspect_image(&snapshot.0).await, &[404])?.is_none() {
            return Err(ContainerError::NotFound(format!(
                "snapshot '{}' not found",
                snapshot.0
            )));
        }

        let cname = container_name(machine.id());
        let info = docker.inspect_container(&cname, None).await?;
        let labels = info.config.and_then(|c| c.labels).unwrap_or_default();
        let spec = read_spec(&labels)?;
        let body = create_body(&spec, &snapshot.0, &self.config.network)?;

        // Hold the allocation lock so no new machine takes the IP while the
        // container is briefly absent.
        let _guard = self.alloc_lock.lock().await;
        docker
            .remove_container(
                &cname,
                Some(RemoveContainerOptions {
                    force: true,
                    v: true,
                    ..Default::default()
                }),
            )
            .await?;
        docker
            .create_container(
                Some(CreateContainerOptions {
                    name: Some(cname.clone()),
                    ..Default::default()
                }),
                body,
            )
            .await?;
        info!(container = %cname, snapshot = %snapshot, "Container recreated from snapshot");
        Ok(())
    }

    async fn remove_snapshot(&self, snapshot: &SnapshotId) -> Result<(), ContainerError> {
        let docker = self.docker().await?;
        // Never force: Podman's forced removal also deletes containers using
        // the image, i.e. the machine itself. Without force, an image with
        // newer child snapshots is only untagged, and the image the machine
        // currently runs is refused with 409.
        let result = docker
            .remove_image(&snapshot.0, Some(RemoveImageOptions::default()), None)
            .await;
        match result {
            Ok(_) => Ok(()),
            Err(e) if status(&e) == Some(404) => Err(ContainerError::NotFound(format!(
                "snapshot '{}' not found",
                snapshot.0
            ))),
            Err(e) if status(&e) == Some(409) => Err(ContainerError::Config(format!(
                "snapshot '{}' is the image the machine container currently runs; \
                 it can be removed after the machine reverts to another snapshot",
                snapshot.0
            ))),
            Err(e) => Err(e.into()),
        }
    }

    async fn snapshots(&self, machine: &Machine) -> Result<Vec<SnapshotInfo>, ContainerError> {
        let docker = self.docker().await?;
        let current_image = tolerate(
            docker
                .inspect_container(&container_name(machine.id()), None)
                .await,
            &[404],
        )?
        .and_then(|info| info.image);

        Ok(self
            .snapshot_images(machine.id())
            .await?
            .into_iter()
            .map(|(tag, image)| SnapshotInfo {
                name: image
                    .labels
                    .get(LABEL_SNAPSHOT_NAME)
                    .cloned()
                    .unwrap_or_else(|| tag.clone()),
                id: SnapshotId(tag),
                description: None,
                created_at: image.created,
                size_bytes: Some(image.size as u64),
                is_current: current_image.as_deref() == Some(image.id.as_str()),
            })
            .collect())
    }
}

/// Short `malbox-snapshot:<tag>` form of a repo tag, if it is a snapshot tag.
///
/// Podman's compat API reports fully qualified references
/// (`docker.io/library/malbox-snapshot:<tag>`); Docker reports the short form.
fn snapshot_tag(repo_tag: &str) -> Option<&str> {
    let short = repo_tag.rsplit('/').next()?;
    short
        .strip_prefix(SNAPSHOT_REPO)
        .is_some_and(|rest| rest.starts_with(':'))
        .then_some(short)
}

#[async_trait]
impl Snapshot for ContainerProvider {
    /// Commit the (stopped) container to a `malbox-snapshot:<uuid>` image.
    async fn create_snapshot(
        &self,
        machine: &Machine,
        name: &str,
    ) -> Result<SnapshotId, Box<dyn Error + Send + Sync>> {
        Ok(self.commit(machine, name).await?)
    }

    /// Recreate the container from the snapshot image; it is left stopped.
    async fn restore_snapshot(
        &self,
        machine: &Machine,
        snapshot: &SnapshotId,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(self.restore(machine, snapshot).await?)
    }

    async fn delete_snapshot(
        &self,
        snapshot: &SnapshotId,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(self.remove_snapshot(snapshot).await?)
    }

    async fn list_snapshots(
        &self,
        machine: &Machine,
    ) -> Result<Vec<SnapshotInfo>, Box<dyn Error + Send + Sync>> {
        Ok(self.snapshots(machine).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::snapshot_tag;

    #[test]
    fn snapshot_tag_accepts_short_and_qualified_references() {
        assert_eq!(
            snapshot_tag("malbox-snapshot:ab12"),
            Some("malbox-snapshot:ab12")
        );
        assert_eq!(
            snapshot_tag("docker.io/library/malbox-snapshot:ab12"),
            Some("malbox-snapshot:ab12")
        );
        assert_eq!(snapshot_tag("malbox-snapshotx:ab12"), None);
        assert_eq!(snapshot_tag("malbox-base:ab12"), None);
    }
}
