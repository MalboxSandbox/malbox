use crate::context::Context;
use crate::error::Result;

pub trait Command {
    fn execute(self, ctx: &Context) -> impl std::future::Future<Output = Result<()>> + Send;
}
