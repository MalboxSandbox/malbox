use crate::api::ApiClient;
use crate::types::OutputFormat;

/// Shared command context.
///
/// Deliberately does not carry the daemon configuration: commands that need
/// it load it lazily via `malbox_config::load_config()`, so first-run
/// commands (`install`, `config init`, completions) work before any
/// configuration exists.
pub struct Context {
    pub api: ApiClient,
    pub format: OutputFormat,
    pub verbose: bool,
    pub yes: bool,
}
