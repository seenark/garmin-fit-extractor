use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::get,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use time::OffsetDateTime;
use url::Url;

use crate::{app::AppState, auth::AuthenticatedUser, error::ApiError};

const YOUTUBE_HOSTS: &[&str] = &[
    "m.youtube.com",
    "youtube-nocookie.com",
    "youtube.com",
    "www.youtube-nocookie.com",
    "www.youtube.com",
    "www.youtu.be",
    "youtu.be",
];

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TranscriptEntryInput {
    #[serde(default)]
    pub channel_name: String,
    #[serde(default)]
    pub youtube_url: String,
    #[serde(default)]
    pub video_id: Option<String>,
    #[serde(default)]
    pub transcription: String,
}

#[derive(Clone, Debug, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptEntry {
    pub id: i64,
    pub channel_name: String,
    pub youtube_url: String,
    pub video_id: String,
    pub transcription: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptEntryList {
    pub items: Vec<TranscriptEntry>,
}

#[derive(Debug, Serialize)]
pub struct TranscriptEntryDeleteAllResponse {
    pub deleted: u64,
}

#[derive(Debug)]
struct NormalizedInput {
    channel_name: String,
    youtube_url: String,
    video_id: String,
    transcription: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/admin/transcript-entries",
            get(list_entries)
                .post(create_entry)
                .delete(delete_all_entries),
        )
        .route(
            "/api/admin/transcript-entries/{id}",
            get(get_entry).put(update_entry).delete(delete_entry),
        )
}

async fn list_entries(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<Json<TranscriptEntryList>, ApiError> {
    require_admin(&state, &user)?;
    let items = sqlx::query_as::<_, TranscriptEntry>(
        "SELECT id, channel_name, youtube_url, video_id, transcription, created_at, updated_at
         FROM transcript_entries
         ORDER BY updated_at DESC, id DESC",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|error| {
        tracing::error!(%error, "listing transcript entries failed");
        ApiError::transcript_database_error()
    })?;
    Ok(Json(TranscriptEntryList { items }))
}

async fn get_entry(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<Json<TranscriptEntry>, ApiError> {
    require_admin(&state, &user)?;
    let id = parse_id(&id)?;
    let entry = sqlx::query_as::<_, TranscriptEntry>(
        "SELECT id, channel_name, youtube_url, video_id, transcription, created_at, updated_at
         FROM transcript_entries
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|error| {
        tracing::error!(%error, "getting transcript entry failed");
        ApiError::transcript_database_error()
    })?;
    entry.map(Json).ok_or_else(ApiError::transcript_not_found)
}

async fn create_entry(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    headers: HeaderMap,
    Json(input): Json<TranscriptEntryInput>,
) -> Result<(StatusCode, Json<TranscriptEntry>), ApiError> {
    require_admin(&state, &user)?;
    require_same_origin(&state, &headers)?;
    let input = normalize_input(input)?;
    let now = timestamp_millis();
    let result = sqlx::query_as::<_, TranscriptEntry>(
        "INSERT INTO transcript_entries
            (channel_name, youtube_url, video_id, transcription, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $5)
         RETURNING id, channel_name, youtube_url, video_id, transcription, created_at, updated_at",
    )
    .bind(input.channel_name)
    .bind(input.youtube_url)
    .bind(input.video_id)
    .bind(input.transcription)
    .bind(now)
    .fetch_one(&state.db)
    .await;
    match result {
        Ok(entry) => Ok((StatusCode::CREATED, Json(entry))),
        Err(error) if is_unique_violation(&error) => Err(ApiError::transcript_duplicate_video()),
        Err(error) => {
            tracing::error!(%error, "creating transcript entry failed");
            Err(ApiError::transcript_database_error())
        }
    }
}

async fn update_entry(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<TranscriptEntryInput>,
) -> Result<Json<TranscriptEntry>, ApiError> {
    require_admin(&state, &user)?;
    require_same_origin(&state, &headers)?;
    let id = parse_id(&id)?;
    let input = normalize_input(input)?;
    let now = timestamp_millis();
    let result = sqlx::query_as::<_, TranscriptEntry>(
        "UPDATE transcript_entries
         SET channel_name = $1,
             youtube_url = $2,
             video_id = $3,
             transcription = $4,
             updated_at = GREATEST($5, updated_at + 1)
         WHERE id = $6
         RETURNING id, channel_name, youtube_url, video_id, transcription, created_at, updated_at",
    )
    .bind(input.channel_name)
    .bind(input.youtube_url)
    .bind(input.video_id)
    .bind(input.transcription)
    .bind(now)
    .bind(id)
    .fetch_optional(&state.db)
    .await;
    match result {
        Ok(Some(entry)) => Ok(Json(entry)),
        Ok(None) => Err(ApiError::transcript_not_found()),
        Err(error) if is_unique_violation(&error) => Err(ApiError::transcript_duplicate_video()),
        Err(error) => {
            tracing::error!(%error, "updating transcript entry failed");
            Err(ApiError::transcript_database_error())
        }
    }
}

async fn delete_entry(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_admin(&state, &user)?;
    require_same_origin(&state, &headers)?;
    let id = parse_id(&id)?;
    let result = sqlx::query("DELETE FROM transcript_entries WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(|error| {
            tracing::error!(%error, "deleting transcript entry failed");
            ApiError::transcript_database_error()
        })?;
    if result.rows_affected() == 0 {
        return Err(ApiError::transcript_not_found());
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_all_entries(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Result<Json<TranscriptEntryDeleteAllResponse>, ApiError> {
    require_admin(&state, &user)?;
    require_same_origin(&state, &headers)?;
    let confirmed = payload.as_object().is_some_and(|object| {
        object.len() == 1
            && object
                .get("confirmation")
                .and_then(Value::as_str)
                .is_some_and(|value| value == "DELETE_ALL")
    });
    if !confirmed {
        return Err(ApiError::transcript_confirmation_required());
    }

    let result = sqlx::query("DELETE FROM transcript_entries")
        .execute(&state.db)
        .await
        .map_err(|error| {
            tracing::error!(%error, "deleting all transcript entries failed");
            ApiError::transcript_database_error()
        })?;
    Ok(Json(TranscriptEntryDeleteAllResponse {
        deleted: result.rows_affected(),
    }))
}

fn require_admin(state: &AppState, user: &AuthenticatedUser) -> Result<(), ApiError> {
    state
        .auth
        .is_admin_email(&user.email)
        .then_some(())
        .ok_or_else(ApiError::admin_forbidden)
}

fn require_same_origin(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let Some(expected_origin) = state.app_origin.as_deref() else {
        return Err(ApiError::admin_origin_forbidden());
    };
    let Some(actual_origin) = headers.get(axum::http::header::ORIGIN) else {
        return Err(ApiError::admin_origin_forbidden());
    };
    let Ok(actual_origin) = actual_origin.to_str() else {
        return Err(ApiError::admin_origin_forbidden());
    };
    (actual_origin == expected_origin)
        .then_some(())
        .ok_or_else(ApiError::admin_origin_forbidden)
}

fn parse_id(value: &str) -> Result<i64, ApiError> {
    value
        .parse::<i64>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(ApiError::transcript_not_found)
}

fn normalize_input(input: TranscriptEntryInput) -> Result<NormalizedInput, ApiError> {
    if !validate_required_fields(
        &input.channel_name,
        &input.youtube_url,
        &input.transcription,
    ) {
        return Err(ApiError::invalid_transcript_entry());
    }
    let channel_name = input.channel_name.trim().to_owned();
    let youtube_url = input.youtube_url.trim().to_owned();
    let video_id =
        parse_youtube_video_id(&youtube_url).map_err(|_| ApiError::invalid_transcript_entry())?;
    if let Some(provided) = input.video_id {
        let provided = provided.trim();
        if provided.is_empty() || provided != video_id {
            return Err(ApiError::invalid_transcript_entry());
        }
    }
    Ok(NormalizedInput {
        channel_name,
        youtube_url,
        video_id,
        transcription: input.transcription,
    })
}

fn parse_youtube_video_id(value: &str) -> Result<String, ()> {
    let value = value.trim();
    if value.is_empty() {
        return Err(());
    }
    let parsed = Url::parse(value).map_err(|_| ())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(());
    }
    let hostname = parsed.host_str().ok_or(())?.to_ascii_lowercase();
    if !YOUTUBE_HOSTS.contains(&hostname.as_str()) {
        return Err(());
    }

    let path_segments: Vec<_> = parsed
        .path_segments()
        .ok_or(())?
        .filter(|segment| !segment.is_empty())
        .collect();
    let video_id = if matches!(hostname.as_str(), "youtu.be" | "www.youtu.be") {
        (path_segments.len() == 1).then(|| path_segments[0].to_owned())
    } else if parsed.path() == "/watch" {
        parsed
            .query_pairs()
            .find_map(|(key, value)| (key == "v").then(|| value.into_owned()))
    } else if path_segments.len() == 2 && matches!(path_segments[0], "shorts" | "embed") {
        Some(path_segments[1].to_owned())
    } else {
        None
    }
    .ok_or(())?;

    if video_id.len() < 6
        || !video_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(());
    }
    Ok(video_id)
}

fn validate_required_fields(channel_name: &str, youtube_url: &str, transcription: &str) -> bool {
    !channel_name.trim().is_empty()
        && !transcription.trim().is_empty()
        && parse_youtube_video_id(youtube_url).is_ok()
}

fn timestamp_millis() -> i64 {
    i64::try_from(OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000)
        .expect("current Unix timestamp fits in BIGINT")
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .and_then(|database_error| database_error.code())
        .is_some_and(|code| code == "23505")
}

#[cfg(test)]
mod tests {
    use super::{parse_youtube_video_id, validate_required_fields};

    #[test]
    fn accepts_supported_youtube_urls_and_rejects_non_youtube_urls() {
        assert_eq!(
            parse_youtube_video_id(" https://www.youtube.com/watch?v=abc123XYZ99&si=one ").unwrap(),
            "abc123XYZ99"
        );
        assert_eq!(
            parse_youtube_video_id("https://youtu.be/abc123XYZ99").unwrap(),
            "abc123XYZ99"
        );
        assert_eq!(
            parse_youtube_video_id("https://www.youtube.com/shorts/abc123XYZ99").unwrap(),
            "abc123XYZ99"
        );
        assert!(parse_youtube_video_id("https://example.com/watch?v=abc123XYZ99").is_err());
        assert!(parse_youtube_video_id("https://youtu.be/nope").is_err());
    }

    #[test]
    fn required_fields_trim_for_validation_without_rewriting_transcription() {
        assert!(validate_required_fields(
            "  Channel  ",
            "https://youtu.be/abc123XYZ99",
            "\nbody\n"
        ));
        assert!(!validate_required_fields(
            "  ",
            "https://youtu.be/abc123XYZ99",
            "body"
        ));
        assert!(!validate_required_fields(
            "Channel",
            "https://youtu.be/abc123XYZ99",
            " \n\t "
        ));
    }
}
