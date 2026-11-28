//! FFI accessor functions for task info via Context.
//!
//! The `MalboxContext*` pointer is a `*const Context` cast through the
//! opaque `MalboxContext` enum. Task information is accessed via
//! `ctx.task()`. All string-returning functions store their `CString`
//! (or raw byte buffer) in a thread-local so the pointer remains valid
//! until `clear_temp_storage()` is called by the runtime after each
//! plugin callback returns.

use crate::error::{clear_last_error, result_to_rc, set_last_error};
use crate::ffi_types::MalboxContext;
use malbox_plugin_sdk::context::Context;
use std::cell::RefCell;
use std::ffi::{CString, c_char};

thread_local! {
    /// CStrings kept alive for the duration of the current plugin callback.
    static TEMP_STRINGS: RefCell<Vec<CString>> = const { RefCell::new(Vec::new()) };
    /// Byte buffers kept alive for the duration of the current plugin callback.
    static TEMP_BYTES: RefCell<Option<Vec<u8>>> = const { RefCell::new(None) };
    /// Per-config-entry key/value pairs kept alive for the current callback.
    static TEMP_CONFIG_ENTRIES: RefCell<Vec<(CString, CString)>> = const { RefCell::new(Vec::new()) };
}

/// Release all temporaries created for the current callback invocation.
///
/// The runtime **must** call this after every plugin callback returns.
pub(crate) fn clear_temp_storage() {
    TEMP_STRINGS.with(|c| c.borrow_mut().clear());
    TEMP_BYTES.with(|c| *c.borrow_mut() = None);
    TEMP_CONFIG_ENTRIES.with(|c| c.borrow_mut().clear());
}

/// Store a `CString` in the thread-local and return its raw pointer.
///
/// The pointer targets the string's heap buffer, which does not move when
/// the `CString` itself is moved into the thread-local vector.
fn store_string(s: CString) -> *const c_char {
    let ptr = s.as_ptr();
    TEMP_STRINGS.with(|cell| cell.borrow_mut().push(s));
    ptr
}

/// Cast an opaque `MalboxContext*` back to a Rust `Context` reference.
///
/// # Safety
/// The caller must guarantee that `ptr` is a valid `*const Context`.
unsafe fn ctx_from_ptr<'a>(ptr: *const MalboxContext) -> Option<&'a Context> {
    if ptr.is_null() {
        set_last_error("null context pointer");
        return None;
    }
    // SAFETY: MalboxContext is an opaque stand-in for Context; the runtime always
    // passes the original *const Context cast to *const MalboxContext.
    Some(unsafe { &*ptr.cast::<Context>() })
}

/// Return the numeric task ID.
///
/// Returns `-1` and sets the last error if `ctx` is null.
///
/// # Safety
///
/// `ctx` must be a valid `*const Context` cast to `*const MalboxContext`, as
/// provided by the runtime to plugin callbacks.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_task_get_id(ctx: *const MalboxContext) -> i32 {
    // SAFETY: forwarded from this function's caller contract on `ctx`.
    match unsafe { ctx_from_ptr(ctx) } {
        Some(c) => {
            clear_last_error();
            c.task().id()
        }
        None => -1,
    }
}

/// Return a pointer to a null-terminated string containing the sample path.
///
/// The pointer is valid until `clear_temp_storage` is called (which the
/// runtime does automatically after each plugin callback returns).
/// Returns null and sets the last error if `ctx` is null or the path is not
/// valid UTF-8.
///
/// # Safety
///
/// `ctx` must be a valid `*const Context` cast to `*const MalboxContext`, as
/// provided by the runtime to plugin callbacks.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_task_get_sample_path(ctx: *const MalboxContext) -> *const c_char {
    // SAFETY: forwarded from this function's caller contract on `ctx`.
    let Some(c) = (unsafe { ctx_from_ptr(ctx) }) else {
        return std::ptr::null();
    };

    let task_info = c.task();
    let Some(path_str) = task_info.sample_path().to_str() else {
        set_last_error("sample path is not valid UTF-8");
        return std::ptr::null();
    };

    match CString::new(path_str) {
        Ok(cs) => {
            clear_last_error();
            store_string(cs)
        }
        Err(e) => {
            set_last_error(&format!("sample path contains null byte: {e}"));
            std::ptr::null()
        }
    }
}

/// Read the sample file into memory and return a pointer and length.
///
/// On success, `*out_ptr` is set to the start of the byte buffer and
/// `*out_len` is set to the number of bytes.  The buffer is valid until
/// `clear_temp_storage` is called.
///
/// Returns `0` on success, `-1` on failure (last error is set).
///
/// # Safety
///
/// - `ctx` must be a valid `*const Context` cast to `*const MalboxContext`.
/// - `out_ptr` and `out_len` must both be valid, non-null, writable pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_task_get_sample_bytes(
    ctx: *const MalboxContext,
    out_ptr: *mut *const u8,
    out_len: *mut usize,
) -> i32 {
    if out_ptr.is_null() || out_len.is_null() {
        set_last_error("out_ptr and out_len must not be null");
        return -1;
    }

    // SAFETY: forwarded from this function's caller contract on `ctx`.
    let Some(c) = (unsafe { ctx_from_ptr(ctx) }) else {
        return -1;
    };

    let (_, Some(bytes)) = result_to_rc(c.task().sample_bytes()) else {
        return -1;
    };

    let len = bytes.len();
    // The Vec's heap buffer does not move when the Vec is moved into the slot.
    let ptr = bytes.as_ptr();
    TEMP_BYTES.with(|cell| *cell.borrow_mut() = Some(bytes));

    // SAFETY: `out_ptr` is non-null (checked above) and the caller guarantees
    // it is valid for writes.
    unsafe { *out_ptr = ptr };
    // SAFETY: `out_len` is non-null (checked above) and the caller guarantees
    // it is valid for writes.
    unsafe { *out_len = len };

    0
}

/// Return the number of configuration entries for the task.
///
/// Returns `0` and sets the last error if `ctx` is null.
///
/// # Safety
///
/// `ctx` must be a valid `*const Context` cast to `*const MalboxContext`, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_task_get_config_count(ctx: *const MalboxContext) -> usize {
    // SAFETY: forwarded from this function's caller contract on `ctx`.
    match unsafe { ctx_from_ptr(ctx) } {
        Some(c) => {
            clear_last_error();
            c.task().config().len()
        }
        None => 0,
    }
}

/// Fill `*out_key` and `*out_value` with the key/value strings at position
/// `index` in the config map.
///
/// Iteration order is unspecified but stable within one callback invocation.
/// Both pointers are valid until `clear_temp_storage` is called.
///
/// Returns `0` on success, `-1` if `ctx` is null or `index` is out of range.
///
/// # Safety
///
/// - `ctx` must be a valid `*const Context` cast to `*const MalboxContext`, or null.
/// - `out_key` and `out_value` must both be valid, non-null, writable pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_task_get_config_entry(
    ctx: *const MalboxContext,
    index: usize,
    out_key: *mut *const c_char,
    out_value: *mut *const c_char,
) -> i32 {
    if out_key.is_null() || out_value.is_null() {
        set_last_error("out_key and out_value must not be null");
        return -1;
    }

    // SAFETY: forwarded from this function's caller contract on `ctx`.
    let Some(c) = (unsafe { ctx_from_ptr(ctx) }) else {
        return -1;
    };

    let task_info = c.task();
    let config = task_info.config();
    let Some((k, v)) = config.iter().nth(index) else {
        set_last_error(&format!(
            "config index {} out of range (len = {})",
            index,
            config.len()
        ));
        return -1;
    };

    let ck = match CString::new(k.as_str()) {
        Ok(cs) => cs,
        Err(e) => {
            set_last_error(&format!("config key contains null byte: {e}"));
            return -1;
        }
    };
    let cv = match CString::new(v.as_str()) {
        Ok(cs) => cs,
        Err(e) => {
            set_last_error(&format!("config value contains null byte: {e}"));
            return -1;
        }
    };

    // The CStrings' heap buffers do not move when the pair is moved into storage.
    let (key_ptr, value_ptr) = (ck.as_ptr(), cv.as_ptr());
    TEMP_CONFIG_ENTRIES.with(|cell| cell.borrow_mut().push((ck, cv)));

    // SAFETY: `out_key` is non-null (checked above) and the caller guarantees
    // it is valid for writes.
    unsafe { *out_key = key_ptr };
    // SAFETY: `out_value` is non-null (checked above) and the caller guarantees
    // it is valid for writes.
    unsafe { *out_value = value_ptr };

    clear_last_error();
    0
}

/// Look up a config value by key.
///
/// Returns a pointer to a null-terminated string (valid until
/// `clear_temp_storage` is called) or null if the key is not present.
/// The last error is set only on a null `ctx` or `key` pointer, not on a
/// missing key.
///
/// # Safety
///
/// - `ctx` must be a valid `*const Context` cast to `*const MalboxContext`, or null.
/// - `key`, if non-null, must point to a valid null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_task_get_config_value(
    ctx: *const MalboxContext,
    key: *const c_char,
) -> *const c_char {
    // SAFETY: forwarded from this function's caller contract on `ctx`.
    let Some(c) = (unsafe { ctx_from_ptr(ctx) }) else {
        return std::ptr::null();
    };

    if key.is_null() {
        set_last_error("key must not be null");
        return std::ptr::null();
    }

    // SAFETY: `key` is non-null and the caller guarantees it is NUL-terminated.
    let key_str = match unsafe { std::ffi::CStr::from_ptr(key) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(&format!("key is not valid UTF-8: {e}"));
            return std::ptr::null();
        }
    };

    match c.task().config_value(key_str) {
        None => {
            clear_last_error();
            std::ptr::null()
        }
        Some(val) => match CString::new(val) {
            Ok(cs) => {
                clear_last_error();
                store_string(cs)
            }
            Err(e) => {
                set_last_error(&format!("config value contains null byte: {e}"));
                std::ptr::null()
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use malbox_plugin_sdk::context::Context;
    use std::collections::HashMap;
    use std::ffi::CStr;
    use std::sync::Arc;

    fn noop_emitter() -> Arc<dyn malbox_plugin_transport::traits::TransportEmitter + Send + Sync> {
        Arc::new(())
    }

    fn make_ctx(id: i32, config: HashMap<String, String>, sample_path: &str) -> Context {
        Context::test_new_full(
            id,
            std::path::PathBuf::from(sample_path),
            config,
            noop_emitter(),
            None,
        )
    }

    fn ctx_ptr(ctx: &Context) -> *const MalboxContext {
        std::ptr::from_ref(ctx).cast::<MalboxContext>()
    }

    #[test]
    fn test_get_id() {
        let ctx = make_ctx(42, HashMap::new(), "/tmp/sample.bin");
        // SAFETY: the context pointer borrows a live local.
        let rc = unsafe { malbox_task_get_id(ctx_ptr(&ctx)) };
        assert_eq!(rc, 42);
    }

    #[test]
    fn test_get_id_null() {
        // SAFETY: a null context is rejected before any dereference.
        let rc = unsafe { malbox_task_get_id(std::ptr::null()) };
        assert_eq!(rc, -1);
    }

    #[test]
    fn test_get_sample_path() {
        let ctx = make_ctx(1, HashMap::new(), "/tmp/my_sample.exe");
        // SAFETY: the context pointer borrows a live local.
        let ptr = unsafe { malbox_task_get_sample_path(ctx_ptr(&ctx)) };
        assert!(!ptr.is_null());
        // SAFETY: non-null result points into temp storage, alive until
        // `clear_temp_storage` below.
        let s = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap();
        assert_eq!(s, "/tmp/my_sample.exe");
        clear_temp_storage();
    }

    #[test]
    fn test_get_sample_path_null_ctx() {
        // SAFETY: a null context is rejected before any dereference.
        let ptr = unsafe { malbox_task_get_sample_path(std::ptr::null()) };
        assert!(ptr.is_null());
    }

    #[test]
    fn test_get_sample_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.bin");
        std::fs::write(&path, b"HELLO").unwrap();

        let ctx = make_ctx(1, HashMap::new(), path.to_str().unwrap());
        let mut out_ptr: *const u8 = std::ptr::null();
        let mut out_len: usize = 0;
        // SAFETY: the context and both out-pointers borrow live locals.
        let rc = unsafe {
            malbox_task_get_sample_bytes(ctx_ptr(&ctx), &raw mut out_ptr, &raw mut out_len)
        };
        assert_eq!(rc, 0);
        assert!(!out_ptr.is_null());
        assert_eq!(out_len, 5);
        // SAFETY: on success `out_ptr`/`out_len` describe the temp byte buffer,
        // alive until `clear_temp_storage` below.
        let slice = unsafe { std::slice::from_raw_parts(out_ptr, out_len) };
        assert_eq!(slice, b"HELLO");
        clear_temp_storage();
    }

    #[test]
    fn test_get_sample_bytes_missing_file() {
        let ctx = make_ctx(1, HashMap::new(), "/nonexistent/file.bin");
        let mut out_ptr: *const u8 = std::ptr::null();
        let mut out_len: usize = 0;
        // SAFETY: the context and both out-pointers borrow live locals.
        let rc = unsafe {
            malbox_task_get_sample_bytes(ctx_ptr(&ctx), &raw mut out_ptr, &raw mut out_len)
        };
        assert_eq!(rc, -1);
    }

    #[test]
    fn test_get_config_count() {
        let mut cfg = HashMap::new();
        cfg.insert("k1".into(), "v1".into());
        cfg.insert("k2".into(), "v2".into());
        let ctx = make_ctx(1, cfg, "/tmp/s.bin");
        // SAFETY: the context pointer borrows a live local.
        let count = unsafe { malbox_task_get_config_count(ctx_ptr(&ctx)) };
        assert_eq!(count, 2);
    }

    #[test]
    fn test_get_config_count_null() {
        // SAFETY: a null context is rejected before any dereference.
        let count = unsafe { malbox_task_get_config_count(std::ptr::null()) };
        assert_eq!(count, 0);
    }

    #[test]
    fn test_get_config_entry() {
        let mut cfg = HashMap::new();
        cfg.insert("alpha".into(), "one".into());
        let ctx = make_ctx(1, cfg, "/tmp/s.bin");

        let mut out_key: *const c_char = std::ptr::null();
        let mut out_val: *const c_char = std::ptr::null();
        // SAFETY: the context and both out-pointers borrow live locals.
        let rc = unsafe {
            malbox_task_get_config_entry(ctx_ptr(&ctx), 0, &raw mut out_key, &raw mut out_val)
        };
        assert_eq!(rc, 0);
        assert!(!out_key.is_null());
        assert!(!out_val.is_null());
        // SAFETY: on success both pointers target temp CStrings, alive until
        // `clear_temp_storage` below.
        let k = unsafe { CStr::from_ptr(out_key) }.to_str().unwrap();
        // SAFETY: as for `out_key`.
        let v = unsafe { CStr::from_ptr(out_val) }.to_str().unwrap();
        assert_eq!(k, "alpha");
        assert_eq!(v, "one");
        clear_temp_storage();
    }

    #[test]
    fn test_get_config_entry_out_of_range() {
        let ctx = make_ctx(1, HashMap::new(), "/tmp/s.bin");
        let mut out_key: *const c_char = std::ptr::null();
        let mut out_val: *const c_char = std::ptr::null();
        // SAFETY: the context and both out-pointers borrow live locals.
        let rc = unsafe {
            malbox_task_get_config_entry(ctx_ptr(&ctx), 0, &raw mut out_key, &raw mut out_val)
        };
        assert_eq!(rc, -1);
    }

    #[test]
    fn test_get_config_value_found() {
        let mut cfg = HashMap::new();
        cfg.insert("timeout".into(), "30".into());
        let ctx = make_ctx(1, cfg, "/tmp/s.bin");

        let key = CString::new("timeout").unwrap();
        // SAFETY: context and key pointers borrow live locals.
        let ptr = unsafe { malbox_task_get_config_value(ctx_ptr(&ctx), key.as_ptr()) };
        assert!(!ptr.is_null());
        // SAFETY: non-null result points into temp storage, alive until
        // `clear_temp_storage` below.
        let val = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap();
        assert_eq!(val, "30");
        clear_temp_storage();
    }

    #[test]
    fn test_get_config_value_missing() {
        let ctx = make_ctx(1, HashMap::new(), "/tmp/s.bin");
        let key = CString::new("missing").unwrap();
        // SAFETY: context and key pointers borrow live locals.
        let ptr = unsafe { malbox_task_get_config_value(ctx_ptr(&ctx), key.as_ptr()) };
        assert!(ptr.is_null());
    }

    #[test]
    fn test_get_config_value_null_ctx() {
        let key = CString::new("k").unwrap();
        // SAFETY: a null context is rejected before any dereference.
        let ptr = unsafe { malbox_task_get_config_value(std::ptr::null(), key.as_ptr()) };
        assert!(ptr.is_null());
    }
}
