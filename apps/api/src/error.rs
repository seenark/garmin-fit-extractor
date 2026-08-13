use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

use crate::model::{ApiErrorBody, ApiErrorDetail};

#[derive(Debug, Error)]
#[error("FIT payload is invalid")]
pub enum FitError {
    #[error("FIT payload is invalid")]
    InvalidFit,
}

#[derive(Clone, Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: &'static str,
    file_name: Option<String>,
}

impl ApiError {
    pub const fn new(status: StatusCode, code: &'static str, message: &'static str) -> Self {
        Self {
            status,
            code,
            message,
            file_name: None,
        }
    }

    pub fn with_file_name(mut self, file_name: String) -> Self {
        self.file_name = Some(file_name);
        self
    }

    pub const fn code(&self) -> &'static str {
        self.code
    }

    pub const fn invalid_multipart() -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "INVALID_MULTIPART",
            "The multipart request is invalid.",
        )
    }
    pub const fn empty_batch() -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "EMPTY_BATCH",
            "Upload at least one ZIP file.",
        )
    }
    pub const fn unknown_field() -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "UNKNOWN_FIELD",
            "Only the files field is accepted.",
        )
    }
    pub const fn too_many_files() -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "TOO_MANY_FILES",
            "Upload at most 10 ZIP files.",
        )
    }
    pub const fn request_too_large() -> Self {
        Self::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "REQUEST_TOO_LARGE",
            "The upload request is too large.",
        )
    }
    pub const fn invalid_file_name() -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "INVALID_FILE_NAME",
            "File name must end with .zip and be at most 255 bytes.",
        )
    }
    pub const fn file_too_large() -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "FILE_TOO_LARGE",
            "Uploaded ZIP or extracted FIT member exceeds the 20 MiB limit.",
        )
    }
    pub const fn invalid_zip() -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "INVALID_ZIP",
            "File is not a valid ZIP archive or failed its integrity check.",
        )
    }
    pub const fn archive_limit_exceeded() -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "ARCHIVE_LIMIT_EXCEEDED",
            "ZIP archive exceeds the extracted FIT limits.",
        )
    }
    pub const fn invalid_fit() -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "INVALID_FIT",
            "File is not a valid FIT file or failed its integrity check.",
        )
    }
    pub const fn invalid_pagination() -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "INVALID_PAGINATION",
            "limit must be 1–100 and offset must be nonnegative.",
        )
    }
    pub const fn invalid_id() -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "INVALID_ID",
            "Extraction ID must be a valid UUID.",
        )
    }
    pub const fn not_found() -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "Extraction was not found.",
        )
    }
    pub const fn api_route_not_found() -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "API route was not found.",
        )
    }
    pub const fn invalid_view() -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "INVALID_VIEW",
            "view must be normalized or raw.",
        )
    }
    pub const fn invalid_activity_limit() -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "INVALID_ACTIVITY_LIMIT",
            "limit must be between 1 and 20.",
        )
    }
    pub const fn extraction_failed() -> Self {
        Self::new(
            StatusCode::CONFLICT,
            "EXTRACTION_FAILED",
            "JSON is unavailable for a failed extraction.",
        )
    }
    pub const fn processing_error() -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "PROCESSING_ERROR",
            "The FIT file could not be processed.",
        )
    }
    pub const fn database_error() -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DATABASE_ERROR",
            "The extraction could not be saved.",
        )
    }
    pub const fn service_unavailable() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "SERVICE_UNAVAILABLE",
            "The service is temporarily unavailable.",
        )
    }
}
impl ApiError {
    pub const fn auth_required() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "AUTH_REQUIRED",
            "Sign in with Google to continue.",
        )
    }
    pub const fn auth_not_configured() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "AUTH_NOT_CONFIGURED",
            "Google sign-in is not configured.",
        )
    }
    pub const fn auth_provider_unavailable() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "AUTH_PROVIDER_UNAVAILABLE",
            "Google sign-in is temporarily unavailable.",
        )
    }
}
impl ApiError {
    pub const fn coach_not_configured() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "COACH_NOT_CONFIGURED",
            "FIT Coach OAuth is not configured.",
        )
    }

    pub const fn coach_authentication_failed() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "COACH_AUTHENTICATION_FAILED",
            "Bearer authentication failed.",
        )
    }

    pub const fn coach_insufficient_scope() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            "INSUFFICIENT_SCOPE",
            "The bearer token lacks activities:read scope.",
        )
    }

    pub const fn activity_not_found() -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "ACTIVITY_NOT_FOUND",
            "Activity was not found.",
        )
    }

    pub const fn invalid_activity_detail() -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "INVALID_ACTIVITY_DETAIL",
            "detail must be summary or laps.",
        )
    }

    pub const fn coach_processing_error() -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "COACH_PROCESSING_ERROR",
            "The activity could not be processed.",
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ApiErrorBody {
                error: ApiErrorDetail {
                    code: self.code.to_owned(),
                    message: self.message.to_owned(),
                    file_name: self.file_name,
                },
            }),
        )
            .into_response()
    }
}
