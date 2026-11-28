//! Shared utilities for Malbox: hashing, file identification, image store
//! watching and content-addressed on-disk storage.

/// Hex-encoded digests (MD5, SHA-1/256/512, CRC-32) over byte buffers.
pub mod hashing;
pub mod identify;
pub mod image_store;
pub mod storage;

pub use storage::SampleStore;
pub use storage::results::ResultStore;
