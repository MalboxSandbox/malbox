//! Effective per-task limits: submission overrides, else the configured
//! defaults.

use malbox_config::OrchestrationConfig;
use time::OffsetDateTime;

use crate::params::SubmissionParams;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EffectiveLimits {
    pub(crate) max_depth: u32,
    pub(crate) max_artifacts: u32,
    pub(crate) max_detonations: u32,
    pub(crate) max_total_bytes: u64,
    pub(crate) max_artifact_time_secs: u64,
    pub(crate) deadline_secs: u64,
}

impl EffectiveLimits {
    pub(crate) fn resolve(settings: &OrchestrationConfig, params: &SubmissionParams) -> Self {
        Self {
            max_depth: params.max_depth.unwrap_or(settings.max_depth),
            max_artifacts: params.max_artifacts.unwrap_or(settings.max_artifacts),
            max_detonations: params.max_detonations.unwrap_or(settings.max_detonations),
            max_total_bytes: params.max_total_bytes.unwrap_or(settings.max_total_bytes),
            max_artifact_time_secs: params
                .max_artifact_time_secs
                .unwrap_or(settings.max_artifact_time_secs),
            deadline_secs: params.deadline_secs.unwrap_or(settings.task_deadline_secs),
        }
    }

    /// Task deadline for a task (re)started now.
    pub(crate) fn deadline_from_now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc() + time::Duration::seconds(to_i64(self.deadline_secs))
    }
}

/// Saturating `u32` → `i32` for limit columns.
pub(crate) fn to_i32(v: u32) -> i32 {
    i32::try_from(v).unwrap_or(i32::MAX)
}

/// Saturating `u64` → `i64` for byte and second columns.
pub(crate) fn to_i64(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overrides_win_over_settings() {
        let settings = OrchestrationConfig::default();
        let params = SubmissionParams {
            max_depth: Some(2),
            deadline_secs: Some(60),
            ..Default::default()
        };
        let l = EffectiveLimits::resolve(&settings, &params);
        assert_eq!(l.max_depth, 2);
        assert_eq!(l.deadline_secs, 60);
        assert_eq!(l.max_artifacts, settings.max_artifacts);
        assert_eq!(l.max_artifact_time_secs, settings.max_artifact_time_secs);
    }
}
