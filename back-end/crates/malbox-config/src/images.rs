use serde::{Deserialize, Serialize};

/// Configuration for the image store.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImagesConfig {
    /// Path to the managed image store directory.
    pub store_path: String,
}
