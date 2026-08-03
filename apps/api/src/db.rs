use std::{str::FromStr, time::Duration};

use sqlx::{
    Row, SqlitePool,
    sqlite::{
        SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteRow, SqliteSynchronous,
    },
};
use time::{OffsetDateTime, format_description::FormatItem, macros::format_description};
use uuid::Uuid;

use crate::model::{ApiErrorDetail, ExtractionPage, ExtractionStatus, ExtractionSummary};

const CREATED_AT_FORMAT: &[FormatItem<'static>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z");

pub struct NewSuccess {
    pub file_name: String,
    pub file_size_bytes: u64,
    pub activity_type: Option<String>,
    pub activity_date: Option<String>,
    pub normalized_json: String,
    pub raw_json: String,
}

pub struct NewFailure {
    pub file_name: String,
    pub file_size_bytes: u64,
    pub error_code: String,
    pub error_message: String,
}

pub struct StoredExtraction {
    pub summary: ExtractionSummary,
    pub normalized_json: Option<String>,
    pub raw_json: Option<String>,
}

pub async fn connect(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5));
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    sqlx::migrate!().run(&pool).await?;
    Ok(pool)
}

pub async fn insert_success(
    pool: &SqlitePool,
    value: NewSuccess,
) -> Result<ExtractionSummary, sqlx::Error> {
    let summary = ExtractionSummary {
        id: Uuid::now_v7(),
        file_name: value.file_name,
        file_size_bytes: value.file_size_bytes,
        status: ExtractionStatus::Succeeded,
        activity_type: value.activity_type,
        activity_date: value.activity_date,
        error: None,
        created_at: created_at_now(),
    };

    sqlx::query(
        "INSERT INTO extractions (
            id, file_name, file_size_bytes, status, activity_type, activity_date,
            normalized_json, raw_json, error_code, error_message, created_at
        ) VALUES (?, ?, ?, 'succeeded', ?, ?, ?, ?, NULL, NULL, ?)",
    )
    .bind(summary.id.to_string())
    .bind(&summary.file_name)
    .bind(as_sqlite_size(summary.file_size_bytes)?)
    .bind(&summary.activity_type)
    .bind(&summary.activity_date)
    .bind(value.normalized_json)
    .bind(value.raw_json)
    .bind(&summary.created_at)
    .execute(pool)
    .await?;

    Ok(summary)
}

pub async fn insert_failure(
    pool: &SqlitePool,
    value: NewFailure,
) -> Result<ExtractionSummary, sqlx::Error> {
    let summary = ExtractionSummary {
        id: Uuid::now_v7(),
        file_name: value.file_name,
        file_size_bytes: value.file_size_bytes,
        status: ExtractionStatus::Failed,
        activity_type: None,
        activity_date: None,
        error: Some(ApiErrorDetail {
            code: value.error_code,
            message: value.error_message,
            file_name: None,
        }),
        created_at: created_at_now(),
    };
    let error = summary
        .error
        .as_ref()
        .expect("failed extraction always has an error");

    sqlx::query(
        "INSERT INTO extractions (
            id, file_name, file_size_bytes, status, activity_type, activity_date,
            normalized_json, raw_json, error_code, error_message, created_at
        ) VALUES (?, ?, ?, 'failed', NULL, NULL, NULL, NULL, ?, ?, ?)",
    )
    .bind(summary.id.to_string())
    .bind(&summary.file_name)
    .bind(as_sqlite_size(summary.file_size_bytes)?)
    .bind(&error.code)
    .bind(&error.message)
    .bind(&summary.created_at)
    .execute(pool)
    .await?;

    let mut summary = summary;
    summary
        .error
        .as_mut()
        .expect("failed extraction always has an error")
        .file_name = Some(summary.file_name.clone());
    Ok(summary)
}

pub async fn list(
    pool: &SqlitePool,
    limit: u32,
    offset: u32,
) -> Result<ExtractionPage, sqlx::Error> {
    let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM extractions")
        .fetch_one(pool)
        .await?;
    let rows = sqlx::query(
        "SELECT id, file_name, file_size_bytes, status, activity_type, activity_date,
                error_code, error_message, created_at
         FROM extractions
         ORDER BY created_at DESC, id DESC
         LIMIT ? OFFSET ?",
    )
    .bind(i64::from(limit))
    .bind(i64::from(offset))
    .fetch_all(pool)
    .await?;

    let items = rows
        .iter()
        .map(summary_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ExtractionPage {
        items,
        total: nonnegative_u64(total, "total")?,
        limit,
        offset,
    })
}

pub async fn get_stored(
    pool: &SqlitePool,
    id: Uuid,
) -> Result<Option<StoredExtraction>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, file_name, file_size_bytes, status, activity_type, activity_date,
                normalized_json, raw_json, error_code, error_message, created_at
         FROM extractions WHERE id = ?",
    )
    .bind(id.to_string())
    .fetch_optional(pool)
    .await?;

    row.map(stored_from_row).transpose()
}

pub async fn delete_one(pool: &SqlitePool, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM extractions WHERE id = ?")
        .bind(id.to_string())
        .execute(pool)
        .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn delete_all(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query("DELETE FROM extractions")
        .execute(pool)
        .await?
        .rows_affected())
}

fn summary_from_row(row: &SqliteRow) -> Result<ExtractionSummary, sqlx::Error> {
    let id = row.try_get::<String, _>("id")?;
    let status = row.try_get::<String, _>("status")?;
    let file_name: String = row.try_get("file_name")?;
    let error_code: Option<String> = row.try_get("error_code")?;
    let error_message: Option<String> = row.try_get("error_message")?;
    let error = match (error_code, error_message) {
        (None, None) => None,
        (Some(code), Some(message)) => Some(ApiErrorDetail {
            code,
            message,
            file_name: Some(file_name.clone()),
        }),
        _ => return Err(protocol_error("extraction error columns are inconsistent")),
    };

    Ok(ExtractionSummary {
        id: Uuid::parse_str(&id).map_err(|_| protocol_error("stored extraction id is invalid"))?,
        file_name,
        file_size_bytes: nonnegative_u64(row.try_get("file_size_bytes")?, "file_size_bytes")?,
        status: status_from_database(&status)?,
        activity_type: row.try_get("activity_type")?,
        activity_date: row.try_get("activity_date")?,
        error,
        created_at: row.try_get("created_at")?,
    })
}

fn stored_from_row(row: SqliteRow) -> Result<StoredExtraction, sqlx::Error> {
    let normalized_json = row.try_get("normalized_json")?;
    let raw_json = row.try_get("raw_json")?;
    let summary = summary_from_row(&row)?;
    Ok(StoredExtraction {
        summary,
        normalized_json,
        raw_json,
    })
}

fn created_at_now() -> String {
    OffsetDateTime::now_utc()
        .format(CREATED_AT_FORMAT)
        .expect("fixed timestamp format is valid")
}

fn as_sqlite_size(size: u64) -> Result<i64, sqlx::Error> {
    i64::try_from(size).map_err(|_| protocol_error("file size exceeds SQLite integer range"))
}

fn nonnegative_u64(value: i64, field: &str) -> Result<u64, sqlx::Error> {
    u64::try_from(value).map_err(|_| protocol_error(format!("{field} must be nonnegative")))
}

fn status_from_database(status: &str) -> Result<ExtractionStatus, sqlx::Error> {
    match status {
        "succeeded" => Ok(ExtractionStatus::Succeeded),
        "failed" => Ok(ExtractionStatus::Failed),
        _ => Err(protocol_error("stored extraction status is invalid")),
    }
}

fn protocol_error(message: impl Into<String>) -> sqlx::Error {
    sqlx::Error::Protocol(message.into())
}
