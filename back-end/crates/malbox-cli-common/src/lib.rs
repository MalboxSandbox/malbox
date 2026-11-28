//! Shared building blocks for the malbox command-line interface.
//!
//! Provides the HTTP API client, command context, error type, CLI argument
//! types, and terminal formatting helpers used by the `malbox` binary.

pub mod api;
pub mod command;
pub mod context;
pub mod error;
pub mod types;
pub mod utils;
