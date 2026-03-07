use thiserror::Error;

#[derive(Debug, Error)]
pub enum AnsibleError {
    #[error("Ansible execution failed: {0}")]
    Execution(String),

    #[error("Ansible configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
