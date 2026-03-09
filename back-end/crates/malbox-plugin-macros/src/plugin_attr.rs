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
        }

        #main_fn
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
