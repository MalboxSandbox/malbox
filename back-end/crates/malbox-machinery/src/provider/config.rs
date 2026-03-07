//! Configuration helpers for providers.

use crate::provider::TomlValue;

/// Deserialize a TOML value into a typed configuration.
///
/// This helper allows providers to deserialize their configuration without
/// depending on the toml crate directly.
///
/// # Example
///
/// ```ignore
/// use malbox_machinery::provider::{config, TomlValue};
///
/// #[derive(Deserialize)]
/// struct MyProviderConfig {
///     uri: String,
///     pool: String,
/// }
///
/// pub fn new(config: &TomlValue) -> Result<Self> {
///     let config: MyProviderConfig = config::deserialize(config)?;
///     // ...
/// }
/// ```
pub fn deserialize<T>(config: &TomlValue) -> Result<T, Box<dyn std::error::Error + Send + Sync>>
where
    T: serde::de::DeserializeOwned,
{
    let toml_str = toml::to_string(config)?;
    let deserialized: T = toml::from_str(&toml_str)?;
    Ok(deserialized)
}
