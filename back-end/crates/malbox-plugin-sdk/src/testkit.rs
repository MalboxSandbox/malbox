//! Test-only constructors for SDK types.
//!
//! This module is gated behind the `testkit` feature (or the built-in `cfg(test)`
//! flag for in-crate use). It exposes construction APIs that are otherwise
//! `pub(crate)` so that downstream test code can build `Task` and `Context`
//! values directly.
//!
//! **Do not depend on this module in production code.**

#![cfg(any(test, feature = "testkit"))]

use crate::context::Context;
use crate::types::Task;
use malbox_plugin_transport::traits::TransportEmitter;
use std::collections::HashMap;
use std::path::PathBuf;

impl Task {
    /// Construct a `Task` for testing. Available under `cfg(test)` or the
    /// `testkit` feature.
    pub fn test_new(id: i32, sample_path: PathBuf, config: HashMap<String, String>) -> Self {
        Self::new(id, sample_path, config)
    }
}

impl<'a> Context<'a> {
    /// Construct a `Context` for testing with no result channel.
    pub fn test_new(emitter: &'a dyn TransportEmitter) -> Context<'a> {
        Context::new(emitter, None)
    }

    /// Construct a `Context` for testing with an mpsc result sender.
    pub fn test_new_with_tx(
        emitter: &'a dyn TransportEmitter,
        tx: crate::context::ResultSender,
    ) -> Context<'a> {
        Context::new(emitter, Some(tx))
    }
}
