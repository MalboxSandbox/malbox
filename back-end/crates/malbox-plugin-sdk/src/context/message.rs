//! Internal result message types for the plugin-runtime channel.
//!
//! These decouple the shared Context/ResultSink API from the gRPC transport
//! types (proto::TaskResult, tonic::Status), allowing host plugins to avoid
//! pulling in tonic/prost.

/// Wire format hint attached to result data so the receiver knows how to
/// interpret the bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultFormat {
    /// The data is valid JSON (UTF-8 encoded).
    Json,
    /// The data is arbitrary binary.
    Bytes,
    /// Format is unknown or not applicable (e.g. for ref messages).
    Unspecified,
}

/// What a [`TaskResultMessage`] represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultKind {
    /// A normal inline result.
    Result,
    /// A reference to a stashed large result (handle in `stash_handle`).
    ResultRef,
    /// A progress update (not a final result).
    Progress,
}

/// Transport-agnostic result message sent from [`ResultSink`](super::ResultSink)
/// to the runtime, which converts it to the appropriate wire format
/// (e.g. `proto::TaskResult` for gRPC, postcard for IPC).
#[derive(Debug, Clone)]
pub struct TaskResultMessage {
    /// ID of the task this result belongs to.
    pub task_id: i32,
    /// Logical name identifying this result (e.g. `"yara_matches"`).
    pub result_name: String,
    /// Payload bytes (inline result data, or empty for ref messages).
    pub data: Vec<u8>,
    /// How to interpret `data`.
    pub format: ResultFormat,
    /// Whether this is the final message for the task.
    pub is_final: bool,
    /// What kind of message this is (inline result, stash ref, or progress).
    pub kind: ResultKind,
    /// For ResultRef: the stash handle. Empty for inline results.
    pub stash_handle: String,
    /// For ResultRef: format of the original stashed data.
    pub stash_format: ResultFormat,
    /// For ResultRef: byte size of the stashed data.
    pub stash_size: u64,
}
