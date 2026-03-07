use crate::{
    commands::Command,
    error::{CliError, Result},
    types::OutputFormat,
    utils::progress::Progress,
};
use clap::{Parser, Subcommand};
use malbox_config::Config;
