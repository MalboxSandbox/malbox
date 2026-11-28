//! Repository functions, one module per database table.

/// Files extracted or produced during analysis tasks.
pub mod artifacts;
/// User-defined transforms applied to plugin output.
pub mod custom_transforms;
/// Base VM images that machines and snapshots are built from.
pub mod images;
/// Analysis jobs that group a sample's tasks.
pub mod jobs;
/// Analysis machines and their lifecycle state.
pub mod machinery;
/// Reports produced by plugins for tasks, samples, and artifacts.
pub mod plugin_reports;
/// Records of machine provisioning attempts.
pub mod provision_runs;
/// Provisioning recipes describing how machines are built.
pub mod recipes;
/// Cache of plugin results keyed by input, for reuse across jobs.
pub mod result_cache;
/// Aggregated verdicts per sample.
pub mod sample_verdicts;
/// Submitted samples and their metadata.
pub mod samples;
/// VM snapshots taken of provisioned machines.
pub mod snapshots;
/// Individual analysis tasks and their scheduling state.
pub mod tasks;
