use thiserror::Error;

#[derive(Debug, Error)]
pub enum KvmError {
    #[error("Libvirt error: {0}")]
    Libvirt(String),
}
