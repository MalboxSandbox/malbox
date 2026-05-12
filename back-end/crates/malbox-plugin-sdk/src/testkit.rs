//! Test-only constructors for SDK types.
//!
//! This module is gated behind the `testkit` feature (or the built-in `cfg(test)`
//! flag for in-crate use). It exposes construction APIs that are otherwise
//! `pub(crate)` so that downstream test code can build `Context`
//! values directly.
//!
//! **Do not depend on this module in production code.**

#![cfg(any(test, feature = "testkit"))]

use crate::context::Context;
use malbox_plugin_transport::traits::TransportEmitter;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

impl Context {
    /// Create a minimal test context with task ID 0, an empty sample path,
    /// and no result channel. Results pushed on this context will be dropped.
    pub fn test_new(emitter: Arc<dyn TransportEmitter + Send + Sync>) -> Context {
        Context::new(
            0,
            PathBuf::new(),
            HashMap::new(),
            emitter,
            None,
            #[cfg(feature = "guest")]
            None,
        )
    }

    /// Create a test context wired to an mpsc sender so you can inspect
    /// results pushed during the test.
    pub fn test_new_with_tx(
        emitter: Arc<dyn TransportEmitter + Send + Sync>,
        tx: crate::context::ResultSender,
    ) -> Context {
        Context::new(
            0,
            PathBuf::new(),
            HashMap::new(),
            emitter,
            Some(tx),
            #[cfg(feature = "guest")]
            None,
        )
    }

    /// Create a test context with explicit values for every field.
    pub fn test_new_full(
        task_id: i32,
        sample_path: PathBuf,
        config: HashMap<String, String>,
        emitter: Arc<dyn TransportEmitter + Send + Sync>,
        result_tx: Option<crate::context::ResultSender>,
    ) -> Context {
        Context::new(
            task_id,
            sample_path,
            config,
            emitter,
            result_tx,
            #[cfg(feature = "guest")]
            None,
        )
    }
}
