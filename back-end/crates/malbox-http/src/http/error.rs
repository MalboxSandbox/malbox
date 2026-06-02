use axum::{
    Json,
    http::{HeaderMap, HeaderValue, StatusCode, header::WWW_AUTHENTICATE},
    response::{IntoResponse, Response},
};
use malbox_database::Error as SqlxError;
use std::borrow::Cow;
use std::collections::HashMap;
use tracing::error;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Authentication required")]
    Unauthorized,

    #[error("User may not perform that action")]
    Forbidden,

    #[error("Request path not found")]
    NotFound,

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Error in the request body")]
    UnprocessableEntity {
        errors: HashMap<Cow<'static, str>, Vec<Cow<'static, str>>>,
    },

    #[error("Not implemented: {0}")]
    NotImplemented(String),

    #[error("An internal server error occurred: {0}")]
    Internal(String),
}

impl Error {
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
