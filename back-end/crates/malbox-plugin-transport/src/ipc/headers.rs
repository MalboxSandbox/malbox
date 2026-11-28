//! Zero-copy `#[repr(C)]` headers for IPC messages.
//!
//! These live in iceoryx2 `user_header` slots and are accessed via direct
//! pointer dereference from shared memory - no serialization needed.

use iceoryx2::prelude::ZeroCopySend;

/// Header for task requests (daemon -> plugin via request/response).
#[derive(Debug, Clone, Copy, Default, ZeroCopySend)]
#[repr(C)]
pub struct TaskRequestHeader {
    /// The task being dispatched.
    pub task_id: i32,
    /// 0 = Execute, 1 = Cancel.
    pub request_type: u8,
    /// Padding that keeps the layout explicit; always zero.
    pub reserved: [u8; 3],
}

/// Header for task responses (plugin -> daemon via request/response).
#[derive(Debug, Clone, Copy, Default, ZeroCopySend)]
#[repr(C)]
pub struct TaskResponseHeader {
    /// The task this response belongs to.
    pub task_id: i32,
    /// [`ResponseKind`] discriminant.
    pub kind: u8,
    /// [`ResultFormat`] discriminant.
    pub format: u8,
    /// Bit 0: `is_final`, Bit 1: `has_more_chunks`.
    pub flags: u8,
    /// Length of the result-name prefix in the payload (0 = no prefix).
    pub name_len: u8,
    /// For chunked results: 0-based chunk index.
    pub chunk_index: u32,
    /// Total payload size across all chunks (first chunk only; 0 if unknown).
    pub total_size: u64,
}

/// Header for lightweight events (per-plugin pub/sub).
#[derive(Debug, Clone, Copy, Default, ZeroCopySend)]
#[repr(C)]
pub struct EventHeader {
    /// [`EventKind`] discriminant.
    pub event_type: u16,
    /// Which plugin originated this event (0 for daemon events).
    pub source_plugin_id: i32,
    /// Context-dependent: `task_id`, `sample_id`, etc.
    pub associated_id: i32,
    /// Padding that keeps the layout explicit; always zero.
    pub reserved: [u8; 6],
}

// --- Flag constants ---

/// [`TaskResponseHeader::flags`] bit marking the last frame of a task's response.
pub const FLAG_IS_FINAL: u8 = 0b0000_0001;
/// [`TaskResponseHeader::flags`] bit marking a chunked result with more chunks to come.
pub const FLAG_HAS_MORE_CHUNKS: u8 = 0b0000_0010;

// --- Discriminant enums ---

/// What a task response frame carries; stored in [`TaskResponseHeader::kind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[expect(
    clippy::exhaustive_enums,
    reason = "wire protocol discriminant matched exhaustively by malbox-plugin-internal's IPC reactor"
)]
pub enum ResponseKind {
    /// A (possibly chunked) plugin result payload.
    Result = 0,
    /// A progress update for the running task.
    Progress = 1,
    /// The plugin is ready to accept tasks.
    Ready = 2,
    /// The payload names a file holding the result instead of inlining it.
    FileRef = 3,
    /// The task failed; the payload is the error message.
    Error = 4,
    /// Metadata for the next output (postcard-encoded `ArtifactMeta`).
    ArtifactMeta = 5,
}

impl From<ResponseKind> for u8 {
    /// Wire discriminant stored in [`TaskResponseHeader::kind`].
    #[expect(
        clippy::as_conversions,
        reason = "reading a fieldless repr(u8) enum discriminant requires `as`"
    )]
    fn from(kind: ResponseKind) -> Self {
        kind as u8
    }
}

/// Encoding of a result payload; stored in [`TaskResponseHeader::format`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[non_exhaustive]
pub enum ResultFormat {
    /// Opaque binary data.
    Bytes = 0,
    /// UTF-8 JSON document.
    Json = 1,
    /// Postcard-serialized value.
    Postcard = 2,
}

impl From<ResultFormat> for u8 {
    /// Wire discriminant stored in [`TaskResponseHeader::format`].
    #[expect(
        clippy::as_conversions,
        reason = "reading a fieldless repr(u8) enum discriminant requires `as`"
    )]
    fn from(format: ResultFormat) -> Self {
        format as u8
    }
}

/// System event carried by an [`EventHeader`]; stored in [`EventHeader::event_type`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
#[non_exhaustive]
pub enum EventKind {
    // Task lifecycle (associated_id = task_id)
    /// A task was created and queued.
    TaskCreated = 0,
    /// A task is about to begin processing.
    TaskStarting = 1,
    /// A task finished processing.
    TaskCompleted = 2,
    /// A task failed.
    TaskFailed = 3,
    /// A task was canceled.
    TaskCanceled = 4,

    // Plugin lifecycle (associated_id = plugin_id)
    /// A plugin started.
    PluginStarted = 10,
    /// A plugin stopped.
    PluginStopped = 11,

    // Sample lifecycle (associated_id = sample_id)
    /// A sample started.
    SampleStarted = 20,
    /// A sample stopped.
    SampleStopped = 21,
    /// A sample produced a result.
    SampleResultProduced = 22,

    // System (associated_id = 0)
    /// The daemon is shutting down.
    DaemonShutdown = 30,
    /// The daemon configuration was reloaded.
    ConfigReloaded = 31,
}

impl From<EventKind> for u16 {
    /// Wire discriminant stored in [`EventHeader::event_type`].
    #[expect(
        clippy::as_conversions,
        reason = "reading a fieldless repr(u16) enum discriminant requires `as`"
    )]
    fn from(kind: EventKind) -> Self {
        kind as u16
    }
}

impl TryFrom<u8> for ResponseKind {
    type Error = u8;
    fn try_from(v: u8) -> Result<Self, u8> {
        match v {
            0 => Ok(Self::Result),
            1 => Ok(Self::Progress),
            2 => Ok(Self::Ready),
            3 => Ok(Self::FileRef),
            4 => Ok(Self::Error),
            5 => Ok(Self::ArtifactMeta),
            _ => Err(v),
        }
    }
}

impl TryFrom<u8> for ResultFormat {
    type Error = u8;
    fn try_from(v: u8) -> Result<Self, u8> {
        match v {
            0 => Ok(Self::Bytes),
            1 => Ok(Self::Json),
            2 => Ok(Self::Postcard),
            _ => Err(v),
        }
    }
}

impl TryFrom<u16> for EventKind {
    type Error = u16;
    fn try_from(v: u16) -> Result<Self, u16> {
        match v {
            0 => Ok(Self::TaskCreated),
            1 => Ok(Self::TaskStarting),
            2 => Ok(Self::TaskCompleted),
            3 => Ok(Self::TaskFailed),
            4 => Ok(Self::TaskCanceled),
            10 => Ok(Self::PluginStarted),
            11 => Ok(Self::PluginStopped),
            20 => Ok(Self::SampleStarted),
            21 => Ok(Self::SampleStopped),
            22 => Ok(Self::SampleResultProduced),
            30 => Ok(Self::DaemonShutdown),
            31 => Ok(Self::ConfigReloaded),
            _ => Err(v),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_request_header_is_repr_c() {
        assert_eq!(std::mem::size_of::<TaskRequestHeader>(), 8);
    }

    #[test]
    fn task_response_header_is_repr_c() {
        assert_eq!(std::mem::size_of::<TaskResponseHeader>(), 24);
    }

    #[test]
    fn event_header_is_repr_c() {
        // u16(2) + pad(2) + i32(4) + i32(4) + [u8;6](6) + pad(2) = 20
        assert_eq!(std::mem::size_of::<EventHeader>(), 20);
    }

    #[test]
    fn response_kind_roundtrip() {
        for v in 0..=5u8 {
            let kind = ResponseKind::try_from(v).unwrap();
            assert_eq!(u8::from(kind), v);
        }
        assert!(ResponseKind::try_from(6).is_err());
    }

    #[test]
    fn event_kind_roundtrip() {
        let cases = [0, 1, 2, 3, 4, 10, 11, 20, 21, 22, 30, 31u16];
        for v in cases {
            let kind = EventKind::try_from(v).unwrap();
            assert_eq!(u16::from(kind), v);
        }
        assert!(EventKind::try_from(99).is_err());
    }
}
