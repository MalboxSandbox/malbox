use std::ffi::c_char;

use super::structs::MalboxHealthStatus;
use crate::ffi_events::MalboxEvent;

/// Current ABI version of the Malbox plugin SDK.
///
/// The runtime entry points ([`malbox_run_host_plugin`](crate::ffi_runtime::malbox_run_host_plugin),
/// [`malbox_run_guest_plugin`](crate::ffi_runtime::malbox_run_guest_plugin)) reject
/// vtables whose `abi_version` field does not equal this constant.  Increment
/// this value whenever the vtable layout or calling convention changes in a
/// backward-incompatible way.
pub const MALBOX_ABI_VERSION: u32 = 5;

/// Opaque handle representing a [`malbox_plugin_sdk::types::Task`].
///
/// C/C++ code must only access a `MalboxTask` through the `malbox_task_*`
/// accessor functions; it must never dereference the pointer directly.
pub enum MalboxTask {}

/// Opaque handle representing a [`malbox_plugin_sdk::context::Context`].
///
/// C/C++ code must only access a `MalboxContext` through the
/// `malbox_context_*` functions; it must never dereference the pointer
/// directly.
pub enum MalboxContext {}

/// Opaque handle representing execution info returned by wait_for_execution.
pub enum MalboxExecutionInfo {}

/// Opaque handle representing a `ResultBuilder` (defined in `ffi_result`).
///
/// C/C++ code must only interact with a `MalboxResultBuilder` through the
/// `malbox_result_builder_*` functions; it must never dereference the pointer
/// directly.
pub enum MalboxResultBuilder {}

/// Opaque handle representing an execute command request.
///
/// C/C++ code accesses fields via the `malbox_exec_request_*` accessor
/// functions; it must never dereference the pointer directly.
pub enum MalboxExecRequest {}

/// Opaque handle representing an execute command result (filled by plugin).
///
/// C/C++ code fills fields via the `malbox_exec_result_*` setter
/// functions; it must never dereference the pointer directly.
pub enum MalboxExecResult {}

/// Function-pointer table filled in by a C++ plugin and passed to a runtime
/// entry point ([`malbox_run_host_plugin`](crate::ffi_runtime::malbox_run_host_plugin) or
/// [`malbox_run_guest_plugin`](crate::ffi_runtime::malbox_run_guest_plugin)).
///
/// Each `Option` field may be `None` (null function pointer) to opt out of
/// that callback; the runtime treats a missing callback as a successful no-op.
///
/// All callbacks receive `plugin_ptr` as their first argument, which is the
/// C++ `this` pointer (or any other user data the plugin author chooses).
/// Every callback returns `0` on success or a non-zero value on failure; on
/// failure the callback should first call
/// [`set_last_error`](crate::error::malbox_last_error) (or the equivalent C
/// macro) so that the error message can be retrieved by the runtime.
#[repr(C)]
pub struct MalboxPluginVtable {
    /// Must equal [`MALBOX_ABI_VERSION`]; the runtime rejects mismatches.
    pub abi_version: u32,
    /// Opaque pointer passed as the first argument to every callback (typically
    /// the C++ plugin object pointer).
    pub plugin_ptr: *mut std::ffi::c_void,

    /// Called when the daemon dispatches a task to this plugin.
    ///
    /// Arguments: `plugin_ptr`, task handle, context handle, result builder.
    pub on_task: Option<
        unsafe extern "C" fn(
            *mut std::ffi::c_void,
            *const MalboxTask,
            *const MalboxContext,
            *mut MalboxResultBuilder,
        ) -> i32,
    >,
    /// Called once when the plugin is started with its configuration.
    ///
    /// Arguments: `plugin_ptr`, array of key strings, array of value strings,
    /// number of entries.
    pub on_start: Option<
        unsafe extern "C" fn(
            *mut std::ffi::c_void,
            *const *const c_char,
            *const *const c_char,
            usize,
        ) -> i32,
    >,
    /// Called when the plugin is stopped.
    ///
    /// Argument: `plugin_ptr`.
    pub on_stop: Option<unsafe extern "C" fn(*mut std::ffi::c_void) -> i32>,
    /// Called periodically to check whether the plugin is healthy.
    ///
    /// Arguments: `plugin_ptr`, pointer to a [`MalboxHealthStatus`] to fill in.
    pub health_check:
        Option<unsafe extern "C" fn(*mut std::ffi::c_void, *mut MalboxHealthStatus) -> i32>,
    /// Called when a system event occurs.
    ///
    /// Arguments: `plugin_ptr`, flat event (tag + id), context handle.
    pub on_event: Option<
        unsafe extern "C" fn(*mut std::ffi::c_void, MalboxEvent, *const MalboxContext) -> i32,
    >,
    /// Called when the daemon requests command execution on the guest.
    ///
    /// Arguments: `plugin_ptr`, request handle (access via `malbox_exec_request_*`
    /// accessor functions), result handle (fill via `malbox_exec_result_*` setter
    /// functions).
    /// Returns 0 to use runtime default, 1 if plugin handled it, -1 on error.
    pub on_execute_command: Option<
        unsafe extern "C" fn(
            *mut std::ffi::c_void,    // plugin_ptr
            *const MalboxExecRequest, // request handle
            *mut MalboxExecResult,    // result handle
        ) -> i32,
    >,
}

impl Default for MalboxPluginVtable {
    fn default() -> Self {
        Self {
            abi_version: 0,
            plugin_ptr: std::ptr::null_mut(),
            on_task: None,
            on_start: None,
            on_stop: None,
            health_check: None,
            on_event: None,
            on_execute_command: None,
        }
    }
}

/// Function-pointer table for a C++ guest plugin.
#[repr(C)]
pub struct MalboxGuestPluginVtable {
    pub abi_version: u32,
    pub plugin_ptr: *mut std::ffi::c_void,
    pub on_start: Option<
        unsafe extern "C" fn(*mut std::ffi::c_void, *const MalboxTask, *const MalboxContext) -> i32,
    >,
    pub on_stop: Option<unsafe extern "C" fn(*mut std::ffi::c_void, *const MalboxContext) -> i32>,
    pub execute_sample: Option<unsafe extern "C" fn(*mut std::ffi::c_void, *const c_char) -> i32>,
    pub health_check:
        Option<unsafe extern "C" fn(*mut std::ffi::c_void, *mut MalboxHealthStatus) -> i32>,
}

impl Default for MalboxGuestPluginVtable {
    fn default() -> Self {
        Self {
            abi_version: 0,
            plugin_ptr: std::ptr::null_mut(),
            on_start: None,
            on_stop: None,
            execute_sample: None,
            health_check: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vtable_default() {
        let vtable = MalboxPluginVtable::default();
        assert_eq!(vtable.abi_version, 0);
        assert!(vtable.plugin_ptr.is_null());
        assert!(vtable.on_task.is_none());
        assert!(vtable.on_event.is_none());
        assert!(vtable.on_execute_command.is_none());
    }
}
