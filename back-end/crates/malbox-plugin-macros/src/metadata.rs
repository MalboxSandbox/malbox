use proc_macro2::Span;
use syn::LitStr;

/// Whether the plugin runs on the daemon host or inside a guest VM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginKind {
    /// IPC-based plugin that runs directly on the daemon host.
    Host,
    /// gRPC-based plugin that runs inside a guest VM/container.
    Guest,
}

/// Parsed metadata from `#[malbox(state = "...", execution = "...")]`.
///
/// Generic package metadata (name, version, description, authors) is read from
/// `Cargo.toml` at compile time via `env!()` macros in the generated code,
/// so the macro only needs plugin-specific attributes.
#[derive(Debug)]
pub struct PluginMetadata {
    pub state: String,
    pub execution: String,
}

impl PluginMetadata {
    /// Parse metadata from a list of `#[malbox(...)]` attributes on the struct.
    pub fn from_attrs(attrs: &[syn::Attribute]) -> syn::Result<Self> {
        let mut state = None;
        let mut execution = None;

        for attr in attrs {
            if !attr.path().is_ident("malbox") {
                continue;
            }

            attr.parse_nested_meta(|meta| {
                let ident = meta.path.get_ident().ok_or_else(|| {
                    syn::Error::new_spanned(&meta.path, "expected identifier")
                })?;

                match ident.to_string().as_str() {
                    "state" => {
                        let value = meta.value()?;
                        let lit: LitStr = value.parse()?;
                        let s = lit.value();
                        match s.as_str() {
                            "persistent" | "ephemeral" | "scoped" => {}
                            _ => {
                                return Err(syn::Error::new(
                                    lit.span(),
                                    format!(
                                        "invalid state '{}': expected persistent, ephemeral, or scoped",
                                        s
                                    ),
                                ));
                            }
                        }
                        state = Some(s);
                    }
                    "execution" => {
                        let value = meta.value()?;
                        let lit: LitStr = value.parse()?;
                        let s = lit.value();
                        match s.as_str() {
                            "exclusive" | "sequential" | "parallel" | "unrestricted" => {}
                            _ => {
                                return Err(syn::Error::new(
                                    lit.span(),
                                    format!(
                                        "invalid execution '{}': expected exclusive, sequential, parallel, or unrestricted",
                                        s
                                    ),
                                ));
                            }
                        }
                        execution = Some(s);
                    }
                    other => {
                        return Err(syn::Error::new(
                            ident.span(),
                            format!("unknown attribute '{}'", other),
                        ));
                    }
                }

                Ok(())
            })?;
        }

        Ok(PluginMetadata {
            state: state.ok_or_else(|| {
                syn::Error::new(Span::call_site(), "missing required attribute: state")
            })?,
            execution: execution.ok_or_else(|| {
                syn::Error::new(Span::call_site(), "missing required attribute: execution")
            })?,
        })
    }
}
