//! Code generation utilities shared across macro expansions.

use crate::metadata::PluginKind;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

/// Generate the `fn main()` that initializes and runs the plugin runtime.
///
/// For unit structs (`struct Foo;`) the plugin is constructed directly.
/// For structs with fields the generated code uses `Default::default()`,
/// so the struct must implement `Default`.
pub fn generate_main(struct_name: &Ident, kind: PluginKind, is_unit_struct: bool) -> TokenStream {
    let plugin_init = if is_unit_struct {
        quote! { #struct_name }
    } else {
        quote! { <#struct_name as ::std::default::Default>::default() }
    };

    match kind {
        PluginKind::Host => quote! {
            fn main() {
                malbox_plugin_sdk::internal::init_tracing();
                let plugin = #plugin_init;
                let meta = #struct_name::__MALBOX_META;
                let runtime = malbox_plugin_sdk::host_runtime::HostRuntime::new(plugin, meta)
                    .expect("failed to initialize host plugin runtime");
                runtime.run().expect("host plugin runtime error");
            }
        },
        PluginKind::Guest => quote! {
            fn main() {
                malbox_plugin_sdk::internal::init_tracing();
                let plugin = #plugin_init;
                let meta = #struct_name::__MALBOX_META;
                let runtime = malbox_plugin_sdk::guest_runtime::GuestPluginRuntime::new_v2(plugin, meta);
                runtime.run_blocking().expect("guest plugin runtime error");
            }
        },
    }
}
