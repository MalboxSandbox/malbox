//! Re-exports of the shared manifest parser.
//!
//! The canonical types and `parse_manifest`/`validate_manifest` live in the
//! `malbox-plugin-manifest` crate (also consumed by the SDK proc-macros and
//! the C++ codegen CLI).
pub use malbox_plugin_manifest::{
    EventFilterConfig, EventsConfig, ExecutionContextConfig, ManifestError, PathsConfig,
    PluginInfo, PluginManifest, PluginStateConfig, PluginTypeConfig, ResultConfig, RuntimeConfig,
    ScopeConfig, StashConfig, parse_manifest, validate_manifest,
};
