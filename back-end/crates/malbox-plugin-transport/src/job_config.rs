//! Keys of the per-job config map the daemon sends to plugins on both
//! transports.

/// Analysis timeout in seconds.
pub const ANALYSIS_TIMEOUT: &str = "analysis_timeout";
/// Job id (decimal i64).
pub const JOB_ID: &str = "malbox.job_id";
/// Artifact id (decimal i64).
pub const ARTIFACT_ID: &str = "malbox.artifact_id";
/// Artifact SHA-256 (hex).
pub const ARTIFACT_SHA256: &str = "malbox.artifact_sha256";
/// Artifact file-type taxonomy string.
pub const ARTIFACT_TYPE: &str = "malbox.artifact_type";
/// Artifact display name.
pub const ARTIFACT_NAME: &str = "malbox.artifact_name";
/// Artifact depth in the task tree (decimal).
pub const ARTIFACT_DEPTH: &str = "malbox.artifact_depth";
/// How many analysed artifacts this job may still emit (decimal).
pub const MAX_ARTIFACTS: &str = "malbox.max_artifacts";
/// `true` / `false`.
pub const DEEP_SCAN: &str = "malbox.deep_scan";
/// `daemon` / `plugin` (guest plugins only).
pub const LAUNCH_OWNER: &str = "malbox.launch_owner";
/// JSON object of task-wide shared data.
pub const SHARED: &str = "malbox.shared";
/// JSON array of `{sha256, artifact_type, relation}`, root first.
pub const ANCESTRY: &str = "malbox.ancestry";
/// JSON array of SDK indicators.
pub const INDICATORS: &str = "malbox.indicators";
/// JSON object of resolved plugin parameters.
pub const PARAMS: &str = "malbox.params";
