use crate::api::ApiClient;
use crate::types::OutputFormat;
use malbox_config::Config;

pub struct Context {
    pub config: Config,
    pub api: ApiClient,
    pub format: OutputFormat,
    pub verbose: bool,
    pub yes: bool,
}
