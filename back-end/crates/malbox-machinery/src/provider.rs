use super::machine::{DynMachine, Machine, MachineSpec};
use async_trait::async_trait;
use std::marker::PhantomData;

pub mod extension;

/// Provider trait that all providers must implement.
#[async_trait]
pub trait Provider: Sized + Send + Sync + 'static {
    /// Provider-specific configuration.
    type Config: Send + Sync;
    /// Provider-specific machine type.
    type Machine: Machine;
    /// Provider-specific error type.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Initialize the provider from configuration.
    async fn initialize(config: Self::Config) -> Result<Self, Self::Error>;
    /// Allocate a machine.
    async fn allocate(&self, spec: MachineSpec) -> Result<Self::Machine, Self::Error>;
    /// Deallocate a machine and clean up resources.
    async fn deallocate(&self, machine: &Self::Machine) -> Result<(), Self::Error>;
    /// List machines.
    async fn list(&self) -> Result<Option<Vec<Self::Machine>>, Self::Error>;
}

/// Object-safe provider trait for dynamic dispatch.
///
/// This trait provides a type-erased interface to providers, allowing them to be
/// stored and used without knowing their concrete types at compile time.
#[async_trait]
pub trait DynProvider: Send + Sync {
    /// Provider name/identifier for debugging.
    fn name(&self) -> &str;

    /// Allocate a machine with the given spec.
    async fn allocate(
        &self,
        spec: MachineSpec,
    ) -> Result<Box<dyn DynMachine>, Box<dyn std::error::Error + Send + Sync>>;

    /// Deallocate a machine (machine must be from this provider).
    async fn deallocate(
        &self,
        machine: &dyn DynMachine,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// List all machines managed by this provider.
    async fn list(
        &self,
    ) -> Result<Option<Vec<Box<dyn DynMachine>>>, Box<dyn std::error::Error + Send + Sync>>;
}

/// Wrapper that implements DynProvider for any Provider type.
///
/// This allows providers with associated types to be used through the object-safe
/// DynProvider trait. The wrapper stores the concrete machine type information for
/// safe downcasting during deallocation.
pub struct ProviderWrapper<P: Provider> {
    provider: P,
    _phantom: PhantomData<P::Machine>,
}

impl<P: Provider> ProviderWrapper<P> {
    /// Create a new provider wrapper.
    pub fn new(provider: P) -> Self {
        Self {
            provider,
            _phantom: PhantomData,
        }
    }

    /// Get a reference to the underlying provider.
    pub fn inner(&self) -> &P {
        &self.provider
    }
}

#[async_trait]
impl<P: Provider + 'static> DynProvider for ProviderWrapper<P>
where
    P::Machine: 'static,
{
    fn name(&self) -> &str {
        std::any::type_name::<P>()
    }

    async fn allocate(
        &self,
        spec: MachineSpec,
    ) -> Result<Box<dyn DynMachine>, Box<dyn std::error::Error + Send + Sync>> {
        let machine = self
            .provider
            .allocate(spec)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        Ok(Box::new(machine) as Box<dyn DynMachine>)
    }

    async fn deallocate(
        &self,
        machine: &dyn DynMachine,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Downcast to the concrete machine type
        let concrete_machine = machine
            .as_any()
            .downcast_ref::<P::Machine>()
            .ok_or_else(|| {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!(
                        "Machine type mismatch: cannot deallocate machine from different provider. \
                         Expected {}, got a different type.",
                        std::any::type_name::<P::Machine>()
                    ),
                )) as Box<dyn std::error::Error + Send + Sync>
            })?;

        self.provider
            .deallocate(concrete_machine)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    async fn list(
        &self,
    ) -> Result<Option<Vec<Box<dyn DynMachine>>>, Box<dyn std::error::Error + Send + Sync>> {
        let machines = self
            .provider
            .list()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        Ok(machines.map(|vec| {
            vec.into_iter()
                .map(|m| Box::new(m) as Box<dyn DynMachine>)
                .collect()
        }))
    }
}
