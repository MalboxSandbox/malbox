//! Plugin health status.
//!
//! The daemon polls [`HealthStatus`] via [`Plugin::health_check`](crate::plugin::Plugin::health_check)
//! to decide whether a plugin is ready to receive tasks.

/// Reports whether a plugin is ready to accept tasks.
///
/// Constructed via [`HealthStatus::ready`] or [`HealthStatus::not_ready`].
/// The daemon polls this periodically and will not dispatch tasks to a
/// plugin that reports not-ready.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct HealthStatus {
    pub(crate) ready: bool,
    pub(crate) reason: String,
}

impl HealthStatus {
    /// Create a status indicating the plugin is ready to accept tasks.
    pub fn ready() -> Self {
        Self {
            ready: true,
            reason: String::new(),
        }
    }

    /// Create a status indicating the plugin is **not** ready, with a reason.
    pub fn not_ready(reason: impl Into<String>) -> Self {
        Self {
            ready: false,
            reason: reason.into(),
        }
    }

    /// Return whether the plugin is ready to accept tasks.
    pub fn is_ready(&self) -> bool {
        self.ready
    }

    /// Return the reason string (empty if ready).
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_status_ready() {
        let status = HealthStatus::ready();
        assert!(status.is_ready());
        assert!(status.reason().is_empty());
    }

    #[test]
    fn health_status_not_ready() {
        let status = HealthStatus::not_ready("loading rules");
        assert!(!status.is_ready());
        assert_eq!(status.reason(), "loading rules");
    }

    #[test]
    fn health_status_exposes_getters() {
        let ready = HealthStatus::ready();
        assert!(ready.is_ready());
        assert_eq!(ready.reason(), "");

        let busy = HealthStatus::not_ready("loading rules");
        assert!(!busy.is_ready());
        assert_eq!(busy.reason(), "loading rules");
    }
}
