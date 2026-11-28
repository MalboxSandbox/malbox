//! `docker-exec` guest session: file transfer and exec through the engine API.

use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use bollard::Docker;
use bollard::container::LogOutput;
use bollard::exec::{CreateExecOptions, StartExecOptions, StartExecResults};
use bollard::query_parameters::{DownloadFromContainerOptions, UploadToContainerOptions};
use bytes::Bytes;
use futures::TryStreamExt;
use malbox_machinery::{ExecOptions, ExecResult, GuestSession, GuestStatus};
use std::error::Error;

use crate::ContainerError;
use crate::error::tolerate;

/// Guest session backed by engine exec and archive APIs.
pub(crate) struct ContainerSession {
    pub(crate) docker: Docker,
    pub(crate) container: String,
    pub(crate) sample_dir: String,
}

/// Resolve a guest path: absolute paths stay, relative ones join `sample_dir`.
/// Any `..` component is rejected.
pub(crate) fn resolve_path(sample_dir: &str, path: &str) -> Result<String, ContainerError> {
    if path.split('/').any(|component| component == "..") {
        return Err(ContainerError::Config(
            "path traversal rejected".to_string(),
        ));
    }
    if path.starts_with('/') {
        Ok(path.to_string())
    } else {
        Ok(format!("{}/{path}", sample_dir.trim_end_matches('/')))
    }
}

impl ContainerSession {
    fn resolve(&self, path: &str) -> Result<String, ContainerError> {
        resolve_path(&self.sample_dir, path)
    }

    async fn push(&self, source: &Path, dest: &str) -> Result<(), ContainerError> {
        let dest = self.resolve(dest)?;
        if dest.ends_with('/') {
            return Err(ContainerError::Config(format!(
                "destination '{dest}' has no file name"
            )));
        }

        // Upload relative to `/` with the full path as the entry name: the
        // engine creates missing parent directories while extracting, so no
        // `mkdir` exec is needed. (Podman's compat API leaks two conmon
        // processes per attached exec.) No directory entries are written, so
        // existing parents such as `/tmp` keep their mode and ownership.
        let bytes = tokio::fs::read(source).await?;
        let mut header = tar::Header::new_gnu();
        header.set_size(u64::try_from(bytes.len()).expect("file size fits in u64"));
        header.set_mode(0o755);
        header.set_cksum();
        let mut builder = tar::Builder::new(Vec::new());
        builder.append_data(&mut header, dest.trim_start_matches('/'), &bytes[..])?;
        let archive = builder.into_inner()?;

        self.docker
            .upload_to_container(
                &self.container,
                Some(UploadToContainerOptions {
                    path: "/".to_string(),
                    ..Default::default()
                }),
                bollard::body_full(Bytes::from(archive)),
            )
            .await?;
        Ok(())
    }

    async fn pull(&self, source: &str, dest: &Path) -> Result<(), ContainerError> {
        let path = self.resolve(source)?;
        let data: Vec<u8> = self
            .docker
            .download_from_container(&self.container, Some(DownloadFromContainerOptions { path }))
            .try_fold(Vec::new(), |mut acc, chunk| async move {
                acc.extend_from_slice(&chunk);
                Ok(acc)
            })
            .await?;

        let dest = dest.to_path_buf();
        tokio::task::spawn_blocking(move || -> Result<(), ContainerError> {
            let mut archive = tar::Archive::new(&data[..]);
            for entry in archive.entries()? {
                let mut entry = entry?;
                if entry.header().entry_type() == tar::EntryType::Regular {
                    entry.unpack(&dest)?;
                    return Ok(());
                }
            }
            Err(ContainerError::NotFound("not a regular file".to_string()))
        })
        .await
        .map_err(|e| ContainerError::Io(std::io::Error::other(e)))?
    }

    async fn exec(
        &self,
        command: &str,
        args: &[String],
        opts: ExecOptions,
    ) -> Result<ExecResult, ContainerError> {
        let mut cmd = Vec::with_capacity(args.len() + 1);
        cmd.push(command.to_string());
        cmd.extend_from_slice(args);
        let env = opts.env.iter().map(|(k, v)| format!("{k}={v}")).collect();
        let working_dir = opts
            .cwd
            .as_deref()
            .map(|cwd| self.resolve(cwd))
            .transpose()?;

        let exec = self
            .docker
            .create_exec(
                &self.container,
                CreateExecOptions {
                    cmd: Some(cmd),
                    attach_stdout: Some(!opts.background),
                    attach_stderr: Some(!opts.background),
                    env: Some(env),
                    working_dir,
                    ..Default::default()
                },
            )
            .await?;

        let started = self
            .docker
            .start_exec(
                &exec.id,
                Some(StartExecOptions {
                    detach: opts.background,
                    ..Default::default()
                }),
            )
            .await?;

        let mut output = match started {
            StartExecResults::Detached => {
                return Ok(ExecResult {
                    exit_code: None,
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                });
            }
            StartExecResults::Attached { output, .. } => output,
        };

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let drain = async {
            while let Some(chunk) = output.try_next().await? {
                match chunk {
                    LogOutput::StdOut { message } => stdout.extend_from_slice(&message),
                    LogOutput::StdErr { message } => stderr.extend_from_slice(&message),
                    LogOutput::StdIn { .. } | LogOutput::Console { .. } => {}
                }
            }
            Ok::<_, bollard::errors::Error>(())
        };
        match opts.timeout {
            Some(duration) => tokio::time::timeout(duration, drain).await.map_err(|_| {
                ContainerError::Timeout {
                    operation: "exec".to_string(),
                    duration,
                }
            })??,
            None => drain.await?,
        }

        // The output stream can close just before the engine records the
        // exit code; poll briefly until the exec is no longer running.
        let mut inspect = self.docker.inspect_exec(&exec.id).await?;
        for _ in 0..50 {
            if inspect.running != Some(true) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
            inspect = self.docker.inspect_exec(&exec.id).await?;
        }

        Ok(ExecResult {
            exit_code: inspect.exit_code.and_then(|c| i32::try_from(c).ok()),
            stdout,
            stderr,
        })
    }

    async fn health(&self) -> Result<GuestStatus, ContainerError> {
        let Some(info) = tolerate(
            self.docker.inspect_container(&self.container, None).await,
            &[404],
        )?
        else {
            return Ok(GuestStatus::NotReady {
                reason: "container not found".to_string(),
            });
        };
        let state = info.state.unwrap_or_default();
        if state.running == Some(true) {
            Ok(GuestStatus::Ready)
        } else {
            Ok(GuestStatus::NotReady {
                reason: format!("container status {:?}", state.status),
            })
        }
    }
}

#[async_trait]
impl GuestSession for ContainerSession {
    async fn push_file(
        &self,
        source: &Path,
        dest: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(self.push(source, dest).await?)
    }

    async fn pull_file(
        &self,
        source: &str,
        dest: &Path,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(self.pull(source, dest).await?)
    }

    async fn execute(
        &self,
        command: &str,
        args: &[String],
        opts: ExecOptions,
    ) -> Result<ExecResult, Box<dyn Error + Send + Sync>> {
        Ok(self.exec(command, args, opts).await?)
    }

    async fn health_check(&self) -> Result<GuestStatus, Box<dyn Error + Send + Sync>> {
        Ok(self.health().await?)
    }

    async fn close(self: Box<Self>) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLES: &str = "/opt/malbox/samples";

    #[test]
    fn relative_paths_join_sample_dir() {
        assert_eq!(
            resolve_path(SAMPLES, "sample.bin").unwrap(),
            "/opt/malbox/samples/sample.bin"
        );
    }

    #[test]
    fn absolute_paths_are_unchanged() {
        assert_eq!(resolve_path(SAMPLES, "/tmp/x").unwrap(), "/tmp/x");
    }

    #[test]
    fn traversal_is_rejected() {
        assert!(resolve_path(SAMPLES, "../etc/passwd").is_err());
        assert!(resolve_path(SAMPLES, "/opt/../etc/passwd").is_err());
    }
}
