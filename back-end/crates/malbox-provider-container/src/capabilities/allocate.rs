use std::collections::HashSet;
use std::error::Error;
use std::net::Ipv4Addr;

use async_trait::async_trait;
use bollard::models::ContainerSummaryStateEnum;
use bollard::query_parameters::{
    CreateContainerOptions, ListContainersOptions, RemoveContainerOptions, RemoveImageOptions,
    StopContainerOptions,
};
use malbox_machinery::{
    Allocate, CreateMachineParams, Machine, MachineEndpoint, MachineId, MachineState,
    ManagementAccess, Platform, StopMode,
};
use tracing::{debug, info, warn};

use crate::error::tolerate;
use crate::network::pick_ip;
use crate::spec::{ContainerSpec, create_body, read_spec};
use crate::{
    ContainerError, ContainerOverrides, ContainerProvider, LABEL_MACHINE, container_name,
    managed_filter, validate_name,
};

/// Runtime machine handle with its endpoint set.
fn machine_handle(id: &str, ip: Ipv4Addr) -> Machine {
    let mut machine = Machine::new(MachineId(id.to_string()));
    machine.set_endpoint(Some(MachineEndpoint {
        address: ip.into(),
        id: id.to_string(),
        platform: Platform::Linux,
    }));
    machine
}

fn map_state(state: Option<ContainerSummaryStateEnum>) -> MachineState {
    match state {
        Some(ContainerSummaryStateEnum::RUNNING) => MachineState::Running,
        Some(ContainerSummaryStateEnum::PAUSED) => MachineState::Suspended,
        Some(ContainerSummaryStateEnum::RESTARTING) => MachineState::Starting,
        Some(ContainerSummaryStateEnum::REMOVING | ContainerSummaryStateEnum::STOPPING) => {
            MachineState::Stopping
        }
        Some(ContainerSummaryStateEnum::DEAD) => MachineState::Failed,
        _ => MachineState::Stopped,
    }
}

impl ContainerProvider {
    async fn create_machine(
        &self,
        params: &CreateMachineParams,
    ) -> Result<Machine, ContainerError> {
        if params.platform != Platform::Linux {
            return Err(ContainerError::Unsupported(
                "container provider only supports Linux machines".to_string(),
            ));
        }
        validate_name(&params.name)?;

        let overrides: ContainerOverrides = match &params.provider_config {
            Some(value) => malbox_machinery::provider::config::deserialize(value).map_err(|e| {
                ContainerError::Config(format!(
                    "invalid provider_config for machine '{}': {e}",
                    params.name
                ))
            })?,
            None => ContainerOverrides::default(),
        };

        let archive = params.base_image.as_deref().ok_or_else(|| {
            ContainerError::Config("container machines require an image archive".to_string())
        })?;
        let image = self.ensure_image(archive).await?;

        let docker = self.docker().await?;
        let cname = container_name(&MachineId(params.name.clone()));

        if let Some(existing) = tolerate(docker.inspect_container(&cname, None).await, &[404])? {
            warn!(container = %cname, "Container already exists, reusing");
            let labels = existing.config.and_then(|c| c.labels).unwrap_or_default();
            let spec = read_spec(&labels)?;
            return Ok(machine_handle(&params.name, spec.ip));
        }

        debug!(
            machine = %params.name,
            disk_size_mb = params.disk_size_mb,
            "Ignoring disk_size_mb; container storage quotas need overlay2 on xfs+pquota"
        );

        let _guard = self.alloc_lock.lock().await;
        let containers = docker
            .list_containers(Some(ListContainersOptions {
                all: true,
                filters: Some(managed_filter()),
                ..Default::default()
            }))
            .await?;
        let taken: HashSet<Ipv4Addr> = containers
            .iter()
            .filter_map(|c| c.labels.as_ref())
            .filter_map(|labels| read_spec(labels).ok())
            .map(|spec| spec.ip)
            .collect();
        let ip = pick_ip(self.subnet, &taken)
            .ok_or_else(|| ContainerError::Config("subnet exhausted".to_string()))?;

        let spec = ContainerSpec {
            name: params.name.clone(),
            ip,
            cpus: params.cpus,
            memory_mb: params.memory_mb,
            runtime: overrides.runtime.or_else(|| self.config.runtime.clone()),
            cap_add: overrides.cap_add,
            command: overrides.command,
        };
        docker
            .create_container(
                Some(CreateContainerOptions {
                    name: Some(cname.clone()),
                    ..Default::default()
                }),
                create_body(&spec, &image, &self.config.network)?,
            )
            .await?;
        info!(container = %cname, %ip, %image, runtime = ?spec.runtime, "Container created");

        Ok(machine_handle(&params.name, ip))
    }

    async fn destroy_machine(&self, machine: &Machine) -> Result<(), ContainerError> {
        let docker = self.docker().await?;
        let cname = container_name(machine.id());
        tolerate(
            docker
                .remove_container(
                    &cname,
                    Some(RemoveContainerOptions {
                        force: true,
                        v: true,
                        ..Default::default()
                    }),
                )
                .await,
            &[404],
        )?;

        for (tag, _) in self.snapshot_images(machine.id()).await? {
            tolerate(
                docker
                    .remove_image(
                        &tag,
                        Some(RemoveImageOptions {
                            force: true,
                            ..Default::default()
                        }),
                        None,
                    )
                    .await,
                &[404],
            )?;
        }
        info!(container = %cname, "Container and snapshots removed");
        Ok(())
    }

    async fn start_machine(&self, machine: &Machine) -> Result<(), ContainerError> {
        let docker = self.docker().await?;
        tolerate(
            docker
                .start_container(&container_name(machine.id()), None)
                .await,
            &[304],
        )?;
        Ok(())
    }

    async fn stop_machine(&self, machine: &Machine, mode: StopMode) -> Result<(), ContainerError> {
        let docker = self.docker().await?;
        let cname = container_name(machine.id());
        if mode == StopMode::Force {
            tolerate(docker.kill_container(&cname, None).await, &[304, 409])?;
        } else {
            tolerate(
                docker
                    .stop_container(
                        &cname,
                        Some(StopContainerOptions {
                            t: Some(5),
                            signal: None,
                        }),
                    )
                    .await,
                &[304],
            )?;
        }
        Ok(())
    }

    async fn list_machines(&self) -> Result<Vec<Machine>, ContainerError> {
        let docker = self.docker().await?;
        let containers = docker
            .list_containers(Some(ListContainersOptions {
                all: true,
                filters: Some(managed_filter()),
                ..Default::default()
            }))
            .await?;

        Ok(containers
            .into_iter()
            .filter_map(|c| {
                let labels = c.labels.unwrap_or_default();
                let id = labels.get(LABEL_MACHINE)?.clone();
                let mut machine = Machine::new(MachineId(id.clone()));
                machine.set_state(map_state(c.state));
                match read_spec(&labels) {
                    Ok(spec) => machine.set_endpoint(Some(MachineEndpoint {
                        address: spec.ip.into(),
                        id,
                        platform: Platform::Linux,
                    })),
                    Err(e) => warn!(machine = %id, error = %e, "Container has no usable spec"),
                }
                Some(machine)
            })
            .collect())
    }

    async fn endpoint(&self, machine: &Machine) -> Result<Option<MachineEndpoint>, ContainerError> {
        let docker = self.docker().await?;
        let Some(info) = tolerate(
            docker
                .inspect_container(&container_name(machine.id()), None)
                .await,
            &[404],
        )?
        else {
            return Ok(None);
        };
        let labels = info.config.and_then(|c| c.labels).unwrap_or_default();
        let spec = read_spec(&labels)?;
        Ok(Some(MachineEndpoint {
            address: spec.ip.into(),
            id: machine.id().0.clone(),
            platform: Platform::Linux,
        }))
    }
}

#[async_trait]
impl Allocate for ContainerProvider {
    /// Create (but do not start) a container with a static IP on the
    /// provider network.
    async fn create(
        &self,
        params: &CreateMachineParams,
    ) -> Result<Machine, Box<dyn Error + Send + Sync>> {
        Ok(self.create_machine(params).await?)
    }

    async fn destroy(&self, machine: &Machine) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(self.destroy_machine(machine).await?)
    }

    async fn start(&self, machine: &Machine) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(self.start_machine(machine).await?)
    }

    async fn stop(
        &self,
        machine: &Machine,
        mode: StopMode,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(self.stop_machine(machine, mode).await?)
    }

    async fn list(&self) -> Result<Vec<Machine>, Box<dyn Error + Send + Sync>> {
        Ok(self.list_machines().await?)
    }

    async fn resolve_endpoint(
        &self,
        machine: &Machine,
    ) -> Result<Option<MachineEndpoint>, Box<dyn Error + Send + Sync>> {
        Ok(self.endpoint(machine).await?)
    }

    fn management_access(&self, machine: &Machine, _platform: Platform) -> ManagementAccess {
        ManagementAccess::ContainerEngine {
            container: container_name(machine.id()),
            engine_host: self.config.socket.clone(),
        }
    }
}
