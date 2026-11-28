//! `GuestVtablePlugin` adapter - bridges the C guest vtable to the Rust `GuestPlugin` trait.

use std::ffi::CStr;
use std::path::Path;

use malbox_plugin_sdk::context::Context;
use malbox_plugin_sdk::error::{Result, SdkError};
use malbox_plugin_sdk::health::HealthStatus;
use malbox_plugin_sdk::plugin::Plugin;
use malbox_plugin_sdk::plugin::guest::{GuestPlugin, LaunchResult, default_launch};

use crate::error::last_error_string;
use crate::ffi_types::{MalboxContext, MalboxGuestPluginVtable, MalboxHealthStatus};

/// Rust guest plugin adapter that dispatches `GuestPlugin` calls to a C vtable.
///
/// A null (absent) function pointer falls back to the trait's default behavior.
pub(crate) struct GuestVtablePlugin {
    vtable: MalboxGuestPluginVtable,
}

impl GuestVtablePlugin {
    /// Wrap a filled-in C guest vtable.
    pub(crate) fn new(vtable: MalboxGuestPluginVtable) -> Self {
        Self { vtable }
    }
}

// SAFETY: The vtable only holds `plugin_ptr` and C function pointers. The SDK
// contract (see `malbox_run_guest_plugin`) requires the C++ plugin object and its
// callbacks to be callable from any thread, so moving the adapter is sound.
unsafe impl Send for GuestVtablePlugin {}
// SAFETY: Same contract as `Send`: the C++ callbacks must tolerate concurrent
// invocation through a shared `&GuestVtablePlugin`; there is no Rust-side state.
unsafe impl Sync for GuestVtablePlugin {}

/// Error carrying the message a C++ guest callback reported via the last-error slot.
#[derive(Debug)]
struct GuestCallbackError(String);

impl std::fmt::Display for GuestCallbackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for GuestCallbackError {}

/// Convert a non-zero callback return code to `SdkError::Plugin`.
fn check_rc(rc: i32) -> Result<()> {
    if rc == 0 {
        Ok(())
    } else {
        let msg = last_error_string().unwrap_or_else(|| "unknown error".into());
        Err(SdkError::Plugin(Box::new(GuestCallbackError(msg))))
    }
}

impl Plugin for GuestVtablePlugin {
    fn health_check(&self) -> HealthStatus {
        let Some(f) = self.vtable.health_check else {
            return HealthStatus::ready();
        };
        let mut status = MalboxHealthStatus {
            ready: true,
            reason: std::ptr::null(),
        };
        // SAFETY: plugin_ptr is valid for the vtable's lifetime per the runtime
        // contract; `status` is a live local, writable for the call duration.
        let rc = unsafe { f(self.vtable.plugin_ptr, &raw mut status) };
        if rc != 0 {
            return HealthStatus::not_ready("health_check callback failed");
        }
        let reason = if status.reason.is_null() {
            String::new()
        } else {
            // SAFETY: a non-null `reason` must point to a NUL-terminated string
            // that stays valid until `health_check` returns to us (vtable contract).
            unsafe { CStr::from_ptr(status.reason) }
                .to_string_lossy()
                .into_owned()
        };
        if status.ready {
            HealthStatus::ready()
        } else {
            HealthStatus::not_ready(reason)
        }
    }
}

impl GuestPlugin for GuestVtablePlugin {
    fn on_start(&self, ctx: &Context) -> Result<()> {
        let Some(f) = self.vtable.on_start else {
            return Ok(());
        };
        let ctx_ptr = std::ptr::from_ref(ctx).cast::<MalboxContext>();
        // SAFETY: ctx_ptr was derived from `&Context`, which outlives the call;
        // plugin_ptr is valid for the vtable's lifetime per the runtime contract.
        let rc = unsafe { f(self.vtable.plugin_ptr, ctx_ptr) };
        check_rc(rc)
    }

    fn on_stop(&self, ctx: &Context) -> Result<()> {
        let Some(f) = self.vtable.on_stop else {
            return Ok(());
        };
        let ctx_ptr = std::ptr::from_ref(ctx).cast::<MalboxContext>();
        // SAFETY: ctx_ptr was derived from `&Context`, which outlives the call;
        // plugin_ptr is valid for the vtable's lifetime per the runtime contract.
        let rc = unsafe { f(self.vtable.plugin_ptr, ctx_ptr) };
        check_rc(rc)
    }

    fn execute_sample(&self, sample_path: &Path) -> Result<LaunchResult> {
        let Some(f) = self.vtable.execute_sample else {
            return Ok(default_launch(sample_path));
        };
        let Some(path_str) = sample_path.to_str() else {
            return Ok(LaunchResult::UseDefault);
        };
        let Ok(c_path) = std::ffi::CString::new(path_str) else {
            return Ok(LaunchResult::UseDefault);
        };
        // SAFETY: c_path is a NUL-terminated CString alive across the call;
        // plugin_ptr is valid for the vtable's lifetime per the runtime contract.
        let rc = unsafe { f(self.vtable.plugin_ptr, c_path.as_ptr()) };
        match rc {
            0 => Ok(LaunchResult::UseDefault),
            1 => Ok(LaunchResult::Launched),
            _ => check_rc(rc).map(|()| LaunchResult::UseDefault),
        }
    }
}
