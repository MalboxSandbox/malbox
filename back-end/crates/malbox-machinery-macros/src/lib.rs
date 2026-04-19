use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, Meta, parse_macro_input};

/// Derive macro for automatically registering a provider with the machinery registry.
#[proc_macro_derive(RegisterProvider, attributes(provider, capability))]
pub fn register_provider_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    // Extract provider name from #[provider(name = "...")] attribute
    let provider_name = extract_provider_name(&input);

    // Extract capabilities from #[capability(...)] attributes
    let capabilities = extract_capabilities(&input);

    let struct_name = &input.ident;

    // Generate capability casts
    let snapshot_cast = if capabilities.contains(&"Snapshot".to_string()) {
        quote! {
            ::std::option::Option::Some(
                provider.clone() as ::std::sync::Arc<dyn ::malbox_machinery::Snapshot>
            )
        }
    } else {
        quote! { ::std::option::Option::None }
    };

    let clone_cast = if capabilities.contains(&"Clone".to_string()) {
        quote! {
            ::std::option::Option::Some(
                provider.clone() as ::std::sync::Arc<dyn ::malbox_machinery::Clone>
            )
        }
    } else {
        quote! { ::std::option::Option::None }
    };

    let migrate_cast = if capabilities.contains(&"Migrate".to_string()) {
        quote! {
            ::std::option::Option::Some(
                provider.clone() as ::std::sync::Arc<dyn ::malbox_machinery::Migrate>
            )
        }
    } else {
        quote! { ::std::option::Option::None }
    };

    let guest_access_cast = if capabilities.contains(&"GuestAccess".to_string()) {
        quote! {
            ::std::option::Option::Some(
                provider.clone() as ::std::sync::Arc<dyn ::malbox_machinery::GuestAccess>
            )
        }
    } else {
        quote! { ::std::option::Option::None }
    };

    // Generate the registration code
    let expanded = quote! {
        // Ensure the provider struct is in scope for the macro-generated code
        const _: () = {
            // Submit provider metadata to inventory
            ::inventory::submit! {
                ::malbox_machinery::ProviderMetadata {
                    name: #provider_name,
                    create: |config: &::malbox_machinery::provider::TomlValue| {
                        // Create provider instance with configuration
                        let provider = ::std::sync::Arc::new(
                            #struct_name::new(config)
                                .map_err(|e| Box::new(e) as Box<dyn ::std::error::Error + Send + Sync>)?
                        );

                        // Create ProviderHandle with capabilities
                        let handle = ::malbox_machinery::ProviderHandle::new(
                            #provider_name.to_string(),
                            // Allocate capability (always required)
                            provider.clone() as ::std::sync::Arc<dyn ::malbox_machinery::Allocate>,
                            // Snapshot capability (optional)
                            #snapshot_cast,
                            // Clone capability (optional)
                            #clone_cast,
                            // Migrate capability (optional)
                            #migrate_cast,
                            // GuestAccess capability (optional)
                            #guest_access_cast,
                        );

                        Ok(handle)
                    },
                }
            }
        };
    };

    TokenStream::from(expanded)
}

/// Extract the provider name from #[provider(name = "...")] attribute
fn extract_provider_name(input: &DeriveInput) -> String {
    for attr in &input.attrs {
        if attr.path().is_ident("provider")
            && let Meta::List(meta_list) = &attr.meta
        {
            let tokens_str = meta_list.tokens.to_string();

            // Parse: name = "value"
            for part in tokens_str.split(',') {
                let part = part.trim();
                if let Some(name_part) = part.strip_prefix("name") {
                    let name_part = name_part.trim();
                    if let Some(value_part) = name_part.strip_prefix('=') {
                        let value = value_part.trim().trim_matches('"');
                        return value.to_string();
                    }
                }
            }
        }
    }

    panic!("RegisterProvider requires #[provider(name = \"...\")] attribute");
}

/// Extract capability names from #[capability(...)] attributes
fn extract_capabilities(input: &DeriveInput) -> Vec<String> {
    let mut capabilities = Vec::new();

    for attr in &input.attrs {
        if attr.path().is_ident("capability")
            && let Meta::List(meta_list) = &attr.meta
        {
            let tokens_str = meta_list.tokens.to_string();
            capabilities.push(tokens_str.trim().to_string());
        }
    }

    capabilities
}
