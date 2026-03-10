use malbox_config::{Config, ManagerType};
use malbox_database::init_database;
use malbox_http::http;
use malbox_machinery::provider::{create_provider, list_providers};
use malbox_machinery::provisioner::{create_provisioner, list_provisioners};
use malbox_plugin_internal::manager::PluginManager;
use malbox_plugin_internal::transport::ipc::{
    EventEmitter, IpcService, NodeBuilder, daemon_channel,
};
use malbox_resources::{MachineryManager, OnDemandManager, PooledManager, resolve_transport};
use malbox_scheduler::init_scheduler;
use malbox_utils::SampleStore;
use std::sync::Arc;

pub mod error;
mod providers;
mod provisioners;

pub use error::DaemonError;

/// Validate that the configured manager is compatible with provider capabilities.
fn validate_manager_compatibility(
    manager_type: ManagerType,
    provider: &malbox_machinery::ProviderHandle,
) -> error::Result<()> {
    match manager_type {
        ManagerType::Pooled => {
            // Pooled manager requires Snapshot capability
            if provider.snapshot().is_none() {
                return Err(DaemonError::Configuration(format!(
                    "Manager 'pooled' requires provider '{}' to support Snapshot capability. \
                     Available capabilities: {:?}. \
                     Either:\n  1. Change [machinery] manager to 'ondemand' in config, or\n  \
                     2. Use a provider that supports Snapshot capability",
                    provider.name(),
                    provider.list_capabilities()
                )));
            }
        }
        ManagerType::OnDemand => {
            // On-demand manager works with any provider (only needs Allocate, which is mandatory)
        }
    }
    Ok(())
}

// TODO: add provider-specific validation and move this into malbox-config
/// Validate that the configured base image exists and is readable.
fn validate_base_image(config: &malbox_config::MachineryConfig) -> error::Result<()> {
    if let Some(ref path_str) = config.defaults.image {
        let path = std::path::Path::new(path_str);

        if !path.exists() {
            return Err(DaemonError::Configuration(format!(
                "Base image not found: '{}'. \
                 Set [machinery.defaults] base_image to a valid image path, \
                 or remove it to use blank disks.",
                path_str
            )));
        }

        let metadata = std::fs::metadata(path).map_err(|e| {
            DaemonError::Configuration(format!("Cannot read base image '{}': {}", path_str, e))
        })?;

        if !metadata.is_file() {
            return Err(DaemonError::Configuration(format!(
                "Base image path '{}' is not a regular file",
                path_str
            )));
        }

        // Verify the file is readable
        std::fs::File::open(path).map_err(|e| {
            DaemonError::Configuration(format!("Base image '{}' is not readable: {}", path_str, e))
        })?;

        tracing::info!("Base image validated: {}", path_str);
    }
    Ok(())
}

pub async fn run(config: &Config) -> error::Result<()> {
    // Validate provider configuration
    let compiled_providers: Vec<String> = list_providers()
        .into_iter()
        .map(|s| s.to_string())
        .collect();

    tracing::info!("Providers compiled into daemon: {:?}", compiled_providers);

    config
        .providers
        .validate(&compiled_providers)
        .map_err(|e| DaemonError::Internal(e.to_string()))?;

    let db = init_database(&config.database).await;

    // Initialize provider from configuration
    let provider_name = config
        .providers
        .get_default()
        .or_else(|| config.providers.enabled.first().map(|s| s.as_str()))
        .ok_or_else(|| {
            DaemonError::Internal(
                "No providers enabled. Add providers to config [providers] section".to_string(),
            )
        })?;

    tracing::info!("Using provider: {}", provider_name);

    // Get provider-specific configuration from the config
    let provider_config = config.providers.configs.get(provider_name).ok_or_else(|| {
        DaemonError::Internal(format!(
            "No configuration found for provider '{}'. Add [providers.{}] section to config",
            provider_name, provider_name
        ))
    })?;

    tracing::debug!("Provider config: {:?}", provider_config);

    // Create the provider
    let provider = create_provider(provider_name, provider_config).map_err(|e| {
        DaemonError::Internal(format!(
            "Failed to create provider '{}': {}",
            provider_name, e
        ))
    })?;

    // Validate manager/provider compatibility
    let manager_type = config.machinery.manager;
    validate_manager_compatibility(manager_type, &provider)?;

    // Validate base image if configured
    validate_base_image(&config.machinery)?;

    tracing::info!("Using machinery manager: {}", manager_type);

    // Create provisioner if configured
    let provisioner: Option<Arc<dyn malbox_machinery::Provisioner>> = if let Some(ref prov_config) =
        config.provisioning
    {
        let compiled_provisioners = list_provisioners();
        tracing::info!(
            "Provisioners compiled into daemon: {:?}",
            compiled_provisioners
        );

        tracing::info!("Using provisioner: {}", prov_config.provisioner_type);

        let provisioner = create_provisioner(&prov_config.provisioner_type, &prov_config.config)
            .map_err(|e| {
                DaemonError::Internal(format!(
                    "Failed to create provisioner '{}': {}",
                    prov_config.provisioner_type, e
                ))
            })?;

        Some(Arc::from(provisioner))
    } else {
        tracing::info!("No provisioning configured");
        None
    };

    // Create machinery manager based on configuration
    let provider_arc = Arc::new(provider);

    // Resolve guest access transport (if configured)
    let transport = if let Some(ref ga_config) = config.guest_access {
        let default_network_mode = malbox_machinery::NetworkMode::Nat;

        let resolved =
            resolve_transport(&provider_arc, ga_config, &default_network_mode).map_err(|e| {
                DaemonError::Configuration(format!(
                    "Guest access transport resolution failed: {}",
                    e
                ))
            })?;

        tracing::info!(
            transport = ga_config.transport.as_str(),
            "Guest access transport resolved"
        );
        Some(Arc::new(resolved))
    } else {
        tracing::info!("No guest access configured — tasks requiring guest access will fail");
        None
    };

    // Initialize plugin registry
    let plugin_dir = &config.plugins.directory;

    // Ensure plugin directory exists
    if !plugin_dir.exists() {
        std::fs::create_dir_all(plugin_dir).map_err(|e| {
            DaemonError::Internal(format!(
                "Failed to create plugin directory '{}': {}",
                plugin_dir.display(),
                e
            ))
        })?;
    }

    let registry = Arc::new(
        malbox_plugin_internal::registry::PluginRegistry::new(plugin_dir.clone()).map_err(|e| {
            DaemonError::Internal(format!("Failed to initialize plugin registry: {}", e))
        })?,
    );

    let initial_snapshot = registry.snapshot();
    tracing::info!(
        plugins = initial_snapshot.len(),
        dir = %plugin_dir.display(),
        "Plugin registry initialized"
    );

    for entry in initial_snapshot.list() {
        tracing::info!(
            name = entry.id.as_str(),
            status = %entry.status,
            "Discovered plugin"
        );
    }

    let machinery_manager: Arc<dyn MachineryManager> = match manager_type {
        ManagerType::Pooled => {
            let pooled_manager = PooledManager::new(provider_arc, &config.machinery, provisioner)
                .map_err(|e| {
                DaemonError::Internal(format!("Failed to create pooled manager: {}", e))
            })?;

            // Initialize pool with machines
            pooled_manager
                .initialize_pool()
                .await
                .map_err(|e| DaemonError::Internal(format!("Failed to initialize pool: {}", e)))?;

            Arc::new(pooled_manager)
        }
        ManagerType::OnDemand => Arc::new(OnDemandManager::new(provider_arc, provisioner)),
    };

    // Initialize IPC event emitter for daemon → plugin communication
    let node = NodeBuilder::new()
        .create::<IpcService>()
        .map_err(|e| DaemonError::Internal(format!("Failed to create IPC node: {}", e)))?;

    let emitter = Arc::new(
        EventEmitter::new(&node, daemon_channel::EVENTS, daemon_channel::PAYLOADS)
            .map_err(|e| DaemonError::Internal(format!("Failed to create event emitter: {}", e)))?,
    );

    // Create plugin manager (replaces the logging-only event listener)
    let plugin_manager = Arc::new(
        PluginManager::new(
            Arc::clone(&registry),
            emitter,
            std::time::Duration::from_secs(10),
        )
        .await
        .map_err(|e| DaemonError::Internal(format!("Failed to create plugin manager: {}", e)))?,
    );

    // Initialize sample store for file uploads and worker access
    let sample_store = Arc::new(SampleStore::new(&config.paths.data_dir));

    // Initialize scheduler and keep channels alive
    let (task_tx, _shutdown_tx) = init_scheduler(
        db.clone(),
        machinery_manager,
        config.machinery.clone(),
        Arc::clone(&plugin_manager),
        config.general.worker_threads,
        transport,
        Arc::clone(&sample_store),
    )
    .await
    .map_err(|e| DaemonError::Internal(e.to_string()))?;

    // Start HTTP server (this blocks until shutdown)
    http::serve(config.clone(), db, task_tx, sample_store)
        .await
        .map_err(|e| DaemonError::Internal(e.to_string()))
}
