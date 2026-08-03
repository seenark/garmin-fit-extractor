use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use garmin_fit_extractor_api::{
    app::{AppState, router},
    db,
};
use serde_json::{Value, json};
use tower::ServiceExt;

static NEXT_DATABASE: AtomicU64 = AtomicU64::new(0);

async fn test_app() -> axum::Router {
    test_app_with_static(PathBuf::from("apps/web/dist")).await
}

async fn test_app_with_static(static_dir: PathBuf) -> axum::Router {
    let nonce = NEXT_DATABASE.fetch_add(1, Ordering::Relaxed);
    let database_path = std::env::temp_dir().join(format!(
        "garmin-fit-extractor-http-{}-{nonce}.sqlite3",
        std::process::id()
    ));
    let database_url = format!("sqlite://{}", database_path.display());
    let db = db::connect(&database_url).await.expect("database connects");
    router(AppState { db }, static_dir)
}

fn multipart(parts: &[(&str, Option<&str>, &[u8])]) -> Request<Body> {
    const BOUNDARY: &str = "----garmin-fit-extractor-test";
    let mut body = Vec::new();
    for (name, file_name, value) in parts {
        body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
        body.extend_from_slice(
            match file_name {
                Some(file_name) => format!(
                    "Content-Disposition: form-data; name=\"{name}\"; filename=\"{file_name}\"\r\n\r\n"
                ),
                None => format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n"),
            }
            .as_bytes(),
        );
        body.extend_from_slice(value);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());

    Request::post("/api/v1/extractions")
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .body(Body::from(body))
        .expect("multipart request")
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body");
    serde_json::from_slice(&body).expect("JSON response")
}

#[tokio::test]
async fn rejects_empty_batches_with_standard_envelope() {
    let app = test_app().await;
    let response = app.oneshot(multipart(&[])).await.expect("response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(response).await,
        json!({"error": {"code": "EMPTY_BATCH", "message": "Upload at least one FIT file."}})
    );
}

#[tokio::test]
async fn rejects_unknown_field_without_persisting_earlier_parts() {
    let app = test_app().await;
    let response = app
        .oneshot(multipart(&[
            ("files", Some("earlier.fit"), b"not-a-fit"),
            ("wrong", Some("later.fit"), b"not-a-fit"),
        ]))
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(response).await,
        json!({"error": {"code": "UNKNOWN_FIELD", "message": "Only the files field is accepted."}})
    );
}

#[tokio::test]
async fn rejects_eleventh_part_before_any_decode_or_insert() {
    let app = test_app().await;
    let parts = (0..11)
        .map(|_| ("files", Some("activity.fit"), b"not-a-fit" as &[u8]))
        .collect::<Vec<_>>();
    let response = app.oneshot(multipart(&parts)).await.expect("response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(response).await,
        json!({"error": {"code": "TOO_MANY_FILES", "message": "Upload at most 10 FIT files."}})
    );
}

#[tokio::test]
async fn records_invalid_fit_and_filename_failures_in_request_order() {
    let app = test_app().await;
    let response = app
        .oneshot(multipart(&[
            ("files", Some("corrupt.fit"), b"not-a-fit"),
            ("files", Some("notes.txt"), b"also-not-a-fit"),
        ]))
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::CREATED);
    let response = response_json(response).await;
    let items = response["items"].as_array().expect("items");
    assert_eq!(items[0]["error"]["fileName"], "corrupt.fit");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["fileName"], "corrupt.fit");
    assert_eq!(items[0]["status"], "failed");
    assert_eq!(items[0]["error"]["code"], "INVALID_FIT");
    assert_eq!(items[1]["fileName"], "notes.txt");
    assert_eq!(items[1]["error"]["code"], "INVALID_FILE_NAME");
}

#[tokio::test]
async fn returns_json_404_for_unknown_api_route() {
    let app = test_app().await;
    let response = app
        .oneshot(
            Request::get("/api/v1")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        response_json(response).await,
        json!({"error": {"code": "NOT_FOUND", "message": "API route was not found."}})
    );
}

#[tokio::test]
async fn serves_index_for_non_api_client_routes() {
    let static_dir = std::env::temp_dir().join(format!(
        "garmin-fit-extractor-static-{}",
        NEXT_DATABASE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&static_dir).expect("static directory");
    std::fs::write(static_dir.join("index.html"), "<main>FIT app</main>").expect("index file");
    let app = test_app_with_static(static_dir).await;

    let response = app
        .oneshot(
            Request::get("/history")
                .body(Body::empty())
                .expect("SPA request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("SPA body"),
        "<main>FIT app</main>"
    );
}

#[tokio::test]
async fn healthz_pings_sqlite_and_returns_ok() {
    let app = test_app().await;
    let response = app
        .oneshot(
            Request::get("/healthz")
                .body(Body::empty())
                .expect("health request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response_json(response).await, json!({"status": "ok"}));
}

#[tokio::test]
async fn maps_missing_multipart_boundary_to_invalid_multipart() {
    let app = test_app().await;
    let response = app
        .oneshot(
            Request::post("/api/v1/extractions")
                .header(header::CONTENT_TYPE, "multipart/form-data")
                .body(Body::from("not multipart"))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(response).await,
        json!({"error": {"code": "INVALID_MULTIPART", "message": "The multipart request is invalid."}})
    );
}

#[tokio::test]
async fn persists_mixed_results_and_exposes_both_success_json_views() {
    let app = test_app().await;
    let fixture = std::fs::read("tests/fixtures/activity.fit").expect("FIT fixture");
    let response = app
        .clone()
        .oneshot(multipart(&[
            ("files", Some("first.FIT"), &fixture),
            ("files", Some("corrupt.fit"), b"corrupt"),
            ("files", Some("last.fit"), &fixture),
        ]))
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::CREATED);
    let created = response_json(response).await;
    let items = created["items"].as_array().expect("ordered response items");
    assert_eq!(
        items
            .iter()
            .map(|item| item["status"].as_str())
            .collect::<Vec<_>>(),
        vec![Some("succeeded"), Some("failed"), Some("succeeded")]
    );
    assert_eq!(items[1]["error"]["code"], "INVALID_FIT");

    let first_id = items[0]["id"].as_str().expect("first ID");
    let detail = app
        .oneshot(
            Request::get(format!("/api/v1/extractions/{first_id}"))
                .body(Body::empty())
                .expect("detail request"),
        )
        .await
        .expect("detail response");
    assert_eq!(detail.status(), StatusCode::OK);
    let detail = response_json(detail).await;
    assert_eq!(detail["normalized"]["schemaVersion"], "1.0.0");
    assert!(
        detail["raw"]
            .as_array()
            .is_some_and(|records| !records.is_empty())
    );
}

#[tokio::test]
async fn treats_exact_file_limit_as_a_decode_attempt_and_larger_file_as_file_too_large() {
    let app = test_app().await;
    let exact_limit = vec![0_u8; 20 * 1024 * 1024];
    let over_limit = vec![0_u8; 20 * 1024 * 1024 + 1];
    let response = app
        .oneshot(multipart(&[
            ("files", Some("at-limit.fit"), &exact_limit),
            ("files", Some("over-limit.fit"), &over_limit),
        ]))
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::CREATED);
    let items = response_json(response).await["items"]
        .as_array()
        .expect("items")
        .clone();
    assert_eq!(items[0]["fileSizeBytes"], 20 * 1024 * 1024);
    assert_eq!(items[0]["error"]["code"], "INVALID_FIT");
    assert_eq!(items[1]["fileSizeBytes"], 20 * 1024 * 1024 + 1);
    assert_eq!(items[1]["error"]["code"], "FILE_TOO_LARGE");
}

#[tokio::test]
async fn lists_downloads_and_deletes_persisted_successful_extractions() {
    let app = test_app().await;
    let fixture = std::fs::read("tests/fixtures/activity.fit").expect("FIT fixture");
    let created = app
        .clone()
        .oneshot(multipart(&[("files", Some("route test.fit"), &fixture)]))
        .await
        .expect("create response");
    assert_eq!(created.status(), StatusCode::CREATED);
    let id = response_json(created).await["items"][0]["id"]
        .as_str()
        .expect("successful ID")
        .to_owned();

    let listed = app
        .clone()
        .oneshot(
            Request::get("/api/v1/extractions?limit=1&offset=0")
                .body(Body::empty())
                .expect("list request"),
        )
        .await
        .expect("list response");
    assert_eq!(listed.status(), StatusCode::OK);
    let listed = response_json(listed).await;
    assert_eq!(listed["total"], 1);
    assert_eq!(listed["items"][0]["id"], id);

    let downloaded = app
        .clone()
        .oneshot(
            Request::get(format!("/api/v1/extractions/{id}/download?view=raw"))
                .body(Body::empty())
                .expect("download request"),
        )
        .await
        .expect("download response");
    assert_eq!(downloaded.status(), StatusCode::OK);
    assert_eq!(
        downloaded.headers()[header::CONTENT_DISPOSITION],
        "attachment; filename=\"route_test.raw.json\""
    );
    let raw = to_bytes(downloaded.into_body(), usize::MAX)
        .await
        .expect("download body");
    assert!(raw.ends_with(b"\n"));
    let _: Value = serde_json::from_slice(&raw).expect("downloaded JSON");

    let deleted = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/v1/extractions/{id}"))
                .body(Body::empty())
                .expect("delete request"),
        )
        .await
        .expect("delete response");
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);

    let missing = app
        .oneshot(
            Request::get(format!("/api/v1/extractions/{id}"))
                .body(Body::empty())
                .expect("detail request"),
        )
        .await
        .expect("detail response");
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}
