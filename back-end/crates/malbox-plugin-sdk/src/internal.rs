//! Internal helpers consumed by macro-generated code.
//!
//! This module is `#[doc(hidden)]` - plugin authors should never use it
//! directly. It exists so the proc macros can reference stable functions
//! without exposing them in the public API.

use crate::error::Result;
use std::collections::HashMap;
use std::sync::Arc;

/// Re-export the HostPlugin trait for macro codegen.
pub use crate::plugin::HostPlugin;

/// Deserialize a config HashMap into a typed struct.
///
/// Used by generated `HostPlugin` impls when the user's `on_start`
/// method takes a typed config parameter.
pub fn deserialize_config<T: serde::de::DeserializeOwned>(
    raw: HashMap<String, String>,
) -> Result<T> {
    let value = serde_json::to_value(raw).map_err(crate::error::SdkError::Serialization)?;
    serde_json::from_value(value).map_err(crate::error::SdkError::Serialization)
}

/// Initialize tracing with sensible defaults for plugins.
///
/// When a [`LogBus`](crate::log::LogBus) is provided, a
/// [`GuestLogLayer`](crate::log::GuestLogLayer) is added to the
/// subscriber so that log events are captured and can be streamed back to the
/// daemon via gRPC.
///
/// `filter` is the default `tracing_subscriber::EnvFilter` directive - e.g.
/// "info", "info,hyper=warn" - baked into the plugin binary at compile time.
/// It applies only when `RUST_LOG` is unset; a valid `RUST_LOG` takes
/// precedence, letting operators retune verbosity at runtime without a rebuild.
pub fn init_tracing(
    filter: &str,
    log_bus: Option<Arc<crate::log::LogBus>>,
) -> Option<Arc<crate::log::LogBus>> {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let fmt_layer = tracing_subscriber::fmt::layer();
    // `RUST_LOG` wins when set and valid; otherwise fall back to the compiled-in
    // default. `EnvFilter::new` never reads the environment, so this is what
    // makes runtime overrides work.
    let filter_layer = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(filter));

    if let Some(ref bus) = log_bus {
        let log_layer = bus.layer();
        if let Err(e) = tracing_subscriber::registry()
            .with(filter_layer)
            .with(fmt_layer)
            .with(log_layer)
            .try_init()
        {
            eprintln!(
                "[malbox-plugin-sdk] tracing subscriber already installed; \
                 LogBus log forwarding will not work: {e}"
            );
        }
    } else if let Err(e) = tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .try_init()
    {
        eprintln!(
            "[malbox-plugin-sdk] tracing subscriber already installed; \
             SDK formatting not applied: {e}"
        );
    }

    log_bus
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    struct TestConfig {
        name: String,
        count: String, // HashMap<String,String> values are always strings
    }

    #[test]
    fn deserialize_config_works_with_valid_map() {
        let mut raw = HashMap::new();
        raw.insert("name".to_string(), "test".to_string());
        raw.insert("count".to_string(), "42".to_string());

        let config: TestConfig = deserialize_config(raw).unwrap();
        assert_eq!(config.name, "test");
        assert_eq!(config.count, "42");
    }

    #[test]
    fn deserialize_config_fails_on_missing_field() {
        let mut raw = HashMap::new();
        raw.insert("name".to_string(), "test".to_string());
        // missing "count"

        let result: Result<TestConfig> = deserialize_config(raw);
        assert!(result.is_err());
    }
}
