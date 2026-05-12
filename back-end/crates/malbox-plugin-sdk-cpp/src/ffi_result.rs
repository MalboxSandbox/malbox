//! FFI result-builder functions.
//!
//! The `MalboxResultBuilder*` pointer is actually a `*mut ResultBuilder` cast
//! through the opaque `MalboxResultBuilder` enum.

use crate::error::{clear_last_error, set_last_error};
use crate::ffi_types::MalboxResultBuilder;
use malbox_plugin_sdk::result::PluginResult;
use std::ffi::c_char;
use std::path::PathBuf;

/// Internal result accumulator passed to plugin callbacks.
pub(crate) struct ResultBuilder {
    results: Vec<PluginResult>,
}

impl ResultBuilder {
    /// Create an empty builder.
    pub(crate) fn new() -> Self {
        Self {
            results: Vec::new(),
        }
    }

    /// Consume the builder and return all accumulated results.
    pub(crate) fn take(self) -> Vec<PluginResult> {
        self.results
    }
}

/// Cast an opaque `MalboxResultBuilder*` to a Rust `&mut ResultBuilder`.
///
/// # Safety
/// `ptr` must be a valid `*mut ResultBuilder`.
unsafe fn builder_from_ptr<'a>(ptr: *mut MalboxResultBuilder) -> Option<&'a mut ResultBuilder> {
    if ptr.is_null() {
        set_last_error("null result builder pointer");
        return None;
    }
    // SAFETY: MalboxResultBuilder is an opaque stand-in for ResultBuilder.
    Some(unsafe { &mut *(ptr as *mut ResultBuilder) })
}

/// Read a `*const c_char` into a `String`.
///
/// Sets the last error and returns `None` if the pointer is null or not valid
/// UTF-8.
unsafe fn cstr_to_string(ptr: *const c_char, field: &str) -> Option<String> {
    if ptr.is_null() {
        set_last_error(&format!("{field} must not be null"));
        return None;
    }
    match unsafe { std::ffi::CStr::from_ptr(ptr) }.to_str() {
        Ok(s) => Some(s.to_owned()),
        Err(e) => {
            set_last_error(&format!("{field} is not valid UTF-8: {e}"));
            None
        }
    }
}

/// Push a JSON result into the builder.
///
/// `name` is a null-terminated UTF-8 string naming the result artifact.
/// `data` points to `data_len` bytes of JSON-encoded data; when `data_len`
/// is `0`, `data` may be null.
///
/// Returns `0` on success, `-1` on failure (last error is set).
///
/// # Safety
///
/// - `builder` must be a valid `*mut ResultBuilder` cast to
///   `*mut MalboxResultBuilder`, as provided by the runtime to the `on_task`
///   callback.
/// - `name` must point to a valid null-terminated C string.
/// - If `data_len > 0`, `data` must point to at least `data_len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_result_builder_push_json(
    builder: *mut MalboxResultBuilder,
    name: *const c_char,
    data: *const u8,
    data_len: usize,
) -> i32 {
    let b = match unsafe { builder_from_ptr(builder) } {
        Some(b) => b,
        None => return -1,
    };
    let name = match unsafe { cstr_to_string(name, "name") } {
        Some(n) => n,
        None => return -1,
    };
    if data.is_null() && data_len > 0 {
        set_last_error("data must not be null when data_len > 0");
        return -1;
    }
    let bytes = if data_len == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(data, data_len) }.to_vec()
    };

    b.results.push(PluginResult::Json { name, data: bytes });
    clear_last_error();
    0
}

/// Push a raw-bytes result into the builder.
///
/// `name` is a null-terminated UTF-8 string naming the result artifact.
/// `data` points to `data_len` bytes of arbitrary binary data; when
/// `data_len` is `0`, `data` may be null.
///
/// Returns `0` on success, `-1` on failure (last error is set).
///
/// # Safety
///
/// - `builder` must be a valid `*mut ResultBuilder` cast to
///   `*mut MalboxResultBuilder`, as provided by the runtime to the `on_task`
///   callback.
/// - `name` must point to a valid null-terminated C string.
/// - If `data_len > 0`, `data` must point to at least `data_len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_result_builder_push_bytes(
    builder: *mut MalboxResultBuilder,
    name: *const c_char,
    data: *const u8,
    data_len: usize,
) -> i32 {
    let b = match unsafe { builder_from_ptr(builder) } {
        Some(b) => b,
        None => return -1,
    };
    let name = match unsafe { cstr_to_string(name, "name") } {
        Some(n) => n,
        None => return -1,
    };
    if data.is_null() && data_len > 0 {
        set_last_error("data must not be null when data_len > 0");
        return -1;
    }
    let bytes = if data_len == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(data, data_len) }.to_vec()
    };

    b.results.push(PluginResult::Bytes { name, data: bytes });
    clear_last_error();
    0
}

/// Push a file result into the builder.
///
/// `name` is a null-terminated UTF-8 string naming the result artifact.
/// `path` must be a null-terminated UTF-8 string containing a filesystem path
/// to the result file; the file does not need to exist at the time this
/// function is called.
///
/// Returns `0` on success, `-1` on failure (last error is set).
///
/// # Safety
///
/// - `builder` must be a valid `*mut ResultBuilder` cast to
///   `*mut MalboxResultBuilder`, as provided by the runtime to the `on_task`
///   callback.
/// - `name` and `path` must each point to a valid null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_result_builder_push_file(
    builder: *mut MalboxResultBuilder,
    name: *const c_char,
    path: *const c_char,
) -> i32 {
    let b = match unsafe { builder_from_ptr(builder) } {
        Some(b) => b,
        None => return -1,
    };
    let name = match unsafe { cstr_to_string(name, "name") } {
        Some(n) => n,
        None => return -1,
    };
    let path_str = match unsafe { cstr_to_string(path, "path") } {
        Some(p) => p,
        None => return -1,
    };

    b.results.push(PluginResult::File {
        name,
        path: PathBuf::from(path_str),
    });
    clear_last_error();
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    fn builder_ptr(b: &mut ResultBuilder) -> *mut MalboxResultBuilder {
        (b as *mut ResultBuilder) as *mut MalboxResultBuilder
    }

    #[test]
    fn test_push_json() {
        let mut b = ResultBuilder::new();
        let name = CString::new("my_json").unwrap();
        let payload = b"{\"x\":1}";
        let rc = unsafe {
            malbox_result_builder_push_json(
                builder_ptr(&mut b),
                name.as_ptr(),
                payload.as_ptr(),
                payload.len(),
            )
        };
        assert_eq!(rc, 0);
        let results = b.take();
        assert_eq!(results.len(), 1);
        match &results[0] {
            PluginResult::Json { name, data } => {
                assert_eq!(name, "my_json");
                assert_eq!(data, b"{\"x\":1}");
            }
            _ => panic!("expected Json"),
        }
    }

    #[test]
    fn test_push_bytes() {
        let mut b = ResultBuilder::new();
        let name = CString::new("raw").unwrap();
        let payload: &[u8] = &[0xDE, 0xAD, 0xBE, 0xEF];
        let rc = unsafe {
            malbox_result_builder_push_bytes(
                builder_ptr(&mut b),
                name.as_ptr(),
                payload.as_ptr(),
                payload.len(),
            )
        };
        assert_eq!(rc, 0);
        let results = b.take();
        assert_eq!(results.len(), 1);
        match &results[0] {
            PluginResult::Bytes { name, data } => {
                assert_eq!(name, "raw");
                assert_eq!(data.as_slice(), &[0xDE, 0xAD, 0xBE, 0xEF]);
            }
            _ => panic!("expected Bytes"),
        }
    }

    #[test]
    fn test_push_file() {
        let mut b = ResultBuilder::new();
        let name = CString::new("capture").unwrap();
        let path = CString::new("/tmp/out.pcap").unwrap();
        let rc = unsafe {
            malbox_result_builder_push_file(builder_ptr(&mut b), name.as_ptr(), path.as_ptr())
        };
        assert_eq!(rc, 0);
        let results = b.take();
        assert_eq!(results.len(), 1);
        match &results[0] {
            PluginResult::File { name, path } => {
                assert_eq!(name, "capture");
                assert_eq!(path, &PathBuf::from("/tmp/out.pcap"));
            }
            _ => panic!("expected File"),
        }
    }

    #[test]
    fn test_push_multiple() {
        let mut b = ResultBuilder::new();
        let name_j = CString::new("j").unwrap();
        let name_b = CString::new("b").unwrap();
        let name_f = CString::new("f").unwrap();
        let path = CString::new("/x").unwrap();
        let data: &[u8] = b"hi";
        unsafe {
            malbox_result_builder_push_json(
                builder_ptr(&mut b),
                name_j.as_ptr(),
                data.as_ptr(),
                data.len(),
            );
            malbox_result_builder_push_bytes(
                builder_ptr(&mut b),
                name_b.as_ptr(),
                data.as_ptr(),
                data.len(),
            );
            malbox_result_builder_push_file(builder_ptr(&mut b), name_f.as_ptr(), path.as_ptr());
        }
        let results = b.take();
        assert_eq!(results.len(), 3);
        assert!(matches!(&results[0], PluginResult::Json { .. }));
        assert!(matches!(&results[1], PluginResult::Bytes { .. }));
        assert!(matches!(&results[2], PluginResult::File { .. }));
    }

    #[test]
    fn test_null_builder_returns_minus_one() {
        let name = CString::new("x").unwrap();
        let data: &[u8] = b"";
        let rc = unsafe {
            malbox_result_builder_push_json(std::ptr::null_mut(), name.as_ptr(), data.as_ptr(), 0)
        };
        assert_eq!(rc, -1);
    }

    #[test]
    fn test_null_name_returns_minus_one() {
        let mut b = ResultBuilder::new();
        let data: &[u8] = b"";
        let rc = unsafe {
            malbox_result_builder_push_json(builder_ptr(&mut b), std::ptr::null(), data.as_ptr(), 0)
        };
        assert_eq!(rc, -1);
    }

    #[test]
    fn test_empty_data_ok() {
        let mut b = ResultBuilder::new();
        let name = CString::new("empty").unwrap();
        let rc = unsafe {
            malbox_result_builder_push_bytes(
                builder_ptr(&mut b),
                name.as_ptr(),
                std::ptr::null(),
                0,
            )
        };
        assert_eq!(rc, 0);
        let results = b.take();
        match &results[0] {
            PluginResult::Bytes { data, .. } => assert!(data.is_empty()),
            _ => panic!(),
        }
    }
}
