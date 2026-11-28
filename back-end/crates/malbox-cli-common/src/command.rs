//! Trait implemented by every executable CLI subcommand.

use crate::context::Context;
use crate::error::Result;

/// A parsed CLI subcommand that can be run against the shared [`Context`].
pub trait Command {
    /// Runs the command, consuming its parsed arguments.
    fn execute(self, ctx: &Context) -> impl std::future::Future<Output = Result<()>> + Send;
}
