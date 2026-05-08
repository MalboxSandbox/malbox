use malbox_installer::Step;
use malbox_installer::manifest::Manifest;
use tempfile::TempDir;

#[test]
fn manifest_round_trip() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("manifest.json");

    let manifest = Manifest {
        version: "0.1.0".to_string(),
        schema_version: 1,
        installed_at: "2026-05-05T14:30:00Z".to_string(),
        updated_at: "2026-05-05T14:30:00Z".to_string(),
        arch: "x86_64-unknown-linux-gnu".to_string(),
        nix: "skipped".to_string(),
        daemon: malbox_installer::manifest::DaemonManifest {
            source: "prebuilt".to_string(),
            version: "0.1.0".to_string(),
            commit: Some("891348d".to_string()),
            path: "/home/user/.local/bin/malboxd".into(),
            prev_path: None,
            providers: vec!["libvirt".to_string()],
        },
        frontend: malbox_installer::manifest::FrontendManifest {
            source: "prebuilt".to_string(),
            path: "/home/user/.local/share/malbox/web".into(),
        },
        postgres: malbox_installer::manifest::PostgresManifest {
            strategy: "existing".to_string(),
            url: "postgres://postgres@localhost:5433/malbox_db".to_string(),
        },
        systemd: malbox_installer::manifest::SystemdManifest {
            enabled: false,
            unit: None,
        },
        last_completed_step: None,
    };

    manifest.save(&path).unwrap();
    let loaded = Manifest::load(&path).unwrap();

    assert_eq!(loaded.version, "0.1.0");
    assert_eq!(loaded.schema_version, 1);
    assert_eq!(loaded.daemon.providers, vec!["libvirt"]);
    assert!(!loaded.systemd.enabled);
}

#[test]
fn manifest_load_missing_file() {
    let result = Manifest::load("/tmp/nonexistent_malbox_manifest.json".as_ref());
    assert!(result.is_err());
}

#[test]
fn manifest_update_for_upgrade() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("manifest.json");

    let mut manifest = Manifest {
        version: "0.1.0".to_string(),
        schema_version: 1,
        installed_at: "2026-05-05T14:30:00Z".to_string(),
        updated_at: "2026-05-05T14:30:00Z".to_string(),
        arch: "x86_64-unknown-linux-gnu".to_string(),
        nix: "skipped".to_string(),
        daemon: malbox_installer::manifest::DaemonManifest {
            source: "prebuilt".to_string(),
            version: "0.1.0".to_string(),
            commit: Some("891348d".to_string()),
            path: "/home/user/.local/bin/malboxd".into(),
            prev_path: None,
            providers: vec!["libvirt".to_string()],
        },
        frontend: malbox_installer::manifest::FrontendManifest {
            source: "prebuilt".to_string(),
            path: "/home/user/.local/share/malbox/web".into(),
        },
        postgres: malbox_installer::manifest::PostgresManifest {
            strategy: "existing".to_string(),
            url: "postgres://postgres@localhost:5433/malbox_db".to_string(),
        },
        systemd: malbox_installer::manifest::SystemdManifest {
            enabled: false,
            unit: None,
        },
        last_completed_step: None,
    };

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

    let mut manifest = Manifest {
        version: "0.1.0".to_string(),
        schema_version: 1,
        installed_at: "2026-05-05T14:30:00Z".to_string(),
        updated_at: "2026-05-05T14:30:00Z".to_string(),
        arch: "x86_64-unknown-linux-gnu".to_string(),
        nix: "skipped".to_string(),
        daemon: malbox_installer::manifest::DaemonManifest {
            source: "prebuilt".to_string(),
            version: "0.1.0".to_string(),
            commit: None,
            path: "/home/user/.local/bin/malboxd".into(),
            prev_path: None,
            providers: vec![],
        },
        frontend: malbox_installer::manifest::FrontendManifest {
            source: "prebuilt".to_string(),
            path: "/home/user/.local/share/malbox/web".into(),
        },
        postgres: malbox_installer::manifest::PostgresManifest {
            strategy: "setup".to_string(),
            url: "postgres://postgres@localhost:5433/malbox_db".to_string(),
        },
        systemd: malbox_installer::manifest::SystemdManifest {
            enabled: false,
            unit: None,
        },
        last_completed_step: None,
    };

    manifest.mark_step_completed(Step::Nix);
    assert_eq!(manifest.last_completed_step, Some(Step::Nix));

    manifest.mark_step_completed(Step::Daemon);
    assert_eq!(manifest.last_completed_step, Some(Step::Daemon));

    manifest.mark_complete();
    assert_eq!(manifest.last_completed_step, None);

    manifest.save(&path).unwrap();
    let loaded = Manifest::load(&path).unwrap();
    assert!(loaded.last_completed_step.is_none());
}
