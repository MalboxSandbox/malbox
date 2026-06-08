use malbox_installer::github::{Release, ReleaseAsset};
use malbox_plugin_registry::resolve::{
    Platform, SpecifierSource, find_matching_asset, parse_specifier,
};

#[test]
fn parse_registry_name() {
    let spec = parse_specifier("yara-scanner").unwrap();
    assert_eq!(spec.name, "yara-scanner");
    assert!(spec.version.is_none());
    assert!(matches!(spec.source, SpecifierSource::Registry));
}

#[test]
fn parse_registry_name_with_version() {
    let spec = parse_specifier("yara-scanner@0.1.0").unwrap();
    assert_eq!(spec.name, "yara-scanner");
    assert_eq!(spec.version.as_deref(), Some("0.1.0"));
    assert!(matches!(spec.source, SpecifierSource::Registry));
}

#[test]
fn parse_direct_owner_repo() {
    let spec = parse_specifier("someuser/my-plugin").unwrap();
    assert_eq!(spec.name, "my-plugin");
    assert!(spec.version.is_none());
    match &spec.source {
        SpecifierSource::Direct { owner, repo } => {
            assert_eq!(owner, "someuser");
            assert_eq!(repo, "my-plugin");
        }
        _ => panic!("expected Direct source"),
    }
}

#[test]
fn parse_direct_with_version() {
    let spec = parse_specifier("someuser/my-plugin@2.0.0").unwrap();
    assert_eq!(spec.name, "my-plugin");
    assert_eq!(spec.version.as_deref(), Some("2.0.0"));
    match &spec.source {
        SpecifierSource::Direct { owner, repo } => {
            assert_eq!(owner, "someuser");
            assert_eq!(repo, "my-plugin");
        }
        _ => panic!("expected Direct source"),
    }
}

#[test]
fn parse_empty_is_error() {
    assert!(parse_specifier("").is_err());
}

#[test]
fn current_platform_is_valid() {
    let p = Platform::current();
    assert!(!p.arch.is_empty());
    assert!(!p.os.is_empty());
}

#[test]
fn platform_asset_suffix() {
    let p = Platform {
        arch: "x86_64".into(),
        os: "linux".into(),
    };
    assert_eq!(p.asset_suffix(), "x86_64-linux");
}

fn sample_release() -> Release {
    Release {
        tag_name: "v0.1.0".to_string(),
        name: Some("v0.1.0".to_string()),
        body: None,
        assets: vec![
            ReleaseAsset {
                name: "yara-scanner-v0.1.0-x86_64-linux.tar.gz".to_string(),
                browser_download_url: "https://example.com/yara-scanner-v0.1.0-x86_64-linux.tar.gz"
                    .to_string(),
                size: 5_000_000,
            },
            ReleaseAsset {
                name: "yara-scanner-v0.1.0-x86_64-linux.tar.gz.sha256".to_string(),
                browser_download_url:
                    "https://example.com/yara-scanner-v0.1.0-x86_64-linux.tar.gz.sha256".to_string(),
                size: 64,
            },
            ReleaseAsset {
                name: "yara-scanner-v0.1.0-aarch64-linux.tar.gz".to_string(),
                browser_download_url:
                    "https://example.com/yara-scanner-v0.1.0-aarch64-linux.tar.gz".to_string(),
                size: 4_800_000,
            },
        ],
    }
}

#[test]
fn find_asset_matching_platform() {
    let release = sample_release();
    let platform = Platform {
        arch: "x86_64".into(),
        os: "linux".into(),
    };
    let asset = find_matching_asset(&release, "yara-scanner", &platform);
    assert!(asset.is_some());
    assert_eq!(
        asset.unwrap().name,
        "yara-scanner-v0.1.0-x86_64-linux.tar.gz"
    );
}

#[test]
fn find_asset_wrong_platform() {
    let release = sample_release();
    let platform = Platform {
        arch: "riscv64".into(),
        os: "linux".into(),
    };
    let asset = find_matching_asset(&release, "yara-scanner", &platform);
    assert!(asset.is_none());
}

#[test]
fn find_asset_skips_checksum_files() {
    let release = sample_release();
    let platform = Platform {
        arch: "x86_64".into(),
        os: "linux".into(),
    };
    let asset = find_matching_asset(&release, "yara-scanner", &platform).unwrap();
    assert!(!asset.name.ends_with(".sha256"));
}

use malbox_plugin_registry::resolve::RequestedStrategy;

#[test]
fn requested_strategy_default_is_prebuilt_with_fallback() {
    let strategy = RequestedStrategy::default();
    assert!(matches!(strategy, RequestedStrategy::PrebuiltWithFallback));
}
