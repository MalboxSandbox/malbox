use serde::{Deserialize, Serialize};

/// Configuration for the image store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImagesConfig {
    /// Path to the managed image store directory.
    pub store_path: String,
}
