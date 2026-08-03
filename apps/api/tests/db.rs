use std::path::PathBuf;

use garmin_fit_extractor_api::{
    db::{self, NewFailure, NewSuccess},
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

fn success(file_name: &str) -> NewSuccess {
    NewSuccess {
        file_name: file_name.into(),
        file_size_bytes: 42,
        activity_type: Some("running".into()),
        activity_date: Some("2026-07-27T10:00:00.000Z".into()),
        normalized_json: r#"{"schemaVersion":"1.0.0"}"#.into(),
        raw_json: r#"[{"kind":"session","fields":[]}]"#.into(),
    }
}

fn failure(file_name: &str) -> NewFailure {
    NewFailure {
        file_name: file_name.into(),
        file_size_bytes: 9,
        error_code: "INVALID_FIT".into(),
        error_message: "File is not a valid FIT file or failed its integrity check.".into(),
    }
}

#[tokio::test]
async fn migrates_and_persists_success_and_failure_with_their_required_shapes() {
    let (directory, url) = temporary_database_url();
    let pool = db::connect(&url)
        .await
        .expect("database should connect and migrate");

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

    let stored = db::get_stored(&pool, succeeded.id)
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

    let failed_stored = db::get_stored(&pool, failed.id)
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
    let first = db::insert_success(&pool, success("first.fit"))
        .await
        .expect("first extraction should persist");
    let second = db::insert_success(&pool, success("second.fit"))
        .await
        .expect("second extraction should persist");

    sqlx::query("UPDATE extractions SET created_at = ? WHERE id = ?")
        .bind("2026-07-27T10:00:00.000Z")
        .bind(first.id.to_string())
        .execute(&pool)
        .await
        .expect("first timestamp should be controlled");
    sqlx::query("UPDATE extractions SET created_at = ? WHERE id = ?")
        .bind("2026-07-27T10:00:00.500Z")
        .bind(second.id.to_string())
        .execute(&pool)
        .await
        .expect("second timestamp should be controlled");

    let page = db::list(&pool, 1, 0).await.expect("page should load");
    assert_eq!(page.total, 2);
    assert_eq!(page.limit, 1);
    assert_eq!(page.offset, 0);
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, second.id);

    let next = db::list(&pool, 1, 1).await.expect("next page should load");
    assert_eq!(next.items.len(), 1);
    assert_eq!(next.items[0].id, first.id);

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
    let journal_mode = sqlx::query_scalar::<_, String>("PRAGMA journal_mode")
        .fetch_one(&pool)
        .await
        .expect("journal mode should be readable");
    assert_eq!(journal_mode, "wal");
    assert!(
        db::get_stored(&pool, uuid::Uuid::now_v7())
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
        db::delete_one(&pool, one.id)
            .await
            .expect("delete should succeed")
    );
    assert!(
        !db::delete_one(&pool, one.id)
            .await
            .expect("repeated delete should succeed")
    );
    assert!(
        db::get_stored(&pool, one.id)
            .await
            .expect("missing lookup should succeed")
            .is_none()
    );

    drop(pool);
    let reopened = db::connect(&url)
        .await
        .expect("database should reopen after WAL use");
    assert!(
        db::get_stored(&reopened, two.id)
            .await
            .expect("persisted lookup should succeed")
            .is_some()
    );
    assert_eq!(
        db::delete_all(&reopened)
            .await
            .expect("clear should succeed"),
        1
    );
    assert_eq!(
        db::delete_all(&reopened)
            .await
            .expect("empty clear should succeed"),
        0
    );
    assert_eq!(
        db::list(&reopened, 50, 0)
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

    let inconsistent = sqlx::query(
        "INSERT INTO extractions (
            id, file_name, file_size_bytes, status, normalized_json, raw_json,
            error_code, error_message, created_at
        ) VALUES (?, ?, ?, 'failed', ?, ?, NULL, NULL, ?)",
    )
    .bind(uuid::Uuid::now_v7().to_string())
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
