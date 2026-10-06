use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use garmin_fit_extractor_api::{
    app::{AppState, router},
    auth::{AuthState, hash_token},
    db,
    model::Analysis,
};
use serde_json::{Value, json};
use sqlx::PgPool;
use std::{path::PathBuf, sync::Arc};
use tower::ServiceExt;
use uuid::Uuid;

async fn app() -> (axum::Router, PgPool, Uuid, String) {
    let url = std::env::var("TEST_DATABASE_URL").expect("dedicated disposable TEST_DATABASE_URL");
    let expected =
        std::env::var("PGDATA").expect("disposable PGDATA for database identity verification");
    let verification = PgPool::connect(&url).await.unwrap();
    let directory: String = sqlx::query_scalar("SHOW data_directory")
        .fetch_one(&verification)
        .await
        .unwrap();
    assert_eq!(
        directory, expected,
        "verify disposable database before migrations"
    );
    let version: String = sqlx::query_scalar("SHOW server_version_num")
        .fetch_one(&verification)
        .await
        .unwrap();
    assert!((180000..190000).contains(&version.parse::<i32>().unwrap()));
    verification.close().await;
    let pool = db::connect(&url).await.unwrap();
    let owner = Uuid::new_v4();
    let token = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO users(id,google_subject,email,created_at,updated_at) VALUES($1,$2,$3,$4,$4)",
    )
    .bind(owner.to_string())
    .bind(format!("runs-http:{owner}"))
    .bind(format!("{owner}@example.test"))
    .bind(db::created_at_now())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO sessions(token_hash,user_id,created_at,expires_at) VALUES($1,$2,$3,'2099-01-01T00:00:00.000Z')")
        .bind(hash_token(&token)).bind(owner.to_string()).bind(db::created_at_now()).execute(&pool).await.unwrap();
    let app = router(
        AppState {
            db: pool.clone(),
            auth: Arc::new(AuthState::new(None, None)),
            app_origin: None,
        },
        PathBuf::from("nonexistent-test-static"),
    );
    (app, pool, owner, token)
}

async fn send(
    app: &axum::Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    payload: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(token) = token {
        request = request.header(header::COOKIE, format!("garmin_fit_session={token}"));
    }
    if payload.is_some() {
        request = request.header(header::CONTENT_TYPE, "application/json");
    }
    let response = app
        .clone()
        .oneshot(
            request
                .body(Body::from(
                    payload.map(|p| p.to_string()).unwrap_or_default(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 10 * 1024 * 1024)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn source_unavailable_legacy_stays_visible_but_cannot_reprocess_or_export() {
    let (app, pool, owner, token) = app().await;
    let mut analysis = Analysis::empty("private-upload-name.fit");
    analysis.activity.date = Some("2026-01-01T10:00:00Z".into());
    analysis.summary.distance.value = Some(5000.0);
    analysis.summary.duration.value = Some(1800.0);
    let legacy = db::insert_success(
        &pool,
        db::NewSuccess {
            user_id: owner,
            file_name: "private-upload-name.fit".into(),
            file_size_bytes: 0,
            activity_type: Some("running".into()),
            activity_date: analysis.activity.date.clone(),
            normalized_json: serde_json::to_string(&analysis).unwrap(),
            raw_json: "[]".into(),
        },
    )
    .await
    .unwrap();
    let path = format!("/api/v2/runs/{}", legacy.id);
    let (status, detail) = send(&app, "GET", &path, Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["id"], legacy.id.to_string());
    assert_eq!(detail["sourceUnavailable"], true);
    assert_eq!(detail["summary"]["distanceMeters"], 5000.0);
    assert!(detail["normalized"].is_null());
    assert!(detail["revisionId"].is_null());
    assert!(!detail.to_string().contains("private-upload-name"));
    let (status, error) = send(
        &app,
        "POST",
        &format!("{path}/reprocess"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error["error"]["code"], "SOURCE_UNAVAILABLE");
    let (status, error) = send(
        &app,
        "POST",
        "/api/v2/runs/exports",
        Some(&token),
        Some(json!({"activityIds":[legacy.id],"mode":"full"})),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error["error"]["code"], "LEGACY_EXPORT_UNSUPPORTED");
    let (status, _) = send(&app, "GET", &path, None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = send(&app, "DELETE", &path, Some(&token), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = send(&app, "GET", &path, Some(&token), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    sqlx::query("DELETE FROM users WHERE id=$1")
        .bind(owner.to_string())
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn deleting_pinned_detail_during_read_fails_body_instead_of_completing_json() {
    use futures_util::StreamExt;
    use garmin_fit_extractor_api::runs::{jobs, store};
    let (app, pool, owner, token) = app().await;
    let bytes = include_bytes!("fixtures/runs/garmin_run.fit");
    let admission = jobs::import_admission(&pool).await.unwrap();
    let mut document = jobs::decode_import(
        &pool,
        bytes,
        std::future::pending(),
        Some(admission.decoder_hold().unwrap()),
    )
    .await
    .unwrap();
    document.protect_cpu(&admission);
    let accepted = store::accept_spool(&pool, owner, bytes, document, admission.holder())
        .await
        .unwrap();
    drop(admission);
    for _ in 0..64 {
        let published:bool=sqlx::query_scalar("SELECT current_manifest_id IS NOT NULL FROM runs_activities WHERE owner_id=$1 AND id=$2")
            .bind(owner.to_string()).bind(accepted.activity_id.to_string()).fetch_one(&pool).await.unwrap();
        if published {
            break;
        }
        assert!(
            jobs::run_once(&pool).await.unwrap(),
            "real processing must publish before serving a pinned detail"
        );
    }
    let path = format!("/api/v2/runs/{}", accepted.activity_id);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&path)
                .header(header::COOKIE, format!("garmin_fit_session={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get(header::CONTENT_LENGTH).is_some());
    let mut stream = response.into_body().into_data_stream();
    let prefix = stream.next().await.unwrap().unwrap();
    assert!(
        serde_json::from_slice::<Value>(&prefix).is_err(),
        "delete barrier occurs before a complete JSON response"
    );
    let (status, _) = send(&app, "DELETE", &path, Some(&token), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(
        stream.next().await.unwrap().is_err(),
        "deleted chunks must cause transport failure, not clean EOF"
    );
    assert!(stream.next().await.is_none());
    sqlx::query("DELETE FROM users WHERE id=$1")
        .bind(owner.to_string())
        .execute(&pool)
        .await
        .unwrap();
}
