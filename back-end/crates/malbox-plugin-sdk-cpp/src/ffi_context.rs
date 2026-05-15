//! FFI context functions.
//!
//! These functions provide the C API for interacting with the plugin execution
//! context. The `MalboxContext*` pointer passed to plugin callbacks is actually
//! a `*const Context` created on the Rust side (in `ffi_callbacks.rs`) and
//! cast through the opaque `MalboxContext` enum.

use crate::error::{clear_last_error, result_to_rc, set_last_error};
use crate::ffi_events::{MalboxEvent, c_event_to_rust};
use crate::ffi_types::MalboxContext;
use malbox_plugin_sdk::context::Context;
use std::ffi::c_char;

/// Cast an opaque `MalboxContext*` back to a Rust `Context` reference.
///
/// # Safety
/// `ptr` must have been created from a `&Context` in `ffi_callbacks.rs`
/// and must remain valid for the duration of the callback.
unsafe fn ctx_ref<'a>(ptr: *const MalboxContext) -> Option<&'a Context> {
    if ptr.is_null() {
        set_last_error("null context pointer");
        return None;
    }
    // SAFETY: MalboxContext is an opaque stand-in for Context;
    // the runtime always passes the original *const Context cast to *const MalboxContext.
    Some(unsafe { &*(ptr as *const Context) })
}

/// Clone a Context, incrementing its internal Arc refcount.
///
/// Returns a heap-allocated `Context` that the caller owns. The caller must
/// eventually pass it to [`malbox_context_drop`] to release it.
///
/// # Safety
/// `ctx` must be a valid `*const Context` cast to `*const MalboxContext`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_context_clone(ctx: *const MalboxContext) -> *mut MalboxContext {
    if ctx.is_null() {
        return std::ptr::null_mut();
    }
    let context = unsafe { &*(ctx as *const Context) };
    let cloned = Box::new(context.clone());
    Box::into_raw(cloned) as *mut MalboxContext
}

/// Drop a heap-allocated Context previously returned by [`malbox_context_clone`].
///
/// # Safety
/// `ctx` must have been returned by `malbox_context_clone` (i.e., a
/// heap-allocated `Box<Context>`). Passing a runtime-owned pointer
/// (the one passed to callbacks) is undefined behavior.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_context_drop(ctx: *mut MalboxContext) {
    if ctx.is_null() {
        return;
    }
    let _ = unsafe { Box::from_raw(ctx as *mut Context) };
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

    let (rc, _) = result_to_rc(context.progress(progress, msg));
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
    rc
}

/// Mark a file path as already collected so auto-collection skips it.
///
/// Call this when your plugin reads a file from the artifacts directory and
/// sends its own processed version as a result. Without this, the
/// auto-collector would send the raw file as a duplicate.
///
/// Returns `0` on success, `-1` on failure (last error is set).
///
/// # Safety
///
/// - `ctx` must be a valid `*const Context` cast to `*const MalboxContext`, as
///   provided by the runtime to plugin callbacks.
/// - `path` must point to a valid null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_context_mark_collected(
    ctx: *const MalboxContext,
    path: *const c_char,
) -> i32 {
    let context = match unsafe { ctx_ref(ctx) } {
        Some(c) => c,
        None => return -1,
    };

    if path.is_null() {
        set_last_error("path must not be null");
        return -1;
    }

    let path_str = match unsafe { std::ffi::CStr::from_ptr(path) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(&format!("path is not valid UTF-8: {e}"));
            return -1;
        }
    };

    context.mark_collected(std::path::Path::new(path_str));
    clear_last_error();
    0
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
    use malbox_plugin_sdk::result::PluginResult;

    let context = match unsafe { ctx_ref(ctx) } {
        Some(c) => c,
        None => return -1,
    };

    if count == 0 {
        return 0;
    }

    if names.is_null() || datas.is_null() || data_lens.is_null() || formats.is_null() {
        set_last_error("array pointers must not be null when count > 0");
        return -1;
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
        let (rc, _) = result_to_rc(context.results().push(result));
        if rc != 0 {
            return rc;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffi_events::MalboxEventTag;
    use crate::ffi_types::MalboxContext;
    use malbox_plugin_sdk::context::Context;
    use std::ffi::CString;
    use std::sync::Arc;

    fn noop_emitter() -> Arc<dyn malbox_plugin_transport::traits::TransportEmitter + Send + Sync> {
        Arc::new(())
    }

    fn make_ctx_ptr(ctx: &Context) -> *const MalboxContext {
        (ctx as *const Context) as *const MalboxContext
    }

    #[test]
    fn emit_progress_succeeds() {
        let ctx = Context::test_new(noop_emitter());
        let msg = CString::new("halfway").unwrap();
        let rc = unsafe { malbox_context_emit_progress(make_ctx_ptr(&ctx), 0.5, msg.as_ptr()) };
        assert_eq!(rc, 0);
    }

    #[test]
    fn emit_progress_null_message_succeeds() {
        let ctx = Context::test_new(noop_emitter());
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
        let ctx = Context::test_new(noop_emitter());
        let event = MalboxEvent {
            tag: MalboxEventTag::TaskCreated,
            id: 1,
            source: std::ptr::null(),
            result_name: std::ptr::null(),
        };
        let rc = unsafe { malbox_context_emit_event(make_ctx_ptr(&ctx), event) };
        assert_eq!(rc, 0);
    }

    #[test]
    fn emit_event_daemon_succeeds() {
        let ctx = Context::test_new(noop_emitter());
        let event = MalboxEvent {
            tag: MalboxEventTag::DaemonShutdown,
            id: 0,
            source: std::ptr::null(),
            result_name: std::ptr::null(),
        };
        let rc = unsafe { malbox_context_emit_event(make_ctx_ptr(&ctx), event) };
        assert_eq!(rc, 0);
    }

    #[test]
    fn emit_event_null_context_returns_minus_one() {
        let event = MalboxEvent {
            tag: MalboxEventTag::TaskCreated,
            id: 0,
            source: std::ptr::null(),
            result_name: std::ptr::null(),
        };
        let rc = unsafe { malbox_context_emit_event(std::ptr::null(), event) };
        assert_eq!(rc, -1);
    }

    #[test]
    fn warn_succeeds() {
        let ctx = Context::test_new(noop_emitter());
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
        let ctx = Context::test_new(noop_emitter());
        let rc = unsafe { malbox_context_warn(make_ctx_ptr(&ctx), std::ptr::null()) };
        assert_eq!(rc, -1);
    }
}
