//! Single source of truth for the daemon Cargo features the install wizard,
//! upgrades and provider commands can toggle.

/// A daemon Cargo feature exposed to users.
pub struct DaemonFeature {
    /// Human-readable label shown in interactive prompts.
    pub display: &'static str,
    /// Cargo feature name as declared in `bin/malboxctl/Cargo.toml`.
    pub feature: &'static str,
    /// Part of the default set. Must mirror the Cargo `default` feature list
    /// (enforced by the `feature_sync` test in malboxctl).
    pub default: bool,
}

pub const DAEMON_FEATURES: &[DaemonFeature] = &[
    DaemonFeature {
        display: "libvirt (virtualization provider)",
        feature: "provider-libvirt",
        default: true,
    },
    DaemonFeature {
        display: "ansible (machine provisioner)",
        feature: "provisioner-ansible",
        default: true,
    },
];

/// Cargo features baked into the prebuilt `malboxctl` release binaries
/// (the release workflow builds with default features).
///
/// The install wizard compares the user's selection against this set to
/// decide whether the stock binary is equivalent to compiling, and upgrades
/// fall back to it for manifests that predate feature recording.
pub fn default_features() -> Vec<String> {
    DAEMON_FEATURES
        .iter()
        .filter(|f| f.default)
        .map(|f| f.feature.to_string())
        .collect()
}

/// Cargo feature for a provider name (`"libvirt"` -> `"provider-libvirt"`),
/// or `None` when no such provider feature exists.
pub fn provider_feature(name: &str) -> Option<&'static str> {
    DAEMON_FEATURES
        .iter()
        .map(|f| f.feature)
        .find(|feature| feature.strip_prefix("provider-") == Some(name))
}

/// Provider names recognized by the build, for error messages.
pub fn known_providers() -> impl Iterator<Item = &'static str> {
    DAEMON_FEATURES
        .iter()
        .filter_map(|f| f.feature.strip_prefix("provider-"))
}

/// Split a feature set into `(providers, provisioners)` by prefix. Providers
/// configure the daemon; provisioners only affect the build.
pub fn split_features(features: &[String]) -> (Vec<String>, Vec<String>) {
    (
        strip_prefixed(features, "provider-"),
        strip_prefixed(features, "provisioner-"),
    )
}

fn strip_prefixed(features: &[String], prefix: &str) -> Vec<String> {
    features
        .iter()
        .filter_map(|feature| feature.strip_prefix(prefix))
        .map(str::to_string)
        .collect()
}
