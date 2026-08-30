use std::str::FromStr;

use axum::{
    Json, Router,
    body::Bytes,
    extract::{FromRequest, Multipart, Path, Request, State, multipart::MultipartRejection},
    http::{HeaderValue, StatusCode, Uri, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use sqlx::SqlitePool;
use tokio::task;
use uuid::Uuid;

use crate::{
    app::AppState,
    auth::AuthenticatedUser,
    db::{self, HistoryOrder, NewFailure, NewSuccess, StoredExtraction},
    error::{ApiError, FitError},
    fit::{normalize::normalize, raw::decode_raw},
    model::{Analysis, BatchCreateResponse, ExtractionDetail, ExtractionStatus, RawFitRecord},
};

const MAX_FILES: usize = 10;
const MAX_FILE_BYTES: usize = 20 * 1024 * 1024;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/extractions",
            post(create_extractions)
                .layer(axum::extract::DefaultBodyLimit::max(210 * 1024 * 1024))
                .get(list_extractions)
                .delete(delete_extractions),
        )
        .route(
            "/api/v1/extractions/{id}",
            get(get_extraction).delete(delete_extraction),
        )
        .route(
            "/api/v1/extractions/{id}/download",
            get(download_extraction),
        )
        .route("/api/v1", axum::routing::any(api_not_found))
        .route("/api/v1/{*path}", axum::routing::any(api_not_found))
}

#[derive(Debug)]
pub struct PendingUpload {
    supplied_name: Option<String>,
    bytes: Vec<u8>,
    file_size_bytes: u64,
    too_large: bool,
}

pub struct UploadMultipart(pub Vec<PendingUpload>);

impl<S> FromRequest<S> for UploadMultipart
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let mut multipart = Multipart::from_request(request, state)
            .await
            .map_err(map_multipart_rejection)?;
        let mut uploads = Vec::new();
        while let Some(mut field) = multipart.next_field().await.map_err(map_multipart_error)? {
            if field.name() != Some("files") {
                return Err(ApiError::unknown_field());
            }
            if uploads.len() == MAX_FILES {
                return Err(ApiError::too_many_files());
            }
            let supplied_name = field.file_name().map(str::to_owned);
            let mut bytes = Vec::new();
            let mut file_size_bytes = 0_u64;
            let mut too_large = false;
            while let Some(chunk) = field.chunk().await.map_err(map_multipart_error)? {
                append_chunk(&mut bytes, &mut file_size_bytes, &mut too_large, chunk);
            }
            uploads.push(PendingUpload {
                supplied_name,
                bytes,
                file_size_bytes,
                too_large,
            });
        }
        if uploads.is_empty() {
            return Err(ApiError::empty_batch());
        }
        Ok(Self(uploads))
    }
}

fn append_chunk(bytes: &mut Vec<u8>, size: &mut u64, too_large: &mut bool, chunk: Bytes) {
    *size = size.saturating_add(chunk.len() as u64);
    if *too_large {
        return;
    }
    if bytes.len().saturating_add(chunk.len()) > MAX_FILE_BYTES {
        *too_large = true;
        return;
    }
    bytes.extend_from_slice(&chunk);
}

fn map_multipart_rejection(error: MultipartRejection) -> ApiError {
    if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
        ApiError::request_too_large()
    } else {
        tracing::debug!(%error, "multipart extractor rejected request");
        ApiError::invalid_multipart()
    }
}
fn map_multipart_error(error: axum::extract::multipart::MultipartError) -> ApiError {
    if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
        ApiError::request_too_large()
    } else {
        tracing::debug!(%error, "multipart stream rejected request");
        ApiError::invalid_multipart()
    }
}

async fn create_extractions(
    State(state): State<AppState>,
    crate::auth::AuthenticatedUser { user_id, .. }: crate::auth::AuthenticatedUser,
    UploadMultipart(uploads): UploadMultipart,
) -> Result<(StatusCode, Json<BatchCreateResponse>), ApiError> {
    let mut items = Vec::new();
    for upload in uploads {
        let archive_name = stored_file_name(upload.supplied_name.as_deref());
        if upload.too_large {
            items.push(
                insert_failure(
                    &state.db,
                    user_id,
                    archive_name,
                    upload.file_size_bytes,
                    "FILE_TOO_LARGE",
                    "Uploaded ZIP or extracted FIT file is larger than the 20-megabyte limit.",
                )
                .await?,
            );
            continue;
        }
        if !is_valid_file_name(&archive_name) {
            items.push(
                insert_failure(
                    &state.db,
                    user_id,
                    archive_name,
                    upload.file_size_bytes,
                    "INVALID_FILE_NAME",
                    "File name must end with .zip and be at most 255 bytes.",
                )
                .await?,
            );
            continue;
        }
        let archive_name_for_work = archive_name.clone();
        let archive = match run_blocking(move || {
            preflight_archive(upload.bytes, archive_name_for_work)
        })
        .await
        {
            Ok(value) => value,
            Err(error) => {
                let (code, message) = if error.code() == "ARCHIVE_LIMIT_EXCEEDED" {
                    (
                        "ARCHIVE_LIMIT_EXCEEDED",
                        "ZIP archive exceeds the extracted FIT limits.",
                    )
                } else {
                    (
                        "INVALID_ZIP",
                        "File is not a valid ZIP archive or failed its integrity check.",
                    )
                };
                vec![MemberResult::Failure {
                    name: archive_name.clone(),
                    size: upload.file_size_bytes,
                    code,
                    message,
                }]
            }
        };
        for result in archive {
            match result {
                MemberResult::Failure {
                    name,
                    size,
                    code,
                    message,
                } => {
                    items.push(insert_failure(&state.db, user_id, name, size, code, message).await?)
                }
                MemberResult::Success { bytes, name, size } => {
                    let process_name = name.clone();
                    let processed = run_blocking(move || {
                        Ok::<_, ApiError>(process_upload(bytes, process_name))
                    })
                    .await?;
                    match processed {
                        Ok(p) => items.push(
                            db::insert_success(
                                &state.db,
                                NewSuccess {
                                    user_id,
                                    file_name: p.file_name,
                                    file_size_bytes: size,
                                    activity_type: p.activity_type,
                                    activity_date: p.activity_date,
                                    normalized_json: p.normalized_json,
                                    raw_json: p.raw_json,
                                },
                            )
                            .await
                            .map_err(|_| ApiError::database_error())?,
                        ),
                        Err(FitError::InvalidFit) => items.push(
                            insert_failure(
                                &state.db,
                                user_id,
                                name,
                                size,
                                "INVALID_FIT",
                                "File is not a valid FIT file or failed its integrity check.",
                            )
                            .await?,
                        ),
                    }
                }
            }
        }
    }
    Ok((StatusCode::CREATED, Json(BatchCreateResponse { items })))
}

enum MemberResult {
    Success {
        bytes: Vec<u8>,
        name: String,
        size: u64,
    },
    Failure {
        name: String,
        size: u64,
        code: &'static str,
        message: &'static str,
    },
}

fn preflight_archive(bytes: Vec<u8>, archive_name: String) -> Result<Vec<MemberResult>, ApiError> {
    use std::io::{Cursor, Read};
    let archive_size = bytes.len() as u64;
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| ApiError::invalid_zip())?;
    let mut selected = Vec::new();
    let mut total = 0_u64;
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|_| ApiError::invalid_zip())?;
        if entry.is_dir() || !entry.name().to_ascii_lowercase().ends_with(".fit") {
            continue;
        }
        if entry.enclosed_name().is_none() {
            return Err(ApiError::invalid_zip());
        }
        if selected.len() == 50 {
            return Err(ApiError::archive_limit_exceeded());
        }
        let size = entry.size();
        total = total.saturating_add(size);
        let member = stored_file_name(Some(entry.name()));
        if !is_valid_member_name(&member) {
            return Err(ApiError::invalid_zip());
        }
        selected.push((index, member, size, size > MAX_FILE_BYTES as u64));
        if total > 100 * 1024 * 1024 {
            return Err(ApiError::archive_limit_exceeded());
        }
    }
    if selected.is_empty() {
        return Ok(vec![MemberResult::Failure {
            name: archive_name,
            size: archive_size,
            code: "NO_FIT_FILES",
            message: "ZIP archive contains no FIT files.",
        }]);
    }
    let mut results = Vec::with_capacity(selected.len());
    let mut actual_total = 0_u64;
    for (index, member, size, too_large) in selected {
        let display = format!("{}::{}", archive_name, member);
        if too_large {
            results.push(MemberResult::Failure {
                name: display,
                size,
                code: "FILE_TOO_LARGE",
                message: "Uploaded ZIP or extracted FIT file is larger than the 20-megabyte limit.",
            });
            continue;
        }
        let entry = archive
            .by_index(index)
            .map_err(|_| ApiError::invalid_zip())?;
        let mut data = Vec::with_capacity(size as usize);
        let mut limited = entry.take(MAX_FILE_BYTES as u64 + 1);
        let read_result = limited.read_to_end(&mut data);
        actual_total = actual_total.saturating_add(data.len() as u64);
        if actual_total > 100 * 1024 * 1024 {
            return Err(ApiError::archive_limit_exceeded());
        }
        if read_result.is_err() {
            results.push(MemberResult::Failure {
                name: display,
                size,
                code: "INVALID_FIT",
                message: "File is not a valid FIT file or failed its integrity check.",
            });
            continue;
        }
        if data.len() > MAX_FILE_BYTES {
            results.push(MemberResult::Failure {
                name: display,
                size: data.len() as u64,
                code: "FILE_TOO_LARGE",
                message: "Uploaded ZIP or extracted FIT file is larger than the 20-megabyte limit.",
            });
            continue;
        }
        let actual_size = data.len() as u64;
        results.push(MemberResult::Success {
            bytes: data,
            name: display,
            size: actual_size,
        });
    }
    Ok(results)
}

async fn insert_failure(
    pool: &SqlitePool,
    user_id: Uuid,
    file_name: String,
    file_size_bytes: u64,
    code: &str,
    message: &str,
) -> Result<crate::model::ExtractionSummary, ApiError> {
    db::insert_failure(
        pool,
        NewFailure {
            user_id,
            file_name,
            file_size_bytes,
            error_code: code.to_owned(),
            error_message: message.to_owned(),
        },
    )
    .await
    .map_err(|error| {
        tracing::error!(%error, "inserting failed extraction failed");
        ApiError::database_error()
    })
}
struct ProcessedUpload {
    file_name: String,
    activity_type: Option<String>,
    activity_date: Option<String>,
    normalized_json: String,
    raw_json: String,
}

fn process_upload(bytes: Vec<u8>, file_name: String) -> Result<ProcessedUpload, FitError> {
    let raw = decode_raw(&bytes)?;
    let normalized = normalize(&raw, &file_name);
    let activity_type = normalized.activity.r#type.clone();
    let activity_date = normalized.activity.date.clone();
    let normalized_json = serde_json::to_string(&normalized).map_err(|error| {
        tracing::error!(%error, "serializing normalized extraction failed");
        FitError::InvalidFit
    })?;
    let raw_json = serde_json::to_string(&raw).map_err(|error| {
        tracing::error!(%error, "serializing raw extraction failed");
        FitError::InvalidFit
    })?;
    Ok(ProcessedUpload {
        file_name,
        activity_type,
        activity_date,
        normalized_json,
        raw_json,
    })
}

fn stored_file_name(supplied_name: Option<&str>) -> String {
    let Some(name) = supplied_name else {
        return "invalid-file".to_owned();
    };
    let normalized_path = name.replace('\\', "/");
    let base_name = normalized_path
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .trim();
    if base_name.is_empty()
        || matches!(base_name, "." | "..")
        || base_name.chars().any(char::is_control)
    {
        "invalid-file".to_owned()
    } else {
        base_name.to_owned()
    }
}

fn is_valid_file_name(file_name: &str) -> bool {
    file_name != "invalid-file"
        && file_name.len() <= 255
        && !file_name.chars().any(char::is_control)
        && file_name.to_ascii_lowercase().ends_with(".zip")
}

fn is_valid_member_name(file_name: &str) -> bool {
    file_name != "invalid-file"
        && file_name.len() <= 255
        && !file_name.chars().any(char::is_control)
        && file_name.to_ascii_lowercase().ends_with(".fit")
}

async fn list_extractions(
    State(state): State<AppState>,
    AuthenticatedUser { user_id, .. }: AuthenticatedUser,
    uri: Uri,
) -> Result<Json<crate::model::ExtractionPage>, ApiError> {
    let (limit, offset, order) = parse_pagination(uri.query())?;
    db::list(&state.db, user_id, limit, offset, order)
        .await
        .map(Json)
        .map_err(|error| {
            tracing::error!(%error, "listing extractions failed");
            ApiError::database_error()
        })
}

fn parse_pagination(query: Option<&str>) -> Result<(u32, u32, HistoryOrder), ApiError> {
    let mut limit = 50;
    let mut offset = 0;
    let mut order = HistoryOrder::Desc;
    if let Some(query) = query {
        for pair in query.split('&') {
            let Some((key, value)) = pair.split_once('=') else {
                continue;
            };
            match key {
                "limit" => {
                    limit = u32::from_str(value).map_err(|_| ApiError::invalid_pagination())?
                }
                "offset" => {
                    offset = u32::from_str(value).map_err(|_| ApiError::invalid_pagination())?
                }
                "order" => {
                    order = match value {
                        "asc" => HistoryOrder::Asc,
                        "desc" => HistoryOrder::Desc,
                        _ => return Err(ApiError::invalid_pagination()),
                    }
                }
                _ => {}
            }
        }
    }
    if !(1..=100).contains(&limit) {
        return Err(ApiError::invalid_pagination());
    }
    Ok((limit, offset, order))
}

async fn get_extraction(
    State(state): State<AppState>,
    AuthenticatedUser { user_id, .. }: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<Json<ExtractionDetail>, ApiError> {
    let stored = get_stored(&state.db, user_id, &id).await?;
    if stored.summary.status == ExtractionStatus::Failed {
        return Ok(Json(ExtractionDetail {
            summary: stored.summary,
            normalized: None,
            raw: None,
        }));
    }
    Ok(Json(run_blocking(move || parse_detail(stored)).await?))
}

async fn download_extraction(
    State(state): State<AppState>,
    AuthenticatedUser { user_id, .. }: AuthenticatedUser,
    Path(id): Path<String>,
    uri: Uri,
) -> Result<Response, ApiError> {
    let stored = get_stored(&state.db, user_id, &id).await?;
    if stored.summary.status == ExtractionStatus::Failed {
        return Err(ApiError::extraction_failed());
    }
    let view = parse_view(uri.query())?.to_owned();
    let body_view = view.clone();
    let file_name = stored.summary.file_name.clone();
    let bytes = run_blocking(move || pretty_view(stored, &body_view)).await?;
    let disposition = format!(
        "attachment; filename=\"{}.{}.json\"",
        download_stem(&file_name),
        view
    );
    Ok((
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            ),
            (
                header::CONTENT_DISPOSITION,
                HeaderValue::from_str(&disposition).expect("sanitized disposition"),
            ),
        ],
        bytes,
    )
        .into_response())
}

async fn delete_extraction(
    State(state): State<AppState>,
    AuthenticatedUser { user_id, .. }: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let deleted = db::delete_one(&state.db, user_id, parse_id(&id)?)
        .await
        .map_err(|_| ApiError::database_error())?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found())
    }
}

async fn delete_extractions(
    State(state): State<AppState>,
    AuthenticatedUser { user_id, .. }: AuthenticatedUser,
) -> Result<StatusCode, ApiError> {
    db::delete_all(&state.db, user_id)
        .await
        .map_err(|_| ApiError::database_error())?;
    Ok(StatusCode::NO_CONTENT)
}
async fn api_not_found() -> ApiError {
    ApiError::api_route_not_found()
}

fn parse_view(query: Option<&str>) -> Result<&str, ApiError> {
    let value = query
        .and_then(|query| query.split('&').find_map(|item| item.strip_prefix("view=")))
        .ok_or_else(ApiError::invalid_view)?;
    match value {
        "normalized" | "raw" => Ok(value),
        _ => Err(ApiError::invalid_view()),
    }
}

fn parse_id(id: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(id).map_err(|_| ApiError::invalid_id())
}

async fn get_stored(
    pool: &SqlitePool,
    user_id: Uuid,
    id: &str,
) -> Result<StoredExtraction, ApiError> {
    db::get_stored(pool, user_id, parse_id(id)?)
        .await
        .map_err(|_| ApiError::database_error())?
        .ok_or_else(ApiError::not_found)
}

fn parse_detail(stored: StoredExtraction) -> Result<ExtractionDetail, ApiError> {
    let StoredExtraction {
        summary,
        normalized_json,
        raw_json,
    } = stored;
    let normalized = normalized_json.ok_or_else(ApiError::processing_error)?;
    let raw = raw_json.ok_or_else(ApiError::processing_error)?;
    Ok(ExtractionDetail {
        summary,
        normalized: Some(serde_json::from_str::<Analysis>(&normalized).map_err(log_processing)?),
        raw: Some(serde_json::from_str::<Vec<RawFitRecord>>(&raw).map_err(log_processing)?),
    })
}

fn pretty_view(stored: StoredExtraction, view: &str) -> Result<Vec<u8>, ApiError> {
    let json = match view {
        "normalized" => serde_json::to_vec_pretty(
            &serde_json::from_str::<Analysis>(
                &stored
                    .normalized_json
                    .ok_or_else(ApiError::processing_error)?,
            )
            .map_err(log_processing)?,
        ),
        "raw" => serde_json::to_vec_pretty(
            &serde_json::from_str::<Vec<RawFitRecord>>(
                &stored.raw_json.ok_or_else(ApiError::processing_error)?,
            )
            .map_err(log_processing)?,
        ),
        _ => return Err(ApiError::invalid_view()),
    }
    .map_err(log_processing)?;
    let mut json = json;
    json.push(b'\n');
    Ok(json)
}

fn log_processing(error: serde_json::Error) -> ApiError {
    tracing::error!(%error, "stored extraction JSON is invalid");
    ApiError::processing_error()
}

fn download_stem(file_name: &str) -> String {
    let stem = if file_name.len() >= 4
        && (file_name[file_name.len() - 4..].eq_ignore_ascii_case(".fit")
            || file_name[file_name.len() - 4..].eq_ignore_ascii_case(".zip"))
    {
        &file_name[..file_name.len() - 4]
    } else {
        file_name
    };
    let stem = stem
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '-' => character,
            _ => '_',
        })
        .collect::<String>();
    if stem.is_empty() {
        "extraction".to_owned()
    } else {
        stem
    }
}

async fn run_blocking<T, F>(work: F) -> Result<T, ApiError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, ApiError> + Send + 'static,
{
    match task::spawn_blocking(work).await {
        Ok(value) => value,
        Err(error) => {
            tracing::error!(%error, "blocking extraction task panicked or was cancelled");
            Err(ApiError::processing_error())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn maps_blocking_panic_to_processing_error() {
        let response = run_blocking::<(), _>(|| panic!("deliberate test panic"))
            .await
            .expect_err("panic maps to API error")
            .into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn sanitizes_download_stems() {
        assert_eq!(download_stem(" path /run fit.FIT"), "_path__run_fit");
        assert_eq!(download_stem(".fit"), "extraction");
    }
}
