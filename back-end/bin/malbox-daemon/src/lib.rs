use malbox_config::Config;
use malbox_database::init_database;
use malbox_http::http;
use malbox_machinery::provider::{create_provider, list_providers};
use malbox_plugin_internal::manager::PluginManager;
use malbox_plugin_internal::transport::ipc::{
    EventEmitter, IpcService, NodeBuilder, daemon_channel,
};
use malbox_resources::{MachinePool, resolve_transport};
use malbox_scheduler::init_scheduler;
use malbox_utils::{ResultStore, SampleStore};
use std::sync::Arc;
use tracing::{debug, info, info_span, instrument, warn};

pub mod error;
mod providers;
mod provisioners;

pub use error::DaemonError;

#[instrument(skip_all, err)]
pub async fn run(config: &Config) -> error::Result<()> {
    // Validate provider configuration
    {
        let _span = info_span!("init.providers").entered();

        let compiled_providers: Vec<String> = list_providers()
            .into_iter()
            .map(|s| s.to_string())
            .collect();

        debug!(providers = ?compiled_providers, "Providers compiled into daemon");

        config
            .providers
            .validate(&compiled_providers)
            .map_err(|e| DaemonError::Internal(e.to_string()))?;

        malbox_config::validate_machine_configs(&config.machines)
            .map_err(|e| DaemonError::Configuration(format!("Invalid machine config: {}", e)))?;
    }

    let db = {
        let _span = info_span!("init.database").entered();
        init_database(&config.database).await
    };

    // Start image store watcher if configured
    {
        let _span = info_span!("init.image_store").entered();
        if let Some(ref images_config) = config.images {
            let store_path = std::path::PathBuf::from(&images_config.store_path);
            if store_path.exists() {
                malbox_utils::image_store::spawn_image_watcher(store_path, db.clone());
            } else {
                warn!(path = %images_config.store_path, "Image store path does not exist");
            }
        }
    }

    // Initialize provider from configuration
    let (provider_arc, transport) = {
        let _span = info_span!("init.provider").entered();

        let provider_name = config
            .providers
            .get_default()
            .or_else(|| config.providers.enabled.first().map(|s| s.as_str()))
            .ok_or_else(|| {
                DaemonError::Internal(
                    "No providers enabled. Add providers to config [providers] section".to_string(),
                )
            })?;

        info!(provider = %provider_name, "Provider selected");

        // Get provider-specific configuration from the config
        let provider_config = config.providers.configs.get(provider_name).ok_or_else(|| {
            DaemonError::Internal(format!(
                "No configuration found for provider '{}'. Add [providers.{}] section to config",
                provider_name, provider_name
            ))
        })?;

        // Create the provider
        let provider = create_provider(provider_name, provider_config).map_err(|e| {
            DaemonError::Internal(format!(
                "Failed to create provider '{}': {}",
                provider_name, e
            ))
        })?;

        // Create machine pool
        let provider_arc = Arc::new(provider);

        // Resolve guest access transport (if configured)
        let transport = if let Some(ref ga_config) = config.guest_access {
            let resolved = resolve_transport(&provider_arc, ga_config, false).map_err(|e| {
                DaemonError::Configuration(format!(
                    "Guest access transport resolution failed: {}",
                    e
                ))
            })?;

            info!(
                transport = ?ga_config.transport,
                "Guest access transport selected"
            );
            Some(Arc::new(resolved))
        } else {
            warn!("No guest access configured — tasks requiring guest access will fail");
            None
        };

        (provider_arc, transport)
    };

    // Initialize plugin registry
    let registry = {
        let _span = info_span!("init.plugins").entered();

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
            malbox_plugin_internal::registry::PluginRegistry::new(plugin_dir.clone()).map_err(
                |e| DaemonError::Internal(format!("Failed to initialize plugin registry: {}", e)),
            )?,
        );

        let initial_snapshot = registry.snapshot();
        info!(
            plugins = initial_snapshot.len(),
            dir = %plugin_dir.display(),
            "Plugin registry initialized"
        );

        for entry in initial_snapshot.list() {
            info!(
                name = entry.id.as_str(),
                status = %entry.status,
                "Discovered plugin"
            );
        }

        registry
    };

    let machine_pool = Arc::new(MachinePool::new(Arc::clone(&provider_arc), db.clone()));

    // Reconcile DB machines against provider state on startup
    {
        let _span = info_span!("init.reconcile").entered();
        machine_pool.reconcile().await.map_err(|e| {
            DaemonError::Internal(format!("Failed to reconcile machine pool: {}", e))
        })?;

        machine_pool
            .reconcile_config(&config.machines)
            .await
            .map_err(|e| DaemonError::Internal(format!("Config reconciliation failed: {}", e)))?;
    }

    // Initialize IPC event emitter and plugin manager
    let plugin_manager = {
        let _span = info_span!("init.plugin_manager").entered();

        let ipc_node = Arc::new(
            NodeBuilder::new()
                .create::<IpcService>()
                .map_err(|e| DaemonError::Internal(format!("Failed to create IPC node: {}", e)))?,
        );

        let emitter = Arc::new(
            EventEmitter::new(&ipc_node, daemon_channel::EVENTS, daemon_channel::PAYLOADS)
                .map_err(|e| {
                    DaemonError::Internal(format!("Failed to create event emitter: {}", e))
                })?,
        );

        // Create plugin manager (replaces the logging-only event listener)
        Arc::new(
            PluginManager::new(
                Arc::clone(&registry),
                emitter,
                Arc::clone(&ipc_node),
                std::time::Duration::from_secs(10),
            )
            .await
            .map_err(|e| {
                DaemonError::Internal(format!("Failed to create plugin manager: {}", e))
            })?,
        )
    };

    // Initialize sample and result stores for file uploads and worker access
    let sample_store = Arc::new(SampleStore::new(&config.paths.data_dir));
    let result_store = Arc::new(ResultStore::new(&config.paths.data_dir));

    // Initialize scheduler and keep channels alive
    let (task_tx, _shutdown_tx) = {
        let _span = info_span!("init.scheduler").entered();
        init_scheduler(
            db.clone(),
            Arc::clone(&machine_pool),
            Arc::clone(&plugin_manager),
            config.general.max_workers,
            config.general.min_workers,
            config.general.idle_timeout_ms,
            transport,
            Arc::clone(&sample_store),
            Arc::clone(&result_store),
        )
        .await
        .map_err(|e| DaemonError::Internal(e.to_string()))?
    };

    // Start HTTP server (this blocks until shutdown — no init span here;
    // per-request spans come from `tower_http::TraceLayer` inside `serve`).
    http::serve(
        config.clone(),
        db,
        task_tx,
        sample_store,
        machine_pool,
        registry,
    )
    .await
    .map_err(|e| DaemonError::Internal(e.to_string()))
}
