//! FFI accessor functions for `Task`.
//!
//! The `MalboxTask*` pointer is actually a `*const Task` cast through the
//! opaque `MalboxTask` enum.  All string-returning functions store their
//! `CString` (or raw byte buffer) in a thread-local so the pointer remains
//! valid until `clear_temp_storage()` is called by the runtime after each
//! plugin callback returns.

use crate::error::{clear_last_error, result_to_rc, set_last_error};
use crate::ffi_types::MalboxTask;
use malbox_plugin_sdk::types::Task;
use std::cell::RefCell;
use std::ffi::{CString, c_char};

thread_local! {
    /// CStrings kept alive for the duration of the current plugin callback.
    static TEMP_STRINGS: RefCell<Vec<CString>> = RefCell::new(Vec::new());
    /// Byte buffers kept alive for the duration of the current plugin callback.
    static TEMP_BYTES: RefCell<Option<Vec<u8>>> = RefCell::new(None);
    /// Per-config-entry key/value pairs kept alive for the current callback.
    static TEMP_CONFIG_ENTRIES: RefCell<Vec<(CString, CString)>> = RefCell::new(Vec::new());
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
fn store_string(s: CString) -> *const c_char {
    TEMP_STRINGS.with(|cell| {
        let mut v = cell.borrow_mut();
        v.push(s);
        v.last().unwrap().as_ptr()
    })
}

/// Cast an opaque `MalboxTask*` back to a Rust `Task` reference.
///
/// # Safety
/// The caller must guarantee that `ptr` is a valid `*const Task`.
unsafe fn task_from_ptr<'a>(ptr: *const MalboxTask) -> Option<&'a Task> {
    if ptr.is_null() {
        set_last_error("null task pointer");
        return None;
    }
    // SAFETY: MalboxTask is an opaque stand-in for Task; the runtime always
    // passes the original *const Task cast to *const MalboxTask.
    Some(unsafe { &*(ptr as *const Task) })
}

/// Return the numeric task ID.
///
/// Returns `-1` and sets the last error if `task` is null.
///
/// # Safety
///
/// `task` must be a valid `*const Task` cast to `*const MalboxTask`, as
/// provided by the runtime to plugin callbacks.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_task_get_id(task: *const MalboxTask) -> i32 {
    match unsafe { task_from_ptr(task) } {
        Some(t) => {
            clear_last_error();
            t.id()
        }
        None => -1,
    }
}

/// Return a pointer to a null-terminated string containing the sample path.
///
/// The pointer is valid until `clear_temp_storage` is called (which the
/// runtime does automatically after each plugin callback returns).
/// Returns null and sets the last error if `task` is null or the path is not
/// valid UTF-8.
///
/// # Safety
///
/// `task` must be a valid `*const Task` cast to `*const MalboxTask`, as
/// provided by the runtime to plugin callbacks.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_task_get_sample_path(task: *const MalboxTask) -> *const c_char {
    let t = match unsafe { task_from_ptr(task) } {
        Some(t) => t,
        None => return std::ptr::null(),
    };

    let path_str = match t.sample_path().to_str() {
        Some(s) => s,
        None => {
            set_last_error("sample path is not valid UTF-8");
            return std::ptr::null();
        }
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
/// - `task` must be a valid `*const Task` cast to `*const MalboxTask`.
/// - `out_ptr` and `out_len` must both be valid, non-null, writable pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_task_get_sample_bytes(
    task: *const MalboxTask,
    out_ptr: *mut *const u8,
    out_len: *mut usize,
) -> i32 {
    if out_ptr.is_null() || out_len.is_null() {
        set_last_error("out_ptr and out_len must not be null");
        return -1;
    }

    let t = match unsafe { task_from_ptr(task) } {
        Some(t) => t,
        None => return -1,
    };

    let (rc, bytes_opt) = result_to_rc(t.sample_bytes());
    if rc != 0 {
        return -1;
    }

    let bytes = bytes_opt.unwrap();
    let len = bytes.len();

    TEMP_BYTES.with(|cell| {
        let mut slot = cell.borrow_mut();
        *slot = Some(bytes);
        let ptr = slot.as_ref().unwrap().as_ptr();
        unsafe {
            *out_ptr = ptr;
            *out_len = len;
        }
    });

    0
}

/// Return the number of configuration entries for the task.
///
/// Returns `0` and sets the last error if `task` is null.
///
/// # Safety
///
/// `task` must be a valid `*const Task` cast to `*const MalboxTask`, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_task_get_config_count(task: *const MalboxTask) -> usize {
    match unsafe { task_from_ptr(task) } {
        Some(t) => {
            clear_last_error();
            t.config().len()
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
/// Returns `0` on success, `-1` if `task` is null or `index` is out of range.
///
/// # Safety
///
/// - `task` must be a valid `*const Task` cast to `*const MalboxTask`, or null.
/// - `out_key` and `out_value` must both be valid, non-null, writable pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_task_get_config_entry(
    task: *const MalboxTask,
    index: usize,
    out_key: *mut *const c_char,
    out_value: *mut *const c_char,
) -> i32 {
    if out_key.is_null() || out_value.is_null() {
        set_last_error("out_key and out_value must not be null");
        return -1;
    }

    let t = match unsafe { task_from_ptr(task) } {
        Some(t) => t,
        None => return -1,
    };

    let Some((k, v)) = t.config().iter().nth(index) else {
        set_last_error(&format!(
            "config index {} out of range (len = {})",
            index,
            t.config().len()
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

    TEMP_CONFIG_ENTRIES.with(|cell| {
        let mut entries = cell.borrow_mut();
        entries.push((ck, cv));
        let (ref rk, ref rv) = *entries.last().unwrap();
        unsafe {
            *out_key = rk.as_ptr();
            *out_value = rv.as_ptr();
        }
    });

    clear_last_error();
    0
}

/// Look up a config value by key.
///
/// Returns a pointer to a null-terminated string (valid until
/// `clear_temp_storage` is called) or null if the key is not present.
/// The last error is set only on a null `task` or `key` pointer, not on a
/// missing key.
///
/// # Safety
///
/// - `task` must be a valid `*const Task` cast to `*const MalboxTask`, or null.
/// - `key`, if non-null, must point to a valid null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_task_get_config_value(
    task: *const MalboxTask,
    key: *const c_char,
) -> *const c_char {
    let t = match unsafe { task_from_ptr(task) } {
        Some(t) => t,
        None => return std::ptr::null(),
    };

    if key.is_null() {
        set_last_error("key must not be null");
        return std::ptr::null();
    }

    let key_str = match unsafe { std::ffi::CStr::from_ptr(key) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(&format!("key is not valid UTF-8: {e}"));
            return std::ptr::null();
        }
    };

    match t.config().get(key_str) {
        None => {
            clear_last_error();
            std::ptr::null()
        }
        Some(val) => match CString::new(val.as_str()) {
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
    use malbox_plugin_sdk::types::Task;
    use std::collections::HashMap;
    use std::ffi::CStr;

    fn make_task(id: i32, config: HashMap<String, String>, sample_path: &str) -> Task {
        Task::test_new(id, std::path::PathBuf::from(sample_path), config)
    }

    fn task_ptr(task: &Task) -> *const MalboxTask {
        (task as *const Task) as *const MalboxTask
    }

    #[test]
    fn test_get_id() {
        let task = make_task(42, HashMap::new(), "/tmp/sample.bin");
        let rc = unsafe { malbox_task_get_id(task_ptr(&task)) };
        assert_eq!(rc, 42);
    }

    #[test]
    fn test_get_id_null() {
        let rc = unsafe { malbox_task_get_id(std::ptr::null()) };
        assert_eq!(rc, -1);
    }

    #[test]
    fn test_get_sample_path() {
        let task = make_task(1, HashMap::new(), "/tmp/my_sample.exe");
        let ptr = unsafe { malbox_task_get_sample_path(task_ptr(&task)) };
        assert!(!ptr.is_null());
        let s = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap();
        assert_eq!(s, "/tmp/my_sample.exe");
        clear_temp_storage();
    }

    #[test]
    fn test_get_sample_path_null_task() {
        let ptr = unsafe { malbox_task_get_sample_path(std::ptr::null()) };
        assert!(ptr.is_null());
    }

    #[test]
    fn test_get_sample_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.bin");
        std::fs::write(&path, b"HELLO").unwrap();

        let task = make_task(1, HashMap::new(), path.to_str().unwrap());
        let mut out_ptr: *const u8 = std::ptr::null();
        let mut out_len: usize = 0;
        let rc =
            unsafe { malbox_task_get_sample_bytes(task_ptr(&task), &mut out_ptr, &mut out_len) };
        assert_eq!(rc, 0);
        assert!(!out_ptr.is_null());
        assert_eq!(out_len, 5);
        let slice = unsafe { std::slice::from_raw_parts(out_ptr, out_len) };
        assert_eq!(slice, b"HELLO");
        clear_temp_storage();
    }

    #[test]
    fn test_get_sample_bytes_missing_file() {
        let task = make_task(1, HashMap::new(), "/nonexistent/file.bin");
        let mut out_ptr: *const u8 = std::ptr::null();
        let mut out_len: usize = 0;
        let rc =
            unsafe { malbox_task_get_sample_bytes(task_ptr(&task), &mut out_ptr, &mut out_len) };
        assert_eq!(rc, -1);
    }

    #[test]
    fn test_get_config_count() {
        let mut cfg = HashMap::new();
        cfg.insert("k1".into(), "v1".into());
        cfg.insert("k2".into(), "v2".into());
        let task = make_task(1, cfg, "/tmp/s.bin");
        let count = unsafe { malbox_task_get_config_count(task_ptr(&task)) };
        assert_eq!(count, 2);
    }

    #[test]
    fn test_get_config_count_null() {
        let count = unsafe { malbox_task_get_config_count(std::ptr::null()) };
        assert_eq!(count, 0);
    }

    #[test]
    fn test_get_config_entry() {
        let mut cfg = HashMap::new();
        cfg.insert("alpha".into(), "one".into());
        let task = make_task(1, cfg, "/tmp/s.bin");

        let mut out_key: *const c_char = std::ptr::null();
        let mut out_val: *const c_char = std::ptr::null();
        let rc =
            unsafe { malbox_task_get_config_entry(task_ptr(&task), 0, &mut out_key, &mut out_val) };
        assert_eq!(rc, 0);
        assert!(!out_key.is_null());
        assert!(!out_val.is_null());
        let k = unsafe { CStr::from_ptr(out_key) }.to_str().unwrap();
        let v = unsafe { CStr::from_ptr(out_val) }.to_str().unwrap();
        assert_eq!(k, "alpha");
        assert_eq!(v, "one");
        clear_temp_storage();
    }

    #[test]
    fn test_get_config_entry_out_of_range() {
        let task = make_task(1, HashMap::new(), "/tmp/s.bin");
        let mut out_key: *const c_char = std::ptr::null();
        let mut out_val: *const c_char = std::ptr::null();
        let rc =
            unsafe { malbox_task_get_config_entry(task_ptr(&task), 0, &mut out_key, &mut out_val) };
        assert_eq!(rc, -1);
    }

    #[test]
    fn test_get_config_value_found() {
        let mut cfg = HashMap::new();
        cfg.insert("timeout".into(), "30".into());
        let task = make_task(1, cfg, "/tmp/s.bin");

        let key = CString::new("timeout").unwrap();
        let ptr = unsafe { malbox_task_get_config_value(task_ptr(&task), key.as_ptr()) };
        assert!(!ptr.is_null());
        let val = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap();
        assert_eq!(val, "30");
        clear_temp_storage();
    }

    #[test]
    fn test_get_config_value_missing() {
        let task = make_task(1, HashMap::new(), "/tmp/s.bin");
        let key = CString::new("missing").unwrap();
        let ptr = unsafe { malbox_task_get_config_value(task_ptr(&task), key.as_ptr()) };
        assert!(ptr.is_null());
    }

    #[test]
    fn test_get_config_value_null_task() {
        let key = CString::new("k").unwrap();
        let ptr = unsafe { malbox_task_get_config_value(std::ptr::null(), key.as_ptr()) };
        assert!(ptr.is_null());
    }
}
