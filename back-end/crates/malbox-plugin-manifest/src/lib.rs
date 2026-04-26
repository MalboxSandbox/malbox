//! Shared parser for `plugin.toml` manifests.
//!
//! Consumed by:
//! - `malbox-plugin-internal` at plugin-registry scan time
//! - `malbox-plugin-macros` at proc-macro expansion time
//! - `malbox-codegen` when emitting C++ runtime-config headers

pub mod error;
pub mod manifest;
pub mod runtime;

pub use error::ManifestError;
pub use manifest::{
    EventFilterConfig, EventsConfig, ExecutionContextConfig, PluginInfo, PluginManifest,
    PluginStateConfig, PluginTypeConfig, ResultConfig, ScopeConfig, parse_manifest,
    validate_manifest,
};
pub use runtime::{PathsConfig, ResolvedRuntimeConfig, RuntimeConfig, StashConfig};
