use crate::{
    auth::hash_token,
    model::{ApiErrorDetail, ExtractionPage, ExtractionStatus, ExtractionSummary},
};
use sqlx::{
    PgPool, Row,
    postgres::{PgConnectOptions, PgPoolOptions, PgRow},
};
use std::str::FromStr;
use time::{OffsetDateTime, format_description::FormatItem, macros::format_description};
use uuid::Uuid;
const CREATED_AT_FORMAT: &[FormatItem<'static>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z");
pub struct NewSuccess {
    pub user_id: Uuid,
    pub file_name: String,
    pub file_size_bytes: u64,
    pub activity_type: Option<String>,
    pub activity_date: Option<String>,
    pub normalized_json: String,
    pub raw_json: String,
}
pub struct NewFailure {
    pub user_id: Uuid,
    pub file_name: String,
    pub file_size_bytes: u64,
    pub error_code: String,
    pub error_message: String,
}
#[derive(Copy, Clone)]
pub enum HistoryOrder {
    Asc,
    Desc,
}
pub struct StoredExtraction {
    pub summary: ExtractionSummary,
    pub normalized_json: Option<String>,
    pub raw_json: Option<String>,
}
#[derive(Clone, Debug)]
pub struct StoredActivity {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub sport: Option<String>,
    pub started_at: String,
    pub activity_data: String,
    pub created_at: String,
}

#[derive(Clone, Debug)]
pub struct OAuthLoginRequest {
    pub client_id: String,
    pub redirect_uri: String,
    pub state: String,
    pub scope: String,
}

#[derive(Clone, Debug)]
pub struct OAuthAuthorizationCode {
    pub client_id: String,
    pub redirect_uri: String,
    pub user_id: Uuid,
    pub scope: String,
}

#[derive(Clone, Debug)]
pub struct OAuthAccessToken {
    pub client_id: String,
    pub user_id: Uuid,
    pub scope: String,
}

#[derive(Clone, Debug)]
pub struct OAuthRefreshToken {
    pub client_id: String,
    pub user_id: Uuid,
    pub scope: String,
}
pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let options = PgConnectOptions::from_str(database_url)?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;
    sqlx::migrate!().run(&pool).await?;
    backfill_activities(&pool).await?;
    Ok(pool)
}
pub async fn insert_success(
    pool: &PgPool,
    value: NewSuccess,
) -> Result<ExtractionSummary, sqlx::Error> {
    let NewSuccess {
        user_id,
        file_name,
        file_size_bytes,
        activity_type,
        activity_date,
        normalized_json,
        raw_json,
    } = value;
    let summary = ExtractionSummary {
        id: Uuid::now_v7(),
        file_name,
        file_size_bytes,
        status: ExtractionStatus::Succeeded,
        activity_type,
        activity_date,
        error: None,
        created_at: created_at_now(),
    };
    let mut transaction = pool.begin().await?;
    sqlx::query("INSERT INTO extractions (id,user_id,file_name,file_size_bytes,status,activity_type,activity_date,normalized_json,raw_json,error_code,error_message,created_at) VALUES ($1,$2,$3,$4, 'succeeded',$5,$6,$7,$8,NULL,NULL,$9)")
        .bind(summary.id.to_string())
        .bind(user_id.to_string())
        .bind(&summary.file_name)
        .bind(as_database_size(summary.file_size_bytes)?)
        .bind(&summary.activity_type)
        .bind(&summary.activity_date)
        .bind(&normalized_json)
        .bind(&raw_json)
        .bind(&summary.created_at)
        .execute(&mut *transaction)
        .await?;
    if let Some(started_at) = &summary.activity_date {
        sqlx::query(
            "INSERT INTO activities
                (id, owner_id, sport, started_at, activity_data, created_at)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(summary.id.to_string())
        .bind(user_id.to_string())
        .bind(&summary.activity_type)
        .bind(started_at)
        .bind(&normalized_json)
        .bind(&summary.created_at)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(summary)
}
pub async fn insert_failure(
    pool: &PgPool,
    value: NewFailure,
) -> Result<ExtractionSummary, sqlx::Error> {
    let mut summary = ExtractionSummary {
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
    let e = summary.error.as_ref().unwrap();
    sqlx::query("INSERT INTO extractions (id,user_id,file_name,file_size_bytes,status,activity_type,activity_date,normalized_json,raw_json,error_code,error_message,created_at) VALUES ($1,$2,$3,$4,'failed',NULL,NULL,NULL,NULL,$5,$6,$7)").bind(summary.id.to_string()).bind(value.user_id.to_string()).bind(&summary.file_name).bind(as_database_size(summary.file_size_bytes)?).bind(&e.code).bind(&e.message).bind(&summary.created_at).execute(pool).await?;
    summary.error.as_mut().unwrap().file_name = Some(summary.file_name.clone());
    Ok(summary)
}
pub async fn list(
    pool: &PgPool,
    user_id: Uuid,
    limit: u32,
    offset: u32,
    order: HistoryOrder,
) -> Result<ExtractionPage, sqlx::Error> {
    let user_id = user_id.to_string();
    let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM extractions WHERE user_id = $1")
        .bind(&user_id)
        .fetch_one(pool)
        .await?;

    let rows = match order {
        HistoryOrder::Asc => {
            sqlx::query(
                "SELECT id, file_name, file_size_bytes, status, activity_type, activity_date,
                        error_code, error_message, created_at
                 FROM extractions
                 WHERE user_id = $1
                 ORDER BY activity_date IS NULL ASC, activity_date ASC, created_at DESC, id DESC
                 LIMIT $2 OFFSET $3",
            )
            .bind(&user_id)
            .bind(i64::from(limit))
            .bind(i64::from(offset))
            .fetch_all(pool)
            .await?
        }
        HistoryOrder::Desc => {
            sqlx::query(
                "SELECT id, file_name, file_size_bytes, status, activity_type, activity_date,
                        error_code, error_message, created_at
                 FROM extractions
                 WHERE user_id = $1
                 ORDER BY activity_date IS NULL ASC, activity_date DESC, created_at DESC, id DESC
                 LIMIT $2 OFFSET $3",
            )
            .bind(&user_id)
            .bind(i64::from(limit))
            .bind(i64::from(offset))
            .fetch_all(pool)
            .await?
        }
    };

    Ok(ExtractionPage {
        items: rows
            .iter()
            .map(summary_from_row)
            .collect::<Result<Vec<_>, _>>()?,
        total: nonnegative_u64(total, "total")?,
        limit,
        offset,
    })
}
pub async fn get_stored(
    pool: &PgPool,
    user_id: Uuid,
    id: Uuid,
) -> Result<Option<StoredExtraction>, sqlx::Error> {
    sqlx::query("SELECT id,file_name,file_size_bytes,status,activity_type,activity_date,normalized_json,raw_json,error_code,error_message,created_at FROM extractions WHERE user_id=$1 AND id=$2").bind(user_id.to_string()).bind(id.to_string()).fetch_optional(pool).await?.map(stored_from_row).transpose()
}
pub async fn delete_one(pool: &PgPool, user_id: Uuid, id: Uuid) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("DELETE FROM extractions WHERE user_id=$1 AND id=$2")
            .bind(user_id.to_string())
            .bind(id.to_string())
            .execute(pool)
            .await?
            .rows_affected()
            == 1,
    )
}
pub async fn delete_all(pool: &PgPool, user_id: Uuid) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query("DELETE FROM extractions WHERE user_id=$1")
        .bind(user_id.to_string())
        .execute(pool)
        .await?
        .rows_affected())
}
fn summary_from_row(row: &PgRow) -> Result<ExtractionSummary, sqlx::Error> {
    let id: String = row.try_get("id")?;
    let status: String = row.try_get("status")?;
    let file_name: String = row.try_get("file_name")?;
    let ec: Option<String> = row.try_get("error_code")?;
    let em: Option<String> = row.try_get("error_message")?;
    let error = match (ec, em) {
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
fn stored_from_row(row: PgRow) -> Result<StoredExtraction, sqlx::Error> {
    Ok(StoredExtraction {
        summary: summary_from_row(&row)?,
        normalized_json: row.try_get("normalized_json")?,
        raw_json: row.try_get("raw_json")?,
    })
}
fn created_at_now() -> String {
    OffsetDateTime::now_utc()
        .format(CREATED_AT_FORMAT)
        .expect("fixed timestamp format is valid")
}
fn as_database_size(size: u64) -> Result<i64, sqlx::Error> {
    i64::try_from(size).map_err(|_| protocol_error("file size exceeds PostgreSQL integer range"))
}
fn nonnegative_u64(value: i64, name: &str) -> Result<u64, sqlx::Error> {
    u64::try_from(value).map_err(|_| protocol_error(&format!("{name} is negative")))
}
fn status_from_database(value: &str) -> Result<ExtractionStatus, sqlx::Error> {
    match value {
        "succeeded" => Ok(ExtractionStatus::Succeeded),
        "failed" => Ok(ExtractionStatus::Failed),
        _ => Err(protocol_error("stored extraction status is invalid")),
    }
}
fn protocol_error(message: &str) -> sqlx::Error {
    sqlx::Error::Protocol(message.to_owned())
}
pub async fn backfill_activities(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO activities
            (id, owner_id, sport, started_at, activity_data, created_at)
         SELECT id, user_id, activity_type, activity_date, normalized_json, created_at
         FROM extractions
         WHERE status = 'succeeded'
           AND activity_date IS NOT NULL
           AND normalized_json IS NOT NULL
         ON CONFLICT (id) DO NOTHING",
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn latest_activity(
    pool: &PgPool,
    owner_id: Uuid,
) -> Result<Option<StoredActivity>, sqlx::Error> {
    sqlx::query(
        "SELECT id, owner_id, sport, started_at, activity_data, created_at
         FROM activities
         WHERE owner_id = $1
         ORDER BY started_at DESC, id DESC
         LIMIT 1",
    )
    .bind(owner_id.to_string())
    .fetch_optional(pool)
    .await?
    .map(stored_activity_from_row)
    .transpose()
}

pub async fn list_activities(
    pool: &PgPool,
    owner_id: Uuid,
    limit: u32,
) -> Result<Vec<StoredActivity>, sqlx::Error> {
    sqlx::query(
        "SELECT id, owner_id, sport, started_at, activity_data, created_at
         FROM activities
         WHERE owner_id = $1
         ORDER BY started_at DESC, id DESC
         LIMIT $2",
    )
    .bind(owner_id.to_string())
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(stored_activity_from_row)
    .collect()
}

pub async fn get_activity(
    pool: &PgPool,
    owner_id: Uuid,
    activity_id: Uuid,
) -> Result<Option<StoredActivity>, sqlx::Error> {
    sqlx::query(
        "SELECT id, owner_id, sport, started_at, activity_data, created_at
         FROM activities
         WHERE id = $1 AND owner_id = $2",
    )
    .bind(activity_id.to_string())
    .bind(owner_id.to_string())
    .fetch_optional(pool)
    .await?
    .map(stored_activity_from_row)
    .transpose()
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_oauth_login_request(
    pool: &PgPool,
    request_token: &str,
    client_id: &str,
    redirect_uri: &str,
    state: &str,
    scope: &str,
    created_at: &str,
    expires_at: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO oauth_login_requests
            (request_hash, client_id, redirect_uri, state, scope, created_at, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(hash_token(request_token))
    .bind(client_id)
    .bind(redirect_uri)
    .bind(state)
    .bind(scope)
    .bind(created_at)
    .bind(expires_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn consume_oauth_login_request(
    pool: &PgPool,
    request_token: &str,
) -> Result<Option<OAuthLoginRequest>, sqlx::Error> {
    delete_expired_oauth_records(pool).await?;
    let now = timestamp_now();
    sqlx::query_as::<_, (String, String, String, String)>(
        "DELETE FROM oauth_login_requests
         WHERE request_hash = $1
           AND expires_at > $2
         RETURNING client_id, redirect_uri, state, scope",
    )
    .bind(hash_token(request_token))
    .bind(now)
    .fetch_optional(pool)
    .await
    .map(|row| {
        row.map(
            |(client_id, redirect_uri, state, scope)| OAuthLoginRequest {
                client_id,
                redirect_uri,
                state,
                scope,
            },
        )
    })
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_authorization_code(
    pool: &PgPool,
    code: &str,
    client_id: &str,
    redirect_uri: &str,
    user_id: Uuid,
    scope: &str,
    created_at: &str,
    expires_at: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO oauth_authorization_codes
            (code_hash, client_id, redirect_uri, user_id, scope, created_at, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(hash_token(code))
    .bind(client_id)
    .bind(redirect_uri)
    .bind(user_id.to_string())
    .bind(scope)
    .bind(created_at)
    .bind(expires_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn consume_authorization_code(
    pool: &PgPool,
    code: &str,
    client_id: &str,
    redirect_uri: &str,
    scope: &str,
) -> Result<Option<OAuthAuthorizationCode>, sqlx::Error> {
    delete_expired_oauth_records(pool).await?;
    let now = timestamp_now();
    sqlx::query_as::<_, (String, String, String, String)>(
        "DELETE FROM oauth_authorization_codes
         WHERE code_hash = $1
           AND client_id = $2
           AND redirect_uri = $3
           AND scope = $4
           AND expires_at > $5
         RETURNING client_id, redirect_uri, user_id, scope",
    )
    .bind(hash_token(code))
    .bind(client_id)
    .bind(redirect_uri)
    .bind(scope)
    .bind(now)
    .fetch_optional(pool)
    .await?
    .map(|(client_id, redirect_uri, user_id, scope)| {
        Ok(OAuthAuthorizationCode {
            client_id,
            redirect_uri,
            user_id: parse_uuid(&user_id, "authorization code user id")?,
            scope,
        })
    })
    .transpose()
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_token_pair(
    pool: &PgPool,
    access_token: &str,
    refresh_token: &str,
    client_id: &str,
    user_id: Uuid,
    scope: &str,
    created_at: &str,
    access_expires_at: &str,
    refresh_expires_at: &str,
) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;
    insert_token_pair_in_transaction(
        &mut transaction,
        access_token,
        refresh_token,
        client_id,
        user_id,
        scope,
        created_at,
        access_expires_at,
        refresh_expires_at,
    )
    .await?;
    transaction.commit().await?;
    Ok(())
}

pub async fn find_access_token(
    pool: &PgPool,
    token: &str,
) -> Result<Option<OAuthAccessToken>, sqlx::Error> {
    let now = timestamp_now();
    sqlx::query(
        "DELETE FROM oauth_access_tokens
         WHERE expires_at <= $1",
    )
    .bind(&now)
    .execute(pool)
    .await?;
    sqlx::query_as::<_, (String, String, String)>(
        "SELECT client_id, user_id, scope
         FROM oauth_access_tokens
         WHERE token_hash = $1
           AND expires_at > $2",
    )
    .bind(hash_token(token))
    .bind(&now)
    .fetch_optional(pool)
    .await?
    .map(|(client_id, user_id, scope)| {
        Ok(OAuthAccessToken {
            client_id,
            user_id: parse_uuid(&user_id, "access token user id")?,
            scope,
        })
    })
    .transpose()
}

#[allow(clippy::too_many_arguments)]
pub async fn rotate_refresh_token(
    pool: &PgPool,
    refresh_token: &str,
    client_id: &str,
    scope: &str,
    access_token: &str,
    replacement_refresh_token: &str,
    created_at: &str,
    access_expires_at: &str,
    refresh_expires_at: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let old_hash = hash_token(refresh_token);
    let now = timestamp_now();
    let row = sqlx::query_as::<_, (String,)>(
        "SELECT user_id
         FROM oauth_refresh_tokens
         WHERE token_hash = $1
           AND client_id = $2
           AND scope = $3
           AND revoked_at IS NULL
           AND expires_at > $4",
    )
    .bind(&old_hash)
    .bind(client_id)
    .bind(scope)
    .bind(now)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some((user_id,)) = row else {
        transaction.commit().await?;
        return Ok(None);
    };
    let updated = sqlx::query(
        "UPDATE oauth_refresh_tokens
         SET revoked_at = $1
         WHERE token_hash = $2 AND revoked_at IS NULL",
    )
    .bind(created_at)
    .bind(&old_hash)
    .execute(&mut *transaction)
    .await?;
    if updated.rows_affected() != 1 {
        transaction.commit().await?;
        return Ok(None);
    }
    let user_id = parse_uuid(&user_id, "refresh token user id")?;
    insert_token_pair_in_transaction(
        &mut transaction,
        access_token,
        replacement_refresh_token,
        client_id,
        user_id,
        scope,
        created_at,
        access_expires_at,
        refresh_expires_at,
    )
    .await?;
    transaction.commit().await?;
    Ok(Some(user_id))
}

pub async fn delete_expired_oauth_records(pool: &PgPool) -> Result<(), sqlx::Error> {
    let now = timestamp_now();
    sqlx::query(
        "DELETE FROM oauth_login_requests
         WHERE expires_at <= $1",
    )
    .bind(&now)
    .execute(pool)
    .await?;
    sqlx::query(
        "DELETE FROM oauth_authorization_codes
         WHERE expires_at <= $1",
    )
    .bind(&now)
    .execute(pool)
    .await?;
    sqlx::query(
        "DELETE FROM oauth_access_tokens
         WHERE expires_at <= $1",
    )
    .bind(&now)
    .execute(pool)
    .await?;
    sqlx::query(
        "DELETE FROM oauth_refresh_tokens
         WHERE expires_at <= $1",
    )
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

pub fn timestamp_now() -> String {
    created_at_now()
}

pub fn timestamp_after(seconds: i64) -> String {
    (OffsetDateTime::now_utc() + time::Duration::seconds(seconds))
        .format(CREATED_AT_FORMAT)
        .expect("fixed timestamp format is valid")
}

#[allow(clippy::too_many_arguments)]
async fn insert_token_pair_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    access_token: &str,
    refresh_token: &str,
    client_id: &str,
    user_id: Uuid,
    scope: &str,
    created_at: &str,
    access_expires_at: &str,
    refresh_expires_at: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO oauth_access_tokens
            (token_hash, client_id, user_id, scope, created_at, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(hash_token(access_token))
    .bind(client_id)
    .bind(user_id.to_string())
    .bind(scope)
    .bind(created_at)
    .bind(access_expires_at)
    .execute(&mut **transaction)
    .await?;
    sqlx::query(
        "INSERT INTO oauth_refresh_tokens
            (token_hash, client_id, user_id, scope, created_at, expires_at, revoked_at)
         VALUES ($1, $2, $3, $4, $5, $6, NULL)",
    )
    .bind(hash_token(refresh_token))
    .bind(client_id)
    .bind(user_id.to_string())
    .bind(scope)
    .bind(created_at)
    .bind(refresh_expires_at)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

fn stored_activity_from_row(row: PgRow) -> Result<StoredActivity, sqlx::Error> {
    let id: String = row.try_get("id")?;
    let owner_id: String = row.try_get("owner_id")?;
    Ok(StoredActivity {
        id: parse_uuid(&id, "activity id")?,
        owner_id: parse_uuid(&owner_id, "activity owner id")?,
        sport: row.try_get("sport")?,
        started_at: row.try_get("started_at")?,
        activity_data: row.try_get("activity_data")?,
        created_at: row.try_get("created_at")?,
    })
}

fn parse_uuid(value: &str, label: &str) -> Result<Uuid, sqlx::Error> {
    Uuid::parse_str(value).map_err(|_| protocol_error(&format!("stored {label} is invalid")))
}
