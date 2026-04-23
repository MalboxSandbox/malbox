use crate::codegen;
use crate::metadata::{PluginKind, PluginMetadata};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{self, ItemStruct};

/// Expand `#[host_plugin]` or `#[guest_plugin]` on a struct.
///
/// Parses `#[malbox(state = "...", execution = "...")]`, generates a
/// `__MALBOX_META` constant, a `Default` impl (for non-unit structs), and
/// a `fn main()` that bootstraps the appropriate runtime.
pub fn expand_plugin(item: TokenStream, kind: PluginKind) -> syn::Result<TokenStream> {
    let input: ItemStruct = syn::parse2(item)?;
    let meta = PluginMetadata::from_attrs(&input.attrs)?;

    // Strip #[malbox(...)] attrs from the struct
    let mut clean_struct = input.clone();
    clean_struct.attrs.retain(|a| !a.path().is_ident("malbox"));

    let struct_name = &input.ident;

    let plugin_type = match kind {
        PluginKind::Host => quote! { malbox_plugin_sdk::types::PluginType::Host },
        PluginKind::Guest => quote! { malbox_plugin_sdk::types::PluginType::Guest },
    };

    let state = match meta.state.as_str() {
        "persistent" => quote! { malbox_plugin_sdk::types::PluginState::Persistent },
        "ephemeral" => quote! { malbox_plugin_sdk::types::PluginState::Ephemeral },
        "scoped" => quote! { malbox_plugin_sdk::types::PluginState::Scoped },
        _ => unreachable!(),
    };

    let execution = match meta.execution.as_str() {
        "exclusive" => quote! { malbox_plugin_sdk::types::ExecutionContext::Exclusive },
        "sequential" => quote! { malbox_plugin_sdk::types::ExecutionContext::Sequential },
        "parallel" => quote! { malbox_plugin_sdk::types::ExecutionContext::Parallel },
        "unrestricted" => quote! { malbox_plugin_sdk::types::ExecutionContext::Unrestricted },
        _ => unreachable!(),
    };

    let is_unit_struct = matches!(input.fields, syn::Fields::Unit);
    let default_impl = generate_default_impl(struct_name, &input.fields);
    let main_fn = codegen::generate_main(struct_name, kind, is_unit_struct);

    // Runtime-config codegen (guest plugins only — host uses a fixed default).
    let runtime_const = match kind {
        PluginKind::Guest => generate_runtime_const(&input)?,
        PluginKind::Host => proc_macro2::TokenStream::new(),
    };

    Ok(quote! {
        #clean_struct

        #default_impl

        impl #struct_name {
            #[doc(hidden)]
            pub const __MALBOX_META: malbox_plugin_sdk::types::PluginMeta = {
                const DESC: &str = env!("CARGO_PKG_DESCRIPTION");
                malbox_plugin_sdk::types::PluginMeta {
                    name: env!("CARGO_PKG_NAME"),
                    version: env!("CARGO_PKG_VERSION"),
                    description: if DESC.is_empty() { None } else { Some(DESC) },
                    authors: env!("CARGO_PKG_AUTHORS"),
                    plugin_type: #plugin_type,
                    state: #state,
                    execution: #execution,
                }
            };

            #runtime_const
        }

        #main_fn
    })
}

/// Read `plugin.toml` at macro expansion time and emit a `__MALBOX_RUNTIME`
/// associated const containing the resolved runtime configuration.
///
/// Only called for `#[guest_plugin]` — host plugins use a fixed default and
/// don't require a `[runtime]` section.
fn generate_runtime_const(input: &syn::ItemStruct) -> syn::Result<proc_macro2::TokenStream> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").map_err(|_| {
        syn::Error::new_spanned(input, "CARGO_MANIFEST_DIR not set during macro expansion")
    })?;
    let toml_path = std::path::Path::new(&manifest_dir).join("plugin.toml");
    let contents = std::fs::read_to_string(&toml_path).map_err(|e| {
        syn::Error::new_spanned(
            input,
            format!(
                "failed to read {}: {} — plugin.toml must exist at the crate root for #[guest_plugin]",
                toml_path.display(),
                e
            ),
        )
    })?;
    let manifest: malbox_plugin_manifest::PluginManifest = toml::from_str(&contents)
        .map_err(|e| syn::Error::new_spanned(input, format!("failed to parse plugin.toml: {e}")))?;

    let raw = manifest.runtime.clone().unwrap_or_default();
    let resolved = malbox_plugin_manifest::ResolvedRuntimeConfig::from_raw(&raw);
    resolved.validate().map_err(|e| {
        syn::Error::new_spanned(input, format!("invalid [runtime] in plugin.toml: {e}"))
    })?;

    let port = resolved.port;
    let work_dir = resolved.work_dir.to_string_lossy().into_owned();
    let log_overflow_dir_lit = match &raw.log_overflow_dir {
        Some(_) => {
            let s = resolved.log_overflow_dir.to_string_lossy().into_owned();
            quote::quote! { Some(#s) }
        }
        None => quote::quote! { None },
    };
    let stash_threshold = resolved.stash_threshold_bytes;
    let stash_ttl = resolved.stash_ttl_secs;
    let log_filter = resolved.log_filter.clone();

    Ok(quote::quote! {
        #[doc(hidden)]
        pub const __MALBOX_RUNTIME:
            malbox_plugin_sdk::runtime::guest::GuestRuntimeConfig =
            malbox_plugin_sdk::runtime::guest::GuestRuntimeConfig {
                listen_addr: std::net::SocketAddr::V4(
                    std::net::SocketAddrV4::new(std::net::Ipv4Addr::UNSPECIFIED, #port),
                ),
                work_dir: #work_dir,
                log_overflow_dir: #log_overflow_dir_lit,
                stash_threshold_bytes: #stash_threshold,
                stash_ttl_secs: #stash_ttl,
                log_filter: #log_filter,
            };
    })
}

/// Generate a `Default` impl for non-unit structs so the macro-generated
/// `main()` can construct the plugin without user intervention.
///
/// Unit structs don't need `Default` — they're constructed directly.
fn generate_default_impl(struct_name: &syn::Ident, fields: &syn::Fields) -> TokenStream {
    match fields {
        syn::Fields::Unit => quote! {},
        syn::Fields::Named(named) => {
            let field_defaults = named.named.iter().map(|f| {
                let name = &f.ident;
                quote! { #name: ::std::default::Default::default() }
            });
            quote! {
                impl ::std::default::Default for #struct_name {
                    fn default() -> Self {
                        Self {
                            #(#field_defaults,)*
                        }
                    }
                }
            }
        }
        syn::Fields::Unnamed(unnamed) => {
            let field_defaults = unnamed.unnamed.iter().map(|_| {
                quote! { ::std::default::Default::default() }
            });
            quote! {
                impl ::std::default::Default for #struct_name {
                    fn default() -> Self {
                        Self(#(#field_defaults,)*)
                    }
                }
            }
        }
    }
}
