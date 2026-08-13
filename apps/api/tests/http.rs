use std::{
    io::Write,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use garmin_fit_extractor_api::{
    app::{AppState, router},
    auth::{AuthState, hash_token},
    config::{COACH_CLIENT_ID, CoachOAuthConfig},
    db,
};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

const TEST_TOKEN: &str = "http-test-session-token";
const TEST_USER: Uuid = Uuid::from_u128(1);

static NEXT_DATABASE: AtomicU64 = AtomicU64::new(0);

async fn test_app() -> axum::Router {
    test_app_with_static(PathBuf::from("apps/web/dist")).await
}

async fn test_app_with_static(static_dir: PathBuf) -> axum::Router {
    let (app, _) = test_app_with_static_and_db(static_dir).await;
    app
}

async fn test_app_with_static_and_db(static_dir: PathBuf) -> (axum::Router, sqlx::SqlitePool) {
    let nonce = NEXT_DATABASE.fetch_add(1, Ordering::Relaxed);
    let database_path = std::env::temp_dir().join(format!(
        "garmin-fit-extractor-http-{}-{nonce}.sqlite3",
        std::process::id()
    ));
    let database_url = format!("sqlite://{}", database_path.display());
    let db = db::connect(&database_url).await.expect("database connects");
    sqlx::query(
        "INSERT INTO users (id, google_subject, email, display_name, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(TEST_USER.to_string())
    .bind("test:http")
    .bind("http@example.test")
    .bind("HTTP Test")
    .bind("2026-01-01T00:00:00.000Z")
    .bind("2026-01-01T00:00:00.000Z")
    .execute(&db)
    .await
    .expect("test user should persist");
    sqlx::query(
        "INSERT INTO sessions (token_hash, user_id, created_at, expires_at)
         VALUES (?, ?, ?, ?)",
    )
    .bind(hash_token(TEST_TOKEN))
    .bind(TEST_USER.to_string())
    .bind("2026-01-01T00:00:00.000Z")
    .bind("2099-01-01T00:00:00.000Z")
    .execute(&db)
    .await
    .expect("test session should persist");
    let app = router(
        AppState {
            db: db.clone(),
            auth: Arc::new(AuthState::new(
                None,
                Some(CoachOAuthConfig {
                    client_id: COACH_CLIENT_ID.to_owned(),
                    client_secret: "test-chatgpt-secret".to_owned(),
                    redirect_uri: "https://chatgpt.test/oauth/callback".to_owned(),
                }),
            )),
        },
        static_dir,
    );
    (app, db)
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
        .header(header::COOKIE, format!("garmin_fit_session={TEST_TOKEN}"))
        .body(Body::from(body))
        .expect("multipart request")
}

fn zip_archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let cursor = std::io::Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(cursor);
    let options = zip::write::SimpleFileOptions::default();
    for (name, bytes) in entries {
        writer
            .start_file(*name, options)
            .expect("ZIP entry should start");
        writer.write_all(bytes).expect("ZIP entry should write");
    }
    writer
        .finish()
        .expect("ZIP archive should finish")
        .into_inner()
}
fn multipart_for_token(token: &str, parts: &[(&str, Option<&str>, &[u8])]) -> Request<Body> {
    let mut request = multipart(parts);
    request
        .headers_mut()
        .insert(header::COOKIE, token.parse().expect("cookie header"));
    request
}
fn session_cookie(response: &axum::response::Response) -> String {
    response
        .headers()
        .get(header::SET_COOKIE)
        .expect("session cookie")
        .to_str()
        .expect("session cookie header")
        .split(';')
        .next()
        .expect("cookie value")
        .to_owned()
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body");
    serde_json::from_slice(&body).expect("JSON response")
}
async fn debug_login(app: &axum::Router, user: &str) -> String {
    unsafe {
        std::env::set_var("GARMIN_FIT_TEST_AUTH", "true");
    }
    let response = app
        .clone()
        .oneshot(
            Request::get(format!("/api/v1/auth/test-login?user={user}"))
                .body(Body::empty())
                .expect("test login request"),
        )
        .await
        .expect("test login response");
    assert!(response.status().is_redirection());
    session_cookie(&response)
}

async fn upload_fixture(app: &axum::Router, cookie: &str, name: &str) -> String {
    let archive = std::fs::read("tests/fixtures/activity.zip").expect("canonical FIT archive");
    let response = app
        .clone()
        .oneshot(multipart_for_token(
            cookie,
            &[("files", Some(name), &archive)],
        ))
        .await
        .expect("upload response");
    assert_eq!(response.status(), StatusCode::CREATED);
    response_json(response).await["items"][0]["id"]
        .as_str()
        .expect("activity ID")
        .to_owned()
}

async fn authorize_code(app: &axum::Router, cookie: &str, state_value: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::get(format!(
                "/oauth/authorize?client_id={COACH_CLIENT_ID}&redirect_uri=https%3A%2F%2Fchatgpt.test%2Foauth%2Fcallback&response_type=code&scope=activities%3Aread&state={state_value}"
            ))
            .header(header::COOKIE, cookie)
            .body(Body::empty())
            .expect("authorize request"),
        )
        .await
        .expect("authorize response");
    assert!(response.status().is_redirection());
    let location = response.headers()[header::LOCATION]
        .to_str()
        .expect("authorize location");
    location
        .split("code=")
        .nth(1)
        .and_then(|value| value.split('&').next())
        .expect("authorization code")
        .to_owned()
}

async fn exchange_code(app: &axum::Router, code: &str) -> Value {
    let body = format!(
        "grant_type=authorization_code&client_id={COACH_CLIENT_ID}&client_secret=test-chatgpt-secret&code={code}&redirect_uri=https%3A%2F%2Fchatgpt.test%2Foauth%2Fcallback"
    );
    let response = app
        .clone()
        .oneshot(
            Request::post("/oauth/token")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("token request"),
        )
        .await
        .expect("token response");
    assert_eq!(response.status(), StatusCode::OK);
    response_json(response).await
}
#[tokio::test]
async fn rejects_unauthenticated_private_requests() {
    let app = test_app().await;
    let response = app
        .oneshot(
            Request::get("/api/v1/extractions")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response_json(response).await,
        json!({"error": {"code": "AUTH_REQUIRED", "message": "Sign in with Google to continue."}})
    );
}
#[tokio::test]
async fn rejects_invalid_session_cookies() {
    let app = test_app().await;
    let response = app
        .oneshot(
            Request::get("/api/v1/extractions")
                .header(header::COOKIE, "garmin_fit_session=invalid")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response_json(response).await["error"]["code"],
        "AUTH_REQUIRED"
    );
}

#[tokio::test]
async fn supports_guarded_test_login_and_user_scoped_history() {
    unsafe {
        std::env::set_var("GARMIN_FIT_TEST_AUTH", "true");
    }
    let app = test_app().await;

    let alice_login = app
        .clone()
        .oneshot(
            Request::get("/api/v1/auth/test-login?user=alice")
                .body(Body::empty())
                .expect("Alice login request"),
        )
        .await
        .expect("Alice login response");
    assert!(alice_login.status().is_redirection());
    let alice_cookie = session_cookie(&alice_login);

    let bob_login = app
        .clone()
        .oneshot(
            Request::get("/api/v1/auth/test-login?user=bob")
                .body(Body::empty())
                .expect("Bob login request"),
        )
        .await
        .expect("Bob login response");
    assert!(bob_login.status().is_redirection());
    let bob_cookie = session_cookie(&bob_login);

    let me = app
        .clone()
        .oneshot(
            Request::get("/api/v1/auth/me")
                .header(header::COOKIE, &alice_cookie)
                .body(Body::empty())
                .expect("current user request"),
        )
        .await
        .expect("current user response");
    assert_eq!(me.status(), StatusCode::OK);
    assert_eq!(
        response_json(me).await["user"]["email"],
        "alice@example.test"
    );

    let invalid_order = app
        .clone()
        .oneshot(
            Request::get("/api/v1/extractions?order=sideways")
                .header(header::COOKIE, &alice_cookie)
                .body(Body::empty())
                .expect("invalid history order request"),
        )
        .await
        .expect("invalid history order response");
    assert_eq!(invalid_order.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(invalid_order).await["error"]["code"],
        "INVALID_PAGINATION"
    );
    let fixture = std::fs::read("tests/fixtures/activity.fit").expect("FIT fixture");
    let archive = zip_archive(&[("alice.fit", &fixture)]);
    let created = app
        .clone()
        .oneshot(multipart_for_token(
            &alice_cookie,
            &[("files", Some("alice.zip"), &archive)],
        ))
        .await
        .expect("Alice upload response");
    assert_eq!(created.status(), StatusCode::CREATED);
    let id = response_json(created).await["items"][0]["id"]
        .as_str()
        .expect("Alice extraction ID")
        .to_owned();

    let bob_list = app
        .clone()
        .oneshot(
            Request::get("/api/v1/extractions")
                .header(header::COOKIE, &bob_cookie)
                .body(Body::empty())
                .expect("Bob history request"),
        )
        .await
        .expect("Bob history response");
    assert_eq!(bob_list.status(), StatusCode::OK);
    assert_eq!(response_json(bob_list).await["total"], 0);

    let bob_detail = app
        .clone()
        .oneshot(
            Request::get(format!("/api/v1/extractions/{id}"))
                .header(header::COOKIE, &bob_cookie)
                .body(Body::empty())
                .expect("Bob detail request"),
        )
        .await
        .expect("Bob detail response");
    assert_eq!(bob_detail.status(), StatusCode::NOT_FOUND);
    let bob_delete = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/v1/extractions/{id}"))
                .header(header::COOKIE, &bob_cookie)
                .body(Body::empty())
                .expect("Bob delete request"),
        )
        .await
        .expect("Bob delete response");
    assert_eq!(bob_delete.status(), StatusCode::NOT_FOUND);

    let alice_list = app
        .clone()
        .oneshot(
            Request::get("/api/v1/extractions")
                .header(header::COOKIE, &alice_cookie)
                .body(Body::empty())
                .expect("Alice history request"),
        )
        .await
        .expect("Alice history response");
    assert_eq!(alice_list.status(), StatusCode::OK);
    assert_eq!(response_json(alice_list).await["total"], 1);
    let logout = app
        .clone()
        .oneshot(
            Request::post("/api/v1/auth/logout")
                .header(header::COOKIE, &bob_cookie)
                .body(Body::empty())
                .expect("logout request"),
        )
        .await
        .expect("logout response");
    assert_eq!(logout.status(), StatusCode::NO_CONTENT);

    let bob_me = app
        .clone()
        .oneshot(
            Request::get("/api/v1/auth/me")
                .header(header::COOKIE, &bob_cookie)
                .body(Body::empty())
                .expect("logged-out current user request"),
        )
        .await
        .expect("logged-out current user response");
    assert_eq!(bob_me.status(), StatusCode::OK);
    assert!(response_json(bob_me).await["user"].is_null());

    let logout_again = app
        .oneshot(
            Request::post("/api/v1/auth/logout")
                .body(Body::empty())
                .expect("logged-out logout request"),
        )
        .await
        .expect("logged-out logout response");
    assert_eq!(logout_again.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn auth_failures_do_not_expose_provider_details() {
    let app = test_app().await;
    let login = app
        .clone()
        .oneshot(
            Request::get("/api/v1/auth/login")
                .body(Body::empty())
                .expect("login request"),
        )
        .await
        .expect("login response");
    assert_eq!(login.status(), StatusCode::SERVICE_UNAVAILABLE);

    assert_eq!(
        response_json(login).await["error"]["code"],
        "AUTH_NOT_CONFIGURED"
    );

    let callback = app
        .oneshot(
            Request::get("/api/v1/auth/callback?code=&state=")
                .body(Body::empty())
                .expect("callback request"),
        )
        .await
        .expect("callback response");
    assert!(callback.status().is_redirection());
    assert_eq!(
        callback.headers()[header::LOCATION],
        "/?authError=AUTH_FAILED"
    );
}
#[tokio::test]
async fn records_no_fit_archive_failures_with_archive_size() {
    let app = test_app().await;
    let archive = zip_archive(&[("notes.txt", b"not a FIT file")]);
    let archive_size = archive.len();
    let response = app
        .oneshot(multipart(&[("files", Some("empty.zip"), &archive)]))
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::CREATED);
    let item = &response_json(response).await["items"][0];
    assert_eq!(item["fileName"], "empty.zip");
    assert_eq!(item["fileSizeBytes"], archive_size);
    assert_eq!(item["error"]["code"], "NO_FIT_FILES");
}

#[tokio::test]
async fn preserves_duplicate_member_rows_and_continues_after_bad_members() {
    let app = test_app().await;
    let fixture = std::fs::read("tests/fixtures/activity.fit").expect("FIT fixture");
    let archive = zip_archive(&[
        ("one.fit", b"corrupt"),
        ("nested/one.fit", &fixture),
        ("other/one.fit", &fixture),
    ]);
    let response = app
        .oneshot(multipart(&[("files", Some("duplicates.zip"), &archive)]))
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = response_json(response).await;
    let items = body["items"].as_array().expect("ordered items");
    assert_eq!(items[0]["error"]["code"], "INVALID_FIT");
    assert_eq!(items[1]["status"], "succeeded");
    assert_eq!(items[2]["status"], "succeeded");
    assert_eq!(items[1]["fileName"], "duplicates.zip::one.fit");
    assert_eq!(items[2]["fileName"], "duplicates.zip::one.fit");
    assert_ne!(items[1]["id"], items[2]["id"]);
}

#[tokio::test]
async fn rejects_unsafe_member_paths_and_archive_member_count_limits() {
    let app = test_app().await;
    let unsafe_archive = zip_archive(&[("../escape.fit", b"corrupt")]);
    let unsafe_response = app
        .clone()
        .oneshot(multipart(&[("files", Some("unsafe.zip"), &unsafe_archive)]))
        .await
        .expect("unsafe archive response");
    assert_eq!(unsafe_response.status(), StatusCode::CREATED);
    assert_eq!(
        response_json(unsafe_response).await["items"][0]["error"]["code"],
        "INVALID_ZIP"
    );

    let members = (0..51)
        .map(|index| (format!("member-{index}.fit"), b"corrupt".as_slice()))
        .collect::<Vec<_>>();
    let member_refs = members
        .iter()
        .map(|(name, bytes)| (name.as_str(), *bytes))
        .collect::<Vec<_>>();
    let oversized_archive = zip_archive(&member_refs);
    let response = app
        .oneshot(multipart(&[(
            "files",
            Some("too-many-members.zip"),
            &oversized_archive,
        )]))
        .await
        .expect("member limit response");
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(
        response_json(response).await["items"][0]["error"]["code"],
        "ARCHIVE_LIMIT_EXCEEDED"
    );
}

#[tokio::test]
async fn rejects_empty_batches_with_standard_envelope() {
    let app = test_app().await;
    let response = app.oneshot(multipart(&[])).await.expect("response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(response).await,
        json!({"error": {"code": "EMPTY_BATCH", "message": "Upload at least one ZIP file."}})
    );
}

#[tokio::test]
async fn rejects_unknown_field_without_persisting_earlier_parts() {
    let app = test_app().await;
    let response = app
        .oneshot(multipart(&[
            ("files", Some("earlier.zip"), b"not-a-fit"),
            ("wrong", Some("later.zip"), b"not-a-fit"),
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
        .map(|_| ("files", Some("activity.zip"), b"not-a-fit" as &[u8]))
        .collect::<Vec<_>>();
    let response = app.oneshot(multipart(&parts)).await.expect("response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(response).await,
        json!({"error": {"code": "TOO_MANY_FILES", "message": "Upload at most 10 ZIP files."}})
    );
}

#[tokio::test]
async fn records_invalid_fit_and_filename_failures_in_request_order() {
    let app = test_app().await;
    let corrupt_archive = zip_archive(&[("corrupt.fit", b"not-a-fit")]);
    let response = app
        .oneshot(multipart(&[
            ("files", Some("corrupt.zip"), &corrupt_archive),
            ("files", Some("notes.txt"), b"also-not-a-fit"),
        ]))
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::CREATED);
    let response = response_json(response).await;
    let items = response["items"].as_array().expect("items");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["fileName"], "corrupt.zip::corrupt.fit");
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
                .header(header::COOKIE, format!("garmin_fit_session={TEST_TOKEN}"))
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
    let first_zip = zip_archive(&[("nested/first.fit", &fixture)]);
    let corrupt_zip = zip_archive(&[("corrupt.fit", b"corrupt")]);
    let last_zip = zip_archive(&[("last.fit", &fixture)]);
    let response = app
        .clone()
        .oneshot(multipart(&[
            ("files", Some("first.zip"), &first_zip),
            ("files", Some("corrupt.zip"), &corrupt_zip),
            ("files", Some("last.zip"), &last_zip),
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
    assert_eq!(items[0]["fileName"], "first.zip::first.fit");
    assert_eq!(items[1]["error"]["code"], "INVALID_FIT");

    let first_id = items[0]["id"].as_str().expect("first ID");
    let detail = app
        .oneshot(
            Request::get(format!("/api/v1/extractions/{first_id}"))
                .header(header::COOKIE, format!("garmin_fit_session={TEST_TOKEN}"))
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
            ("files", Some("at-limit.zip"), &exact_limit),
            ("files", Some("over-limit.zip"), &over_limit),
        ]))
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::CREATED);
    let items = response_json(response).await["items"]
        .as_array()
        .expect("items")
        .clone();
    assert_eq!(items[0]["fileSizeBytes"], 20 * 1024 * 1024);
    assert_eq!(items[0]["error"]["code"], "INVALID_ZIP");
    assert_eq!(items[1]["fileSizeBytes"], 20 * 1024 * 1024 + 1);
    assert_eq!(items[1]["error"]["code"], "FILE_TOO_LARGE");
}

#[tokio::test]
async fn lists_downloads_and_deletes_persisted_successful_extractions() {
    let app = test_app().await;
    let fixture = std::fs::read("tests/fixtures/activity.fit").expect("FIT fixture");
    let archive = zip_archive(&[("route test.fit", &fixture)]);
    let created = app
        .clone()
        .oneshot(multipart(&[("files", Some("route test.zip"), &archive)]))
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
                .header(header::COOKIE, format!("garmin_fit_session={TEST_TOKEN}"))
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
                .header(header::COOKIE, format!("garmin_fit_session={TEST_TOKEN}"))
                .body(Body::empty())
                .expect("download request"),
        )
        .await
        .expect("download response");
    assert_eq!(downloaded.status(), StatusCode::OK);
    assert_eq!(
        downloaded.headers()[header::CONTENT_DISPOSITION],
        "attachment; filename=\"route_test.zip__route_test.raw.json\""
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
                .header(header::COOKIE, format!("garmin_fit_session={TEST_TOKEN}"))
                .body(Body::empty())
                .expect("delete request"),
        )
        .await
        .expect("delete response");
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);

    let missing = app
        .oneshot(
            Request::get(format!("/api/v1/extractions/{id}"))
                .header(header::COOKIE, format!("garmin_fit_session={TEST_TOKEN}"))
                .body(Body::empty())
                .expect("detail request"),
        )
        .await
        .expect("detail response");
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn coach_activities_are_owner_filtered_and_bearer_only() {
    let app = test_app().await;
    let alice = debug_login(&app, "alice").await;
    let bob = debug_login(&app, "bob").await;
    let alice_id = upload_fixture(&app, &alice, "alice.zip").await;
    let bob_id = upload_fixture(&app, &bob, "bob.zip").await;

    for token in ["", "Bearer", "Bearer malformed", "Bearer unknown-token"] {
        let response = app
            .clone()
            .oneshot(
                Request::get("/api/v1/activities/latest")
                    .header(header::AUTHORIZATION, token)
                    .body(Body::empty())
                    .expect("bearer request"),
            )
            .await
            .expect("bearer response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    let alice_code = authorize_code(&app, &alice, "alice-state").await;
    let alice_tokens = exchange_code(&app, &alice_code).await;
    let access = alice_tokens["access_token"].as_str().expect("access token");
    let listed = app
        .clone()
        .oneshot(
            Request::get(
                "/api/v1/activities?limit=20&user_id=bob&owner_id=bob&email=bob@example.test",
            )
            .header(header::AUTHORIZATION, format!("Bearer {access}"))
            .body(Body::empty())
            .expect("activity list"),
        )
        .await
        .expect("activity list response");
    assert_eq!(listed.status(), StatusCode::OK);
    let rows = response_json(listed).await;
    assert_eq!(rows.as_array().expect("activity array").len(), 3);
    assert!(
        rows.as_array()
            .expect("activity array")
            .iter()
            .all(|row| row["activity_id"] != bob_id)
    );
    assert!(
        rows.as_array()
            .expect("activity array")
            .iter()
            .any(|row| row["activity_id"] == alice_id)
    );
    let latest = app
        .clone()
        .oneshot(
            Request::get("/api/v1/activities/latest?detail=summary")
                .header(header::AUTHORIZATION, format!("Bearer {access}"))
                .body(Body::empty())
                .expect("latest activity request"),
        )
        .await
        .expect("latest activity response");
    assert_eq!(latest.status(), StatusCode::OK);
    let latest_body = response_json(latest).await;
    assert_ne!(latest_body["activity_id"], bob_id);
    assert_eq!(latest_body["laps"], json!([]));
    assert_eq!(latest_body["heart_rate_zones"], json!([]));
    assert_eq!(latest_body["derived_metrics"], json!({}));

    let detailed = app
        .clone()
        .oneshot(
            Request::get("/api/v1/activities/latest?detail=laps")
                .header(header::AUTHORIZATION, format!("Bearer {access}"))
                .body(Body::empty())
                .expect("detailed activity request"),
        )
        .await
        .expect("detailed activity response");
    assert_eq!(detailed.status(), StatusCode::OK);
    let detailed_body = response_json(detailed).await;
    assert!(!detailed_body["laps"].as_array().unwrap().is_empty());
    assert!(!detailed_body["heart_rate_zones"].is_null());
    let serialized = rows.to_string();
    for forbidden in [
        "owner_id",
        "user_id",
        "email",
        "google_subject",
        "access_token",
        "refresh_token",
    ] {
        assert!(
            !serialized.contains(forbidden),
            "forbidden field {forbidden}"
        );
    }

    let cross_owner = app
        .clone()
        .oneshot(
            Request::get(format!("/api/v1/activities/{bob_id}"))
                .header(header::AUTHORIZATION, format!("Bearer {access}"))
                .body(Body::empty())
                .expect("cross-owner request"),
        )
        .await
        .expect("cross-owner response");
    assert_eq!(cross_owner.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn rejects_expired_cookie_fallback_and_insufficient_scope_tokens() {
    let (app, pool) = test_app_with_static_and_db(PathBuf::from("apps/web/dist")).await;
    let now = db::timestamp_now();
    db::insert_token_pair(
        &pool,
        "expired-access-token",
        "expired-refresh-token",
        COACH_CLIENT_ID,
        TEST_USER,
        "activities:read",
        &now,
        &db::timestamp_after(-60),
        &db::timestamp_after(3600),
    )
    .await
    .expect("expired token should persist");
    db::insert_token_pair(
        &pool,
        "narrow-access-token",
        "narrow-refresh-token",
        COACH_CLIENT_ID,
        TEST_USER,
        "profile",
        &now,
        &db::timestamp_after(3600),
        &db::timestamp_after(3600),
    )
    .await
    .expect("narrow token should persist");

    let expired = app
        .clone()
        .oneshot(
            Request::get("/api/v1/activities/latest")
                .header(header::AUTHORIZATION, "Bearer expired-access-token")
                .body(Body::empty())
                .expect("expired bearer request"),
        )
        .await
        .expect("expired bearer response");
    assert_eq!(expired.status(), StatusCode::UNAUTHORIZED);

    let cookie_only = app
        .clone()
        .oneshot(
            Request::get("/api/v1/activities/latest")
                .header(header::COOKIE, format!("garmin_fit_session={TEST_TOKEN}"))
                .body(Body::empty())
                .expect("cookie-only request"),
        )
        .await
        .expect("cookie-only response");
    assert_eq!(cookie_only.status(), StatusCode::UNAUTHORIZED);

    let narrow = app
        .oneshot(
            Request::get("/api/v1/activities/latest")
                .header(header::AUTHORIZATION, "Bearer narrow-access-token")
                .body(Body::empty())
                .expect("narrow bearer request"),
        )
        .await
        .expect("narrow bearer response");
    assert_eq!(narrow.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn coach_oauth_validates_authorization_and_rotates_refresh_tokens() {
    let app = test_app().await;
    let alice = debug_login(&app, "alice").await;
    let invalid = app
        .clone()
        .oneshot(
            Request::get("/oauth/authorize?client_id=wrong&redirect_uri=https%3A%2F%2Fchatgpt.test%2Foauth%2Fcallback&response_type=code&scope=activities%3Aread&state=x")
                .body(Body::empty())
                .expect("invalid authorize request"),
        )
        .await
        .expect("invalid authorize response");
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(invalid).await,
        json!({"error": "invalid_request"})
    );

    let code = authorize_code(&app, &alice, "state-123").await;
    let tokens = exchange_code(&app, &code).await;
    let keys = tokens.as_object().expect("token object");
    assert_eq!(keys.len(), 5);
    for key in [
        "access_token",
        "token_type",
        "expires_in",
        "refresh_token",
        "scope",
    ] {
        assert!(keys.contains_key(key));
    }
    assert_eq!(tokens["token_type"], "Bearer");
    assert_eq!(tokens["expires_in"], 3600);
    assert_eq!(tokens["scope"], "activities:read");

    let reuse = app
        .clone()
        .oneshot(
            Request::post("/oauth/token")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "grant_type=authorization_code&client_id={COACH_CLIENT_ID}&client_secret=test-chatgpt-secret&code={code}&redirect_uri=https%3A%2F%2Fchatgpt.test%2Foauth%2Fcallback"
                )))
                .expect("reuse request"),
        )
        .await
        .expect("reuse response");
    assert_eq!(reuse.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response_json(reuse).await["error"], "invalid_grant");

    let refresh = tokens["refresh_token"].as_str().expect("refresh token");
    let rotated = app
        .clone()
        .oneshot(
            Request::post("/oauth/token")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "grant_type=refresh_token&client_id={COACH_CLIENT_ID}&client_secret=test-chatgpt-secret&refresh_token={refresh}"
                )))
                .expect("refresh request"),
        )
        .await
        .expect("refresh response");
    assert_eq!(rotated.status(), StatusCode::OK);
    let rotated = response_json(rotated).await;
    assert_ne!(rotated["refresh_token"], refresh);
    let revoked = app
        .oneshot(
            Request::post("/oauth/token")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "grant_type=refresh_token&client_id={COACH_CLIENT_ID}&client_secret=test-chatgpt-secret&refresh_token={refresh}"
                )))
                .expect("revoked refresh request"),
        )
        .await
        .expect("revoked refresh response");
    assert_eq!(revoked.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response_json(revoked).await["error"], "invalid_grant");
}
#[tokio::test]
async fn resumes_pending_oauth_login_after_a_valid_browser_session() {
    let (app, pool) = test_app_with_static_and_db(PathBuf::from("apps/web/dist")).await;
    let cookie = debug_login(&app, "resume-user").await;
    let user_id = Uuid::new_v5(&Uuid::NAMESPACE_URL, b"resume-user");
    db::insert_oauth_login_request(
        &pool,
        "resume-token",
        COACH_CLIENT_ID,
        "https://chatgpt.test/oauth/callback",
        "resume-state",
        "activities:read",
        &db::timestamp_now(),
        &db::timestamp_after(600),
    )
    .await
    .expect("pending request should persist");

    let response = app
        .clone()
        .oneshot(
            Request::get("/oauth/authorize?resume=resume-token")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .expect("resume request"),
        )
        .await
        .expect("resume response");
    assert!(response.status().is_redirection());
    let location = response.headers()[header::LOCATION]
        .to_str()
        .expect("resume callback location");
    assert!(location.starts_with("https://chatgpt.test/oauth/callback?"));
    assert!(location.contains("state=resume-state"));
    let code = location
        .split("code=")
        .nth(1)
        .and_then(|value| value.split('&').next())
        .expect("resumed code");

    let tokens = exchange_code(&app, code).await;
    let access = tokens["access_token"].as_str().expect("access token");
    let stored = db::find_access_token(&pool, access)
        .await
        .expect("access token lookup")
        .expect("access token should exist");
    assert_eq!(stored.client_id, COACH_CLIENT_ID);
    assert_eq!(stored.user_id, user_id);
    assert_eq!(stored.scope, "activities:read");
}
