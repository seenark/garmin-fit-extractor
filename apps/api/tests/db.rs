use std::path::PathBuf;

use garmin_fit_extractor_api::{
    db::{self, HistoryOrder, NewFailure, NewSuccess},
    model::ExtractionStatus,
};
use sqlx::Row;

fn temporary_database_url() -> (PathBuf, String) {
    let directory = std::env::temp_dir().join(format!("garmin-fit-db-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(&directory).expect("temporary directory should be created");
    let path = directory.join("extractions.sqlite3");
    let url = format!("sqlite://{}", path.display());
    (directory, url)
}
fn test_user(slug: &str) -> uuid::Uuid {
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, slug.as_bytes())
}

fn success(file_name: &str) -> NewSuccess {
    success_for("default", file_name, Some("2026-07-27T10:00:00.000Z"))
}

fn success_for(user: &str, file_name: &str, activity_date: Option<&str>) -> NewSuccess {
    NewSuccess {
        user_id: test_user(user),
        file_name: file_name.into(),
        file_size_bytes: 42,
        activity_type: Some("running".into()),
        activity_date: activity_date.map(str::to_owned),
        normalized_json: r#"{"schemaVersion":"1.0.0"}"#.into(),
        raw_json: r#"[{"kind":"session","fields":[]}]"#.into(),
    }
}

fn failure(file_name: &str) -> NewFailure {
    NewFailure {
        user_id: test_user("default"),
        file_name: file_name.into(),
        file_size_bytes: 9,
        error_code: "INVALID_FIT".into(),
        error_message: "File is not a valid FIT file or failed its integrity check.".into(),
    }
}
async fn seed_user(pool: &sqlx::SqlitePool, slug: &str) {
    let user_id = test_user(slug);
    sqlx::query(
        "INSERT INTO users (id, google_subject, email, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(user_id.to_string())
    .bind(format!("test:{slug}"))
    .bind(format!("{slug}@example.test"))
    .bind("2026-01-01T00:00:00.000Z")
    .bind("2026-01-01T00:00:00.000Z")
    .execute(pool)
    .await
    .expect("test user should persist");
}

#[tokio::test]
async fn migrates_and_persists_success_and_failure_with_their_required_shapes() {
    let (directory, url) = temporary_database_url();
    let pool = db::connect(&url)
        .await
        .expect("database should connect and migrate");
    seed_user(&pool, "default").await;

    let succeeded = db::insert_success(&pool, success("morning.fit"))
        .await
        .expect("successful extraction should persist");
    let failed = db::insert_failure(&pool, failure("broken.fit"))
        .await
        .expect("failed extraction should persist");

    assert_eq!(succeeded.status, ExtractionStatus::Succeeded);
    assert!(succeeded.error.is_none());
    assert_eq!(failed.status, ExtractionStatus::Failed);
    assert_eq!(
        failed.error.as_ref().map(|error| error.code.as_str()),
        Some("INVALID_FIT")
    );
    assert_eq!(
        failed
            .error
            .as_ref()
            .and_then(|error| error.file_name.as_deref()),
        Some("broken.fit")
    );
    assert!(succeeded.created_at.ends_with('Z'));
    assert_eq!(succeeded.created_at.len(), "2026-07-27T10:00:00.000Z".len());
    assert_eq!(succeeded.created_at.as_bytes()[19], b'.');
    assert!(
        succeeded.created_at[20..23]
            .bytes()
            .all(|byte| byte.is_ascii_digit())
    );

    let stored = db::get_stored(&pool, test_user("default"), succeeded.id)
        .await
        .expect("stored row should be read")
        .expect("successful row should exist");
    assert_eq!(
        stored.normalized_json.as_deref(),
        Some(r#"{"schemaVersion":"1.0.0"}"#)
    );
    assert_eq!(
        stored.raw_json.as_deref(),
        Some(r#"[{"kind":"session","fields":[]}]"#)
    );

    let failed_stored = db::get_stored(&pool, test_user("default"), failed.id)
        .await
        .expect("failed row should be read")
        .expect("failed row should exist");
    assert!(failed_stored.normalized_json.is_none());
    assert!(failed_stored.raw_json.is_none());

    drop(pool);
    std::fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[tokio::test]
async fn lists_summaries_in_fixed_width_descending_timestamp_order_without_json_blobs() {
    let (directory, url) = temporary_database_url();
    let pool = db::connect(&url)
        .await
        .expect("database should connect and migrate");
    seed_user(&pool, "default").await;
    seed_user(&pool, "other").await;
    let first = db::insert_success(&pool, success("first.fit"))
        .await
        .expect("first extraction should persist");
    let second = db::insert_success(&pool, success("second.fit"))
        .await
        .expect("second extraction should persist");
    let undated = db::insert_failure(&pool, failure("failed.fit"))
        .await
        .expect("failed extraction should persist");
    let other = db::insert_success(
        &pool,
        success_for("other", "other.fit", Some("2026-07-29T10:00:00.000Z")),
    )
    .await
    .expect("other-user extraction should persist");

    sqlx::query("UPDATE extractions SET created_at = ?, activity_date = ? WHERE id = ?")
        .bind("2026-07-27T10:00:00.000Z")
        .bind("2026-07-28T10:00:00.000Z")
        .bind(first.id.to_string())
        .execute(&pool)
        .await
        .expect("first timestamp should be controlled");
    sqlx::query("UPDATE extractions SET created_at = ?, activity_date = ? WHERE id = ?")
        .bind("2026-07-27T10:00:00.500Z")
        .bind("2026-07-27T10:00:00.000Z")
        .bind(second.id.to_string())
        .execute(&pool)
        .await
        .expect("second timestamp should be controlled");

    let page = db::list(&pool, test_user("default"), 10, 0, HistoryOrder::Desc)
        .await
        .expect("page should load");
    assert_eq!(page.total, 3);
    assert_eq!(page.limit, 10);
    assert_eq!(page.offset, 0);
    assert_eq!(
        page.items.iter().map(|item| item.id).collect::<Vec<_>>(),
        vec![first.id, second.id, undated.id]
    );

    let ascending = db::list(&pool, test_user("default"), 10, 0, HistoryOrder::Asc)
        .await
        .expect("ascending page should load");
    assert_eq!(
        ascending
            .items
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        vec![second.id, first.id, undated.id]
    );

    let other_page = db::list(&pool, test_user("other"), 10, 0, HistoryOrder::Desc)
        .await
        .expect("other-user page should load");
    assert_eq!(other_page.total, 1);
    assert_eq!(other_page.items[0].id, other.id);

    let blobs_selected =
        sqlx::query("SELECT normalized_json, raw_json FROM extractions WHERE id = ?")
            .bind(first.id.to_string())
            .fetch_one(&pool)
            .await
            .expect("row should remain readable");
    assert!(
        blobs_selected
            .try_get::<String, _>("normalized_json")
            .is_ok()
    );
    assert!(blobs_selected.try_get::<String, _>("raw_json").is_ok());

    drop(pool);
    std::fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[tokio::test]
async fn deletes_individual_and_all_rows_and_reopens_with_wal_persistence() {
    let (directory, url) = temporary_database_url();
    let pool = db::connect(&url)
        .await
        .expect("database should connect and migrate");
    seed_user(&pool, "default").await;
    let journal_mode = sqlx::query_scalar::<_, String>("PRAGMA journal_mode")
        .fetch_one(&pool)
        .await
        .expect("journal mode should be readable");
    assert_eq!(journal_mode, "wal");
    assert!(
        db::get_stored(&pool, test_user("default"), uuid::Uuid::now_v7())
            .await
            .expect("unknown lookup should succeed")
            .is_none()
    );
    let one = db::insert_success(&pool, success("one.fit"))
        .await
        .expect("first extraction should persist");
    let two = db::insert_failure(&pool, failure("two.fit"))
        .await
        .expect("second extraction should persist");

    assert!(
        db::delete_one(&pool, test_user("default"), one.id)
            .await
            .expect("delete should succeed")
    );
    assert!(
        !db::delete_one(&pool, test_user("default"), one.id)
            .await
            .expect("repeated delete should succeed")
    );
    assert!(
        db::get_stored(&pool, test_user("default"), one.id)
            .await
            .expect("missing lookup should succeed")
            .is_none()
    );

    drop(pool);
    let reopened = db::connect(&url)
        .await
        .expect("database should reopen after WAL use");
    assert!(
        db::get_stored(&reopened, test_user("default"), two.id)
            .await
            .expect("persisted lookup should succeed")
            .is_some()
    );
    assert_eq!(
        db::delete_all(&reopened, test_user("default"))
            .await
            .expect("clear should succeed"),
        1
    );
    assert_eq!(
        db::delete_all(&reopened, test_user("default"))
            .await
            .expect("empty clear should succeed"),
        0
    );
    assert_eq!(
        db::list(&reopened, test_user("default"), 50, 0, HistoryOrder::Desc)
            .await
            .expect("empty page should load")
            .total,
        0
    );

    drop(reopened);
    std::fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[tokio::test]
async fn migration_rejects_rows_that_mix_success_and_failure_payloads() {
    let (directory, url) = temporary_database_url();
    let pool = db::connect(&url)
        .await
        .expect("database should connect and migrate");

    sqlx::query(
        "INSERT INTO users (id, google_subject, email, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(test_user("default").to_string())
    .bind("test:default")
    .bind("default@example.test")
    .bind("2026-07-27T10:00:00.000Z")
    .bind("2026-07-27T10:00:00.000Z")
    .execute(&pool)
    .await
    .expect("test user should persist");

    let inconsistent = sqlx::query(
        "INSERT INTO extractions (
            id, user_id, file_name, file_size_bytes, status, normalized_json, raw_json,
            error_code, error_message, created_at
        ) VALUES (?, ?, ?, ?, 'failed', ?, ?, NULL, NULL, ?)",
    )
    .bind(uuid::Uuid::now_v7().to_string())
    .bind(test_user("default").to_string())
    .bind("impossible.fit")
    .bind(1_i64)
    .bind("{}")
    .bind("[]")
    .bind("2026-07-27T10:00:00.000Z")
    .execute(&pool)
    .await;

    assert!(
        inconsistent.is_err(),
        "table check must reject inconsistent rows"
    );

    drop(pool);
    std::fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[tokio::test]
async fn fit_coach_tables_indexes_and_owner_scoped_activity_contract() {
    let (directory, url) = temporary_database_url();
    let pool = db::connect(&url).await.expect("database should connect");
    seed_user(&pool, "default").await;
    seed_user(&pool, "other").await;

    for table in [
        "activities",
        "oauth_login_requests",
        "oauth_authorization_codes",
        "oauth_access_tokens",
        "oauth_refresh_tokens",
    ] {
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?",
        )
        .bind(table)
        .fetch_one(&pool)
        .await
        .expect("table lookup should work");
        assert_eq!(exists, 1, "migration should create {table}");
    }
    for index in [
        "activities_owner_started_idx",
        "oauth_login_requests_expiry_idx",
        "oauth_authorization_codes_expiry_idx",
        "oauth_access_tokens_user_expiry_idx",
        "oauth_access_tokens_expiry_idx",
        "oauth_refresh_tokens_user_expiry_idx",
        "oauth_refresh_tokens_expiry_idx",
    ] {
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM sqlite_master WHERE type = 'index' AND name = ?",
        )
        .bind(index)
        .fetch_one(&pool)
        .await
        .expect("index lookup should work");
        assert_eq!(exists, 1, "migration should create {index}");
    }

    let oldest = db::insert_success(
        &pool,
        success_for("default", "old.fit", Some("2026-01-01T00:00:00.000Z")),
    )
    .await
    .expect("old activity should persist");
    let newest = db::insert_success(
        &pool,
        success_for("default", "new.fit", Some("2026-01-03T00:00:00.000Z")),
    )
    .await
    .expect("new activity should persist");
    let tie = db::insert_success(
        &pool,
        success_for("default", "tie.fit", Some("2026-01-03T00:00:00.000Z")),
    )
    .await
    .expect("tied activity should persist");
    assert_eq!(
        db::list_activities(&pool, test_user("default"), 20)
            .await
            .expect("activities should list")
            .iter()
            .map(|row| row.id)
            .collect::<Vec<_>>(),
        vec![tie.id, newest.id, oldest.id]
    );
    assert_eq!(
        db::latest_activity(&pool, test_user("default"))
            .await
            .expect("latest should load")
            .expect("latest should exist")
            .id,
        tie.id
    );
    assert!(
        db::get_activity(&pool, test_user("other"), newest.id)
            .await
            .expect("cross-owner lookup should work")
            .is_none()
    );

    assert!(
        db::delete_one(&pool, test_user("default"), newest.id)
            .await
            .expect("extraction deletion should succeed")
    );
    assert!(
        db::get_activity(&pool, test_user("default"), newest.id)
            .await
            .expect("cascade lookup should work")
            .is_none()
    );

    drop(pool);
    std::fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[tokio::test]
async fn fit_coach_backfill_is_idempotent_and_omits_undated_or_failed_rows() {
    let (directory, url) = temporary_database_url();
    let pool = db::connect(&url).await.expect("database should connect");
    seed_user(&pool, "default").await;
    let dated = db::insert_success(
        &pool,
        success_for("default", "dated.fit", Some("2026-02-01T00:00:00.000Z")),
    )
    .await
    .expect("dated success should persist");
    let undated = db::insert_success(&pool, success_for("default", "undated.fit", None))
        .await
        .expect("undated success should persist");
    let failed = db::insert_failure(&pool, failure("failed.fit"))
        .await
        .expect("failed extraction should persist");

    sqlx::query("DELETE FROM activities")
        .execute(&pool)
        .await
        .expect("activity projection should be removable");
    drop(pool);
    let reopened = db::connect(&url).await.expect("database should reopen");
    db::backfill_activities(&reopened)
        .await
        .expect("backfill should be callable repeatedly");
    let rows = db::list_activities(&reopened, test_user("default"), 20)
        .await
        .expect("backfilled activities should list");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, dated.id);
    assert_ne!(rows[0].id, undated.id);
    assert_ne!(rows[0].id, failed.id);
    db::backfill_activities(&reopened)
        .await
        .expect("second backfill should remain idempotent");
    assert_eq!(
        db::list_activities(&reopened, test_user("default"), 20)
            .await
            .expect("activities should still list")
            .len(),
        1
    );
    drop(reopened);
    std::fs::remove_dir_all(directory).expect("database directory should be removed");
}

#[tokio::test]
async fn fit_coach_oauth_values_are_hashed_and_refresh_rotation_revokes_old_token() {
    let (directory, url) = temporary_database_url();
    let pool = db::connect(&url).await.expect("database should connect");
    seed_user(&pool, "default").await;
    let user = test_user("default");
    let now = db::timestamp_now();
    let future = db::timestamp_after(3600);
    db::insert_oauth_login_request(
        &pool,
        "login-secret",
        "client",
        "https://callback",
        "state",
        "activities:read",
        &now,
        &future,
    )
    .await
    .expect("login request should persist");
    assert!(
        db::consume_oauth_login_request(&pool, "wrong")
            .await
            .expect("lookup should work")
            .is_none()
    );
    assert!(
        db::consume_oauth_login_request(&pool, "login-secret")
            .await
            .expect("consume should work")
            .is_some()
    );
    db::insert_token_pair(
        &pool,
        "access-one",
        "refresh-one",
        "client",
        user,
        "activities:read",
        &now,
        &future,
        &future,
    )
    .await
    .expect("token pair should persist");
    let replacement = db::rotate_refresh_token(
        &pool,
        "refresh-one",
        "client",
        "activities:read",
        "access-two",
        "refresh-two",
        &now,
        &future,
        &future,
    )
    .await
    .expect("rotation should work");
    assert_eq!(replacement, Some(user));
    assert!(
        db::rotate_refresh_token(
            &pool,
            "refresh-one",
            "client",
            "activities:read",
            "access-three",
            "refresh-three",
            &now,
            &future,
            &future
        )
        .await
        .expect("revoked token lookup should work")
        .is_none()
    );
    assert!(
        db::find_access_token(&pool, "access-two")
            .await
            .expect("access lookup should work")
            .is_some()
    );
    let raw: String = sqlx::query_scalar("SELECT token_hash FROM oauth_access_tokens")
        .fetch_one(&pool)
        .await
        .expect("hash should be stored");
    assert_ne!(raw, "access-two");
    drop(pool);
    std::fs::remove_dir_all(directory).expect("database directory should be removed");
}
