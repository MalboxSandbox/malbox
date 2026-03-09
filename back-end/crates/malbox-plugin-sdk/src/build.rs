//! Build-time helpers for plugin manifest generation.
//!
//! Plugin authors call `malbox_plugin_sdk::build::generate_manifest()` in their
//! `build.rs` to emit a `plugin.toml` alongside their binary.

/// Generate a `plugin.toml` manifest from the compile-time metadata.
///
/// Call this from your `build.rs`:
/// ```ignore
/// fn main() {
///     malbox_plugin_sdk::build::generate_manifest();
/// }
/// ```
///
/// The manifest is written to `$OUT_DIR/plugin.toml`. A post-build step
/// or `cargo malbox package` copies it next to the final binary.
pub fn generate_manifest() {
    // The actual metadata comes from the proc macro at compile time.
    // For the build.rs helper, we emit a placeholder that gets populated
    // by the binary's generated code at build time.
    //
    // In the v1 approach, the proc macro embeds manifest data as a const,
    // and we provide a simple binary that dumps it. For now, this is a
    // no-op placeholder — the proc macro generates the manifest const
    // directly.
    println!("cargo::rerun-if-changed=build.rs");
}
