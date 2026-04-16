//! Command execution types: `ExecRequest`, `ExecResult`, `ExecutionInfo`.

use std::collections::HashMap;
use std::time::Duration;

/// A command execution request passed to `Plugin::on_execute_command`.
#[non_exhaustive]
pub struct ExecRequest {
    pub(crate) command: String,
    pub(crate) args: Vec<String>,
    pub(crate) cwd: Option<String>,
    pub(crate) env: HashMap<String, String>,
    pub(crate) timeout: Option<Duration>,
    pub(crate) background: bool,
}

impl ExecRequest {
    /// Construct an `ExecRequest`. Used by the runtime; downstream test code
    /// should use [`ExecRequest::test_new`](crate::testkit) under the
    /// `testkit` feature.
    pub(crate) fn new(
        command: String,
        args: Vec<String>,
        cwd: Option<String>,
        env: HashMap<String, String>,
        timeout: Option<Duration>,
        background: bool,
    ) -> Self {
        Self {
            command,
            args,
            cwd,
            env,
            timeout,
            background,
        }
    }

    pub fn command(&self) -> &str {
        &self.command
    }

    pub fn args(&self) -> &[String] {
        &self.args
    }

    pub fn cwd(&self) -> Option<&str> {
        self.cwd.as_deref()
    }

    pub fn env(&self) -> &HashMap<String, String> {
        &self.env
    }

    pub fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    pub fn background(&self) -> bool {
        self.background
    }
}

/// Result of executing a command on the guest OS.
///
/// Returned by `Plugin::on_execute_command` when the plugin handles the
/// command itself (wrapped in `Some`). Return `None` to let the runtime
/// use its built-in executor.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ExecResult {
    pub(crate) exit_code: Option<i32>,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) pid: Option<u32>,
}

impl ExecResult {
    /// Create a new `ExecResult` with the given exit code and empty output.
    /// Use `set_stdout` / `set_stderr` / `set_pid` to populate the rest.
    pub fn new(exit_code: Option<i32>) -> Self {
        Self {
            exit_code,
            stdout: String::new(),
            stderr: String::new(),
            pid: None,
        }
    }

    pub fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }

    pub fn stdout(&self) -> &str {
        &self.stdout
    }

    pub fn stderr(&self) -> &str {
        &self.stderr
    }

    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    pub fn set_pid(&mut self, pid: u32) {
        self.pid = Some(pid);
    }

    pub fn set_stdout(&mut self, stdout: impl Into<String>) {
        self.stdout = stdout.into();
    }

    pub fn set_stderr(&mut self, stderr: impl Into<String>) {
        self.stderr = stderr.into();
    }
}

/// Metadata about a sample execution, delivered to `on_task` after
/// `on_execute_command` completes.
///
/// Obtained by calling `ctx.wait_for_execution()` inside an `on_task` handler.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ExecutionInfo {
    pub(crate) pid: Option<u32>,
    pub(crate) command: String,
    pub(crate) args: Vec<String>,
}

impl ExecutionInfo {
    /// Construct an `ExecutionInfo`. Used by the runtime; downstream test code
    /// should use [`ExecutionInfo::test_new`](crate::testkit) under the
    /// `testkit` feature.
    pub(crate) fn new(pid: Option<u32>, command: String, args: Vec<String>) -> Self {
        Self { pid, command, args }
    }

    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    pub fn command(&self) -> &str {
        &self.command
    }

    pub fn args(&self) -> &[String] {
        &self.args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_result_constructed_with_pid_and_defaults() {
        let mut result = ExecResult::new(None);
        result.set_pid(1234);
        assert_eq!(result.pid(), Some(1234));
        assert_eq!(result.exit_code(), None);
        assert_eq!(result.stdout(), "");
        assert_eq!(result.stderr(), "");
    }

    #[test]
    fn execution_info_stores_metadata() {
        let info = ExecutionInfo::new(
            Some(4567),
            "cmd".to_string(),
            vec!["/c".to_string(), "sample.exe".to_string()],
        );
        assert_eq!(info.pid(), Some(4567));
        assert_eq!(info.command(), "cmd");
        assert_eq!(info.args().len(), 2);
    }

    #[test]
    fn execution_info_exposes_getters() {
        let info = ExecutionInfo::new(
            Some(4567),
            "cmd".to_string(),
            vec!["/c".to_string(), "sample.exe".to_string()],
        );

        assert_eq!(info.pid(), Some(4567));
        assert_eq!(info.command(), "cmd");
        assert_eq!(info.args(), &["/c".to_string(), "sample.exe".to_string()]);
    }

    #[test]
    fn exec_result_new_and_setters() {
        let mut r = ExecResult::new(Some(0));
        assert_eq!(r.exit_code(), Some(0));
        assert_eq!(r.stdout(), "");
        assert_eq!(r.stderr(), "");
        assert_eq!(r.pid(), None);

        r.set_stdout("hello".to_string());
        r.set_stderr("oops");
        r.set_pid(1234);

        assert_eq!(r.stdout(), "hello");
        assert_eq!(r.stderr(), "oops");
        assert_eq!(r.pid(), Some(1234));
    }

    #[test]
    fn exec_request_exposes_getters() {
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/usr/bin".to_string());

        let req = ExecRequest::new(
            "cmd".to_string(),
            vec!["/c".to_string(), "dir".to_string()],
            Some("C:\\".to_string()),
            env,
            Some(Duration::from_secs(30)),
            true,
        );

        assert_eq!(req.command(), "cmd");
        assert_eq!(req.args(), &["/c".to_string(), "dir".to_string()]);
        assert_eq!(req.cwd(), Some("C:\\"));
        assert_eq!(req.env().get("PATH"), Some(&"/usr/bin".to_string()));
        assert_eq!(req.timeout(), Some(Duration::from_secs(30)));
        assert!(req.background());
    }
}
