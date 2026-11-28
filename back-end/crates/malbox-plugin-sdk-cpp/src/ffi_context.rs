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
    Some(unsafe { &*ptr.cast::<Context>() })
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
    // SAFETY: `ctx` is non-null and, per the caller contract, points to a live
    // `Context` disguised as the opaque `MalboxContext`.
    let context = unsafe { &*ctx.cast::<Context>() };
    let cloned = Box::new(context.clone());
    Box::into_raw(cloned).cast::<MalboxContext>()
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
    // SAFETY: the caller guarantees `ctx` came from `Box::into_raw` in
    // `malbox_context_clone` and has not been dropped yet.
    drop(unsafe { Box::from_raw(ctx.cast::<Context>()) });
}

/// Read a nullable NUL-terminated C string as UTF-8, mapping null to `""`.
///
/// Sets the last error and returns `None` if the bytes are not valid UTF-8.
///
/// # Safety
/// `ptr`, if non-null, must point to a valid NUL-terminated C string.
unsafe fn optional_str<'a>(ptr: *const c_char, what: &str) -> Option<&'a str> {
    if ptr.is_null() {
        return Some("");
    }
    // SAFETY: `ptr` is non-null and the caller guarantees it is NUL-terminated.
    match unsafe { std::ffi::CStr::from_ptr(ptr) }.to_str() {
        Ok(s) => Some(s),
        Err(e) => {
            set_last_error(&format!("{what} is not valid UTF-8: {e}"));
            None
        }
    }
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
    // SAFETY: forwarded from this function's caller contract on `ctx`.
    let Some(context) = (unsafe { ctx_ref(ctx) }) else {
        return -1;
    };

    // SAFETY: forwarded from this function's caller contract on `message`.
    let Some(msg) = (unsafe { optional_str(message, "message") }) else {
        return -1;
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
    // SAFETY: forwarded from this function's caller contract on `ctx`.
    let Some(context) = (unsafe { ctx_ref(ctx) }) else {
        return -1;
    };

    let (rc, _) = result_to_rc(context.emit_event(c_event_to_rust(event)));
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
    // SAFETY: forwarded from this function's caller contract on `ctx`.
    let Some(context) = (unsafe { ctx_ref(ctx) }) else {
        return -1;
    };

    // SAFETY: forwarded from this function's caller contract on `message`.
    let Some(msg) = (unsafe { required_str(message, "message") }) else {
        return -1;
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
    // SAFETY: forwarded from this function's caller contract on `ctx`.
    let Some(context) = (unsafe { ctx_ref(ctx) }) else {
        return -1;
    };

    // SAFETY: forwarded from this function's caller contract on `path`.
    let Some(path_str) = (unsafe { required_str(path, "path") }) else {
        return -1;
    };

    context.mark_collected(std::path::Path::new(path_str));
    clear_last_error();
    0
}

/// Build one `PluginResult` from a single entry of the flush arrays.
///
/// Sets the last error and returns `None` on a null or non-UTF-8 field.
///
/// # Safety
/// `name_ptr` must be a valid NUL-terminated C string. For `format == 3`,
/// `data_ptr` (if non-null) must be a NUL-terminated path string; otherwise
/// `data_ptr` (if non-null) must point to `data_len` readable bytes.
unsafe fn flush_entry(
    i: usize,
    name_ptr: *const c_char,
    data_ptr: *const u8,
    data_len: usize,
    format: i32,
) -> Option<malbox_plugin_sdk::result::PluginResult> {
    use malbox_plugin_sdk::result::PluginResult;

    // SAFETY: the caller guarantees `name_ptr` is a valid NUL-terminated string.
    let name = match unsafe { std::ffi::CStr::from_ptr(name_ptr) }.to_str() {
        Ok(s) => s.to_string(),
        Err(e) => {
            set_last_error(&format!("result name at index {i} is not valid UTF-8: {e}"));
            return None;
        }
    };

    if format == 3 {
        // File format: data is a null-terminated path string
        if data_ptr.is_null() {
            set_last_error(&format!("result data at index {i} is null for File format"));
            return None;
        }
        // SAFETY: non-null, and for File format the caller guarantees a
        // NUL-terminated path string.
        let path_str = match unsafe { std::ffi::CStr::from_ptr(data_ptr.cast::<c_char>()) }.to_str()
        {
            Ok(s) => s,
            Err(e) => {
                set_last_error(&format!(
                    "result file path at index {i} is not valid UTF-8: {e}"
                ));
                return None;
            }
        };
        return Some(PluginResult::File {
            name,
            path: std::path::PathBuf::from(path_str),
        });
    }

    let data = if data_len > 0 && !data_ptr.is_null() {
        // SAFETY: non-null and the caller guarantees `data_len` readable bytes.
        unsafe { std::slice::from_raw_parts(data_ptr, data_len) }.to_vec()
    } else {
        vec![]
    };

    Some(if format == 1 {
        PluginResult::Json { name, data }
    } else {
        PluginResult::Bytes { name, data }
    })
}

/// Flush a batch of results to the daemon immediately during `on_task`.
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
    // SAFETY: forwarded from this function's caller contract on `ctx`.
    let Some(context) = (unsafe { ctx_ref(ctx) }) else {
        return -1;
    };

    if count == 0 {
        return 0;
    }

    if names.is_null() || datas.is_null() || data_lens.is_null() || formats.is_null() {
        set_last_error("array pointers must not be null when count > 0");
        return -1;
    }

    // SAFETY: non-null, and the caller guarantees each array has `count` elements.
    let names = unsafe { std::slice::from_raw_parts(names, count) };
    // SAFETY: as above.
    let datas = unsafe { std::slice::from_raw_parts(datas, count) };
    // SAFETY: as above.
    let data_lens = unsafe { std::slice::from_raw_parts(data_lens, count) };
    // SAFETY: as above.
    let formats = unsafe { std::slice::from_raw_parts(formats, count) };

    let mut results = Vec::with_capacity(count);
    for (i, &name_ptr) in names.iter().enumerate() {
        // SAFETY: entry `i` of the caller's arrays satisfies `flush_entry`'s
        // contract per this function's safety requirements.
        let Some(result) =
            (unsafe { flush_entry(i, name_ptr, datas[i], data_lens[i], formats[i]) })
        else {
            return -1;
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

/// Read a non-null, NUL-terminated UTF-8 C string, setting the last error on failure.
///
/// # Safety
/// `ptr`, if non-null, must point to a valid NUL-terminated C string.
unsafe fn required_str<'a>(ptr: *const c_char, what: &str) -> Option<&'a str> {
    if ptr.is_null() {
        set_last_error(&format!("{what} must not be null"));
        return None;
    }
    // SAFETY: `ptr` is non-null and the caller guarantees it is NUL-terminated.
    match unsafe { std::ffi::CStr::from_ptr(ptr) }.to_str() {
        Ok(s) => Some(s),
        Err(e) => {
            set_last_error(&format!("{what} is not valid UTF-8: {e}"));
            None
        }
    }
}

/// Push a child artifact to the daemon.
///
/// - `source_kind`: 1 = bytes (`data`/`data_len`), 2 = file path (UTF-8 path
///   of `data_len` bytes in `data`, not NUL-terminated).
/// - `relation`: 1-7 (extracted, dropped, unpacked, memdump, downloaded,
///   pcap, output); root (0) is reserved.
/// - `analyze`: -1 = relation default, 0 = false, 1 = true.
/// - `allow_dynamic_recursion`: non-zero allows child detonation.
///
/// Returns `0` on success, `-1` on failure (last error is set).
///
/// # Safety
/// - `ctx` must be valid; `name` must be a NUL-terminated C string.
/// - `data` must point to `data_len` readable bytes (may be null when `data_len == 0`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_context_push_artifact(
    ctx: *const MalboxContext,
    name: *const c_char,
    data: *const u8,
    data_len: usize,
    source_kind: i32,
    relation: i32,
    analyze: i32,
    allow_dynamic_recursion: i32,
) -> i32 {
    use malbox_plugin_sdk::{ArtifactOpts, ArtifactRelation, ArtifactSource};

    // SAFETY: forwarded from this function's caller contract on `ctx`.
    let Some(context) = (unsafe { ctx_ref(ctx) }) else {
        return -1;
    };
    // SAFETY: forwarded from this function's caller contract on `name`.
    let Some(name) = (unsafe { required_str(name, "name") }) else {
        return -1;
    };
    let bytes: &[u8] = if data_len == 0 {
        &[]
    } else if data.is_null() {
        set_last_error("data must not be null when data_len > 0");
        return -1;
    } else {
        // SAFETY: non-null and the caller guarantees `data_len` readable bytes.
        unsafe { std::slice::from_raw_parts(data, data_len) }
    };
    let Some(relation) = u8::try_from(relation)
        .ok()
        .and_then(|v| ArtifactRelation::try_from(v).ok())
    else {
        set_last_error(&format!("unknown artifact relation {relation}"));
        return -1;
    };
    let mut opts =
        ArtifactOpts::new(relation).allow_dynamic_recursion(allow_dynamic_recursion != 0);
    match analyze {
        -1 => {}
        0 => opts = opts.analyze(false),
        1 => opts = opts.analyze(true),
        other => {
            set_last_error(&format!("invalid analyze value {other}"));
            return -1;
        }
    }
    let source = match source_kind {
        1 => ArtifactSource::Bytes(bytes.to_vec()),
        2 => match std::str::from_utf8(bytes) {
            Ok(p) => ArtifactSource::File(std::path::PathBuf::from(p)),
            Err(e) => {
                set_last_error(&format!("path is not valid UTF-8: {e}"));
                return -1;
            }
        },
        other => {
            set_last_error(&format!("invalid source_kind {other}"));
            return -1;
        }
    };

    let (rc, _) = result_to_rc(context.results().push_artifact(name, source, opts));
    rc
}

/// Merge `{key: json_value}` into the task-wide shared data.
///
/// `json_value` must be a valid JSON document.
///
/// Returns `0` on success, `-1` on failure (last error is set).
///
/// # Safety
/// `ctx` must be valid; `key` and `json_value` must be NUL-terminated C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_context_shared_put(
    ctx: *const MalboxContext,
    key: *const c_char,
    json_value: *const c_char,
) -> i32 {
    // SAFETY: forwarded from this function's caller contract on `ctx`.
    let Some(context) = (unsafe { ctx_ref(ctx) }) else {
        return -1;
    };
    // SAFETY: forwarded from this function's caller contract on `key`.
    let Some(key) = (unsafe { required_str(key, "key") }) else {
        return -1;
    };
    // SAFETY: forwarded from this function's caller contract on `json_value`.
    let Some(raw) = (unsafe { required_str(json_value, "json_value") }) else {
        return -1;
    };
    let value: serde_json::Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(&format!("json_value is not valid JSON: {e}"));
            return -1;
        }
    };
    let (rc, _) = result_to_rc(context.shared_put(key, value));
    rc
}

/// Ask the daemon to skip this artifact's later analysis stages.
///
/// Returns `0` on success, `-1` on failure (last error is set).
///
/// # Safety
/// `ctx` must be a valid context pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_context_drop_artifact(ctx: *const MalboxContext) -> i32 {
    // SAFETY: forwarded from this function's caller contract on `ctx`.
    let Some(context) = (unsafe { ctx_ref(ctx) }) else {
        return -1;
    };
    let (rc, _) = result_to_rc(context.drop_artifact());
    rc
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
        std::ptr::from_ref(ctx).cast::<MalboxContext>()
    }

    #[test]
    fn push_artifact_sends_meta() {
        use malbox_plugin_sdk::{ArtifactRelation, artifact::ArtifactMeta};
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let ctx = Context::test_new_full(
            1,
            std::path::PathBuf::from("/tmp/sample"),
            std::collections::HashMap::new(),
            noop_emitter(),
            Some(tx),
        );
        let name = CString::new("dropped.exe").unwrap();
        let data = b"MZ";
        // SAFETY: the context pointer, `name` and `data` all borrow locals that
        // outlive the call; `data.len()` matches the buffer.
        let rc = unsafe {
            malbox_context_push_artifact(
                make_ctx_ptr(&ctx),
                name.as_ptr(),
                data.as_ptr(),
                data.len(),
                1,
                2, // ArtifactRelation::Dropped wire value
                -1,
                0,
            )
        };
        assert_eq!(rc, 0, "{:?}", crate::error::last_error_string());
        let msg = rx.try_recv().expect("artifact message");
        assert_eq!(msg.result_name, "dropped.exe");
        assert_eq!(
            msg.artifact,
            Some(ArtifactMeta {
                relation: ArtifactRelation::Dropped,
                analyze: true,
                allow_dynamic_recursion: false,
            })
        );
    }

    #[test]
    fn push_artifact_rejects_root_relation() {
        let ctx = Context::test_new(noop_emitter());
        let name = CString::new("x").unwrap();
        // SAFETY: the context pointer and `name` borrow live locals; `data` is
        // null with `data_len == 0`, which the function accepts.
        let rc = unsafe {
            malbox_context_push_artifact(
                make_ctx_ptr(&ctx),
                name.as_ptr(),
                std::ptr::null(),
                0,
                1,
                0,
                -1,
                0,
            )
        };
        assert_eq!(rc, -1);
    }

    #[test]
    fn shared_put_rejects_invalid_json() {
        let ctx = Context::test_new(noop_emitter());
        let key = CString::new("k").unwrap();
        let value = CString::new("{nope").unwrap();
        // SAFETY: context, key and value pointers borrow live locals.
        let rc =
            unsafe { malbox_context_shared_put(make_ctx_ptr(&ctx), key.as_ptr(), value.as_ptr()) };
        assert_eq!(rc, -1);
    }

    #[test]
    fn emit_progress_succeeds() {
        let ctx = Context::test_new(noop_emitter());
        let msg = CString::new("halfway").unwrap();
        // SAFETY: context and message pointers borrow live locals.
        let rc = unsafe { malbox_context_emit_progress(make_ctx_ptr(&ctx), 0.5, msg.as_ptr()) };
        assert_eq!(rc, 0);
    }

    #[test]
    fn emit_progress_null_message_succeeds() {
        let ctx = Context::test_new(noop_emitter());
        // SAFETY: the context pointer borrows a live local; a null message is allowed.
        let rc = unsafe { malbox_context_emit_progress(make_ctx_ptr(&ctx), 0.0, std::ptr::null()) };
        assert_eq!(rc, 0);
    }

    #[test]
    fn emit_progress_null_context_returns_minus_one() {
        // SAFETY: null pointers are rejected before any dereference.
        let rc = unsafe { malbox_context_emit_progress(std::ptr::null(), 0.5, std::ptr::null()) };
        assert_eq!(rc, -1);
    }

    #[test]
    fn emit_event_succeeds() {
        let ctx = Context::test_new(noop_emitter());
        let event = MalboxEvent {
            tag: MalboxEventTag::TaskCreated,
            id: 1,
        };
        // SAFETY: the context pointer borrows a live local.
        let rc = unsafe { malbox_context_emit_event(make_ctx_ptr(&ctx), event) };
        assert_eq!(rc, 0);
    }

    #[test]
    fn emit_event_daemon_succeeds() {
        let ctx = Context::test_new(noop_emitter());
        let event = MalboxEvent {
            tag: MalboxEventTag::DaemonShutdown,
            id: 0,
        };
        // SAFETY: the context pointer borrows a live local.
        let rc = unsafe { malbox_context_emit_event(make_ctx_ptr(&ctx), event) };
        assert_eq!(rc, 0);
    }

    #[test]
    fn emit_event_null_context_returns_minus_one() {
        let event = MalboxEvent {
            tag: MalboxEventTag::TaskCreated,
            id: 0,
        };
        // SAFETY: a null context is rejected before any dereference.
        let rc = unsafe { malbox_context_emit_event(std::ptr::null(), event) };
        assert_eq!(rc, -1);
    }

    #[test]
    fn warn_succeeds() {
        let ctx = Context::test_new(noop_emitter());
        let msg = CString::new("watch out").unwrap();
        // SAFETY: context and message pointers borrow live locals.
        let rc = unsafe { malbox_context_warn(make_ctx_ptr(&ctx), msg.as_ptr()) };
        assert_eq!(rc, 0);
    }

    #[test]
    fn warn_null_context_returns_minus_one() {
        let msg = CString::new("x").unwrap();
        // SAFETY: a null context is rejected before any dereference.
        let rc = unsafe { malbox_context_warn(std::ptr::null(), msg.as_ptr()) };
        assert_eq!(rc, -1);
    }

    #[test]
    fn warn_null_message_returns_minus_one() {
        let ctx = Context::test_new(noop_emitter());
        // SAFETY: the context pointer borrows a live local; null message is rejected.
        let rc = unsafe { malbox_context_warn(make_ctx_ptr(&ctx), std::ptr::null()) };
        assert_eq!(rc, -1);
    }
}
