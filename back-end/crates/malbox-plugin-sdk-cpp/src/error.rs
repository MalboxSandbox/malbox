//! Thread-local error storage and the `malbox_last_error` FFI function.
//!
//! All `extern "C"` functions in this crate report failure by returning `-1`
//! and storing a human-readable message in a thread-local slot.  C/C++ callers
//! can retrieve the message with [`malbox_last_error`].
//!
//! The helpers `set_last_error`, `clear_last_error`, `result_to_rc`, and
//! `last_error_string` are for internal use by the other modules in this
//! crate.

use std::cell::RefCell;
use std::ffi::{CString, c_char};

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

/// Store `msg` as the current thread-local error message.
///
/// Any previously stored message is overwritten.  The string is converted to a
/// `CString`; if it contains a null byte the message `"(error contained null
/// byte)"` is stored instead.
pub(crate) fn set_last_error(msg: &str) {
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = Some(
            CString::new(msg)
                .unwrap_or_else(|_| CString::new("(error contained null byte)").unwrap()),
        );
    });
}

/// Clear the current thread-local error message.
///
/// After this call [`malbox_last_error`] will return `-1` (no error).
pub(crate) fn clear_last_error() {
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

/// Converts a Result<T, E> to an int32_t return code, storing the error message if any.
pub(crate) fn result_to_rc<T, E: std::fmt::Display>(result: Result<T, E>) -> (i32, Option<T>) {
    match result {
        Ok(val) => {
            clear_last_error();
            (0, Some(val))
        }
        Err(e) => {
            set_last_error(&e.to_string());
            (-1, None)
        }
    }
}

/// Retrieve the last error message as a Rust `String`, if one is set.
pub(crate) fn last_error_string() -> Option<String> {
    LAST_ERROR.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|cs| cs.to_str().ok())
            .map(|s| s.to_owned())
    })
}

/// Retrieve the last error message set on the current thread.
///
/// If an error is present, `*out_message` is set to a pointer to the
/// null-terminated error string and `0` is returned.  The pointer is valid
/// until the next call that modifies the thread-local error slot.
///
/// If no error is present, `-1` is returned and `*out_message` is not written.
///
/// `out_message` may be null; in that case the function still returns `0` if
/// an error exists without writing any pointer.
///
/// # Safety
///
/// `out_message`, if non-null, must be a valid pointer to a `*const c_char`
/// that the caller may write to.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_last_error(out_message: *mut *const c_char) -> i32 {
    LAST_ERROR.with(|cell| {
        let borrow = cell.borrow();
        match borrow.as_ref() {
            Some(msg) => {
                if !out_message.is_null() {
                    unsafe {
                        *out_message = msg.as_ptr();
                    }
                }
                0
            }
            None => -1,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_and_get_last_error() {
        set_last_error("something went wrong");
        let mut msg: *const c_char = std::ptr::null();
        let rc = unsafe { malbox_last_error(&mut msg) };
        assert_eq!(rc, 0);
        let s = unsafe { std::ffi::CStr::from_ptr(msg) }.to_str().unwrap();
        assert_eq!(s, "something went wrong");
    }

    #[test]
    fn test_no_error_returns_negative() {
        clear_last_error();
        let mut msg: *const c_char = std::ptr::null();
        let rc = unsafe { malbox_last_error(&mut msg) };
        assert!(rc < 0);
    }

    #[test]
    fn test_result_to_rc_ok() {
        let (rc, val) = result_to_rc::<i32, String>(Ok(42));
        assert_eq!(rc, 0);
        assert_eq!(val, Some(42));
    }

    #[test]
    fn test_result_to_rc_err() {
        let (rc, val) = result_to_rc::<i32, _>(Err("oops"));
        assert_eq!(rc, -1);
        assert_eq!(val, None);
    }
}
