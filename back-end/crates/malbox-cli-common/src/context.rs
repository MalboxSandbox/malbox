//! Global state shared by all CLI commands during one invocation.

use crate::api::ApiClient;
use crate::types::OutputFormat;

/// Shared command context.
///
/// Deliberately does not carry the daemon configuration: commands that need
/// it load it lazily via `malbox_config::load_config()`, so first-run
/// commands (`install`, `config init`, completions) work before any
/// configuration exists.
#[derive(Debug)]
pub struct Context {
    /// Client for the malbox daemon HTTP API.
    pub api: ApiClient,
    /// Output format selected with the global `--format` flag.
    pub format: OutputFormat,
    /// Whether verbose diagnostic output was requested.
    pub verbose: bool,
    /// Whether confirmation prompts should be answered "yes" automatically.
    pub yes: bool,
}
