pub mod artifacts;
pub mod cancel;
pub mod create;
pub mod get;
pub mod report;

use malbox_scheduler::ControlError;

use crate::http::error::Error;

/// Map an orchestrator control error onto the HTTP error surface.
pub(super) fn control_error(e: ControlError) -> Error {
    match e {
        ControlError::TaskNotFound | ControlError::ArtifactNotFound | ControlError::JobNotFound => {
            Error::NotFound
        }
        ControlError::Conflict(msg) => Error::Conflict(msg),
        e @ (ControlError::Internal(_) | ControlError::Unavailable(_)) => {
            Error::Internal(e.to_string())
        }
    }
}
