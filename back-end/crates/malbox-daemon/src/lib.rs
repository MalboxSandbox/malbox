//! The malbox daemon: wires configuration, database, providers, plugins, the
//! scheduler and the HTTP API together and runs them until shutdown.

use malbox_config::Config;
use malbox_database::{PgPool, init_database};
use malbox_http::http;
use malbox_machinery::ProviderHandle;
use malbox_machinery::provider::{create_provider, list_providers};
use malbox_plugin_internal::manager::PluginManager;
use malbox_plugin_internal::manager::ipc_reactor::IpcReactor;
use malbox_plugin_internal::registry::PluginRegistry;
use malbox_resources::{MachinePool, NetworkMode, ResolvedTransport, resolve_transport};
use malbox_scheduler::OrchestratorCore;
use malbox_utils::{ResultStore, SampleStore};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, info_span, instrument, warn};

/// Daemon error type and result alias.
pub mod error;
mod providers;
mod provisioners;
mod signal;

pub use error::DaemonError;

/// Boots every daemon subsystem, serves HTTP until `shutdown_token` fires, then tears down.
///
/// # Errors
///
/// Returns an error when configuration validation fails, the database cannot be
/// initialised, the provider, plugin registry, IPC reactor, plugin manager or
/// orchestrator fail to start, machine reconciliation fails, or the HTTP server errors.
#[instrument(skip_all, err)]
pub async fn run(config: &Config, shutdown_token: CancellationToken) -> error::Result<()> {
    validate_config(config)?;

    let db = {
        let _span = info_span!("init.database").entered();
        init_database(&config.database).await?
    };

    start_image_watcher(config, &db, &shutdown_token);

    let (provider_arc, transport) = init_provider(config)?;
    let registry = init_registry(config)?;

    let machine_pool = Arc::new(MachinePool::new(Arc::clone(&provider_arc), db.clone()));
    reconcile_machines(&machine_pool, config).await?;

    // Spawn the IPC reactor thread (owns all daemon-side iceoryx2 state)
    // and initialize the plugin manager.
    let (ipc, ipc_join) = {
        let _span = info_span!("init.ipc_reactor").entered();
        IpcReactor::spawn()
            .map_err(|e| DaemonError::Internal(format!("Failed to start IPC reactor: {e}")))?
    };

    // Register signal handlers AFTER iceoryx2 init. iceoryx2 overwrites
    // process-wide sigaction handlers during shared memory setup; any
    // tokio::signal listeners registered earlier are silently dead.
    signal::spawn_signal_handlers(&shutdown_token);

    let plugin_manager = {
        let _span = info_span!("init.plugin_manager").entered();

        Arc::new(
            PluginManager::new(
                Arc::clone(&registry),
                ipc.clone(),
                std::time::Duration::from_secs(10),
                shutdown_token.child_token(),
                config.paths.data_dir.join("logs"),
            )
            .await
            .map_err(|e| DaemonError::Internal(format!("Failed to create plugin manager: {e}")))?,
        )
    };

    warn_missing_soft_deps(config);

    // Initialize sample and result stores for uploads and job outputs
    let sample_store = Arc::new(SampleStore::new(&config.paths.data_dir));
    let result_store = Arc::new(ResultStore::new(&config.paths.data_dir));

    let (orchestrator, orchestrator_join) = {
        let _span = info_span!("init.orchestrator").entered();
        let core = OrchestratorCore {
            db: db.clone(),
            sample_store: Arc::clone(&sample_store),
            result_store: Arc::clone(&result_store),
            settings: Arc::new(config.orchestration.clone()),
            data_dir: config.paths.data_dir.clone(),
            identify: malbox_utils::identify::identify,
            events: Arc::new(plugin_manager.emitter().clone()),
        };
        malbox_scheduler::start(
            core,
            Arc::clone(&plugin_manager),
            Arc::clone(&machine_pool),
            transport,
            shutdown_token.child_token(),
        )
        .await
        .map_err(|e| DaemonError::Internal(format!("Failed to start orchestrator: {e}")))?
    };

    // Start HTTP server (this blocks until shutdown — no init span here;
    // per-request spans come from `tower_http::TraceLayer` inside `serve`).
    http::serve(
        config.clone(),
        db.clone(),
        orchestrator,
        sample_store,
        machine_pool,
        registry,
        shutdown_token.child_token(),
    )
    .await
    .map_err(|e| DaemonError::Internal(e.to_string()))?;

    // --- Sequenced teardown ---
    info!("HTTP server stopped, beginning teardown...");

    let teardown = async {
        if orchestrator_join.await.is_err() {
            warn!("Orchestrator task panicked during shutdown");
        }
        plugin_manager.shutdown().await;
        db.close().await;
        info!("Daemon shut down");
    };

    if tokio::time::timeout(std::time::Duration::from_secs(30), teardown)
        .await
        .is_err()
    {
        warn!("Graceful shutdown timed out after 30s");
    }

    // Stop the IPC reactor last: late scheduler workers draining during the
    // teardown window (and a future DaemonShutdown broadcast) emit through
    // its handle. The join blocks this thread for at most one reactor tick;
    // acceptable since the runtime is already in teardown.
    ipc.shutdown();
    if ipc_join.join().is_err() {
        warn!("IPC reactor thread panicked during shutdown");
    }

    Ok(())
}

/// Start the image store watcher when `[images]` is configured and its path exists.
fn start_image_watcher(config: &Config, db: &PgPool, shutdown_token: &CancellationToken) {
    let _span = info_span!("init.image_store").entered();
    if let Some(images_config) = &config.images {
        let store_path = std::path::PathBuf::from(&images_config.store_path);
        if store_path.exists() {
            malbox_utils::image_store::spawn_image_watcher(
                store_path,
                db.clone(),
                shutdown_token.child_token(),
            );
        } else {
            warn!(path = %images_config.store_path, "Image store path does not exist");
        }
    }
}

/// Checks provider, machine and orchestration settings against what this binary supports.
fn validate_config(config: &Config) -> error::Result<()> {
    let _span = info_span!("init.providers").entered();

    let compiled_providers: Vec<String> = list_providers()
        .into_iter()
        .map(std::string::ToString::to_string)
        .collect();

    debug!(providers = ?compiled_providers, "Providers compiled into daemon");

    config
        .providers
        .validate(&compiled_providers)
        .map_err(|e| DaemonError::Internal(e.to_string()))?;

    malbox_config::validate_machine_configs(&config.machines)
        .map_err(|e| DaemonError::Configuration(format!("Invalid machine config: {e}")))?;

    malbox_config::validate_orchestration(&config.orchestration)
        .map_err(|e| DaemonError::Configuration(format!("Invalid orchestration config: {e}")))?;
    Ok(())
}

/// Creates the default provider and resolves the optional guest-access transport.
fn init_provider(
    config: &Config,
) -> error::Result<(Arc<ProviderHandle>, Option<Arc<ResolvedTransport>>)> {
    let _span = info_span!("init.provider").entered();

    let provider_name = config
        .providers
        .get_default()
        .or_else(|| config.providers.enabled.first().map(String::as_str))
        .ok_or_else(|| {
            DaemonError::Internal(
                "No providers enabled. Add providers to config [providers] section".to_string(),
            )
        })?;

    info!(provider = %provider_name, "Provider selected");

    // Get provider-specific configuration from the config
    let provider_config = config.providers.configs.get(provider_name).ok_or_else(|| {
        DaemonError::Internal(format!(
            "No configuration found for provider '{provider_name}'. Add [providers.{provider_name}] section to config"
        ))
    })?;

    // Create the provider
    let provider = create_provider(provider_name, provider_config).map_err(|e| {
        DaemonError::Internal(format!("Failed to create provider '{provider_name}': {e}"))
    })?;

    let provider_arc = Arc::new(provider);

    // Resolve guest access transport (if configured)
    let transport = if let Some(ref ga_config) = config.guest_access {
        let resolved = resolve_transport(&provider_arc, ga_config, NetworkMode::Connected)
            .map_err(|e| {
                DaemonError::Configuration(format!("Guest access transport resolution failed: {e}"))
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

    Ok((provider_arc, transport))
}

/// Ensures the plugin directory exists and loads the plugin registry from it.
fn init_registry(config: &Config) -> error::Result<Arc<PluginRegistry>> {
    let _span = info_span!("init.plugins").entered();

    let plugin_dir = &config.plugins.directory;

    // Ensure plugin directory exists
    if !plugin_dir.exists() {
        std::fs::create_dir_all(plugin_dir).map_err(|e| {
            DaemonError::Internal(format!(
                "Failed to create plugin directory '{}': {e}",
                plugin_dir.display()
            ))
        })?;
    }

    let registry = Arc::new(PluginRegistry::new(plugin_dir.clone()).map_err(|e| {
        DaemonError::Internal(format!("Failed to initialize plugin registry: {e}"))
    })?);

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

    Ok(registry)
}

/// Reconciles DB machines against provider state and the configured machine list.
async fn reconcile_machines(machine_pool: &MachinePool, config: &Config) -> error::Result<()> {
    let _span = info_span!("init.reconcile").entered();
    machine_pool
        .reconcile()
        .await
        .map_err(|e| DaemonError::Internal(format!("Failed to reconcile machine pool: {e}")))?;

    machine_pool
        .reconcile_config(&config.machines)
        .await
        .map_err(|e| DaemonError::Internal(format!("Config reconciliation failed: {e}")))?;
    Ok(())
}

/// Logs a warning for each missing optional plugin dependency (read-only, no network calls).
fn warn_missing_soft_deps(config: &Config) {
    let cache_dir = config.plugins.registry.as_ref().map_or_else(
        || malbox_config::RegistryConfig::default().cache_dir,
        |r| r.cache_dir.clone(),
    );
    let missing =
        malbox_plugin_registry::deps::check_startup_deps(&config.plugins.directory, &cache_dir);
    for dep in &missing {
        tracing::warn!(
            plugin = %dep.plugin,
            dependency = %dep.dep_name,
            version = %dep.dep_version,
            "missing optional plugin dependency"
        );
    }
}
