//! `#[malbox::handlers]` — scans an impl block for tagged methods and generates
//! trait impls for either `HostPlugin` or `GuestPlugin`.
//!
//! Detection: if `#[on_task]` is present, generates `HostPlugin`.
//! Otherwise, generates `GuestPlugin`.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{self, ImplItem, ItemImpl, LitStr, Token};

/// Handler methods discovered while scanning an `impl` block.
struct FoundHandlers {
    task_method: Option<syn::Ident>,
    start_method: Option<StartMethod>,
    stop_method: Option<StopMethod>,
    health_check_method: Option<syn::Ident>,
    event_handlers: Vec<FoundEventHandler>,
}

/// A start handler, optionally accepting a typed config parameter (host)
/// or a `&Context` parameter (guest).
struct StartMethod {
    name: syn::Ident,
    /// If the user's method takes a parameter, the macro will deserialize
    /// the raw `HashMap<String, String>` into this type (host plugins),
    /// or pass `&Context` through (guest plugins).
    param_type: Option<syn::Type>,
}

/// A stop handler, optionally accepting a `&Context` parameter (guest).
struct StopMethod {
    name: syn::Ident,
    has_param: bool,
}

/// An event handler discovered by scanning `#[on_event(...)]` attributes.
struct FoundEventHandler {
    variant: syn::Ident,
    method_name: syn::Ident,
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
                        param_type: first_param_type(method),
                    });
                } else if is_handler_attr(attr, "on_stop") {
                    to_strip.push(i);
                    found.stop_method = Some(StopMethod {
                        name: method.sig.ident.clone(),
                        has_param: first_param_type(method).is_some(),
                    });
                } else if is_handler_attr(attr, "health_check") {
                    to_strip.push(i);
                    found.health_check_method = Some(method.sig.ident.clone());
                } else if is_handler_attr(attr, "on_event") {
                    to_strip.push(i);
                    let args: EventAttrArgs = attr.parse_args()?;
                    found.event_handlers.push(FoundEventHandler {
                        variant: args.variant,
                        method_name: method.sig.ident.clone(),
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

    let is_guest = found.task_method.is_none();
    let plugin_impl = if is_guest {
        generate_guest_plugin_impl(struct_ty, &found)
    } else {
        generate_host_plugin_impl(struct_ty, &found)
    };

    Ok(quote! {
        #input

        #plugin_impl
    })
}

fn generate_health_check(found: &FoundHandlers) -> Option<TokenStream> {
    found.health_check_method.as_ref().map(|method_name| {
        quote! {
            fn health_check(&self) -> malbox_plugin_sdk::types::HealthStatus {
                self.#method_name()
            }
        }
    })
}

/// Generate `impl Plugin + GuestPlugin for T`.
///
/// Guest plugins use `on_start(&self, ctx)` and `on_stop(&self, ctx)` -
/// both receive `&Context`. No `on_task` or `on_event`.
fn generate_guest_plugin_impl(struct_ty: &syn::Type, found: &FoundHandlers) -> TokenStream {
    let health_check = generate_health_check(found);

    let on_start = found.start_method.as_ref().map(|start| {
        let name = &start.name;
        if start.param_type.is_some() {
            quote! {
                fn on_start(
                    &self,
                    ctx: &malbox_plugin_sdk::context::Context,
                ) -> malbox_plugin_sdk::error::Result<()> {
                    self.#name(ctx)
                }
            }
        } else {
            quote! {
                fn on_start(
                    &self,
                    _ctx: &malbox_plugin_sdk::context::Context,
                ) -> malbox_plugin_sdk::error::Result<()> {
                    self.#name()
                }
            }
        }
    });

    let on_stop = found.stop_method.as_ref().map(|stop| {
        let name = &stop.name;
        if stop.has_param {
            quote! {
                fn on_stop(
                    &self,
                    ctx: &malbox_plugin_sdk::context::Context,
                ) -> malbox_plugin_sdk::error::Result<()> {
                    self.#name(ctx)
                }
            }
        } else {
            quote! {
                fn on_stop(
                    &self,
                    _ctx: &malbox_plugin_sdk::context::Context,
                ) -> malbox_plugin_sdk::error::Result<()> {
                    self.#name()
                }
            }
        }
    });

    let guest_methods: Vec<&TokenStream> = [on_start.as_ref(), on_stop.as_ref()]
        .into_iter()
        .flatten()
        .collect();

    quote! {
        impl malbox_plugin_sdk::plugin::Plugin for #struct_ty {
            #health_check
        }

        impl malbox_plugin_sdk::guest_plugin::GuestPlugin for #struct_ty {
            #(#guest_methods)*
        }
    }
}

/// Generate `impl Plugin + HostPlugin for T`.
///
/// Host plugins use `on_start(&self, config)` and `on_stop(&self)`,
/// plus `on_task(&self, ctx)` and `on_event(&self, event)`.
fn generate_host_plugin_impl(struct_ty: &syn::Type, found: &FoundHandlers) -> TokenStream {
    let health_check = generate_health_check(found);

    let on_task = found.task_method.as_ref().map(|method_name| {
        quote! {
            fn on_task(
                &self,
                ctx: &malbox_plugin_sdk::context::Context,
            ) -> malbox_plugin_sdk::error::Result<()> {
                self.#method_name(ctx)
            }
        }
    });

    let on_start = found.start_method.as_ref().map(|start| {
        let name = &start.name;
        match &start.param_type {
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

    let on_stop = found.stop_method.as_ref().map(|stop| {
        let name = &stop.name;
        quote! {
            fn on_stop(&self) -> malbox_plugin_sdk::error::Result<()> {
                self.#name()
            }
        }
    });

    let on_event = generate_on_event(&found.event_handlers);

    let host_methods: Vec<&TokenStream> = [
        on_task.as_ref(),
        on_start.as_ref(),
        on_stop.as_ref(),
        on_event.as_ref(),
    ]
    .into_iter()
    .flatten()
    .collect();

    quote! {
        impl malbox_plugin_sdk::plugin::Plugin for #struct_ty {
            #health_check
        }

        impl malbox_plugin_sdk::plugin::HostPlugin for #struct_ty {
            #(#host_methods)*
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
        ) -> malbox_plugin_sdk::error::Result<()> {
            match event {
                #(#arms,)*
                _ => Ok(()),
            }
        }
    })
}

/// Generate a method call expression adapted to the event handler's signature.
///
/// All event handlers are called with no arguments. Since `on_event` no longer
/// receives a `ctx` parameter, handlers cannot receive it either.
fn make_event_handler_call(handler: &FoundEventHandler) -> TokenStream {
    let method = &handler.method_name;

    let call = quote! { self.#method() };

    if handler.returns_result {
        call
    } else {
        quote! { { #call; Ok(()) } }
    }
}
