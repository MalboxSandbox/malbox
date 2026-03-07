//! Configuration helpers for provisioners.

/// Deserialize a TOML value into a typed provisioner configuration.
///
/// Allows provisioner crates to deserialize their config without
/// depending on the toml crate directly.
pub fn deserialize<T>(config: &toml::Value) -> Result<T, Box<dyn std::error::Error + Send + Sync>>
where
    T: serde::de::DeserializeOwned,
{
    let toml_str = toml::to_string(config)?;
    let deserialized: T = toml::from_str(&toml_str)?;
    Ok(deserialized)
}
