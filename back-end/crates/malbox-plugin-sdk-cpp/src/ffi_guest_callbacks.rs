//! GuestVtablePlugin adapter - bridges the C guest vtable to the Rust GuestPlugin trait.

use std::ffi::CStr;
use std::path::Path;

use malbox_plugin_sdk::context::Context;
use malbox_plugin_sdk::error::{Result, SdkError};
use malbox_plugin_sdk::health::HealthStatus;
use malbox_plugin_sdk::plugin::Plugin;
use malbox_plugin_sdk::plugin::guest::{GuestPlugin, LaunchResult, default_launch};

use crate::error::last_error_string;
use crate::ffi_types::{MalboxContext, MalboxGuestPluginVtable, MalboxHealthStatus};

pub(crate) struct GuestVtablePlugin {
    vtable: MalboxGuestPluginVtable,
}

impl GuestVtablePlugin {
    pub(crate) fn new(vtable: MalboxGuestPluginVtable) -> Self {
        Self { vtable }
    }
}

unsafe impl Send for GuestVtablePlugin {}
unsafe impl Sync for GuestVtablePlugin {}

fn check_rc(rc: i32) -> Result<()> {
    if rc == 0 {
        Ok(())
    } else {
        let msg = last_error_string().unwrap_or_else(|| "unknown error".into());

        #[derive(Debug)]
        struct GuestCallbackError(String);
        impl std::fmt::Display for GuestCallbackError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl std::error::Error for GuestCallbackError {}

        Err(SdkError::Plugin(Box::new(GuestCallbackError(msg))))
    }
}

impl Plugin for GuestVtablePlugin {
    fn health_check(&self) -> HealthStatus {
        let f = match self.vtable.health_check {
            Some(f) => f,
            None => return HealthStatus::ready(),
        };
        let mut status = MalboxHealthStatus {
            ready: true,
            reason: std::ptr::null(),
        };
        let rc = unsafe { f(self.vtable.plugin_ptr, &mut status) };
        if rc != 0 {
            return HealthStatus::not_ready("health_check callback failed");
        }
        let reason = if status.reason.is_null() {
            String::new()
        } else {
            unsafe { CStr::from_ptr(status.reason) }
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

impl GuestPlugin for GuestVtablePlugin {
    fn on_start(&self, ctx: &Context) -> Result<()> {
        let f = match self.vtable.on_start {
            Some(f) => f,
            None => return Ok(()),
        };
        let ctx_ptr = ctx as *const Context as *const MalboxContext;
        let rc = unsafe { f(self.vtable.plugin_ptr, ctx_ptr) };
        check_rc(rc)
    }

    fn on_stop(&self, ctx: &Context) -> Result<()> {
        let f = match self.vtable.on_stop {
            Some(f) => f,
            None => return Ok(()),
        };
        let ctx_ptr = ctx as *const Context as *const MalboxContext;
        let rc = unsafe { f(self.vtable.plugin_ptr, ctx_ptr) };
        check_rc(rc)
    }

    fn execute_sample(&self, sample_path: &Path) -> Result<LaunchResult> {
        let f = match self.vtable.execute_sample {
            Some(f) => f,
            None => return Ok(default_launch(sample_path)),
        };
        let path_str = match sample_path.to_str() {
            Some(s) => s,
            None => return Ok(LaunchResult::UseDefault),
        };
        let c_path = match std::ffi::CString::new(path_str) {
            Ok(c) => c,
            Err(_) => return Ok(LaunchResult::UseDefault),
        };
        let rc = unsafe { f(self.vtable.plugin_ptr, c_path.as_ptr()) };
        match rc {
            0 => Ok(LaunchResult::UseDefault),
            1 => Ok(LaunchResult::Launched),
            _ => Ok(LaunchResult::UseDefault),
        }
    }
}
