//! Mock test runtime -- exercises the full vtable dispatch path without
//! starting gRPC or IPC.
//!
//! `malbox_test_run_plugin` is a lightweight alternative to the real runtimes
//! that is suitable for unit-testing C++ plugin code:  it calls each handler
//! in a fixed sequence and returns 0 if all succeed.

use std::collections::HashMap;
use std::ffi::{CStr, c_char};
use std::path::PathBuf;

use malbox_plugin_sdk::context::Context;
use malbox_plugin_sdk::plugin::HostPlugin;
use malbox_plugin_sdk::types::Task;
use malbox_plugin_transport::messages::events::Event;

use crate::error::set_last_error;
use crate::ffi_callbacks::VtablePlugin;
use crate::ffi_types::{MALBOX_ABI_VERSION, MalboxPluginVtable};

/// Configuration passed to `malbox_test_run_plugin`.
///
/// All pointer fields must remain valid for the duration of the call.
#[repr(C)]
pub struct MalboxTestConfig {
    /// Numeric task identifier used to construct the test `Task`.
    pub task_id: i32,
    /// Null-terminated path to the sample file (may be a non-existent path for
    /// tests that do not call `task.sample_bytes()`).
    pub sample_path: *const c_char,
    /// Array of `config_count` null-terminated key strings.
    pub config_keys: *const *const c_char,
    /// Array of `config_count` null-terminated value strings.
    pub config_values: *const *const c_char,
    /// Number of entries in `config_keys` / `config_values`.
    pub config_count: usize,
}

/// Run a plugin through a synthetic lifecycle without starting any transport.
///
/// Intended for unit-testing C++ plugin code.  The call sequence is:
///
/// 1. `on_start(config)`
/// 2. `on_task(task, ctx)`
/// 3. `health_check()`
/// 4. `on_event(ConfigReloaded, ctx)`
/// 5. `on_stop()`
///
/// Returns `0` if every step succeeds, `-1` on the first error (the error
/// message is stored in the thread-local and can be retrieved with
/// [`malbox_last_error`](crate::error::malbox_last_error)).
///
/// # Safety
///
/// - `vtable.abi_version` must equal [`MALBOX_ABI_VERSION`]; the call fails
///   immediately with `-1` if it does not.
/// - `vtable.plugin_ptr` and all non-null vtable function pointers must remain
///   valid for the duration of the call.
/// - `config.sample_path`, if non-null, must point to a valid null-terminated
///   C string.
/// - `config.config_keys` and `config.config_values`, if non-null, must each
///   point to at least `config.config_count` valid null-terminated C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malbox_test_run_plugin(
    vtable: MalboxPluginVtable,
    config: MalboxTestConfig,
) -> i32 {
    // ABI check
    if vtable.abi_version != MALBOX_ABI_VERSION {
        set_last_error(&format!(
            "ABI version mismatch: expected {MALBOX_ABI_VERSION}, got {}",
            vtable.abi_version
        ));
        return -1;
    }

    // Build config map
    let config_map = match unsafe { build_config_map(&config) } {
        Ok(m) => m,
        Err(e) => {
            set_last_error(&e);
            return -1;
        }
    };

    // Resolve sample path
    let sample_path = if config.sample_path.is_null() {
        PathBuf::new()
    } else {
        match unsafe { CStr::from_ptr(config.sample_path) }.to_str() {
            Ok(s) => PathBuf::from(s),
            Err(e) => {
                set_last_error(&format!("sample_path is not valid UTF-8: {e}"));
                return -1;
            }
        }
    };

    // Create plugin adapter and no-op context
    let plugin = VtablePlugin::new(vtable);
    let emitter = ();
    let ctx = Context::test_new(&emitter);

    // Step 1: on_start
    if let Err(e) = plugin.on_start(config_map.clone()) {
        set_last_error(&format!("on_start failed: {e}"));
        return -1;
    }

    // Step 2: on_task -- needs a result channel so push_result works.
    // Mirrors the real guest runtime, which wires a ResultSender for on_task
    // and provides a tx-less context for lifecycle callbacks.
    let (tx, mut rx) = tokio::sync::mpsc::channel(1024);
    let ctx_task = Context::test_new_with_tx(&emitter, tx);
    let task = Task::test_new(config.task_id, sample_path, config_map);
    let on_task_result = plugin.on_task(task, &ctx_task);
    drop(ctx_task);
    while rx.try_recv().is_ok() {}
    if let Err(e) = on_task_result {
        set_last_error(&format!("on_task failed: {e}"));
        return -1;
    }

    // Step 3: health_check
    let _status = plugin.health_check();

    // Step 4: on_event(ConfigReloaded)
    if let Err(e) = plugin.on_event(Event::ConfigReloaded, &ctx) {
        set_last_error(&format!("on_event failed: {e}"));
        return -1;
    }

    // Step 5: on_stop
    if let Err(e) = plugin.on_stop() {
        set_last_error(&format!("on_stop failed: {e}"));
        return -1;
    }

    0
}

/// Build a `HashMap<String, String>` from the parallel key/value pointer arrays
/// in a `MalboxTestConfig`.
///
/// # Safety
/// `config.config_keys` and `config.config_values` must each point to at
/// least `config.config_count` valid, null-terminated C strings.
unsafe fn build_config_map(config: &MalboxTestConfig) -> Result<HashMap<String, String>, String> {
    let mut map = HashMap::with_capacity(config.config_count);

    if config.config_count == 0 {
        return Ok(map);
    }

    if config.config_keys.is_null() || config.config_values.is_null() {
        return Err("config_keys or config_values is null but config_count > 0".to_string());
    }

    for i in 0..config.config_count {
        // SAFETY: caller guarantees both arrays have at least config_count elements.
        let key_ptr = unsafe { *config.config_keys.add(i) };
        let val_ptr = unsafe { *config.config_values.add(i) };

        if key_ptr.is_null() {
            return Err(format!("config_keys[{i}] is null"));
        }
        if val_ptr.is_null() {
            return Err(format!("config_values[{i}] is null"));
        }

        let key = unsafe { CStr::from_ptr(key_ptr) }
            .to_str()
            .map_err(|e| format!("config_keys[{i}] is not valid UTF-8: {e}"))?
            .to_owned();

        let val = unsafe { CStr::from_ptr(val_ptr) }
            .to_str()
            .map_err(|e| format!("config_values[{i}] is not valid UTF-8: {e}"))?
            .to_owned();

        map.insert(key, val);
    }

    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffi_events::MalboxEvent;
    use crate::ffi_types::{MalboxContext, MalboxPluginVtable, MalboxResultBuilder, MalboxTask};
    use std::ffi::CString;
    use std::sync::atomic::{AtomicU32, Ordering};

    // Shared counters -- one per callback
    //
    // Because multiple tests run in parallel and share these statics, we
    // serialize every test that reads/writes counters behind a mutex so that
    // each test sees a clean slate.

    static COUNTER_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    static CALL_START: AtomicU32 = AtomicU32::new(0);
    static CALL_TASK: AtomicU32 = AtomicU32::new(0);
    static CALL_HEALTH: AtomicU32 = AtomicU32::new(0);
    static CALL_EVENT: AtomicU32 = AtomicU32::new(0);
    static CALL_STOP: AtomicU32 = AtomicU32::new(0);

    fn reset_counters() {
        CALL_START.store(0, Ordering::SeqCst);
        CALL_TASK.store(0, Ordering::SeqCst);
        CALL_HEALTH.store(0, Ordering::SeqCst);
        CALL_EVENT.store(0, Ordering::SeqCst);
        CALL_STOP.store(0, Ordering::SeqCst);
    }

    // Mock C callbacks

    unsafe extern "C" fn mock_on_start(
        _plugin: *mut std::ffi::c_void,
        _keys: *const *const c_char,
        _vals: *const *const c_char,
        _count: usize,
    ) -> i32 {
        CALL_START.fetch_add(1, Ordering::SeqCst);
        0
    }

    unsafe extern "C" fn mock_on_task(
        _plugin: *mut std::ffi::c_void,
        _task: *const MalboxTask,
        _ctx: *const MalboxContext,
        _builder: *mut MalboxResultBuilder,
    ) -> i32 {
        CALL_TASK.fetch_add(1, Ordering::SeqCst);
        0
    }

    unsafe extern "C" fn mock_health_check(
        _plugin: *mut std::ffi::c_void,
        status: *mut crate::ffi_types::MalboxHealthStatus,
    ) -> i32 {
        CALL_HEALTH.fetch_add(1, Ordering::SeqCst);
        if !status.is_null() {
            unsafe {
                (*status).ready = true;
                (*status).reason = std::ptr::null();
            }
        }
        0
    }

    unsafe extern "C" fn mock_on_event(
        _plugin: *mut std::ffi::c_void,
        event: MalboxEvent,
        _ctx: *const MalboxContext,
    ) -> i32 {
        CALL_EVENT.fetch_add(1, Ordering::SeqCst);
        // Verify we receive the expected event variant (ConfigReloaded = 11).
        assert_eq!(event.tag, crate::ffi_events::MalboxEventTag::ConfigReloaded);
        0
    }

    unsafe extern "C" fn mock_on_stop(_plugin: *mut std::ffi::c_void) -> i32 {
        CALL_STOP.fetch_add(1, Ordering::SeqCst);
        0
    }

    // A callback that always returns an error.
    unsafe extern "C" fn mock_on_stop_fail(_plugin: *mut std::ffi::c_void) -> i32 {
        crate::error::set_last_error("stop failed in test");
        1
    }

    fn full_vtable() -> MalboxPluginVtable {
        MalboxPluginVtable {
            abi_version: MALBOX_ABI_VERSION,
            on_start: Some(mock_on_start),
            on_task: Some(mock_on_task),
            health_check: Some(mock_health_check),
            on_event: Some(mock_on_event),
            on_stop: Some(mock_on_stop),
            ..MalboxPluginVtable::default()
        }
    }

    fn empty_config() -> MalboxTestConfig {
        MalboxTestConfig {
            task_id: 1,
            sample_path: std::ptr::null(),
            config_keys: std::ptr::null(),
            config_values: std::ptr::null(),
            config_count: 0,
        }
    }

    #[test]
    fn full_sequence_calls_all_callbacks() {
        let _guard = COUNTER_LOCK.lock().unwrap();
        reset_counters();

        let rc = unsafe { malbox_test_run_plugin(full_vtable(), empty_config()) };

        assert_eq!(rc, 0, "expected success");
        assert_eq!(CALL_START.load(Ordering::SeqCst), 1, "on_start not called");
        assert_eq!(CALL_TASK.load(Ordering::SeqCst), 1, "on_task not called");
        assert_eq!(
            CALL_HEALTH.load(Ordering::SeqCst),
            1,
            "health_check not called"
        );
        assert_eq!(CALL_EVENT.load(Ordering::SeqCst), 1, "on_event not called");
        assert_eq!(CALL_STOP.load(Ordering::SeqCst), 1, "on_stop not called");
    }

    #[test]
    fn config_entries_passed_to_plugin() {
        let _guard = COUNTER_LOCK.lock().unwrap();
        reset_counters();

        let key1 = CString::new("timeout").unwrap();
        let val1 = CString::new("30").unwrap();
        let key_ptrs: Vec<*const c_char> = vec![key1.as_ptr()];
        let val_ptrs: Vec<*const c_char> = vec![val1.as_ptr()];

        let config = MalboxTestConfig {
            task_id: 42,
            sample_path: std::ptr::null(),
            config_keys: key_ptrs.as_ptr(),
            config_values: val_ptrs.as_ptr(),
            config_count: 1,
        };

        let rc = unsafe { malbox_test_run_plugin(full_vtable(), config) };
        assert_eq!(rc, 0);
    }

    #[test]
    fn abi_mismatch_returns_minus_one() {
        let vtable = MalboxPluginVtable {
            abi_version: MALBOX_ABI_VERSION + 99,
            ..MalboxPluginVtable::default()
        };
        let rc = unsafe { malbox_test_run_plugin(vtable, empty_config()) };
        assert_eq!(rc, -1);

        let err = crate::error::last_error_string();
        assert!(err.unwrap().contains("ABI version mismatch"));
    }

    #[test]
    fn on_stop_failure_returns_minus_one() {
        let _guard = COUNTER_LOCK.lock().unwrap();
        reset_counters();

        let vtable = MalboxPluginVtable {
            abi_version: MALBOX_ABI_VERSION,
            on_start: Some(mock_on_start),
            on_task: Some(mock_on_task),
            health_check: Some(mock_health_check),
            on_event: Some(mock_on_event),
            on_stop: Some(mock_on_stop_fail), // will fail
            ..MalboxPluginVtable::default()
        };

        let rc = unsafe { malbox_test_run_plugin(vtable, empty_config()) };
        assert_eq!(rc, -1);

        let err = crate::error::last_error_string();
        assert!(err.is_some());
        let msg = err.unwrap();
        assert!(
            msg.contains("stop failed in test") || msg.contains("on_stop failed"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn noop_vtable_succeeds() {
        // With no callbacks wired, every handler should return success (the
        // VtablePlugin treats None function pointers as no-ops).
        let vtable = MalboxPluginVtable {
            abi_version: MALBOX_ABI_VERSION,
            ..MalboxPluginVtable::default()
        };
        let rc = unsafe { malbox_test_run_plugin(vtable, empty_config()) };
        assert_eq!(rc, 0);
    }

    #[test]
    fn sample_path_is_forwarded() {
        let _guard = COUNTER_LOCK.lock().unwrap();
        reset_counters();

        let path = CString::new("/tmp/test_sample.bin").unwrap();
        let config = MalboxTestConfig {
            task_id: 7,
            sample_path: path.as_ptr(),
            config_keys: std::ptr::null(),
            config_values: std::ptr::null(),
            config_count: 0,
        };

        // We only verify it does not crash (the sample is not read in the
        // mock on_task callback).
        let rc = unsafe { malbox_test_run_plugin(full_vtable(), config) };
        assert_eq!(rc, 0);
        assert_eq!(CALL_TASK.load(Ordering::SeqCst), 1);
    }
}
