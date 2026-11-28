//! Live lifecycle test against a rootful Docker engine.
//!
//! Run with `cargo test -p malbox-provider-container -- --ignored`.
//! `MALBOX_TEST_SOCKET` overrides the engine socket (e.g.
//! `unix:///run/podman/podman.sock`); the `docker` CLI must target the same engine.

use std::path::Path;
use std::process::Command;

use malbox_machinery::provider::TomlValue;
use malbox_machinery::{
    Allocate, Arch, CreateMachineParams, ExecOptions, GuestAccess, Machine, MachineId, Platform,
    Snapshot, StopMode,
};
use malbox_provider_container::ContainerProvider;

const NETWORK: &str = "malbox-test";

fn docker(args: &[&str]) -> String {
    let output = Command::new("docker")
        .args(args)
        .output()
        .expect("docker CLI available");
    assert!(
        output.status.success(),
        "docker {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Removes the test container, snapshots and network even if an assertion fails.
struct Cleanup {
    name: String,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = Command::new("docker")
            .args(["rm", "-f", &format!("malbox-{}", self.name)])
            .output();
        if let Ok(out) = Command::new("docker")
            .args([
                "images",
                "--format",
                "{{.Repository}}:{{.Tag}}",
                "--filter",
                &format!("label=malbox.machine={}", self.name),
            ])
            .output()
        {
            for tag in String::from_utf8_lossy(&out.stdout).lines() {
                let _ = Command::new("docker").args(["rmi", "-f", tag]).output();
            }
        }
        let _ = Command::new("docker")
            .args(["network", "rm", NETWORK])
            .output();
    }
}

#[tokio::test]
#[ignore = "requires rootful Docker at /var/run/docker.sock"]
async fn container_lifecycle() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("alpine.tar");
    docker(&["pull", "alpine:3.20"]);
    docker(&["save", "-o", archive.to_str().unwrap(), "alpine:3.20"]);

    let name = format!("lifecycle-{}", std::process::id());
    let _cleanup = Cleanup { name: name.clone() };

    let mut config = TomlValue::from(std::collections::BTreeMap::<String, TomlValue>::new());
    let table = config.as_table_mut().unwrap();
    table.insert("network".into(), TomlValue::String(NETWORK.into()));
    table.insert("subnet".into(), TomlValue::String("172.31.250.0/24".into()));
    if let Ok(socket) = std::env::var("MALBOX_TEST_SOCKET") {
        table.insert("socket".into(), TomlValue::String(socket));
    }
    let provider = ContainerProvider::new(&config).unwrap();

    let params = CreateMachineParams {
        name: name.clone(),
        platform: Platform::Linux,
        arch: Arch::X64,
        cpus: 1,
        memory_mb: 256,
        disk_size_mb: 1024,
        base_image: Some(archive.to_str().unwrap().to_string()),
        provider_config: None,
    };

    let machine = provider.create(&params).await.unwrap();
    let expected_ip: std::net::IpAddr = "172.31.250.2".parse().unwrap();
    assert_eq!(machine.endpoint().unwrap().address, expected_ip);

    provider.start(&machine).await.unwrap();
    provider.stop(&machine, StopMode::Force).await.unwrap();
    let base = provider.create_snapshot(&machine, "base").await.unwrap();

    let empty = TomlValue::from(std::collections::BTreeMap::<String, TomlValue>::new());
    let session = provider
        .open_session("docker-exec", &machine, &empty)
        .await
        .unwrap();

    provider.start(&machine).await.unwrap();
    let touched = session
        .execute("touch", &["/dirty".to_string()], ExecOptions::default())
        .await
        .unwrap();
    assert_eq!(touched.exit_code, Some(0));
    provider.stop(&machine, StopMode::Force).await.unwrap();

    provider.restore_snapshot(&machine, &base).await.unwrap();
    provider.start(&machine).await.unwrap();
    let probe = session
        .execute(
            "test",
            &["-e".to_string(), "/dirty".to_string()],
            ExecOptions::default(),
        )
        .await
        .unwrap();
    assert_eq!(probe.exit_code, Some(1), "restore must discard /dirty");
    let endpoint = provider.resolve_endpoint(&machine).await.unwrap().unwrap();
    assert_eq!(endpoint.address, expected_ip);

    let source = dir.path().join("sample.bin");
    let pulled = dir.path().join("pulled.bin");
    let payload: Vec<u8> = (0..=255u8).cycle().take(100_000).collect();
    std::fs::write(&source, &payload).unwrap();
    session.push_file(&source, "s.bin").await.unwrap();
    session
        .pull_file("/opt/malbox/samples/s.bin", &pulled)
        .await
        .unwrap();
    assert_eq!(std::fs::read(Path::new(&pulled)).unwrap(), payload);

    let snapshots = provider.list_snapshots(&machine).await.unwrap();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].name, "base");
    assert!(snapshots[0].is_current);

    session.close().await.unwrap();
    provider
        .destroy(&Machine::new(MachineId(name.clone())))
        .await
        .unwrap();

    let machines = provider.list().await.unwrap();
    assert!(machines.iter().all(|m| m.id.0 != name));
    let leftover = docker(&[
        "images",
        "-q",
        "--filter",
        &format!("label=malbox.machine={name}"),
    ]);
    assert!(
        leftover.trim().is_empty(),
        "snapshot images left: {leftover}"
    );
}
