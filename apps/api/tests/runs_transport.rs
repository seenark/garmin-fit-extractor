use axum::{body::Body, http::{Request, StatusCode}};
use futures_util::StreamExt;
use garmin_fit_extractor_api::{app::{AppState, router}, auth::{AuthState, hash_token}, db, runs::{export, store}};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::{path::PathBuf, sync::Arc};
use tokio::time::{Duration, timeout};
use tower::ServiceExt;
use uuid::Uuid;

async fn fixture() -> (PgPool, PgPool, String, Uuid, Uuid, String, String) {
    let url = std::env::var("TEST_DATABASE_URL").expect("owned disposable TEST_DATABASE_URL");
    let expected = std::env::var("PGDATA").expect("owned disposable PGDATA");
    assert!(expected.starts_with("/tmp/"));
    let admin = PgPool::connect(&url).await.unwrap();
    let directory: String = sqlx::query_scalar("SHOW data_directory").fetch_one(&admin).await.unwrap();
    assert_eq!(directory, expected, "refuse a different database");
    let schema = format!("runs_transport_test_{}", Uuid::new_v4().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE SCHEMA {schema}"))).execute(&admin).await.unwrap();
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
    let activity = Uuid::new_v4();
    let source = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,google_subject,email,created_at,updated_at) VALUES($1,$1,$2,$3,$3)")
        .bind(owner.to_string()).bind(format!("{owner}@example.test")).bind(db::created_at_now()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO runs_sources(id,owner_id,sha256,size_bytes,bytes,integrity) VALUES($1,$2,$3,1,$4,'verified')")
        .bind(source.to_string()).bind(owner.to_string()).bind(Sha256::digest(b"x").to_vec()).bind(b"x".as_slice()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO runs_activities(id,owner_id,source_id,session_index,start_time,end_time,start_order,end_order,summary,observation_group_id,observation_fingerprint,desired_versions,processing_status) VALUES($1,$2,$3,0,'2026-01-01T00:00:00Z','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z','{}',$1,$4,'{}','ready')")
        .bind(activity.to_string()).bind(owner.to_string()).bind(source.to_string()).bind(vec![1u8;32]).execute(&pool).await.unwrap();
    let mut revisions = Vec::new();
    for stage in ["decoded", "normalized", "analysis"] {
        let revision = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO runs_revisions(id,owner_id,activity_id,stage,generation,versions,input_revision_ids,payload,validation) VALUES($1,$2,$3,$4,1,'{}','[]','{}','{}')")
            .bind(&revision).bind(owner.to_string()).bind(activity.to_string()).bind(stage).execute(&pool).await.unwrap();
        revisions.push(revision);
    }
    let manifest = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO runs_manifests(id,owner_id,activity_id,decoded_revision_id,normalized_revision_id,analysis_revision_id,generation,versions) VALUES($1,$2,$3,$4,$5,$6,1,'{}')")
        .bind(&manifest).bind(owner.to_string()).bind(activity.to_string()).bind(&revisions[0]).bind(&revisions[1]).bind(&revisions[2]).execute(&pool).await.unwrap();
    sqlx::query("UPDATE runs_activities SET current_manifest_id=$2 WHERE id=$1").bind(activity.to_string()).bind(&manifest).execute(&pool).await.unwrap();
    let token = Uuid::new_v4().to_string();
    let bytes = [vec![b'x';65536], vec![b'y';65536]].concat();
    sqlx::query("INSERT INTO runs_exports(token,owner_id,activity_ids,manifest_ids,byte_length,mode,meta,generated_at,expires_at) VALUES($1,$2,$3,$4,$5,'full','{}',$6,clock_timestamp()+interval '15 minutes')")
        .bind(&token).bind(owner.to_string()).bind(vec![activity.to_string()]).bind(vec![manifest]).bind(bytes.len() as i64).bind(db::created_at_now()).execute(&pool).await.unwrap();
    for (position, chunk) in bytes.chunks(65536).enumerate() {
        sqlx::query("INSERT INTO runs_export_chunks(token,position,payload) VALUES($1,$2,$3)").bind(&token).bind(position as i64).bind(chunk).execute(&pool).await.unwrap();
    }
    let session = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO sessions(token_hash,user_id,created_at,expires_at) VALUES($1,$2,$3,'2099-01-01T00:00:00Z')")
        .bind(hash_token(&session)).bind(owner.to_string()).bind(db::created_at_now()).execute(&pool).await.unwrap();
    (admin, pool, schema, owner, activity, token, session)
}

async fn cleanup(admin: PgPool, pool: PgPool, schema: String) {
    pool.close().await;
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE"))).execute(&admin).await.unwrap();
    admin.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delete_waits_for_delivered_bytes_after_producer_eof_and_body_drop() {
    let (admin, pool, schema, owner, activity, token, session) = fixture().await;
    let mut body = export::serve(&pool, owner, &token).await.unwrap().into_body().into_data_stream();
    let retained = body.next().await.unwrap().unwrap();
    assert_eq!(retained.as_ref(), vec![b'x';65536]);
    drop(body.next().await.unwrap().unwrap());
    assert!(body.next().await.is_none());
    drop(body);
    let app = router(AppState { db: pool.clone(), auth: Arc::new(AuthState::new(None,None)), app_origin: None }, PathBuf::from("nonexistent-test-static"));
    let deletion = tokio::spawn(async move {
        Box::pin(app.oneshot(Request::builder().method("DELETE").uri(format!("/api/v2/runs/{activity}"))
            .header("cookie",format!("garmin_fit_session={session}")).body(Body::empty()).unwrap())).await.unwrap()
    });
    timeout(Duration::from_secs(3), async {
        loop {
            let revoked: Option<bool> = sqlx::query_scalar("SELECT revoked FROM runs_exports WHERE token=$1").bind(&token).fetch_optional(&pool).await.unwrap();
            assert!(!deletion.is_finished(), "DELETE completed while delivered frame still exists");
            if revoked == Some(true) { break; }
            tokio::task::yield_now().await;
        }
    }).await.unwrap();
    assert!(sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM runs_activities WHERE id=$1)").bind(activity.to_string()).fetch_one(&pool).await.unwrap());
    drop(retained);
    assert_eq!(timeout(Duration::from_secs(5), deletion).await.unwrap().unwrap().status(), StatusCode::NO_CONTENT);
    assert!(!sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM runs_exports WHERE token=$1)").bind(token).fetch_one(&pool).await.unwrap());
    cleanup(admin, pool, schema).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn revocation_interrupts_an_actual_blocked_chunk_query_and_disconnect_drains() {
    let (admin, pool, schema, owner, activity, token, _) = fixture().await;
    let mut body = export::serve(&pool, owner, &token).await.unwrap().into_body().into_data_stream();
    drop(body.next().await.unwrap().unwrap());
    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("LOCK runs_export_chunks IN ACCESS EXCLUSIVE MODE").execute(&mut *blocker).await.unwrap();
    let reading = tokio::spawn(async move {
        let result = body.next().await;
        (body,result)
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
    sqlx::query("UPDATE runs_exports SET revoked=true WHERE token=$1").bind(&token).execute(&pool).await.unwrap();
    let (body, result) = timeout(Duration::from_secs(3), reading).await.expect("revocation must interrupt pending SQL fetch").unwrap();
    assert!(result.unwrap().is_err(), "revoked stream must fail, not report save-complete EOF");
    drop(body);
    blocker.rollback().await.unwrap();
    assert!(timeout(Duration::from_secs(5), store::delete(&pool,owner,activity)).await.unwrap().unwrap());
    cleanup(admin, pool, schema).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn crash_leftover_registry_returns_bounded_503_without_erasing_data() {
    let (admin, pool, schema, owner, activity, token, _) = fixture().await;
    sqlx::query("UPDATE runs_exports SET meta=jsonb_build_object('activeReads',jsonb_build_array($2::text)) WHERE token=$1")
        .bind(&token).bind(Uuid::new_v4().to_string()).execute(&pool).await.unwrap();
    let error = timeout(Duration::from_secs(20), store::delete(&pool,owner,activity)).await
        .expect("crashed transport registry must have a bounded wait").unwrap_err();
    assert_eq!(error.code(), "RUNS_DELETE_INCOMPLETE");
    let retained: (bool, bool) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM runs_activities WHERE id=$1),EXISTS(SELECT 1 FROM runs_exports WHERE token=$2 AND revoked)")
        .bind(activity.to_string()).bind(&token).fetch_one(&pool).await.unwrap();
    assert_eq!(retained, (true, true), "no successful erasure may be claimed without confirmed transport drain");
    cleanup(admin, pool, schema).await;
}
