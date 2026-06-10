use malbox_installer::manifest::{
    CliManifest, DaemonManifest, FrontendManifest, Manifest, PostgresManifest, SystemdManifest,
};
use malbox_installer::{Channel, Step};
use tempfile::TempDir;

fn sample_manifest() -> Manifest {
    Manifest {
        version: "0.1.0".to_string(),
        schema_version: 1,
        installed_at: "2026-05-05T14:30:00Z".to_string(),
        updated_at: "2026-05-05T14:30:00Z".to_string(),
        arch: "linux-x64".to_string(),
        channel: Channel::Nightly,
        daemon: DaemonManifest {
            source: "prebuilt".to_string(),
            version: "0.1.0".to_string(),
            commit: Some("891348d".to_string()),
            path: "/home/user/.local/bin/malboxd".into(),
            prev_path: None,
            prev_version: None,
            providers: vec!["libvirt".to_string()],
            provisioners: vec!["ansible".to_string()],
            features: vec![
                "provider-libvirt".to_string(),
                "provisioner-ansible".to_string(),
            ],
        },
        frontend: FrontendManifest {
            source: "prebuilt".to_string(),
            path: "/home/user/.local/share/malbox/web".into(),
            prev_path: None,
        },
        postgres: PostgresManifest {
            strategy: "existing".to_string(),
            url: "postgres://postgres@localhost:5433".to_string(),
        },
        systemd: SystemdManifest {
            enabled: false,
            unit: None,
        },
        cli: CliManifest {
            path: "/home/user/.local/bin/malbox".into(),
        },
        last_completed_step: None,
    }
}

#[test]
fn manifest_round_trip() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("manifest.json");

    let manifest = sample_manifest();
    manifest.save(&path).unwrap();
    let loaded = Manifest::load(&path).unwrap();

    assert_eq!(loaded.version, "0.1.0");
    assert_eq!(loaded.schema_version, 1);
    assert_eq!(loaded.channel, Channel::Nightly);
    assert_eq!(loaded.daemon.providers, vec!["libvirt"]);
    assert_eq!(loaded.daemon.provisioners, vec!["ansible"]);
    assert_eq!(
        loaded.daemon.features,
        vec!["provider-libvirt", "provisioner-ansible"]
    );
    assert!(!loaded.systemd.enabled);
    assert_eq!(
        loaded.cli.path,
        std::path::PathBuf::from("/home/user/.local/bin/malbox")
    );
}

#[test]
fn manifest_load_missing_file() {
    let result = Manifest::load("/tmp/nonexistent_malbox_manifest.json".as_ref());
    assert!(result.is_err());
}

/// Manifests written before channel/feature recording must still load, with
/// the channel defaulting to nightly (the historical upgrade behavior).
#[test]
fn manifest_pre_channel_schema_loads() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("manifest.json");

    sample_manifest().save(&path).unwrap();
    let mut value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    value.as_object_mut().unwrap().remove("channel");
    let daemon = value["daemon"].as_object_mut().unwrap();
    daemon.remove("provisioners");
    daemon.remove("features");
    std::fs::write(&path, serde_json::to_string(&value).unwrap()).unwrap();

    let loaded = Manifest::load(&path).unwrap();
    assert_eq!(loaded.channel, Channel::Nightly);
    assert!(loaded.daemon.provisioners.is_empty());
    assert!(loaded.daemon.features.is_empty());
}

#[test]
fn manifest_update_for_upgrade() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("manifest.json");

    let mut manifest = sample_manifest();
    manifest.record_upgrade("0.2.0", Some("abc1234"));
    manifest.save(&path).unwrap();

    let loaded = Manifest::load(&path).unwrap();
    assert_eq!(loaded.version, "0.2.0");
    assert_eq!(loaded.daemon.version, "0.2.0");
    assert_eq!(loaded.daemon.commit.as_deref(), Some("abc1234"));
    assert_ne!(loaded.updated_at, "2026-05-05T14:30:00Z");
}

#[test]
fn manifest_resumability_tracking() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("manifest.json");

    let mut manifest = sample_manifest();

    manifest.mark_step_completed(Step::Daemon);
    assert_eq!(manifest.last_completed_step, Some(Step::Daemon));

    manifest.mark_step_completed(Step::Frontend);
    assert_eq!(manifest.last_completed_step, Some(Step::Frontend));

    manifest.mark_complete();
    assert_eq!(manifest.last_completed_step, None);

    manifest.save(&path).unwrap();
    let loaded = Manifest::load(&path).unwrap();
    assert!(loaded.last_completed_step.is_none());
}
