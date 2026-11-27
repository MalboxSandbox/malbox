use std::error::Error;

use async_trait::async_trait;
use malbox_machinery::provider::TomlValue;
use malbox_machinery::{GuestAccess, GuestSession, Machine};

use crate::session::ContainerSession;
use crate::{ContainerError, ContainerProvider, container_name};

const TRANSPORT: &str = "docker-exec";

/// Transport config for `docker-exec`.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ExecTransportConfig {
    /// Base directory for relative guest paths.
    #[serde(default = "default_sample_dir")]
    sample_dir: String,
}

fn default_sample_dir() -> String {
    "/opt/malbox/samples".to_string()
}

impl ContainerProvider {
    async fn session(
        &self,
        transport: &str,
        machine: &Machine,
        config: &TomlValue,
    ) -> Result<ContainerSession, ContainerError> {
        if transport != TRANSPORT {
            return Err(ContainerError::Unsupported(format!(
                "transport '{transport}' (supported: {TRANSPORT})"
            )));
        }
        let config: ExecTransportConfig =
            malbox_machinery::provider::config::deserialize(config)
                .map_err(|e| ContainerError::Config(format!("invalid {TRANSPORT} config: {e}")))?;

        Ok(ContainerSession {
            docker: self.docker().await?.clone(),
            container: container_name(machine.id()),
            sample_dir: config.sample_dir,
        })
    }
}

#[async_trait]
impl GuestAccess for ContainerProvider {
    fn supported_transports(&self) -> Vec<&str> {
        vec![TRANSPORT]
    }

    async fn open_session(
        &self,
        transport: &str,
        machine: &Machine,
        config: &TomlValue,
    ) -> Result<Box<dyn GuestSession>, Box<dyn Error + Send + Sync>> {
        Ok(Box::new(self.session(transport, machine, config).await?))
    }
}
