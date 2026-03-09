//! Procedural macros for the Malbox plugin framework.
//!
//! This crate provides attribute macros that plugin authors use to declare
//! plugins and wire up handler methods. It is not intended to be used
//! directly — import it via [`malbox_plugin_sdk`] instead:
//!
//! ```ignore
//! extern crate malbox_plugin_sdk as malbox;
//! ```
//!
//! See the [`malbox_plugin_sdk`] crate documentation for a full usage guide.

mod codegen;
mod handlers_impl;
mod metadata;
mod plugin_attr;

use proc_macro::TokenStream;

/// Marks a struct as a host plugin (IPC-based, runs on daemon host).
///
/// Requires the `host` feature on `malbox-plugin-sdk`.
///
/// # Example
/// ```ignore
/// #[malbox::host_plugin]
/// #[malbox(state = "persistent", execution = "parallel")]
/// struct MyPlugin;
/// ```
#[proc_macro_attribute]
pub fn host_plugin(_attr: TokenStream, item: TokenStream) -> TokenStream {
    plugin_attr::expand_plugin(item.into(), metadata::PluginKind::Host)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Marks a struct as a guest plugin (gRPC-based, runs inside VM).
///
/// Requires the `guest` feature on `malbox-plugin-sdk`.
///
/// # Example
/// ```ignore
/// #[malbox::guest_plugin]
/// #[malbox(state = "ephemeral", execution = "parallel")]
/// struct MyPlugin;
/// ```
#[proc_macro_attribute]
pub fn guest_plugin(_attr: TokenStream, item: TokenStream) -> TokenStream {
    plugin_attr::expand_plugin(item.into(), metadata::PluginKind::Guest)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Applied to an `impl` block to scan for handler methods and generate trait impls.
///
/// Methods tagged with `#[malbox::on_task]`, `#[malbox::on_start]`,
/// `#[malbox::on_stop]`, `#[malbox::health_check]` are wired to the
/// corresponding internal handler traits. Missing handlers get default
/// no-op implementations.
///
/// # Example
/// ```ignore
/// #[malbox::handlers]
/// impl MyPlugin {
///     #[malbox::on_start]
///     fn init(&self) -> Result<()> { Ok(()) }
///
///     #[malbox::on_task]
///     fn process(&self, task: Task, ctx: &Context) -> Result<Vec<PluginResult>> {
///         Ok(vec![])
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn handlers(_attr: TokenStream, item: TokenStream) -> TokenStream {
    handlers_impl::expand_handlers(item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Marks a method as the task handler (used inside `#[malbox::handlers]`).
///
/// Can also be used standalone — passes through the method unchanged.
#[proc_macro_attribute]
pub fn on_task(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Pass through — the actual wiring is done by #[malbox::handlers]
    item
}

/// Marks a method as the startup handler (used inside `#[malbox::handlers]`).
#[proc_macro_attribute]
pub fn on_start(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Marks a method as the shutdown handler (used inside `#[malbox::handlers]`).
#[proc_macro_attribute]
pub fn on_stop(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Marks a method as an event handler (used inside `#[malbox::handlers]`).
#[proc_macro_attribute]
pub fn on_event(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Marks a method as a custom health check handler (used inside `#[malbox::handlers]`).
#[proc_macro_attribute]
pub fn health_check(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
