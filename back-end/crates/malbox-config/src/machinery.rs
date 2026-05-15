use serde::{Deserialize, Serialize};

/// Generic machinery configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct MachineryConfig {}
