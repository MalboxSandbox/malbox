use std::ffi::c_char;

use super::enums::{MalboxExecutionContext, MalboxPluginState};

/// Health status reported by a plugin's `health_check` callback.
///
/// The `reason` field is optional: when `ready` is `true` it may be null.
/// When `ready` is `false`, `reason` should point to a null-terminated string
/// describing why the plugin is not ready.  The string must remain valid for
/// the duration of the `health_check` call.
#[repr(C)]
#[derive(Debug)]
pub struct MalboxHealthStatus {
    /// `true` if the plugin is ready to accept tasks.
    pub ready: bool,
    /// Optional null-terminated string describing the health state.  May be
    /// null when `ready` is `true`.
    pub reason: *const c_char,
}

/// Static metadata that a C++ plugin provides when registering with the daemon.
///
/// String fields (`name`, `version`, `authors`) must be non-null,
/// null-terminated, valid UTF-8 C strings.  `description` is optional and may
/// be null.
#[repr(C)]
#[derive(Debug)]
pub struct MalboxPluginMeta {
    /// Unique plugin name (non-null, null-terminated UTF-8).
    pub name: *const c_char,
    /// Semantic version string (non-null, null-terminated UTF-8).
    pub version: *const c_char,
    /// Optional human-readable description (null-terminated UTF-8, or null).
    pub description: *const c_char,
    /// Author(s) of the plugin (non-null, null-terminated UTF-8).
    pub authors: *const c_char,
    /// Lifetime policy for the plugin instance.
    pub state: MalboxPluginState,
    /// Concurrency scheduling policy.
    pub execution: MalboxExecutionContext,
}

/// One auto-collect rule: files matching `include` (and not `exclude`) are
/// pushed as artifacts with the given relation.
///
/// `relation` is 1-7 (see `ArtifactRelation`); `analyze` is -1 (relation
/// default), 0 (false) or 1 (true).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MalboxAutoCollectRule {
    /// Array of `include_count` NUL-terminated glob patterns a file must match.
    pub include: *const *const c_char,
    /// Number of entries in `include`.
    pub include_count: usize,
    /// Array of `exclude_count` NUL-terminated glob patterns that veto a match.
    pub exclude: *const *const c_char,
    /// Number of entries in `exclude`.
    pub exclude_count: usize,
    /// `ArtifactRelation` wire value (1-7) assigned to collected files.
    pub relation: i32,
    /// Analysis override: -1 uses the relation default, 0 disables, 1 enables.
    pub analyze: i32,
}

/// Auto-collection configuration for a single directory.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MalboxAutoCollectConfig {
    /// Whether auto-collection runs for this directory at all.
    pub enabled: bool,
    /// Array of `include_count` NUL-terminated glob patterns for files to collect.
    pub include: *const *const c_char,
    /// Number of entries in `include`.
    pub include_count: usize,
    /// Array of `exclude_count` NUL-terminated glob patterns for files to skip.
    pub exclude: *const *const c_char,
    /// Number of entries in `exclude`.
    pub exclude_count: usize,
    /// Files larger than this many bytes are never collected.
    pub max_file_size: u64,
    /// Array of `rule_count` per-pattern artifact rules (artifact directory only).
    pub rules: *const MalboxAutoCollectRule,
    /// Number of entries in `rules`.
    pub rule_count: usize,
}

/// Runtime configuration baked into a guest plugin at build time.
///
/// Passed by-value to [`malbox_run_guest_plugin`](crate::ffi_runtime::malbox_run_guest_plugin).
/// All string pointers must be non-null, valid, null-terminated UTF-8 C strings
/// whose storage lives at least for the duration of that call.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MalboxGuestRuntimeConfig {
    /// TCP port the guest gRPC server listens on (bound on all IPv4 interfaces).
    pub port: u16,
    /// Directory where the daemon pushes sample files.
    pub sample_dir: *const c_char,
    /// Directory where the plugin writes artifact files for collection.
    pub artifact_dir: *const c_char,
    /// Directory backing the disk stash for oversized results.
    pub stash_dir: *const c_char,
    /// Directory for log overflow files written when the ring buffer is full.
    pub log_dir: *const c_char,
    /// Directory of external log files collected after analysis.
    pub external_log_dir: *const c_char,
    /// Results larger than this many bytes are stashed to disk instead of sent inline.
    pub stash_threshold_bytes: usize,
    /// Seconds a stashed result may stay un-pulled before it is cleaned up.
    pub stash_ttl_secs: u64,
    /// `tracing` filter directive such as `"info"` or `"my_plugin=debug"`.
    pub log_filter: *const c_char,
    /// Default analysis timeout in seconds when the daemon does not send one.
    pub analysis_timeout: u64,
    /// Auto-collection settings for `artifact_dir`.
    pub auto_collect_artifacts: MalboxAutoCollectConfig,
    /// Auto-collection settings for `external_log_dir`.
    pub auto_collect_external_logs: MalboxAutoCollectConfig,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_layout() {
        let hs = MalboxHealthStatus {
            ready: true,
            reason: std::ptr::null(),
        };
        assert!(hs.ready);
        assert!(hs.reason.is_null());
    }
}
