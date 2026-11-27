//! Container specification: the single source for (re)building a container.

use std::collections::HashMap;
use std::net::Ipv4Addr;

use bollard::models::{
    ContainerCreateBody, EndpointIpamConfig, EndpointSettings, HostConfig, NetworkingConfig,
};
use serde::{Deserialize, Serialize};

use crate::{ContainerError, LABEL_MACHINE, LABEL_MANAGED, LABEL_SPEC};

/// Default container command. Guest plugins start from executable scripts in
/// `/opt/malbox/autostart/`; the container then idles.
const DEFAULT_COMMAND: &str =
    r#"for f in /opt/malbox/autostart/*; do [ -x "$f" ] && "$f" & done; exec tail -f /dev/null"#;

/// Everything needed to build a machine's container, stored as JSON in the
/// `malbox.spec` label so restores recreate an identical container.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ContainerSpec {
    pub(crate) name: String,
    pub(crate) ip: Ipv4Addr,
    pub(crate) cpus: u32,
    pub(crate) memory_mb: u32,
    pub(crate) runtime: Option<String>,
    pub(crate) cap_add: Vec<String>,
    pub(crate) command: Option<Vec<String>>,
}

/// Engine create body for `spec` running `image` on `network`.
pub(crate) fn create_body(
    spec: &ContainerSpec,
    image: &str,
    network: &str,
) -> Result<ContainerCreateBody, ContainerError> {
    let labels = HashMap::from([
        (LABEL_MANAGED.to_string(), "true".to_string()),
        (LABEL_MACHINE.to_string(), spec.name.clone()),
        (LABEL_SPEC.to_string(), serde_json::to_string(spec)?),
    ]);

    let cmd = spec.command.clone().unwrap_or_else(|| {
        vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            DEFAULT_COMMAND.to_string(),
        ]
    });

    Ok(ContainerCreateBody {
        image: Some(image.to_string()),
        hostname: Some(spec.name.clone()),
        // Reset any image ENTRYPOINT so `cmd` is the full argv.
        entrypoint: Some(vec![String::new()]),
        cmd: Some(cmd),
        labels: Some(labels),
        host_config: Some(HostConfig {
            nano_cpus: Some(i64::from(spec.cpus) * 1_000_000_000),
            memory: Some(i64::from(spec.memory_mb) * 1024 * 1024),
            init: Some(true),
            network_mode: Some(network.to_string()),
            runtime: spec.runtime.clone(),
            cap_add: (!spec.cap_add.is_empty()).then(|| spec.cap_add.clone()),
            ..Default::default()
        }),
        networking_config: Some(NetworkingConfig {
            endpoints_config: Some(HashMap::from([(
                network.to_string(),
                EndpointSettings {
                    ipam_config: Some(EndpointIpamConfig {
                        ipv4_address: Some(spec.ip.to_string()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )])),
        }),
        ..Default::default()
    })
}

/// Read the spec from a container's labels.
pub(crate) fn read_spec(labels: &HashMap<String, String>) -> Result<ContainerSpec, ContainerError> {
    let raw = labels.get(LABEL_SPEC).ok_or_else(|| {
        ContainerError::NotFound("container has no malbox.spec label".to_string())
    })?;
    Ok(serde_json::from_str(raw)?)
}
