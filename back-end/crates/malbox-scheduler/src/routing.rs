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
    pub name: String,
    pub version: String,
    pub stage: Stage,
    pub category: String,
    pub accepts: Regex,
    pub rejects: Option<Regex>,
    pub sources_exclude: Vec<String>,
    pub timeout: Duration,
    pub cacheable: bool,
    pub recursion_prevention: Vec<String>,
    pub uses_indicators: bool,
    pub monitored_keys: Vec<String>,
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
    pub name: String,
    pub accepts: Regex,
    pub platform: MachinePlatform,
    pub arch: Option<MachineArch>,
    pub snapshot: Option<String>,
    pub tags: Vec<String>,
    pub guest_plugins: Vec<String>,
    pub timeout: Duration,
}

#[derive(Debug, Clone, Default)]
pub struct RoutingTable {
    analyzers: BTreeMap<String, AnalyzerSpec>,
    guest_plugins: BTreeSet<String>,
    profiles: Vec<DetonationProfile>,
}

/// Compile a routing expression anchored to the whole artifact type.
pub fn anchored(expr: &str) -> Result<Regex, regex::Error> {
    Regex::new(&format!("^(?:{expr})$"))
}

impl RoutingTable {
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

    pub fn analyzer(&self, name: &str) -> Option<&AnalyzerSpec> {
        self.analyzers.get(name)
    }

    /// Host analyzers in name order.
    pub fn analyzers(&self) -> impl Iterator<Item = &AnalyzerSpec> {
        self.analyzers.values()
    }

    pub fn is_guest_plugin(&self, name: &str) -> bool {
        self.guest_plugins.contains(name)
    }

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
