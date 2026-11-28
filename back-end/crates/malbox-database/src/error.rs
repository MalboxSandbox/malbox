//! Error types returned by the database repositories.

use thiserror::Error;

/// Top-level error returned by every database operation in this crate.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum DatabaseError {
    /// Unclassified driver error.
    #[error("Database error: {0}")]
    SqlxError(#[from] sqlx::Error),
    /// A machine repository operation failed.
    #[error("{0}")]
    Machine(#[from] MachineError),
    /// A task repository operation failed.
    #[error("{0}")]
    Task(#[from] TaskError),
    /// A sample repository operation failed.
    #[error("{0}")]
    Sample(#[from] SampleError),
    /// An image repository operation failed.
    #[error("{0}")]
    Image(#[from] ImageError),
    /// A snapshot repository operation failed.
    #[error("{0}")]
    Snapshot(#[from] SnapshotError),
    /// A provision run repository operation failed.
    #[error("{0}")]
    ProvisionRun(#[from] ProvisionRunError),
    /// An artifact repository operation failed.
    #[error("{0}")]
    Artifact(#[from] ArtifactError),
    /// A job repository operation failed.
    #[error("{0}")]
    Job(#[from] JobError),
    /// A result cache repository operation failed.
    #[error("{0}")]
    ResultCache(#[from] ResultCacheError),
    /// Database was migrated by an incompatible, pre-rewrite release.
    #[error(
        "database schema was created by an incompatible malbox version (migration {version} \
         differs); drop and recreate the \"malbox\" database"
    )]
    IncompatibleSchema {
        /// Applied migration whose checksum differs from the embedded one.
        version: i64,
    },
    /// Database was migrated by a newer release than this binary.
    #[error(
        "database schema is newer than this malbox binary (migration {version} is unknown); \
         upgrade malbox"
    )]
    NewerSchema {
        /// Applied migration unknown to this binary.
        version: i64,
    },
    /// Applying the embedded migrations failed for another reason.
    #[error("Database migration failed: {0}")]
    Migrate(#[source] sqlx::migrate::MigrateError),
    /// A plugin report repository operation failed.
    #[error("{0}")]
    PluginReport(#[from] PluginReportError),
    /// A sample verdict repository operation failed.
    #[error("{0}")]
    SampleVerdict(#[from] SampleVerdictError),
    /// A recipe repository operation failed.
    #[error("{0}")]
    Recipe(#[from] RecipeError),
    /// A custom transform repository operation failed.
    #[error("{0}")]
    CustomTransform(#[from] CustomTransformError),
}

/// Failures of operations on the `machines` table.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum MachineError {
    /// Inserting a row failed.
    #[error("Failed to insert machine '{name}': {message}: {source}")]
    InsertFailed {
        /// Name of the machine being inserted or addressed.
        name: String,
        /// Context describing which step of the operation failed.
        message: String,
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
    /// Deleting rows failed.
    #[error("Failed to delete from `machines` table: {source}")]
    DeleteFailed {
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
    /// Querying rows failed.
    #[error("Failed to fetch machines: {source}")]
    FetchFailed {
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
    /// Updating rows failed.
    #[error("Failed to update machine: {message}: {source}")]
    UpdateFailed {
        /// Context describing which step of the operation failed.
        message: String,
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
}

/// Failures of operations on the `tasks` table.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum TaskError {
    /// Inserting a row failed.
    #[error("Failed to insert task '{name}': {message}: {source}")]
    InsertFailed {
        /// Name of the task being inserted or addressed.
        name: String,
        /// Context describing which step of the operation failed.
        message: String,
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
    /// Querying rows failed.
    #[error("Failed to fetch tasks: {message}: {source}")]
    FetchFailed {
        /// Context describing which step of the operation failed.
        message: String,
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
    /// Updating rows failed.
    #[error("Failed to update task {task_id}: {message}: {source}")]
    UpdateFailed {
        /// Database id of the task the operation addressed.
        task_id: i32,
        /// Context describing which step of the operation failed.
        message: String,
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
}

/// Failures of operations on the `samples` table.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum SampleError {
    /// Inserting a row failed.
    #[error("Failed to insert sample '{hash}': {message}: {source}")]
    InsertFailed {
        /// Content hash identifying the sample.
        hash: String,
        /// Context describing which step of the operation failed.
        message: String,
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
    /// Querying rows failed.
    #[error("Failed to fetch sample '{hash}': {message}: {source}")]
    FetchFailed {
        /// Content hash identifying the sample.
        hash: String,
        /// Context describing which step of the operation failed.
        message: String,
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
}

/// Failures of operations on the `images` table.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ImageError {
    /// Inserting a row failed.
    #[error("Failed to insert image '{name}': {message}: {source}")]
    InsertFailed {
        /// Name of the image being inserted or addressed.
        name: String,
        /// Context describing which step of the operation failed.
        message: String,
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
    /// Querying rows failed.
    #[error("Failed to fetch image: {source}")]
    FetchFailed {
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
    /// Updating rows failed.
    #[error("Failed to update image: {message}: {source}")]
    UpdateFailed {
        /// Context describing which step of the operation failed.
        message: String,
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
    /// Deleting rows failed.
    #[error("Failed to delete image '{name}': {source}")]
    DeleteFailed {
        /// Name of the image being inserted or addressed.
        name: String,
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
}

/// Failures of operations on the `snapshots` table.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum SnapshotError {
    /// Inserting a row failed.
    #[error("Failed to insert snapshot '{name}' for machine {machine_id}: {source}")]
    InsertFailed {
        /// Name of the snapshot being inserted or addressed.
        name: String,
        /// Database id of the machine the row belongs to.
        machine_id: i32,
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
    /// Querying rows failed.
    #[error("Failed to fetch snapshots: {source}")]
    FetchFailed {
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
    /// Updating rows failed.
    #[error("Failed to update snapshot: {message}: {source}")]
    UpdateFailed {
        /// Context describing which step of the operation failed.
        message: String,
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
    /// Deleting rows failed.
    #[error("Failed to delete snapshot: {source}")]
    DeleteFailed {
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
}

/// Failures of operations on the `provision_runs` table.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ProvisionRunError {
    /// Inserting a row failed.
    #[error("Failed to insert provision run for machine {machine_id}: {source}")]
    InsertFailed {
        /// Database id of the machine the row belongs to.
        machine_id: i32,
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
    /// Updating rows failed.
    #[error("Failed to update provision run: {message}: {source}")]
    UpdateFailed {
        /// Context describing which step of the operation failed.
        message: String,
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
    /// Querying rows failed.
    #[error("Failed to fetch provision runs: {source}")]
    FetchFailed {
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
    /// Deleting rows failed.
    #[error("Failed to delete provision runs: {source}")]
    DeleteFailed {
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
}

/// CRUD errors for the orchestration tables. `target` names the rows the
/// operation addressed, e.g. `job 42` or `artifacts of task 7`.
macro_rules! crud_error {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Error, Debug)]
        #[non_exhaustive]
        pub enum $name {
            /// Inserting a row failed.
            #[error("Failed to insert {target}: {source}")]
            InsertFailed {
                /// Human-readable description of the addressed rows, e.g. `job 42`.
                target: String,
                /// Underlying driver error.
                #[source]
                source: sqlx::Error,
            },
            /// Querying rows failed.
            #[error("Failed to fetch {target}: {source}")]
            FetchFailed {
                /// Human-readable description of the addressed rows, e.g. `job 42`.
                target: String,
                /// Underlying driver error.
                #[source]
                source: sqlx::Error,
            },
            /// Updating rows failed.
            #[error("Failed to update {target}: {source}")]
            UpdateFailed {
                /// Human-readable description of the addressed rows, e.g. `job 42`.
                target: String,
                /// Underlying driver error.
                #[source]
                source: sqlx::Error,
            },
            /// Deleting rows failed.
            #[error("Failed to delete {target}: {source}")]
            DeleteFailed {
                /// Human-readable description of the addressed rows, e.g. `job 42`.
                target: String,
                /// Underlying driver error.
                #[source]
                source: sqlx::Error,
            },
        }
    };
}

crud_error!(
    ArtifactError,
    "Failures of operations on the `artifacts` table."
);
crud_error!(JobError, "Failures of operations on the `jobs` table.");
crud_error!(
    ResultCacheError,
    "Failures of operations on the `result_cache` table."
);

/// Failures of operations on the `plugin_reports` table.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum PluginReportError {
    /// Inserting a row failed.
    #[error("Failed to insert plugin report for task {task_id} ({plugin_name}): {source}")]
    InsertFailed {
        /// Database id of the task the operation addressed.
        task_id: i32,
        /// Name of the plugin that produced the report.
        plugin_name: String,
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
    /// Querying rows failed.
    #[error("Failed to fetch plugin reports for task {task_id}: {source}")]
    FetchFailed {
        /// Database id of the task the operation addressed.
        task_id: i32,
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
    /// Querying the reports of a sample failed.
    #[error("Failed to fetch plugin reports for sample {sample_id}: {source}")]
    FetchForSampleFailed {
        /// Database id of the sample the operation addressed.
        sample_id: i64,
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
    /// Querying the reports of an artifact failed.
    #[error("Failed to fetch plugin reports for artifact {artifact_id}: {source}")]
    FetchForArtifactFailed {
        /// Database id of the artifact the operation addressed.
        artifact_id: i64,
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
    /// Copying a cached report onto a new task failed.
    #[error("Failed to copy plugin report of job {from_job_id} to task {to_task_id}: {source}")]
    CopyFailed {
        /// Database id of the job whose report is being reused.
        from_job_id: i64,
        /// Database id of the task receiving the copied report.
        to_task_id: i32,
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
}

/// Failures of operations on the `sample_verdicts` table.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum SampleVerdictError {
    /// Inserting or updating the row failed.
    #[error("Failed to upsert verdict for sample {sample_id}: {source}")]
    UpsertFailed {
        /// Database id of the sample the operation addressed.
        sample_id: i64,
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
    /// Querying rows failed.
    #[error("Failed to fetch verdict for sample: {source}")]
    FetchFailed {
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
}

/// Failures of operations on the `recipes` table.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum RecipeError {
    /// Inserting a row failed.
    #[error("Failed to insert recipe '{name}': {message}: {source}")]
    InsertFailed {
        /// Name of the recipe being inserted or addressed.
        name: String,
        /// Context describing which step of the operation failed.
        message: String,
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
    /// Querying rows failed.
    #[error("Failed to fetch recipe: {source}")]
    FetchFailed {
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
    /// Updating rows failed.
    #[error("Failed to update recipe: {message}: {source}")]
    UpdateFailed {
        /// Context describing which step of the operation failed.
        message: String,
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
    /// Deleting rows failed.
    #[error("Failed to delete recipe: {source}")]
    DeleteFailed {
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
}

/// Failures of operations on the `custom_transforms` table.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum CustomTransformError {
    /// Inserting a row failed.
    #[error("Failed to insert transform '{name}': {message}: {source}")]
    InsertFailed {
        /// Name of the transform being inserted or addressed.
        name: String,
        /// Context describing which step of the operation failed.
        message: String,
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
    /// Querying rows failed.
    #[error("Failed to fetch transform: {source}")]
    FetchFailed {
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
    /// Updating rows failed.
    #[error("Failed to update transform: {message}: {source}")]
    UpdateFailed {
        /// Context describing which step of the operation failed.
        message: String,
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
    /// Deleting rows failed.
    #[error("Failed to delete transform: {source}")]
    DeleteFailed {
        /// Underlying driver error.
        #[source]
        source: sqlx::Error,
    },
}

/// Result alias for fallible database operations.
pub type Result<T> = std::result::Result<T, DatabaseError>;
