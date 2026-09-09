use sqlx::{
    Row, Sqlite, SqlitePool, Transaction,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::{
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use crate::{
    checksum::{CanonicalValue, TableDigest, manifest_digest, table_digest},
    error::{MigratorError, Result},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceKind {
    Garmin,
    Intake,
}

impl SourceKind {
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Garmin => "garmin",
            Self::Intake => "intake",
        }
    }

    pub const fn table_names(self) -> &'static [&'static str] {
        match self {
            Self::Garmin => &[
                "users",
                "sessions",
                "oauth_states",
                "extractions",
                "activities",
                "oauth_login_requests",
                "oauth_authorization_codes",
                "oauth_access_tokens",
                "oauth_refresh_tokens",
            ],
            Self::Intake => &["transcript_entries"],
        }
    }
}

impl FromStr for SourceKind {
    type Err = MigratorError;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "garmin" => Ok(Self::Garmin),
            "intake" => Ok(Self::Intake),
            _ => Err(MigratorError::InvalidOption(
                "source name must be garmin or intake".to_owned(),
            )),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserRow {
    pub id: String,
    pub google_subject: String,
    pub email: String,
    pub display_name: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionRow {
    pub token_hash: String,
    pub user_id: String,
    pub created_at: String,
    pub expires_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OAuthStateRow {
    pub state_hash: String,
    pub nonce: String,
    pub pkce_verifier: String,
    pub created_at: String,
    pub expires_at: String,
    pub continue_path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtractionRow {
    pub id: String,
    pub user_id: String,
    pub file_name: String,
    pub file_size_bytes: i64,
    pub status: String,
    pub activity_type: Option<String>,
    pub activity_date: Option<String>,
    pub normalized_json: Option<String>,
    pub raw_json: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivityRow {
    pub id: String,
    pub owner_id: String,
    pub sport: Option<String>,
    pub started_at: String,
    pub activity_data: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OAuthLoginRequestRow {
    pub request_hash: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub state: String,
    pub scope: String,
    pub created_at: String,
    pub expires_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OAuthAuthorizationCodeRow {
    pub code_hash: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub user_id: String,
    pub scope: String,
    pub created_at: String,
    pub expires_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OAuthAccessTokenRow {
    pub token_hash: String,
    pub client_id: String,
    pub user_id: String,
    pub scope: String,
    pub created_at: String,
    pub expires_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OAuthRefreshTokenRow {
    pub token_hash: String,
    pub client_id: String,
    pub user_id: String,
    pub scope: String,
    pub created_at: String,
    pub expires_at: String,
    pub revoked_at: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranscriptEntryRow {
    pub id: i64,
    pub channel_name: String,
    pub youtube_url: String,
    pub video_id: String,
    pub transcription: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GarminSnapshot {
    pub users: Vec<UserRow>,
    pub sessions: Vec<SessionRow>,
    pub oauth_states: Vec<OAuthStateRow>,
    pub extractions: Vec<ExtractionRow>,
    pub activities: Vec<ActivityRow>,
    pub oauth_login_requests: Vec<OAuthLoginRequestRow>,
    pub oauth_authorization_codes: Vec<OAuthAuthorizationCodeRow>,
    pub oauth_access_tokens: Vec<OAuthAccessTokenRow>,
    pub oauth_refresh_tokens: Vec<OAuthRefreshTokenRow>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct IntakeSnapshot {
    pub transcript_entries: Vec<TranscriptEntryRow>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Snapshot {
    Garmin(GarminSnapshot),
    Intake(IntakeSnapshot),
}

impl Snapshot {
    pub const fn kind(&self) -> SourceKind {
        match self {
            Self::Garmin(_) => SourceKind::Garmin,
            Self::Intake(_) => SourceKind::Intake,
        }
    }

    pub fn manifests(&self) -> Vec<TableDigest> {
        match self {
            Self::Garmin(snapshot) => snapshot.manifests(),
            Self::Intake(snapshot) => snapshot.manifests(),
        }
    }

    pub fn source_sha256(&self) -> String {
        manifest_digest(&self.manifests())
    }

    pub fn schema_tables(&self) -> Vec<String> {
        self.kind()
            .table_names()
            .iter()
            .map(|name| (*name).to_owned())
            .collect()
    }
}

impl GarminSnapshot {
    pub fn manifests(&self) -> Vec<TableDigest> {
        vec![
            table_digest("users", USERS_COLUMNS, &values_of(&self.users), &[0]),
            table_digest(
                "sessions",
                SESSIONS_COLUMNS,
                &values_of(&self.sessions),
                &[0],
            ),
            table_digest(
                "oauth_states",
                OAUTH_STATES_COLUMNS,
                &values_of(&self.oauth_states),
                &[0],
            ),
            table_digest(
                "extractions",
                EXTRACTIONS_COLUMNS,
                &values_of(&self.extractions),
                &[0],
            ),
            table_digest(
                "activities",
                ACTIVITIES_COLUMNS,
                &values_of(&self.activities),
                &[0],
            ),
            table_digest(
                "oauth_login_requests",
                OAUTH_LOGIN_REQUESTS_COLUMNS,
                &values_of(&self.oauth_login_requests),
                &[0],
            ),
            table_digest(
                "oauth_authorization_codes",
                OAUTH_AUTHORIZATION_CODES_COLUMNS,
                &values_of(&self.oauth_authorization_codes),
                &[0],
            ),
            table_digest(
                "oauth_access_tokens",
                OAUTH_ACCESS_TOKENS_COLUMNS,
                &values_of(&self.oauth_access_tokens),
                &[0],
            ),
            table_digest(
                "oauth_refresh_tokens",
                OAUTH_REFRESH_TOKENS_COLUMNS,
                &values_of(&self.oauth_refresh_tokens),
                &[0],
            ),
        ]
    }
    pub fn source_sha256(&self) -> String {
        manifest_digest(&self.manifests())
    }
}

impl IntakeSnapshot {
    pub fn manifests(&self) -> Vec<TableDigest> {
        vec![table_digest(
            "transcript_entries",
            TRANSCRIPT_ENTRIES_COLUMNS,
            &values_of(&self.transcript_entries),
            &[0],
        )]
    }
    pub fn source_sha256(&self) -> String {
        manifest_digest(&self.manifests())
    }
}

pub async fn read_snapshot(path: &Path, kind: SourceKind) -> Result<Snapshot> {
    let pool = open_read_only(path, kind.source_name()).await?;
    run_integrity_checks(&pool, kind.source_name()).await?;
    validate_schema(&pool, kind).await?;
    let snapshot = match kind {
        SourceKind::Garmin => Snapshot::Garmin(read_garmin(&pool).await?),
        SourceKind::Intake => Snapshot::Intake(read_intake(&pool).await?),
    };
    pool.close().await;
    Ok(snapshot)
}

/// Produce a standalone copy through SQLite's `VACUUM INTO` while the source connection is
/// explicitly read-only. Unlike a byte-for-byte file copy, this incorporates committed WAL
/// pages visible to the source connection.
pub async fn write_snapshot(input: &Path, output: &Path, kind: SourceKind) -> Result<Snapshot> {
    if input == output {
        return Err(MigratorError::InvalidOption(
            "snapshot input and output must differ".to_owned(),
        ));
    }
    if !input.is_file() {
        return Err(MigratorError::invalid_source(
            kind.source_name(),
            "input is not a file",
        ));
    }
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|cause| MigratorError::SnapshotIo { cause })?;
    }
    let source = open_read_only(input, kind.source_name()).await?;
    run_integrity_checks(&source, kind.source_name()).await?;
    validate_schema(&source, kind).await?;

    let temporary = temporary_snapshot_path(output);
    if temporary.exists() {
        std::fs::remove_file(&temporary).map_err(|cause| MigratorError::SnapshotIo { cause })?;
    }
    let temporary_sql = temporary.to_string_lossy().into_owned();
    sqlx::query("VACUUM INTO $1")
        .bind(temporary_sql)
        .execute(&source)
        .await
        .map_err(|cause| MigratorError::SnapshotDatabase { cause })?;
    source.close().await;

    let snapshot = read_snapshot(&temporary, kind).await?;
    std::fs::rename(&temporary, output).map_err(|cause| MigratorError::SnapshotIo { cause })?;
    Ok(snapshot)
}

fn temporary_snapshot_path(output: &Path) -> PathBuf {
    let file_name = output
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("snapshot.sqlite3");
    output.with_file_name(format!(".{file_name}.tmp-{}", std::process::id()))
}

async fn open_read_only(path: &Path, source: &str) -> Result<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .create_if_missing(false)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5));
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|cause| MigratorError::source_database(source, "open read-only database", cause))
}

async fn run_integrity_checks(pool: &SqlitePool, source: &str) -> Result<()> {
    let checks: Vec<String> = sqlx::query_scalar("PRAGMA quick_check")
        .fetch_all(pool)
        .await
        .map_err(|cause| MigratorError::source_database(source, "quick_check", cause))?;
    if checks.is_empty() || checks.iter().any(|check| check != "ok") {
        return Err(MigratorError::invalid_source(
            source,
            "SQLite quick_check reported corruption",
        ));
    }
    let foreign_keys = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(pool)
        .await
        .map_err(|cause| MigratorError::source_database(source, "foreign_key_check", cause))?;
    if !foreign_keys.is_empty() {
        return Err(MigratorError::invalid_source(
            source,
            "SQLite foreign_key_check reported violations",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct ColumnSpec {
    name: &'static str,
    sqlite_type: &'static str,
    nullable: bool,
    primary_key_position: i64,
}

const fn column(
    name: &'static str,
    sqlite_type: &'static str,
    nullable: bool,
    primary_key_position: i64,
) -> ColumnSpec {
    ColumnSpec {
        name,
        sqlite_type,
        nullable,
        primary_key_position,
    }
}

const USERS_COLUMNS: &[&str] = &[
    "id",
    "google_subject",
    "email",
    "display_name",
    "created_at",
    "updated_at",
];
const SESSIONS_COLUMNS: &[&str] = &["token_hash", "user_id", "created_at", "expires_at"];
const OAUTH_STATES_COLUMNS: &[&str] = &[
    "state_hash",
    "nonce",
    "pkce_verifier",
    "created_at",
    "expires_at",
    "continue_path",
];
const EXTRACTIONS_COLUMNS: &[&str] = &[
    "id",
    "user_id",
    "file_name",
    "file_size_bytes",
    "status",
    "activity_type",
    "activity_date",
    "normalized_json",
    "raw_json",
    "error_code",
    "error_message",
    "created_at",
];
const ACTIVITIES_COLUMNS: &[&str] = &[
    "id",
    "owner_id",
    "sport",
    "started_at",
    "activity_data",
    "created_at",
];
const OAUTH_LOGIN_REQUESTS_COLUMNS: &[&str] = &[
    "request_hash",
    "client_id",
    "redirect_uri",
    "state",
    "scope",
    "created_at",
    "expires_at",
];
const OAUTH_AUTHORIZATION_CODES_COLUMNS: &[&str] = &[
    "code_hash",
    "client_id",
    "redirect_uri",
    "user_id",
    "scope",
    "created_at",
    "expires_at",
];
const OAUTH_ACCESS_TOKENS_COLUMNS: &[&str] = &[
    "token_hash",
    "client_id",
    "user_id",
    "scope",
    "created_at",
    "expires_at",
];
const OAUTH_REFRESH_TOKENS_COLUMNS: &[&str] = &[
    "token_hash",
    "client_id",
    "user_id",
    "scope",
    "created_at",
    "expires_at",
    "revoked_at",
];
const TRANSCRIPT_ENTRIES_COLUMNS: &[&str] = &[
    "id",
    "channel_name",
    "youtube_url",
    "video_id",
    "transcription",
    "created_at",
    "updated_at",
];

const USERS_SCHEMA: &[ColumnSpec] = &[
    column("id", "TEXT", false, 1),
    column("google_subject", "TEXT", false, 0),
    column("email", "TEXT", false, 0),
    column("display_name", "TEXT", true, 0),
    column("created_at", "TEXT", false, 0),
    column("updated_at", "TEXT", false, 0),
];
const SESSIONS_SCHEMA: &[ColumnSpec] = &[
    column("token_hash", "TEXT", false, 1),
    column("user_id", "TEXT", false, 0),
    column("created_at", "TEXT", false, 0),
    column("expires_at", "TEXT", false, 0),
];
const OAUTH_STATES_SCHEMA: &[ColumnSpec] = &[
    column("state_hash", "TEXT", false, 1),
    column("nonce", "TEXT", false, 0),
    column("pkce_verifier", "TEXT", false, 0),
    column("created_at", "TEXT", false, 0),
    column("expires_at", "TEXT", false, 0),
    column("continue_path", "TEXT", true, 0),
];
const EXTRACTIONS_SCHEMA: &[ColumnSpec] = &[
    column("id", "TEXT", false, 1),
    column("user_id", "TEXT", false, 0),
    column("file_name", "TEXT", false, 0),
    column("file_size_bytes", "INTEGER", false, 0),
    column("status", "TEXT", false, 0),
    column("activity_type", "TEXT", true, 0),
    column("activity_date", "TEXT", true, 0),
    column("normalized_json", "TEXT", true, 0),
    column("raw_json", "TEXT", true, 0),
    column("error_code", "TEXT", true, 0),
    column("error_message", "TEXT", true, 0),
    column("created_at", "TEXT", false, 0),
];
const ACTIVITIES_SCHEMA: &[ColumnSpec] = &[
    column("id", "TEXT", false, 1),
    column("owner_id", "TEXT", false, 0),
    column("sport", "TEXT", true, 0),
    column("started_at", "TEXT", false, 0),
    column("activity_data", "TEXT", false, 0),
    column("created_at", "TEXT", false, 0),
];
const OAUTH_LOGIN_REQUESTS_SCHEMA: &[ColumnSpec] = &[
    column("request_hash", "TEXT", false, 1),
    column("client_id", "TEXT", false, 0),
    column("redirect_uri", "TEXT", false, 0),
    column("state", "TEXT", false, 0),
    column("scope", "TEXT", false, 0),
    column("created_at", "TEXT", false, 0),
    column("expires_at", "TEXT", false, 0),
];
const OAUTH_AUTHORIZATION_CODES_SCHEMA: &[ColumnSpec] = &[
    column("code_hash", "TEXT", false, 1),
    column("client_id", "TEXT", false, 0),
    column("redirect_uri", "TEXT", false, 0),
    column("user_id", "TEXT", false, 0),
    column("scope", "TEXT", false, 0),
    column("created_at", "TEXT", false, 0),
    column("expires_at", "TEXT", false, 0),
];
const OAUTH_ACCESS_TOKENS_SCHEMA: &[ColumnSpec] = &[
    column("token_hash", "TEXT", false, 1),
    column("client_id", "TEXT", false, 0),
    column("user_id", "TEXT", false, 0),
    column("scope", "TEXT", false, 0),
    column("created_at", "TEXT", false, 0),
    column("expires_at", "TEXT", false, 0),
];
const OAUTH_REFRESH_TOKENS_SCHEMA: &[ColumnSpec] = &[
    column("token_hash", "TEXT", false, 1),
    column("client_id", "TEXT", false, 0),
    column("user_id", "TEXT", false, 0),
    column("scope", "TEXT", false, 0),
    column("created_at", "TEXT", false, 0),
    column("expires_at", "TEXT", false, 0),
    column("revoked_at", "TEXT", true, 0),
];
const TRANSCRIPT_ENTRIES_SCHEMA: &[ColumnSpec] = &[
    column("id", "INTEGER", false, 1),
    column("channel_name", "TEXT", false, 0),
    column("youtube_url", "TEXT", false, 0),
    column("video_id", "TEXT", false, 0),
    column("transcription", "TEXT", false, 0),
    column("created_at", "INTEGER", false, 0),
    column("updated_at", "INTEGER", false, 0),
];

fn schema_for(kind: SourceKind) -> &'static [(&'static str, &'static [ColumnSpec])] {
    match kind {
        SourceKind::Garmin => &[
            ("users", USERS_SCHEMA),
            ("sessions", SESSIONS_SCHEMA),
            ("oauth_states", OAUTH_STATES_SCHEMA),
            ("extractions", EXTRACTIONS_SCHEMA),
            ("activities", ACTIVITIES_SCHEMA),
            ("oauth_login_requests", OAUTH_LOGIN_REQUESTS_SCHEMA),
            (
                "oauth_authorization_codes",
                OAUTH_AUTHORIZATION_CODES_SCHEMA,
            ),
            ("oauth_access_tokens", OAUTH_ACCESS_TOKENS_SCHEMA),
            ("oauth_refresh_tokens", OAUTH_REFRESH_TOKENS_SCHEMA),
        ],
        SourceKind::Intake => &[("transcript_entries", TRANSCRIPT_ENTRIES_SCHEMA)],
    }
}

async fn validate_schema(pool: &SqlitePool, kind: SourceKind) -> Result<()> {
    for (table, expected) in schema_for(kind) {
        let pragma = format!("PRAGMA table_info('{}')", table.replace('\'', "''"));
        let rows = sqlx::query(sqlx::AssertSqlSafe(pragma))
            .fetch_all(pool)
            .await
            .map_err(|cause| {
                MigratorError::source_database(kind.source_name(), "schema validation", cause)
            })?;
        if rows.len() != expected.len() {
            return Err(MigratorError::invalid_source(
                kind.source_name(),
                format!("required table {table} has an unexpected schema"),
            ));
        }
        for (row, column) in rows.iter().zip(*expected) {
            let name: String = row.try_get("name").map_err(|_| {
                MigratorError::invalid_source(kind.source_name(), "schema validation failed")
            })?;
            let declared: String = row.try_get("type").map_err(|_| {
                MigratorError::invalid_source(kind.source_name(), "schema validation failed")
            })?;
            let not_null: i64 = row.try_get("notnull").map_err(|_| {
                MigratorError::invalid_source(kind.source_name(), "schema validation failed")
            })?;
            let primary_key_position: i64 = row.try_get("pk").map_err(|_| {
                MigratorError::invalid_source(kind.source_name(), "schema validation failed")
            })?;
            if name != column.name
                || declared.trim().to_ascii_uppercase() != column.sqlite_type
                || primary_key_position != column.primary_key_position
                || (!column.nullable && column.primary_key_position == 0 && not_null == 0)
            {
                return Err(MigratorError::invalid_source(
                    kind.source_name(),
                    format!("required table {table} has an unexpected schema"),
                ));
            }
        }
    }
    Ok(())
}

type SqliteTx<'a> = Transaction<'a, Sqlite>;

async fn read_garmin(pool: &SqlitePool) -> Result<GarminSnapshot> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|cause| MigratorError::source_database("garmin", "consistent read", cause))?;
    let snapshot = GarminSnapshot {
        users: read_users(&mut tx).await?,
        sessions: read_sessions(&mut tx).await?,
        oauth_states: read_oauth_states(&mut tx).await?,
        extractions: read_extractions(&mut tx).await?,
        activities: read_activities(&mut tx).await?,
        oauth_login_requests: read_oauth_login_requests(&mut tx).await?,
        oauth_authorization_codes: read_oauth_authorization_codes(&mut tx).await?,
        oauth_access_tokens: read_oauth_access_tokens(&mut tx).await?,
        oauth_refresh_tokens: read_oauth_refresh_tokens(&mut tx).await?,
    };
    tx.commit()
        .await
        .map_err(|cause| MigratorError::source_database("garmin", "consistent read", cause))?;
    Ok(snapshot)
}

async fn read_intake(pool: &SqlitePool) -> Result<IntakeSnapshot> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|cause| MigratorError::source_database("intake", "consistent read", cause))?;
    let snapshot = IntakeSnapshot {
        transcript_entries: read_transcript_entries(&mut tx).await?,
    };
    tx.commit()
        .await
        .map_err(|cause| MigratorError::source_database("intake", "consistent read", cause))?;
    Ok(snapshot)
}

async fn read_users(tx: &mut SqliteTx<'_>) -> Result<Vec<UserRow>> {
    let rows = sqlx::query_as::<_, (String, String, String, Option<String>, String, String)>(
        "SELECT id, google_subject, email, display_name, created_at, updated_at FROM users ORDER BY id",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(|cause| MigratorError::source_database("garmin", "read users", cause))?;
    Ok(rows
        .into_iter()
        .map(
            |(id, google_subject, email, display_name, created_at, updated_at)| UserRow {
                id,
                google_subject,
                email,
                display_name,
                created_at,
                updated_at,
            },
        )
        .collect())
}

async fn read_sessions(tx: &mut SqliteTx<'_>) -> Result<Vec<SessionRow>> {
    let rows = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT token_hash, user_id, created_at, expires_at FROM sessions ORDER BY token_hash",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(|cause| MigratorError::source_database("garmin", "read sessions", cause))?;
    Ok(rows
        .into_iter()
        .map(|(token_hash, user_id, created_at, expires_at)| SessionRow {
            token_hash,
            user_id,
            created_at,
            expires_at,
        })
        .collect())
}

async fn read_oauth_states(tx: &mut SqliteTx<'_>) -> Result<Vec<OAuthStateRow>> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, Option<String>)>(
        "SELECT state_hash, nonce, pkce_verifier, created_at, expires_at, continue_path FROM oauth_states ORDER BY state_hash",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(|cause| MigratorError::source_database("garmin", "read oauth states", cause))?;
    Ok(rows
        .into_iter()
        .map(
            |(state_hash, nonce, pkce_verifier, created_at, expires_at, continue_path)| {
                OAuthStateRow {
                    state_hash,
                    nonce,
                    pkce_verifier,
                    created_at,
                    expires_at,
                    continue_path,
                }
            },
        )
        .collect())
}

async fn read_extractions(tx: &mut SqliteTx<'_>) -> Result<Vec<ExtractionRow>> {
    let rows = sqlx::query_as::<_, (
        String,
        String,
        String,
        i64,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
    )>(
        "SELECT id, user_id, file_name, file_size_bytes, status, activity_type, activity_date, normalized_json, raw_json, error_code, error_message, created_at FROM extractions ORDER BY id",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(|cause| MigratorError::source_database("garmin", "read extractions", cause))?;
    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                user_id,
                file_name,
                file_size_bytes,
                status,
                activity_type,
                activity_date,
                normalized_json,
                raw_json,
                error_code,
                error_message,
                created_at,
            )| ExtractionRow {
                id,
                user_id,
                file_name,
                file_size_bytes,
                status,
                activity_type,
                activity_date,
                normalized_json,
                raw_json,
                error_code,
                error_message,
                created_at,
            },
        )
        .collect())
}

async fn read_activities(tx: &mut SqliteTx<'_>) -> Result<Vec<ActivityRow>> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, String, String, String)>(
        "SELECT id, owner_id, sport, started_at, activity_data, created_at FROM activities ORDER BY id",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(|cause| MigratorError::source_database("garmin", "read activities", cause))?;
    Ok(rows
        .into_iter()
        .map(
            |(id, owner_id, sport, started_at, activity_data, created_at)| ActivityRow {
                id,
                owner_id,
                sport,
                started_at,
                activity_data,
                created_at,
            },
        )
        .collect())
}

async fn read_oauth_login_requests(tx: &mut SqliteTx<'_>) -> Result<Vec<OAuthLoginRequestRow>> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String)>(
        "SELECT request_hash, client_id, redirect_uri, state, scope, created_at, expires_at FROM oauth_login_requests ORDER BY request_hash",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(|cause| MigratorError::source_database("garmin", "read oauth login requests", cause))?;
    Ok(rows
        .into_iter()
        .map(
            |(request_hash, client_id, redirect_uri, state, scope, created_at, expires_at)| {
                OAuthLoginRequestRow {
                    request_hash,
                    client_id,
                    redirect_uri,
                    state,
                    scope,
                    created_at,
                    expires_at,
                }
            },
        )
        .collect())
}

async fn read_oauth_authorization_codes(
    tx: &mut SqliteTx<'_>,
) -> Result<Vec<OAuthAuthorizationCodeRow>> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String)>(
        "SELECT code_hash, client_id, redirect_uri, user_id, scope, created_at, expires_at FROM oauth_authorization_codes ORDER BY code_hash",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(|cause| MigratorError::source_database("garmin", "read oauth authorization codes", cause))?;
    Ok(rows
        .into_iter()
        .map(
            |(code_hash, client_id, redirect_uri, user_id, scope, created_at, expires_at)| {
                OAuthAuthorizationCodeRow {
                    code_hash,
                    client_id,
                    redirect_uri,
                    user_id,
                    scope,
                    created_at,
                    expires_at,
                }
            },
        )
        .collect())
}

async fn read_oauth_access_tokens(tx: &mut SqliteTx<'_>) -> Result<Vec<OAuthAccessTokenRow>> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String)>(
        "SELECT token_hash, client_id, user_id, scope, created_at, expires_at FROM oauth_access_tokens ORDER BY token_hash",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(|cause| MigratorError::source_database("garmin", "read oauth access tokens", cause))?;
    Ok(rows
        .into_iter()
        .map(
            |(token_hash, client_id, user_id, scope, created_at, expires_at)| OAuthAccessTokenRow {
                token_hash,
                client_id,
                user_id,
                scope,
                created_at,
                expires_at,
            },
        )
        .collect())
}

async fn read_oauth_refresh_tokens(tx: &mut SqliteTx<'_>) -> Result<Vec<OAuthRefreshTokenRow>> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, Option<String>)>(
        "SELECT token_hash, client_id, user_id, scope, created_at, expires_at, revoked_at FROM oauth_refresh_tokens ORDER BY token_hash",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(|cause| MigratorError::source_database("garmin", "read oauth refresh tokens", cause))?;
    Ok(rows
        .into_iter()
        .map(
            |(token_hash, client_id, user_id, scope, created_at, expires_at, revoked_at)| {
                OAuthRefreshTokenRow {
                    token_hash,
                    client_id,
                    user_id,
                    scope,
                    created_at,
                    expires_at,
                    revoked_at,
                }
            },
        )
        .collect())
}

async fn read_transcript_entries(tx: &mut SqliteTx<'_>) -> Result<Vec<TranscriptEntryRow>> {
    let rows = sqlx::query_as::<_, (i64, String, String, String, String, i64, i64)>(
        "SELECT id, channel_name, youtube_url, video_id, transcription, created_at, updated_at FROM transcript_entries ORDER BY id",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(|cause| MigratorError::source_database("intake", "read transcript entries", cause))?;
    Ok(rows
        .into_iter()
        .map(
            |(id, channel_name, youtube_url, video_id, transcription, created_at, updated_at)| {
                TranscriptEntryRow {
                    id,
                    channel_name,
                    youtube_url,
                    video_id,
                    transcription,
                    created_at,
                    updated_at,
                }
            },
        )
        .collect())
}

fn values_of<T>(rows: &[T]) -> Vec<Vec<CanonicalValue>>
where
    T: CanonicalValues,
{
    rows.iter().map(CanonicalValues::values).collect()
}

trait CanonicalValues {
    fn values(&self) -> Vec<CanonicalValue>;
}

fn text(value: &str) -> CanonicalValue {
    CanonicalValue::Text(value.to_owned())
}

fn optional_text(value: &Option<String>) -> CanonicalValue {
    match value {
        Some(value) => text(value),
        None => CanonicalValue::Null,
    }
}

impl CanonicalValues for UserRow {
    fn values(&self) -> Vec<CanonicalValue> {
        vec![
            text(&self.id),
            text(&self.google_subject),
            text(&self.email),
            optional_text(&self.display_name),
            text(&self.created_at),
            text(&self.updated_at),
        ]
    }
}
impl CanonicalValues for SessionRow {
    fn values(&self) -> Vec<CanonicalValue> {
        vec![
            text(&self.token_hash),
            text(&self.user_id),
            text(&self.created_at),
            text(&self.expires_at),
        ]
    }
}
impl CanonicalValues for OAuthStateRow {
    fn values(&self) -> Vec<CanonicalValue> {
        vec![
            text(&self.state_hash),
            text(&self.nonce),
            text(&self.pkce_verifier),
            text(&self.created_at),
            text(&self.expires_at),
            optional_text(&self.continue_path),
        ]
    }
}
impl CanonicalValues for ExtractionRow {
    fn values(&self) -> Vec<CanonicalValue> {
        vec![
            text(&self.id),
            text(&self.user_id),
            text(&self.file_name),
            CanonicalValue::Integer(self.file_size_bytes),
            text(&self.status),
            optional_text(&self.activity_type),
            optional_text(&self.activity_date),
            optional_text(&self.normalized_json),
            optional_text(&self.raw_json),
            optional_text(&self.error_code),
            optional_text(&self.error_message),
            text(&self.created_at),
        ]
    }
}
impl CanonicalValues for ActivityRow {
    fn values(&self) -> Vec<CanonicalValue> {
        vec![
            text(&self.id),
            text(&self.owner_id),
            optional_text(&self.sport),
            text(&self.started_at),
            text(&self.activity_data),
            text(&self.created_at),
        ]
    }
}
impl CanonicalValues for OAuthLoginRequestRow {
    fn values(&self) -> Vec<CanonicalValue> {
        vec![
            text(&self.request_hash),
            text(&self.client_id),
            text(&self.redirect_uri),
            text(&self.state),
            text(&self.scope),
            text(&self.created_at),
            text(&self.expires_at),
        ]
    }
}
impl CanonicalValues for OAuthAuthorizationCodeRow {
    fn values(&self) -> Vec<CanonicalValue> {
        vec![
            text(&self.code_hash),
            text(&self.client_id),
            text(&self.redirect_uri),
            text(&self.user_id),
            text(&self.scope),
            text(&self.created_at),
            text(&self.expires_at),
        ]
    }
}
impl CanonicalValues for OAuthAccessTokenRow {
    fn values(&self) -> Vec<CanonicalValue> {
        vec![
            text(&self.token_hash),
            text(&self.client_id),
            text(&self.user_id),
            text(&self.scope),
            text(&self.created_at),
            text(&self.expires_at),
        ]
    }
}
impl CanonicalValues for OAuthRefreshTokenRow {
    fn values(&self) -> Vec<CanonicalValue> {
        vec![
            text(&self.token_hash),
            text(&self.client_id),
            text(&self.user_id),
            text(&self.scope),
            text(&self.created_at),
            text(&self.expires_at),
            optional_text(&self.revoked_at),
        ]
    }
}
impl CanonicalValues for TranscriptEntryRow {
    fn values(&self) -> Vec<CanonicalValue> {
        vec![
            CanonicalValue::Integer(self.id),
            text(&self.channel_name),
            text(&self.youtube_url),
            text(&self.video_id),
            text(&self.transcription),
            CanonicalValue::Integer(self.created_at),
            CanonicalValue::Integer(self.updated_at),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::{SourceKind, temporary_snapshot_path};
    use std::path::Path;

    #[test]
    fn source_names_are_stable() {
        assert_eq!(SourceKind::Garmin.source_name(), "garmin");
        assert_eq!(SourceKind::Intake.source_name(), "intake");
        let expected = format!("/tmp/.a.sqlite3.tmp-{}", std::process::id());
        assert_eq!(
            temporary_snapshot_path(Path::new("/tmp/a.sqlite3")),
            Path::new(&expected)
        );
    }
}
