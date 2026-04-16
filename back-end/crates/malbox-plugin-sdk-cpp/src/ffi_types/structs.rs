use std::ffi::c_char;

use super::enums::{MalboxExecutionContext, MalboxPluginState, MalboxPluginType};

/// Health status reported by a plugin's `health_check` callback.
///
/// The `reason` field is optional: when `ready` is `true` it may be null.
/// When `ready` is `false`, `reason` should point to a null-terminated string
/// describing why the plugin is not ready.  The string must remain valid for
/// the duration of the `health_check` call.
#[repr(C)]
pub struct MalboxHealthStatus {
    /// `true` if the plugin is ready to accept tasks.
    pub ready: bool,
    /// Optional null-terminated string describing the health state.  May be
    /// null when `ready` is `true`.
    pub reason: *const c_char,
}

/// Static metadata that a C++ plugin provides when registering with the daemon.
///
/// String fields (`name`, `version`, `authors`) must be non-null,
/// null-terminated, valid UTF-8 C strings.  `description` is optional and may
/// be null.
#[repr(C)]
pub struct MalboxPluginMeta {
    /// Unique plugin name (non-null, null-terminated UTF-8).
    pub name: *const c_char,
    /// Semantic version string (non-null, null-terminated UTF-8).
    pub version: *const c_char,
    /// Optional human-readable description (null-terminated UTF-8, or null).
    pub description: *const c_char,
    /// Author(s) of the plugin (non-null, null-terminated UTF-8).
    pub authors: *const c_char,
    /// Whether this is a host or guest plugin.
    pub plugin_type: MalboxPluginType,
    /// Lifetime policy for the plugin instance.
    pub state: MalboxPluginState,
    /// Concurrency scheduling policy.
    pub execution: MalboxExecutionContext,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_layout() {
        let hs = MalboxHealthStatus {
            ready: true,
            reason: std::ptr::null(),
        };
        assert!(hs.ready);
        assert!(hs.reason.is_null());
    }
}
