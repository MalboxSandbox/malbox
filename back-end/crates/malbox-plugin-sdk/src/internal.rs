//! Internal helpers used by generated macro code.
//!
//! This module is `#[doc(hidden)]` -- plugin authors should never use it directly.

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
/// `filter` is a `tracing_subscriber::EnvFilter` directive string — e.g.
/// "info", "info,hyper=warn". Callers pass this explicitly (it's baked into
/// the plugin binary at compile time by the `#[guest_plugin]` macro).
pub fn init_tracing(
    filter: &str,
    log_bus: Option<Arc<crate::log::LogBus>>,
) -> Option<Arc<crate::log::LogBus>> {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let fmt_layer = tracing_subscriber::fmt::layer();
    let filter_layer = tracing_subscriber::EnvFilter::new(filter);

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
