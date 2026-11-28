//! Fundamental C-ABI types shared across all FFI modules.
//!
//! This module contains the enums, structs, opaque types, and the plugin vtable
//! that form the stable C ABI of the Malbox plugin SDK.  Every type here is
//! `#[repr(C)]` (or `#[repr(u8)]`/`#[repr(i32)]`) so that the C compiler and
//! the Rust compiler agree on their layout.
//!
//! Opaque types ([`MalboxContext`], [`MalboxResultBuilder`]) are
//! declared as empty enums; C code sees them only through pointers and must
//! never dereference them directly.

/// C-ABI enums for plugin lifetime and scheduling policies.
pub mod enums;
/// C-ABI structs for plugin metadata, health status and guest runtime configuration.
pub mod structs;
/// Plugin vtables, opaque handle types and the ABI version constant.
pub mod vtable;

pub use enums::{MalboxExecutionContext, MalboxPluginState};
pub use structs::{
    MalboxAutoCollectConfig, MalboxAutoCollectRule, MalboxGuestRuntimeConfig, MalboxHealthStatus,
    MalboxPluginMeta,
};
pub use vtable::{
    MALBOX_ABI_VERSION, MalboxContext, MalboxGuestPluginVtable, MalboxPluginVtable,
    MalboxResultBuilder,
};
