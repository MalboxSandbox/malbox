use malbox_installer::github::{Release, ReleaseAsset};

#[test]
fn parse_release_response() {
    let json = serde_json::json!({
        "tag_name": "v0.2.0",
        "name": "v0.2.0",
        "body": "## Changelog\n- Added feature X",
        "assets": [
            {
                "name": "malbox-daemon-x86_64-unknown-linux-gnu-ansible-libvirt.tar.gz",
                "browser_download_url": "https://github.com/org/malbox/releases/download/v0.2.0/malbox-daemon-x86_64-unknown-linux-gnu-ansible-libvirt.tar.gz",
                "size": 15000000
            },
            {
                "name": "malbox-daemon-x86_64-unknown-linux-gnu-ansible-libvirt.tar.gz.sha256",
                "browser_download_url": "https://github.com/org/malbox/releases/download/v0.2.0/malbox-daemon-x86_64-unknown-linux-gnu-ansible-libvirt.tar.gz.sha256",
                "size": 64
            },
            {
                "name": "malbox-frontend.tar.gz",
                "browser_download_url": "https://github.com/org/malbox/releases/download/v0.2.0/malbox-frontend.tar.gz",
                "size": 5000000
            },
            {
                "name": "source.tar.gz",
                "browser_download_url": "https://github.com/org/malbox/releases/download/v0.2.0/source.tar.gz",
                "size": 2000000
            }
        ]
    });

    let release: Release = serde_json::from_value(json).unwrap();
    assert_eq!(release.tag_name, "v0.2.0");
    assert_eq!(release.assets.len(), 4);
}

#[test]
fn find_daemon_asset_exact_match() {
    let release = Release {
        tag_name: "v0.2.0".to_string(),
        name: Some("v0.2.0".to_string()),
        body: None,
        assets: vec![
            ReleaseAsset {
                name: "malbox-daemon-x86_64-unknown-linux-gnu-ansible-libvirt.tar.gz".to_string(),
                browser_download_url: "https://example.com/daemon.tar.gz".to_string(),
                size: 15_000_000,
            },
            ReleaseAsset {
                name: "malbox-frontend.tar.gz".to_string(),
                browser_download_url: "https://example.com/frontend.tar.gz".to_string(),
                size: 5_000_000,
            },
        ],
    };

    let asset = release.find_daemon_asset("x86_64-unknown-linux-gnu", &["libvirt", "ansible"]);
    assert!(asset.is_some());
    assert!(asset.unwrap().name.contains("ansible-libvirt"));
}

#[test]
fn find_daemon_asset_no_match() {
    let release = Release {
        tag_name: "v0.2.0".to_string(),
        name: Some("v0.2.0".to_string()),
        body: None,
        assets: vec![ReleaseAsset {
            name: "malbox-daemon-x86_64-unknown-linux-gnu-ansible-libvirt.tar.gz".to_string(),
            browser_download_url: "https://example.com/daemon.tar.gz".to_string(),
            size: 15_000_000,
        }],
    };

    let asset = release.find_daemon_asset("x86_64-unknown-linux-gnu", &["vmware"]);
    assert!(asset.is_none());
}

#[test]
fn find_frontend_asset() {
    let release = Release {
        tag_name: "v0.2.0".to_string(),
        name: Some("v0.2.0".to_string()),
        body: None,
        assets: vec![
            ReleaseAsset {
                name: "malbox-daemon-x86_64.tar.gz".to_string(),
                browser_download_url: "https://example.com/daemon.tar.gz".to_string(),
                size: 15_000_000,
            },
            ReleaseAsset {
                name: "malbox-frontend.tar.gz".to_string(),
                browser_download_url: "https://example.com/frontend.tar.gz".to_string(),
                size: 5_000_000,
            },
        ],
    };

    let asset = release.find_frontend_asset();
    assert!(asset.is_some());
    assert_eq!(asset.unwrap().name, "malbox-frontend.tar.gz");
}

#[test]
fn find_source_asset() {
    let release = Release {
        tag_name: "v0.2.0".to_string(),
        name: Some("v0.2.0".to_string()),
        body: None,
        assets: vec![ReleaseAsset {
            name: "source.tar.gz".to_string(),
            browser_download_url: "https://example.com/source.tar.gz".to_string(),
            size: 2_000_000,
        }],
    };

    let asset = release.find_source_asset();
    assert!(asset.is_some());
}

#[test]
fn extract_version_from_tag() {
    assert_eq!(Release::version_from_tag("v0.2.0"), "0.2.0");
    assert_eq!(Release::version_from_tag("0.2.0"), "0.2.0");
}
