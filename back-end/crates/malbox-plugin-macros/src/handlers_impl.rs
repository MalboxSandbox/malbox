//! `#[malbox::handlers]` — scans an impl block for tagged methods and generates
//! internal handler trait impls.

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
    category: EventCategory,
    variant: syn::Ident,
    method_name: syn::Ident,
    /// Number of non-`self` parameters on the handler method.
    param_count: usize,
    /// Whether the method declares an explicit return type (i.e. `-> Result<()>`).
    returns_result: bool,
}

/// The four top-level event categories from the transport layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum EventCategory {
    Daemon,
    Task,
    Plugin,
    Sample,
}

/// Parsed arguments from `#[on_event(Category::Variant, from = [...])]`.
struct EventAttrArgs {
    event_path: syn::Path,
    // Parsed but not yet wired — requires transport-layer source metadata.
    #[allow(dead_code)]
    from_filter: Vec<String>,
}

impl syn::parse::Parse for EventAttrArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let event_path: syn::Path = input.parse()?;
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
            event_path,
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

/// Parse the event category from a two-segment path like `DaemonEvent::ConfigReloaded`.
fn parse_event_category(path: &syn::Path) -> syn::Result<(EventCategory, syn::Ident)> {
    let segments: Vec<_> = path.segments.iter().collect();
    if segments.len() != 2 {
        return Err(syn::Error::new_spanned(
            path,
            "expected Category::Variant (e.g. DaemonEvent::ConfigReloaded)",
        ));
    }

    let category = match segments[0].ident.to_string().as_str() {
        "DaemonEvent" => EventCategory::Daemon,
        "TaskEvent" => EventCategory::Task,
        "PluginEvent" => EventCategory::Plugin,
        "SampleEvent" => EventCategory::Sample,
        other => {
            return Err(syn::Error::new(
                segments[0].ident.span(),
                format!(
                    "unknown event category '{}': expected DaemonEvent, TaskEvent, PluginEvent, or SampleEvent",
                    other
                ),
            ));
        }
    };

    Ok((category, segments[1].ident.clone()))
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
/// `#[health_check]`, `#[on_event(...)]` (or their `#[malbox::...]`
/// equivalents), strips those attributes, generates trait impls for found
/// handlers, and fills defaults for missing ones.
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
                    let (category, variant) = parse_event_category(&args.event_path)?;
                    found.event_handlers.push(FoundEventHandler {
                        category,
                        variant,
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

    // Generate trait impls
    let task_impl = generate_task_handler(struct_ty, &found.task_method);
    let start_impl = generate_start_handler(struct_ty, &found.start_method);
    let stop_impl = generate_stop_handler(struct_ty, &found.stop_method);
    let health_check_impl = generate_health_check_handler(struct_ty, &found.health_check_method);
    let event_impls = generate_event_handlers(struct_ty, &found.event_handlers);

    Ok(quote! {
        #input

        #task_impl
        #start_impl
        #stop_impl
        #health_check_impl
        #event_impls
    })
}

/// Generate a `TaskHandler` trait impl that delegates to the user's tagged method,
/// or returns an empty `Vec` if no `#[on_task]` was found.
fn generate_task_handler(struct_ty: &syn::Type, method: &Option<syn::Ident>) -> TokenStream {
    if let Some(method_name) = method {
        quote! {
            impl malbox_plugin_sdk::internal::TaskHandler for #struct_ty {
                fn __handle_task(
                    &self,
                    task: malbox_plugin_sdk::types::Task,
                    ctx: &malbox_plugin_sdk::context::Context,
                ) -> malbox_plugin_sdk::error::Result<Vec<malbox_plugin_sdk::types::PluginResult>> {
                    self.#method_name(task, ctx)
                }
            }
        }
    } else {
        quote! {
            impl malbox_plugin_sdk::internal::TaskHandler for #struct_ty {
                fn __handle_task(
                    &self,
                    _task: malbox_plugin_sdk::types::Task,
                    _ctx: &malbox_plugin_sdk::context::Context,
                ) -> malbox_plugin_sdk::error::Result<Vec<malbox_plugin_sdk::types::PluginResult>> {
                    Ok(vec![])
                }
            }
        }
    }
}

/// Generate a `StartHandler` trait impl.
///
/// If the user's method accepts a typed config parameter, the generated impl
/// deserializes the raw `HashMap<String, String>` into that type. If no
/// `#[on_start]` was found, the default impl is a no-op.
fn generate_start_handler(struct_ty: &syn::Type, method: &Option<StartMethod>) -> TokenStream {
    match method {
        Some(StartMethod {
            name,
            config_type: Some(config_ty),
        }) => {
            // User's on_start takes a typed config parameter — deserialize it.
            quote! {
                impl malbox_plugin_sdk::internal::StartHandler for #struct_ty {
                    fn __handle_start(
                        &self,
                        raw_config: std::collections::HashMap<String, String>,
                    ) -> malbox_plugin_sdk::error::Result<()> {
                        let config: #config_ty =
                            malbox_plugin_sdk::internal::deserialize_config(raw_config)?;
                        self.#name(config)
                    }
                }
            }
        }
        Some(StartMethod {
            name,
            config_type: None,
        }) => {
            // User's on_start takes no config — just call it.
            quote! {
                impl malbox_plugin_sdk::internal::StartHandler for #struct_ty {
                    fn __handle_start(
                        &self,
                        _raw_config: std::collections::HashMap<String, String>,
                    ) -> malbox_plugin_sdk::error::Result<()> {
                        self.#name()
                    }
                }
            }
        }
        None => {
            quote! {
                impl malbox_plugin_sdk::internal::StartHandler for #struct_ty {
                    fn __handle_start(
                        &self,
                        _raw_config: std::collections::HashMap<String, String>,
                    ) -> malbox_plugin_sdk::error::Result<()> {
                        Ok(())
                    }
                }
            }
        }
    }
}

/// Generate a `StopHandler` trait impl, defaulting to a no-op if absent.
fn generate_stop_handler(struct_ty: &syn::Type, method: &Option<syn::Ident>) -> TokenStream {
    if let Some(method_name) = method {
        quote! {
            impl malbox_plugin_sdk::internal::StopHandler for #struct_ty {
                fn __handle_stop(&self) -> malbox_plugin_sdk::error::Result<()> {
                    self.#method_name()
                }
            }
        }
    } else {
        quote! {
            impl malbox_plugin_sdk::internal::StopHandler for #struct_ty {
                fn __handle_stop(&self) -> malbox_plugin_sdk::error::Result<()> {
                    Ok(())
                }
            }
        }
    }
}

/// Generate a `HealthCheckHandler` trait impl, defaulting to `HealthStatus::ready()`.
fn generate_health_check_handler(
    struct_ty: &syn::Type,
    method: &Option<syn::Ident>,
) -> TokenStream {
    if let Some(method_name) = method {
        quote! {
            impl malbox_plugin_sdk::internal::HealthCheckHandler for #struct_ty {
                fn __handle_health_check(&self) -> malbox_plugin_sdk::types::HealthStatus {
                    self.#method_name()
                }
            }
        }
    } else {
        quote! {
            impl malbox_plugin_sdk::internal::HealthCheckHandler for #struct_ty {
                fn __handle_health_check(&self) -> malbox_plugin_sdk::types::HealthStatus {
                    malbox_plugin_sdk::types::HealthStatus::ready()
                }
            }
        }
    }
}

/// Generate a method call expression adapted to the handler's parameter count.
fn make_handler_call(handler: &FoundEventHandler, has_payload: bool) -> TokenStream {
    let method = &handler.method_name;

    let call = if has_payload {
        match handler.param_count {
            0 => quote! { self.#method() },
            1 => quote! { self.#method(ctx) },
            _ => quote! { self.#method(payload, ctx) },
        }
    } else {
        match handler.param_count {
            0 => quote! { self.#method() },
            _ => quote! { self.#method(ctx) },
        }
    };

    if handler.returns_result {
        call
    } else {
        quote! { { #call; Ok(()) } }
    }
}

/// Generate event handler trait impls for all four categories
/// (`DaemonEventHandler`, `TaskEventHandler`, `PluginEventHandler`,
/// `SampleEventHandler`), routing each variant to the user's handler or
/// falling back to a no-op for unhandled variants.
fn generate_event_handlers(struct_ty: &syn::Type, handlers: &[FoundEventHandler]) -> TokenStream {
    // Group handlers by category
    let mut daemon_handlers = Vec::new();
    let mut task_handlers = Vec::new();
    let mut plugin_handlers = Vec::new();
    let mut sample_handlers = Vec::new();

    for h in handlers {
        match h.category {
            EventCategory::Daemon => daemon_handlers.push(h),
            EventCategory::Task => task_handlers.push(h),
            EventCategory::Plugin => plugin_handlers.push(h),
            EventCategory::Sample => sample_handlers.push(h),
        }
    }

    let daemon_impl = generate_daemon_event_impl(struct_ty, &daemon_handlers);
    let task_impl = generate_task_event_impl(struct_ty, &task_handlers);
    let plugin_impl = generate_plugin_event_impl(struct_ty, &plugin_handlers);
    let sample_impl = generate_sample_event_impl(struct_ty, &sample_handlers);

    quote! {
        #daemon_impl
        #task_impl
        #plugin_impl
        #sample_impl
    }
}

fn generate_daemon_event_impl(
    struct_ty: &syn::Type,
    handlers: &[&FoundEventHandler],
) -> TokenStream {
    if handlers.is_empty() {
        return quote! {
            impl malbox_plugin_sdk::internal::DaemonEventHandler for #struct_ty {
                fn __handle_daemon_event(
                    &self,
                    _event: malbox_plugin_sdk::prelude::DaemonEvent,
                    _ctx: &malbox_plugin_sdk::context::Context,
                ) -> malbox_plugin_sdk::error::Result<()> {
                    Ok(())
                }
            }
        };
    }

    let arms: Vec<_> = handlers
        .iter()
        .map(|h| {
            let variant = &h.variant;
            let call = make_handler_call(h, false);
            quote! { malbox_plugin_sdk::prelude::DaemonEvent::#variant => #call }
        })
        .collect();

    quote! {
        impl malbox_plugin_sdk::internal::DaemonEventHandler for #struct_ty {
            fn __handle_daemon_event(
                &self,
                event: malbox_plugin_sdk::prelude::DaemonEvent,
                ctx: &malbox_plugin_sdk::context::Context,
            ) -> malbox_plugin_sdk::error::Result<()> {
                match event {
                    #(#arms,)*
                    _ => Ok(()),
                }
            }
        }
    }
}

fn generate_task_event_impl(struct_ty: &syn::Type, handlers: &[&FoundEventHandler]) -> TokenStream {
    if handlers.is_empty() {
        return quote! {
            impl malbox_plugin_sdk::internal::TaskEventHandler for #struct_ty {
                fn __handle_task_lifecycle_event(
                    &self,
                    _event: malbox_plugin_sdk::prelude::TaskEvent,
                    _payload: malbox_plugin_sdk::prelude::TaskEventPayload,
                    _ctx: &malbox_plugin_sdk::context::Context,
                ) -> malbox_plugin_sdk::error::Result<()> {
                    Ok(())
                }
            }
        };
    }

    let arms: Vec<_> = handlers
        .iter()
        .map(|h| {
            let variant = &h.variant;
            let call = make_handler_call(h, true);
            quote! { malbox_plugin_sdk::prelude::TaskEvent::#variant => #call }
        })
        .collect();

    quote! {
        impl malbox_plugin_sdk::internal::TaskEventHandler for #struct_ty {
            fn __handle_task_lifecycle_event(
                &self,
                event: malbox_plugin_sdk::prelude::TaskEvent,
                payload: malbox_plugin_sdk::prelude::TaskEventPayload,
                ctx: &malbox_plugin_sdk::context::Context,
            ) -> malbox_plugin_sdk::error::Result<()> {
                match event {
                    #(#arms,)*
                    _ => Ok(()),
                }
            }
        }
    }
}

fn generate_plugin_event_impl(
    struct_ty: &syn::Type,
    handlers: &[&FoundEventHandler],
) -> TokenStream {
    if handlers.is_empty() {
        return quote! {
            impl malbox_plugin_sdk::internal::PluginEventHandler for #struct_ty {
                fn __handle_plugin_event(
                    &self,
                    _event: malbox_plugin_sdk::prelude::PluginEvent,
                    _payload: malbox_plugin_sdk::prelude::PluginEventPayload,
                    _ctx: &malbox_plugin_sdk::context::Context,
                ) -> malbox_plugin_sdk::error::Result<()> {
                    Ok(())
                }
            }
        };
    }

    let arms: Vec<_> = handlers
        .iter()
        .map(|h| {
            let variant = &h.variant;
            let call = make_handler_call(h, true);
            quote! { malbox_plugin_sdk::prelude::PluginEvent::#variant => #call }
        })
        .collect();

    quote! {
        impl malbox_plugin_sdk::internal::PluginEventHandler for #struct_ty {
            fn __handle_plugin_event(
                &self,
                event: malbox_plugin_sdk::prelude::PluginEvent,
                payload: malbox_plugin_sdk::prelude::PluginEventPayload,
                ctx: &malbox_plugin_sdk::context::Context,
            ) -> malbox_plugin_sdk::error::Result<()> {
                match event {
                    #(#arms,)*
                    _ => Ok(()),
                }
            }
        }
    }
}

fn generate_sample_event_impl(
    struct_ty: &syn::Type,
    handlers: &[&FoundEventHandler],
) -> TokenStream {
    if handlers.is_empty() {
        return quote! {
            impl malbox_plugin_sdk::internal::SampleEventHandler for #struct_ty {
                fn __handle_sample_event(
                    &self,
                    _event: malbox_plugin_sdk::prelude::SampleEvent,
                    _payload: malbox_plugin_sdk::prelude::SampleEventPayload,
                    _ctx: &malbox_plugin_sdk::context::Context,
                ) -> malbox_plugin_sdk::error::Result<()> {
                    Ok(())
                }
            }
        };
    }

    let arms: Vec<_> = handlers
        .iter()
        .map(|h| {
            let variant = &h.variant;
            let call = make_handler_call(h, true);
            quote! { malbox_plugin_sdk::prelude::SampleEvent::#variant => #call }
        })
        .collect();

    quote! {
        impl malbox_plugin_sdk::internal::SampleEventHandler for #struct_ty {
            fn __handle_sample_event(
                &self,
                event: malbox_plugin_sdk::prelude::SampleEvent,
                payload: malbox_plugin_sdk::prelude::SampleEventPayload,
                ctx: &malbox_plugin_sdk::context::Context,
            ) -> malbox_plugin_sdk::error::Result<()> {
                match event {
                    #(#arms,)*
                    _ => Ok(()),
                }
            }
        }
    }
}
