use malbox_installer::Channel;
use malbox_installer::config::{DaemonSource, FrontendSource, InstallConfig, PostgresStrategy};
use malbox_installer::install::build_initial_manifest;

#[test]
fn install_config_produces_correct_manifest() {
    let config = InstallConfig {
        providers: vec!["libvirt".to_string()],
        provisioners: vec!["ansible".to_string()],
        features: vec![
            "provider-libvirt".to_string(),
            "provisioner-ansible".to_string(),
        ],
        channel: Channel::Nightly,
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
        "linux-x64",
        "/home/user/.local/bin/malboxctl",
        "/home/user/.local/bin/malbox",
        "/home/user/.local/share/malbox/web",
    );

    assert_eq!(manifest.version, "0.1.0");
    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.channel, Channel::Nightly);
    assert_eq!(manifest.daemon.source, "prebuilt");
    assert_eq!(manifest.daemon.providers, vec!["libvirt"]);
    assert_eq!(manifest.daemon.provisioners, vec!["ansible"]);
    assert_eq!(
        manifest.daemon.features,
        vec!["provider-libvirt", "provisioner-ansible"]
    );
    assert_eq!(
        manifest.daemon.path,
        std::path::PathBuf::from("/home/user/.local/bin/malboxctl")
    );
    assert_eq!(manifest.frontend.source, "prebuilt");
    assert_eq!(manifest.postgres.strategy, "existing");
    assert!(!manifest.systemd.enabled);
    assert_eq!(
        manifest.cli.path,
        std::path::PathBuf::from("/home/user/.local/bin/malbox")
    );
}

#[test]
fn install_config_compiled_recorded() {
    let config = InstallConfig {
        providers: vec![],
        provisioners: vec![],
        features: vec!["provider-libvirt".to_string()],
        channel: Channel::Stable,
        daemon: DaemonSource::Compile,
        frontend: FrontendSource::Compile,
        postgres: PostgresStrategy::Setup,
        systemd: true,
    };

    let manifest = build_initial_manifest(
        &config,
        "0.1.0",
        "linux-x64",
        "/home/user/.local/bin/malboxctl",
        "/home/user/.local/bin/malbox",
        "/home/user/.local/share/malbox/web",
    );

    assert_eq!(manifest.channel, Channel::Stable);
    assert_eq!(manifest.daemon.source, "compiled");
    assert_eq!(manifest.daemon.features, vec!["provider-libvirt"]);
    assert_eq!(manifest.frontend.source, "compiled");
    assert_eq!(manifest.postgres.strategy, "setup");
    assert!(manifest.systemd.enabled);
}
