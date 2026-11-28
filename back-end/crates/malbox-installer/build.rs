//! Generates `DAEMON_FEATURES` from the `provider-*` and `provisioner-*` features of
//! `bin/malboxd/Cargo.toml`, so installer prompts stay in sync with Cargo.

use std::env;
use std::fmt::Write;
use std::fs;
use std::path::Path;

/// One generated `DaemonFeature` table row.
struct Entry {
    display: String,
    feature: String,
    default: bool,
    kind: &'static str,
    sort_key: u8,
}

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let malboxd_toml = Path::new(&manifest_dir).join("../../bin/malboxd/Cargo.toml");

    println!("cargo:rerun-if-changed={}", malboxd_toml.display());

    let content = fs::read_to_string(&malboxd_toml)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", malboxd_toml.display()));
    let parsed: toml::Table = toml::from_str(&content).expect("malboxd Cargo.toml is valid TOML");

    let features = parsed
        .get("features")
        .and_then(|v| v.as_table())
        .expect("[features] table in malboxd Cargo.toml");

    let defaults: Vec<&str> = features
        .get("default")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    let mut entries: Vec<Entry> = features
        .keys()
        .filter(|k| *k != "default")
        .filter_map(|key| {
            if let Some(name) = key.strip_prefix("provider-") {
                Some(Entry {
                    display: name.to_string(),
                    feature: key.clone(),
                    default: defaults.contains(&key.as_str()),
                    kind: "FeatureKind::Provider",
                    sort_key: 0,
                })
            } else {
                key.strip_prefix("provisioner-").map(|name| Entry {
                    display: name.to_string(),
                    feature: key.clone(),
                    default: defaults.contains(&key.as_str()),
                    kind: "FeatureKind::Provisioner",
                    sort_key: 1,
                })
            }
        })
        .collect();

    entries.sort_by(|a, b| a.sort_key.cmp(&b.sort_key).then(a.feature.cmp(&b.feature)));

    let mut generated = String::from(
        "/// Every user-selectable daemon feature, providers first, each group sorted by name.\npub const DAEMON_FEATURES: &[DaemonFeature] = &[\n",
    );
    for entry in &entries {
        write!(
            generated,
            r#"    DaemonFeature {{
        display: "{display}",
        feature: "{feature}",
        default: {default},
        kind: {kind},
    }},
"#,
            display = entry.display,
            feature = entry.feature,
            default = entry.default,
            kind = entry.kind,
        )
        .expect("writing to a String cannot fail");
    }
    generated.push_str("];\n");

    let out_dir = env::var("OUT_DIR").expect("cargo sets OUT_DIR for build scripts");
    fs::write(Path::new(&out_dir).join("daemon_features.rs"), generated)
        .expect("failed to write daemon_features.rs");
}
