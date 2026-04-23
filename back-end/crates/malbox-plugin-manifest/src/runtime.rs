use crate::error::ManifestError;

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeConfig {
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub work_dir: Option<PathBuf>,
    #[serde(default)]
    pub log_overflow_dir: Option<PathBuf>,
    #[serde(default)]
    pub stash_threshold_bytes: Option<usize>,
    #[serde(default)]
    pub stash_ttl_secs: Option<u64>,
    #[serde(default)]
    pub log_filter: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedRuntimeConfig {
    pub port: u16,
    pub work_dir: PathBuf,
    pub log_overflow_dir: PathBuf,
    pub stash_threshold_bytes: usize,
    pub stash_ttl_secs: u64,
    pub log_filter: String,
}

impl ResolvedRuntimeConfig {
    pub fn from_raw(raw: &RuntimeConfig) -> Self {
        let work_dir = raw.work_dir.clone().unwrap_or_else(default_work_dir);
        let log_overflow_dir = raw
            .log_overflow_dir
            .clone()
            .unwrap_or_else(|| work_dir.join("_logs"));
        Self {
            port: raw.port.unwrap_or(50051),
            work_dir,
            log_overflow_dir,
            stash_threshold_bytes: raw.stash_threshold_bytes.unwrap_or(1_048_576),
            stash_ttl_secs: raw.stash_ttl_secs.unwrap_or(120),
            log_filter: raw.log_filter.clone().unwrap_or_else(|| "info".into()),
        }
    }

    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.port < 1024 {
            return Err(ManifestError::Invalid(format!(
                "runtime.port must be >= 1024, got {}",
                self.port
            )));
        }
        if !self.work_dir.is_absolute() {
            return Err(ManifestError::Invalid(format!(
                "runtime.work_dir must be absolute: {}",
                self.work_dir.display()
            )));
        }
        if !self.log_overflow_dir.is_absolute() {
            return Err(ManifestError::Invalid(format!(
                "runtime.log_overflow_dir must be absolute: {}",
                self.log_overflow_dir.display()
            )));
        }
        if self.stash_threshold_bytes < 4096 {
            return Err(ManifestError::Invalid(
                "runtime.stash_threshold_bytes must be >= 4096".into(),
            ));
        }
        if self.stash_ttl_secs < 1 {
            return Err(ManifestError::Invalid(
                "runtime.stash_ttl_secs must be >= 1".into(),
            ));
        }
        tracing_subscriber::EnvFilter::try_new(&self.log_filter).map_err(|e| {
            ManifestError::Invalid(format!(
                "runtime.log_filter is not a valid EnvFilter ({e}): {}",
                self.log_filter
            ))
        })?;
        Ok(())
    }
}

#[cfg(unix)]
fn default_work_dir() -> PathBuf {
    PathBuf::from("/tmp/malbox")
}

#[cfg(windows)]
fn default_work_dir() -> PathBuf {
    PathBuf::from(r"C:\malbox\work")
}
