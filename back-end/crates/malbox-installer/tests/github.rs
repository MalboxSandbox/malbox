use malbox_installer::github::{Release, ReleaseAsset};

fn sample_release() -> Release {
    Release {
        tag_name: "v0.2.0".to_string(),
        name: Some("v0.2.0".to_string()),
        body: Some("## Changelog\n- Added feature X".to_string()),
        assets: vec![
            ReleaseAsset {
                name: "malboxctl-v0.2.0-linux-x64.tar.gz".to_string(),
                browser_download_url: "https://github.com/malboxapp/malbox/releases/download/v0.2.0/malboxctl-v0.2.0-linux-x64.tar.gz".to_string(),
                size: 15_000_000,
            },
            ReleaseAsset {
                name: "malboxctl-v0.2.0-linux-x64.tar.gz.sha256".to_string(),
                browser_download_url: "https://github.com/malboxapp/malbox/releases/download/v0.2.0/malboxctl-v0.2.0-linux-x64.tar.gz.sha256".to_string(),
                size: 64,
            },
            ReleaseAsset {
                name: "malbox-v0.2.0-linux-x64.tar.gz".to_string(),
                browser_download_url: "https://github.com/malboxapp/malbox/releases/download/v0.2.0/malbox-v0.2.0-linux-x64.tar.gz".to_string(),
                size: 10_000_000,
            },
            ReleaseAsset {
                name: "malbox-plugin-sdk-cpp-v0.2.0-linux-x64.tar.gz".to_string(),
                browser_download_url: "https://github.com/malboxapp/malbox/releases/download/v0.2.0/malbox-plugin-sdk-cpp-v0.2.0-linux-x64.tar.gz".to_string(),
                size: 5_000_000,
            },
        ],
    }
}

#[test]
fn parse_release_response() {
    let json = serde_json::json!({
        "tag_name": "v0.2.0",
        "name": "v0.2.0",
        "body": "## Changelog\n- Added feature X",
        "assets": [
            {
                "name": "malboxctl-v0.2.0-linux-x64.tar.gz",
                "browser_download_url": "https://github.com/malboxapp/malbox/releases/download/v0.2.0/malboxctl-v0.2.0-linux-x64.tar.gz",
                "size": 15000000
            },
            {
                "name": "malbox-v0.2.0-linux-x64.tar.gz",
                "browser_download_url": "https://github.com/malboxapp/malbox/releases/download/v0.2.0/malbox-v0.2.0-linux-x64.tar.gz",
                "size": 10000000
            }
        ]
    });

    let release: Release = serde_json::from_value(json).unwrap();
    assert_eq!(release.tag_name, "v0.2.0");
    assert_eq!(release.assets.len(), 2);
}

#[test]
fn find_malboxctl_asset() {
    let release = sample_release();
    let asset = release.find_malboxctl_asset("linux-x64");
    assert!(asset.is_some());
    assert!(asset.unwrap().name.starts_with("malboxctl-"));
}

#[test]
fn find_malboxctl_asset_wrong_arch() {
    let release = sample_release();
    let asset = release.find_malboxctl_asset("linux-arm64");
    assert!(asset.is_none());
}

#[test]
fn find_malbox_asset() {
    let release = sample_release();
    let asset = release.find_malbox_asset("linux-x64");
    assert!(asset.is_some());
    assert_eq!(asset.unwrap().name, "malbox-v0.2.0-linux-x64.tar.gz");
}

#[test]
fn source_archive_url() {
    let release = sample_release();
    let url = release.source_archive_url("malboxapp", "malbox");
    assert_eq!(
        url,
        "https://github.com/malboxapp/malbox/archive/refs/tags/v0.2.0.tar.gz"
    );
}

#[test]
fn extract_version_from_tag() {
    assert_eq!(Release::version_from_tag("v0.2.0"), "0.2.0");
    assert_eq!(Release::version_from_tag("0.2.0"), "0.2.0");
}
