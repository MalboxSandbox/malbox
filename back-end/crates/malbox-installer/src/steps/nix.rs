use crate::config::NixStrategy;
use crate::error::{InstallError, Step};
use crate::progress::InstallProgress;

pub fn detect_nix() -> bool {
    std::process::Command::new("nix")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

pub async fn execute(strategy: &NixStrategy, progress: &dyn InstallProgress) -> crate::Result<()> {
    match strategy {
        NixStrategy::Skip | NixStrategy::Existing => Ok(()),
        NixStrategy::Install => {
            progress.started(
                Step::Nix,
                "Installing Nix via Determinate Systems installer",
            );

            let status = tokio::process::Command::new("sh")
                .arg("-c")
                .arg("curl --proto '=https' --tlsv1.2 -sSf -L https://install.determinate.systems/nix | sh -s -- install --no-confirm")
                .status()
                .await
                .map_err(|e| InstallError::StepFailed {
                    step: Step::Nix,
                    message: format!("failed to run Nix installer: {e}"),
                })?;

            if !status.success() {
                return Err(InstallError::StepFailed {
                    step: Step::Nix,
                    message: "Nix installer exited with non-zero status".to_string(),
                });
            }

            progress.completed(Step::Nix);
            Ok(())
        }
    }
}
