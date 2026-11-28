//! FFI logging functions for C++ plugins.
//!
//! Provides `extern "C"` functions that C++ code can call to emit structured
//! log entries into the guest [`LogBus`].  A global [`OnceLock`] holds the bus
//! reference; it is set once during guest plugin initialisation by
//! [`set_global_log_bus`].

use std::collections::HashMap;
use std::ffi::{CStr, c_char};
use std::sync::{Arc, OnceLock};

use malbox_plugin_sdk::log::{LogBus, LogEntry, LogLevel};

/// Global log bus shared with the guest runtime.
static GLOBAL_LOG_BUS: OnceLock<Arc<LogBus>> = OnceLock::new();

/// Store the log bus so that subsequent FFI calls can push entries.
///
/// This is called once during guest plugin startup.  If called more than once
/// the second call is silently ignored (the `OnceLock` keeps the first value).
pub fn set_global_log_bus(bus: Arc<LogBus>) {
    let _ = GLOBAL_LOG_BUS.set(bus);
}

/// Push a log entry into the global bus.
///
/// Returns silently if the bus has not been set (e.g. when running as a host
/// plugin).
///
/// # Safety
///
/// `target` and `message`, if non-null, must point to valid NUL-terminated C strings.
unsafe fn push_log(level: LogLevel, target: *const c_char, message: *const c_char) {
    let Some(bus) = GLOBAL_LOG_BUS.get() else {
        return;
    };

    let target = if target.is_null() {
        "cpp".to_string()
    } else {
        // SAFETY: `target` is non-null and the caller guarantees it is NUL-terminated.
        unsafe { CStr::from_ptr(target) }
            .to_str()
            .unwrap_or("cpp")
            .to_string()
    };

    let message = if message.is_null() {
        String::new()
    } else {
        // SAFETY: `message` is non-null and the caller guarantees it is NUL-terminated.
        unsafe { CStr::from_ptr(message) }
            .to_str()
            .unwrap_or("")
            .to_string()
    };

    let timestamp_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    // Saturate rather than wrap; u64 nanoseconds only overflow in the year 2554.
    let timestamp_ns = u64::try_from(timestamp_ns).unwrap_or(u64::MAX);

    bus.push(LogEntry {
        timestamp_ns,
        level,
        target,
        message,
        fields: HashMap::new(),
    });
}

// ---------------------------------------------------------------------------
// extern "C" entry points
// ---------------------------------------------------------------------------

/// Emit a log entry at the given numeric level (0=Trace .. 4=Error).
///
/// Unknown level values are clamped to `Error`.
///
/// # Safety
///
/// `target` and `message`, if non-null, must point to valid null-terminated C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_log(level: i32, target: *const c_char, message: *const c_char) {
    let log_level = match level {
        0 => LogLevel::Trace,
        1 => LogLevel::Debug,
        2 => LogLevel::Info,
        3 => LogLevel::Warn,
        _ => LogLevel::Error,
    };
    // SAFETY: forwarded from this function's caller contract on both strings.
    unsafe { push_log(log_level, target, message) };
}

/// Emit a `Trace`-level log entry.
///
/// # Safety
///
/// `target` and `message`, if non-null, must point to valid null-terminated C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_log_trace(target: *const c_char, message: *const c_char) {
    // SAFETY: forwarded from this function's caller contract on both strings.
    unsafe { push_log(LogLevel::Trace, target, message) };
}

/// Emit a `Debug`-level log entry.
///
/// # Safety
///
/// `target` and `message`, if non-null, must point to valid null-terminated C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_log_debug(target: *const c_char, message: *const c_char) {
    // SAFETY: forwarded from this function's caller contract on both strings.
    unsafe { push_log(LogLevel::Debug, target, message) };
}

/// Emit an `Info`-level log entry.
///
/// # Safety
///
/// `target` and `message`, if non-null, must point to valid null-terminated C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_log_info(target: *const c_char, message: *const c_char) {
    // SAFETY: forwarded from this function's caller contract on both strings.
    unsafe { push_log(LogLevel::Info, target, message) };
}

/// Emit a `Warn`-level log entry.
///
/// # Safety
///
/// `target` and `message`, if non-null, must point to valid null-terminated C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_log_warn(target: *const c_char, message: *const c_char) {
    // SAFETY: forwarded from this function's caller contract on both strings.
    unsafe { push_log(LogLevel::Warn, target, message) };
}

/// Emit an `Error`-level log entry.
///
/// # Safety
///
/// `target` and `message`, if non-null, must point to valid null-terminated C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_log_error(target: *const c_char, message: *const c_char) {
    // SAFETY: forwarded from this function's caller contract on both strings.
    unsafe { push_log(LogLevel::Error, target, message) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn push_log_without_bus_does_not_panic() {
        // GLOBAL_LOG_BUS is not set in test context — this must not panic.
        // SAFETY: null pointers are allowed and never dereferenced.
        unsafe { push_log(LogLevel::Info, std::ptr::null(), std::ptr::null()) };
    }

    #[test]
    fn malbox_log_maps_levels() {
        // Exercise the level mapping; since no bus is set these are no-ops
        // but they must not panic or crash.
        let target = CString::new("test").unwrap();
        let message = CString::new("hello").unwrap();

        for level in [0, 1, 2, 3, 4, 99] {
            // SAFETY: both pointers come from CStrings alive for the whole loop.
            unsafe {
                malbox_log(level, target.as_ptr(), message.as_ptr());
            }
        }
    }
}
