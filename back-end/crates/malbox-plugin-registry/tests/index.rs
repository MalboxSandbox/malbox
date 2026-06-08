use malbox_plugin_registry::index::{PluginMetadata, RegistryIndex};

fn sample_index() -> RegistryIndex {
    serde_json::from_str(SAMPLE_INDEX_JSON).unwrap()
}

const SAMPLE_INDEX_JSON: &str = r#"{
    "schema_version": 1,
    "generated_at": "2026-06-07T12:00:00Z",
    "plugins": [
        {
            "name": "yara-scanner",
            "description": "Scans samples using signature-based YARA rules",
            "type": "guest",
            "categories": ["analysis", "signatures"],
            "repository": "malbox-plugins/yara-scanner"
        },
        {
            "name": "file-info",
            "description": "Extracts basic file metadata and hashes",
            "type": "host",
            "categories": ["analysis"],
            "repository": "malbox-plugins/file-info"
        },
        {
            "name": "yara-updater",
            "description": "Auto-updates YARA rule sets from threat feeds",
            "type": "host",
            "categories": ["maintenance", "signatures"],
            "repository": "malbox-plugins/yara-updater"
        }
    ]
}"#;

const SAMPLE_PLUGIN_JSON: &str = r#"{
    "schema_version": 1,
    "name": "yara-scanner",
    "description": "Scans samples inside the guest VM using signature-based YARA rules",
    "repository": "malbox-plugins/yara-scanner",
    "authors": ["Malbox Team"],
    "license": "MIT",
    "type": "guest",
    "categories": ["analysis", "signatures"],
    "keywords": ["yara", "signatures", "detection", "rules"],
    "min_malbox_version": "0.1.0",
    "homepage": "https://github.com/malbox-plugins/yara-scanner",
    "requires": [
        { "name": "file-info", "version": ">=0.1.0" }
    ]
}"#;

#[test]
fn parse_index() {
    let index = sample_index();
    assert_eq!(index.schema_version, 1);
    assert_eq!(index.plugins.len(), 3);
    assert_eq!(index.plugins[0].name, "yara-scanner");
    assert_eq!(index.plugins[0].repository, "malbox-plugins/yara-scanner");
}

#[test]
fn search_by_name() {
    let index = sample_index();
    let results = index.search("yara");
    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|e| e.name == "yara-scanner"));
    assert!(results.iter().any(|e| e.name == "yara-updater"));
}

#[test]
fn search_by_category() {
    let index = sample_index();
    let results = index.search("signatures");
    assert_eq!(results.len(), 2);
}

#[test]
fn search_no_match() {
    let index = sample_index();
    let results = index.search("nonexistent");
    assert!(results.is_empty());
}

#[test]
fn search_case_insensitive() {
    let index = sample_index();
    let results = index.search("YARA");
    assert_eq!(results.len(), 2);
}

#[test]
fn find_by_name_exact() {
    let index = sample_index();
    let entry = index.find("yara-scanner");
    assert!(entry.is_some());
    assert_eq!(entry.unwrap().repository, "malbox-plugins/yara-scanner");
}

#[test]
fn find_by_name_missing() {
    let index = sample_index();
    assert!(index.find("nonexistent").is_none());
}

#[test]
fn parse_plugin_metadata() {
    let meta: PluginMetadata = serde_json::from_str(SAMPLE_PLUGIN_JSON).unwrap();
    assert_eq!(meta.name, "yara-scanner");
    assert_eq!(meta.authors, vec!["Malbox Team"]);
    assert_eq!(meta.license, Some("MIT".to_string()));
    assert_eq!(meta.requires.len(), 1);
    assert_eq!(meta.requires[0].name, "file-info");
    assert_eq!(meta.requires[0].version, ">=0.1.0");
}

#[test]
fn parse_plugin_metadata_minimal() {
    let json = r#"{
        "schema_version": 1,
        "name": "simple-plugin",
        "description": "A simple plugin",
        "repository": "user/simple-plugin",
        "type": "host",
        "categories": []
    }"#;
    let meta: PluginMetadata = serde_json::from_str(json).unwrap();
    assert_eq!(meta.name, "simple-plugin");
    assert!(meta.authors.is_empty());
    assert!(meta.license.is_none());
    assert!(meta.requires.is_empty());
}
