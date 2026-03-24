//! Sample execution parameter resolution.
//!
//! Determines *how* to run a sample on the guest OS based on the target
//! filename and platform.  Today this is a simple direct-execution strategy;
//! future extensions can inspect file type, use package-specific launchers
//! (e.g. `rundll32` for DLLs, `cscript` for VBS), or respect user-provided
//! execution config from the task submission.

use malbox_machinery::Platform;

/// Parameters for executing a sample on the guest OS via `ExecuteCommand`.
#[derive(Debug, Clone)]
pub struct ExecParams {
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub env: Vec<(String, String)>,
    /// Run the process in the background so it doesn't block the gRPC call.
    pub background: bool,
}

/// Resolve execution parameters for a sample target on the given platform.
///
/// The returned [`ExecParams`] are passed directly to the guest plugin's
/// `ExecuteCommand` RPC.  The `cwd` of `"."` resolves to the guest plugin's
/// work directory — the same directory where `PushFile` placed the sample.
pub fn resolve_exec_params(target: &str, platform: Platform) -> ExecParams {
    match platform {
        Platform::Windows => ExecParams {
            command: "cmd".into(),
            args: vec!["/c".into(), target.into()],
            cwd: Some(".".into()),
            env: vec![],
            background: true,
        },
        Platform::Linux => ExecParams {
            command: "sh".into(),
            args: vec!["-c".into(), format!("chmod +x ./{target} && ./{target}")],
            cwd: Some(".".into()),
            env: vec![],
            background: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_exec_params_uses_cmd() {
        let params = resolve_exec_params("sample.exe", Platform::Windows);
        assert_eq!(params.command, "cmd");
        assert_eq!(params.args, vec!["/c", "sample.exe"]);
        assert_eq!(params.cwd.as_deref(), Some("."));
        assert!(params.background);
    }

    #[test]
    fn linux_exec_params_uses_sh_with_chmod() {
        let params = resolve_exec_params("sample.elf", Platform::Linux);
        assert_eq!(params.command, "sh");
        assert_eq!(
            params.args,
            vec!["-c", "chmod +x ./sample.elf && ./sample.elf"]
        );
        assert_eq!(params.cwd.as_deref(), Some("."));
        assert!(params.background);
    }
}
