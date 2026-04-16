//! Rust FFI shim that bridges the Malbox plugin SDK to C/C++ plugin authors.
//!
//! # Architecture
//!
//! The crate is arranged in three layers:
//!
//! 1. **Rust FFI shim** (this crate): exposes a stable `extern "C"` ABI that
//!    the C compiler can link against.  Every public symbol is a `#[no_mangle]`
//!    function whose name begins with `malbox_`.
//!
//! 2. **C header** (`malbox_plugin_sdk.h`, generated from this crate): provides
//!    C declarations for all symbols exported by this crate, plus the
//!    `MalboxPluginVtable` struct that C++ plugins must fill in.
//!
//! 3. **C++20 wrappers** (`malbox_plugin.hpp`): thin RAII wrappers around the
//!    C types and convenience macros that generate the plugin entry point.
//!
//! # Plugin author workflow
//!
//! A C++ plugin author:
//!
//! 1. Includes `malbox_plugin.hpp` and derives from `malbox::Plugin`.
//! 2. Overrides the desired handler methods (`on_task`, `on_start`, etc.).
//! 3. Uses the `MALBOX_PLUGIN_MAIN` macro to generate `main()`, which calls
//!    [`malbox_run_host_plugin`](ffi_runtime::malbox_run_host_plugin) or
//!    [`malbox_run_guest_plugin`](ffi_runtime::malbox_run_guest_plugin).
//! 4. Links against the shared library built from this crate.
//!
//! # Error handling
//!
//! All `extern "C"` functions follow a common convention: they return `0` on
//! success and `-1` on failure.  When a failure occurs the error message is
//! stored in a thread-local slot and can be retrieved by calling
//! [`malbox_last_error`](error::malbox_last_error).
//!
//! # ABI stability
//!
//! The ABI version is tracked by [`MALBOX_ABI_VERSION`](ffi_types::MALBOX_ABI_VERSION).
//! The runtime entry points reject vtables whose `abi_version` field does not
//! match this constant.

pub mod error;
pub mod ffi_callbacks;
pub mod ffi_context;
pub mod ffi_events;
pub mod ffi_exec;
pub mod ffi_log;
pub mod ffi_result;
pub mod ffi_runtime;
pub mod ffi_task;
pub mod ffi_test;
pub mod ffi_types;
