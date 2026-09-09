use std::sync::Arc;

use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
};
use garmin_fit_extractor_api::{
    app::{AppState, router},
    auth::{AuthState, hash_token},
    db,
};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

const ADMIN_EMAIL: &str = "admin@example.test";
const APP_ORIGIN: &str = "https://fit.example.test";

const NON_ADMIN_EMAIL: &str = "operator@example.test";

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body");
    serde_json::from_slice(&bytes).expect("JSON response")
}

async fn test_app() -> (axum::Router, sqlx::PgPool, String, String) {
    let database_url = std::env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL must point to a PostgreSQL test database");
    let pool = db::connect(&database_url).await.expect("database connects");
    sqlx::query("TRUNCATE transcript_entries, sessions, users CASCADE")
        .execute(&pool)
        .await
        .expect("test tables should reset");

    let admin_id = Uuid::now_v7();
    let admin_token = format!("admin-{}", Uuid::now_v7());
    let non_admin_id = Uuid::now_v7();
    let non_admin_token = format!("operator-{}", Uuid::now_v7());
    for (id, email, subject) in [
        (admin_id, ADMIN_EMAIL, "test:admin"),
        (non_admin_id, NON_ADMIN_EMAIL, "test:operator"),
    ] {
        sqlx::query(
            "INSERT INTO users (id, google_subject, email, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $4)",
        )
        .bind(id.to_string())
        .bind(subject)
        .bind(email)
        .bind("2026-01-01T00:00:00.000Z")
        .execute(&pool)
        .await
        .expect("test user should persist");
    }
    for (token, user_id) in [(&admin_token, admin_id), (&non_admin_token, non_admin_id)] {
        sqlx::query(
            "INSERT INTO sessions (token_hash, user_id, created_at, expires_at)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(hash_token(token))
        .bind(user_id.to_string())
        .bind("2026-01-01T00:00:00.000Z")
        .bind("2099-01-01T00:00:00.000Z")
        .execute(&pool)
        .await
        .expect("test session should persist");
    }
    let auth = AuthState::new(None, None).with_admin_emails([ADMIN_EMAIL.to_owned()]);
    let app = router(
        AppState {
            db: pool.clone(),
            auth: Arc::new(auth),
            app_origin: Some(APP_ORIGIN.to_owned()),
        },
        "apps/web/dist".into(),
    );
    (app, pool, admin_token, non_admin_token)
}

fn session_request(method: Method, uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::COOKIE, format!("garmin_fit_session={token}"))
        .body(Body::empty())
        .expect("request")
}

fn session_request_with_origin(
    method: Method,
    uri: &str,
    token: &str,
    origin: Option<&str>,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::COOKIE, format!("garmin_fit_session={token}"));
    if let Some(origin) = origin {
        builder = builder.header(header::ORIGIN, origin);
    }
    builder.body(Body::empty()).expect("request")
}

fn json_request(method: Method, uri: &str, token: &str, body: Value) -> Request<Body> {
    json_request_with_origin(method, uri, token, APP_ORIGIN, body)
}

fn json_request_with_origin(
    method: Method,
    uri: &str,
    token: &str,
    origin: &str,
    body: Value,
) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::COOKIE, format!("garmin_fit_session={token}"))
        .header(header::ORIGIN, origin)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .expect("JSON request")
}

#[tokio::test]
async fn protects_and_performs_transcript_crud_against_postgres() {
    let (app, pool, admin_token, non_admin_token) = test_app().await;

    let response = app
        .clone()
        .oneshot(session_request(
            Method::GET,
            "/api/v1/auth/me",
            &admin_token,
        ))
        .await
        .expect("admin current-user response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response_json(response).await["isAdmin"], true);

    let response = app
        .clone()
        .oneshot(session_request(
            Method::GET,
            "/api/v1/auth/me",
            &non_admin_token,
        ))
        .await
        .expect("non-admin current-user response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response_json(response).await["isAdmin"], false);

    let response = app
        .clone()
        .oneshot(
            Request::get("/api/admin/transcript-entries")
                .body(Body::empty())
                .expect("unauthenticated request"),
        )
        .await
        .expect("unauthenticated response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response_json(response).await["error"]["code"],
        "AUTH_REQUIRED"
    );

    let response = app
        .clone()
        .oneshot(session_request(
            Method::GET,
            "/api/admin/transcript-entries",
            &non_admin_token,
        ))
        .await
        .expect("non-admin response");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response_json(response).await["error"]["code"],
        "ADMIN_FORBIDDEN"
    );

    let response = app
        .clone()
        .oneshot(session_request(
            Method::GET,
            "/api/admin/transcript-entries",
            &admin_token,
        ))
        .await
        .expect("empty list response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response_json(response).await, json!({"items": []}));

    let input = json!({
        "channelName": "  Test Channel  ",
        "youtubeUrl": " https://youtu.be/abc123XYZ99?si=tracking ",
        "transcription": "line one\nline two"
    });
    let response = app
        .clone()
        .oneshot(json_request_with_origin(
            Method::POST,
            "/api/admin/transcript-entries",
            &admin_token,
            "https://evil.example.test",
            input.clone(),
        ))
        .await
        .expect("mismatched-origin response");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response_json(response).await["error"]["code"],
        "ADMIN_ORIGIN_FORBIDDEN"
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/admin/transcript-entries")
                .header(header::COOKIE, format!("garmin_fit_session={admin_token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(input.to_string()))
                .expect("missing-origin request"),
        )
        .await
        .expect("missing-origin response");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response_json(response).await["error"]["code"],
        "ADMIN_ORIGIN_FORBIDDEN"
    );

    let response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/admin/transcript-entries",
            &admin_token,
            input.clone(),
        ))
        .await
        .expect("create response");
    assert_eq!(response.status(), StatusCode::CREATED);
    let created = response_json(response).await;
    let id = created["id"].as_i64().expect("created id");
    let created_at = created["createdAt"].as_i64().expect("created timestamp");
    assert_eq!(created["channelName"], "Test Channel");
    assert_eq!(created["videoId"], "abc123XYZ99");
    assert_eq!(created["transcription"], "line one\nline two");
    assert_eq!(created["createdAt"], created["updatedAt"]);

    let response = app
        .clone()
        .oneshot(session_request(
            Method::GET,
            &format!("/api/admin/transcript-entries/{id}"),
            &admin_token,
        ))
        .await
        .expect("get-by-id response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response_json(response).await["id"], id);

    let response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/admin/transcript-entries",
            &admin_token,
            input,
        ))
        .await
        .expect("duplicate response");
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let duplicate = response_json(response).await;
    assert_eq!(duplicate["error"]["code"], "TRANSCRIPT_DUPLICATE_VIDEO");
    assert!(
        !duplicate["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("23505")
    );

    let updated_input = json!({
        "channelName": "Edited Channel",
        "youtubeUrl": "https://www.youtube.com/watch?v=abc123XYZ99",
        "videoId": "abc123XYZ99",
        "transcription": "edited\ntranscription"
    });
    let response = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            &format!("/api/admin/transcript-entries/{id}"),
            &admin_token,
            updated_input,
        ))
        .await
        .expect("update response");
    assert_eq!(response.status(), StatusCode::OK);
    let updated = response_json(response).await;
    assert_eq!(updated["createdAt"], created_at);
    assert!(updated["updatedAt"].as_i64().expect("updated timestamp") > created_at);
    assert_eq!(updated["transcription"], "edited\ntranscription");

    let second_input = json!({
        "channelName": "Second Channel",
        "youtubeUrl": "https://youtu.be/secondVideo99",
        "transcription": "second transcription"
    });
    let response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/admin/transcript-entries",
            &admin_token,
            second_input,
        ))
        .await
        .expect("second create response");
    assert_eq!(response.status(), StatusCode::CREATED);
    let second_id = response_json(response).await["id"]
        .as_i64()
        .expect("second id");

    let response = app
        .clone()
        .oneshot(session_request_with_origin(
            Method::DELETE,
            &format!("/api/admin/transcript-entries/{second_id}"),
            &admin_token,
            Some(APP_ORIGIN),
        ))
        .await
        .expect("delete response");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = app
        .clone()
        .oneshot(session_request(
            Method::GET,
            "/api/admin/transcript-entries",
            &admin_token,
        ))
        .await
        .expect("post-delete list response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response_json(response).await["items"]
            .as_array()
            .expect("post-delete list items")
            .len(),
        1
    );

    let response = app
        .clone()
        .oneshot(json_request(
            Method::DELETE,
            "/api/admin/transcript-entries",
            &admin_token,
            json!({ "confirmation": "ลบทั้งหมด" }),
        ))
        .await
        .expect("rejected delete-all response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(response).await["error"]["code"],
        "TRANSCRIPT_CONFIRMATION_REQUIRED"
    );

    let response = app
        .clone()
        .oneshot(json_request(
            Method::DELETE,
            "/api/admin/transcript-entries",
            &admin_token,
            json!({ "confirmation": "DELETE_ALL" }),
        ))
        .await
        .expect("delete-all response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response_json(response).await, json!({ "deleted": 1 }));

    let response = app
        .oneshot(session_request(
            Method::GET,
            "/api/admin/transcript-entries",
            &admin_token,
        ))
        .await
        .expect("post-delete-all list response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response_json(response).await, json!({"items": []}));

    sqlx::query("TRUNCATE transcript_entries, sessions, users CASCADE")
        .execute(&pool)
        .await
        .expect("test tables should clean up");
}
