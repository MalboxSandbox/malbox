pub mod client;
pub mod deps;
pub mod error;
pub mod index;
pub mod install;
pub mod lockfile;
pub mod remove;
pub mod resolve;
pub mod source;
pub mod update;

pub use error::{RegistryError, Result};
