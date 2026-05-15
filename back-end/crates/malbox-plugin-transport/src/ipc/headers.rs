//! Zero-copy `#[repr(C)]` headers for IPC messages.
//!
//! These live in iceoryx2 user_header slots and are accessed via direct
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
    pub _reserved: [u8; 3],
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
    /// Bit 0: is_final, Bit 1: has_more_chunks.
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
    /// Context-dependent: task_id, sample_id, etc.
    pub associated_id: i32,
    pub _reserved: [u8; 6],
}

/// Header for result data on the per-plugin result pub/sub channel.
#[derive(Debug, Clone, Copy, Default, ZeroCopySend)]
#[repr(C)]
pub struct ResultHeader {
    /// The task (or chain context) this result belongs to.
    pub task_id: i32,
    /// [`ResultPayloadKind`] discriminant.
    pub payload_kind: u8,
    /// [`ResultFormat`] discriminant.
    pub format: u8,
    /// Bit 0: has_more_chunks.
    pub flags: u8,
    pub _reserved: u8,
    /// 0-based chunk index for multi-chunk results.
    pub chunk_index: u32,
    /// Total size in bytes (first chunk only; 0 if unknown).
    pub total_size: u64,
    /// Length of the result name prefix in the first chunk's payload.
    pub name_len: u16,
    pub _reserved2: [u8; 2],
}

// --- Flag constants ---

pub const FLAG_IS_FINAL: u8 = 0b0000_0001;
pub const FLAG_HAS_MORE_CHUNKS: u8 = 0b0000_0010;

// --- Discriminant enums ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResponseKind {
    Result = 0,
    Progress = 1,
    Ready = 2,
    FileRef = 3,
    Error = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResultFormat {
    Bytes = 0,
    Json = 1,
    Postcard = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResultPayloadKind {
    Inline = 0,
    FileRef = 1,
    Structured = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum EventKind {
    // Task lifecycle (associated_id = task_id)
    TaskCreated = 0,
    TaskStarting = 1,
    TaskCompleted = 2,
    TaskFailed = 3,
    TaskCanceled = 4,

    // Plugin lifecycle (associated_id = plugin_id)
    PluginStarted = 10,
    PluginStopped = 11,
    PluginResultAvailable = 12,

    // Sample lifecycle (associated_id = sample_id)
    SampleStarted = 20,
    SampleStopped = 21,
    SampleResultProduced = 22,

    // System (associated_id = 0)
    DaemonShutdown = 30,
    ConfigReloaded = 31,
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

impl TryFrom<u8> for ResultPayloadKind {
    type Error = u8;
    fn try_from(v: u8) -> Result<Self, u8> {
        match v {
            0 => Ok(Self::Inline),
            1 => Ok(Self::FileRef),
            2 => Ok(Self::Structured),
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
            12 => Ok(Self::PluginResultAvailable),
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
    fn result_header_is_repr_c() {
        // i32(4) + u8 + u8 + u8 + u8 + u32(4) + u64(8) + u16(2) + [u8;2](2) + pad(4) = 32
        assert_eq!(std::mem::size_of::<ResultHeader>(), 32);
    }

    #[test]
    fn response_kind_roundtrip() {
        for v in 0..=4u8 {
            let kind = ResponseKind::try_from(v).unwrap();
            assert_eq!(kind as u8, v);
        }
        assert!(ResponseKind::try_from(5).is_err());
    }

    #[test]
    fn event_kind_roundtrip() {
        let cases = [0, 1, 2, 3, 4, 10, 11, 12, 20, 21, 22, 30, 31u16];
        for v in cases {
            let kind = EventKind::try_from(v).unwrap();
            assert_eq!(kind as u16, v);
        }
        assert!(EventKind::try_from(99).is_err());
    }
}
