use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use futures_util::StreamExt;
use garmin_fit_extractor_api::{
    app::{AppState, router},
    auth::{AuthState, hash_token},
    db,
    runs::{export, jobs, privacy, store},
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::{path::PathBuf, sync::Arc};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::time::{Duration, timeout};
use tower::ServiceExt;
use uuid::Uuid;

async fn fixture() -> (PgPool, PgPool, String, Uuid, Uuid, String, String) {
    fixture_with_fit(include_bytes!("fixtures/runs/garmin_run.fit")).await
}

async fn fixture_with_fit(fit: &[u8]) -> (PgPool, PgPool, String, Uuid, Uuid, String, String) {
    let url = std::env::var("TEST_DATABASE_URL").expect("owned disposable TEST_DATABASE_URL");
    let expected = std::env::var("PGDATA").expect("owned disposable PGDATA");
    assert!(
        !expected.is_empty(),
        "disposable PGDATA identity must not be empty"
    );
    let admin = PgPool::connect(&url).await.unwrap();
    let directory: String = sqlx::query_scalar("SHOW data_directory")
        .fetch_one(&admin)
        .await
        .unwrap();
    assert_eq!(directory, expected, "refuse a different database");
    let version: String = sqlx::query_scalar("SHOW server_version_num")
        .fetch_one(&admin)
        .await
        .unwrap();
    assert!((180000..190000).contains(&version.parse::<i32>().unwrap()));
    let schema = format!("runs_transport_test_{}", Uuid::new_v4().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
        .execute(&admin)
        .await
        .unwrap();
    let selected = schema.clone();
    let pool = PgPoolOptions::new().max_connections(6).after_connect(move |connection, _| {
        let selected = selected.clone();
        Box::pin(async move {
            sqlx::query("SELECT set_config('search_path',$1,false),set_config('application_name',$1,false)")
                .bind(selected).execute(connection).await?;
            Ok(())
        })
    }).connect(&url).await.unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    let owner = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users(id,google_subject,email,created_at,updated_at) VALUES($1,$1,$2,$3,$3)",
    )
    .bind(owner.to_string())
    .bind(format!("{owner}@example.test"))
    .bind(db::created_at_now())
    .execute(&pool)
    .await
    .unwrap();
    // Exercise native acceptance and coherent publication; do not bypass the
    // revision constraints with synthetic normalized payloads.
    unsafe {
        std::env::set_var(
            "RUNS_DECODER_EXECUTABLE",
            env!("CARGO_BIN_EXE_garmin-fit-extractor-api"),
        );
    }
    let admission = jobs::import_admission(&pool).await.unwrap();
    let mut spool = Box::pin(jobs::decode_import(
        &pool,
        fit,
        std::future::pending(),
        Some(admission.decoder_hold().unwrap()),
    ))
    .await
    .unwrap();
    spool.protect_cpu(&admission);
    let activity = Box::pin(store::accept_spool(
        &pool,
        owner,
        fit,
        spool,
        admission.holder(),
    ))
    .await
    .unwrap()
    .activity_id;
    drop(admission);
    assert!(Box::pin(jobs::run_once(&pool)).await.unwrap());
    assert!(Box::pin(jobs::run_once(&pool)).await.unwrap());
    let manifest: String =
        sqlx::query_scalar("SELECT current_manifest_id FROM runs_activities WHERE id=$1")
            .bind(activity.to_string())
            .fetch_one(&pool)
            .await
            .unwrap();
    let token = Uuid::new_v4().to_string();
    let bytes = [vec![b'x'; 65536], vec![b'y'; 65536]].concat();
    sqlx::query("INSERT INTO runs_exports(token,owner_id,activity_ids,manifest_ids,byte_length,mode,meta,generated_at,expires_at) VALUES($1,$2,$3,$4,$5,'full',$6::jsonb,$7,clock_timestamp()+interval '15 minutes')")
        .bind(&token).bind(owner.to_string()).bind(vec![activity.to_string()]).bind(vec![manifest]).bind(bytes.len() as i64).bind(serde_json::json!({"policyVersion":privacy::POLICY_VERSION}).to_string()).bind(db::created_at_now()).execute(&pool).await.unwrap();
    for (position, chunk) in bytes.chunks(65536).enumerate() {
        sqlx::query("INSERT INTO runs_export_chunks(token,position,payload) VALUES($1,$2,$3)")
            .bind(&token)
            .bind(position as i64)
            .bind(chunk)
            .execute(&pool)
            .await
            .unwrap();
    }
    let session = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO sessions(token_hash,user_id,created_at,expires_at) VALUES($1,$2,$3,'2099-01-01T00:00:00Z')")
        .bind(hash_token(&session)).bind(owner.to_string()).bind(db::created_at_now()).execute(&pool).await.unwrap();
    (admin, pool, schema, owner, activity, token, session)
}

async fn cleanup(admin: PgPool, pool: PgPool, schema: String) {
    pool.close().await;
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE")))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delete_waits_for_delivered_bytes_after_producer_eof_and_body_drop() {
    let (admin, pool, schema, owner, activity, token, session) = fixture().await;
    let mut body = export::serve(&pool, owner, &token)
        .await
        .unwrap()
        .into_body()
        .into_data_stream();
    let retained = body.next().await.unwrap().unwrap();
    assert_eq!(retained.as_ref(), vec![b'x'; 65536]);
    drop(body.next().await.unwrap().unwrap());
    assert!(body.next().await.is_none());
    drop(body);
    let app = router(
        AppState {
            db: pool.clone(),
            auth: Arc::new(AuthState::new(None, None)),
            app_origin: None,
        },
        PathBuf::from("nonexistent-test-static"),
    );
    let mut deletion = tokio::spawn(async move {
        Box::pin(
            app.oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/v2/runs/{activity}"))
                    .header("cookie", format!("garmin_fit_session={session}"))
                    .body(Body::empty())
                    .unwrap(),
            ),
        )
        .await
        .unwrap()
    });
    timeout(Duration::from_secs(3), async {
        loop {
            let revoked: Option<bool> =
                sqlx::query_scalar("SELECT revoked FROM runs_exports WHERE token=$1")
                    .bind(&token)
                    .fetch_optional(&pool)
                    .await
                    .unwrap();
            assert!(
                !deletion.is_finished(),
                "DELETE completed while delivered frame still exists"
            );
            if revoked == Some(true) {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM runs_activities WHERE id=$1)")
            .bind(activity.to_string())
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    assert!(
        timeout(Duration::from_millis(250), &mut deletion)
            .await
            .is_err(),
        "DELETE must remain incomplete after revocation while delivered Bytes are retained"
    );
    drop(retained);
    assert_eq!(
        timeout(Duration::from_secs(5), deletion)
            .await
            .unwrap()
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    assert!(
        !sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM runs_exports WHERE token=$1)")
            .bind(token)
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    cleanup(admin, pool, schema).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn revocation_interrupts_an_actual_blocked_chunk_query_and_disconnect_drains() {
    let (admin, pool, schema, owner, activity, token, _) = fixture().await;
    let mut body = export::serve(&pool, owner, &token)
        .await
        .unwrap()
        .into_body()
        .into_data_stream();
    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("LOCK runs_export_chunks IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let reading = tokio::spawn(async move {
        let result = body.next().await;
        (body, result)
    });
    timeout(Duration::from_secs(3), async {
        loop {
            let blocked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE application_name=$1 AND wait_event_type='Lock' AND query LIKE '%c.payload%')")
                .bind(&schema).fetch_one(&admin).await.unwrap();
            if blocked { break; }
            assert!(!reading.is_finished());
            tokio::task::yield_now().await;
        }
    }).await.unwrap();
    sqlx::query("UPDATE runs_exports SET revoked=true WHERE token=$1")
        .bind(&token)
        .execute(&pool)
        .await
        .unwrap();
    let (body, result) = timeout(Duration::from_secs(3), reading)
        .await
        .expect("revocation must interrupt pending SQL fetch")
        .unwrap();
    assert!(
        result.unwrap().is_err(),
        "revoked stream must fail, not report save-complete EOF"
    );
    drop(body);
    blocker.rollback().await.unwrap();
    assert!(
        timeout(
            Duration::from_secs(5),
            store::delete(&pool, owner, activity)
        )
        .await
        .unwrap()
        .unwrap()
    );
    cleanup(admin, pool, schema).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unverified_reader_registry_returns_bounded_503_without_erasing_data() {
    let (admin, pool, schema, owner, activity, token, _) = fixture().await;
    sqlx::query("UPDATE runs_exports SET meta=jsonb_build_object('activeReads',jsonb_build_array($2::text)) WHERE token=$1")
        .bind(&token).bind(Uuid::new_v4().to_string()).execute(&pool).await.unwrap();
    let error = timeout(
        Duration::from_secs(20),
        store::delete(&pool, owner, activity),
    )
    .await
    .expect("unverified reader registry must have a bounded wait")
    .unwrap_err();
    assert_eq!(error.code(), "RUNS_DELETE_INCOMPLETE");
    let retained: (bool, bool) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM runs_activities WHERE id=$1),EXISTS(SELECT 1 FROM runs_exports WHERE token=$2 AND revoked)")
        .bind(activity.to_string()).bind(&token).fetch_one(&pool).await.unwrap();
    assert_eq!(
        retained,
        (true, true),
        "no successful erasure may be claimed without confirmed transport drain"
    );
    cleanup(admin, pool, schema).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actual_http_disconnect_drops_transport_guard_during_blocked_chunk_fetch() {
    let (admin, pool, schema, owner, activity, token, session) = fixture().await;
    let app = router(
        AppState {
            db: pool.clone(),
            auth: Arc::new(AuthState::new(None, None)),
            app_origin: None,
        },
        PathBuf::from("nonexistent-test-static"),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("LOCK runs_export_chunks IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let mut client = tokio::net::TcpStream::connect(address).await.unwrap();
    client.write_all(format!("GET /api/v2/runs/exports/{token} HTTP/1.1\r\nHost: {address}\r\nCookie: garmin_fit_session={session}\r\n\r\n").as_bytes()).await.unwrap();
    timeout(Duration::from_secs(3), async {
        loop {
            let blocked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE application_name=$1 AND wait_event_type='Lock' AND query LIKE '%c.payload%')")
                .bind(&schema).fetch_one(&admin).await.unwrap();
            if blocked { break; }
            tokio::task::yield_now().await;
        }
    }).await.unwrap();
    assert!(sqlx::query_scalar::<_,bool>("SELECT COALESCE(meta->'activeReads','[]'::jsonb)<>'[]'::jsonb FROM runs_exports WHERE token=$1")
        .bind(&token).fetch_one(&pool).await.unwrap());
    drop(client);
    timeout(Duration::from_secs(3), async {
        loop {
            let active: bool = sqlx::query_scalar("SELECT COALESCE(meta->'activeReads','[]'::jsonb)<>'[]'::jsonb FROM runs_exports WHERE token=$1")
                .bind(&token).fetch_one(&pool).await.unwrap();
            if !active { break; }
            tokio::task::yield_now().await;
        }
    }).await.expect("client disconnect must drop Body and its actual transport guard");
    blocker.rollback().await.unwrap();
    assert!(
        timeout(
            Duration::from_secs(5),
            store::delete(&pool, owner, activity)
        )
        .await
        .unwrap()
        .unwrap()
    );
    server.abort();
    let _ = server.await;
    cleanup(admin, pool, schema).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actual_process_crash_recovers_registry_but_live_expired_reader_never_does() {
    if let Ok(schema) = std::env::var("RUNS_TRANSPORT_CHILD_SCHEMA") {
        assert!(schema.starts_with("runs_transport_test_"));
        let url = std::env::var("TEST_DATABASE_URL").expect("owned disposable TEST_DATABASE_URL");
        let expected = std::env::var("PGDATA").expect("owned disposable PGDATA");
        assert!(
            !expected.is_empty(),
            "disposable PGDATA identity must not be empty"
        );
        let owner = Uuid::parse_str(&std::env::var("RUNS_TRANSPORT_CHILD_OWNER").unwrap()).unwrap();
        let token = std::env::var("RUNS_TRANSPORT_CHILD_TOKEN").unwrap();
        let pool = PgPoolOptions::new()
            .max_connections(3)
            .after_connect(move |connection, _| {
                let schema = schema.clone();
                Box::pin(async move {
                    sqlx::query("SELECT set_config('search_path',$1,false)")
                        .bind(schema)
                        .execute(connection)
                        .await?;
                    Ok(())
                })
            })
            .connect(&url)
            .await
            .unwrap();
        let directory: String = sqlx::query_scalar("SHOW data_directory")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(directory, expected, "refuse a different database");
        let version: String = sqlx::query_scalar("SHOW server_version_num")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!((180000..190000).contains(&version.parse::<i32>().unwrap()));
        let mut body = export::serve(&pool, owner, &token)
            .await
            .unwrap()
            .into_body()
            .into_data_stream();
        let retained = body.next().await.unwrap().unwrap();
        println!("RUNS_TRANSPORT_FRAME_READY");
        std::io::Write::flush(&mut std::io::stdout()).unwrap();
        // A real serving process retains both the Body and a delivered frame,
        // even after revocation/TTL. Only its death proves those bytes are gone.
        std::future::pending::<()>().await;
        drop(retained);
        drop(body);
        return;
    }
    let (admin, pool, schema, owner, activity, token, _) = fixture().await;
    let mut child = tokio::process::Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("actual_process_crash_recovers_registry_but_live_expired_reader_never_does")
        .arg("--nocapture")
        .env("RUNS_TRANSPORT_CHILD_SCHEMA", &schema)
        .env("RUNS_TRANSPORT_CHILD_OWNER", owner.to_string())
        .env("RUNS_TRANSPORT_CHILD_TOKEN", &token)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let pid = child.id().unwrap();
    let mut stdout = tokio::io::BufReader::new(child.stdout.take().unwrap());
    timeout(Duration::from_secs(5), async {
        loop {
            let mut line = String::new();
            assert!(
                stdout.read_line(&mut line).await.unwrap() > 0,
                "reader exited before retaining a delivered frame"
            );
            if line.trim_end().ends_with("RUNS_TRANSPORT_FRAME_READY") {
                break;
            }
        }
    })
    .await
    .expect("reader must retain an actual delivered frame before TTL/revocation");
    timeout(Duration::from_secs(5), async {
        loop {
            let registered: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM runs_exports e,jsonb_each(COALESCE(e.meta->'readProcesses','{}'::jsonb)) p WHERE e.token=$1 AND p.value->>'pid'=$2)")
                .bind(&token).bind(pid.to_string()).fetch_one(&pool).await.unwrap();
            if registered { break; }
            assert!(child.try_wait().unwrap().is_none(), "reader subprocess must reach the real export Body");
            tokio::task::yield_now().await;
        }
    }).await.unwrap();
    sqlx::query(
        "UPDATE runs_exports SET expires_at=clock_timestamp()-interval '1 second' WHERE token=$1",
    )
    .bind(&token)
    .execute(&pool)
    .await
    .unwrap();
    let incomplete = timeout(
        Duration::from_secs(20),
        store::delete(&pool, owner, activity),
    )
    .await
    .unwrap()
    .unwrap_err();
    assert_eq!(
        incomplete.code(),
        "RUNS_DELETE_INCOMPLETE",
        "TTL cannot erase a live process's retained transport bytes"
    );
    assert!(child.try_wait().unwrap().is_none());
    child.kill().await.unwrap();
    let _ = child.wait().await.unwrap();
    assert!(
        timeout(
            Duration::from_secs(5),
            store::delete(&pool, owner, activity)
        )
        .await
        .expect("a restarted serving process must recover a proven-dead reader")
        .unwrap()
    );
    assert!(
        !sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM runs_exports WHERE token=$1)")
            .bind(&token)
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    cleanup(admin, pool, schema).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_revocation_returns_503_without_claiming_successful_erasure() {
    let (admin, pool, schema, owner, activity, token, session) = fixture().await;
    let mut body = export::serve(&pool, owner, &token)
        .await
        .unwrap()
        .into_body()
        .into_data_stream();
    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("LOCK runs_exports IN SHARE MODE")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let app = router(
        AppState {
            db: pool.clone(),
            auth: Arc::new(AuthState::new(None, None)),
            app_origin: None,
        },
        PathBuf::from("nonexistent-test-static"),
    );
    let response = timeout(
        Duration::from_secs(10),
        Box::pin(
            app.oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/v2/runs/{activity}"))
                    .header("cookie", format!("garmin_fit_session={session}"))
                    .body(Body::empty())
                    .unwrap(),
            ),
        ),
    )
    .await
    .expect("blocked revocation must have a bounded failure")
    .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let error: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 65536)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(error["error"]["code"], "RUNS_DELETE_INCOMPLETE");
    let state: (bool, bool) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM runs_activities WHERE id=$1),(SELECT revoked FROM runs_exports WHERE token=$2)")
        .bind(activity.to_string()).bind(&token).fetch_one(&pool).await.unwrap();
    assert_eq!(
        state,
        (true, false),
        "failed token revocation cannot claim that activity or transport was erased"
    );
    blocker.rollback().await.unwrap();
    assert_eq!(
        body.next().await.unwrap().unwrap().as_ref(),
        vec![b'x'; 65536]
    );
    drop(body);
    assert!(
        timeout(
            Duration::from_secs(5),
            store::delete(&pool, owner, activity)
        )
        .await
        .unwrap()
        .unwrap()
    );
    cleanup(admin, pool, schema).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_chunk_storage_failure_is_http_500_and_aborts_detail_body_without_publication() {
    let (admin, pool, schema, owner, activity, existing_token, session) = fixture().await;
    let app = router(
        AppState {
            db: pool.clone(),
            auth: Arc::new(AuthState::new(None, None)),
            app_origin: None,
        },
        PathBuf::from("nonexistent-test-static"),
    );
    let baseline = Box::pin(
        app.clone().oneshot(
            Request::builder()
                .uri(format!("/api/v2/runs/{activity}"))
                .header("cookie", format!("garmin_fit_session={session}"))
                .body(Body::empty())
                .unwrap(),
        ),
    )
    .await
    .unwrap();
    assert_eq!(baseline.status(), StatusCode::OK);
    let baseline: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(baseline.into_body(), 16 * 1024 * 1024)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(baseline["normalized"]["schemaVersion"], "2.0.0");
    let request = serde_json::json!({
        "activityIds": [activity],
        "mode": "full",
        "includeLocation": true,
        "includeDeviceIdentifiers": true,
    })
    .to_string();

    // This pool's search_path is only the unique fixture schema. Break an
    // operational chunk read, not the accepted FIT or immutable document bytes.
    sqlx::query("ALTER TABLE runs_revision_chunks RENAME COLUMN owner_id TO unavailable_owner_id")
        .execute(&pool)
        .await
        .unwrap();
    let failed_export = Box::pin(
        app.clone().oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v2/runs/exports")
                .header("cookie", format!("garmin_fit_session={session}"))
                .header("content-type", "application/json")
                .body(Body::from(request.clone()))
                .unwrap(),
        ),
    )
    .await
    .unwrap();
    let failed_status = failed_export.status();
    let failed_error: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(failed_export.into_body(), 65536)
            .await
            .unwrap(),
    )
    .unwrap();
    let falsely_published: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM runs_exports WHERE owner_id=$1 AND token<>$2)",
    )
    .bind(owner.to_string())
    .bind(&existing_token)
    .fetch_one(&pool)
    .await
    .unwrap();
    let failed_detail = Box::pin(
        app.clone().oneshot(
            Request::builder()
                .uri(format!("/api/v2/runs/{activity}"))
                .header("cookie", format!("garmin_fit_session={session}"))
                .body(Body::empty())
                .unwrap(),
        ),
    )
    .await
    .unwrap();
    let detail_status = failed_detail.status();
    let detail_body = axum::body::to_bytes(failed_detail.into_body(), 16 * 1024 * 1024).await;
    // Restore before outcome assertions so a regression leaves no operational
    // break in even this disposable schema.
    sqlx::query("ALTER TABLE runs_revision_chunks RENAME COLUMN unavailable_owner_id TO owner_id")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(failed_status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(failed_error["error"]["code"], "RUNS_STORAGE_FAILED");
    assert!(
        !falsely_published,
        "failed native reads must not publish a snapshot"
    );
    assert_eq!(
        detail_status,
        StatusCode::OK,
        "stream headers precede the failed chunk read"
    );
    assert!(
        detail_body.is_err(),
        "a failed native read must abort the Body, not invent a successful or semantic-error document"
    );

    let recovered_detail = Box::pin(
        app.clone().oneshot(
            Request::builder()
                .uri(format!("/api/v2/runs/{activity}"))
                .header("cookie", format!("garmin_fit_session={session}"))
                .body(Body::empty())
                .unwrap(),
        ),
    )
    .await
    .unwrap();
    assert_eq!(recovered_detail.status(), StatusCode::OK);
    let recovered_detail: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(recovered_detail.into_body(), 16 * 1024 * 1024)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(recovered_detail["normalized"], baseline["normalized"]);
    let recovered_export = Box::pin(
        app.clone().oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v2/runs/exports")
                .header("cookie", format!("garmin_fit_session={session}"))
                .header("content-type", "application/json")
                .body(Body::from(request))
                .unwrap(),
        ),
    )
    .await
    .unwrap();
    assert_eq!(recovered_export.status(), StatusCode::CREATED);
    let created: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(recovered_export.into_body(), 65536)
            .await
            .unwrap(),
    )
    .unwrap();
    let download = Box::pin(
        app.oneshot(
            Request::builder()
                .uri(created["downloadUrl"].as_str().unwrap())
                .header("cookie", format!("garmin_fit_session={session}"))
                .body(Body::empty())
                .unwrap(),
        ),
    )
    .await
    .unwrap();
    assert_eq!(download.status(), StatusCode::OK);
    let snapshot: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(download.into_body(), 16 * 1024 * 1024)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(snapshot["selection"], serde_json::json!([activity]));
    assert_eq!(snapshot["activities"][0]["id"], activity.to_string());
    assert_eq!(
        snapshot["activities"][0]["decoded"]["schemaVersion"],
        "2.0.0"
    );
    assert_eq!(
        snapshot["activities"][0]["normalized"]["schemaVersion"],
        "2.0.0"
    );
    cleanup(admin, pool, schema).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_snapshot_policy_cutover_rejects_old_and_missing_versions_but_preserves_fresh_bytes()
{
    let (admin, pool, schema, owner, activity, _, session) = fixture().await;
    let app = router(
        AppState {
            db: pool.clone(),
            auth: Arc::new(AuthState::new(None, None)),
            app_origin: None,
        },
        PathBuf::from("nonexistent-test-static"),
    );
    let request = |token: &str, method: axum::http::Method| {
        Request::builder()
            .method(method)
            .uri(format!("/api/v2/runs/exports/{token}"))
            .header("cookie", format!("garmin_fit_session={session}"))
            .body(Body::empty())
            .unwrap()
    };
    for (include_location, include_device_identifiers) in
        [(false, false), (true, false), (false, true), (true, true)]
    {
        let created = Box::pin(export::create(
            &pool,
            owner,
            export::ExportRequest {
                activity_ids: vec![activity],
                mode: export::ExportMode::Full,
                include_location,
                include_device_identifiers,
            },
        ))
        .await
        .unwrap();
        let token = created["token"].as_str().unwrap();
        let first = Box::pin(app.clone().oneshot(request(token, axum::http::Method::GET)))
            .await
            .unwrap();
        assert_eq!(first.status(), StatusCode::OK);
        let first = axum::body::to_bytes(first.into_body(), 16 * 1024 * 1024)
            .await
            .unwrap();
        let repeated = Box::pin(app.clone().oneshot(request(token, axum::http::Method::GET)))
            .await
            .unwrap();
        assert_eq!(repeated.status(), StatusCode::OK);
        let repeated = axum::body::to_bytes(repeated.into_body(), 16 * 1024 * 1024)
            .await
            .unwrap();
        assert_eq!(
            repeated, first,
            "serving current-policy tokens must not reproject immutable bytes"
        );
        let head = Box::pin(
            app.clone()
                .oneshot(request(token, axum::http::Method::HEAD)),
        )
        .await
        .unwrap();
        assert_eq!(head.status(), StatusCode::OK);
        assert_eq!(
            head.headers()["content-length"].to_str().unwrap(),
            first.len().to_string()
        );
        assert!(
            axum::body::to_bytes(head.into_body(), 65536)
                .await
                .unwrap()
                .is_empty()
        );
        drop(first);
        drop(repeated);
    }
    for previous_policy in [Some("pre-native-source-proof"), None] {
        let created = Box::pin(export::create(
            &pool,
            owner,
            export::ExportRequest {
                activity_ids: vec![activity],
                mode: export::ExportMode::Full,
                include_location: false,
                include_device_identifiers: false,
            },
        ))
        .await
        .unwrap();
        let token = created["token"].as_str().unwrap();
        let admitted = Box::pin(app.clone().oneshot(request(token, axum::http::Method::GET)))
            .await
            .unwrap();
        assert_eq!(admitted.status(), StatusCode::OK);
        let mut active_body = admitted.into_body().into_data_stream();
        drop(active_body.next().await.unwrap().unwrap());
        // Simulate legacy immutable snapshots by changing only their policy
        // provenance. Do not replay, mutate source bytes, or rewrite the snapshot.
        sqlx::query("UPDATE runs_exports SET meta=CASE WHEN $2::text IS NULL THEN meta-'policyVersion' ELSE jsonb_set(meta,'{policyVersion}',to_jsonb($2::text),true) END WHERE token=$1")
            .bind(token).bind(previous_policy).execute(&pool).await.unwrap();
        let next = timeout(Duration::from_secs(3), active_body.next())
            .await
            .expect("policy mismatch must interrupt an admitted stream")
            .expect("policy mismatch must not silently finish the body");
        assert!(
            next.is_err(),
            "admitted old-policy streams must stop before another frame"
        );
        drop(active_body);
        let get = Box::pin(app.clone().oneshot(request(token, axum::http::Method::GET)))
            .await
            .unwrap();
        assert_eq!(get.status(), StatusCode::NOT_FOUND);
        let error: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(get.into_body(), 65536).await.unwrap())
                .unwrap();
        assert_eq!(error["error"]["code"], "EXPORT_UNAVAILABLE");
        let head = Box::pin(
            app.clone()
                .oneshot(request(token, axum::http::Method::HEAD)),
        )
        .await
        .unwrap();
        assert_eq!(head.status(), StatusCode::NOT_FOUND);
        assert!(
            axum::body::to_bytes(head.into_body(), 65536)
                .await
                .unwrap()
                .is_empty()
        );
    }
    cleanup(admin, pool, schema).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_coach_callback_chunk_insert_failure_is_http_500_without_publication() {
    fn crc(bytes: &[u8]) -> u16 {
        let mut value = 0u16;
        for &byte in bytes {
            value ^= u16::from(byte);
            for _ in 0..8 {
                value = (value >> 1) ^ if value & 1 == 0 { 0 } else { 0xa001 };
            }
        }
        value
    }
    // Replace the original two CC0 laps with 360 contiguous native one-second
    // laps, preserving its session, samples, timer events, and source admission.
    let original = include_bytes!("fixtures/runs/garmin_run.fit");
    let lap_definition = [
        0x44, 0, 0, 19, 0, 10, 254, 2, 0x84, 253, 4, 0x86, 2, 4, 0x86, 7, 4, 0x86, 8, 4, 0x86, 9,
        4, 0x86, 25, 1, 0, 39, 1, 0, 0, 1, 0, 1, 1, 0,
    ];
    let lap_start = original
        .windows(lap_definition.len())
        .position(|bytes| bytes == lap_definition)
        .unwrap();
    let lap_data = lap_start + lap_definition.len();
    let mut fit = original[..lap_data].to_vec();
    for offset in 0..360u32 {
        fit.push(4);
        fit.extend_from_slice(&(offset as u16).to_le_bytes());
        for number in [
            1_000_000_001 + offset,
            1_000_000_000 + offset,
            1000,
            1000,
            100,
        ] {
            fit.extend_from_slice(&number.to_le_bytes());
        }
        fit.extend_from_slice(&[1, 0, 9, 1]);
    }
    fit.extend_from_slice(&original[lap_data + 2 * 27..original.len() - 2]);
    // Original session definition has 13 fields. Its num_laps UInt16 follows
    // message_index, timestamp, start_time, sport, subtype, totals, first_lap_index.
    let session_definition = [0x45, 0, 0, 18, 0, 13];
    let session_start = fit
        .windows(session_definition.len())
        .position(|bytes| bytes == session_definition)
        .unwrap();
    let num_laps = session_start + 6 + 13 * 3 + 27;
    fit[num_laps..num_laps + 2].copy_from_slice(&360u16.to_le_bytes());
    let length = (fit.len() - usize::from(fit[0])) as u32;
    fit[4..8].copy_from_slice(&length.to_le_bytes());
    let header_crc = crc(&fit[..12]);
    fit[12..14].copy_from_slice(&header_crc.to_le_bytes());
    let file_crc = crc(&fit);
    fit.extend_from_slice(&file_crc.to_le_bytes());
    let (admin, pool, schema, owner, activity, _, session) = fixture_with_fit(&fit).await;
    let app = router(
        AppState {
            db: pool.clone(),
            auth: Arc::new(AuthState::new(None, None)),
            app_origin: None,
        },
        PathBuf::from("nonexistent-test-static"),
    );
    let post = |mode: &str| {
        Request::builder()
            .method("POST")
            .uri("/api/v2/runs/exports")
            .header("cookie", format!("garmin_fit_session={session}"))
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::json!({"activityIds":[activity],"mode":mode}).to_string(),
            ))
            .unwrap()
    };
    let mut healthy = Vec::new();
    for mode in ["full", "coach"] {
        let response = Box::pin(app.clone().oneshot(post(mode))).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let created: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 65536)
                .await
                .unwrap(),
        )
        .unwrap();
        let download = Box::pin(
            app.clone().oneshot(
                Request::builder()
                    .uri(created["downloadUrl"].as_str().unwrap())
                    .header("cookie", format!("garmin_fit_session={session}"))
                    .body(Body::empty())
                    .unwrap(),
            ),
        )
        .await
        .unwrap();
        assert_eq!(download.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(download.into_body(), 16 * 1024 * 1024)
            .await
            .unwrap();
        if mode == "coach" {
            let snapshot: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            let laps = &snapshot["activities"][0]["laps"];
            assert_eq!(laps.as_array().unwrap().len(), 360);
            assert!(
                serde_json::to_vec_pretty(laps).unwrap().len() > 2 * 65536,
                "native lap callbacks must force chunk insertion before final flush",
            );
            let prefix = std::str::from_utf8(&bytes)
                .unwrap()
                .find("\"laps\"")
                .unwrap();
            assert!(
                prefix < 65536,
                "first chunk must fill inside native lap callbacks"
            );
        }
        healthy.push((
            mode,
            created["generatedAt"].as_str().unwrap().to_owned(),
            bytes,
        ));
    }
    let published_before: i64 =
        sqlx::query_scalar("SELECT count(*) FROM runs_exports WHERE owner_id=$1")
            .bind(owner.to_string())
            .fetch_one(&pool)
            .await
            .unwrap();
    // Only this fixture schema is affected. The actual SnapshotWriter INSERT,
    // not the callback itself, fails while streaming accepted native lap rows.
    sqlx::query("ALTER TABLE runs_export_chunks RENAME COLUMN payload TO unavailable_payload")
        .execute(&pool)
        .await
        .unwrap();
    let failed = Box::pin(app.clone().oneshot(post("coach"))).await.unwrap();
    let status = failed.status();
    let error: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(failed.into_body(), 65536)
            .await
            .unwrap(),
    )
    .unwrap();
    sqlx::query("ALTER TABLE runs_export_chunks RENAME COLUMN unavailable_payload TO payload")
        .execute(&pool)
        .await
        .unwrap();
    let published_after: i64 =
        sqlx::query_scalar("SELECT count(*) FROM runs_exports WHERE owner_id=$1")
            .bind(owner.to_string())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        published_after, published_before,
        "failed callback must publish no snapshot"
    );
    for (mode, generated, baseline) in healthy {
        let response = Box::pin(app.clone().oneshot(post(mode))).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let created: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 65536)
                .await
                .unwrap(),
        )
        .unwrap();
        let download = Box::pin(
            app.clone().oneshot(
                Request::builder()
                    .uri(created["downloadUrl"].as_str().unwrap())
                    .header("cookie", format!("garmin_fit_session={session}"))
                    .body(Body::empty())
                    .unwrap(),
            ),
        )
        .await
        .unwrap();
        assert_eq!(download.status(), StatusCode::OK);
        let recovered = axum::body::to_bytes(download.into_body(), 16 * 1024 * 1024)
            .await
            .unwrap();
        let expected = std::str::from_utf8(&baseline)
            .unwrap()
            .replace(&generated, created["generatedAt"].as_str().unwrap());
        assert_eq!(
            recovered.as_ref(),
            expected.as_bytes(),
            "restored {mode} snapshot preserves exact bytes except generation time"
        );
    }
    cleanup(admin, pool, schema).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(error["error"]["code"], "EXPORT_FAILED");
}
