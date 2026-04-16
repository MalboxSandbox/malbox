//! FFI accessor/setter functions for the opaque `MalboxExecRequest` and
//! `MalboxExecResult` types used by the `on_execute_command` vtable entry.
//!
//! The C++ side receives opaque pointers to these types and uses the
//! `malbox_exec_request_*` functions to read request parameters, and the
//! `malbox_exec_result_*` functions to fill in the result.

use crate::ffi_types::vtable::{MalboxExecRequest, MalboxExecResult};
use std::ffi::{CString, c_char};

/// Internal representation backing `MalboxExecRequest`.
///
/// Created on the Rust side from `ExecRequest`, then cast to
/// `*const MalboxExecRequest` for the C callback.
pub(crate) struct ExecRequestData {
    pub command: CString,
    pub args: Vec<CString>,
    pub cwd: Option<CString>,
    pub env_keys: Vec<CString>,
    pub env_values: Vec<CString>,
    pub timeout_ms: Option<u64>,
    pub background: bool,
}

impl ExecRequestData {
    /// Build from an `ExecRequest`.
    pub fn from_exec_request(req: &malbox_plugin_sdk::types::ExecRequest) -> Self {
        let command = CString::new(req.command()).unwrap_or_default();
        let args = req
            .args()
            .iter()
            .map(|a| CString::new(a.as_str()).unwrap_or_default())
            .collect();
        let cwd = req.cwd().map(|s| CString::new(s).unwrap_or_default());
        let env_keys = req
            .env()
            .keys()
            .map(|k| CString::new(k.as_str()).unwrap_or_default())
            .collect();
        let env_values = req
            .env()
            .values()
            .map(|v| CString::new(v.as_str()).unwrap_or_default())
            .collect();
        let timeout_ms = req.timeout().map(|d| d.as_millis() as u64);

        Self {
            command,
            args,
            cwd,
            env_keys,
            env_values,
            timeout_ms,
            background: req.background(),
        }
    }
}

/// Internal representation backing `MalboxExecResult`.
///
/// Created on the Rust side, passed as `*mut MalboxExecResult` to the C
/// callback, which fills it in via the `malbox_exec_result_set_*` functions.
pub(crate) struct ExecResultData {
    pub exit_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub pid: Option<u32>,
}

impl Default for ExecResultData {
    fn default() -> Self {
        Self {
            exit_code: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
            pid: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Helper casts
// ---------------------------------------------------------------------------

unsafe fn req_ref<'a>(ptr: *const MalboxExecRequest) -> Option<&'a ExecRequestData> {
    if ptr.is_null() {
        return None;
    }
    Some(unsafe { &*(ptr as *const ExecRequestData) })
}

unsafe fn res_mut<'a>(ptr: *mut MalboxExecResult) -> Option<&'a mut ExecResultData> {
    if ptr.is_null() {
        return None;
    }
    Some(unsafe { &mut *(ptr as *mut ExecResultData) })
}

// ---------------------------------------------------------------------------
// ExecRequest accessors
// ---------------------------------------------------------------------------

/// Get the command string from an exec request.
///
/// Returns a pointer to a null-terminated C string. The pointer is valid
/// for the duration of the `on_execute_command` callback. Returns null if
/// `req` is null.
///
/// # Safety
/// `req` must be a valid `*const ExecRequestData` cast to `*const MalboxExecRequest`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_exec_request_get_command(
    req: *const MalboxExecRequest,
) -> *const c_char {
    match unsafe { req_ref(req) } {
        Some(r) => r.command.as_ptr(),
        None => std::ptr::null(),
    }
}

/// Get the number of arguments in the exec request.
///
/// Returns 0 if `req` is null.
///
/// # Safety
/// `req` must be a valid `*const ExecRequestData` cast to `*const MalboxExecRequest`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_exec_request_get_args_count(
    req: *const MalboxExecRequest,
) -> usize {
    match unsafe { req_ref(req) } {
        Some(r) => r.args.len(),
        None => 0,
    }
}

/// Get a single argument by index from the exec request.
///
/// Returns null if `req` is null or `index` is out of range.
///
/// # Safety
/// `req` must be a valid `*const ExecRequestData` cast to `*const MalboxExecRequest`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_exec_request_get_arg(
    req: *const MalboxExecRequest,
    index: usize,
) -> *const c_char {
    match unsafe { req_ref(req) } {
        Some(r) => {
            if index < r.args.len() {
                r.args[index].as_ptr()
            } else {
                std::ptr::null()
            }
        }
        None => std::ptr::null(),
    }
}

/// Get the working directory from the exec request.
///
/// Returns null if `req` is null or no cwd was specified.
///
/// # Safety
/// `req` must be a valid `*const ExecRequestData` cast to `*const MalboxExecRequest`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_exec_request_get_cwd(
    req: *const MalboxExecRequest,
) -> *const c_char {
    match unsafe { req_ref(req) } {
        Some(r) => match &r.cwd {
            Some(s) => s.as_ptr(),
            None => std::ptr::null(),
        },
        None => std::ptr::null(),
    }
}

/// Get the number of environment variable entries in the exec request.
///
/// Returns 0 if `req` is null.
///
/// # Safety
/// `req` must be a valid `*const ExecRequestData` cast to `*const MalboxExecRequest`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_exec_request_get_env_count(req: *const MalboxExecRequest) -> usize {
    match unsafe { req_ref(req) } {
        Some(r) => r.env_keys.len(),
        None => 0,
    }
}

/// Get an environment variable key/value pair by index.
///
/// On success, writes the key and value pointers to `out_key` and `out_value`
/// and returns 0. Returns -1 if `req` is null or `index` is out of range.
///
/// # Safety
/// - `req` must be a valid `*const ExecRequestData` cast to `*const MalboxExecRequest`.
/// - `out_key` and `out_value` must be valid writable pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_exec_request_get_env_entry(
    req: *const MalboxExecRequest,
    index: usize,
    out_key: *mut *const c_char,
    out_value: *mut *const c_char,
) -> i32 {
    let r = match unsafe { req_ref(req) } {
        Some(r) => r,
        None => return -1,
    };
    if index >= r.env_keys.len() || out_key.is_null() || out_value.is_null() {
        return -1;
    }
    unsafe {
        *out_key = r.env_keys[index].as_ptr();
        *out_value = r.env_values[index].as_ptr();
    }
    0
}

/// Get the timeout in milliseconds from the exec request.
///
/// Returns -1 if no timeout was specified, or if `req` is null.
///
/// # Safety
/// `req` must be a valid `*const ExecRequestData` cast to `*const MalboxExecRequest`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_exec_request_get_timeout_ms(req: *const MalboxExecRequest) -> i64 {
    match unsafe { req_ref(req) } {
        Some(r) => match r.timeout_ms {
            Some(ms) => ms as i64,
            None => -1,
        },
        None => -1,
    }
}

/// Check whether the request is for a background execution.
///
/// Returns false if `req` is null.
///
/// # Safety
/// `req` must be a valid `*const ExecRequestData` cast to `*const MalboxExecRequest`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_exec_request_is_background(req: *const MalboxExecRequest) -> bool {
    match unsafe { req_ref(req) } {
        Some(r) => r.background,
        None => false,
    }
}

// ---------------------------------------------------------------------------
// ExecResult setters
// ---------------------------------------------------------------------------

/// Set the exit code on an exec result.
///
/// Returns 0 on success, -1 if `result` is null.
///
/// # Safety
/// `result` must be a valid `*mut ExecResultData` cast to `*mut MalboxExecResult`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_exec_result_set_exit_code(
    result: *mut MalboxExecResult,
    code: i32,
) -> i32 {
    match unsafe { res_mut(result) } {
        Some(r) => {
            r.exit_code = Some(code);
            0
        }
        None => -1,
    }
}

/// Set the stdout data on an exec result.
///
/// `data` points to `len` bytes. Pass null with len 0 for empty stdout.
///
/// Returns 0 on success, -1 if `result` is null.
///
/// # Safety
/// - `result` must be a valid `*mut ExecResultData` cast to `*mut MalboxExecResult`.
/// - If `len > 0`, `data` must point to at least `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_exec_result_set_stdout(
    result: *mut MalboxExecResult,
    data: *const u8,
    len: usize,
) -> i32 {
    match unsafe { res_mut(result) } {
        Some(r) => {
            if len > 0 && !data.is_null() {
                r.stdout = unsafe { std::slice::from_raw_parts(data, len) }.to_vec();
            } else {
                r.stdout.clear();
            }
            0
        }
        None => -1,
    }
}

/// Set the stderr data on an exec result.
///
/// `data` points to `len` bytes. Pass null with len 0 for empty stderr.
///
/// Returns 0 on success, -1 if `result` is null.
///
/// # Safety
/// - `result` must be a valid `*mut ExecResultData` cast to `*mut MalboxExecResult`.
/// - If `len > 0`, `data` must point to at least `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_exec_result_set_stderr(
    result: *mut MalboxExecResult,
    data: *const u8,
    len: usize,
) -> i32 {
    match unsafe { res_mut(result) } {
        Some(r) => {
            if len > 0 && !data.is_null() {
                r.stderr = unsafe { std::slice::from_raw_parts(data, len) }.to_vec();
            } else {
                r.stderr.clear();
            }
            0
        }
        None => -1,
    }
}

/// Set the PID on an exec result.
///
/// Returns 0 on success, -1 if `result` is null.
///
/// # Safety
/// `result` must be a valid `*mut ExecResultData` cast to `*mut MalboxExecResult`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_exec_result_set_pid(
    result: *mut MalboxExecResult,
    pid: u32,
) -> i32 {
    match unsafe { res_mut(result) } {
        Some(r) => {
            r.pid = Some(pid);
            0
        }
        None => -1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_round_trip() {
        let mut env = std::collections::HashMap::new();
        env.insert("KEY".to_string(), "VALUE".to_string());

        let exec_req = malbox_plugin_sdk::types::ExecRequest::test_new(
            "cmd.exe".to_string(),
            vec!["/c".to_string(), "dir".to_string()],
            Some("C:\\Users".to_string()),
            env,
            Some(std::time::Duration::from_millis(5000)),
            true,
        );
        let req = ExecRequestData::from_exec_request(&exec_req);
        let req_ptr = &req as *const ExecRequestData as *const MalboxExecRequest;

        unsafe {
            // command
            let cmd = malbox_exec_request_get_command(req_ptr);
            assert!(!cmd.is_null());
            assert_eq!(std::ffi::CStr::from_ptr(cmd).to_str().unwrap(), "cmd.exe");

            // args
            assert_eq!(malbox_exec_request_get_args_count(req_ptr), 2);
            let arg0 = malbox_exec_request_get_arg(req_ptr, 0);
            assert_eq!(std::ffi::CStr::from_ptr(arg0).to_str().unwrap(), "/c");
            let arg1 = malbox_exec_request_get_arg(req_ptr, 1);
            assert_eq!(std::ffi::CStr::from_ptr(arg1).to_str().unwrap(), "dir");
            assert!(malbox_exec_request_get_arg(req_ptr, 99).is_null());

            // cwd
            let cwd = malbox_exec_request_get_cwd(req_ptr);
            assert!(!cwd.is_null());
            assert_eq!(std::ffi::CStr::from_ptr(cwd).to_str().unwrap(), "C:\\Users");

            // env
            assert_eq!(malbox_exec_request_get_env_count(req_ptr), 1);
            let mut key: *const c_char = std::ptr::null();
            let mut val: *const c_char = std::ptr::null();
            assert_eq!(
                malbox_exec_request_get_env_entry(req_ptr, 0, &mut key, &mut val),
                0
            );
            assert_eq!(std::ffi::CStr::from_ptr(key).to_str().unwrap(), "KEY");
            assert_eq!(std::ffi::CStr::from_ptr(val).to_str().unwrap(), "VALUE");

            // timeout
            assert_eq!(malbox_exec_request_get_timeout_ms(req_ptr), 5000);

            // background
            assert!(malbox_exec_request_is_background(req_ptr));
        }
    }

    #[test]
    fn request_no_optional_fields() {
        let exec_req = malbox_plugin_sdk::types::ExecRequest::test_new(
            "test".to_string(),
            vec![],
            None,
            std::collections::HashMap::new(),
            None,
            false,
        );
        let req = ExecRequestData::from_exec_request(&exec_req);
        let req_ptr = &req as *const ExecRequestData as *const MalboxExecRequest;

        unsafe {
            assert!(malbox_exec_request_get_cwd(req_ptr).is_null());
            assert_eq!(malbox_exec_request_get_env_count(req_ptr), 0);
            assert_eq!(malbox_exec_request_get_timeout_ms(req_ptr), -1);
            assert!(!malbox_exec_request_is_background(req_ptr));
        }
    }

    #[test]
    fn null_request_returns_safe_defaults() {
        unsafe {
            assert!(malbox_exec_request_get_command(std::ptr::null()).is_null());
            assert_eq!(malbox_exec_request_get_args_count(std::ptr::null()), 0);
            assert!(malbox_exec_request_get_arg(std::ptr::null(), 0).is_null());
            assert!(malbox_exec_request_get_cwd(std::ptr::null()).is_null());
            assert_eq!(malbox_exec_request_get_env_count(std::ptr::null()), 0);
            assert_eq!(malbox_exec_request_get_timeout_ms(std::ptr::null()), -1);
            assert!(!malbox_exec_request_is_background(std::ptr::null()));
        }
    }

    #[test]
    fn result_round_trip() {
        let mut result = ExecResultData::default();
        let result_ptr = &mut result as *mut ExecResultData as *mut MalboxExecResult;

        unsafe {
            assert_eq!(malbox_exec_result_set_exit_code(result_ptr, 42), 0);
            let out = b"hello stdout";
            assert_eq!(
                malbox_exec_result_set_stdout(result_ptr, out.as_ptr(), out.len()),
                0
            );
            let err = b"hello stderr";
            assert_eq!(
                malbox_exec_result_set_stderr(result_ptr, err.as_ptr(), err.len()),
                0
            );
            assert_eq!(malbox_exec_result_set_pid(result_ptr, 1234), 0);
        }

        assert_eq!(result.exit_code, Some(42));
        assert_eq!(result.stdout, b"hello stdout");
        assert_eq!(result.stderr, b"hello stderr");
        assert_eq!(result.pid, Some(1234));
    }

    #[test]
    fn null_result_returns_minus_one() {
        unsafe {
            assert_eq!(
                malbox_exec_result_set_exit_code(std::ptr::null_mut(), 0),
                -1
            );
            assert_eq!(
                malbox_exec_result_set_stdout(std::ptr::null_mut(), std::ptr::null(), 0),
                -1
            );
            assert_eq!(
                malbox_exec_result_set_stderr(std::ptr::null_mut(), std::ptr::null(), 0),
                -1
            );
            assert_eq!(malbox_exec_result_set_pid(std::ptr::null_mut(), 0), -1);
        }
    }
}
