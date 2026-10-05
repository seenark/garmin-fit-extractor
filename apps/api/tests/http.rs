use std::{future::Future, io::Write, panic::AssertUnwindSafe, path::PathBuf, str::FromStr, sync::Arc, time::Duration};

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
use futures_util::FutureExt;
use serde_json::{Value, json};
use sqlx::{Connection, postgres::{PgConnectOptions, PgPoolOptions}};
use tower::ServiceExt;
use uuid::Uuid;

const TEST_TOKEN: &str = "http-test-session-token";
const TEST_USER: Uuid = Uuid::from_u128(1);

struct TestSchema {
    database_url: String,
    name: String,
}

async fn run_in_schema(schema: TestSchema, test: impl Future<Output = ()>) {
    let outcome = AssertUnwindSafe(test).catch_unwind().await;
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut connection = sqlx::PgConnection::connect(&schema.database_url).await?;
        sqlx::query(sqlx::AssertSqlSafe(format!("DROP SCHEMA {} CASCADE", schema.name)))
            .execute(&mut connection).await?;
        connection.close().await
    }).await.expect("owned HTTP schema cleanup completes")
        .expect("owned HTTP schema cleanup succeeds");
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
}

async fn test_app() -> (TestSchema, axum::Router) {
    test_app_with_static(PathBuf::from("apps/web/dist")).await
}

async fn test_app_with_static(static_dir: PathBuf) -> (TestSchema, axum::Router) {
    let (schema, app, _) = test_app_with_static_and_db(static_dir).await;
    (schema, app)
}

async fn test_app_with_static_and_db(static_dir: PathBuf) -> (TestSchema, axum::Router, sqlx::PgPool) {
    let database_url = std::env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL must point to the approved disposable PostgreSQL database");
    let mut connection = sqlx::PgConnection::connect(&database_url).await.expect("database connects");
    let directory: String = sqlx::query_scalar("SHOW data_directory")
        .fetch_one(&mut connection).await.expect("database identity");
    let expected_directory = std::env::var("PGDATA").expect("approved disposable PGDATA");
    assert_eq!(directory, expected_directory, "verify the disposable cluster before creating a schema");
    let version: String = sqlx::query_scalar("SHOW server_version_num")
        .fetch_one(&mut connection).await.expect("PostgreSQL version");
    assert!((180000..190000).contains(&version.parse::<i32>().unwrap()));
    // Only a fixed prefix and UUID hex enter schema identifiers.
    let name = format!("http_{}", Uuid::new_v4().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE SCHEMA {name}")))
        .execute(&mut connection).await.expect("owned test schema");
    let schema = TestSchema { database_url: database_url.clone(), name: name.clone() };
    connection.close().await.expect("identity connection closes");
    let options = PgConnectOptions::from_str(&database_url).expect("database options")
        .options([("search_path", name.as_str())]);
    let db = PgPoolOptions::new().max_connections(5).connect_with(options)
        .await.expect("isolated schema connects");
    sqlx::migrate!().run(&db).await.expect("owned test schema migrates");
    sqlx::query(
        "INSERT INTO users (id, google_subject, email, display_name, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
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
         VALUES ($1, $2, $3, $4)",
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
            app_origin: Some("http://127.0.0.1:5173".to_owned()),
        },
        static_dir,
    );
    (schema, app, db)
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

    Request::post("/api/v2/runs/imports")
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

async fn upload_fixture(
    app: &axum::Router,
    pool: &sqlx::PgPool,
    cookie: &str,
    name: &str,
) -> String {
    let fixture = std::fs::read("tests/fixtures/runs/garmin_run.fit").expect("public Garmin FIT fixture");
    let archive = zip_archive(&[("activity.fit", &fixture)]);
    let response = app
        .clone()
        .oneshot(multipart_for_token(
            cookie,
            &[("files", Some(name), &archive)],
        ))
        .await
        .expect("upload response");
    assert_eq!(response.status(), StatusCode::CREATED);
    let report = response_json(response).await;
    assert_eq!(report["items"][0]["status"], "imported");
    let id = report["items"][0]["activityId"]
        .as_str()
        .expect("activity ID")
        .to_owned();
    for _ in 0..64 {
        if !garmin_fit_extractor_api::runs::jobs::run_once(pool)
            .await
            .expect("durable job should run")
        {
            break;
        }
    }
    let detail = app
        .clone()
        .oneshot(
            Request::get(format!("/api/v2/runs/{id}"))
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .expect("processed run request"),
        )
        .await
        .expect("processed run response");
    assert_eq!(detail.status(), StatusCode::OK);
    assert_eq!(response_json(detail).await["processing"]["status"], "ready");
    id
}

async fn seed_legacy_coach_activities(
    app: &axum::Router,
    pool: &sqlx::PgPool,
    cookie: &str,
) -> String {
    let me = app
        .clone()
        .oneshot(
            Request::get("/api/v1/auth/me")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .expect("legacy owner request"),
        )
        .await
        .expect("legacy owner response");
    assert_eq!(me.status(), StatusCode::OK);
    let profile = response_json(me).await;
    let user_id = profile["user"]["id"].as_str().unwrap().parse().unwrap();
    let fixture = std::fs::read("tests/fixtures/activity.fit").expect("public FIT fixture");
    let raw = garmin_fit_extractor_api::fit::raw::decode_raw(&fixture).expect("public FIT decodes");
    let analysis = garmin_fit_extractor_api::fit::normalize::normalize(&raw, "legacy.fit");
    let mut first = None;
    for index in 0..3 {
        let row = db::insert_success(
            pool,
            db::NewSuccess {
                user_id,
                file_name: format!("legacy-{index}.fit"),
                file_size_bytes: fixture.len() as u64,
                activity_type: analysis.activity.r#type.clone(),
                activity_date: analysis.activity.date.clone(),
                normalized_json: serde_json::to_string(&analysis).expect("legacy analysis"),
                raw_json: serde_json::to_string(&raw).expect("legacy decoded data"),
            },
        )
        .await
        .expect("stored legacy activity");
        first.get_or_insert(row.id.to_string());
    }
    first.expect("seeded legacy activity")
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
async fn rejects_unauthenticated_private_requests() { let (_schema, app) = test_app().await; run_in_schema(_schema, async move { let response = app
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
); }).await; }
#[tokio::test]
async fn rejects_invalid_session_cookies() { let (_schema, app) = test_app().await; run_in_schema(_schema, async move { let response = app
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
); }).await; }

#[tokio::test]
async fn supports_guarded_test_login_and_user_scoped_history() {
    unsafe {
        std::env::set_var("GARMIN_FIT_TEST_AUTH", "true");
    }
    let (_schema, app, pool) = test_app_with_static_and_db(PathBuf::from("apps/web/dist")).await;
    run_in_schema(_schema, async move {

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
            Request::get("/api/v2/runs?order=sideways")
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
    let id = upload_fixture(&app, &pool, &alice_cookie, "alice.zip").await;

    let bob_list = app
        .clone()
        .oneshot(
            Request::get("/api/v2/runs")
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
            Request::get(format!("/api/v2/runs/{id}"))
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
            Request::delete(format!("/api/v2/runs/{id}"))
                .header(header::COOKIE, &bob_cookie)
                .body(Body::empty())
                .expect("Bob delete request"),
        )
        .await
        .expect("Bob delete response");
    assert_eq!(bob_delete.status(), StatusCode::NOT_FOUND);
    let bob_export = app
        .clone()
        .oneshot(
            Request::post("/api/v2/runs/exports")
                .header(header::COOKIE, &bob_cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({
                    "activityIds": [id],
                    "mode": "full",
                    "includeLocation": false,
                    "includeDeviceIdentifiers": false
                }).to_string()))
                .expect("Bob export request"),
        )
        .await
        .expect("Bob export response");
    assert_eq!(bob_export.status(), StatusCode::NOT_FOUND);
    let alice_export = app
        .clone()
        .oneshot(
            Request::post("/api/v2/runs/exports")
                .header(header::COOKIE, &alice_cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({
                    "activityIds": [id],
                    "mode": "coach",
                    "includeLocation": false,
                    "includeDeviceIdentifiers": false
                }).to_string()))
                .expect("Alice export request"),
        )
        .await
        .expect("Alice export response");
    assert_eq!(alice_export.status(), StatusCode::CREATED);
    let alice_export = response_json(alice_export).await;
    let bob_download = app
        .clone()
        .oneshot(
            Request::get(alice_export["downloadUrl"].as_str().unwrap())
                .header(header::COOKIE, &bob_cookie)
                .body(Body::empty())
                .expect("Bob export download request"),
        )
        .await
        .expect("Bob export download response");
    assert_eq!(bob_download.status(), StatusCode::NOT_FOUND);

    let alice_list = app
        .clone()
        .oneshot(
            Request::get("/api/v2/runs")
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
    }).await;
}

#[tokio::test]
async fn auth_failures_do_not_expose_provider_details() { let (_schema, app) = test_app().await; run_in_schema(_schema, async move { let login = app
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
); }).await; }
#[tokio::test]
async fn retired_extraction_routes_require_auth_and_cannot_write_export_or_delete_runs() { let (_schema, app, pool) = test_app_with_static_and_db(PathBuf::from("apps/web/dist")).await; run_in_schema(_schema, async move { let cookie = format!("garmin_fit_session={TEST_TOKEN}");
let id = upload_fixture(&app, &pool, &cookie, "public.zip").await;
let routes = [
    (axum::http::Method::GET, "/api/v1/extractions".to_owned()),
    (axum::http::Method::POST, "/api/v1/extractions".to_owned()),
    (axum::http::Method::DELETE, "/api/v1/extractions".to_owned()),
    (axum::http::Method::GET, format!("/api/v1/extractions/{id}")),
    (axum::http::Method::DELETE, format!("/api/v1/extractions/{id}")),
    (axum::http::Method::GET, format!("/api/v1/extractions/{id}/download?view=raw")),
    (axum::http::Method::GET, format!("/api/v1/extractions/{id}/download?view=normalized")),
];
for (method, path) in routes {
    for authenticated in [false, true] {
        let mut request = Request::builder().method(method.clone()).uri(&path);
        if authenticated {
            request = request.header(header::COOKIE, &cookie);
        }
        let response = app
            .clone()
            .oneshot(request.body(Body::empty()).expect("retired request"))
            .await
            .expect("retired response");
        assert_eq!(
            response.status(),
            if authenticated { StatusCode::GONE } else { StatusCode::UNAUTHORIZED },
        );
        assert_eq!(
            response_json(response).await["error"]["code"],
            if authenticated { "RUNS_ENDPOINT_RETIRED" } else { "AUTH_REQUIRED" },
        );
    }
}
let fixture = std::fs::read("tests/fixtures/runs/garmin_run.fit").expect("public Garmin FIT fixture");
let mut legacy_write = multipart(&[("files", Some("activity.fit"), &fixture)]);
*legacy_write.uri_mut() = "/api/v1/extractions".parse().unwrap();
let response = app.clone().oneshot(legacy_write).await.expect("legacy write response");
assert_eq!(response.status(), StatusCode::GONE);
assert_eq!(response_json(response).await["error"]["code"], "RUNS_ENDPOINT_RETIRED");
let listed = app
    .oneshot(
        Request::get("/api/v2/runs")
            .header(header::COOKIE, cookie)
            .body(Body::empty())
            .expect("unchanged runs request"),
    )
    .await
    .expect("unchanged runs response");
assert_eq!(listed.status(), StatusCode::OK);
let listed = response_json(listed).await;
assert_eq!(listed["total"], 1);
assert_eq!(listed["items"][0]["id"], id); }).await; }

#[tokio::test]
async fn continues_after_bad_zip_members_and_reports_exact_duplicates() { let (_schema, app) = test_app().await; run_in_schema(_schema, async move { let fixture = std::fs::read("tests/fixtures/runs/garmin_run.fit").expect("public Garmin FIT fixture");
let archive = zip_archive(&[
    ("corrupt.fit", b"corrupt"),
    ("nested/one.fit", &fixture),
    ("other/one.fit", &fixture),
]);
let response = app
    .oneshot(multipart(&[("files", Some("duplicates.zip"), &archive)]))
    .await
    .expect("import response");
assert_eq!(response.status(), StatusCode::CREATED);
let body = response_json(response).await;
let items = body["items"].as_array().expect("ordered items");
assert_eq!(items.len(), 3);
assert_eq!(items[0]["status"], "failed");
assert!(items[0]["activityId"].is_null());
assert_eq!(items[1]["status"], "imported");
assert_eq!(items[2]["status"], "duplicate");
assert_eq!(items[1]["activityId"], items[2]["activityId"]);
assert_eq!(body["counts"]["imported"], 1);
assert_eq!(body["counts"]["duplicate"], 1);
assert_eq!(body["counts"]["failed"], 1); }).await; }

#[tokio::test]
async fn rejects_unsafe_archive_paths_without_creating_runs() { let (_schema, app) = test_app().await; run_in_schema(_schema, async move { let archive = zip_archive(&[("../escape.fit", b"corrupt")]);
let response = app
    .clone()
    .oneshot(multipart(&[("files", Some("unsafe.zip"), &archive)]))
    .await
    .expect("unsafe archive response");
assert_eq!(response.status(), StatusCode::CREATED);
let body = response_json(response).await;
assert_eq!(body["items"][0]["status"], "failed");
assert_eq!(body["items"][0]["reason"], "UNSAFE_ARCHIVE_PATH");
assert!(body["items"][0]["activityId"].is_null());
let listed = app
    .oneshot(
        Request::get("/api/v2/runs")
            .header(header::COOKIE, format!("garmin_fit_session={TEST_TOKEN}"))
            .body(Body::empty())
            .expect("list request"),
    )
    .await
    .expect("list response");
assert_eq!(listed.status(), StatusCode::OK);
assert_eq!(response_json(listed).await["total"], 0); }).await; }


#[tokio::test]
async fn rejects_empty_batches_with_standard_envelope() { let (_schema, app) = test_app().await; run_in_schema(_schema, async move { let response = app.oneshot(multipart(&[])).await.expect("response");

assert_eq!(response.status(), StatusCode::BAD_REQUEST);
assert_eq!(response_json(response).await["error"]["code"], "EMPTY_BATCH"); }).await; }

#[tokio::test]
async fn rejects_unknown_field_without_persisting_earlier_parts() { let (_schema, app) = test_app().await; run_in_schema(_schema, async move { let fixture = std::fs::read("tests/fixtures/runs/garmin_run.fit").expect("public Garmin FIT fixture");
let response = app
    .clone()
    .oneshot(multipart(&[
        ("files", Some("earlier.fit"), &fixture),
        ("wrong", Some("later.fit"), &fixture),
    ]))
    .await
    .expect("import response");
assert_eq!(response.status(), StatusCode::BAD_REQUEST);
assert_eq!(response_json(response).await["error"]["code"], "UNKNOWN_FIELD");
let listed = app
    .oneshot(
        Request::get("/api/v2/runs")
            .header(header::COOKIE, format!("garmin_fit_session={TEST_TOKEN}"))
            .body(Body::empty())
            .expect("list request"),
    )
    .await
    .expect("list response");
assert_eq!(listed.status(), StatusCode::OK);
assert_eq!(response_json(listed).await["total"], 0); }).await; }

#[tokio::test]
async fn rejects_eleventh_import_part_without_creating_runs() { let (_schema, app) = test_app().await; run_in_schema(_schema, async move { let fixture = std::fs::read("tests/fixtures/runs/garmin_run.fit").expect("public Garmin FIT fixture");
let parts = (0..11)
    .map(|_| ("files", Some("activity.fit"), fixture.as_slice()))
    .collect::<Vec<_>>();
let response = app.clone().oneshot(multipart(&parts)).await.expect("response");
assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
assert_eq!(response_json(response).await["error"]["code"], "TOO_MANY_FILES");
let listed = app
    .oneshot(
        Request::get("/api/v2/runs")
            .header(header::COOKIE, format!("garmin_fit_session={TEST_TOKEN}"))
            .body(Body::empty())
            .expect("list request"),
    )
    .await
    .expect("list response");
assert_eq!(listed.status(), StatusCode::OK);
assert_eq!(response_json(listed).await["total"], 0); }).await; }

#[tokio::test]
async fn records_invalid_fit_and_filename_failures_in_request_order() { let (_schema, app) = test_app().await; run_in_schema(_schema, async move { let corrupt_archive = zip_archive(&[("corrupt.fit", b"not-a-fit")]);
let invalid_name = "x".repeat(256);
let response = app
    .oneshot(multipart(&[
        ("files", Some("corrupt.zip"), &corrupt_archive),
        ("files", Some("notes.txt"), b"also-not-a-fit"),
        ("files", Some(&invalid_name), b"also-not-a-fit"),
    ]))
    .await
    .expect("response");

assert_eq!(response.status(), StatusCode::CREATED);
let response = response_json(response).await;
let items = response["items"].as_array().expect("items");
assert_eq!(items.len(), 3);
assert_eq!(items[0]["status"], "failed");
assert!(items[0]["activityId"].is_null());
assert_eq!(items[0]["reason"], "FIT_HEADER_INVALID");
assert_eq!(items[1]["status"], "unsupported");
assert_eq!(items[1]["name"], "notes.txt");
assert_eq!(items[1]["reason"], "UNSUPPORTED_FILE");
assert_eq!(items[2]["status"], "failed");
assert_eq!(items[2]["reason"], "INVALID_FILE_NAME");
assert!(items[2]["activityId"].is_null()); }).await; }

#[tokio::test]
async fn rejects_archive_member_count_limits_without_creating_runs() { let (_schema, app) = test_app().await; run_in_schema(_schema, async move { let members = (0..51)
    .map(|index| (format!("member-{index}.fit"), b"corrupt".as_slice()))
    .collect::<Vec<_>>();
let references = members.iter().map(|(name, bytes)| (name.as_str(), *bytes)).collect::<Vec<_>>();
let archive = zip_archive(&references);
let response = app
    .clone()
    .oneshot(multipart(&[("files", Some("too-many-members.zip"), &archive)]))
    .await
    .expect("member limit response");
assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
assert_eq!(response_json(response).await["error"]["code"], "ARCHIVE_LIMIT_EXCEEDED");
let listed = app
    .oneshot(
        Request::get("/api/v2/runs")
            .header(header::COOKIE, format!("garmin_fit_session={TEST_TOKEN}"))
            .body(Body::empty())
            .expect("list request"),
    )
    .await
    .expect("list response");
assert_eq!(listed.status(), StatusCode::OK);
assert_eq!(response_json(listed).await["total"], 0); }).await; }

#[tokio::test]
async fn rejects_oversized_upload_without_creating_runs() { let (_schema, app) = test_app().await; run_in_schema(_schema, async move { let bytes = vec![0; 20 * 1024 * 1024 + 1];
let response = app
    .clone()
    .oneshot(multipart(&[("files", Some("oversized.fit"), &bytes)]))
    .await
    .expect("oversized upload response");
assert_eq!(response.status(), StatusCode::CREATED);
let body = response_json(response).await;
assert_eq!(body["items"][0]["status"], "failed");
assert_eq!(body["items"][0]["reason"], "FILE_TOO_LARGE");
assert!(body["items"][0]["activityId"].is_null());
let listed = app
    .oneshot(
        Request::get("/api/v2/runs")
            .header(header::COOKIE, format!("garmin_fit_session={TEST_TOKEN}"))
            .body(Body::empty())
            .expect("list request"),
    )
    .await
    .expect("list response");
assert_eq!(listed.status(), StatusCode::OK);
assert_eq!(response_json(listed).await["total"], 0); }).await; }

#[tokio::test]
async fn v2_run_routes_require_browser_authentication() { let (_schema, app) = test_app().await; run_in_schema(_schema, async move { let id = Uuid::now_v7();
let routes = [
    (axum::http::Method::POST, "/api/v2/runs/imports".to_owned()),
    (axum::http::Method::GET, "/api/v2/runs".to_owned()),
    (axum::http::Method::GET, format!("/api/v2/runs/{id}")),
    (axum::http::Method::DELETE, format!("/api/v2/runs/{id}")),
    (axum::http::Method::POST, "/api/v2/runs/exports".to_owned()),
    (axum::http::Method::GET, format!("/api/v2/runs/exports/{id}")),
];
for (method, path) in routes {
    let response = app
        .clone()
        .oneshot(
            Request::builder().method(method).uri(path)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .expect("unauthenticated Runs request"),
        )
        .await
        .expect("unauthenticated Runs response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(response_json(response).await["error"]["code"], "AUTH_REQUIRED");
} }).await; }

#[tokio::test]
async fn returns_json_404_for_unknown_api_route() { let (_schema, app) = test_app().await; run_in_schema(_schema, async move { let response = app
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
); }).await; }

#[tokio::test]
async fn serves_index_for_non_api_client_routes() {
    let static_dir =
        std::env::temp_dir().join(format!("garmin-fit-extractor-static-{}", Uuid::now_v7()));
    std::fs::create_dir_all(&static_dir).expect("static directory");
    std::fs::write(static_dir.join("index.html"), "<main>FIT app</main>").expect("index file");
    let (_schema, app) = test_app_with_static(static_dir).await;
    run_in_schema(_schema, async move {

    let root_response = app
        .clone()
        .oneshot(
            Request::get("/")
                .body(Body::empty())
                .expect("root SPA request"),
        )
        .await
        .expect("root response");
    assert_eq!(root_response.status(), StatusCode::OK);
    assert_eq!(
        root_response
            .headers()
            .get(header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some("no-cache"),
    );

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
        response
            .headers()
            .get(header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some("no-cache"),
    );
    assert_eq!(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("SPA body"),
        "<main>FIT app</main>"
    );
    }).await;
}

#[tokio::test]
async fn healthz_pings_postgres_and_returns_ok() { let (_schema, app) = test_app().await; run_in_schema(_schema, async move { let response = app
    .oneshot(
        Request::get("/healthz")
            .body(Body::empty())
            .expect("health request"),
    )
    .await
    .expect("response");

assert_eq!(response.status(), StatusCode::OK);
assert_eq!(response_json(response).await, json!({"status": "ok"})); }).await; }

#[tokio::test]
async fn maps_missing_multipart_boundary_to_invalid_multipart() { let (_schema, app) = test_app().await; run_in_schema(_schema, async move { let response = app
    .oneshot(
        Request::post("/api/v2/runs/imports")
            .header(header::CONTENT_TYPE, "multipart/form-data")
            .header(header::COOKIE, format!("garmin_fit_session={TEST_TOKEN}"))
            .body(Body::from("not multipart"))
            .expect("request"),
    )
    .await
    .expect("response");

assert_eq!(response.status(), StatusCode::BAD_REQUEST);
assert_eq!(response_json(response).await["error"]["code"], "INVALID_MULTIPART"); }).await; }

#[tokio::test]
async fn lists_exports_and_deletes_processed_runs() { let (_schema, app, pool) = test_app_with_static_and_db(PathBuf::from("apps/web/dist")).await; run_in_schema(_schema, async move { let cookie = format!("garmin_fit_session={TEST_TOKEN}");
let id = upload_fixture(&app, &pool, &cookie, "route-test.zip").await;
let listed = app
    .clone()
    .oneshot(
        Request::get("/api/v2/runs?limit=1&offset=0&sort=startTime&order=desc")
            .header(header::COOKIE, &cookie)
            .body(Body::empty())
            .expect("list request"),
    )
    .await
    .expect("list response");
assert_eq!(listed.status(), StatusCode::OK);
let listed = response_json(listed).await;
assert_eq!(listed["items"][0]["id"], id);
assert_eq!(listed["items"][0]["processing"]["status"], "ready");
let detail = app
    .clone()
    .oneshot(
        Request::get(format!("/api/v2/runs/{id}"))
            .header(header::COOKIE, &cookie)
            .body(Body::empty())
            .expect("detail request"),
    )
    .await
    .expect("detail response");
assert_eq!(detail.status(), StatusCode::OK);
let detail = response_json(detail).await;
assert_eq!(detail["id"], id);
assert_eq!(detail["normalized"]["schemaVersion"], "2.0.0");
assert_eq!(detail["normalized"]["sport"], "running");
assert!(detail.get("decoded").is_none());
assert!(detail["revisionId"].is_string());
assert!(detail["analysis"].is_object());
let exported = app
    .clone()
    .oneshot(
        Request::post("/api/v2/runs/exports")
            .header(header::COOKIE, &cookie)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({
                "activityIds": [id],
                "mode": "full",
                "includeLocation": false,
                "includeDeviceIdentifiers": false
            }).to_string()))
            .expect("export request"),
    )
    .await
    .expect("export response");
assert_eq!(exported.status(), StatusCode::CREATED);
let exported = response_json(exported).await;
let url = exported["downloadUrl"].as_str().expect("export download URL");
let mut pinned_bytes = None;
for _ in 0..2 {
    let downloaded = app
        .clone()
        .oneshot(
            Request::get(url)
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .expect("download request"),
        )
        .await
        .expect("download response");
    assert_eq!(downloaded.status(), StatusCode::OK);
    assert!(downloaded.headers()[header::CACHE_CONTROL].to_str().unwrap().contains("no-store"));
    let bytes = to_bytes(downloaded.into_body(), usize::MAX).await.expect("download body");
    assert!(bytes.ends_with(b"\n"));
    assert_eq!(exported["byteLength"], bytes.len());
    let document: Value = serde_json::from_slice(&bytes).expect("exported JSON");
    assert_eq!(document["selection"], json!([id]));
    assert_eq!(document["activities"].as_array().unwrap().len(), 1);
    assert_eq!(document["activities"][0]["id"], id);
    assert_eq!(document["activities"][0]["decoded"]["schemaVersion"], "2.0.0");
    assert_eq!(document["activities"][0]["normalized"]["sport"], "running");
    let summary = &document["activities"][0]["normalized"]["summary"];
    assert_eq!(summary["distanceMeters"], 1000.0);
    assert_eq!(summary["timerTimeSeconds"], 300.0);
    assert_eq!(summary["elapsedTimeSeconds"], 360.0);
    assert!(summary["movingTimeSeconds"].is_null());
    assert!((summary["averageHeartRateBpm"].as_f64().unwrap() - 126.66666666666667).abs() < 1e-10);
    assert_eq!(summary["coverage"]["averageHeartRateBpm"]["coveredSeconds"], 30.0);
    assert_eq!(summary["coverage"]["averageHeartRateBpm"]["windowSeconds"], 360.0);
    assert_eq!(summary["method"]["derived"], "interval_weighted_left_sample");
    if let Some(previous) = &pinned_bytes {
        assert_eq!(&bytes, previous);
    } else {
        pinned_bytes = Some(bytes);
    }
}
let deleted = app
    .clone()
    .oneshot(
        Request::delete(format!("/api/v2/runs/{id}"))
            .header(header::COOKIE, &cookie)
            .body(Body::empty())
            .expect("delete request"),
    )
    .await
    .expect("delete response");
assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
let missing = app
    .oneshot(
        Request::get(format!("/api/v2/runs/{id}"))
            .header(header::COOKIE, &cookie)
            .body(Body::empty())
            .expect("deleted detail request"),
    )
    .await
    .expect("deleted detail response");
assert_eq!(missing.status(), StatusCode::NOT_FOUND); }).await; }

#[tokio::test]
async fn legacy_coach_activities_are_owner_filtered_and_bearer_only() { let (_schema, app, pool) = test_app_with_static_and_db(PathBuf::from("apps/web/dist")).await; run_in_schema(_schema, async move { let alice = debug_login(&app, "alice").await;
let bob = debug_login(&app, "bob").await;
let alice_id = seed_legacy_coach_activities(&app, &pool, &alice).await;
let bob_id = seed_legacy_coach_activities(&app, &pool, &bob).await;

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
assert_eq!(cross_owner.status(), StatusCode::NOT_FOUND); }).await; }

#[tokio::test]
async fn rejects_expired_cookie_fallback_and_insufficient_scope_tokens() { let (_schema, app, pool) = test_app_with_static_and_db(PathBuf::from("apps/web/dist")).await; run_in_schema(_schema, async move { let now = db::timestamp_now();
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
assert_eq!(narrow.status(), StatusCode::FORBIDDEN); }).await; }

#[tokio::test]
async fn coach_oauth_validates_authorization_and_rotates_refresh_tokens() { let (_schema, app) = test_app().await; run_in_schema(_schema, async move { let alice = debug_login(&app, "alice").await;
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
assert_eq!(response_json(revoked).await["error"], "invalid_grant"); }).await; }
#[tokio::test]
async fn resumes_pending_oauth_login_after_a_valid_browser_session() { let (_schema, app, pool) = test_app_with_static_and_db(PathBuf::from("apps/web/dist")).await; run_in_schema(_schema, async move { let cookie = debug_login(&app, "resume-user").await;
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
assert_eq!(stored.scope, "activities:read"); }).await; }

#[tokio::test]
async fn observed_history_sorting_uses_event_time_and_pagination() { let (_schema, app, pool) = test_app_with_static_and_db(PathBuf::from("apps/web/dist")).await; run_in_schema(_schema, async move { let fixture = std::fs::read("tests/fixtures/activity.fit").expect("public FIT fixture");
let raw = garmin_fit_extractor_api::fit::raw::decode_raw(&fixture).expect("public FIT decodes");
let mut analysis = garmin_fit_extractor_api::fit::normalize::normalize(&raw, "history.fit");
let mut ids = Vec::new();
for (index, date) in [
    "2026-01-01T00:00:00Z",
    "2026-01-01T00:00:00.001Z",
    "2026-01-01T00:00:00.100Z",
].into_iter().enumerate() {
    analysis.activity.date = Some(date.to_owned());
    let row = db::insert_success(
        &pool,
        db::NewSuccess {
            user_id: TEST_USER,
            file_name: format!("history-{index}.fit"),
            file_size_bytes: fixture.len() as u64,
            activity_type: analysis.activity.r#type.clone(),
            activity_date: Some(date.to_owned()),
            normalized_json: serde_json::to_string(&analysis).unwrap(),
            raw_json: serde_json::to_string(&raw).unwrap(),
        },
    ).await.expect("stored history activity");
    ids.push(row.id.to_string());
}
for (query, expected) in [
    ("order=desc&limit=2&offset=0", vec![&ids[2], &ids[1]]),
    ("order=desc&limit=2&offset=2", vec![&ids[0]]),
    ("order=asc&limit=2&offset=0", vec![&ids[0], &ids[1]]),
] {
    let response = app
        .clone()
        .oneshot(
            Request::get(format!("/api/v2/runs?sort=startTime&{query}"))
                .header(header::COOKIE, format!("garmin_fit_session={TEST_TOKEN}"))
                .body(Body::empty())
                .expect("sorted history request"),
        )
        .await
        .expect("sorted history response");
    assert_eq!(response.status(), StatusCode::OK);
    let page = response_json(response).await;
    assert_eq!(page["total"], 3);
    assert_eq!(page["limit"], 2);
    let observed = page["items"].as_array().unwrap().iter()
        .map(|row| row["id"].as_str().unwrap()).collect::<Vec<_>>();
    assert_eq!(observed, expected.iter().map(|id| id.as_str()).collect::<Vec<_>>());
} }).await; }

#[tokio::test]
async fn reports_non_garmin_fit_as_unsupported_without_creating_runs() { let (_schema, app) = test_app().await; run_in_schema(_schema, async move { let fixture = std::fs::read("tests/fixtures/activity.fit").expect("licensed non-Garmin FIT fixture");
let response = app
    .clone()
    .oneshot(multipart(&[("files", Some("non-garmin.fit"), &fixture)]))
    .await
    .expect("unsupported import response");
assert_eq!(response.status(), StatusCode::CREATED);
let report = response_json(response).await;
assert_eq!(report["items"][0]["status"], "unsupported");
assert_eq!(report["items"][0]["reason"], "UNSUPPORTED_MANUFACTURER");
assert!(report["items"][0]["activityId"].is_null());
assert_eq!(report["counts"]["unsupported"], 1);
assert_eq!(report["counts"]["imported"], 0);
let listed = app
    .oneshot(
        Request::get("/api/v2/runs")
            .header(header::COOKIE, format!("garmin_fit_session={TEST_TOKEN}"))
            .body(Body::empty())
            .expect("list request"),
    )
    .await
    .expect("list response");
assert_eq!(listed.status(), StatusCode::OK);
assert_eq!(response_json(listed).await["total"], 0); }).await; }
