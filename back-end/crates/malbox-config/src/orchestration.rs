use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::error::ConfigError;
use crate::machines::Arch;
use crate::types::Platform;

/// Profile name of the job emitted for `detonate = always` when nothing
/// matches; the detonation executor fails it as unserviceable.
pub const UNMATCHED_PROFILE: &str = "unmatched";

/// Profile name of a detonation built from the submission's explicit
/// platform / snapshot / machine.
pub const SUBMISSION_PROFILE: &str = "submission";

/// Limits and policies for the analysis orchestrator.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct OrchestrationConfig {
    pub max_depth: u32,
    /// Analysed artifacts per task.
    pub max_artifacts: u32,
    /// Artifacts (any kind) one job may emit.
    pub max_artifacts_per_job: u32,
    pub max_detonations: u32,
    pub max_total_bytes: u64,
    /// Larger artifacts are kept but not analysed.
    pub max_file_size: u64,
    pub task_deadline_secs: u64,
    pub max_artifact_time_secs: u64,
    pub stall_check_secs: u64,
    pub max_attempts: u32,
    /// Multiplied by the attempt number.
    pub retry_backoff_secs: u64,
    /// Monitored-key re-runs per job.
    pub max_reruns: u32,
    /// Concurrently running host jobs, daemon-wide.
    pub host_concurrency: u32,
    pub max_running_jobs_per_task: u32,
    pub max_concurrent_vm_boots: u32,
    pub detonation_decision_window_secs: u64,
    pub cache_ttl_secs: u64,
    pub empty_result_cache_ttl_secs: u64,
    pub detonation_profiles: Vec<DetonationProfileConfig>,
}

impl Default for OrchestrationConfig {
    fn default() -> Self {
        Self {
            max_depth: 6,
            max_artifacts: 500,
            max_artifacts_per_job: 1000,
            max_detonations: 3,
            max_total_bytes: 2_147_483_648,
            max_file_size: 104_857_600,
            task_deadline_secs: 7200,
            max_artifact_time_secs: 1800,
            stall_check_secs: 600,
            max_attempts: 3,
            retry_backoff_secs: 10,
            max_reruns: 2,
            host_concurrency: 4,
            max_running_jobs_per_task: 8,
            max_concurrent_vm_boots: 2,
            detonation_decision_window_secs: 60,
            cache_ttl_secs: 604_800,
            empty_result_cache_ttl_secs: 86_400,
            detonation_profiles: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetonationProfileConfig {
    pub name: String,
    /// Regex over artifact_type, anchored as `^(?:expr)$`.
    pub accepts: String,
    pub platform: Platform,
    /// Snapshot name; `None` = the machine's active snapshot.
    #[serde(default)]
    pub snapshot: Option<String>,
    #[serde(default)]
    pub arch: Option<Arch>,
    /// All must be present on the machine.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Empty = every guest plugin in the chosen snapshot.
    #[serde(default)]
    pub guest_plugins: Vec<String>,
    #[serde(default = "default_profile_timeout")]
    pub timeout_secs: u64,
}

fn default_profile_timeout() -> u64 {
    300
}

fn invalid(field: impl Into<String>, message: impl Into<String>) -> ConfigError {
    ConfigError::InvalidValue {
        field: field.into(),
        message: message.into(),
    }
}

pub fn validate_orchestration(cfg: &OrchestrationConfig) -> Result<(), ConfigError> {
    for (field, value) in [
        ("orchestration.max_attempts", cfg.max_attempts),
        ("orchestration.host_concurrency", cfg.host_concurrency),
        (
            "orchestration.max_concurrent_vm_boots",
            cfg.max_concurrent_vm_boots,
        ),
        (
            "orchestration.max_running_jobs_per_task",
            cfg.max_running_jobs_per_task,
        ),
    ] {
        if value < 1 {
            return Err(invalid(field, "must be at least 1"));
        }
    }
    let mut seen = HashSet::new();
    for p in &cfg.detonation_profiles {
        if p.name == UNMATCHED_PROFILE || p.name == SUBMISSION_PROFILE {
            return Err(invalid(
                format!("orchestration.detonation_profiles.{}.name", p.name),
                "name is reserved",
            ));
        }
        if !seen.insert(p.name.as_str()) {
            return Err(invalid(
                "orchestration.detonation_profiles",
                format!("duplicate profile name '{}'", p.name),
            ));
        }
        if p.timeout_secs == 0 {
            return Err(invalid(
                format!("orchestration.detonation_profiles.{}.timeout_secs", p.name),
                "must be at least 1",
            ));
        }
        regex::Regex::new(&format!("^(?:{})$", p.accepts)).map_err(|e| {
            invalid(
                format!("orchestration.detonation_profiles.{}.accepts", p.name),
                e.to_string(),
            )
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(name: &str, accepts: &str) -> DetonationProfileConfig {
        DetonationProfileConfig {
            name: name.into(),
            accepts: accepts.into(),
            platform: Platform::Windows,
            snapshot: None,
            arch: None,
            tags: vec![],
            guest_plugins: vec![],
            timeout_secs: 300,
        }
    }

    #[test]
    fn default_is_valid() {
        validate_orchestration(&OrchestrationConfig::default()).unwrap();
    }

    #[test]
    fn rejects_duplicate_profiles() {
        let cfg = OrchestrationConfig {
            detonation_profiles: vec![profile("a", ".*"), profile("a", ".*")],
            ..Default::default()
        };
        assert!(validate_orchestration(&cfg).is_err());
    }

    #[test]
    fn rejects_bad_regex() {
        let cfg = OrchestrationConfig {
            detonation_profiles: vec![profile("a", "(")],
            ..Default::default()
        };
        assert!(validate_orchestration(&cfg).is_err());
    }

    #[test]
    fn rejects_reserved_profile_names() {
        for name in [UNMATCHED_PROFILE, SUBMISSION_PROFILE] {
            let cfg = OrchestrationConfig {
                detonation_profiles: vec![profile(name, ".*")],
                ..Default::default()
            };
            assert!(validate_orchestration(&cfg).is_err());
        }
    }

    #[test]
    fn rejects_zero_profile_timeout() {
        let mut p = profile("a", ".*");
        p.timeout_secs = 0;
        let cfg = OrchestrationConfig {
            detonation_profiles: vec![p],
            ..Default::default()
        };
        assert!(validate_orchestration(&cfg).is_err());
    }

    #[test]
    fn rejects_zero_values() {
        let zeroed: [fn(&mut OrchestrationConfig); 4] = [
            |c| c.max_attempts = 0,
            |c| c.host_concurrency = 0,
            |c| c.max_concurrent_vm_boots = 0,
            |c| c.max_running_jobs_per_task = 0,
        ];
        for f in zeroed {
            let mut cfg = OrchestrationConfig::default();
            f(&mut cfg);
            assert!(validate_orchestration(&cfg).is_err());
        }
    }

    #[test]
    fn partial_section_uses_defaults() {
        let cfg: OrchestrationConfig = toml::from_str(
            r#"
max_depth = 3
[[detonation_profiles]]
name = "win"
accepts = "executable/windows/.*"
platform = "windows"
"#,
        )
        .unwrap();
        assert_eq!(cfg.max_depth, 3);
        assert_eq!(cfg.max_artifacts, 500);
        assert_eq!(cfg.detonation_profiles[0].timeout_secs, 300);
        validate_orchestration(&cfg).unwrap();
    }

    #[test]
    fn rejects_unknown_key() {
        assert!(toml::from_str::<OrchestrationConfig>("bogus = 1").is_err());
    }
}
