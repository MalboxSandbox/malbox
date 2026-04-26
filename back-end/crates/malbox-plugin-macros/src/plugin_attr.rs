use crate::codegen;
use crate::metadata::PluginKind;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{self, ItemStruct};

/// Expand `#[host_plugin]` or `#[guest_plugin]` on a struct.
///
/// Reads `plugin.toml` at macro expansion time for `state` and `execution`
/// from `[runtime]`. Generates a `__MALBOX_META` constant, a `Default` impl
/// (for non-unit structs), and a `fn main()` that bootstraps the appropriate
/// runtime.
pub fn expand_plugin(item: TokenStream, kind: PluginKind) -> syn::Result<TokenStream> {
    let input: ItemStruct = syn::parse2(item)?;

    // Reject any remaining #[malbox(...)] attrs - they are no longer supported.
    for attr in &input.attrs {
        if attr.path().is_ident("malbox") {
            return Err(syn::Error::new_spanned(
                attr,
                "#[malbox(...)] attributes are no longer supported. Set state and execution in the [runtime] section of plugin.toml instead.",
            ));
        }
    }

    let manifest = read_manifest(&input)?;

    let struct_name = &input.ident;

    let plugin_type = match kind {
        PluginKind::Host => quote! { malbox_plugin_sdk::types::PluginType::Host },
        PluginKind::Guest => quote! { malbox_plugin_sdk::types::PluginType::Guest },
    };

    let state = match manifest.runtime.state {
        malbox_plugin_manifest::PluginStateConfig::Persistent => {
            quote! { malbox_plugin_sdk::types::PluginState::Persistent }
        }
        malbox_plugin_manifest::PluginStateConfig::Ephemeral => {
            quote! { malbox_plugin_sdk::types::PluginState::Ephemeral }
        }
        malbox_plugin_manifest::PluginStateConfig::Scoped => {
            quote! { malbox_plugin_sdk::types::PluginState::Scoped }
        }
    };

    let execution = match manifest.runtime.execution {
        malbox_plugin_manifest::ExecutionContextConfig::Exclusive => {
            quote! { malbox_plugin_sdk::types::ExecutionContext::Exclusive }
        }
        malbox_plugin_manifest::ExecutionContextConfig::Sequential => {
            quote! { malbox_plugin_sdk::types::ExecutionContext::Sequential }
        }
        malbox_plugin_manifest::ExecutionContextConfig::Parallel => {
            quote! { malbox_plugin_sdk::types::ExecutionContext::Parallel }
        }
        malbox_plugin_manifest::ExecutionContextConfig::Unrestricted => {
            quote! { malbox_plugin_sdk::types::ExecutionContext::Unrestricted }
        }
    };

    let is_unit_struct = matches!(input.fields, syn::Fields::Unit);
    let default_impl = generate_default_impl(struct_name, &input.fields);
    let main_fn = codegen::generate_main(struct_name, kind, is_unit_struct);

    // Runtime-config codegen (guest plugins only - host uses a fixed default).
    let runtime_const = match kind {
        PluginKind::Guest => generate_runtime_const(&manifest, &input)?,
        PluginKind::Host => proc_macro2::TokenStream::new(),
    };

    Ok(quote! {
        #input

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

/// Read `plugin.toml` at macro expansion time.
fn read_manifest(input: &syn::ItemStruct) -> syn::Result<malbox_plugin_manifest::PluginManifest> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").map_err(|_| {
        syn::Error::new_spanned(input, "CARGO_MANIFEST_DIR not set during macro expansion")
    })?;
    let toml_path = std::path::Path::new(&manifest_dir).join("plugin.toml");
    let contents = std::fs::read_to_string(&toml_path).map_err(|e| {
        syn::Error::new_spanned(
            input,
            format!(
                "failed to read {}: {} - plugin.toml must exist at the crate root",
                toml_path.display(),
                e
            ),
        )
    })?;
    toml::from_str(&contents)
        .map_err(|e| syn::Error::new_spanned(input, format!("failed to parse plugin.toml: {e}")))
}

/// Emit a `__MALBOX_RUNTIME` associated const containing the resolved runtime
/// configuration. Only called for `#[guest_plugin]`.
fn generate_runtime_const(
    manifest: &malbox_plugin_manifest::PluginManifest,
    input: &syn::ItemStruct,
) -> syn::Result<proc_macro2::TokenStream> {
    let resolved = malbox_plugin_manifest::ResolvedRuntimeConfig::from_raw(&manifest.runtime);
    resolved
        .validate(manifest.plugin.plugin_type)
        .map_err(|e| {
            syn::Error::new_spanned(input, format!("invalid [runtime] in plugin.toml: {e}"))
        })?;

    let port = resolved.port;
    let sample_dir = resolved.sample_dir.to_string_lossy().into_owned();
    let artifact_dir = resolved.artifact_dir.to_string_lossy().into_owned();
    let stash_dir = resolved.stash_dir.to_string_lossy().into_owned();
    let log_dir = resolved.log_dir.to_string_lossy().into_owned();
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
                sample_dir: #sample_dir,
                artifact_dir: #artifact_dir,
                stash_dir: #stash_dir,
                log_dir: #log_dir,
                stash_threshold_bytes: #stash_threshold,
                stash_ttl_secs: #stash_ttl,
                log_filter: #log_filter,
            };
    })
}

/// Generate a `Default` impl for non-unit structs so the macro-generated
/// `main()` can construct the plugin without user intervention.
///
/// Unit structs don't need `Default` - they're constructed directly.
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
