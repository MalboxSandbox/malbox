//! Which analyzers run on which artifact types, and which detonation
//! profiles exist. Built once from the plugin registry and the config.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use malbox_config::DetonationProfileConfig;
use malbox_database::repositories::machinery::{MachineArch, MachinePlatform};
use malbox_plugin_internal::registry::manifest::PluginTypeConfig;
use malbox_plugin_internal::registry::snapshot::PluginSnapshot;
use malbox_plugin_internal::registry::types::PluginStatus;
use malbox_plugin_manifest::{ParameterConfig, Stage};
use regex::Regex;

use crate::error::SchedulerError;

/// Default host-plugin timeout when neither the routing nor the runtime
/// section sets one.
const DEFAULT_ANALYZER_TIMEOUT_SECS: u64 = 300;

/// A host plugin as the orchestrator sees it.
#[derive(Debug, Clone)]
pub struct AnalyzerSpec {
    /// Plugin name; also the job's `analyzer`.
    pub name: String,
    /// Plugin version, part of the result-cache key.
    pub version: String,
    /// Pipeline stage the plugin runs in.
    pub stage: Stage,
    /// Category used by recursion prevention to forbid plugins on descendants.
    pub category: String,
    /// Artifact types the plugin runs on (anchored regex).
    pub accepts: Regex,
    /// Artifact types excluded even when `accepts` matches.
    pub rejects: Option<Regex>,
    /// Plugins whose outputs this plugin must not analyse.
    pub sources_exclude: Vec<String>,
    /// Per-job time limit.
    pub timeout: Duration,
    /// Whether results may be cached and replayed.
    pub cacheable: bool,
    /// Categories forbidden on artifacts this plugin produces.
    pub recursion_prevention: Vec<String>,
    /// Whether the plugin reads task-wide indicators (makes results uncacheable).
    pub uses_indicators: bool,
    /// Shared-data keys whose change re-runs this plugin.
    pub monitored_keys: Vec<String>,
    /// Declared submission parameters, by name.
    pub parameters: BTreeMap<String, ParameterConfig>,
}

impl AnalyzerSpec {
    /// `accepts` matches and `rejects` (if any) does not.
    pub fn routes(&self, artifact_type: &str) -> bool {
        self.accepts.is_match(artifact_type)
            && !self
                .rejects
                .as_ref()
                .is_some_and(|r| r.is_match(artifact_type))
    }
}

/// A configured detonation profile.
#[derive(Debug, Clone)]
pub struct DetonationProfile {
    /// Profile name, referenced by submissions and stored in jobs.
    pub name: String,
    /// Artifact types this profile detonates (anchored regex).
    pub accepts: Regex,
    /// Guest operating system of matching machines.
    pub platform: MachinePlatform,
    /// Required CPU architecture, if any.
    pub arch: Option<MachineArch>,
    /// Snapshot to restore before detonation.
    pub snapshot: Option<String>,
    /// Tags a machine must carry.
    pub tags: Vec<String>,
    /// Guest plugins run during detonation.
    pub guest_plugins: Vec<String>,
    /// Detonation time limit.
    pub timeout: Duration,
}

/// Lookup of host analyzers, guest plugins and detonation profiles.
#[derive(Debug, Clone, Default)]
pub struct RoutingTable {
    analyzers: BTreeMap<String, AnalyzerSpec>,
    guest_plugins: BTreeSet<String>,
    profiles: Vec<DetonationProfile>,
}

/// Compile a routing expression anchored to the whole artifact type.
///
/// # Errors
///
/// Fails when `expr` is not a valid regular expression.
pub fn anchored(expr: &str) -> Result<Regex, regex::Error> {
    Regex::new(&format!("^(?:{expr})$"))
}

impl RoutingTable {
    /// Build a table from already-resolved analyzers, guest plugin names and profiles.
    #[must_use]
    pub fn new(
        analyzers: Vec<AnalyzerSpec>,
        guest_plugins: BTreeSet<String>,
        profiles: Vec<DetonationProfile>,
    ) -> Self {
        Self {
            analyzers: analyzers.into_iter().map(|a| (a.name.clone(), a)).collect(),
            guest_plugins,
            profiles,
        }
    }

    /// Build the table from the registered plugins and the configured
    /// detonation profiles.
    ///
    /// # Errors
    ///
    /// Fails when a plugin's `accepts`/`rejects` or a profile's `accepts` is an
    /// invalid regular expression.
    pub fn from_registry(
        snap: &PluginSnapshot,
        profiles: &[DetonationProfileConfig],
    ) -> Result<Self, SchedulerError> {
        let mut analyzers = Vec::new();
        let mut guest_plugins = BTreeSet::new();

        for entry in snap.list() {
            if !matches!(entry.status, PluginStatus::Registered) {
                continue;
            }
            let manifest = &entry.manifest;
            let name = manifest.plugin.name.clone();
            match manifest.plugin.plugin_type {
                PluginTypeConfig::Guest => {
                    guest_plugins.insert(name);
                }
                PluginTypeConfig::Host => {
                    let routing = &manifest.routing;
                    let accepts = anchored(&routing.accepts).map_err(|e| {
                        SchedulerError::Internal(format!(
                            "plugin {name}: invalid routing.accepts: {e}"
                        ))
                    })?;
                    let rejects = if routing.rejects.is_empty() {
                        None
                    } else {
                        Some(anchored(&routing.rejects).map_err(|e| {
                            SchedulerError::Internal(format!(
                                "plugin {name}: invalid routing.rejects: {e}"
                            ))
                        })?)
                    };
                    let timeout_secs = routing
                        .timeout
                        .or(entry.runtime_config.as_ref().map(|r| r.analysis_timeout))
                        .unwrap_or(DEFAULT_ANALYZER_TIMEOUT_SECS);
                    analyzers.push(AnalyzerSpec {
                        version: manifest.plugin.version.clone(),
                        stage: routing.stage,
                        category: routing.category().to_string(),
                        accepts,
                        rejects,
                        sources_exclude: routing.sources_exclude.clone(),
                        timeout: Duration::from_secs(timeout_secs),
                        cacheable: routing.cacheable,
                        recursion_prevention: routing.recursion_prevention.clone(),
                        uses_indicators: routing.uses_indicators,
                        monitored_keys: routing.monitored_keys.clone(),
                        parameters: manifest.parameters.clone(),
                        name,
                    });
                }
            }
        }

        let profiles = profiles
            .iter()
            .map(|p| {
                Ok(DetonationProfile {
                    name: p.name.clone(),
                    accepts: anchored(&p.accepts).map_err(|e| {
                        SchedulerError::Internal(format!(
                            "detonation profile {}: invalid accepts: {e}",
                            p.name
                        ))
                    })?,
                    platform: MachinePlatform::from(p.platform),
                    arch: p.arch.map(MachineArch::from),
                    snapshot: p.snapshot.clone(),
                    tags: p.tags.clone(),
                    guest_plugins: p.guest_plugins.clone(),
                    timeout: Duration::from_secs(p.timeout_secs),
                })
            })
            .collect::<Result<Vec<_>, SchedulerError>>()?;

        Ok(Self::new(analyzers, guest_plugins, profiles))
    }

    /// Host analyzer with the given plugin name.
    pub fn analyzer(&self, name: &str) -> Option<&AnalyzerSpec> {
        self.analyzers.get(name)
    }

    /// Host analyzers in name order.
    pub fn analyzers(&self) -> impl Iterator<Item = &AnalyzerSpec> {
        self.analyzers.values()
    }

    /// Whether `name` is a registered guest plugin.
    pub fn is_guest_plugin(&self, name: &str) -> bool {
        self.guest_plugins.contains(name)
    }

    /// Detonation profile with the given name.
    pub fn profile(&self, name: &str) -> Option<&DetonationProfile> {
        self.profiles.iter().find(|p| p.name == name)
    }

    /// First profile, in config order, whose `accepts` matches.
    pub fn match_profile(&self, artifact_type: &str) -> Option<&DetonationProfile> {
        self.profiles
            .iter()
            .find(|p| p.accepts.is_match(artifact_type))
    }
}
