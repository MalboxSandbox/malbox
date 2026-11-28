//! Shared parser for `plugin.toml` manifests.
//!
//! Consumed by:
//! - `malbox-plugin-internal` at plugin-registry scan time
//! - `malbox-plugin-macros` at proc-macro expansion time
//! - `malbox-codegen` when emitting C++ runtime-config headers

/// Manifest error type.
pub mod error;
/// Top-level manifest schema, parsing and validation.
pub mod manifest;
/// `[runtime]` section schema and default resolution.
pub mod runtime;

pub use error::ManifestError;
pub use manifest::{
    ExecutionContextConfig, ParameterConfig, ParameterKind, PluginInfo, PluginManifest,
    PluginStateConfig, PluginTypeConfig, ResultConfig, RoutingConfig, ScopeConfig, Stage,
    parse_manifest, validate_manifest,
};
pub use runtime::{
    AutoCollectConfig, AutoCollectRuleConfig, AutoCollectSectionConfig, PathsConfig,
    ResolvedAutoCollectRule, ResolvedAutoCollectSection, ResolvedRuntimeConfig, RuntimeConfig,
    StashConfig,
};
