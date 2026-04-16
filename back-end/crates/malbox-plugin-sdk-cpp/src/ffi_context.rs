//! FFI context functions.
//!
//! These functions provide the C API for interacting with the plugin execution
//! context. The `MalboxContext*` pointer passed to plugin callbacks is actually
//! a `*const Context<'_>` created on the Rust side (in `ffi_callbacks.rs`) and
//! cast through the opaque `MalboxContext` enum.

use crate::error::{clear_last_error, result_to_rc, set_last_error};
use crate::ffi_events::{MalboxEvent, c_event_to_rust};
use crate::ffi_types::MalboxContext;
use malbox_plugin_sdk::context::Context;
use std::ffi::c_char;

/// Cast an opaque `MalboxContext*` back to a Rust `Context` reference.
///
/// # Safety
/// `ptr` must have been created from a `&Context<'a>` in `ffi_callbacks.rs`
/// and must remain valid for the duration of the callback.
unsafe fn ctx_ref<'a>(ptr: *const MalboxContext) -> Option<&'a Context<'a>> {
    if ptr.is_null() {
        set_last_error("null context pointer");
        return None;
    }
    // SAFETY: MalboxContext is an opaque stand-in for Context<'a>;
    // the runtime always passes the original *const Context cast to *const MalboxContext.
    Some(unsafe { &*(ptr as *const Context<'a>) })
}

/// Report execution progress (0.0-1.0) with an optional status message.
///
/// `progress` should be in the range `[0.0, 1.0]`.  `message` may be null,
/// in which case an empty string is used.
///
/// Returns `0` on success, `-1` on failure (last error is set).
///
/// # Safety
///
/// - `ctx` must be a valid `*const Context` cast to `*const MalboxContext`, as
///   provided by the runtime to plugin callbacks.
/// - `message`, if non-null, must point to a valid null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_context_emit_progress(
    ctx: *const MalboxContext,
    progress: f64,
    message: *const c_char,
) -> i32 {
    let context = match unsafe { ctx_ref(ctx) } {
        Some(c) => c,
        None => return -1,
    };

    let msg = if message.is_null() {
        ""
    } else {
        match unsafe { std::ffi::CStr::from_ptr(message) }.to_str() {
            Ok(s) => s,
            Err(e) => {
                set_last_error(&format!("message is not valid UTF-8: {e}"));
                return -1;
            }
        }
    };

    let (rc, _) = result_to_rc(context.emit_progress(progress, msg));
    rc
}

/// Emit a flat event back to the daemon.
///
/// The `event` struct carries a tag discriminant and an integer ID.
///
/// Returns `0` on success, `-1` on failure (last error is set).
///
/// # Safety
///
/// `ctx` must be a valid `*const Context` cast to `*const MalboxContext`, as
/// provided by the runtime to plugin callbacks.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_context_emit_event(
    ctx: *const MalboxContext,
    event: MalboxEvent,
) -> i32 {
    let context = match unsafe { ctx_ref(ctx) } {
        Some(c) => c,
        None => return -1,
    };

    let rust_event = match c_event_to_rust(&event) {
        Ok(e) => e,
        Err(()) => {
            set_last_error("unrecognised event tag");
            return -1;
        }
    };

    let (rc, _) = result_to_rc(context.emit_event(rust_event));
    rc
}

/// Log a warning message that will be attached to the current task report.
///
/// `message` must be non-null and point to a valid null-terminated UTF-8 string.
///
/// Returns `0` on success, `-1` on failure (last error is set).
///
/// # Safety
///
/// - `ctx` must be a valid `*const Context` cast to `*const MalboxContext`, as
///   provided by the runtime to plugin callbacks.
/// - `message` must point to a valid null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_context_warn(
    ctx: *const MalboxContext,
    message: *const c_char,
) -> i32 {
    let context = match unsafe { ctx_ref(ctx) } {
        Some(c) => c,
        None => return -1,
    };

    if message.is_null() {
        set_last_error("message must not be null");
        return -1;
    }

    let msg = match unsafe { std::ffi::CStr::from_ptr(message) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(&format!("message is not valid UTF-8: {e}"));
            return -1;
        }
    };

    let (rc, _) = result_to_rc(context.warn(msg));
    clear_last_error();
    rc
}

/// Flush a batch of results to the daemon immediately during on_task.
///
/// `names`, `datas`, `data_lens`, and `formats` are parallel arrays of length `count`.
/// Each entry describes one result to flush.
/// `formats`: 1 = JSON, 2 = Bytes, 3 = File (data is a null-terminated path string).
///
/// Returns `0` on success, `-1` on failure.
///
/// # Safety
/// - `ctx` must be valid. All arrays must have `count` elements.
/// - Each `names[i]` must be null-terminated. Each `datas[i]` must point to `data_lens[i]` bytes.
/// - For format 3 (File), `datas[i]` must point to a null-terminated path string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_context_flush_results(
    ctx: *const MalboxContext,
    names: *const *const c_char,
    datas: *const *const u8,
    data_lens: *const usize,
    formats: *const i32,
    count: usize,
) -> i32 {
    use malbox_plugin_sdk::types::PluginResult;

    let context = match unsafe { ctx_ref(ctx) } {
        Some(c) => c,
        None => return -1,
    };

    if count == 0 {
        return 0;
    }

    let mut results = Vec::with_capacity(count);
    for i in 0..count {
        let name_ptr = unsafe { *names.add(i) };
        let name = match unsafe { std::ffi::CStr::from_ptr(name_ptr) }.to_str() {
            Ok(s) => s.to_string(),
            Err(e) => {
                set_last_error(&format!("result name at index {i} is not valid UTF-8: {e}"));
                return -1;
            }
        };

        let format = unsafe { *formats.add(i) };
        let result = if format == 3 {
            // File format: data is a null-terminated path string
            let data_ptr = unsafe { *datas.add(i) };
            if data_ptr.is_null() {
                set_last_error(&format!("result data at index {i} is null for File format"));
                return -1;
            }
            let path_str =
                match unsafe { std::ffi::CStr::from_ptr(data_ptr as *const c_char) }.to_str() {
                    Ok(s) => s,
                    Err(e) => {
                        set_last_error(&format!(
                            "result file path at index {i} is not valid UTF-8: {e}"
                        ));
                        return -1;
                    }
                };
            PluginResult::File {
                name,
                path: std::path::PathBuf::from(path_str),
            }
        } else {
            let data_ptr = unsafe { *datas.add(i) };
            let data_len = unsafe { *data_lens.add(i) };
            let data = if data_len > 0 && !data_ptr.is_null() {
                unsafe { std::slice::from_raw_parts(data_ptr, data_len) }.to_vec()
            } else {
                vec![]
            };

            if format == 1 {
                PluginResult::Json { name, data }
            } else {
                PluginResult::Bytes { name, data }
            }
        };
        results.push(result);
    }

    for result in results {
        let (rc, _) = result_to_rc(context.push_result(result));
        if rc != 0 {
            return rc;
        }
    }
    0
}

/// Block until sample execution begins. Returns execution metadata.
///
/// On success, writes PID to `*out_pid` (or 0 if no PID), command to `*out_command`
/// (pointer valid until next FFI call on this thread), args array to `*out_args`
/// with count in `*out_args_count`, and returns 0.
/// Returns -1 on timeout or if not in an on_task context.
///
/// `out_args` and `out_args_count` may be null if the caller does not need
/// the argument list.
///
/// # Safety
/// `ctx` must be valid. `out_pid` and `out_command` must be valid writable pointers.
/// `out_args` and `out_args_count`, if non-null, must be valid writable pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_context_wait_for_execution(
    ctx: *const MalboxContext,
    out_pid: *mut u32,
    out_command: *mut *const c_char,
    out_args: *mut *const *const c_char,
    out_args_count: *mut usize,
) -> i32 {
    let context = match unsafe { ctx_ref(ctx) } {
        Some(c) => c,
        None => return -1,
    };

    match context.wait_for_execution() {
        Ok(info) => unsafe {
            write_execution_info(info, out_pid, out_command, out_args, out_args_count)
        },
        Err(e) => {
            set_last_error(&e.to_string());
            -1
        }
    }
}

/// Block until sample execution begins, with a custom timeout in milliseconds.
///
/// Same as `malbox_context_wait_for_execution` but with explicit timeout.
///
/// # Safety
/// Same as `malbox_context_wait_for_execution`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_context_wait_for_execution_timeout(
    ctx: *const MalboxContext,
    timeout_ms: u64,
    out_pid: *mut u32,
    out_command: *mut *const c_char,
    out_args: *mut *const *const c_char,
    out_args_count: *mut usize,
) -> i32 {
    let context = match unsafe { ctx_ref(ctx) } {
        Some(c) => c,
        None => return -1,
    };

    match context.wait_for_execution_timeout(std::time::Duration::from_millis(timeout_ms)) {
        Ok(info) => unsafe {
            write_execution_info(info, out_pid, out_command, out_args, out_args_count)
        },
        Err(e) => {
            set_last_error(&e.to_string());
            -1
        }
    }
}

/// Thread-local storage for execution info strings returned by wait_for_execution.
///
/// Stores the CString for the command and the CStrings + pointer array for args
/// so that the pointers remain valid until the next call.
struct ExecInfoStorage {
    _command: std::ffi::CString,
    _args: Vec<std::ffi::CString>,
    arg_ptrs: Vec<*const c_char>,
}

thread_local! {
    static LAST_EXEC_INFO: std::cell::RefCell<Option<ExecInfoStorage>> =
        const { std::cell::RefCell::new(None) };
}

/// Write execution info fields to the output pointers, storing string data in
/// thread-local storage so pointers remain valid until the next call.
///
/// # Safety
/// All non-null output pointers must be valid and writable.
unsafe fn write_execution_info(
    info: malbox_plugin_sdk::types::ExecutionInfo,
    out_pid: *mut u32,
    out_command: *mut *const c_char,
    out_args: *mut *const *const c_char,
    out_args_count: *mut usize,
) -> i32 {
    if !out_pid.is_null() {
        unsafe { *out_pid = info.pid().unwrap_or(0) };
    }

    let cmd = std::ffi::CString::new(info.command()).unwrap_or_default();
    let args_cstrings: Vec<std::ffi::CString> = info
        .args()
        .iter()
        .map(|a: &String| std::ffi::CString::new(a.as_str()).unwrap_or_default())
        .collect();
    let arg_ptrs: Vec<*const c_char> = args_cstrings
        .iter()
        .map(|cs: &std::ffi::CString| cs.as_ptr())
        .collect();

    LAST_EXEC_INFO.with(|cell| {
        let storage = ExecInfoStorage {
            _command: cmd,
            _args: args_cstrings,
            arg_ptrs,
        };
        let mut borrow = cell.borrow_mut();
        *borrow = Some(storage);
        let storage_ref = borrow.as_ref().unwrap();

        if !out_command.is_null() {
            unsafe { *out_command = storage_ref._command.as_ptr() };
        }
        if !out_args.is_null() {
            if storage_ref.arg_ptrs.is_empty() {
                unsafe { *out_args = std::ptr::null() };
            } else {
                unsafe { *out_args = storage_ref.arg_ptrs.as_ptr() };
            }
        }
        if !out_args_count.is_null() {
            unsafe { *out_args_count = storage_ref.arg_ptrs.len() };
        }
    });

    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffi_events::MalboxEventTag;
    use crate::ffi_types::MalboxContext;
    use malbox_plugin_sdk::context::Context;
    use std::ffi::CString;

    fn make_ctx_ptr<'a>(ctx: &'a Context<'a>) -> *const MalboxContext {
        (ctx as *const Context<'a>) as *const MalboxContext
    }

    #[test]
    fn emit_progress_succeeds() {
        let emitter = ();
        let ctx = Context::test_new(&emitter);
        let msg = CString::new("halfway").unwrap();
        let rc = unsafe { malbox_context_emit_progress(make_ctx_ptr(&ctx), 0.5, msg.as_ptr()) };
        assert_eq!(rc, 0);
    }

    #[test]
    fn emit_progress_null_message_succeeds() {
        let emitter = ();
        let ctx = Context::test_new(&emitter);
        let rc = unsafe { malbox_context_emit_progress(make_ctx_ptr(&ctx), 0.0, std::ptr::null()) };
        assert_eq!(rc, 0);
    }

    #[test]
    fn emit_progress_null_context_returns_minus_one() {
        let rc = unsafe { malbox_context_emit_progress(std::ptr::null(), 0.5, std::ptr::null()) };
        assert_eq!(rc, -1);
    }

    #[test]
    fn emit_event_succeeds() {
        let emitter = ();
        let ctx = Context::test_new(&emitter);
        let event = MalboxEvent {
            tag: MalboxEventTag::TaskCreated,
            id: 1,
        };
        let rc = unsafe { malbox_context_emit_event(make_ctx_ptr(&ctx), event) };
        assert_eq!(rc, 0);
    }

    #[test]
    fn emit_event_daemon_succeeds() {
        let emitter = ();
        let ctx = Context::test_new(&emitter);
        let event = MalboxEvent {
            tag: MalboxEventTag::DaemonShutdown,
            id: 0,
        };
        let rc = unsafe { malbox_context_emit_event(make_ctx_ptr(&ctx), event) };
        assert_eq!(rc, 0);
    }

    #[test]
    fn emit_event_null_context_returns_minus_one() {
        let event = MalboxEvent {
            tag: MalboxEventTag::TaskCreated,
            id: 0,
        };
        let rc = unsafe { malbox_context_emit_event(std::ptr::null(), event) };
        assert_eq!(rc, -1);
    }

    #[test]
    fn warn_succeeds() {
        let emitter = ();
        let ctx = Context::test_new(&emitter);
        let msg = CString::new("watch out").unwrap();
        let rc = unsafe { malbox_context_warn(make_ctx_ptr(&ctx), msg.as_ptr()) };
        assert_eq!(rc, 0);
    }

    #[test]
    fn warn_null_context_returns_minus_one() {
        let msg = CString::new("x").unwrap();
        let rc = unsafe { malbox_context_warn(std::ptr::null(), msg.as_ptr()) };
        assert_eq!(rc, -1);
    }

    #[test]
    fn warn_null_message_returns_minus_one() {
        let emitter = ();
        let ctx = Context::test_new(&emitter);
        let rc = unsafe { malbox_context_warn(make_ctx_ptr(&ctx), std::ptr::null()) };
        assert_eq!(rc, -1);
    }
}
