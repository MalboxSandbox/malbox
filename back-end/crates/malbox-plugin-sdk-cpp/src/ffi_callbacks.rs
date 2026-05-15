//! VtablePlugin adapter -- bridges the C vtable to the Rust `HostPlugin` trait.
//!
//! `VtablePlugin` holds a `MalboxPluginVtable` and implements the unified
//! `HostPlugin` trait from `malbox_plugin_sdk::plugin`.  Each trait method
//! marshals arguments into C-compatible types, invokes the corresponding
//! function pointer, and converts the return code back into a Rust `Result`.

use std::collections::HashMap;
use std::ffi::CString;

use malbox_plugin_sdk::context::Context;
use malbox_plugin_sdk::error::{Result, SdkError};
use malbox_plugin_sdk::health::HealthStatus;
use malbox_plugin_sdk::plugin::{HostPlugin, Plugin};
use malbox_plugin_transport::messages::events::Event;

use crate::error::last_error_string;
use crate::ffi_events::rust_event_to_c;
use crate::ffi_result::ResultBuilder;
use crate::ffi_types::{MalboxContext, MalboxHealthStatus};

/// Rust plugin adapter that wraps a C [`MalboxPluginVtable`](crate::ffi_types::MalboxPluginVtable)
/// and implements the `HostPlugin` trait.
///
/// Each trait method marshals its arguments into the C-compatible types
/// defined in [`crate::ffi_types`] and [`crate::ffi_events`], invokes the
/// corresponding vtable function pointer, and converts the integer return
/// code back to a Rust `Result`.  A null (absent) function pointer is treated
/// as a successful no-op.
pub(crate) struct VtablePlugin {
    vtable: crate::ffi_types::MalboxPluginVtable,
}

impl VtablePlugin {
    /// Create a new `VtablePlugin` from a filled-in C vtable.
    pub(crate) fn new(vtable: crate::ffi_types::MalboxPluginVtable) -> Self {
        Self { vtable }
    }
}

// SAFETY: The C++ HostPlugin object and vtable function pointers are required to be
// thread-safe per the SDK contract (documented in the spec).
unsafe impl Send for VtablePlugin {}
unsafe impl Sync for VtablePlugin {}

/// Helper to convert a non-zero return code to SdkError::Plugin
fn check_rc(rc: i32) -> Result<()> {
    if rc == 0 {
        Ok(())
    } else {
        let msg =
            last_error_string().unwrap_or_else(|| format!("plugin callback failed (rc={rc})"));

        #[derive(Debug)]
        struct PluginCallbackError(String);
        impl std::fmt::Display for PluginCallbackError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl std::error::Error for PluginCallbackError {}

        Err(SdkError::Plugin(Box::new(PluginCallbackError(msg))))
    }
}

impl Plugin for VtablePlugin {
    fn health_check(&self) -> HealthStatus {
        let health_check = match self.vtable.health_check {
            Some(f) => f,
            None => return HealthStatus::ready(),
        };

        let mut status = MalboxHealthStatus {
            ready: true,
            reason: std::ptr::null(),
        };

        // SAFETY: plugin_ptr is valid; &mut status is valid for the call duration.
        let rc = unsafe { health_check(self.vtable.plugin_ptr, &mut status) };

        if rc != 0 {
            return HealthStatus::not_ready("health_check callback failed");
        }

        let reason = if status.reason.is_null() {
            String::new()
        } else {
            unsafe { std::ffi::CStr::from_ptr(status.reason) }
                .to_string_lossy()
                .into_owned()
        };

        if status.ready {
            HealthStatus::ready()
        } else {
            HealthStatus::not_ready(reason)
        }
    }
}

impl HostPlugin for VtablePlugin {
    fn on_task(&self, ctx: &Context) -> Result<()> {
        let on_task = match self.vtable.on_task {
            Some(f) => f,
            None => return Ok(()),
        };

        let mut builder = ResultBuilder::new();
        let ctx_ptr = (ctx as *const Context) as *const MalboxContext;
        let builder_ptr =
            (&mut builder as *mut ResultBuilder) as *mut crate::ffi_types::MalboxResultBuilder;

        // SAFETY: ctx_ptr was created from &Context in this frame;
        // builder_ptr is valid for the duration of this call.
        let rc = unsafe { on_task(self.vtable.plugin_ptr, ctx_ptr, builder_ptr) };

        check_rc(rc)?;

        // Forward the accumulated results to the Rust runtime via the new
        // push-only API. The C++ surface (ResultBuilder) is unchanged.
        for result in builder.take() {
            ctx.results().push(result)?;
        }

        Ok(())
    }

    fn on_start(&self, raw_config: HashMap<String, String>) -> Result<()> {
        let on_start = match self.vtable.on_start {
            Some(f) => f,
            None => return Ok(()),
        };

        // Build parallel Vec<CString> for keys and values.
        let keys: Vec<CString> = raw_config
            .keys()
            .map(|k| {
                CString::new(k.as_str())
                    .unwrap_or_else(|_| CString::new("(key contained null byte)").unwrap())
            })
            .collect();

        let values: Vec<CString> = raw_config
            .values()
            .map(|v| {
                CString::new(v.as_str())
                    .unwrap_or_else(|_| CString::new("(value contained null byte)").unwrap())
            })
            .collect();

        let key_ptrs: Vec<*const std::ffi::c_char> = keys.iter().map(|cs| cs.as_ptr()).collect();
        let val_ptrs: Vec<*const std::ffi::c_char> = values.iter().map(|cs| cs.as_ptr()).collect();

        let count = raw_config.len();

        // SAFETY: key_ptrs and val_ptrs point into CStrings that live until the
        // end of this scope; count equals the length of both slices.
        let rc = unsafe {
            on_start(
                self.vtable.plugin_ptr,
                key_ptrs.as_ptr(),
                val_ptrs.as_ptr(),
                count,
            )
        };

        check_rc(rc)
    }

    fn on_stop(&self) -> Result<()> {
        let on_stop = match self.vtable.on_stop {
            Some(f) => f,
            None => return Ok(()),
        };

        // SAFETY: plugin_ptr is valid for the lifetime of the vtable.
        let rc = unsafe { on_stop(self.vtable.plugin_ptr) };
        check_rc(rc)
    }

    fn on_event(&self, event: Event) -> Result<()> {
        let on_event = match self.vtable.on_event {
            Some(f) => f,
            None => return Ok(()),
        };

        let owned = rust_event_to_c(&event);

        // SAFETY: owned.event is Copy; the OwnedCEvent keeps any backing
        // CStrings alive until after the callback returns.
        let rc = unsafe { on_event(self.vtable.plugin_ptr, owned.event) };
        check_rc(rc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffi_types::{MalboxContext, MalboxPluginVtable, MalboxResultBuilder};
    use std::ffi::CString;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn noop_vtable() -> MalboxPluginVtable {
        MalboxPluginVtable::default()
    }

    fn noop_emitter() -> Arc<dyn malbox_plugin_transport::traits::TransportEmitter + Send + Sync> {
        Arc::new(())
    }

    // -- on_event tests --

    #[test]
    fn event_none_is_noop() {
        let plugin = VtablePlugin::new(noop_vtable());
        assert!(plugin.on_event(Event::DaemonShutdown).is_ok());
    }

    // -- on_task tests --

    static ON_TASK_CALLED: AtomicBool = AtomicBool::new(false);

    unsafe extern "C" fn mock_on_task(
        _plugin: *mut std::ffi::c_void,
        _ctx: *const MalboxContext,
        builder: *mut MalboxResultBuilder,
    ) -> i32 {
        ON_TASK_CALLED.store(true, Ordering::SeqCst);
        let name = CString::new("result").unwrap();
        let data: &[u8] = b"{}";
        unsafe {
            crate::ffi_result::malbox_result_builder_push_json(
                builder,
                name.as_ptr(),
                data.as_ptr(),
                data.len(),
            );
        }
        0
    }

    #[test]
    fn on_task_dispatches_via_push_result() {
        ON_TASK_CALLED.store(false, Ordering::SeqCst);

        let vtable = MalboxPluginVtable {
            on_task: Some(mock_on_task),
            ..noop_vtable()
        };
        let plugin = VtablePlugin::new(vtable);

        // Use a real mpsc channel so we can observe what push sends.
        let (tx, mut rx) = tokio::sync::mpsc::channel(4);
        let ctx = Context::test_new_full(
            1,
            std::path::PathBuf::new(),
            HashMap::new(),
            noop_emitter(),
            Some(tx),
        );

        plugin.on_task(&ctx).expect("on_task should succeed");

        assert!(ON_TASK_CALLED.load(Ordering::SeqCst));
        let msg = rx.try_recv().expect("one result expected");
        assert_eq!(msg.result_name, "result");
    }

    #[test]
    fn on_task_none_is_noop() {
        let plugin = VtablePlugin::new(noop_vtable());
        let ctx = Context::test_new(noop_emitter());
        plugin.on_task(&ctx).expect("on_task should succeed");
        // No assertion on results -- a noop vtable should not push anything.
    }

    // -- on_start tests --

    static ON_START_CALLED: AtomicBool = AtomicBool::new(false);

    unsafe extern "C" fn mock_on_start(
        _plugin: *mut std::ffi::c_void,
        keys: *const *const std::ffi::c_char,
        vals: *const *const std::ffi::c_char,
        count: usize,
    ) -> i32 {
        ON_START_CALLED.store(true, Ordering::SeqCst);
        assert_eq!(count, 1, "expected 1 config entry");
        let key = unsafe { std::ffi::CStr::from_ptr(*keys) }
            .to_str()
            .unwrap()
            .to_owned();
        let val = unsafe { std::ffi::CStr::from_ptr(*vals) }
            .to_str()
            .unwrap()
            .to_owned();
        assert_eq!(key, "timeout");
        assert_eq!(val, "30");
        0
    }

    #[test]
    fn on_start_called_with_correct_key_value() {
        ON_START_CALLED.store(false, Ordering::SeqCst);

        let vtable = MalboxPluginVtable {
            on_start: Some(mock_on_start),
            ..noop_vtable()
        };
        let plugin = VtablePlugin::new(vtable);

        let mut config = HashMap::new();
        config.insert("timeout".to_string(), "30".to_string());
        plugin.on_start(config).expect("on_start failed");

        assert!(ON_START_CALLED.load(Ordering::SeqCst));
    }

    #[test]
    fn on_start_none_is_noop() {
        let plugin = VtablePlugin::new(noop_vtable());
        let mut config = HashMap::new();
        config.insert("k".to_string(), "v".to_string());
        assert!(plugin.on_start(config).is_ok());
    }

    // -- on_stop tests --

    static ON_STOP_CALLED: AtomicBool = AtomicBool::new(false);

    unsafe extern "C" fn mock_on_stop(_plugin: *mut std::ffi::c_void) -> i32 {
        ON_STOP_CALLED.store(true, Ordering::SeqCst);
        0
    }

    unsafe extern "C" fn mock_on_stop_error(_plugin: *mut std::ffi::c_void) -> i32 {
        crate::error::set_last_error("stop failed intentionally");
        1
    }

    #[test]
    fn on_stop_called() {
        ON_STOP_CALLED.store(false, Ordering::SeqCst);

        let vtable = MalboxPluginVtable {
            on_stop: Some(mock_on_stop),
            ..noop_vtable()
        };
        let plugin = VtablePlugin::new(vtable);
        plugin.on_stop().expect("on_stop failed");
        assert!(ON_STOP_CALLED.load(Ordering::SeqCst));
    }

    #[test]
    fn on_stop_none_is_noop() {
        let plugin = VtablePlugin::new(noop_vtable());
        assert!(plugin.on_stop().is_ok());
    }

    #[test]
    fn on_stop_error_returns_err() {
        let vtable = MalboxPluginVtable {
            on_stop: Some(mock_on_stop_error),
            ..noop_vtable()
        };
        let plugin = VtablePlugin::new(vtable);
        let result = plugin.on_stop();
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("stop failed intentionally"),
            "unexpected error: {msg}"
        );
    }

    // -- health_check tests --

    #[test]
    fn health_check_none_returns_ready() {
        let plugin = VtablePlugin::new(noop_vtable());
        let status = plugin.health_check();
        assert!(status.is_ready());
    }
}
