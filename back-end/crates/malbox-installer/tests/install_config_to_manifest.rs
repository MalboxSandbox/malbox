use malbox_installer::config::{
    DaemonSource, FrontendSource, InstallConfig, NixStrategy, PostgresStrategy,
};
use malbox_installer::install::build_initial_manifest;

#[test]
fn install_config_produces_correct_manifest() {
    let config = InstallConfig {
        nix: NixStrategy::Skip,
        providers: vec!["libvirt".to_string(), "ansible".to_string()],
        daemon: DaemonSource::Prebuilt {
            url: "https://example.com/daemon.tar.gz".to_string(),
        },
        frontend: FrontendSource::Prebuilt {
            url: "https://example.com/frontend.tar.gz".to_string(),
        },
        postgres: PostgresStrategy::Existing {
            url: "postgres://localhost/malbox_db".to_string(),
        },
        systemd: false,
    };

    let manifest = build_initial_manifest(
        &config,
        "0.1.0",
        "x86_64-unknown-linux-gnu",
        "/home/user/.local/bin/malboxd",
        "/home/user/.local/share/malbox/web",
    );

    assert_eq!(manifest.version, "0.1.0");
    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.nix, "skipped");
    assert_eq!(manifest.daemon.source, "prebuilt");
    assert_eq!(manifest.daemon.providers, vec!["libvirt", "ansible"]);
    assert_eq!(manifest.frontend.source, "prebuilt");
    assert_eq!(manifest.postgres.strategy, "existing");
    assert!(!manifest.systemd.enabled);
}

#[test]
fn install_config_nix_installed_recorded() {
    let config = InstallConfig {
        nix: NixStrategy::Install,
        providers: vec![],
        daemon: DaemonSource::Compile {
            features: vec!["provider-libvirt".to_string()],
        },
        frontend: FrontendSource::Compile,
        postgres: PostgresStrategy::Setup,
        systemd: true,
    };

    let manifest = build_initial_manifest(
        &config,
        "0.1.0",
        "x86_64-unknown-linux-gnu",
        "/home/user/.local/bin/malboxd",
        "/home/user/.local/share/malbox/web",
    );

    assert_eq!(manifest.nix, "installed");
    assert_eq!(manifest.daemon.source, "compiled");
    assert_eq!(manifest.frontend.source, "compiled");
    assert_eq!(manifest.postgres.strategy, "setup");
    assert!(manifest.systemd.enabled);
}
