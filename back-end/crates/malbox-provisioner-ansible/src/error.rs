use thiserror::Error;

/// Failure raised while configuring or running `ansible-playbook`.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AnsibleError {
    /// The playbook process could not be spawned or awaited.
    #[error("Ansible execution failed: {0}")]
    Execution(String),

    /// The provisioner configuration is invalid (e.g. missing playbook).
    #[error("Ansible configuration error: {0}")]
    Config(String),

    /// Host I/O error, e.g. writing the temporary inventory.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
