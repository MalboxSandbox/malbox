//! Malbox crate for integration with Terraform.
//!
//! This crate provides an interface for managing Terraform operations
//! within the Malbox platform.

pub mod error;
pub mod executor;
pub mod output;
pub mod templates;
pub mod utils;
pub mod workspace;

pub use error::TerraformError as Error;
pub use output::{TerraformEvent, log_terraform_event, parse_terraform_event};
