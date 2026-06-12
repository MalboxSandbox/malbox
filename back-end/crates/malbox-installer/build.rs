use std::env;
use std::fmt::Write;
use std::fs;
use std::path::Path;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
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

    struct Entry {
        display: String,
        feature: String,
        default: bool,
        kind: &'static str,
        sort_key: u8,
    }

    let mut entries: Vec<Entry> = features
        .keys()
        .filter(|k| *k != "default")
        .filter_map(|key| {
            if let Some(name) = key.strip_prefix("provider-") {
                Some(Entry {
                    display: name.to_string(),
                    feature: key.to_string(),
                    default: defaults.contains(&key.as_str()),
                    kind: "FeatureKind::Provider",
                    sort_key: 0,
                })
            } else {
                key.strip_prefix("provisioner-").map(|name| Entry {
                    display: name.to_string(),
                    feature: key.to_string(),
                    default: defaults.contains(&key.as_str()),
                    kind: "FeatureKind::Provisioner",
                    sort_key: 1,
                })
            }
        })
        .collect();

    entries.sort_by(|a, b| a.sort_key.cmp(&b.sort_key).then(a.feature.cmp(&b.feature)));

    let mut generated = String::from("pub const DAEMON_FEATURES: &[DaemonFeature] = &[\n");
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
        .unwrap();
    }
    generated.push_str("];\n");

    let out_dir = env::var("OUT_DIR").unwrap();
    fs::write(Path::new(&out_dir).join("daemon_features.rs"), generated)
        .expect("failed to write daemon_features.rs");
}
