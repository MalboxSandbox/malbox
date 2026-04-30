//! `#[malbox::handlers]` — scans an impl block for tagged methods and generates
//! a single `impl HostPlugin for T` block.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{self, ImplItem, ItemImpl, LitStr, Token};

/// Handler methods discovered while scanning an `impl` block.
struct FoundHandlers {
    task_method: Option<syn::Ident>,
    start_method: Option<StartMethod>,
    stop_method: Option<syn::Ident>,
    health_check_method: Option<syn::Ident>,
    event_handlers: Vec<FoundEventHandler>,
}

/// A start handler, optionally accepting a typed config parameter.
struct StartMethod {
    name: syn::Ident,
    /// If the user's method takes a parameter, the macro will deserialize
    /// the raw `HashMap<String, String>` into this type.
    config_type: Option<syn::Type>,
}

/// An event handler discovered by scanning `#[on_event(...)]` attributes.
struct FoundEventHandler {
    variant: syn::Ident,
    method_name: syn::Ident,
    /// Number of non-`self` parameters on the handler method.
    param_count: usize,
    /// Whether the method declares an explicit return type (i.e. `-> Result<()>`).
    returns_result: bool,
}

/// Parsed arguments from `#[on_event(VariantName, from = [...])]`.
struct EventAttrArgs {
    variant: syn::Ident,
    // Parsed but not yet wired — requires transport-layer source metadata.
    #[allow(dead_code)]
    from_filter: Vec<String>,
}

impl syn::parse::Parse for EventAttrArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        // Support both flat `TaskCompleted` and legacy `TaskEvent::TaskCompleted` syntax.
        // For legacy paths, we take the last segment as the variant name.
        let path: syn::Path = input.parse()?;
        let variant = path
            .segments
            .last()
            .ok_or_else(|| syn::Error::new_spanned(&path, "expected event variant name"))?
            .ident
            .clone();

        let mut from_filter = Vec::new();

        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            let ident: syn::Ident = input.parse()?;
            if ident != "from" {
                return Err(syn::Error::new(ident.span(), "expected 'from'"));
            }
            input.parse::<Token![=]>()?;
            let content;
            syn::bracketed!(content in input);
            while !content.is_empty() {
                let lit: LitStr = content.parse()?;
                from_filter.push(lit.value());
                if !content.is_empty() {
                    content.parse::<Token![,]>()?;
                }
            }
        }

        Ok(EventAttrArgs {
            variant,
            from_filter,
        })
    }
}

/// Check whether an attribute matches a handler name.
///
/// Supports both bare `#[on_task]` and path-qualified `#[malbox::on_task]`.
fn is_handler_attr(attr: &syn::Attribute, name: &str) -> bool {
    let path = attr.path();
    if path.is_ident(name) {
        return true;
    }
    let segments: Vec<_> = path.segments.iter().collect();
    segments.len() == 2 && segments[0].ident == "malbox" && segments[1].ident == name
}

/// Count non-self parameters in a method signature.
fn non_self_param_count(method: &syn::ImplItemFn) -> usize {
    method
        .sig
        .inputs
        .iter()
        .filter(|arg| !matches!(arg, syn::FnArg::Receiver(_)))
        .count()
}

/// Check whether a method has an explicit return type (i.e., returns Result<()>).
fn has_return_type(method: &syn::ImplItemFn) -> bool {
    !matches!(method.sig.output, syn::ReturnType::Default)
}

/// Extract the type of the first non-self parameter, if any.
fn first_param_type(method: &syn::ImplItemFn) -> Option<syn::Type> {
    method
        .sig
        .inputs
        .iter()
        .filter_map(|arg| {
            if let syn::FnArg::Typed(pat_type) = arg {
                Some((*pat_type.ty).clone())
            } else {
                None
            }
        })
        .next()
}

/// Expand `#[malbox::handlers]` on an impl block.
///
/// Scans for methods tagged with `#[on_task]`, `#[on_start]`, `#[on_stop]`,
/// `#[health_check]`, `#[on_event(...)]` (or their
/// `#[malbox::...]` equivalents), strips those attributes, and generates a
/// single `impl HostPlugin for T` block with only the annotated methods.
pub fn expand_handlers(item: TokenStream) -> syn::Result<TokenStream> {
    let mut input: ItemImpl = syn::parse2(item)?;

    let struct_ty = &input.self_ty;

    let mut found = FoundHandlers {
        task_method: None,
        start_method: None,
        stop_method: None,
        health_check_method: None,
        event_handlers: Vec::new(),
    };

    // Scan methods and strip handler attributes
    for item in &mut input.items {
        if let ImplItem::Fn(method) = item {
            let mut to_strip = Vec::new();

            for (i, attr) in method.attrs.iter().enumerate() {
                if is_handler_attr(attr, "on_task") {
                    to_strip.push(i);
                    found.task_method = Some(method.sig.ident.clone());
                } else if is_handler_attr(attr, "on_start") {
                    to_strip.push(i);
                    found.start_method = Some(StartMethod {
                        name: method.sig.ident.clone(),
                        config_type: first_param_type(method),
                    });
                } else if is_handler_attr(attr, "on_stop") {
                    to_strip.push(i);
                    found.stop_method = Some(method.sig.ident.clone());
                } else if is_handler_attr(attr, "health_check") {
                    to_strip.push(i);
                    found.health_check_method = Some(method.sig.ident.clone());
                } else if is_handler_attr(attr, "on_event") {
                    to_strip.push(i);
                    let args: EventAttrArgs = attr.parse_args()?;
                    found.event_handlers.push(FoundEventHandler {
                        variant: args.variant,
                        method_name: method.sig.ident.clone(),
                        param_count: non_self_param_count(method),
                        returns_result: has_return_type(method),
                    });
                }
            }

            // Strip handler attrs (reverse order to preserve indices)
            for i in to_strip.into_iter().rev() {
                method.attrs.remove(i);
            }
        }
    }

    // Generate the single HostPlugin trait impl
    let plugin_impl = generate_plugin_impl(struct_ty, &found);

    Ok(quote! {
        #input

        #plugin_impl
    })
}

/// Generate a single `impl HostPlugin for T` block containing only the methods
/// that the user annotated. Unannotated methods fall through to the trait defaults.
fn generate_plugin_impl(struct_ty: &syn::Type, found: &FoundHandlers) -> TokenStream {
    let on_task = found.task_method.as_ref().map(|method_name| {
        quote! {
            fn on_task(
                &self,
                task: malbox_plugin_sdk::types::Task,
                ctx: &malbox_plugin_sdk::context::Context,
            ) -> malbox_plugin_sdk::error::Result<()> {
                self.#method_name(task, ctx)
            }
        }
    });

    let on_start = found.start_method.as_ref().map(|start| {
        let name = &start.name;
        match &start.config_type {
            Some(config_ty) => {
                quote! {
                    fn on_start(
                        &self,
                        raw_config: std::collections::HashMap<String, String>,
                    ) -> malbox_plugin_sdk::error::Result<()> {
                        let config: #config_ty =
                            malbox_plugin_sdk::internal::deserialize_config(raw_config)?;
                        self.#name(config)
                    }
                }
            }
            None => {
                quote! {
                    fn on_start(
                        &self,
                        _config: std::collections::HashMap<String, String>,
                    ) -> malbox_plugin_sdk::error::Result<()> {
                        self.#name()
                    }
                }
            }
        }
    });

    let on_stop = found.stop_method.as_ref().map(|method_name| {
        quote! {
            fn on_stop(&self) -> malbox_plugin_sdk::error::Result<()> {
                self.#method_name()
            }
        }
    });

    let health_check = found.health_check_method.as_ref().map(|method_name| {
        quote! {
            fn health_check(&self) -> malbox_plugin_sdk::types::HealthStatus {
                self.#method_name()
            }
        }
    });

    let on_event = generate_on_event(&found.event_handlers);

    // Collect all generated methods — only those that were annotated
    let methods: Vec<&TokenStream> = [
        on_task.as_ref(),
        on_start.as_ref(),
        on_stop.as_ref(),
        health_check.as_ref(),
        on_event.as_ref(),
    ]
    .into_iter()
    .flatten()
    .collect();

    quote! {
        impl malbox_plugin_sdk::plugin::HostPlugin for #struct_ty {
            #(#methods)*
        }
    }
}

/// Generate the `on_event` method if there are any event handlers.
///
/// Produces a single match statement routing each variant to the user's method.
fn generate_on_event(handlers: &[FoundEventHandler]) -> Option<TokenStream> {
    if handlers.is_empty() {
        return None;
    }

    let arms: Vec<TokenStream> = handlers
        .iter()
        .map(|h| {
            let variant = &h.variant;
            let call = make_event_handler_call(h);
            quote! {
                malbox_plugin_transport::messages::events::Event::#variant { .. } => #call
            }
        })
        .collect();

    Some(quote! {
        fn on_event(
            &self,
            event: malbox_plugin_transport::messages::events::Event,
            ctx: &malbox_plugin_sdk::context::Context,
        ) -> malbox_plugin_sdk::error::Result<()> {
            match event {
                #(#arms,)*
                _ => Ok(()),
            }
        }
    })
}

/// Generate a method call expression adapted to the event handler's parameter count.
fn make_event_handler_call(handler: &FoundEventHandler) -> TokenStream {
    let method = &handler.method_name;

    let call = match handler.param_count {
        0 => quote! { self.#method() },
        _ => quote! { self.#method(ctx) },
    };

    if handler.returns_result {
        call
    } else {
        quote! { { #call; Ok(()) } }
    }
}
