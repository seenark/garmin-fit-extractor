use std::collections::{BTreeMap, BTreeSet};

use sqlx::{PgPool, Postgres, Row, Transaction, postgres::PgPoolOptions};

use crate::{
    error::{MigratorError, Result},
    report::Report,
    source::{
        ActivityRow, ExtractionRow, GarminSnapshot, IntakeSnapshot, OAuthAccessTokenRow,
        OAuthAuthorizationCodeRow, OAuthLoginRequestRow, OAuthRefreshTokenRow, OAuthStateRow,
        SessionRow, TranscriptEntryRow, UserRow,
    },
};

pub const ADVISORY_LOCK_KEY: i64 = 0x4c4547414359;

const TARGET_TABLES: &[&str] = &[
    "users",
    "sessions",
    "oauth_states",
    "extractions",
    "activities",
    "oauth_login_requests",
    "oauth_authorization_codes",
    "oauth_access_tokens",
    "oauth_refresh_tokens",
    "transcript_entries",
    "legacy_imports",
];

#[derive(Clone, Debug)]
struct LedgerEntry {
    source_sha256: String,
    manifest: Vec<crate::checksum::TableDigest>,
}

pub fn ensure_postgres_url(database_url: &str) -> Result<()> {
    let trimmed = database_url.trim();
    let scheme = trimmed
        .split_once(':')
        .map(|(scheme, _)| scheme.to_ascii_lowercase())
        .unwrap_or_default();
    if trimmed.is_empty() || (scheme != "postgres" && scheme != "postgresql") {
        return Err(MigratorError::InvalidDatabaseUrl);
    }
    Ok(())
}

pub async fn connect(database_url: &str) -> Result<PgPool> {
    ensure_postgres_url(database_url)?;
    PgPoolOptions::new()
        .max_connections(4)
        .connect(database_url)
        .await
        .map_err(|cause| MigratorError::target_database("connect", cause))
}

pub async fn prepare_target(database_url: &str) -> Result<Report> {
    let pool = connect(database_url).await?;
    sqlx::migrate!("../../apps/api/migrations")
        .run(&pool)
        .await
        .map_err(|_| MigratorError::MigrationFailed)?;
    let report = Report::new("prepare-target", false).status("prepared");
    pool.close().await;
    Ok(report)
}

pub async fn verify_target_schema(pool: &PgPool) -> Result<()> {
    for table in TARGET_TABLES {
        let present: Option<String> = sqlx::query_scalar("SELECT to_regclass($1)::text")
            .bind(format!("public.{table}"))
            .fetch_one(pool)
            .await
            .map_err(|cause| MigratorError::target_database("verify target schema", cause))?;
        if present.is_none() {
            return Err(MigratorError::TargetSchema {
                table: (*table).to_owned(),
            });
        }
    }

    for (table, columns) in target_columns() {
        let rows = sqlx::query(
            "SELECT column_name, data_type, is_nullable
             FROM information_schema.columns
             WHERE table_schema = 'public' AND table_name = $1
             ORDER BY ordinal_position",
        )
        .bind(table)
        .fetch_all(pool)
        .await
        .map_err(|cause| MigratorError::target_database("verify target columns", cause))?;
        if rows.len() != columns.len() {
            return Err(MigratorError::TargetSchema {
                table: table.to_owned(),
            });
        }
        for (row, (name, data_type, nullable)) in rows.iter().zip(columns.iter()) {
            let actual_name: String =
                row.try_get("column_name")
                    .map_err(|_| MigratorError::TargetSchema {
                        table: table.to_owned(),
                    })?;
            let actual_type: String =
                row.try_get("data_type")
                    .map_err(|_| MigratorError::TargetSchema {
                        table: table.to_owned(),
                    })?;
            let actual_nullable: String =
                row.try_get("is_nullable")
                    .map_err(|_| MigratorError::TargetSchema {
                        table: table.to_owned(),
                    })?;
            if actual_name != *name || actual_type != *data_type || actual_nullable != *nullable {
                return Err(MigratorError::TargetSchema {
                    table: table.to_owned(),
                });
            }
        }
    }
    verify_target_constraints(pool).await?;
    Ok(())
}

async fn verify_target_constraints(pool: &PgPool) -> Result<()> {
    for (table, expected_constraints) in target_constraints() {
        let rows = sqlx::query(
            "SELECT pg_get_constraintdef(c.oid) AS definition
             FROM pg_constraint AS c
             JOIN pg_namespace AS n ON n.oid = c.connamespace
             WHERE n.nspname = 'public'
               AND c.conrelid = $1::regclass
               AND c.contype IN ('p', 'u', 'f', 'c')",
        )
        .bind(table)
        .fetch_all(pool)
        .await
        .map_err(|cause| MigratorError::target_database("verify target constraints", cause))?;
        let actual_constraints = rows
            .iter()
            .map(|row| {
                row.try_get::<String, _>("definition")
                    .map(|definition| normalize_constraint(&definition))
                    .map_err(|_| MigratorError::TargetSchema {
                        table: table.to_owned(),
                    })
            })
            .collect::<Result<BTreeSet<_>>>()?;
        let expected_constraints = expected_constraints
            .into_iter()
            .map(normalize_constraint)
            .collect::<BTreeSet<_>>();
        if actual_constraints != expected_constraints {
            return Err(MigratorError::TargetSchema {
                table: table.to_owned(),
            });
        }
    }
    Ok(())
}

fn normalize_constraint(definition: &str) -> String {
    definition
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn target_constraints() -> BTreeMap<&'static str, Vec<&'static str>> {
    BTreeMap::from([
        ("users", vec!["PRIMARY KEY (id)", "UNIQUE (google_subject)"]),
        (
            "sessions",
            vec![
                "PRIMARY KEY (token_hash)",
                "FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE",
            ],
        ),
        ("oauth_states", vec!["PRIMARY KEY (state_hash)"]),
        (
            "extractions",
            vec![
                "PRIMARY KEY (id)",
                "FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE",
                "CHECK ((file_size_bytes >= 0))",
                "CHECK ((status = ANY (ARRAY['succeeded'::text, 'failed'::text])))",
                "CHECK ((((status = 'succeeded'::text) AND (normalized_json IS NOT NULL) AND (raw_json IS NOT NULL) AND (error_code IS NULL) AND (error_message IS NULL)) OR ((status = 'failed'::text) AND (normalized_json IS NULL) AND (raw_json IS NULL) AND (error_code IS NOT NULL) AND (error_message IS NOT NULL))))",
            ],
        ),
        (
            "activities",
            vec![
                "PRIMARY KEY (id)",
                "FOREIGN KEY (id) REFERENCES extractions(id) ON DELETE CASCADE",
                "FOREIGN KEY (owner_id) REFERENCES users(id) ON DELETE CASCADE",
            ],
        ),
        ("oauth_login_requests", vec!["PRIMARY KEY (request_hash)"]),
        (
            "oauth_authorization_codes",
            vec![
                "PRIMARY KEY (code_hash)",
                "FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE",
            ],
        ),
        (
            "oauth_access_tokens",
            vec![
                "PRIMARY KEY (token_hash)",
                "FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE",
            ],
        ),
        (
            "oauth_refresh_tokens",
            vec![
                "PRIMARY KEY (token_hash)",
                "FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE",
            ],
        ),
        (
            "transcript_entries",
            vec!["PRIMARY KEY (id)", "UNIQUE (video_id)"],
        ),
        ("legacy_imports", vec!["PRIMARY KEY (source_name)"]),
    ])
}

fn target_columns() -> BTreeMap<&'static str, Vec<(&'static str, &'static str, &'static str)>> {
    BTreeMap::from([
        (
            "users",
            vec![
                ("id", "text", "NO"),
                ("google_subject", "text", "NO"),
                ("email", "text", "NO"),
                ("display_name", "text", "YES"),
                ("created_at", "text", "NO"),
                ("updated_at", "text", "NO"),
            ],
        ),
        (
            "sessions",
            vec![
                ("token_hash", "text", "NO"),
                ("user_id", "text", "NO"),
                ("created_at", "text", "NO"),
                ("expires_at", "text", "NO"),
            ],
        ),
        (
            "oauth_states",
            vec![
                ("state_hash", "text", "NO"),
                ("nonce", "text", "NO"),
                ("pkce_verifier", "text", "NO"),
                ("created_at", "text", "NO"),
                ("expires_at", "text", "NO"),
                ("continue_path", "text", "YES"),
            ],
        ),
        (
            "extractions",
            vec![
                ("id", "text", "NO"),
                ("file_name", "text", "NO"),
                ("file_size_bytes", "bigint", "NO"),
                ("status", "text", "NO"),
                ("activity_type", "text", "YES"),
                ("activity_date", "text", "YES"),
                ("normalized_json", "text", "YES"),
                ("raw_json", "text", "YES"),
                ("error_code", "text", "YES"),
                ("error_message", "text", "YES"),
                ("created_at", "text", "NO"),
                ("user_id", "text", "NO"),
            ],
        ),
        (
            "activities",
            vec![
                ("id", "text", "NO"),
                ("owner_id", "text", "NO"),
                ("sport", "text", "YES"),
                ("started_at", "text", "NO"),
                ("activity_data", "text", "NO"),
                ("created_at", "text", "NO"),
            ],
        ),
        (
            "oauth_login_requests",
            vec![
                ("request_hash", "text", "NO"),
                ("client_id", "text", "NO"),
                ("redirect_uri", "text", "NO"),
                ("state", "text", "NO"),
                ("scope", "text", "NO"),
                ("created_at", "text", "NO"),
                ("expires_at", "text", "NO"),
            ],
        ),
        (
            "oauth_authorization_codes",
            vec![
                ("code_hash", "text", "NO"),
                ("client_id", "text", "NO"),
                ("redirect_uri", "text", "NO"),
                ("user_id", "text", "NO"),
                ("scope", "text", "NO"),
                ("created_at", "text", "NO"),
                ("expires_at", "text", "NO"),
            ],
        ),
        (
            "oauth_access_tokens",
            vec![
                ("token_hash", "text", "NO"),
                ("client_id", "text", "NO"),
                ("user_id", "text", "NO"),
                ("scope", "text", "NO"),
                ("created_at", "text", "NO"),
                ("expires_at", "text", "NO"),
            ],
        ),
        (
            "oauth_refresh_tokens",
            vec![
                ("token_hash", "text", "NO"),
                ("client_id", "text", "NO"),
                ("user_id", "text", "NO"),
                ("scope", "text", "NO"),
                ("created_at", "text", "NO"),
                ("expires_at", "text", "NO"),
                ("revoked_at", "text", "YES"),
            ],
        ),
        (
            "transcript_entries",
            vec![
                ("id", "bigint", "NO"),
                ("channel_name", "text", "NO"),
                ("youtube_url", "text", "NO"),
                ("video_id", "text", "NO"),
                ("transcription", "text", "NO"),
                ("created_at", "bigint", "NO"),
                ("updated_at", "bigint", "NO"),
            ],
        ),
        (
            "legacy_imports",
            vec![
                ("source_name", "text", "NO"),
                ("source_sha256", "text", "NO"),
                ("imported_at", "timestamp with time zone", "NO"),
                ("table_manifest", "jsonb", "NO"),
            ],
        ),
    ])
}

pub async fn import(
    database_url: &str,
    garmin: &GarminSnapshot,
    intake: &IntakeSnapshot,
    apply: bool,
) -> Result<Report> {
    let pool = connect(database_url).await?;
    verify_target_schema(&pool).await?;
    let mut tx = pool
        .begin()
        .await
        .map_err(|cause| MigratorError::target_database("begin serializable transaction", cause))?;
    let result = import_transaction(&mut tx, garmin, intake, apply).await;
    match result {
        Ok(mut report) => {
            if apply {
                tx.commit()
                    .await
                    .map_err(|cause| MigratorError::target_database("commit import", cause))?;
                report.status = "committed".to_owned();
            } else {
                tx.rollback()
                    .await
                    .map_err(|cause| MigratorError::target_database("rollback dry run", cause))?;
                report.status = "dry-run-rolled-back".to_owned();
            }
            pool.close().await;
            Ok(report)
        }
        Err(error) => {
            let _ = tx.rollback().await;
            pool.close().await;
            Err(error)
        }
    }
}

async fn import_transaction(
    tx: &mut Transaction<'_, Postgres>,
    garmin: &GarminSnapshot,
    intake: &IntakeSnapshot,
    apply: bool,
) -> Result<Report> {
    sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
        .execute(&mut **tx)
        .await
        .map_err(|cause| MigratorError::target_database("set serializable isolation", cause))?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(ADVISORY_LOCK_KEY)
        .execute(&mut **tx)
        .await
        .map_err(|cause| MigratorError::target_database("acquire migration lock", cause))?;

    let source_manifests = [garmin.manifests(), intake.manifests()];
    let ledger_garmin = read_ledger(tx, "garmin").await?;
    let ledger_intake = read_ledger(tx, "intake").await?;
    let already_imported = ledger_garmin.is_some() && ledger_intake.is_some();
    if let Some(entry) = &ledger_garmin {
        ensure_ledger_matches(
            "garmin",
            entry,
            &source_manifests[0],
            garmin.source_sha256(),
        )?;
    }
    if let Some(entry) = &ledger_intake {
        ensure_ledger_matches(
            "intake",
            entry,
            &source_manifests[1],
            intake.source_sha256(),
        )?;
    }

    let (target_garmin, target_intake) = if already_imported {
        let target_garmin = read_garmin_target(tx).await?;
        let target_intake = read_intake_target(tx).await?;
        compare_snapshot_manifests(&target_garmin, &target_intake, garmin, intake)?;
        verify_foreign_keys(tx).await?;
        verify_sequences(tx, &target_intake).await?;
        (target_garmin, target_intake)
    } else {
        insert_users(tx, &garmin.users).await?;
        insert_sessions(tx, &garmin.sessions).await?;
        insert_oauth_states(tx, &garmin.oauth_states).await?;
        insert_extractions(tx, &garmin.extractions).await?;
        insert_activities(tx, &garmin.activities).await?;
        insert_oauth_login_requests(tx, &garmin.oauth_login_requests).await?;
        insert_oauth_authorization_codes(tx, &garmin.oauth_authorization_codes).await?;
        insert_oauth_access_tokens(tx, &garmin.oauth_access_tokens).await?;
        insert_oauth_refresh_tokens(tx, &garmin.oauth_refresh_tokens).await?;
        insert_transcript_entries(tx, &intake.transcript_entries).await?;

        let target_garmin = read_garmin_target(tx).await?;
        let target_intake = read_intake_target(tx).await?;
        compare_snapshot_manifests(&target_garmin, &target_intake, garmin, intake)?;
        repair_sequences(tx, &target_intake).await?;
        verify_foreign_keys(tx).await?;
        (target_garmin, target_intake)
    };

    let target_manifests = [target_garmin.manifests(), target_intake.manifests()];
    if !already_imported {
        if ledger_garmin.is_none() {
            insert_ledger(tx, "garmin", garmin.source_sha256(), &source_manifests[0]).await?;
        }
        if ledger_intake.is_none() {
            insert_ledger(tx, "intake", intake.source_sha256(), &source_manifests[1]).await?;
        }
    }

    let mut report = Report::new("import", !apply);
    report.add_source(&crate::source::Snapshot::Garmin(garmin.clone()));
    report.add_source(&crate::source::Snapshot::Intake(intake.clone()));
    let combined_target = target_manifests.into_iter().flatten().collect::<Vec<_>>();
    report.set_target(&combined_target);
    Ok(report)
}

pub async fn verify(
    database_url: &str,
    garmin: &GarminSnapshot,
    intake: &IntakeSnapshot,
) -> Result<Report> {
    let pool = connect(database_url).await?;
    verify_target_schema(&pool).await?;
    let mut tx = pool
        .begin()
        .await
        .map_err(|cause| MigratorError::target_database("begin verification", cause))?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await
        .map_err(|cause| MigratorError::target_database("set read-only verification", cause))?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(ADVISORY_LOCK_KEY)
        .execute(&mut *tx)
        .await
        .map_err(|cause| MigratorError::target_database("acquire verification lock", cause))?;
    let garmin_ledger =
        read_ledger(&mut tx, "garmin")
            .await?
            .ok_or_else(|| MigratorError::LedgerMismatch {
                source_name: "garmin".to_owned(),
            })?;
    let intake_ledger =
        read_ledger(&mut tx, "intake")
            .await?
            .ok_or_else(|| MigratorError::LedgerMismatch {
                source_name: "intake".to_owned(),
            })?;
    ensure_ledger_matches(
        "garmin",
        &garmin_ledger,
        &garmin.manifests(),
        garmin.source_sha256(),
    )?;
    ensure_ledger_matches(
        "intake",
        &intake_ledger,
        &intake.manifests(),
        intake.source_sha256(),
    )?;
    let target_garmin = read_garmin_target(&mut tx).await?;
    let target_intake = read_intake_target(&mut tx).await?;
    compare_snapshot_manifests(&target_garmin, &target_intake, garmin, intake)?;
    verify_foreign_keys(&mut tx).await?;
    verify_sequences(&mut tx, &target_intake).await?;

    let mut report = Report::new("verify", false).status("verified");
    report.add_source(&crate::source::Snapshot::Garmin(garmin.clone()));
    report.add_source(&crate::source::Snapshot::Intake(intake.clone()));
    let manifests = target_garmin
        .manifests()
        .into_iter()
        .chain(target_intake.manifests())
        .collect::<Vec<_>>();
    report.set_target(&manifests);
    tx.rollback()
        .await
        .map_err(|cause| MigratorError::target_database("finish verification", cause))?;
    pool.close().await;
    Ok(report)
}

async fn read_ledger(
    tx: &mut Transaction<'_, Postgres>,
    source: &str,
) -> Result<Option<LedgerEntry>> {
    let row = sqlx::query(
        "SELECT source_sha256, table_manifest::text AS table_manifest
         FROM legacy_imports WHERE source_name = $1",
    )
    .bind(source)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|cause| MigratorError::target_database("read import ledger", cause))?;
    let Some(row) = row else { return Ok(None) };
    let source_sha256: String =
        row.try_get("source_sha256")
            .map_err(|_| MigratorError::LedgerManifest {
                source_name: source.to_owned(),
            })?;
    let manifest_text: String =
        row.try_get("table_manifest")
            .map_err(|_| MigratorError::LedgerManifest {
                source_name: source.to_owned(),
            })?;
    let manifest =
        serde_json::from_str(&manifest_text).map_err(|_| MigratorError::LedgerManifest {
            source_name: source.to_owned(),
        })?;
    Ok(Some(LedgerEntry {
        source_sha256,
        manifest,
    }))
}

fn ensure_ledger_matches(
    source: &str,
    entry: &LedgerEntry,
    manifests: &[crate::checksum::TableDigest],
    source_sha256: String,
) -> Result<()> {
    if entry.source_sha256 != source_sha256 {
        return Err(MigratorError::LedgerMismatch {
            source_name: source.to_owned(),
        });
    }
    if !manifests_equal(&entry.manifest, manifests) {
        return Err(MigratorError::LedgerManifest {
            source_name: source.to_owned(),
        });
    }
    Ok(())
}

async fn insert_ledger(
    tx: &mut Transaction<'_, Postgres>,
    source: &str,
    source_sha256: String,
    manifests: &[crate::checksum::TableDigest],
) -> Result<()> {
    let manifest = serde_json::to_string(manifests).map_err(|_| MigratorError::LedgerManifest {
        source_name: source.to_owned(),
    })?;
    sqlx::query(
        "INSERT INTO legacy_imports (source_name, source_sha256, table_manifest)
         VALUES ($1, $2, $3::jsonb) ON CONFLICT (source_name) DO NOTHING",
    )
    .bind(source)
    .bind(source_sha256)
    .bind(manifest)
    .execute(&mut **tx)
    .await
    .map_err(|cause| MigratorError::target_database("write import ledger", cause))?;
    Ok(())
}

fn compare_snapshot_manifests(
    target_garmin: &GarminSnapshot,
    target_intake: &IntakeSnapshot,
    source_garmin: &GarminSnapshot,
    source_intake: &IntakeSnapshot,
) -> Result<()> {
    compare_manifests(&target_garmin.manifests(), &source_garmin.manifests())?;
    compare_manifests(&target_intake.manifests(), &source_intake.manifests())?;
    Ok(())
}

fn compare_manifests(
    actual: &[crate::checksum::TableDigest],
    expected: &[crate::checksum::TableDigest],
) -> Result<()> {
    let actual_by_table: BTreeMap<&str, &crate::checksum::TableDigest> = actual
        .iter()
        .map(|manifest| (manifest.table.as_str(), manifest))
        .collect();
    for expected_manifest in expected {
        let Some(actual_manifest) = actual_by_table.get(expected_manifest.table.as_str()) else {
            return Err(MigratorError::TargetMismatch {
                table: expected_manifest.table.clone(),
                reason: "table manifest differs",
            });
        };
        if actual_manifest.row_count != expected_manifest.row_count
            || actual_manifest.digest != expected_manifest.digest
        {
            return Err(MigratorError::TargetMismatch {
                table: expected_manifest.table.clone(),
                reason: "table manifest differs",
            });
        }
    }
    if actual.len() != expected.len() {
        return Err(MigratorError::TargetMismatch {
            table: "manifest".to_owned(),
            reason: "table manifest differs",
        });
    }
    Ok(())
}

fn manifests_equal(
    left: &[crate::checksum::TableDigest],
    right: &[crate::checksum::TableDigest],
) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut left = left
        .iter()
        .map(|manifest| (&manifest.table, manifest.row_count, &manifest.digest))
        .collect::<Vec<_>>();
    let mut right = right
        .iter()
        .map(|manifest| (&manifest.table, manifest.row_count, &manifest.digest))
        .collect::<Vec<_>>();
    left.sort_by(|a, b| a.0.cmp(b.0));
    right.sort_by(|a, b| a.0.cmp(b.0));
    left == right
}

fn equal_or_mismatch<T: PartialEq>(
    table: &'static str,
    expected: &T,
    actual: Option<&T>,
) -> Result<()> {
    match actual {
        Some(actual) if actual == expected => Ok(()),
        Some(_) => Err(MigratorError::TargetMismatch {
            table: table.to_owned(),
            reason: "row differs",
        }),
        None => Err(MigratorError::TargetMismatch {
            table: table.to_owned(),
            reason: "primary-key conflict or row missing",
        }),
    }
}

async fn insert_users(tx: &mut Transaction<'_, Postgres>, rows: &[UserRow]) -> Result<()> {
    for row in rows {
        sqlx::query(
            "INSERT INTO users (id, google_subject, email, display_name, created_at, updated_at)
             VALUES ($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING",
        )
        .bind(&row.id)
        .bind(&row.google_subject)
        .bind(&row.email)
        .bind(&row.display_name)
        .bind(&row.created_at)
        .bind(&row.updated_at)
        .execute(&mut **tx)
        .await
        .map_err(|cause| MigratorError::target_database("insert users", cause))?;
        equal_or_mismatch("users", row, find_user(tx, &row.id).await?.as_ref())?;
    }
    Ok(())
}

async fn find_user(tx: &mut Transaction<'_, Postgres>, id: &str) -> Result<Option<UserRow>> {
    let row = sqlx::query_as::<_, (String, String, String, Option<String>, String, String)>(
        "SELECT id, google_subject, email, display_name, created_at, updated_at FROM users WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|cause| MigratorError::target_database("compare users", cause))?;
    Ok(row.map(
        |(id, google_subject, email, display_name, created_at, updated_at)| UserRow {
            id,
            google_subject,
            email,
            display_name,
            created_at,
            updated_at,
        },
    ))
}

async fn insert_sessions(tx: &mut Transaction<'_, Postgres>, rows: &[SessionRow]) -> Result<()> {
    for row in rows {
        sqlx::query(
            "INSERT INTO sessions (token_hash, user_id, created_at, expires_at)
             VALUES ($1,$2,$3,$4) ON CONFLICT DO NOTHING",
        )
        .bind(&row.token_hash)
        .bind(&row.user_id)
        .bind(&row.created_at)
        .bind(&row.expires_at)
        .execute(&mut **tx)
        .await
        .map_err(|cause| MigratorError::target_database("insert sessions", cause))?;
        equal_or_mismatch(
            "sessions",
            row,
            find_session(tx, &row.token_hash).await?.as_ref(),
        )?;
    }
    Ok(())
}

async fn find_session(tx: &mut Transaction<'_, Postgres>, id: &str) -> Result<Option<SessionRow>> {
    let row = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT token_hash, user_id, created_at, expires_at FROM sessions WHERE token_hash = $1",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|cause| MigratorError::target_database("compare sessions", cause))?;
    Ok(
        row.map(|(token_hash, user_id, created_at, expires_at)| SessionRow {
            token_hash,
            user_id,
            created_at,
            expires_at,
        }),
    )
}

async fn insert_oauth_states(
    tx: &mut Transaction<'_, Postgres>,
    rows: &[OAuthStateRow],
) -> Result<()> {
    for row in rows {
        sqlx::query(
            "INSERT INTO oauth_states (state_hash, nonce, pkce_verifier, created_at, expires_at, continue_path)
             VALUES ($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING",
        )
        .bind(&row.state_hash)
        .bind(&row.nonce)
        .bind(&row.pkce_verifier)
        .bind(&row.created_at)
        .bind(&row.expires_at)
        .bind(&row.continue_path)
        .execute(&mut **tx)
        .await
        .map_err(|cause| MigratorError::target_database("insert oauth states", cause))?;
        equal_or_mismatch(
            "oauth_states",
            row,
            find_oauth_state(tx, &row.state_hash).await?.as_ref(),
        )?;
    }
    Ok(())
}

async fn find_oauth_state(
    tx: &mut Transaction<'_, Postgres>,
    id: &str,
) -> Result<Option<OAuthStateRow>> {
    let row = sqlx::query_as::<_, (String, String, String, String, String, Option<String>)>(
        "SELECT state_hash, nonce, pkce_verifier, created_at, expires_at, continue_path FROM oauth_states WHERE state_hash = $1",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|cause| MigratorError::target_database("compare oauth states", cause))?;
    Ok(row.map(
        |(state_hash, nonce, pkce_verifier, created_at, expires_at, continue_path)| OAuthStateRow {
            state_hash,
            nonce,
            pkce_verifier,
            created_at,
            expires_at,
            continue_path,
        },
    ))
}

async fn insert_extractions(
    tx: &mut Transaction<'_, Postgres>,
    rows: &[ExtractionRow],
) -> Result<()> {
    for row in rows {
        sqlx::query(
            "INSERT INTO extractions (id, user_id, file_name, file_size_bytes, status, activity_type, activity_date, normalized_json, raw_json, error_code, error_message, created_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) ON CONFLICT DO NOTHING",
        )
        .bind(&row.id)
        .bind(&row.user_id)
        .bind(&row.file_name)
        .bind(row.file_size_bytes)
        .bind(&row.status)
        .bind(&row.activity_type)
        .bind(&row.activity_date)
        .bind(&row.normalized_json)
        .bind(&row.raw_json)
        .bind(&row.error_code)
        .bind(&row.error_message)
        .bind(&row.created_at)
        .execute(&mut **tx)
        .await
        .map_err(|cause| MigratorError::target_database("insert extractions", cause))?;
        equal_or_mismatch(
            "extractions",
            row,
            find_extraction(tx, &row.id).await?.as_ref(),
        )?;
    }
    Ok(())
}

async fn find_extraction(
    tx: &mut Transaction<'_, Postgres>,
    id: &str,
) -> Result<Option<ExtractionRow>> {
    let row = sqlx::query_as::<_, (String, String, String, i64, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, String)>(
        "SELECT id, user_id, file_name, file_size_bytes, status, activity_type, activity_date, normalized_json, raw_json, error_code, error_message, created_at FROM extractions WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|cause| MigratorError::target_database("compare extractions", cause))?;
    Ok(row.map(
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
    ))
}

async fn insert_activities(tx: &mut Transaction<'_, Postgres>, rows: &[ActivityRow]) -> Result<()> {
    for row in rows {
        sqlx::query(
            "INSERT INTO activities (id, owner_id, sport, started_at, activity_data, created_at)
             VALUES ($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING",
        )
        .bind(&row.id)
        .bind(&row.owner_id)
        .bind(&row.sport)
        .bind(&row.started_at)
        .bind(&row.activity_data)
        .bind(&row.created_at)
        .execute(&mut **tx)
        .await
        .map_err(|cause| MigratorError::target_database("insert activities", cause))?;
        equal_or_mismatch(
            "activities",
            row,
            find_activity(tx, &row.id).await?.as_ref(),
        )?;
    }
    Ok(())
}

async fn find_activity(
    tx: &mut Transaction<'_, Postgres>,
    id: &str,
) -> Result<Option<ActivityRow>> {
    let row = sqlx::query_as::<_, (String, String, Option<String>, String, String, String)>(
        "SELECT id, owner_id, sport, started_at, activity_data, created_at FROM activities WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|cause| MigratorError::target_database("compare activities", cause))?;
    Ok(row.map(
        |(id, owner_id, sport, started_at, activity_data, created_at)| ActivityRow {
            id,
            owner_id,
            sport,
            started_at,
            activity_data,
            created_at,
        },
    ))
}

async fn insert_oauth_login_requests(
    tx: &mut Transaction<'_, Postgres>,
    rows: &[OAuthLoginRequestRow],
) -> Result<()> {
    for row in rows {
        sqlx::query(
            "INSERT INTO oauth_login_requests (request_hash, client_id, redirect_uri, state, scope, created_at, expires_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT DO NOTHING",
        )
        .bind(&row.request_hash)
        .bind(&row.client_id)
        .bind(&row.redirect_uri)
        .bind(&row.state)
        .bind(&row.scope)
        .bind(&row.created_at)
        .bind(&row.expires_at)
        .execute(&mut **tx)
        .await
        .map_err(|cause| MigratorError::target_database("insert oauth login requests", cause))?;
        equal_or_mismatch(
            "oauth_login_requests",
            row,
            find_oauth_login_request(tx, &row.request_hash)
                .await?
                .as_ref(),
        )?;
    }
    Ok(())
}

async fn find_oauth_login_request(
    tx: &mut Transaction<'_, Postgres>,
    id: &str,
) -> Result<Option<OAuthLoginRequestRow>> {
    let row = sqlx::query_as::<_, (String, String, String, String, String, String, String)>(
        "SELECT request_hash, client_id, redirect_uri, state, scope, created_at, expires_at FROM oauth_login_requests WHERE request_hash = $1",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|cause| MigratorError::target_database("compare oauth login requests", cause))?;
    Ok(row.map(
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
    ))
}

async fn insert_oauth_authorization_codes(
    tx: &mut Transaction<'_, Postgres>,
    rows: &[OAuthAuthorizationCodeRow],
) -> Result<()> {
    for row in rows {
        sqlx::query(
            "INSERT INTO oauth_authorization_codes (code_hash, client_id, redirect_uri, user_id, scope, created_at, expires_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT DO NOTHING",
        )
        .bind(&row.code_hash)
        .bind(&row.client_id)
        .bind(&row.redirect_uri)
        .bind(&row.user_id)
        .bind(&row.scope)
        .bind(&row.created_at)
        .bind(&row.expires_at)
        .execute(&mut **tx)
        .await
        .map_err(|cause| MigratorError::target_database("insert oauth authorization codes", cause))?;
        equal_or_mismatch(
            "oauth_authorization_codes",
            row,
            find_oauth_authorization_code(tx, &row.code_hash)
                .await?
                .as_ref(),
        )?;
    }
    Ok(())
}

async fn find_oauth_authorization_code(
    tx: &mut Transaction<'_, Postgres>,
    id: &str,
) -> Result<Option<OAuthAuthorizationCodeRow>> {
    let row = sqlx::query_as::<_, (String, String, String, String, String, String, String)>(
        "SELECT code_hash, client_id, redirect_uri, user_id, scope, created_at, expires_at FROM oauth_authorization_codes WHERE code_hash = $1",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|cause| MigratorError::target_database("compare oauth authorization codes", cause))?;
    Ok(row.map(
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
    ))
}

async fn insert_oauth_access_tokens(
    tx: &mut Transaction<'_, Postgres>,
    rows: &[OAuthAccessTokenRow],
) -> Result<()> {
    for row in rows {
        sqlx::query(
            "INSERT INTO oauth_access_tokens (token_hash, client_id, user_id, scope, created_at, expires_at)
             VALUES ($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING",
        )
        .bind(&row.token_hash)
        .bind(&row.client_id)
        .bind(&row.user_id)
        .bind(&row.scope)
        .bind(&row.created_at)
        .bind(&row.expires_at)
        .execute(&mut **tx)
        .await
        .map_err(|cause| MigratorError::target_database("insert oauth access tokens", cause))?;
        equal_or_mismatch(
            "oauth_access_tokens",
            row,
            find_oauth_access_token(tx, &row.token_hash).await?.as_ref(),
        )?;
    }
    Ok(())
}

async fn find_oauth_access_token(
    tx: &mut Transaction<'_, Postgres>,
    id: &str,
) -> Result<Option<OAuthAccessTokenRow>> {
    let row = sqlx::query_as::<_, (String, String, String, String, String, String)>(
        "SELECT token_hash, client_id, user_id, scope, created_at, expires_at FROM oauth_access_tokens WHERE token_hash = $1",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|cause| MigratorError::target_database("compare oauth access tokens", cause))?;
    Ok(row.map(
        |(token_hash, client_id, user_id, scope, created_at, expires_at)| OAuthAccessTokenRow {
            token_hash,
            client_id,
            user_id,
            scope,
            created_at,
            expires_at,
        },
    ))
}

async fn insert_oauth_refresh_tokens(
    tx: &mut Transaction<'_, Postgres>,
    rows: &[OAuthRefreshTokenRow],
) -> Result<()> {
    for row in rows {
        sqlx::query(
            "INSERT INTO oauth_refresh_tokens (token_hash, client_id, user_id, scope, created_at, expires_at, revoked_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT DO NOTHING",
        )
        .bind(&row.token_hash)
        .bind(&row.client_id)
        .bind(&row.user_id)
        .bind(&row.scope)
        .bind(&row.created_at)
        .bind(&row.expires_at)
        .bind(&row.revoked_at)
        .execute(&mut **tx)
        .await
        .map_err(|cause| MigratorError::target_database("insert oauth refresh tokens", cause))?;
        equal_or_mismatch(
            "oauth_refresh_tokens",
            row,
            find_oauth_refresh_token(tx, &row.token_hash)
                .await?
                .as_ref(),
        )?;
    }
    Ok(())
}

async fn find_oauth_refresh_token(
    tx: &mut Transaction<'_, Postgres>,
    id: &str,
) -> Result<Option<OAuthRefreshTokenRow>> {
    let row = sqlx::query_as::<_, (String, String, String, String, String, String, Option<String>)>(
        "SELECT token_hash, client_id, user_id, scope, created_at, expires_at, revoked_at FROM oauth_refresh_tokens WHERE token_hash = $1",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|cause| MigratorError::target_database("compare oauth refresh tokens", cause))?;
    Ok(row.map(
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
    ))
}

async fn insert_transcript_entries(
    tx: &mut Transaction<'_, Postgres>,
    rows: &[TranscriptEntryRow],
) -> Result<()> {
    for row in rows {
        sqlx::query(
            "INSERT INTO transcript_entries (id, channel_name, youtube_url, video_id, transcription, created_at, updated_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT DO NOTHING",
        )
        .bind(row.id)
        .bind(&row.channel_name)
        .bind(&row.youtube_url)
        .bind(&row.video_id)
        .bind(&row.transcription)
        .bind(row.created_at)
        .bind(row.updated_at)
        .execute(&mut **tx)
        .await
        .map_err(|cause| MigratorError::target_database("insert transcript entries", cause))?;
        equal_or_mismatch(
            "transcript_entries",
            row,
            find_transcript_entry(tx, row.id).await?.as_ref(),
        )?;
    }
    Ok(())
}

async fn find_transcript_entry(
    tx: &mut Transaction<'_, Postgres>,
    id: i64,
) -> Result<Option<TranscriptEntryRow>> {
    let row = sqlx::query_as::<_, (i64, String, String, String, String, i64, i64)>(
        "SELECT id, channel_name, youtube_url, video_id, transcription, created_at, updated_at FROM transcript_entries WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|cause| MigratorError::target_database("compare transcript entries", cause))?;
    Ok(row.map(
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
    ))
}

async fn read_garmin_target(tx: &mut Transaction<'_, Postgres>) -> Result<GarminSnapshot> {
    Ok(GarminSnapshot {
        users: read_target_users(tx).await?,
        sessions: read_target_sessions(tx).await?,
        oauth_states: read_target_oauth_states(tx).await?,
        extractions: read_target_extractions(tx).await?,
        activities: read_target_activities(tx).await?,
        oauth_login_requests: read_target_oauth_login_requests(tx).await?,
        oauth_authorization_codes: read_target_oauth_authorization_codes(tx).await?,
        oauth_access_tokens: read_target_oauth_access_tokens(tx).await?,
        oauth_refresh_tokens: read_target_oauth_refresh_tokens(tx).await?,
    })
}

async fn read_intake_target(tx: &mut Transaction<'_, Postgres>) -> Result<IntakeSnapshot> {
    Ok(IntakeSnapshot {
        transcript_entries: read_target_transcript_entries(tx).await?,
    })
}

async fn read_target_users(tx: &mut Transaction<'_, Postgres>) -> Result<Vec<UserRow>> {
    let rows = sqlx::query_as::<_, (String, String, String, Option<String>, String, String)>(
        "SELECT id, google_subject, email, display_name, created_at, updated_at FROM users ORDER BY id",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(|cause| MigratorError::target_database("read target users", cause))?;
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

async fn read_target_sessions(tx: &mut Transaction<'_, Postgres>) -> Result<Vec<SessionRow>> {
    let rows = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT token_hash, user_id, created_at, expires_at FROM sessions ORDER BY token_hash",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(|cause| MigratorError::target_database("read target sessions", cause))?;
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

async fn read_target_oauth_states(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<Vec<OAuthStateRow>> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, Option<String>)>(
        "SELECT state_hash, nonce, pkce_verifier, created_at, expires_at, continue_path FROM oauth_states ORDER BY state_hash",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(|cause| MigratorError::target_database("read target oauth states", cause))?;
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

async fn read_target_extractions(tx: &mut Transaction<'_, Postgres>) -> Result<Vec<ExtractionRow>> {
    let rows = sqlx::query_as::<_, (String, String, String, i64, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, String)>(
        "SELECT id, user_id, file_name, file_size_bytes, status, activity_type, activity_date, normalized_json, raw_json, error_code, error_message, created_at FROM extractions ORDER BY id",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(|cause| MigratorError::target_database("read target extractions", cause))?;
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

async fn read_target_activities(tx: &mut Transaction<'_, Postgres>) -> Result<Vec<ActivityRow>> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, String, String, String)>(
        "SELECT id, owner_id, sport, started_at, activity_data, created_at FROM activities ORDER BY id",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(|cause| MigratorError::target_database("read target activities", cause))?;
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

async fn read_target_oauth_login_requests(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<Vec<OAuthLoginRequestRow>> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String)>(
        "SELECT request_hash, client_id, redirect_uri, state, scope, created_at, expires_at FROM oauth_login_requests ORDER BY request_hash",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(|cause| MigratorError::target_database("read target oauth login requests", cause))?;
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

async fn read_target_oauth_authorization_codes(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<Vec<OAuthAuthorizationCodeRow>> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String)>(
        "SELECT code_hash, client_id, redirect_uri, user_id, scope, created_at, expires_at FROM oauth_authorization_codes ORDER BY code_hash",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(|cause| MigratorError::target_database("read target oauth authorization codes", cause))?;
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

async fn read_target_oauth_access_tokens(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<Vec<OAuthAccessTokenRow>> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String)>(
        "SELECT token_hash, client_id, user_id, scope, created_at, expires_at FROM oauth_access_tokens ORDER BY token_hash",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(|cause| MigratorError::target_database("read target oauth access tokens", cause))?;
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

async fn read_target_oauth_refresh_tokens(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<Vec<OAuthRefreshTokenRow>> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, Option<String>)>(
        "SELECT token_hash, client_id, user_id, scope, created_at, expires_at, revoked_at FROM oauth_refresh_tokens ORDER BY token_hash",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(|cause| MigratorError::target_database("read target oauth refresh tokens", cause))?;
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

async fn read_target_transcript_entries(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<Vec<TranscriptEntryRow>> {
    let rows = sqlx::query_as::<_, (i64, String, String, String, String, i64, i64)>(
        "SELECT id, channel_name, youtube_url, video_id, transcription, created_at, updated_at FROM transcript_entries ORDER BY id",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(|cause| MigratorError::target_database("read target transcript entries", cause))?;
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

fn sequence_restart_value(max_id: i64) -> Option<i64> {
    max_id.max(0).checked_add(1)
}

async fn repair_sequences(
    tx: &mut Transaction<'_, Postgres>,
    target_intake: &IntakeSnapshot,
) -> Result<()> {
    let Some(max_id) = target_intake
        .transcript_entries
        .iter()
        .map(|row| row.id)
        .max()
    else {
        return Ok(());
    };
    let restart = sequence_restart_value(max_id).ok_or(MigratorError::TargetMismatch {
        table: "transcript_entries".to_owned(),
        reason: "identity sequence cannot represent the next imported ID",
    })?;
    let sequence: Option<String> =
        sqlx::query_scalar("SELECT pg_get_serial_sequence('public.transcript_entries', 'id')")
            .fetch_one(&mut **tx)
            .await
            .map_err(|cause| MigratorError::target_database("find transcript sequence", cause))?;
    let sequence = sequence.ok_or_else(|| MigratorError::TargetSchema {
        table: "transcript_entries".to_owned(),
    })?;
    let statement = format!(
        "ALTER SEQUENCE {} RESTART WITH {restart}",
        quote_qualified_identifier(&sequence)
    );
    sqlx::query(sqlx::AssertSqlSafe(statement))
        .execute(&mut **tx)
        .await
        .map_err(|cause| MigratorError::target_database("repair transcript sequence", cause))?;
    Ok(())
}

async fn verify_sequences(
    tx: &mut Transaction<'_, Postgres>,
    target_intake: &IntakeSnapshot,
) -> Result<()> {
    let Some(max_id) = target_intake
        .transcript_entries
        .iter()
        .map(|row| row.id)
        .max()
    else {
        return Ok(());
    };
    let sequence: Option<String> =
        sqlx::query_scalar("SELECT pg_get_serial_sequence('public.transcript_entries', 'id')")
            .fetch_one(&mut **tx)
            .await
            .map_err(|cause| MigratorError::target_database("find transcript sequence", cause))?;
    let sequence = sequence.ok_or_else(|| MigratorError::TargetSchema {
        table: "transcript_entries".to_owned(),
    })?;
    let (last_value, is_called): (i64, bool) = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT last_value, is_called FROM {}",
        quote_qualified_identifier(&sequence)
    )))
    .fetch_one(&mut **tx)
    .await
    .map_err(|cause| MigratorError::target_database("verify transcript sequence", cause))?;
    if last_value < max_id || (last_value == max_id && !is_called) {
        return Err(MigratorError::TargetMismatch {
            table: "transcript_entries".to_owned(),
            reason: "identity sequence is behind imported IDs",
        });
    }
    Ok(())
}

fn quote_qualified_identifier(identifier: &str) -> String {
    identifier
        .split('.')
        .map(|part| format!("\"{}\"", part.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(".")
}

async fn verify_foreign_keys(tx: &mut Transaction<'_, Postgres>) -> Result<()> {
    let checks = [
        (
            "sessions",
            "SELECT EXISTS (SELECT 1 FROM sessions AS child LEFT JOIN users AS parent ON parent.id = child.user_id WHERE parent.id IS NULL)",
        ),
        (
            "extractions",
            "SELECT EXISTS (SELECT 1 FROM extractions AS child LEFT JOIN users AS parent ON parent.id = child.user_id WHERE parent.id IS NULL)",
        ),
        (
            "activities",
            "SELECT EXISTS (SELECT 1 FROM activities AS child LEFT JOIN extractions AS extraction ON extraction.id = child.id LEFT JOIN users AS parent ON parent.id = child.owner_id WHERE extraction.id IS NULL OR parent.id IS NULL)",
        ),
        (
            "oauth_authorization_codes",
            "SELECT EXISTS (SELECT 1 FROM oauth_authorization_codes AS child LEFT JOIN users AS parent ON parent.id = child.user_id WHERE parent.id IS NULL)",
        ),
        (
            "oauth_access_tokens",
            "SELECT EXISTS (SELECT 1 FROM oauth_access_tokens AS child LEFT JOIN users AS parent ON parent.id = child.user_id WHERE parent.id IS NULL)",
        ),
        (
            "oauth_refresh_tokens",
            "SELECT EXISTS (SELECT 1 FROM oauth_refresh_tokens AS child LEFT JOIN users AS parent ON parent.id = child.user_id WHERE parent.id IS NULL)",
        ),
    ];
    for (table, query) in checks {
        let invalid: bool = sqlx::query_scalar(query)
            .fetch_one(&mut **tx)
            .await
            .map_err(|cause| MigratorError::target_database("verify foreign keys", cause))?;
        if invalid {
            return Err(MigratorError::TargetMismatch {
                table: table.to_owned(),
                reason: "foreign-key consistency check failed",
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ensure_postgres_url, sequence_restart_value};

    #[test]
    fn target_rejects_sqlite_urls() {
        assert!(ensure_postgres_url("sqlite:///tmp/source.sqlite").is_err());
        assert!(ensure_postgres_url("postgresql://user:password@localhost/db").is_ok());
    }

    #[test]
    fn sequence_restart_is_one_past_imported_maximum() {
        assert_eq!(sequence_restart_value(56), Some(57));
        assert_eq!(sequence_restart_value(0), Some(1));
        assert_eq!(sequence_restart_value(i64::MAX), None);
    }
}
