use axum::{
    Json,
    http::{HeaderMap, HeaderValue, StatusCode, header::WWW_AUTHENTICATE},
    response::{IntoResponse, Response},
};
use malbox_database::Error as SqlxError;
use std::borrow::Cow;
use std::collections::HashMap;
use tracing::error;

/// API error, rendered as a JSON body with the matching HTTP status.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// 401: the request lacks valid credentials.
    #[error("Authentication required")]
    Unauthorized,

    /// 403: the caller is not allowed to perform the action.
    #[error("User may not perform that action")]
    Forbidden,

    /// 404: the requested resource does not exist.
    #[error("Request path not found")]
    NotFound,

    /// 409: the request conflicts with current state; holds the reason.
    #[error("Conflict: {0}")]
    Conflict(String),

    /// 400: the request is malformed; holds the reason.
    #[error("Bad request: {0}")]
    BadRequest(String),

    /// 422: field-level validation failures.
    #[error("Error in the request body")]
    UnprocessableEntity {
        /// Validation messages keyed by field name.
        errors: HashMap<Cow<'static, str>, Vec<Cow<'static, str>>>,
    },

    /// 501: the feature is not implemented; holds a description.
    #[error("Not implemented: {0}")]
    NotImplemented(String),

    /// 500: unexpected server failure; the details are logged, not returned.
    #[error("An internal server error occurred: {0}")]
    Internal(String),
}

impl Error {
    /// Build an [`Error::UnprocessableEntity`] with one message per field.
    pub fn unprocessable_entity<K, V>(errors: impl IntoIterator<Item = (K, V)>) -> Self
    where
        K: Into<Cow<'static, str>>,
        V: Into<Cow<'static, str>>,
    {
        let errors = errors
            .into_iter()
            .map(|(k, v)| (k.into(), vec![v.into()]))
            .collect();

        Self::UnprocessableEntity { errors }
    }

    fn status_code(&self) -> StatusCode {
        match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::UnprocessableEntity { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            Self::NotImplemented(_) => StatusCode::NOT_IMPLEMENTED,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        match self {
            Self::UnprocessableEntity { errors } => (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({ "errors": errors })),
            )
                .into_response(),
            Self::Unauthorized => {
                let mut headers = HeaderMap::new();
                headers.insert(WWW_AUTHENTICATE, HeaderValue::from_static("Token"));
                (
                    StatusCode::UNAUTHORIZED,
                    headers,
                    Json(serde_json::json!({ "error": self.to_string() })),
                )
                    .into_response()
            }
            Self::Internal(ref msg) => {
                error!(details = %msg, "Internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": "An internal server error occurred" })),
                )
                    .into_response()
            }
            ref other => (
                other.status_code(),
                Json(serde_json::json!({ "error": other.to_string() })),
            )
                .into_response(),
        }
    }
}

impl From<SqlxError> for Error {
    fn from(err: SqlxError) -> Self {
        error!(error = ?err, "Database error");
        Error::Internal("Database error occurred".to_string())
    }
}
