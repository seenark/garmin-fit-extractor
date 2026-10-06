use axum::response::IntoResponse;
use garmin_fit_extractor_api::runs::privacy::{Policy, project};
use garmin_fit_extractor_api::runs::stream::OBSERVATION_RULE;
use serde_json::json;

#[test]
fn numeric_identity_and_recursive_schema_keep_metrics_not_canaries() {
    let mut activity = activity(Uuid::new_v4(), [0]);
    activity["normalized"]["summary"]["device"] = json!({"serial":"serial-canary"});
    activity["normalized"]["samples"][0]["unknown"] = json!({"heartRateBpm":"nested-canary"});
    activity["normalized"]["extensions"]
        .as_array_mut()
        .unwrap()
        .push(json!({"value":"extension-canary"}));
    activity["normalized"]["filename"] = json!("private-canary.fit");
    activity["analysis"]["quality"]["unknown"] = json!({"value":"analysis-canary"});
    activity["historicalThresholds"] = json!({"evidenceCutoff":activity["normalized"]["endTime"],"lt1":{"status":"low_confidence","value":{"heartRateBpm":150},"evidence":[{"activityId":"unselected-canary","samples":[{"heartRateBpm":199}]}]}});
    let (safe, omissions) =
        project(&activity, Policy::default()).expect("optional unsafe values can be omitted");
    let record = &safe["decoded"]["messages"][7];
    assert!(
        record["fields"]
            .as_array()
            .unwrap()
            .iter()
            .any(|field| { field["fieldNumber"] == 3 && field["value"] == 100 })
    );
    assert_eq!(safe["normalized"]["summary"]["distanceMeters"], 1000.0);
    assert_eq!(safe["normalized"]["samples"][0]["heartRateBpm"], 100.0);
    assert_eq!(
        safe["historicalThresholds"]["lt1"]["value"]["heartRateBpm"],
        150
    );
    let output = serde_json::to_string(&(safe, omissions)).unwrap();
    for canary in [
        "123456789",
        "444444444",
        "opaque-canary",
        "serial-canary",
        "nested-canary",
        "extension-canary",
        "private-canary",
        "analysis-canary",
        "unselected-canary",
    ] {
        assert!(!output.contains(canary), "leaked {canary}");
    }
}

#[test]
fn location_consent_does_not_enable_native_device_identifiers_or_unknown_fields() {
    let mut activity = activity(Uuid::new_v4(), [0]);
    activity["normalized"]["opaque"] = json!({"value":"unknown-canary"});
    let (safe, omissions) = project(
        &activity,
        Policy {
            include_location: true,
            include_device_identifiers: false,
        },
    )
    .expect("genuine native location remains provable");
    let record = &safe["decoded"]["messages"][7];
    let position = record["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|field| field["fieldNumber"] == 0)
        .unwrap();
    assert_eq!(position["value"], 123456789);
    assert_eq!(
        position["sourceReference"],
        activity["decoded"]["messages"][7]["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|field| field["fieldNumber"] == 0)
            .unwrap()["sourceReference"]
    );
    let output = serde_json::to_string(&(safe, omissions)).unwrap();
    for secret in ["444444444", "opaque-canary", "unknown-canary"] {
        assert!(!output.contains(secret), "leaked {secret}");
    }
}

use garmin_fit_extractor_api::runs::export::{ExportMode, ExportRequest, snapshot};
use serde_json::Value;
use uuid::Uuid;

fn request(id: Uuid, mode: ExportMode) -> ExportRequest {
    ExportRequest {
        activity_ids: vec![id],
        mode,
        include_location: false,
        include_device_identifiers: false,
    }
}

async fn export_test_pool() -> (sqlx::PgPool, sqlx::PgPool, String) {
    use sqlx::{PgPool, postgres::PgPoolOptions};
    let url = std::env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL must point to a disposable PostgreSQL database");
    let parsed = url::Url::parse(&url).expect("parse explicit TEST_DATABASE_URL");
    let database_name = parsed.path().strip_prefix('/').unwrap_or_default();
    assert!(
        database_name.ends_with("_test") || database_name.ends_with("_rehearsal"),
        "TEST_DATABASE_URL database name must end in _test or _rehearsal"
    );
    assert!(
        matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")),
        "TEST_DATABASE_URL must identify a loopback PostgreSQL instance"
    );
    let admin = PgPool::connect(&url).await.unwrap();
    let directory: String = sqlx::query_scalar("SHOW data_directory")
        .fetch_one(&admin)
        .await
        .unwrap();
    let expected =
        std::env::var("PGDATA").expect("PGDATA must identify the disposable PostgreSQL directory");
    assert!(
        std::path::Path::new(&expected).is_absolute() && directory == expected,
        "Refuse a non-disposable database"
    );
    let version: String = sqlx::query_scalar("SHOW server_version_num")
        .fetch_one(&admin)
        .await
        .unwrap();
    assert!((180000..190000).contains(&version.parse::<i32>().unwrap()));
    let schema = format!("runs_export_test_{}", Uuid::new_v4().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
        .execute(&admin)
        .await
        .unwrap();
    let connect_schema = schema.clone();
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .after_connect(move |connection, _| {
            let statement = format!("SET search_path TO {connect_schema}");
            Box::pin(async move {
                sqlx::query(sqlx::AssertSqlSafe(statement))
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    for migration in [
        include_str!("../migrations/0001_extractions.sql"),
        include_str!("../migrations/0002_google_users_sessions_zip_history.sql"),
        include_str!("../migrations/0003_fit_coach.sql"),
        include_str!("../migrations/0004_transcript_entries.sql"),
        include_str!("../migrations/0005_legacy_imports.sql"),
        include_str!("../migrations/0006_runs.sql"),
        include_str!("../migrations/0007_runs_legacy_summary.sql"),
        include_str!("../migrations/0008_runs_physical_slots.sql"),
    ] {
        sqlx::raw_sql(migration).execute(&pool).await.unwrap();
    }
    (pool, admin, schema)
}

fn activity(id: Uuid, sample_indices: impl IntoIterator<Item = usize>) -> Value {
    let mut run = legacy_native_activity(
        include_bytes!("fixtures/runs/native_export_consumers.json"),
        "59aa24242c81d343ceff3562f7d53b7feed56d6ac06dc5145d602b6f83d0a344",
        id,
    );
    let source = run["normalized"]["samples"].as_array().unwrap();
    // Copy real native rows; only structural output indices change for selection/repetition.
    let samples: Vec<Value> = sample_indices
        .into_iter()
        .enumerate()
        .map(|(index, selected)| {
            let mut sample = source[selected].clone();
            sample["index"] = json!(index);
            sample
        })
        .collect();
    run["normalized"]["samples"] = json!(samples);
    run["analysis"] = json!({"schemaVersion":"2.0.0","quality":{"sampleCount":run["normalized"]["samples"].as_array().unwrap().len()},"segments":[],"thresholds":{"lt1":{"status":"low_confidence","method":{"id":"running-dfa-a1-075","version":"real-version"},"value":{"heartRateBpm":150},"evidence":{"activityId":"unselected-canary","independentActivityCount":1},"trace":{"crossing":150,"regression":{"slope":-0.01,"intercept":2.25},"windows":[{"samples":["trace-canary"]}]}}},"transformations":[]});
    run["historicalThresholds"] = garmin_fit_extractor_api::runs::thresholds::estimate_history(
        run["normalized"]["endTime"].as_str().unwrap(),
        &[],
    );
    run
}

#[test]
fn full_snapshot_keeps_all_100000_samples_values_order_and_exact_bytes() {
    let id = Uuid::new_v4();
    let mut original = activity(id, (0..100000).map(|index| index % 8));
    original["analysis"]["thresholds"]["lt1"]["contextUnverified"] =
        json!(["fatigueState", "treadmillCalibration"]);
    original["analysis"]["thresholds"]["lt1"]["trace"]["parameters"] = json!({"lambda":500,"maximumRrContinuityErrorSeconds":0.005,"scales":[4,5,6,7,8,9,10,11,12,13,14,15,16]});
    original["analysis"]["transformations"] = json!([{"type":"derivedWorkloadBlocks","derivedTimelineSorted":true,"originalSamplesChanged":false}]);
    let request = request(id, ExportMode::Full);
    let bytes = snapshot(
        &[id],
        std::slice::from_ref(&original),
        &request,
        "2026-01-01T00:00:00Z",
    )
    .unwrap();
    assert_eq!(bytes.last(), Some(&b'\n'));
    assert_eq!(
        bytes,
        snapshot(
            &[id],
            std::slice::from_ref(&original),
            &request,
            "2026-01-01T00:00:00Z"
        )
        .unwrap()
    );
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    let samples = value["activities"][0]["normalized"]["samples"]
        .as_array()
        .unwrap();
    assert_eq!(samples.len(), 100000);
    for (index, sample) in samples.iter().enumerate() {
        assert_eq!(sample, &original["normalized"]["samples"][index]);
        assert_eq!(sample["index"], index);
    }
    assert_eq!(
        value["activities"][0]["analysis"]["thresholds"]["lt1"]["value"]["heartRateBpm"],
        150
    );
    assert_eq!(
        value["activities"][0]["analysis"]["thresholds"]["lt1"]["trace"]["crossing"],
        150
    );
    assert_eq!(
        value["activities"][0]["analysis"]["thresholds"]["lt1"]["contextUnverified"],
        json!(["fatigueState", "treadmillCalibration"])
    );
    assert_eq!(
        value["activities"][0]["analysis"]["thresholds"]["lt1"]["trace"]["parameters"]["maximumRrContinuityErrorSeconds"],
        0.005
    );
    assert_eq!(
        value["activities"][0]["analysis"]["transformations"][0]["derivedTimelineSorted"],
        true
    );
    let text = String::from_utf8(bytes).unwrap();
    assert!(!text.contains("unselected-canary"));
    assert!(!text.contains("trace-canary"));
}

#[test]
fn explicit_invalid_selection_never_materializes_remaining_activities() {
    let id = Uuid::new_v4();
    let other = Uuid::new_v4();
    let mut selected = request(id, ExportMode::Full);
    selected.activity_ids.push(other);
    assert!(
        snapshot(
            &[id, other],
            &[activity(id, vec![])],
            &selected,
            "2026-01-01T00:00:00Z"
        )
        .is_err()
    );
    selected.activity_ids = vec![id, id];
    assert!(
        snapshot(
            &[id, id],
            &[activity(id, vec![]), activity(id, vec![])],
            &selected,
            "2026-01-01T00:00:00Z"
        )
        .is_err()
    );
    let mut pending = activity(id, vec![]);
    pending["historicalThresholds"] = Value::Null;
    assert_eq!(
        snapshot(
            &[id],
            &[pending],
            &request(id, ExportMode::Full),
            "2026-01-01T00:00:00Z"
        )
        .unwrap_err()
        .code(),
        "EXPORT_NOT_READY"
    );
    assert!(
        snapshot(
            &[id],
            &[activity(other, vec![])],
            &request(id, ExportMode::Full),
            "2026-01-01T00:00:00Z"
        )
        .is_err()
    );
}

#[test]
fn coach_aggregation_preserves_lap_pause_gap_repeats_and_missing_metric_weights() {
    let id = Uuid::new_v4();
    let mut run = activity(id, [0, 1, 2, 3, 4]);
    run["analysis"]["segments"] = json!([{"index":0,"kind":"repeatedsurge","startElapsedSeconds":0,"endElapsedSeconds":4},{"index":1,"kind":"repeatedsurge","startElapsedSeconds":20,"endElapsedSeconds":21}]);
    let bytes = snapshot(
        &[id],
        &[run],
        &request(id, ExportMode::Coach),
        "2026-01-01T00:00:00Z",
    )
    .unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    let samples = value["activities"][0]["samples"].as_array().unwrap();
    assert_eq!(
        samples
            .iter()
            .map(|v| v["sourceCount"].as_u64().unwrap())
            .sum::<u64>(),
        5
    );
    assert!((samples[0]["heartRateBpm"].as_f64().unwrap() - 500.0 / 3.0).abs() < 1e-10);
    assert_eq!(
        samples[0]["metricCoverageSeconds"]["heartRateBpm"].as_f64(),
        Some(3.0)
    );
    // Power coverage: 300 W for two seconds, then 600 W for one second.
    assert_eq!(samples[0]["powerWatts"].as_f64(), Some(400.0));
    assert_eq!(
        samples[0]["metricCoverageSeconds"]["powerWatts"].as_f64(),
        Some(3.0)
    );
    assert_eq!(samples.len(), 3);
    assert_eq!(samples[1]["startElapsedSeconds"].as_f64(), Some(4.0));
    assert_eq!(samples[1]["timerRunning"], false);
    assert_eq!(samples[2]["startElapsedSeconds"].as_f64(), Some(20.0));
    assert_eq!(
        value["activities"][0]["laps"][0]["endElapsedSeconds"].as_f64(),
        Some(4.0)
    );
    assert_eq!(
        value["activities"][0]["segments"][1]["kind"],
        "repeatedsurge"
    );
    assert!(
        !String::from_utf8(bytes)
            .unwrap()
            .contains("unselected-canary")
    );
}

#[test]
fn both_modes_apply_independent_consents_to_nested_canaries() {
    let id = Uuid::new_v4();
    let mut run = activity(id, [0]);
    run["normalized"]["laps"][0]["unknown"] = json!({"value":"lap-canary"});
    run["normalized"]["extensions"]
        .as_array_mut()
        .unwrap()
        .push(json!({"value":"extension-canary"}));
    run["analysis"]["quality"]["unknown"] = json!({"value":"quality-canary"});
    run["historicalThresholds"] = json!({"evidenceCutoff":run["normalized"]["endTime"],"lt1":{"status":"low_confidence","value":{"heartRateBpm":150},"trace":{"crossing":150,"location":{"value":"history-canary"}},"evidence":{"activityId":"unselected-history-canary","sampleCount":1}}});
    for mode in [ExportMode::Coach, ExportMode::Full] {
        for include_location in [false, true] {
            for include_device_identifiers in [false, true] {
                let mut consent = request(id, mode);
                consent.include_location = include_location;
                consent.include_device_identifiers = include_device_identifiers;
                let bytes = snapshot(
                    &[id],
                    std::slice::from_ref(&run),
                    &consent,
                    "2026-01-01T00:00:00Z",
                )
                .unwrap();
                let text = std::str::from_utf8(&bytes).unwrap();
                for secret in [
                    "opaque-canary",
                    "lap-canary",
                    "extension-canary",
                    "quality-canary",
                    "history-canary",
                    "unselected-history-canary",
                ] {
                    assert!(!text.contains(secret), "leaked {secret} in {mode:?}");
                }
                assert_eq!(
                    text.contains("123456789"),
                    mode == ExportMode::Full && include_location
                );
                assert_eq!(
                    text.contains("444444444"),
                    mode == ExportMode::Full && include_device_identifiers
                );
            }
        }
    }
}

#[test]
fn coach_splits_recorded_intervals_at_boundaries_without_discarding_time_coverage() {
    let id = Uuid::new_v4();
    let run = activity(id, [5, 6, 7]);
    let bytes = snapshot(
        &[id],
        &[run],
        &request(id, ExportMode::Coach),
        "2026-01-01T00:00:00Z",
    )
    .unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    let samples = value["activities"][0]["samples"].as_array().unwrap();
    assert_eq!(samples[0]["endElapsedSeconds"].as_f64(), Some(30.0));
    assert_eq!(
        samples[0]["metricCoverageSeconds"]["heartRateBpm"].as_f64(),
        Some(2.0)
    );
    assert_eq!(samples[1]["startElapsedSeconds"].as_f64(), Some(30.0));
    assert_eq!(
        samples[1]["metricCoverageSeconds"]["heartRateBpm"].as_f64(),
        Some(3.0)
    );
    assert!((samples[1]["heartRateBpm"].as_f64().unwrap() - 500.0 / 3.0).abs() < 1e-10);
    assert_eq!(samples[1]["powerWatts"].as_f64(), Some(100.0));
}

#[tokio::test]
async fn pinned_transport_rejects_revocation_releases_drop_guard_and_matches_copy_download_bytes() {
    use futures_util::StreamExt;
    use garmin_fit_extractor_api::runs::export::{create, serve, serve_head};
    use sha2::{Digest, Sha256};
    let (pool, admin, schema) = export_test_pool().await;
    let owner = Uuid::new_v4();
    let id = Uuid::new_v4();
    let source = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,google_subject,email,created_at,updated_at) VALUES($1,$1,'export-test@example.test','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')").bind(owner.to_string()).execute(&pool).await.unwrap();
    let bytes = include_bytes!("fixtures/runs/native_export_consumers.fit");
    let legacy = garmin_fit_extractor_api::fit::normalize::normalize(
        &garmin_fit_extractor_api::fit::raw::decode_raw(bytes).unwrap(),
        "",
    );
    sqlx::query("INSERT INTO runs_sources(id,owner_id,sha256,size_bytes,bytes,integrity) VALUES($1,$2,$3,$4,$5,'verified')").bind(source.to_string()).bind(owner.to_string()).bind(Sha256::digest(bytes).to_vec()).bind(bytes.len() as i64).bind(bytes.as_slice()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO runs_activities(id,owner_id,source_id,session_index,start_time,end_time,start_order,end_order,summary,observation_group_id,observation_fingerprint,desired_versions,processing_status) VALUES($1,$2,$3,0,'2026-01-01T00:00:00Z','2026-01-02T03:46:40Z','2026-01-01T00:00:00Z','2026-01-02T03:46:40Z','{}',$1,$4,'{}','ready')").bind(id.to_string()).bind(owner.to_string()).bind(source.to_string()).bind(vec![1u8;32]).execute(&pool).await.unwrap();
    let mut run = activity(id, (0..100000).map(|index| index % 8));
    run["analysis"]["quality"]["sampleCount"] = json!(100000);
    run["historicalThresholds"] = json!({"evidenceCutoff":"2026-01-02T03:46:40Z","computedAt":"2026-01-02T04:00:00Z","lt1":{"status":"low_confidence","value":{"heartRateBpm":143},"evidence":{"independentActivityCount":1,"activityId":"unselected-canary"}}});
    run["normalized"]["private-canary-name.fit"] =
        json!({"samples":[{"value":"private-opaque-canary"}]});
    let mut revisions = Vec::new();
    for stage in ["decoded", "normalized", "analysis"] {
        let revision = Uuid::new_v4().to_string();
        let legacy_projection = if stage == "normalized" {
            Some(serde_json::to_string(&legacy).unwrap())
        } else {
            None
        };
        let archive = if stage == "analysis" {
            None
        } else {
            Some(serde_json::to_vec(&run[stage]).unwrap())
        };
        let payload = if let Some(bytes) = archive.as_ref() {
            let mut metadata = if stage == "decoded" {
                json!({"schemaVersion":"2.0.0"})
            } else {
                json!({"schemaVersion":"2.0.0","startTime":run[stage]["startTime"],"endTime":run[stage]["endTime"],"summary":run[stage]["summary"],"subtype":run[stage]["subtype"]})
            };
            metadata["documents"] = json!({"archive":{"byteLength":bytes.len(),"sha256":format!("{:x}",Sha256::digest(bytes)),"chunkCount":bytes.len().div_ceil(65536)}});
            metadata
        } else {
            run[stage].clone()
        };
        sqlx::query("INSERT INTO runs_revisions(id,owner_id,activity_id,stage,generation,versions,input_revision_ids,payload,legacy_projection,validation) VALUES($1,$2,$3,$4,1,'{}','[]',$5::jsonb,$6::jsonb,'{}')").bind(&revision).bind(owner.to_string()).bind(id.to_string()).bind(stage).bind(payload.to_string()).bind(legacy_projection).execute(&pool).await.unwrap();
        if let Some(bytes) = archive {
            for (position, chunk) in bytes.chunks(65536).enumerate() {
                sqlx::query("INSERT INTO runs_revision_chunks(revision_id,owner_id,document,position,payload) VALUES($1,$2,'archive',$3,$4)").bind(&revision).bind(owner.to_string()).bind(position as i64).bind(chunk).execute(&pool).await.unwrap();
            }
        }
        revisions.push(revision);
    }
    let manifest = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO runs_manifests(id,owner_id,activity_id,decoded_revision_id,normalized_revision_id,analysis_revision_id,generation,versions) VALUES($1,$2,$3,$4,$5,$6,1,$7::jsonb)").bind(&manifest).bind(owner.to_string()).bind(id.to_string()).bind(&revisions[0]).bind(&revisions[1]).bind(&revisions[2]).bind(json!({"observationRule":OBSERVATION_RULE}).to_string()).execute(&pool).await.unwrap();
    sqlx::query(
        "UPDATE runs_activities SET current_manifest_id=$1,history_generation=2 WHERE id=$2",
    )
    .bind(&manifest)
    .bind(id.to_string())
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        create(&pool, owner, request(id, ExportMode::Full))
            .await
            .unwrap_err()
            .code(),
        "EXPORT_NOT_READY"
    );
    sqlx::query("INSERT INTO runs_estimates(id,owner_id,activity_id,evidence_cutoff,computed_at,generation,manifest_id,payload) VALUES($1,$2,$3,'2026-01-02T03:46:40Z','2026-01-02T04:00:00Z',2,$4,$5::jsonb)").bind(Uuid::new_v4().to_string()).bind(owner.to_string()).bind(id.to_string()).bind(&manifest).bind(run["historicalThresholds"].to_string()).execute(&pool).await.unwrap();
    assert_eq!(
        create(&pool, Uuid::new_v4(), request(id, ExportMode::Full))
            .await
            .unwrap_err()
            .code(),
        "NOT_FOUND"
    );
    assert_eq!(
        create(&pool, owner, request(Uuid::new_v4(), ExportMode::Full))
            .await
            .unwrap_err()
            .code(),
        "NOT_FOUND"
    );
    let mut invalid = request(id, ExportMode::Full);
    invalid.activity_ids.push(Uuid::new_v4());
    assert_eq!(
        create(&pool, owner, invalid).await.unwrap_err().code(),
        "NOT_FOUND"
    );
    let mut duplicate = request(id, ExportMode::Full);
    duplicate.activity_ids.push(id);
    assert_eq!(
        create(&pool, owner, duplicate).await.unwrap_err().code(),
        "INVALID_EXPORT_SELECTION"
    );
    let mut empty = request(id, ExportMode::Full);
    empty.activity_ids.clear();
    assert_eq!(
        create(&pool, owner, empty).await.unwrap_err().code(),
        "INVALID_EXPORT_SELECTION"
    );
    let legacy = Uuid::new_v4();
    sqlx::query("INSERT INTO extractions(id,user_id,file_name,file_size_bytes,status,error_code,error_message,created_at) VALUES($1,$2,'legacy-private-canary.fit',1,'failed','INVALID_FIT','Unsupported fixture','2026-01-01T00:00:00Z')").bind(legacy.to_string()).bind(owner.to_string()).execute(&pool).await.unwrap();
    assert_eq!(
        create(&pool, owner, request(legacy, ExportMode::Full))
            .await
            .unwrap_err()
            .code(),
        "LEGACY_EXPORT_UNSUPPORTED"
    );
    let mut mixed = request(legacy, ExportMode::Full);
    mixed.activity_ids.push(Uuid::new_v4());
    assert_eq!(
        create(&pool, owner, mixed).await.unwrap_err().code(),
        "NOT_FOUND"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM runs_exports WHERE owner_id=$1")
            .bind(owner.to_string())
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    let created = create(&pool, owner, request(id, ExportMode::Full))
        .await
        .unwrap();
    let token = created["token"].as_str().unwrap();
    assert!(serve(&pool, Uuid::new_v4(), token).await.is_err());
    assert!(serve_head(&pool, Uuid::new_v4(), token).await.is_err());
    let head = serve_head(&pool, owner, token).await.unwrap();
    assert_eq!(
        head.headers()["content-length"],
        created["byteLength"].as_u64().unwrap().to_string()
    );
    assert_eq!(
        head.headers()["content-disposition"],
        "attachment; filename=\"runs-full.json\""
    );
    assert!(
        axum::body::to_bytes(head.into_body(), 0)
            .await
            .unwrap()
            .is_empty()
    );
    let generated =
        chrono::DateTime::parse_from_rfc3339(created["generatedAt"].as_str().unwrap()).unwrap();
    let expires =
        chrono::DateTime::parse_from_rfc3339(created["expiresAt"].as_str().unwrap()).unwrap();
    assert_eq!((expires - generated).num_seconds(), 900);
    let response = serve(&pool, owner, token).await.unwrap();
    assert_eq!(response.headers()["cache-control"], "private, no-store");
    let copy = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let download = axum::body::to_bytes(
        serve(&pool, owner, token).await.unwrap().into_body(),
        usize::MAX,
    )
    .await
    .unwrap();
    assert_eq!(copy, download);
    assert_eq!(copy.len() as u64, created["byteLength"].as_u64().unwrap());
    let expected = snapshot(
        &[id],
        std::slice::from_ref(&run),
        &request(id, ExportMode::Full),
        created["generatedAt"].as_str().unwrap(),
    )
    .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&copy).unwrap(),
        serde_json::from_slice::<Value>(&expected).unwrap()
    );
    assert_eq!(copy.last(), Some(&b'\n'));
    let full: Value = serde_json::from_slice(&copy).unwrap();
    assert_eq!(
        copy.as_ref(),
        serde_json::to_vec_pretty(&full)
            .unwrap()
            .into_iter()
            .chain([b'\n'])
            .collect::<Vec<_>>()
            .as_slice()
    );
    assert_eq!(full["privacyOmissions"], created["privacyOmissions"]);
    let text = std::str::from_utf8(&copy).unwrap();
    for secret in [
        "123456789",
        "private-canary-name",
        "private-opaque-canary",
        "unselected-canary",
        "trace-canary",
    ] {
        assert!(!text.contains(secret));
    }
    let coach = create(&pool, owner, request(id, ExportMode::Coach))
        .await
        .unwrap();
    let coach_bytes = axum::body::to_bytes(
        serve(&pool, owner, coach["token"].as_str().unwrap())
            .await
            .unwrap()
            .into_body(),
        usize::MAX,
    )
    .await
    .unwrap();
    let coach_value: Value = serde_json::from_slice(&coach_bytes).unwrap();
    let expected: Value = serde_json::from_slice(
        &snapshot(
            &[id],
            &[run],
            &request(id, ExportMode::Coach),
            coach["generatedAt"].as_str().unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(coach_value["activities"], expected["activities"]);
    assert_eq!(coach_value["privacyOmissions"], coach["privacyOmissions"]);
    let document: Value = serde_json::from_slice(&copy).unwrap();
    assert_eq!(
        document["activities"][0]["historicalThresholds"]["lt1"]["value"]["heartRateBpm"],
        143
    );
    // Corrupt backing storage must fail the body, never masquerade as clean EOF.
    let damaged = create(&pool, owner, request(id, ExportMode::Full))
        .await
        .unwrap();
    sqlx::query("DELETE FROM runs_export_chunks WHERE token=$1 AND position=1")
        .bind(damaged["token"].as_str().unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let mut damaged_body = serve(&pool, owner, damaged["token"].as_str().unwrap())
        .await
        .unwrap()
        .into_body()
        .into_data_stream();
    assert_eq!(damaged_body.next().await.unwrap().unwrap().len(), 65536);
    assert!(damaged_body.next().await.unwrap().is_err());
    drop(damaged_body);
    // TTL is checked after headers and on every chunk, not frozen at transaction start.
    let expiring = create(&pool, owner, request(id, ExportMode::Full))
        .await
        .unwrap();
    let mut expiring_body = serve(&pool, owner, expiring["token"].as_str().unwrap())
        .await
        .unwrap()
        .into_body()
        .into_data_stream();
    assert!(expiring_body.next().await.unwrap().is_ok());
    sqlx::query(
        "UPDATE runs_exports SET expires_at=clock_timestamp()-interval '1 second' WHERE token=$1",
    )
    .bind(expiring["token"].as_str().unwrap())
    .execute(&pool)
    .await
    .unwrap();
    assert!(expiring_body.next().await.unwrap().is_err());
    drop(expiring_body);
    // PostgreSQL must release guards even if a client never polls the body again.
    let idle = create(&pool, owner, request(id, ExportMode::Full))
        .await
        .unwrap();
    sqlx::query(
        "UPDATE runs_exports SET expires_at=clock_timestamp()+interval '1 second' WHERE token=$1",
    )
    .bind(idle["token"].as_str().unwrap())
    .execute(&pool)
    .await
    .unwrap();
    let unpolled = serve(&pool, owner, idle["token"].as_str().unwrap())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(8)).await;
    let mut guard = pool.begin().await.unwrap();
    tokio::time::timeout(
        std::time::Duration::from_secs(3),
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
            .bind(owner.to_string())
            .execute(&mut *guard),
    )
    .await
    .unwrap()
    .unwrap();
    guard.rollback().await.unwrap();
    let mut expired_body = unpolled.into_body().into_data_stream();
    assert!(expired_body.next().await.unwrap().is_err());
    drop(expired_body);
    let expired = create(&pool, owner, request(id, ExportMode::Full))
        .await
        .unwrap();
    sqlx::query("UPDATE runs_exports SET expires_at=now()-interval '1 second' WHERE token=$1")
        .bind(expired["token"].as_str().unwrap())
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        serve(&pool, owner, expired["token"].as_str().unwrap())
            .await
            .is_err()
    );
    assert!(
        serve_head(&pool, owner, expired["token"].as_str().unwrap())
            .await
            .is_err()
    );
    sqlx::query("UPDATE runs_activities SET current_manifest_id=NULL,processing_status='processing' WHERE id=$1").bind(id.to_string()).execute(&pool).await.unwrap();
    assert!(
        create(&pool, owner, request(id, ExportMode::Full))
            .await
            .is_err()
    );
    let pinned = axum::body::to_bytes(
        serve(&pool, owner, token).await.unwrap().into_body(),
        usize::MAX,
    )
    .await
    .unwrap();
    assert_eq!(pinned, copy);
    // Body drop must release both the transaction and owner deletion guard.
    let dropped = serve(&pool, owner, token).await.unwrap();
    drop(dropped);
    let mut guard = pool.begin().await.unwrap();
    tokio::time::timeout(
        std::time::Duration::from_secs(3),
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
            .bind(owner.to_string())
            .execute(&mut *guard),
    )
    .await
    .unwrap()
    .unwrap();
    guard.rollback().await.unwrap();
    // Deterministic midstream barrier: first chunk is read before revocation commits.
    let mut stream = serve(&pool, owner, token)
        .await
        .unwrap()
        .into_body()
        .into_data_stream();
    let first = stream.next().await.unwrap().unwrap();
    assert!(first.len() < copy.len());
    sqlx::query("UPDATE runs_exports SET revoked=true WHERE token=$1")
        .bind(token)
        .execute(&pool)
        .await
        .unwrap();
    assert!(serve_head(&pool, owner, token).await.is_err());
    let delete_pool = pool.clone();
    let deletion = tokio::spawn(async move {
        let mut transaction = delete_pool.begin().await.unwrap();
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
            .bind(owner.to_string())
            .execute(&mut *transaction)
            .await
            .unwrap();
        sqlx::query("DELETE FROM runs_exports WHERE owner_id=$1 AND $2=ANY(activity_ids)")
            .bind(owner.to_string())
            .bind(id.to_string())
            .execute(&mut *transaction)
            .await
            .unwrap();
        sqlx::query("DELETE FROM runs_activities WHERE id=$1 AND owner_id=$2")
            .bind(id.to_string())
            .bind(owner.to_string())
            .execute(&mut *transaction)
            .await
            .unwrap();
        transaction.commit().await.unwrap();
    });
    assert!(stream.next().await.unwrap().is_err());
    drop(stream);
    tokio::time::timeout(std::time::Duration::from_secs(3), deletion)
        .await
        .unwrap()
        .unwrap();
    assert!(serve(&pool, owner, token).await.is_err());
    pool.close().await;
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE")))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}

#[tokio::test]
async fn deleting_during_first_snapshot_chunk_returns_unavailable_and_rolls_back_publication() {
    use axum::{
        body::Body,
        http::{Request, StatusCode, header::CONTENT_DISPOSITION},
    };
    use garmin_fit_extractor_api::{
        app::{AppState, router},
        auth::{AuthState, hash_token},
    };
    use sha2::{Digest, Sha256};
    use std::{path::PathBuf, sync::Arc, time::Duration};
    use tower::ServiceExt;

    let (pool, admin, schema) = export_test_pool().await;
    let owner = Uuid::new_v4();
    let id = Uuid::new_v4();
    let source = Uuid::new_v4();
    let session = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO users(id,google_subject,email,created_at,updated_at) VALUES($1,$1,'cancel-export-test@example.test','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')")
        .bind(owner.to_string()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO sessions(token_hash,user_id,created_at,expires_at) VALUES($1,$2,'2026-01-01T00:00:00Z','2099-01-01T00:00:00Z')")
        .bind(hash_token(&session)).bind(owner.to_string()).execute(&pool).await.unwrap();
    let input = include_bytes!("fixtures/runs/native_export_consumers.fit");
    let run = activity(id, (0..2048).map(|index| index % 8));
    let legacy = garmin_fit_extractor_api::fit::normalize::normalize(
        &garmin_fit_extractor_api::fit::raw::decode_raw(input).unwrap(),
        "",
    );
    sqlx::query("INSERT INTO runs_sources(id,owner_id,sha256,size_bytes,bytes,integrity) VALUES($1,$2,$3,$4,$5,'verified')")
        .bind(source.to_string()).bind(owner.to_string()).bind(Sha256::digest(input).to_vec())
        .bind(input.len() as i64).bind(input.as_slice()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO runs_activities(id,owner_id,source_id,session_index,start_time,end_time,start_order,end_order,summary,observation_group_id,observation_fingerprint,desired_versions,processing_status) VALUES($1,$2,$3,0,$4,$5,$4::timestamptz,$5::timestamptz,$6::jsonb,$1,$7,'{}','ready')")
        .bind(id.to_string()).bind(owner.to_string()).bind(source.to_string())
        .bind(run["normalized"]["startTime"].as_str().unwrap())
        .bind(run["normalized"]["endTime"].as_str().unwrap())
        .bind(run["normalized"]["summary"].to_string()).bind(Sha256::digest(input).to_vec())
        .execute(&pool).await.unwrap();
    let mut revisions = Vec::new();
    for stage in ["decoded", "normalized", "analysis"] {
        let revision = Uuid::new_v4().to_string();
        let archive = (stage != "analysis").then(|| serde_json::to_vec(&run[stage]).unwrap());
        let payload = if let Some(document) = archive.as_ref() {
            let mut metadata = if stage == "decoded" {
                json!({"schemaVersion":"2.0.0"})
            } else {
                json!({"schemaVersion":"2.0.0","startTime":run[stage]["startTime"],"endTime":run[stage]["endTime"],"summary":run[stage]["summary"],"subtype":run[stage]["subtype"]})
            };
            metadata["documents"] = json!({"archive":{"byteLength":document.len(),"sha256":format!("{:x}",Sha256::digest(document)),"chunkCount":document.len().div_ceil(65536)}});
            metadata
        } else {
            run[stage].clone()
        };
        sqlx::query("INSERT INTO runs_revisions(id,owner_id,activity_id,stage,generation,versions,input_revision_ids,payload,legacy_projection,validation) VALUES($1,$2,$3,$4,1,'{}','[]',$5::jsonb,$6::jsonb,'{}')")
            .bind(&revision).bind(owner.to_string()).bind(id.to_string()).bind(stage)
            .bind(payload.to_string()).bind((stage == "normalized").then(|| serde_json::to_string(&legacy).unwrap()))
            .execute(&pool).await.unwrap();
        if let Some(document) = archive {
            for (position, chunk) in document.chunks(65536).enumerate() {
                sqlx::query("INSERT INTO runs_revision_chunks(revision_id,owner_id,document,position,payload) VALUES($1,$2,'archive',$3,$4)")
                    .bind(&revision).bind(owner.to_string()).bind(position as i64).bind(chunk)
                    .execute(&pool).await.unwrap();
            }
        }
        revisions.push(revision);
    }
    let manifest = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO runs_manifests(id,owner_id,activity_id,decoded_revision_id,normalized_revision_id,analysis_revision_id,generation,versions) VALUES($1,$2,$3,$4,$5,$6,1,$7::jsonb)")
        .bind(&manifest).bind(owner.to_string()).bind(id.to_string())
        .bind(&revisions[0]).bind(&revisions[1]).bind(&revisions[2])
        .bind(json!({"observationRule":OBSERVATION_RULE}).to_string()).execute(&pool).await.unwrap();
    sqlx::query("UPDATE runs_activities SET current_manifest_id=$1 WHERE id=$2")
        .bind(&manifest)
        .bind(id.to_string())
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO runs_estimates(id,owner_id,activity_id,evidence_cutoff,computed_at,generation,manifest_id,payload) VALUES($1,$2,$3,$4,'2026-01-01T00:00:00Z',1,$5,$6::jsonb)")
        .bind(Uuid::new_v4().to_string()).bind(owner.to_string()).bind(id.to_string())
        .bind(run["normalized"]["endTime"].as_str().unwrap()).bind(&manifest)
        .bind(run["historicalThresholds"].to_string()).execute(&pool).await.unwrap();
    let app = router(
        AppState {
            db: pool.clone(),
            auth: Arc::new(AuthState::new(None, None)),
            app_origin: None,
        },
        PathBuf::from("nonexistent-test-static"),
    );
    let selection = json!({"activityIds":[id],"mode":"full","includeLocation":false,"includeDeviceIdentifiers":false});

    // Without a tombstone, the native archive OutputWrite failure retains its storage error code.
    sqlx::raw_sql("CREATE FUNCTION first_snapshot_chunk() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'test export storage failure'; END $$; CREATE TRIGGER first_snapshot_chunk BEFORE INSERT ON runs_export_chunks FOR EACH ROW EXECUTE FUNCTION first_snapshot_chunk()")
        .execute(&pool).await.unwrap();
    let failed = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v2/runs/exports")
                .header("cookie", format!("garmin_fit_session={session}"))
                .header("content-type", "application/json")
                .body(Body::from(selection.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(failed.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let failed_body: Value = serde_json::from_slice(
        &axum::body::to_bytes(failed.into_body(), 1024 * 1024)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(failed_body["error"]["code"], "RUNS_STORAGE_FAILED");

    // Block the real first materialized INSERT, not a mocked serializer callback.
    sqlx::raw_sql("CREATE OR REPLACE FUNCTION first_snapshot_chunk() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.position=0 THEN PERFORM set_config('application_name',NEW.token,false); PERFORM pg_advisory_xact_lock(hashtextextended(TG_TABLE_SCHEMA||':first-snapshot-chunk',0)); END IF; RETURN NEW; END $$")
        .execute(&pool).await.unwrap();
    let mut barrier = pool.begin().await.unwrap();
    let barrier_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *barrier)
        .await
        .unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("{schema}:first-snapshot-chunk"))
        .execute(&mut *barrier)
        .await
        .unwrap();
    let creation_app = app.clone();
    let creation_session = session.clone();
    let creation = tokio::spawn(async move {
        creation_app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v2/runs/exports")
                    .header("cookie", format!("garmin_fit_session={creation_session}"))
                    .header("content-type", "application/json")
                    .body(Body::from(selection.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap()
    });
    let (creator_pid, token) = tokio::time::timeout(Duration::from_secs(3), async {
        let mut poll = tokio::time::interval(Duration::from_millis(10));
        loop {
            poll.tick().await;
            if let Some(blocked) = sqlx::query_as::<_, (i32, String)>("SELECT pid,application_name FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)) AND query LIKE 'INSERT INTO runs_export_chunks%' AND wait_event='advisory'")
                .bind(barrier_pid).fetch_optional(&admin).await.unwrap()
            {
                break blocked;
            }
        }
    }).await.expect("first snapshot chunk must reach the INSERT barrier");
    Uuid::parse_str(&token).expect("capture the actual uncommitted export token");
    let deletion_app = app.clone();
    let deletion_session = session.clone();
    let deletion = tokio::spawn(async move {
        deletion_app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/v2/runs/{id}"))
                    .header("cookie", format!("garmin_fit_session={deletion_session}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    });
    tokio::time::timeout(Duration::from_secs(3), async {
        let mut poll = tokio::time::interval(Duration::from_millis(10));
        let mut tombstone_observed = None;
        loop {
            poll.tick().await;
            let committed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM runs_tombstones WHERE owner_id=$1 AND activity_id=$2)")
                .bind(owner.to_string()).bind(id.to_string()).fetch_one(&pool).await.unwrap();
            let waiting: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)) AND wait_event='advisory')")
                .bind(creator_pid).fetch_one(&admin).await.unwrap();
            if committed && waiting {
                // Hold the chunk across three native 100ms cancellation-monitor
                // intervals after observing the committed tombstone and DELETE wait.
                let observed = tombstone_observed.get_or_insert_with(tokio::time::Instant::now);
                if observed.elapsed() >= Duration::from_millis(300) {
                    break;
                }
            }
        }
    }).await.expect("DELETE must publish cancellation before waiting for the creator");
    assert!(
        !creation.is_finished(),
        "creation must still be blocked at its first chunk"
    );
    assert!(
        !deletion.is_finished(),
        "DELETE must still wait for the creator owner lock"
    );
    barrier.rollback().await.unwrap();

    let created = tokio::time::timeout(Duration::from_secs(3), creation)
        .await
        .unwrap()
        .unwrap();
    let deleted = tokio::time::timeout(Duration::from_secs(3), deletion)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(created.status(), StatusCode::NOT_FOUND);
    assert!(!created.headers().contains_key(CONTENT_DISPOSITION));
    let created_body: Value = serde_json::from_slice(
        &axum::body::to_bytes(created.into_body(), 1024 * 1024)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(created_body["error"]["code"], "EXPORT_UNAVAILABLE");
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
    let survivors: (i64, i64, i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM runs_exports WHERE owner_id=$1),(SELECT count(*) FROM runs_export_chunks),(SELECT count(*) FROM runs_activities WHERE owner_id=$1),(SELECT count(*) FROM runs_sources WHERE owner_id=$1)")
        .bind(owner.to_string()).fetch_one(&pool).await.unwrap();
    assert_eq!(survivors, (0, 0, 0, 0));
    let download = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v2/runs/exports/{token}"))
                .header("cookie", format!("garmin_fit_session={session}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(download.status(), StatusCode::NOT_FOUND);
    assert!(!download.headers().contains_key(CONTENT_DISPOSITION));
    let mut drained = pool.begin().await.unwrap();
    tokio::time::timeout(
        Duration::from_secs(3),
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
            .bind(owner.to_string())
            .execute(&mut *drained),
    )
    .await
    .expect("creation and DELETE must release the owner guard")
    .unwrap();
    drained.rollback().await.unwrap();
    pool.close().await;
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE")))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}

async fn assert_native_history_status(app: &axum::Router, session: &str, id: Uuid, status: &str) {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    for uri in [
        "/api/v2/runs?limit=100&order=asc".to_owned(),
        format!("/api/v2/runs/{id}"),
    ] {
        let response = Box::pin(
            app.clone().oneshot(
                Request::builder()
                    .uri(uri)
                    .header("cookie", format!("garmin_fit_session={session}"))
                    .body(Body::empty())
                    .unwrap(),
            ),
        )
        .await
        .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let value: Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 1024 * 1024)
                .await
                .unwrap(),
        )
        .unwrap();
        let activity = if let Some(items) = value["items"].as_array() {
            items
                .iter()
                .find(|item| item["id"] == id.to_string())
                .unwrap()
        } else {
            if status != "ready" {
                assert_eq!(value["historicalThresholds"], Value::Null);
            }
            &value
        };
        assert_eq!(activity["processing"]["status"], "ready");
        assert_eq!(activity["processing"]["stale"], false);
        assert_eq!(activity["processing"]["historyStatus"], status);
        assert_eq!(activity["processing"]["historyStale"], status != "ready");
    }
}

#[tokio::test]
async fn stored_native_exports_reject_old_observation_readiness_preserve_pins_and_source_proof() {
    use axum::http::StatusCode;
    use garmin_fit_extractor_api::runs::export::{create, serve, serve_head};
    use garmin_fit_extractor_api::{
        app::{AppState, router},
        auth::{AuthState, hash_token},
    };
    use sha2::{Digest, Sha256};
    use std::{path::PathBuf, sync::Arc};
    let (pool, admin, schema) = export_test_pool().await;
    let owner = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,google_subject,email,created_at,updated_at) VALUES($1,$1,'legacy-export-test@example.test','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')")
        .bind(owner.to_string()).execute(&pool).await.unwrap();
    let session = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO sessions(token_hash,user_id,created_at,expires_at) VALUES($1,$2,'2026-01-01T00:00:00Z','2099-01-01T00:00:00Z')")
        .bind(hash_token(&session)).bind(owner.to_string()).execute(&pool).await.unwrap();
    let app = router(
        AppState {
            db: pool.clone(),
            auth: Arc::new(AuthState::new(None, None)),
            app_origin: None,
        },
        PathBuf::from("nonexistent-test-static"),
    );
    for (archive, input, output_sha, bad) in [
        (
            include_bytes!("fixtures/runs/legacy_native_wrong_wire.json").as_slice(),
            include_bytes!("fixtures/runs/legacy_native_wrong_wire.fit").as_slice(),
            "5e1763ba952ffad2018fbc8546ce1e6ac956a267485983121e5a2532a1f78da1",
            true,
        ),
        (
            include_bytes!("fixtures/runs/legacy_native_good_wire.json").as_slice(),
            include_bytes!("fixtures/runs/legacy_native_good_wire.fit").as_slice(),
            "1e889b74ceaf3f89c088e655bf39c77be7cbc1d93d41e77b6c8c22a2bce9e18e",
            false,
        ),
    ] {
        let id = Uuid::new_v4();
        let source = Uuid::new_v4();
        let run = legacy_native_activity(archive, output_sha, id);
        assert_eq!(
            run["decoded"]["decoder"]["sourceSha256"],
            format!("{:x}", Sha256::digest(input))
        );
        assert_eq!(run["decoded"]["decoder"]["sourceByteLength"], input.len());
        // These checksum-pinned, compact fixture documents have this producer root layout.
        // Split exact byte spans; do not serialize, reprocess, or rewrite producer receipts.
        let body = std::str::from_utf8(archive)
            .unwrap()
            .strip_prefix("{\"decoded\":")
            .unwrap();
        let (decoded, body) = body.split_once(",\"normalized\":").unwrap();
        let (normalized, legacy) = body.split_once(",\"legacy\":").unwrap();
        let legacy = legacy.strip_suffix('}').unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(decoded).unwrap(),
            run["decoded"]
        );
        assert_eq!(
            serde_json::from_str::<Value>(normalized).unwrap(),
            run["normalized"]
        );
        sqlx::query("INSERT INTO runs_sources(id,owner_id,sha256,size_bytes,bytes,integrity) VALUES($1,$2,$3,$4,$5,'verified')")
            .bind(source.to_string()).bind(owner.to_string())
            .bind(Sha256::digest(input).to_vec()).bind(input.len() as i64).bind(input)
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO runs_activities(id,owner_id,source_id,session_index,start_time,end_time,start_order,end_order,summary,observation_group_id,observation_fingerprint,desired_versions,processing_status) VALUES($1,$2,$3,0,$4,$5,$4::timestamptz,$5::timestamptz,$6::jsonb,$1,$7,'{}','ready')")
            .bind(id.to_string()).bind(owner.to_string()).bind(source.to_string())
            .bind(run["normalized"]["startTime"].as_str().unwrap())
            .bind(run["normalized"]["endTime"].as_str().unwrap())
            .bind(run["normalized"]["summary"].to_string())
            .bind(Sha256::digest(input).to_vec()).execute(&pool).await.unwrap();
        let mut revisions = Vec::new();
        for (stage, document) in [
            ("decoded", Some(decoded)),
            ("normalized", Some(normalized)),
            ("analysis", None),
        ] {
            let revision = Uuid::new_v4().to_string();
            let payload = if let Some(document) = document {
                let mut metadata = if stage == "decoded" {
                    json!({"schemaVersion":"2.0.0"})
                } else {
                    json!({"schemaVersion":"2.0.0","startTime":run["normalized"]["startTime"],"endTime":run["normalized"]["endTime"],"summary":run["normalized"]["summary"],"subtype":run["normalized"]["subtype"]})
                };
                metadata["documents"] = json!({"archive":{"byteLength":document.len(),"sha256":format!("{:x}",Sha256::digest(document.as_bytes())),"chunkCount":document.len().div_ceil(65536)}});
                metadata
            } else {
                run["analysis"].clone()
            };
            sqlx::query("INSERT INTO runs_revisions(id,owner_id,activity_id,stage,generation,versions,input_revision_ids,payload,legacy_projection,validation) VALUES($1,$2,$3,$4,1,'{}','[]',$5::jsonb,$6::jsonb,'{}')")
                .bind(&revision).bind(owner.to_string()).bind(id.to_string()).bind(stage)
                .bind(payload.to_string()).bind((stage == "normalized").then_some(legacy))
                .execute(&pool).await.unwrap();
            if let Some(document) = document {
                for (position, chunk) in document.as_bytes().chunks(65536).enumerate() {
                    sqlx::query("INSERT INTO runs_revision_chunks(revision_id,owner_id,document,position,payload) VALUES($1,$2,'archive',$3,$4)")
                        .bind(&revision).bind(owner.to_string()).bind(position as i64).bind(chunk)
                        .execute(&pool).await.unwrap();
                }
            }
            revisions.push(revision);
        }
        let manifest = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO runs_manifests(id,owner_id,activity_id,decoded_revision_id,normalized_revision_id,analysis_revision_id,generation,versions) VALUES($1,$2,$3,$4,$5,$6,1,$7::jsonb)")
            .bind(&manifest).bind(owner.to_string()).bind(id.to_string())
            .bind(&revisions[0]).bind(&revisions[1]).bind(&revisions[2])
            .bind(json!({"observationRule":"exact-stream-v2"}).to_string())
            .execute(&pool).await.unwrap();
        sqlx::query(
            "UPDATE runs_activities SET current_manifest_id=$1,history_generation=1 WHERE id=$2",
        )
        .bind(&manifest)
        .bind(id.to_string())
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO runs_estimates(id,owner_id,activity_id,evidence_cutoff,computed_at,generation,manifest_id,payload) VALUES($1,$2,$3,$4,'2026-01-01T00:00:00Z',1,$5,$6::jsonb)")
            .bind(Uuid::new_v4().to_string()).bind(owner.to_string()).bind(id.to_string())
            .bind(run["normalized"]["endTime"].as_str().unwrap()).bind(&manifest)
            .bind(run["historicalThresholds"].to_string()).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO runs_jobs(id,owner_id,activity_id,stage,desired_generation,status,input_revision_ids,versions) VALUES($1,$2,$3,'history',1,'failed','[]',$4::jsonb)")
            .bind(Uuid::new_v4().to_string()).bind(owner.to_string()).bind(id.to_string())
            .bind(json!({"observationRule":"exact-stream-v2"}).to_string()).execute(&pool).await.unwrap();
        // Recreate an already-pinned snapshot of the actual archived native producer.
        // Its old observation rule must block new publication, not immutable downloads.
        let old_bytes = snapshot(
            &[id],
            std::slice::from_ref(&run),
            &request(id, ExportMode::Full),
            "2026-01-01T00:00:00Z",
        )
        .unwrap();
        let old_token = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO runs_exports(token,owner_id,activity_ids,manifest_ids,byte_length,mode,meta,generated_at,expires_at) VALUES($1,$2,$3,$4,$5,'full',$6::jsonb,'2026-01-01T00:00:00Z',clock_timestamp()+interval '15 minutes')")
            .bind(&old_token).bind(owner.to_string()).bind(vec![id.to_string()])
            .bind(vec![manifest.clone()]).bind(old_bytes.len() as i64)
            .bind(json!({"policyVersion":garmin_fit_extractor_api::runs::privacy::POLICY_VERSION}).to_string())
            .execute(&pool).await.unwrap();
        for (position, chunk) in old_bytes.chunks(65536).enumerate() {
            sqlx::query("INSERT INTO runs_export_chunks(token,position,payload) VALUES($1,$2,$3)")
                .bind(&old_token)
                .bind(position as i64)
                .bind(chunk)
                .execute(&pool)
                .await
                .unwrap();
        }
        for mode in [ExportMode::Full, ExportMode::Coach] {
            assert_eq!(
                create(&pool, owner, request(id, mode))
                    .await
                    .unwrap_err()
                    .code(),
                "EXPORT_NOT_READY"
            );
        }
        // Obsolete failure must not stop browser polling before the bounded version scan.
        assert_native_history_status(&app, &session, id, "pending").await;
        assert_eq!(
            serve_head(&pool, owner, &old_token).await.unwrap().status(),
            StatusCode::OK
        );
        assert_eq!(
            axum::body::to_bytes(
                serve(&pool, owner, &old_token).await.unwrap().into_body(),
                usize::MAX
            )
            .await
            .unwrap(),
            old_bytes,
        );
        // Append a compatible manifest; never rewrite the old manifest or producer documents.
        let current = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO runs_manifests(id,owner_id,activity_id,decoded_revision_id,normalized_revision_id,analysis_revision_id,generation,versions) VALUES($1,$2,$3,$4,$5,$6,2,$7::jsonb)")
            .bind(&current).bind(owner.to_string()).bind(id.to_string())
            .bind(&revisions[0]).bind(&revisions[1]).bind(&revisions[2])
            .bind(json!({"observationRule":OBSERVATION_RULE}).to_string())
            .execute(&pool).await.unwrap();
        sqlx::query("UPDATE runs_activities SET current_manifest_id=$1,desired_generation=2,history_generation=2 WHERE id=$2")
            .bind(&current).bind(id.to_string()).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO runs_jobs(id,owner_id,activity_id,stage,desired_generation,status,input_revision_ids,versions) VALUES($1,$2,$3,'history',2,'failed','[]',$4::jsonb)")
            .bind(Uuid::new_v4().to_string()).bind(owner.to_string()).bind(id.to_string())
            .bind(json!({"observationRule":OBSERVATION_RULE}).to_string()).execute(&pool).await.unwrap();
        // A genuine failure of compatible history remains visible until a result is ready.
        assert_native_history_status(&app, &session, id, "failed").await;
        sqlx::query("INSERT INTO runs_estimates(id,owner_id,activity_id,evidence_cutoff,computed_at,generation,manifest_id,payload) VALUES($1,$2,$3,$4,'2026-01-01T00:00:00Z',2,$5,$6::jsonb)")
            .bind(Uuid::new_v4().to_string()).bind(owner.to_string()).bind(id.to_string())
            .bind(run["normalized"]["endTime"].as_str().unwrap()).bind(&current)
            .bind(run["historicalThresholds"].to_string()).execute(&pool).await.unwrap();
        for include_location in [false, true] {
            for include_device_identifiers in [false, true] {
                for mode in [ExportMode::Full, ExportMode::Coach] {
                    let mut selection = request(id, mode);
                    selection.include_location = include_location;
                    selection.include_device_identifiers = include_device_identifiers;
                    let created = create(&pool, owner, selection)
                        .await
                        .expect("compatible manifests must retain old native source proofs");
                    let token = created["token"].as_str().unwrap();
                    assert_eq!(
                        serve_head(&pool, owner, token).await.unwrap().status(),
                        axum::http::StatusCode::OK
                    );
                    let response = serve(&pool, owner, token).await.unwrap();
                    assert_eq!(response.status(), axum::http::StatusCode::OK);
                    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                        .await
                        .unwrap();
                    let text = std::str::from_utf8(&bytes).unwrap();
                    if bad {
                        for secret in ["239.125", "231.125", "735019"] {
                            assert!(
                                !text.contains(secret),
                                "{mode:?} leaked stored bad native value {secret}"
                            );
                        }
                    }
                    let exported: Value = serde_json::from_slice(&bytes).unwrap();
                    let consumer = &exported["activities"][0];
                    let summary = if mode == ExportMode::Full {
                        &consumer["normalized"]["summary"]
                    } else {
                        &consumer["summary"]
                    };
                    assert_eq!(summary["recorded"]["distanceMeters"], 1000.0);
                    assert_eq!(
                        summary["sourceReferences"]["distanceMeters"],
                        run["normalized"]["summary"]["sourceReferences"]["distanceMeters"]
                    );
                    if bad {
                        assert!(summary["recorded"].get("averageHeartRateBpm").is_none());
                        assert!(summary["derived"].get("averagePowerWatts").is_none());
                    } else {
                        assert_eq!(summary, &run["normalized"]["summary"]);
                    }
                    if mode == ExportMode::Full {
                        let normalized = &consumer["normalized"];
                        assert_eq!(normalized["samples"][0], run["normalized"]["samples"][0]);
                        assert_eq!(normalized["rr"], run["normalized"]["rr"]);
                        if bad {
                            assert!(normalized["samples"][4].get("heartRateBpm").is_none());
                            assert!(
                                normalized["samples"][4]["sourceReferences"]
                                    .get("heartRateBpm")
                                    .is_none()
                            );
                            assert!(normalized["samples"][4].get("powerWatts").is_none());
                            assert!(
                                normalized["samples"][4]["sourceReferences"]
                                    .get("powerWatts")
                                    .is_none()
                            );
                            assert_eq!(
                                normalized["samples"][4]["sourceReferences"]["speedMps"],
                                run["normalized"]["samples"][4]["sourceReferences"]["speedMps"]
                            );
                            assert!(
                                normalized["session"]["sourceLocalTimeContexts"][1]
                                    .get("timeOffsetsSeconds")
                                    .is_none()
                            );
                        } else {
                            assert_eq!(normalized["samples"], run["normalized"]["samples"]);
                            assert_eq!(
                                normalized["session"]["sourceLocalTimeContexts"],
                                run["normalized"]["session"]["sourceLocalTimeContexts"]
                            );
                        }
                    } else {
                        assert!(
                            consumer["samples"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .any(|sample| {
                                    sample["heartRateBpm"]
                                        .as_f64()
                                        .is_some_and(|hr| (120.0..=155.0).contains(&hr))
                                })
                        );
                    }
                }
            }
        }
        assert_native_history_status(&app, &session, id, "ready").await;
        assert_eq!(
            axum::body::to_bytes(
                serve(&pool, owner, &old_token).await.unwrap().into_body(),
                usize::MAX
            )
            .await
            .unwrap(),
            old_bytes,
        );
        // Publication must not alter the pinned source documents, including bad decoded fields.
        for (revision, original) in [(&revisions[0], decoded), (&revisions[1], normalized)] {
            let chunks = sqlx::query_scalar::<_, Vec<u8>>("SELECT payload FROM runs_revision_chunks WHERE revision_id=$1 AND owner_id=$2 AND document='archive' ORDER BY position")
                .bind(revision).bind(owner.to_string()).fetch_all(&pool).await.unwrap();
            assert_eq!(chunks.concat(), original.as_bytes());
        }
    }
    pool.close().await;
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE")))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}

#[test]
fn native_metric_types_keep_scaled_numbers_zero_and_invalid_state_not_opaque_strings() {
    let activity = json!({"decoded":{"messages":[{"globalMessageNumber":20,"fields":[
        {"fieldNumber":5,"value":123.45,"rawValue":12345,"baseType":134,"validity":"valid","unit":"m"},
        {"fieldNumber":3,"value":0,"rawValue":0,"baseType":2,"validity":"valid","unit":"bpm"},
        {"fieldNumber":7,"value":null,"rawValue":null,"baseType":132,"validity":"invalid"},
        {"fieldNumber":3,"value":"opaque-safe-looking-canary","baseType":2,"classification":"metric"}
    ]}]}});
    let (safe, omissions) = native_privacy_stream(
        garmin_fit_extractor_api::runs::privacy::Document::Decoded,
        &activity["decoded"],
        Policy::default(),
    );
    let fields = safe["messages"][0]["fields"].as_array().unwrap();
    assert_eq!(fields[0]["value"], 123.45);
    assert_eq!(fields[0]["rawValue"], 12345);
    assert_eq!(fields[1]["value"], 0);
    assert_eq!(fields[2]["value"], Value::Null);
    assert_eq!(fields[2]["validity"], "invalid");
    assert!(
        !serde_json::to_string(&(safe, omissions))
            .unwrap()
            .contains("opaque-safe-looking-canary")
    );
}

#[test]
fn unproven_native_extensions_cannot_promote_metric_names_or_developer_duplicates() {
    let activity = json!({"normalized":{"extensions":[
        {"identity":{"globalMessageNumber":20,"fieldNumber":3,"developerIdentity":null},"name":"private-name-canary","value":151,"rawValue":151,"type":2,"unit":"bpm","sourceReference":{"messageIndex":4,"globalMessageNumber":20,"fieldNumber":3}},
        {"identity":{"globalMessageNumber":20,"fieldNumber":3,"developerIdentity":{"developerDataIndex":0,"fieldDefinitionNumber":3}},"name":"heart_rate","value":777777777},
        {"identity":{"globalMessageNumber":20,"fieldNumber":250},"name":"heart_rate","value":888888888}
    ]}});
    let (safe, omissions) =
        project(&activity, Policy::default()).expect("optional unsafe values can be omitted");
    assert_eq!(safe["normalized"]["extensions"], json!([]));
    let text = serde_json::to_string(&(safe, omissions)).unwrap();
    for secret in ["151", "private-name-canary", "777777777", "888888888"] {
        assert!(!text.contains(secret));
    }
}

#[test]
fn streaming_rr_without_source_fails_closed_instead_of_promoting_compound_values() {
    use garmin_fit_extractor_api::runs::{
        privacy::{Document, Projection},
        stream::project_document,
    };
    use std::cell::RefCell;
    let normalized = json!({"schemaVersion":"2.0.0","samples":[{"index":0,"heartRateBpm":987654321,"sourceReferences":{"heartRateBpm":{"globalMessageNumber":20,"fieldNumber":3,"componentParent":0}}}],"rr":{"alignmentEligible":false,"intervals":[
        {"index":0,"rrMs":800,"sourceReference":{"globalMessageNumber":78,"fieldNumber":0,"arrayIndex":0}},
        {"index":1,"rrMs":805,"sourceReference":{"globalMessageNumber":132,"fieldNumber":9,"componentParent":10,"arrayIndex":7}},
        {"index":2,"rrMs":123456789,"sourceReference":{"globalMessageNumber":78,"fieldNumber":0,"arrayIndex":2,"developerIdentity":{"developerDataIndex":0,"fieldDefinitionNumber":0}}}
    ]}});
    let bytes = serde_json::to_vec(&normalized).unwrap();
    let project = |bytes: &[u8]| {
        let mut output = Vec::new();
        let projection = RefCell::new(Projection::new(Document::Normalized, Policy::default()));
        let result = project_document(
            bytes,
            &mut output,
            &mut |path| projection.borrow_mut().keep(path),
            &mut |path, value| projection.borrow_mut().try_value(path, value),
        );
        if let Some(error) = projection.borrow().error() {
            return Err(error);
        }
        result?;
        projection.into_inner().finish_checked()
    };
    let error = project(&bytes).unwrap_err();
    assert_eq!(error.code(), "RUNS_EXPORT_SOURCE_UNPROVABLE");
    assert_eq!(
        error.into_response().status(),
        axum::http::StatusCode::UNPROCESSABLE_ENTITY
    );
    let error = project(br#"{"schemaVersion":"2.0.0","rr":{"intervals":[}"#).unwrap_err();
    assert_eq!(error.code(), "INVALID_RUN_DOCUMENT");
}

#[test]
fn summary_retains_recorded_derived_coverage_and_literal_method_without_reference_promotion() {
    use garmin_fit_extractor_api::runs::privacy::Document;
    let mut source: Value =
        serde_json::from_slice(include_bytes!("fixtures/runs/legacy_native_good_wire.json"))
            .unwrap();
    let original_summary = source["normalized"]["summary"].clone();
    let original_lap_summary = source["normalized"]["laps"][0]["summary"].clone();
    source["normalized"]["summary"]["derived"]["private-canary.fit"] = json!("opaque-canary");
    source["normalized"]["summary"]["coverage"]["averageHeartRateBpm"]["filename"] =
        json!("coverage-canary");
    source["normalized"]["summary"]["method"]["filename"] = json!("method-canary");
    let (value, omissions) =
        native_privacy_stream(Document::Normalized, &source, Policy::default());
    assert_eq!(value["summary"], original_summary);
    assert_eq!(value["laps"][0]["summary"], original_lap_summary);
    let text = serde_json::to_string(&(value, omissions)).unwrap();
    for secret in [
        "private-canary",
        "opaque-canary",
        "coverage-canary",
        "method-canary",
    ] {
        assert!(!text.contains(secret));
    }
    source["normalized"]["summary"]["method"]["derived"] = json!("known-key-private-filename.fit");
    source["normalized"]["summary"]["method"]["gapThresholdSeconds"] = json!("opaque-gap");
    let (safe, omissions) =
        project(&source, Policy::default()).expect("invalid optional method can be omitted");
    let text = serde_json::to_string(&(safe, omissions)).unwrap();
    assert!(!text.contains("known-key-private"));
    assert!(!text.contains("opaque-gap"));
}

fn native_privacy_stream(
    document: garmin_fit_extractor_api::runs::privacy::Document,
    input: &Value,
    policy: Policy,
) -> (Value, Value) {
    use garmin_fit_extractor_api::runs::{
        privacy::{Document, Projection},
        stream::{SourceProof, project_document},
    };
    use std::cell::RefCell;
    let mut proof = SourceProof::default();
    let normalized = matches!(document, Document::Normalized);
    let document_input = if normalized {
        proof
            .decoder(&input["decoded"]["decoder"])
            .expect("actual archived profile receipt must be compatible");
        for message in input["decoded"]["messages"].as_array().unwrap() {
            proof
                .message(message)
                .expect("actual archived source must be structurally valid");
        }
        for sample in input["normalized"]["samples"].as_array().unwrap() {
            proof
                .observe_sample(sample)
                .expect("actual archived row must expose source coordinates");
        }
        &input["normalized"]
    } else {
        input
    };
    let source = serde_json::to_vec(document_input).unwrap();
    let mut output = Vec::new();
    let projection = RefCell::new(if normalized {
        Projection::with_source(document, policy, &proof)
    } else {
        Projection::new(document, policy)
    });
    project_document(
        source.as_slice(),
        &mut output,
        &mut |path| projection.borrow_mut().keep(path),
        &mut |path, value| projection.borrow_mut().try_value(path, value),
    )
    .unwrap();
    (
        serde_json::from_slice(&output).unwrap(),
        projection.into_inner().finish_checked().unwrap(),
    )
}

fn native_privacy_fixture(extra: &[u8]) -> Vec<u8> {
    fn crc(bytes: &[u8]) -> u16 {
        let mut crc = 0u16;
        for &byte in bytes {
            crc ^= u16::from(byte);
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xa001
                } else {
                    crc >> 1
                };
            }
        }
        crc
    }
    let original = include_bytes!("fixtures/runs/garmin_run.fit");
    let header = original[0] as usize;
    let mut bytes = original[..original.len() - 2].to_vec();
    bytes.extend_from_slice(extra);
    let length = (bytes.len() - header) as u32;
    bytes[4..8].copy_from_slice(&length.to_le_bytes());
    if header == 14 {
        let checksum = crc(&bytes[..12]);
        bytes[12..14].copy_from_slice(&checksum.to_le_bytes());
    }
    let checksum = crc(&bytes);
    bytes.extend_from_slice(&checksum.to_le_bytes());
    bytes
}

// These are unmodified run.json documents captured from the approved native MIT child,
// binary SHA-256 6239bb5abed2135f98c210b2bbc492999506099fb658655b97a99c91f8c0ed0e.
// Inputs use only CC0 garmin_run.fit (SHA-256
// 16f00e938171283656a36f0e41e4fda43368f662f28a2abde8c1379b87876232):
// extend session 18 with average HR field16 and power field20, append device_settings
// field2, records at FIT timestamps 1000000020/21, and HRV field0; repair both CRCs.
// Wrong-wire input: 757 bytes, b80c3d3f35316e36cc82a74ccae19d02a911d34c59e2cd7eca4be66da74d3136.
// Good-wire input: 715 bytes, d7c6029d0ed0d5389b747a4784983b8cd030ae709fcbd901366dc1a3da304fb5.
// Wrong-RR input: 719 bytes, 64ed7fc38478e866503b93d05c8c5a80b847c5e2f623461ce833d2b2336900c7.
// Consumer source: native_export_consumers.fit, original CC0 wire literals following
// generate.py definitions. Input 693 bytes, bfc9a198e38cd0eec559d22a82cd2134c74e85340a2f0745d2a61e42f94492e3;
// exact output 98988 bytes, 59aa24242c81d343ceff3562f7d53b7feed56d6ac06dc5145d602b6f83d0a344.
// Native records use offsets 0/1/3/4/20/28/31/33, native pause 4..20, and valid UInt8 HR,
// UInt16 power, Sint32 GPS, UInt32z serial, plus native HRV UInt16 array 800/810.
// Mixed alias input: 841 bytes, c315f4921ff2ff69aecfca4f0507a027f73f098209be57fe4be2addff41adfbf;
// exact output 122272 bytes, 80b3ca4fe94e371f6ab4115524bda57114a876df620af416dfcc25d8c2ce1255.
// Two appended records contain Float64 parents6/2 and correct UInt32 fields73/78.
// The producer naturally emits both native and expanded copies of identities73/78.
// The runs-spool-1 stdout byteLength/SHA matched each 0600 document in its 0700 directory.
fn legacy_native_activity(bytes: &[u8], expected_sha: &str, id: Uuid) -> Value {
    use sha2::{Digest, Sha256};
    assert_eq!(format!("{:x}", Sha256::digest(bytes)), expected_sha);
    let archive: Value = serde_json::from_slice(bytes).unwrap();
    let decoder = &archive["decoded"]["decoder"];
    assert_eq!(
        decoder["rawProjectionSourceSha256"],
        "3b983feaabf607b735170d6c58e442c39562c95de44da5f15df64c633583dff5"
    );
    assert_eq!(
        decoder["normalizerSourceSha256"],
        "37b2d91074e867440b7e942205729c47570ee4b91cb9fd67e81e3775631367db"
    );
    assert_eq!(decoder["profileVersion"], "21.202.0");
    assert_eq!(
        decoder["profileSourceSha256"],
        "fe9695a0ee955c4cdfad275bf11bc83706bdd87868a497894f8327e527145f9e"
    );
    assert_eq!(
        decoder["vendorSourceSha256"],
        "5cba14de1b4f88f3c3a55d755d27ceb8e7fb782027e3786b347b2b1e0a597bf1"
    );
    // Supply only the consumer envelope. Never alter the captured decoded/normalized revision.
    json!({
        "id": id,
        "decoded": archive["decoded"],
        "normalized": archive["normalized"],
        "analysis": {"schemaVersion":"2.0.0","segments":[],"quality":{},"transformations":[]},
        "historicalThresholds": {"evidenceCutoff":archive["normalized"]["endTime"]}
    })
}

#[test]
fn missing_or_other_profile_receipt_rejects_exports_without_promoting_native_values() {
    let id = Uuid::new_v4();
    let mut run = activity(id, [0]);
    let mut other_profile = run["decoded"]["decoder"].clone();
    other_profile["profileSourceSha256"] = json!("0".repeat(64));
    for receipt in [Value::Null, other_profile] {
        // Change only the receipt. Every captured native field remains intact.
        run["decoded"]["decoder"] = receipt;
        for include_location in [false, true] {
            for include_device_identifiers in [false, true] {
                let policy = Policy {
                    include_location,
                    include_device_identifiers,
                };
                let error = project(&run, policy).unwrap_err();
                assert_eq!(error.code(), "RUNS_EXPORT_SOURCE_UNPROVABLE");
                for mode in [ExportMode::Full, ExportMode::Coach] {
                    let mut selection = request(id, mode);
                    selection.include_location = include_location;
                    selection.include_device_identifiers = include_device_identifiers;
                    let error = snapshot(
                        &[id],
                        std::slice::from_ref(&run),
                        &selection,
                        "2026-01-01T00:00:00Z",
                    )
                    .unwrap_err();
                    assert_eq!(error.code(), "RUNS_EXPORT_SOURCE_UNPROVABLE");
                }
            }
        }
    }
}

#[test]
fn legacy_native_selected_alias_keeps_good_values_despite_wrong_wire_virtual_duplicates() {
    use garmin_fit_extractor_api::runs::privacy::Document;
    let id = Uuid::new_v4();
    let run = legacy_native_activity(
        include_bytes!("fixtures/runs/legacy_native_mixed_alias.json"),
        "80b3ca4fe94e371f6ab4115524bda57114a876df620af416dfcc25d8c2ce1255",
        id,
    );
    for (sample_index, message_index) in [(6, 28), (7, 29)] {
        let fields = run["decoded"]["messages"][message_index]["fields"]
            .as_array()
            .unwrap();
        for (number, parent, value) in [(73, 6, 3.0), (78, 2, 100.0)] {
            let native = fields
                .iter()
                .find(|field| field["fieldNumber"] == number && field["role"] == "native")
                .unwrap();
            let expanded = fields
                .iter()
                .find(|field| field["fieldNumber"] == number && field["role"] == "expanded")
                .unwrap();
            assert_eq!(native["baseType"], 134);
            assert_eq!(native["value"], value);
            assert_eq!(expanded["sourceReference"]["componentParent"], parent);
            assert_ne!(expanded["value"], native["value"]);
        }
        assert_eq!(run["normalized"]["samples"][sample_index]["speedMps"], 3.0);
        assert_eq!(
            run["normalized"]["samples"][sample_index]["altitudeMeters"],
            100.0
        );
        assert!(
            run["normalized"]["samples"][sample_index]["sourceReferences"]["speedMps"]
                .get("componentParent")
                .is_none()
        );
        assert!(
            run["normalized"]["samples"][sample_index]["sourceReferences"]["altitudeMeters"]
                .get("componentParent")
                .is_none()
        );
    }
    for include_location in [false, true] {
        for include_device_identifiers in [false, true] {
            let policy = Policy {
                include_location,
                include_device_identifiers,
            };
            let (safe, _) = project(&run, policy).expect("selected native aliases remain valid");
            let (streamed, _) = native_privacy_stream(Document::Normalized, &run, policy);
            for normalized in [&safe["normalized"], &streamed] {
                for index in [6, 7] {
                    assert_eq!(
                        normalized["samples"][index],
                        run["normalized"]["samples"][index]
                    );
                }
                assert_eq!(
                    normalized["summary"]["derived"],
                    run["normalized"]["summary"]["derived"]
                );
                assert_eq!(
                    normalized["summary"]["coverage"],
                    run["normalized"]["summary"]["coverage"]
                );
            }
            for index in [28, 29] {
                let fields = safe["decoded"]["messages"][index]["fields"]
                    .as_array()
                    .unwrap();
                assert!(
                    !fields
                        .iter()
                        .any(|field| field["fieldNumber"] == 2 || field["fieldNumber"] == 6)
                );
                for number in [73, 78] {
                    let mut aliases = fields.iter().filter(|field| field["fieldNumber"] == number);
                    let alias = aliases.next().unwrap();
                    assert!(aliases.next().is_none());
                    assert_eq!(alias["baseType"], 134);
                    assert_eq!(alias["role"], "native");
                }
            }
            for mode in [ExportMode::Full, ExportMode::Coach] {
                let mut selection = request(id, mode);
                selection.include_location = include_location;
                selection.include_device_identifiers = include_device_identifiers;
                let bytes = snapshot(
                    &[id],
                    std::slice::from_ref(&run),
                    &selection,
                    "2026-01-01T00:00:00Z",
                )
                .unwrap();
                assert!(!std::str::from_utf8(&bytes).unwrap().contains("9876.125"));
                let exported: Value = serde_json::from_slice(&bytes).unwrap();
                let consumer = &exported["activities"][0];
                if mode == ExportMode::Full {
                    for index in [6, 7] {
                        assert_eq!(
                            consumer["normalized"]["samples"][index],
                            run["normalized"]["samples"][index]
                        );
                    }
                } else {
                    assert_eq!(
                        consumer["summary"]["derived"],
                        run["normalized"]["summary"]["derived"]
                    );
                    assert!(
                        consumer["samples"].as_array().unwrap().iter().any(
                            |sample| sample["speedMps"] == 3.0 && sample["powerWatts"] == 275.0
                        )
                    );
                }
            }
        }
    }
}

#[test]
fn stored_native_field_payload_cannot_borrow_authority_from_valid_source_reference() {
    use garmin_fit_extractor_api::runs::privacy::Document;
    let id = Uuid::new_v4();
    for payload in [
        json!("stored-field-private-canary"),
        json!({"value":"stored-field-private-canary"}),
    ] {
        let mut run = activity(id, [0]);
        // This is an adversarial stored-document mutation, not producer output.
        // The native source, field identity, wire type and exact coordinate remain genuine.
        let fields = run["normalized"]["sensors"][0]["fields"]
            .as_array_mut()
            .unwrap();
        let field = fields
            .iter_mut()
            .find(|field| field["identity"]["fieldNumber"] == 5)
            .unwrap();
        field["value"] = payload;
        for include_location in [false, true] {
            for include_device_identifiers in [false, true] {
                let policy = Policy {
                    include_location,
                    include_device_identifiers,
                };
                let (safe, omissions) =
                    project(&run, policy).expect("bad optional field payload can be omitted");
                let (normalized, streamed_omissions) =
                    native_privacy_stream(Document::Normalized, &run, policy);
                for document in [&safe["normalized"], &normalized] {
                    let fields = document["sensors"][0]["fields"].as_array().unwrap();
                    assert!(!fields.iter().any(|field| field["fieldNumber"] == 5));
                    let control = fields
                        .iter()
                        .find(|field| field["fieldNumber"] == 0)
                        .unwrap();
                    assert_eq!(control["value"], "creator");
                    assert_eq!(control["rawValue"], 0);
                    assert_eq!(control["enumCode"], 0);
                    assert_eq!(
                        control["sourceReference"],
                        run["normalized"]["sensors"][0]["fields"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|field| field["identity"]["fieldNumber"] == 0)
                            .unwrap()["sourceReference"]
                    );
                }
                let text =
                    serde_json::to_string(&(safe, omissions, normalized, streamed_omissions))
                        .unwrap();
                assert!(!text.contains("stored-field-private-canary"));
                for mode in [ExportMode::Full, ExportMode::Coach] {
                    let mut selection = request(id, mode);
                    selection.include_location = include_location;
                    selection.include_device_identifiers = include_device_identifiers;
                    let bytes = snapshot(
                        &[id],
                        std::slice::from_ref(&run),
                        &selection,
                        "2026-01-01T00:00:00Z",
                    )
                    .unwrap();
                    assert!(
                        !std::str::from_utf8(&bytes)
                            .unwrap()
                            .contains("stored-field-private-canary")
                    );
                }
            }
        }
    }
}

#[test]
fn legacy_native_wrong_wire_is_filtered_by_project_and_both_export_modes() {
    let id = Uuid::new_v4();
    let run = legacy_native_activity(
        include_bytes!("fixtures/runs/legacy_native_wrong_wire.json"),
        "5e1763ba952ffad2018fbc8546ce1e6ac956a267485983121e5a2532a1f78da1",
        id,
    );
    let original = run.clone();
    assert_eq!(
        run["decoded"]["decoder"]["sourceSha256"],
        "b80c3d3f35316e36cc82a74ccae19d02a911d34c59e2cd7eca4be66da74d3136"
    );
    let record = &run["decoded"]["messages"][25];
    for (number, wire, value) in [(3, 137, json!(239.125)), (7, 143, json!(735019))] {
        let field = record["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|field| field["fieldNumber"] == number)
            .unwrap();
        assert_eq!(field["baseType"], wire);
        assert_eq!(field["value"], value);
        assert_eq!(field["rawValue"], value);
        assert_eq!(field["sourceReference"]["messageIndex"], 25);
    }
    assert_eq!(run["normalized"]["samples"][4]["heartRateBpm"], 239.125);
    assert_eq!(run["normalized"]["samples"][4]["powerWatts"], 735019.0);
    assert_eq!(
        run["normalized"]["samples"][4]["sourceReferences"]["heartRateBpm"]["byteLength"],
        8
    );
    assert_eq!(
        run["normalized"]["summary"]["recorded"]["averageHeartRateBpm"],
        231.125
    );
    assert_eq!(
        run["normalized"]["summary"]["recorded"]["averagePowerWatts"],
        735019.0
    );
    assert_eq!(
        run["normalized"]["laps"][0]["summary"]["derived"]["averagePowerWatts"],
        735019.0
    );
    assert_eq!(
        run["normalized"]["session"]["sourceLocalTimeContexts"][1]["timeOffsetsSeconds"],
        json!(["735019"])
    );
    for include_location in [false, true] {
        for include_device_identifiers in [false, true] {
            let policy = Policy {
                include_location,
                include_device_identifiers,
            };
            let (safe, omissions) =
                project(&run, policy).expect("bad optional sources can be omitted");
            let normalized = &safe["normalized"];
            for index in 0..4 {
                assert_eq!(
                    normalized["samples"][index],
                    run["normalized"]["samples"][index]
                );
            }
            for index in [4, 5] {
                for key in ["heartRateBpm", "powerWatts"] {
                    assert!(normalized["samples"][index].get(key).is_none());
                    assert!(
                        normalized["samples"][index]["sourceReferences"]
                            .get(key)
                            .is_none()
                    );
                }
                for key in [
                    "timestamp",
                    "speedMps",
                    "paceSecondsPerKm",
                    "cadenceStepsPerMinute",
                ] {
                    assert_eq!(
                        normalized["samples"][index][key],
                        run["normalized"]["samples"][index][key]
                    );
                    assert_eq!(
                        normalized["samples"][index]["sourceReferences"][key],
                        run["normalized"]["samples"][index]["sourceReferences"][key]
                    );
                }
                assert_eq!(normalized["samples"][index]["altitudeMeters"], Value::Null);
                assert_eq!(normalized["samples"][index]["distanceMeters"], Value::Null);
            }
            for key in ["averageHeartRateBpm", "averagePowerWatts"] {
                assert!(normalized["summary"]["recorded"].get(key).is_none());
                assert!(normalized["summary"]["sourceReferences"].get(key).is_none());
            }
            for (summary, source_summary) in [
                (&normalized["summary"], &run["normalized"]["summary"]),
                (
                    &normalized["laps"][0]["summary"],
                    &run["normalized"]["laps"][0]["summary"],
                ),
            ] {
                assert!(summary.get("averagePowerWatts").is_none());
                assert!(summary["derived"].get("averagePowerWatts").is_none());
                for key in ["distanceMeters", "elapsedTimeSeconds", "timerTimeSeconds"] {
                    assert_eq!(summary["recorded"][key], source_summary["recorded"][key]);
                    assert_eq!(
                        summary["sourceReferences"][key],
                        source_summary["sourceReferences"][key]
                    );
                }
            }
            assert_eq!(
                normalized["session"]["sourceLocalTimeContexts"][0],
                run["normalized"]["session"]["sourceLocalTimeContexts"][0]
            );
            let bad_context = &normalized["session"]["sourceLocalTimeContexts"][1];
            assert!(bad_context.get("timeOffsetsSeconds").is_none());
            assert!(
                bad_context["sourceReferences"]
                    .get("timeOffsetsSeconds")
                    .is_none()
            );
            assert_eq!(normalized["rr"], run["normalized"]["rr"]);
            let text = serde_json::to_string(&(safe, omissions)).unwrap();
            for secret in ["239.125", "231.125", "735019"] {
                assert!(!text.contains(secret), "project leaked {secret}");
            }
            for mode in [ExportMode::Full, ExportMode::Coach] {
                let mut selection = request(id, mode);
                selection.include_location = include_location;
                selection.include_device_identifiers = include_device_identifiers;
                let bytes = snapshot(
                    &[id],
                    std::slice::from_ref(&run),
                    &selection,
                    "2026-01-01T00:00:00Z",
                )
                .unwrap();
                let text = std::str::from_utf8(&bytes).unwrap();
                for secret in ["239.125", "231.125", "735019"] {
                    assert!(!text.contains(secret), "{mode:?} leaked {secret}");
                }
                let exported: Value = serde_json::from_slice(&bytes).unwrap();
                let summary = if mode == ExportMode::Full {
                    &exported["activities"][0]["normalized"]["summary"]
                } else {
                    &exported["activities"][0]["summary"]
                };
                assert_eq!(summary["recorded"]["distanceMeters"], 1000.0);
                assert_eq!(
                    summary["sourceReferences"]["distanceMeters"],
                    run["normalized"]["summary"]["sourceReferences"]["distanceMeters"]
                );
            }
        }
    }
    assert_eq!(run, original);
}

#[test]
fn legacy_native_good_fields_references_and_nulls_survive_both_export_modes() {
    let id = Uuid::new_v4();
    let run = legacy_native_activity(
        include_bytes!("fixtures/runs/legacy_native_good_wire.json"),
        "1e889b74ceaf3f89c088e655bf39c77be7cbc1d93d41e77b6c8c22a2bce9e18e",
        id,
    );
    let original = run.clone();
    assert_eq!(run["normalized"]["samples"][4]["heartRateBpm"], 155.0);
    assert_eq!(run["normalized"]["samples"][4]["powerWatts"], 275.0);
    assert_eq!(run["normalized"]["rr"]["intervals"][0]["rrMs"], 800.0);
    assert_eq!(run["normalized"]["rr"]["intervals"][1]["rrMs"], 810.0);
    let original_heart_rate = run["decoded"]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|message| message["globalMessageNumber"] == 20)
        .flat_map(|message| message["fields"].as_array().unwrap())
        .find(|field| field["fieldNumber"] == 3 && field["role"] == "native")
        .unwrap();
    assert_eq!(
        original_heart_rate.get("developerIdentity"),
        Some(&Value::Null)
    );
    let mut developer_marked = run["decoded"].clone();
    for message in developer_marked["messages"].as_array_mut().unwrap() {
        if message["globalMessageNumber"] == 20 {
            for field in message["fields"].as_array_mut().unwrap() {
                if field["fieldNumber"] == 3 && field["role"] == "native" {
                    field["developerIdentity"] = json!({
                        "developerDataIndex": 0,
                        "fieldDefinitionNumber": 3,
                        "applicationId": "private-developer-identity"
                    });
                }
            }
        }
    }
    for include_location in [false, true] {
        for include_device_identifiers in [false, true] {
            let policy = Policy {
                include_location,
                include_device_identifiers,
            };
            let (rejected, _) = native_privacy_stream(
                garmin_fit_extractor_api::runs::privacy::Document::Decoded,
                &developer_marked,
                policy,
            );
            assert!(
                rejected["messages"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|message| message["globalMessageNumber"] == 20)
                    .flat_map(|message| message["fields"].as_array().unwrap())
                    .all(|field| field["fieldNumber"] != 3)
            );
            assert!(!rejected.to_string().contains("private-developer-identity"));
            let (safe, _) =
                project(&run, policy).expect("genuine old native sources remain provable");
            for key in ["samples", "summary", "laps", "rr"] {
                assert_eq!(
                    safe["normalized"][key], run["normalized"][key],
                    "changed {key}"
                );
            }
            assert_eq!(
                safe["normalized"]["session"]["sourceLocalTimeContexts"],
                run["normalized"]["session"]["sourceLocalTimeContexts"]
            );
            for mode in [ExportMode::Full, ExportMode::Coach] {
                let mut selection = request(id, mode);
                selection.include_location = include_location;
                selection.include_device_identifiers = include_device_identifiers;
                let bytes = snapshot(
                    &[id],
                    std::slice::from_ref(&run),
                    &selection,
                    "2026-01-01T00:00:00Z",
                )
                .unwrap();
                let exported: Value = serde_json::from_slice(&bytes).unwrap();
                let consumer = &exported["activities"][0];
                if mode == ExportMode::Full {
                    let exported_heart_rate = consumer["decoded"]["messages"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter(|message| message["globalMessageNumber"] == 20)
                        .flat_map(|message| message["fields"].as_array().unwrap())
                        .find(|field| field["fieldNumber"] == 3 && field["role"] == "native")
                        .unwrap();
                    assert_eq!(
                        exported_heart_rate.get("developerIdentity"),
                        original_heart_rate.get("developerIdentity")
                    );
                    for key in ["samples", "summary", "laps", "rr"] {
                        assert_eq!(consumer["normalized"][key], run["normalized"][key]);
                    }
                } else {
                    assert_eq!(consumer["summary"], run["normalized"]["summary"]);
                    assert_eq!(consumer["laps"], run["normalized"]["laps"]);
                    assert!(
                        consumer["samples"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|sample| {
                                sample["heartRateBpm"] == 155.0 && sample["powerWatts"] == 275.0
                            })
                    );
                }
            }
        }
    }
    assert_eq!(run, original);
}

#[test]
fn legacy_native_wrong_rr_required_value_rejects_atomic_export() {
    use axum::response::IntoResponse;
    let id = Uuid::new_v4();
    let run = legacy_native_activity(
        include_bytes!("fixtures/runs/legacy_native_wrong_rr.json"),
        "173e50a422f570c6c659eed79bd75ee82990bd9d445be158fafe7ccfebc49a59",
        id,
    );
    let original = run.clone();
    let rr = &run["decoded"]["messages"][27]["fields"][0];
    assert_eq!(rr["baseType"], 143);
    assert_eq!(rr["rawValue"], 735019);
    assert_eq!(rr["value"], 735.019);
    assert_eq!(run["normalized"]["rr"]["intervals"][0]["rrMs"], 735019.0);
    assert_eq!(run["normalized"]["samples"][4]["heartRateBpm"], 155.0);
    for include_location in [false, true] {
        for include_device_identifiers in [false, true] {
            let policy = Policy {
                include_location,
                include_device_identifiers,
            };
            let error = project(&run, policy).unwrap_err();
            assert_eq!(error.code(), "RUNS_EXPORT_SOURCE_UNPROVABLE");
            assert_eq!(
                error.into_response().status(),
                axum::http::StatusCode::UNPROCESSABLE_ENTITY
            );
            for mode in [ExportMode::Full, ExportMode::Coach] {
                let mut selection = request(id, mode);
                selection.include_location = include_location;
                selection.include_device_identifiers = include_device_identifiers;
                let error = snapshot(
                    &[id],
                    std::slice::from_ref(&run),
                    &selection,
                    "2026-01-01T00:00:00Z",
                )
                .unwrap_err();
                assert_eq!(error.code(), "RUNS_EXPORT_SOURCE_UNPROVABLE");
                assert_eq!(
                    error.into_response().status(),
                    axum::http::StatusCode::UNPROCESSABLE_ENTITY
                );
            }
        }
    }
    assert_eq!(run, original);
}

#[test]
fn native_privacy_wrong_wire_strings_never_escape_archive_or_local_context() {
    use garmin_fit_extractor_api::{fit::runs::decode_run_to_writer, runs::privacy::Document};
    let canary = b"time-offset-private-canary\0";
    // CC0 synthetic device_settings definition: numeric profile field, String wire type.
    let mut extra = vec![0x40, 0, 0, 2, 0, 1, 2, canary.len() as u8, 7, 0];
    // A numeric-looking String must not acquire numeric field authority either.
    extra.extend_from_slice(canary);
    extra.extend_from_slice(&[0x40, 0, 0, 20, 0, 1, 3, 2, 7, 0, b'0', 0]);
    let fixture = native_privacy_fixture(&extra);
    let original = fixture.clone();
    let mut archive = Vec::new();
    decode_run_to_writer(&fixture, &mut archive).unwrap();
    assert_eq!(fixture, original);
    let source: Value = serde_json::from_slice(&archive).unwrap();
    assert!(
        source["decoded"]["messages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|message| message["globalMessageNumber"] == 2)
            .flat_map(|message| message["fields"].as_array().unwrap())
            .any(|field| field["value"] == "time-offset-private-canary" && field["baseType"] == 7)
    );
    assert!(
        !serde_json::to_string(&source["normalized"])
            .unwrap()
            .contains("time-offset-private-canary")
    );
    for include_location in [false, true] {
        for include_device_identifiers in [false, true] {
            let policy = Policy {
                include_location,
                include_device_identifiers,
            };
            for (document, key) in [
                (Document::Decoded, "decoded"),
                (Document::Normalized, "normalized"),
            ] {
                let input = if key == "decoded" {
                    &source[key]
                } else {
                    &source
                };
                let (safe, omissions) = native_privacy_stream(document, input, policy);
                let output = serde_json::to_string(&(safe, omissions)).unwrap();
                assert!(
                    !output.contains("time-offset-private-canary"),
                    "leaked native String with location={include_location}, device={include_device_identifiers}, document={key}"
                );
            }
        }
    }
}

#[test]
fn native_privacy_shape_gates_keep_native_arrays_and_reject_unprovable_archive() {
    use garmin_fit_extractor_api::runs::privacy::Document;
    for bad in [
        json!("shape-private-canary"),
        json!("0"),
        json!(["shape-private-canary"]),
        json!({"private-name-canary":{"value":"shape-private-canary"}}),
        json!(true),
    ] {
        let id = Uuid::new_v4();
        let mut run = activity(id, vec![]);
        run["decoded"]["messages"] = json!([
            {"globalMessageNumber":20,"fields":[
                {"fieldNumber":3,"value":bad,"rawValue":bad,"baseType":2,"name":"private-name-canary"},
                {"fieldNumber":0,"value":bad,"baseType":133},
                {"fieldNumber":3,"value":bad,"rawValue":bad,"baseType":7,"profileType":"uint8","classification":"metric"},
                {"fieldNumber":3,"value":[0,null,151],"rawValue":[0,null,151],"baseType":2,"validity":"mixed"},
                {"fieldNumber":3,"value":0,"rawValue":0,"baseType":2},
                {"fieldNumber":5,"value":1000.0,"rawValue":100000,"baseType":134},
                {"fieldNumber":5,"value":"18446744073709551614","rawValue":"18446744073709551614","baseType":143},
                {"fieldNumber":3,"value":231111.2233334444,"rawValue":231111.2233334444,"baseType":137},
                {"fieldNumber":7,"value":151.5,"rawValue":151.5,"baseType":137},
                {"fieldNumber":0,"value":1111111111111111_i64,"rawValue":1111111111111111_i64,"baseType":143},
                {"fieldNumber":1,"value":123456789,"baseType":133},
                {"fieldNumber":250,"value":"unknown-private-canary"},
                {"fieldNumber":3,"value":"developer-private-canary","developerIdentity":{"developerDataIndex":7,"fieldDefinitionNumber":3}}]},
            {"globalMessageNumber":23,"fields":[{"fieldNumber":3,"value":bad,"baseType":140},{"fieldNumber":3,"value":2222222.5,"rawValue":2222222.5,"baseType":137},{"fieldNumber":21,"value":44444,"rawValue":44444,"baseType":139}]},
            {"globalMessageNumber":78,"fields":[{"fieldNumber":0,"value":[0,0.8,null],"rawValue":[0,800,null],"baseType":132}]}
        ]);
        run["normalized"]["session"] = json!({"sourceLocalTimeContext":{"activeTimeZone":0,"timeOffsetsSeconds":[0,null,3600],"timeZoneOffsetsHours":[-5,0,null]},"sourceLocalTimeContexts":[{"timeOffsetsSeconds":[bad],"timeZoneOffsetsHours":[bad]}]});
        run["normalized"]["samples"] = json!([{"index":0,"latitudeDegrees":bad,"heartRateBpm":0}]);
        run["normalized"]["sensors"] = json!([{"serialNumber":bad}]);
        let original = run.clone();
        for include_location in [false, true] {
            for include_device_identifiers in [false, true] {
                let policy = Policy {
                    include_location,
                    include_device_identifiers,
                };
                let (safe, omissions) =
                    native_privacy_stream(Document::Decoded, &run["decoded"], policy);
                let fields = safe["messages"][0]["fields"].as_array().unwrap();
                assert!(!fields.iter().any(|field| field["value"] == bad));
                assert!(
                    !fields
                        .iter()
                        .any(|field| field["value"] == json!([0, null, 151]))
                );
                assert!(
                    fields
                        .iter()
                        .any(|field| field["fieldNumber"] == 3 && field["value"] == 0)
                );
                assert_eq!(
                    safe["messages"][2]["fields"][0]["value"],
                    json!([0, 0.8, null])
                );
                assert_eq!(
                    safe["messages"][2]["fields"][0]["rawValue"],
                    json!([0, 800, null])
                );
                assert!(
                    fields
                        .iter()
                        .any(|field| field["value"] == 1000.0 && field["rawValue"] == 100000)
                );
                assert_eq!(
                    fields.iter().any(|field| field["value"] == 123456789),
                    include_location
                );
                assert_eq!(
                    safe["messages"][1]["fields"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|field| field["value"] == 44444),
                    include_device_identifiers
                );
                // This synthetic decoded input has no archive coordinates. It may exercise
                // standalone native guards, but it cannot authorize a normalized revision.
                let error = project(&run, policy).unwrap_err();
                assert_eq!(error.code(), "RUNS_EXPORT_SOURCE_UNPROVABLE");
                let output = serde_json::to_string(&(safe, omissions)).unwrap();
                for canary in [
                    "shape-private-canary",
                    "private-name-canary",
                    "unknown-private-canary",
                    "developer-private-canary",
                    "18446744073709551614",
                    "231111.2233334444",
                    "151.5",
                    "1111111111111111",
                    "2222222.5",
                ] {
                    assert!(
                        !output.contains(canary),
                        "leaked {canary} with location={include_location}, device={include_device_identifiers}"
                    );
                }
                for mode in [ExportMode::Full, ExportMode::Coach] {
                    let mut consent = request(id, mode);
                    consent.include_location = include_location;
                    consent.include_device_identifiers = include_device_identifiers;
                    let error = snapshot(&[id], &[run.clone()], &consent, "2026-01-01T00:00:00Z")
                        .unwrap_err();
                    assert_eq!(error.code(), "RUNS_EXPORT_SOURCE_UNPROVABLE");
                }
            }
        }
        assert_eq!(run, original);
    }
}

#[test]
fn native_privacy_event_profile_variants_keep_raw_codes_and_references_not_strings() {
    use fitparser::profile::{FieldDataType, get_field_variant_as_string};
    use garmin_fit_extractor_api::{fit::runs::decode_run_to_writer, runs::privacy::Document};
    let mut extra = vec![0x40, 0, 0, 21, 0, 1, 0, 1, 0];
    let codes: Vec<_> = (0..255)
        .filter(|code| FieldDataType::Event.is_named_variant(*code))
        .collect();
    for &code in &codes {
        extra.extend_from_slice(&[0, code as u8]);
    }
    let fixture = native_privacy_fixture(&extra);
    let mut archive = Vec::new();
    decode_run_to_writer(&fixture, &mut archive).unwrap();
    let source: Value = serde_json::from_slice(&archive).unwrap();
    let appended = source["decoded"]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .take(codes.len())
        .rev()
        .cloned()
        .collect::<Vec<_>>();
    let input = json!({"schemaVersion":"2.0.0","messages":appended});
    let (safe, omissions) = native_privacy_stream(Document::Decoded, &input, Policy::default());
    for (index, &code) in codes.iter().enumerate() {
        let field = &safe["messages"][index]["fields"][0];
        assert_eq!(
            field["value"],
            get_field_variant_as_string(FieldDataType::Event, code),
            "lost verified event {code}"
        );
        assert_eq!(field["rawValue"], code);
        assert_eq!(field["enumCode"], code);
        assert_eq!(
            field["sourceReference"],
            input["messages"][index]["fields"][0]["sourceReference"]
        );
        if code == 9 {
            assert_eq!(field["value"], "lap");
        }
    }
    assert_eq!(omissions, json!([]));
    for malicious in [
        json!({"fieldNumber":0,"value":"lap","rawValue":"lap","enumCode":9,"baseType":7}),
        json!({"fieldNumber":0,"value":"running","rawValue":9,"enumCode":9,"baseType":0}),
        json!({"fieldNumber":0,"value":"enum-private-canary","rawValue":250,"enumCode":250,"baseType":0}),
        json!({"fieldNumber":0,"value":"lap","rawValue":9,"enumCode":9,"baseType":7,"profileType":"event","classification":"metric"}),
        json!({"fieldNumber":0,"value":"lap","rawValue":9,"enumCode":9,"baseType":0,"developerIdentity":{"developerDataIndex":1,"fieldDefinitionNumber":0}}),
        json!({"fieldNumber":250,"value":"lap","rawValue":9,"enumCode":9,"baseType":0}),
        json!({"fieldNumber":0,"value":"lap","enumCode":9,"baseType":0}),
    ] {
        let input = json!({"messages":[{"globalMessageNumber":21,"fields":[malicious]}]});
        let (safe, omissions) = native_privacy_stream(Document::Decoded, &input, Policy::default());
        assert_eq!(safe["messages"][0]["fields"], json!([]));
        assert!(
            !serde_json::to_string(&(safe, omissions))
                .unwrap()
                .contains("enum-private-canary")
        );
    }
}

#[test]
fn native_privacy_compressed_timestamp_keeps_reconstruction_and_lineage() {
    use garmin_fit_extractor_api::{fit::runs::decode_run_to_writer, runs::privacy::Document};
    let mut archive = Vec::new();
    decode_run_to_writer(
        include_bytes!("fixtures/runs/garmin_archive.fit"),
        &mut archive,
    )
    .unwrap();
    let source: Value = serde_json::from_slice(&archive).unwrap();
    let original = source["decoded"]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|message| message["globalMessageNumber"] == 20)
        .flat_map(|message| message["fields"].as_array().unwrap())
        .filter(|field| field["role"] == "reconstructed")
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(original.len(), 2);
    let (safe, _) = native_privacy_stream(Document::Decoded, &source["decoded"], Policy::default());
    let projected = safe["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|message| message["globalMessageNumber"] == 20)
        .flat_map(|message| message["fields"].as_array().unwrap())
        .filter(|field| field["role"] == "reconstructed")
        .collect::<Vec<_>>();
    assert_eq!(projected.len(), 2);
    for (expected, actual) in original.iter().zip(projected) {
        assert_eq!(actual["value"], expected["value"]);
        assert_eq!(actual["sourceReference"], expected["sourceReference"]);
        assert_eq!(
            actual["compressedTimeOffset"],
            expected["compressedTimeOffset"]
        );
    }
}

#[test]
fn native_privacy_unknown_semantic_enum_fails_closed_not_numeric_profile_fallbacks() {
    use fitparser::profile::{
        FieldDataType, field_types::Manufacturer, get_field_variant_as_string,
    };
    use garmin_fit_extractor_api::{fit::runs::decode_run_to_writer, runs::privacy::Document};
    let mut extra = vec![0x40, 0, 0, 23, 0, 3, 0, 1, 2, 2, 2, 132, 4, 2, 132, 0, 0];
    extra.extend_from_slice(&(Manufacturer::Suunto.as_i64() as u16).to_le_bytes());
    extra.extend_from_slice(&50000u16.to_le_bytes());
    // Native scalar index zero is Creator. Same-wire arrays are still malformed
    // for the declared scalar device_index profile and must not gain authority.
    extra.extend_from_slice(&[0x40, 0, 0, 23, 0, 1, 0, 3, 2, 0, 0, 1, 255]);
    extra.extend_from_slice(&[0x40, 0, 0, 23, 0, 1, 0, 1, 2, 0, 1]);
    extra.extend_from_slice(&[0x40, 0, 0, 21, 0, 1, 0, 1, 0, 0, 250]);
    let fixture = native_privacy_fixture(&extra);
    let mut archive = Vec::new();
    decode_run_to_writer(&fixture, &mut archive).unwrap();
    let source: Value = serde_json::from_slice(&archive).unwrap();
    assert_eq!(
        source["decoded"]["messages"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["fields"][0]["value"],
        250
    );
    let creator_source = source["decoded"]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|message| message["globalMessageNumber"] == 23)
        .flat_map(|message| message["fields"].as_array().unwrap())
        .find(|field| field["fieldNumber"] == 0 && field["rawValue"] == 0)
        .unwrap();
    assert_eq!(
        creator_source["value"],
        get_field_variant_as_string(FieldDataType::DeviceIndex, 0)
    );
    assert_eq!(creator_source["enumCode"], 0);
    let products = source["decoded"]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|message| message["globalMessageNumber"] == 23)
        .flat_map(|message| message["fields"].as_array().unwrap())
        .filter(|field| field["fieldNumber"] == 4)
        .collect::<Vec<_>>();
    let generic_product = products.last().unwrap();
    assert_eq!(generic_product["profileType"], "uint16");
    assert_eq!(generic_product["value"], 50000);
    for include_location in [false, true] {
        for include_device_identifiers in [false, true] {
            let policy = Policy {
                include_location,
                include_device_identifiers,
            };
            let (decoded, omissions) =
                native_privacy_stream(Document::Decoded, &source["decoded"], policy);
            assert_eq!(
                decoded["messages"].as_array().unwrap().last().unwrap()["fields"],
                json!([])
            );
            let fields = decoded["messages"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|message| message["globalMessageNumber"] == 23)
                .flat_map(|message| message["fields"].as_array().unwrap())
                .collect::<Vec<_>>();
            let creator = fields
                .iter()
                .find(|field| field["fieldNumber"] == 0 && field["rawValue"] == 0)
                .unwrap();
            assert_eq!(
                creator["value"],
                get_field_variant_as_string(FieldDataType::DeviceIndex, 0)
            );
            assert_eq!(creator["enumCode"], 0);
            assert_eq!(
                creator["sourceReference"],
                creator_source["sourceReference"]
            );
            assert!(fields.iter().any(|field| field["fieldNumber"] == 0
                && field["value"] == 1
                && field["rawValue"] == 1));
            assert!(
                !fields
                    .iter()
                    .any(|field| field["fieldNumber"] == 0 && field["value"].is_array())
            );
            for original in &products {
                let product = fields
                    .iter()
                    .find(|field| field["sourceReference"] == original["sourceReference"])
                    .unwrap();
                assert_eq!(product["value"], original["value"]);
                assert_eq!(product["rawValue"], original["rawValue"]);
                assert_eq!(product.get("enumCode"), original.get("enumCode"));
            }
            let (normalized, _) = native_privacy_stream(Document::Normalized, &source, policy);
            let fields = normalized["sensors"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|sensor| sensor["fields"].as_array().unwrap())
                .collect::<Vec<_>>();
            let creator = fields
                .iter()
                .find(|field| field["fieldNumber"] == 0 && field["rawValue"] == 0)
                .unwrap();
            assert_eq!(
                creator["value"],
                get_field_variant_as_string(FieldDataType::DeviceIndex, 0)
            );
            assert_eq!(creator["enumCode"], 0);
            assert_eq!(
                creator["sourceReference"],
                creator_source["sourceReference"]
            );
            assert!(fields.iter().any(|field| field["fieldNumber"] == 0
                && field["value"] == 1
                && field["rawValue"] == 1));
            assert!(
                !fields
                    .iter()
                    .any(|field| field["fieldNumber"] == 0 && field["value"].is_array())
            );
            for original in &products {
                let product = fields
                    .iter()
                    .find(|field| field["sourceReference"] == original["sourceReference"])
                    .unwrap();
                assert_eq!(product["value"], original["value"]);
                assert_eq!(product["rawValue"], original["rawValue"]);
                if original["value"].is_string() {
                    assert_eq!(product["enumCode"], original["rawValue"]);
                }
            }
            assert!(
                omissions
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|row| row["pathPattern"] == "activities.*.decoded.messages.*.fields.*")
            );
        }
    }
}

#[test]
fn native_privacy_declared_enum_array_keeps_codes_null_positions_and_lineage() {
    use garmin_fit_extractor_api::{fit::runs::decode_run_to_writer, runs::privacy::Document};
    let fixture = native_privacy_fixture(&[0x40, 0, 0, 2, 0, 1, 4, 3, 0, 0, 0, 255, 1]);
    let mut archive = Vec::new();
    decode_run_to_writer(&fixture, &mut archive).unwrap();
    let source: Value = serde_json::from_slice(&archive).unwrap();
    let original = &source["decoded"]["messages"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()["fields"][0];
    assert_eq!(original["value"], json!([0, null, 1]));
    assert_eq!(original["rawValue"], json!([0, null, 1]));
    for include_location in [false, true] {
        for include_device_identifiers in [false, true] {
            let policy = Policy {
                include_location,
                include_device_identifiers,
            };
            let (safe, _) = native_privacy_stream(Document::Decoded, &source["decoded"], policy);
            let field = &safe["messages"].as_array().unwrap().last().unwrap()["fields"][0];
            assert_eq!(field["value"], original["value"]);
            assert_eq!(field["rawValue"], original["rawValue"]);
            assert_eq!(field["sourceReference"], original["sourceReference"]);
            assert!(
                field.get("enumCode").is_none(),
                "producer did not invent scalar enumCode for arrays"
            );
        }
    }
}

#[test]
fn native_privacy_scalar_array_definitions_do_not_authorize_fields_or_references() {
    use garmin_fit_extractor_api::{fit::runs::decode_run_to_writer, runs::privacy::Document};
    let mut extra = vec![0x40, 0, 0, 20, 0, 3, 253, 4, 134, 3, 3, 2, 7, 6, 132, 0];
    extra.extend_from_slice(&1_000_000_000u32.to_le_bytes());
    extra.extend_from_slice(&[0, 255, 151]);
    for power in [0u16, u16::MAX, 250] {
        extra.extend_from_slice(&power.to_le_bytes());
    }
    // Correct UInt32z serial wire; this is a profile-shape violation, not a type violation.
    extra.extend_from_slice(&[0x40, 0, 0, 23, 0, 1, 3, 8, 140, 0]);
    for serial in [44444444u32, 55555555] {
        extra.extend_from_slice(&serial.to_le_bytes());
    }
    let fixture = native_privacy_fixture(&extra);
    let mut archive = Vec::new();
    decode_run_to_writer(&fixture, &mut archive).unwrap();
    let source: Value = serde_json::from_slice(&archive).unwrap();
    let messages = source["decoded"]["messages"].as_array().unwrap();
    let record = &messages[messages.len() - 2];
    assert!(
        record["fields"]
            .as_array()
            .unwrap()
            .iter()
            .any(|field| field["fieldNumber"] == 3
                && field["value"] == json!([0, null, 151])
                && field["baseType"] == 2)
    );
    assert!(
        record["fields"]
            .as_array()
            .unwrap()
            .iter()
            .any(|field| field["fieldNumber"] == 7
                && field["value"] == json!([0, null, 250])
                && field["baseType"] == 132)
    );
    assert_eq!(
        messages.last().unwrap()["fields"][0]["value"],
        json!([44444444, 55555555])
    );
    assert_eq!(messages.last().unwrap()["fields"][0]["baseType"], 140);
    let sample = source["normalized"]["samples"]
        .as_array()
        .unwrap()
        .last()
        .unwrap();
    assert_eq!(sample["heartRateBpm"], Value::Null);
    assert_eq!(sample["powerWatts"], Value::Null);
    assert!(sample["sourceReferences"].get("heartRateBpm").is_none());
    assert!(sample["sourceReferences"].get("powerWatts").is_none());
    let sensor = source["normalized"]["sensors"]
        .as_array()
        .unwrap()
        .last()
        .unwrap();
    assert_eq!(sensor["serialNumber"], Value::Null);
    assert!(sensor["sourceReferences"].get("serialNumber").is_none());
    for include_location in [false, true] {
        for include_device_identifiers in [false, true] {
            let policy = Policy {
                include_location,
                include_device_identifiers,
            };
            let (safe, _) = native_privacy_stream(Document::Decoded, &source["decoded"], policy);
            let messages = safe["messages"].as_array().unwrap();
            assert!(
                messages[messages.len() - 2]["fields"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|field| field["fieldNumber"] != 3 && field["fieldNumber"] != 7)
            );
            assert_eq!(messages.last().unwrap()["fields"], json!([]));
            let (safe, _) = native_privacy_stream(Document::Normalized, &source, policy);
            assert!(
                safe["sensors"].as_array().unwrap().last().unwrap()["fields"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|field| field["fieldNumber"] != 3)
            );
        }
    }
}

#[test]
fn native_privacy_unknown_uint64_archive_is_exact_but_never_promoted() {
    use garmin_fit_extractor_api::{fit::runs::decode_run_to_writer, runs::privacy::Document};
    let mut extra = vec![0x40, 0, 0, 20, 0, 1, 250, 8, 143, 0];
    extra.extend_from_slice(&(u64::MAX - 1).to_le_bytes());
    let fixture = native_privacy_fixture(&extra);
    let original = fixture.clone();
    let mut archive = Vec::new();
    decode_run_to_writer(&fixture, &mut archive).unwrap();
    assert_eq!(fixture, original);
    let source: Value = serde_json::from_slice(&archive).unwrap();
    let field = &source["decoded"]["messages"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()["fields"][0];
    assert_eq!(field["value"], "18446744073709551614");
    assert_eq!(field["rawValue"], "18446744073709551614");
    assert_eq!(field["sourceReference"]["byteLength"], 8);
    for include_location in [false, true] {
        for include_device_identifiers in [false, true] {
            let policy = Policy {
                include_location,
                include_device_identifiers,
            };
            for (document, key) in [
                (Document::Decoded, "decoded"),
                (Document::Normalized, "normalized"),
            ] {
                let input = if key == "decoded" {
                    &source[key]
                } else {
                    &source
                };
                let (safe, omissions) = native_privacy_stream(document, input, policy);
                assert!(
                    !serde_json::to_string(&(safe, omissions))
                        .unwrap()
                        .contains("18446744073709551614")
                );
            }
        }
    }
}

#[test]
fn native_privacy_scaled_numeric_event_data_keeps_profile_value_raw_and_reference() {
    use fitparser::profile::field_types::Event;
    use garmin_fit_extractor_api::{fit::runs::decode_run_to_writer, runs::privacy::Document};
    let mut extra = vec![
        0x40,
        0,
        0,
        21,
        0,
        2,
        0,
        1,
        0,
        3,
        4,
        134,
        0,
        Event::Battery.as_i64() as u8,
    ];
    extra.extend_from_slice(&12000u32.to_le_bytes());
    let fixture = native_privacy_fixture(&extra);
    let mut archive = Vec::new();
    decode_run_to_writer(&fixture, &mut archive).unwrap();
    let source: Value = serde_json::from_slice(&archive).unwrap();
    let original = source["decoded"]["messages"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|field| field["fieldNumber"] == 3)
        .unwrap();
    assert_eq!(original["value"], 12.0);
    assert_eq!(original["rawValue"], 12000);
    let (decoded, _) =
        native_privacy_stream(Document::Decoded, &source["decoded"], Policy::default());
    let field = decoded["messages"].as_array().unwrap().last().unwrap()["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|field| field["fieldNumber"] == 3)
        .unwrap();
    assert_eq!(field["value"], 12.0);
    assert_eq!(field["rawValue"], 12000);
    assert_eq!(field["sourceReference"], original["sourceReference"]);
    let (normalized, _) = native_privacy_stream(Document::Normalized, &source, Policy::default());
    let field = normalized["extensions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|field| field["identity"]["globalMessageNumber"] == 21 && field["fieldNumber"] == 3)
        .unwrap();
    assert_eq!(field["value"], 12.0);
    assert_eq!(field["rawValue"], 12000);
    assert_eq!(field["sourceReference"], original["sourceReference"]);
    assert!(
        field.get("nativeContext").is_none(),
        "dependency proof is not export payload"
    );
}

#[test]
fn native_privacy_selected_index_bound_is_logical_not_carrier_width() {
    use fitparser::profile::field_types::Event;
    use garmin_fit_extractor_api::{fit::runs::decode_run_to_writer, runs::privacy::Document};
    let mut extra = vec![0x40, 0, 0, 21, 0, 2, 0, 1, 0, 3, 4, 134];
    let codes = [777u32, 65535, 65536, u32::MAX];
    for code in codes {
        extra.extend_from_slice(&[0, Event::CoursePoint.as_i64() as u8]);
        extra.extend_from_slice(&code.to_le_bytes());
    }
    let fixture = native_privacy_fixture(&extra);
    let mut archive = Vec::new();
    decode_run_to_writer(&fixture, &mut archive).unwrap();
    let source: Value = serde_json::from_slice(&archive).unwrap();
    let messages = source["decoded"]["messages"].as_array().unwrap();
    let originals = &messages[messages.len() - codes.len()..];
    for (message, code) in originals.iter().zip(codes) {
        let field = message["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|field| field["fieldNumber"] == 3)
            .unwrap();
        assert_eq!(field["baseType"], 134, "physical carrier is UInt32");
        assert_eq!(field["profileType"], "message_index");
        assert_eq!(
            field["value"],
            if code == u32::MAX {
                Value::Null
            } else {
                json!(code)
            }
        );
    }
    for include_location in [false, true] {
        for include_device_identifiers in [false, true] {
            let policy = Policy {
                include_location,
                include_device_identifiers,
            };
            let (safe, _) = native_privacy_stream(Document::Decoded, &source["decoded"], policy);
            let messages = safe["messages"].as_array().unwrap();
            for ((message, original), code) in messages[messages.len() - codes.len()..]
                .iter()
                .zip(originals)
                .zip(codes)
            {
                let field = message["fields"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|field| field["fieldNumber"] == 3);
                if [65535, 65536].contains(&code) {
                    assert!(
                        field.is_none(),
                        "logical UInt16 index cannot accept carrier-width value{code}"
                    );
                } else {
                    let field = field.unwrap();
                    let original = original["fields"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|field| field["fieldNumber"] == 3)
                        .unwrap();
                    assert_eq!(field["value"], original["value"]);
                    assert_eq!(field["rawValue"], original["rawValue"]);
                    assert_eq!(field["sourceReference"], original["sourceReference"]);
                }
            }
            let (safe, _) = native_privacy_stream(Document::Normalized, &source, policy);
            let fields = safe["extensions"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|field| {
                    field["identity"]["globalMessageNumber"] == 21 && field["fieldNumber"] == 3
                })
                .collect::<Vec<_>>();
            assert_eq!(fields.len(), 2);
            assert!(
                fields
                    .iter()
                    .any(|field| field["value"] == 777 && field["rawValue"] == 777)
            );
            assert!(
                fields
                    .iter()
                    .any(|field| field["value"].is_null() && field["rawValue"].is_null())
            );
            assert!(
                fields
                    .iter()
                    .all(|field| field["value"] != 65535 && field["value"] != 65536)
            );
        }
    }
}

#[test]
fn native_privacy_normalization_never_promotes_wrong_numeric_wire_into_samples() {
    use garmin_fit_extractor_api::{fit::runs::decode_run_to_writer, runs::privacy::Document};
    let mut extra = vec![0x40, 0, 0, 20, 0, 3, 253, 4, 134, 3, 8, 137, 7, 8, 137, 0];
    extra.extend_from_slice(&1_000_000_000u32.to_le_bytes());
    extra.extend_from_slice(&151.0f64.to_le_bytes());
    extra.extend_from_slice(&250.0f64.to_le_bytes());
    let fixture = native_privacy_fixture(&extra);
    let original = fixture.clone();
    let mut archive = Vec::new();
    decode_run_to_writer(&fixture, &mut archive).unwrap();
    assert_eq!(fixture, original);
    let source: Value = serde_json::from_slice(&archive).unwrap();
    let original = source["decoded"]["messages"]
        .as_array()
        .unwrap()
        .last()
        .unwrap();
    assert!(
        original["fields"]
            .as_array()
            .unwrap()
            .iter()
            .any(|field| field["fieldNumber"] == 3
                && field["value"] == 151.0
                && field["baseType"] == 137)
    );
    assert!(
        original["fields"]
            .as_array()
            .unwrap()
            .iter()
            .any(|field| field["fieldNumber"] == 7
                && field["value"] == 250.0
                && field["baseType"] == 137)
    );
    let sample = source["normalized"]["samples"]
        .as_array()
        .unwrap()
        .last()
        .unwrap();
    assert_eq!(sample["heartRateBpm"], Value::Null);
    assert_eq!(sample["powerWatts"], Value::Null);
    assert!(sample["sourceReferences"].get("heartRateBpm").is_none());
    assert!(sample["sourceReferences"].get("powerWatts").is_none());
    for include_location in [false, true] {
        for include_device_identifiers in [false, true] {
            let policy = Policy {
                include_location,
                include_device_identifiers,
            };
            let (safe, _) = native_privacy_stream(Document::Decoded, &source["decoded"], policy);
            assert!(
                safe["messages"].as_array().unwrap().last().unwrap()["fields"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|field| field["fieldNumber"] != 3 && field["fieldNumber"] != 7)
            );
            let (safe, _) = native_privacy_stream(Document::Normalized, &source, policy);
            let sample = safe["samples"].as_array().unwrap().last().unwrap();
            assert_eq!(sample["heartRateBpm"], Value::Null);
            assert_eq!(sample["powerWatts"], Value::Null);
        }
    }
}

#[test]
fn native_privacy_compressed_sensor_timestamp_keeps_native_extension_lineage() {
    use garmin_fit_extractor_api::{fit::runs::decode_run_to_writer, runs::privacy::Document};
    let fixture = native_privacy_fixture(&[0x40, 0, 0, 23, 0, 1, 0, 1, 2, 0x82, 1]);
    let mut archive = Vec::new();
    decode_run_to_writer(&fixture, &mut archive).unwrap();
    let source: Value = serde_json::from_slice(&archive).unwrap();
    let original = source["decoded"]["messages"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|field| field["fieldNumber"] == 253 && field["role"] == "reconstructed")
        .unwrap();
    assert_eq!(original["sourceReference"]["byteLength"], 1);
    for (document, key) in [
        (Document::Decoded, "decoded"),
        (Document::Normalized, "normalized"),
    ] {
        let input = if key == "decoded" {
            &source[key]
        } else {
            &source
        };
        let (safe, _) = native_privacy_stream(document, input, Policy::default());
        let fields = if key == "decoded" {
            safe["messages"].as_array().unwrap().last().unwrap()["fields"]
                .as_array()
                .unwrap()
        } else {
            safe["sensors"].as_array().unwrap().last().unwrap()["fields"]
                .as_array()
                .unwrap()
        };
        let field = fields
            .iter()
            .find(|field| field["fieldNumber"] == 253)
            .unwrap();
        assert_eq!(field["value"], original["value"]);
        assert_eq!(field["rawValue"], Value::Null);
        assert_eq!(field["sourceReference"], original["sourceReference"]);
    }
}

#[test]
fn native_privacy_developer_dependency_collision_does_not_suppress_native_event_data() {
    use fitparser::profile::field_types::Event;
    use garmin_fit_extractor_api::{fit::runs::decode_run_to_writer, runs::privacy::Document};
    let name = b"event-dependency-private-name\0";
    let canary = b"event-dependency-private-value\0";
    // Real FIT developer metadata precedes the native/developer field-zero collision.
    let mut extra = vec![0x40, 0, 0, 207, 0, 2, 1, 16, 13, 3, 1, 2, 0];
    extra.extend_from_slice(&[42; 16]);
    extra.push(0);
    extra.extend_from_slice(&[
        0x40,
        0,
        0,
        206,
        0,
        4,
        0,
        1,
        2,
        1,
        1,
        2,
        2,
        1,
        2,
        3,
        name.len() as u8,
        7,
        0,
        0,
        0,
        7,
    ]);
    extra.extend_from_slice(name);
    extra.extend_from_slice(&[
        0x60,
        0,
        0,
        21,
        0,
        2,
        0,
        1,
        0,
        3,
        4,
        134,
        1,
        0,
        canary.len() as u8,
        0,
        0,
        Event::Battery.as_i64() as u8,
    ]);
    extra.extend_from_slice(&12000u32.to_le_bytes());
    extra.extend_from_slice(canary);
    let fixture = native_privacy_fixture(&extra);
    let original_fixture = fixture.clone();
    let mut archive = Vec::new();
    decode_run_to_writer(&fixture, &mut archive).unwrap();
    assert_eq!(fixture, original_fixture);
    let source: Value = serde_json::from_slice(&archive).unwrap();
    let fields = source["decoded"]["messages"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()["fields"]
        .as_array()
        .unwrap();
    let native = fields
        .iter()
        .find(|field| field["fieldNumber"] == 0 && field["developerIdentity"].is_null())
        .unwrap();
    assert_eq!(native["rawValue"], Event::Battery.as_i64());
    let developer = fields
        .iter()
        .find(|field| field["fieldNumber"] == 0 && !field["developerIdentity"].is_null())
        .unwrap();
    assert_eq!(developer["developerIdentity"]["developerDataIndex"], 0);
    assert_eq!(developer["name"], "event-dependency-private-name");
    assert_eq!(developer["value"], "event-dependency-private-value");
    let original = fields
        .iter()
        .find(|field| field["fieldNumber"] == 3 && field["developerIdentity"].is_null())
        .unwrap();
    assert_eq!(original["value"], 12.0);
    assert_eq!(original["rawValue"], 12000);
    for include_location in [false, true] {
        for include_device_identifiers in [false, true] {
            let policy = Policy {
                include_location,
                include_device_identifiers,
            };
            for (document, key) in [
                (Document::Decoded, "decoded"),
                (Document::Normalized, "normalized"),
            ] {
                let input = if key == "decoded" {
                    &source[key]
                } else {
                    &source
                };
                let (safe, omissions) = native_privacy_stream(document, input, policy);
                let field = if key == "decoded" {
                    safe["messages"].as_array().unwrap().last().unwrap()["fields"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|field| field["fieldNumber"] == 3)
                        .unwrap()
                } else {
                    safe["extensions"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|field| {
                            field["identity"]["globalMessageNumber"] == 21
                                && field["fieldNumber"] == 3
                        })
                        .unwrap()
                };
                assert_eq!(field["value"], 12.0);
                assert_eq!(field["rawValue"], 12000);
                assert_eq!(field["sourceReference"], original["sourceReference"]);
                assert!(
                    field.get("nativeContext").is_none(),
                    "dependency proof must not enter the export payload"
                );
                let mut values = vec![&safe, &omissions];
                while let Some(value) = values.pop() {
                    match value {
                        Value::Object(object) => {
                            assert!(
                                object.get("developerIdentity").is_none_or(Value::is_null),
                                "leaked non-null developer identity with location={include_location}, device={include_device_identifiers}, document={key}"
                            );
                            values.extend(object.values());
                        }
                        Value::Array(array) => values.extend(array),
                        _ => {}
                    }
                }
                let output = serde_json::to_string(&(safe, omissions)).unwrap();
                for secret in [
                    "event-dependency-private-name",
                    "event-dependency-private-value",
                ] {
                    assert!(
                        !output.contains(secret),
                        "leaked {secret} with location={include_location}, device={include_device_identifiers}, document={key}"
                    );
                }
            }
        }
    }
}

#[test]
fn native_privacy_expanded_hr_timestamp_rollover_keeps_uint64_counter_and_lineage() {
    use garmin_fit_extractor_api::{fit::runs::decode_run_to_writer, runs::privacy::Document};
    // CC0 HR wire data: field 9 seeds the SDK's native anchor before field 10.
    // Its first 12-bit counter is zero, so 4294967294 rolls over to 4294967296.
    // The two-byte parent contains one complete counter; absent counters are invalid.
    let mut extra = vec![0x40, 0, 0, 132, 0, 2, 9, 4, 134, 10, 2, 13, 0];
    extra.extend_from_slice(&4_294_967_294u32.to_le_bytes());
    extra.extend_from_slice(&[0, 0]);
    let fixture = native_privacy_fixture(&extra);
    let original_fixture = fixture.clone();
    let mut archive = Vec::new();
    decode_run_to_writer(&fixture, &mut archive).unwrap();
    assert_eq!(fixture, original_fixture);
    let source: Value = serde_json::from_slice(&archive).unwrap();
    let original_archive = source.clone();
    let message = source["decoded"]["messages"]
        .as_array()
        .unwrap()
        .last()
        .unwrap();
    assert_eq!(message["globalMessageNumber"], 132);
    let fields = message["fields"].as_array().unwrap();
    let anchor = fields
        .iter()
        .find(|field| field["fieldNumber"] == 9 && field["role"] == "native")
        .unwrap();
    assert_eq!(anchor["baseType"], 134);
    assert_eq!(anchor["rawValue"], 4_294_967_294u64);
    assert_eq!(anchor["value"], 4_294_967_294.0 / 1024.0);
    let parent = fields
        .iter()
        .find(|field| field["fieldNumber"] == 10)
        .unwrap();
    assert_eq!(parent["baseType"], 13);
    assert_eq!(parent["rawValue"], json!([0, 0]));
    let original = fields
        .iter()
        .find(|field| field["fieldNumber"] == 9 && field["role"] == "expanded")
        .unwrap();
    assert_eq!(original["value"], json!([4_194_304.0]));
    assert_eq!(original["rawValue"], Value::Null);
    assert_eq!(original["baseType"], "uint32");
    assert_eq!(original["scale"], 1024.0);
    assert_eq!(original["offset"], 0.0);
    assert_eq!(original["componentParent"], 10);
    let counter = original["value"][0].as_f64().unwrap() * 1024.0;
    assert_eq!(counter, 4_294_967_296.0);
    assert!(
        counter > f64::from(u32::MAX),
        "the expanded counter must cross the native UInt32 bound"
    );
    assert_eq!(
        original["sourceReference"],
        json!({
            "messageIndex":message["index"],"globalMessageNumber":132,"fieldNumber":9,
            "componentParent":10,"byteOffset":fixture.len()-4,"byteLength":2
        })
    );
    assert_eq!(
        original["sourceReference"]["byteOffset"],
        parent["sourceReference"]["byteOffset"]
    );
    let mut interval_reference = original["sourceReference"].clone();
    interval_reference["arrayIndex"] = json!(0);
    let interval = source["normalized"]["rr"]["intervals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|interval| interval["sourceReference"] == interval_reference)
        .unwrap();
    assert_eq!(interval["rrMs"], 1.953125);
    assert_eq!(
        interval["rrMs"],
        (original["value"][0].as_f64().unwrap() - anchor["value"].as_f64().unwrap()) * 1000.0
    );
    assert_eq!(interval["provenance"]["messageNumber"], 132);
    assert_eq!(interval["provenance"]["fieldNumber"], 9);
    for include_location in [false, true] {
        for include_device_identifiers in [false, true] {
            let policy = Policy {
                include_location,
                include_device_identifiers,
            };
            let (decoded, _) = native_privacy_stream(Document::Decoded, &source["decoded"], policy);
            let actual = decoded["messages"].as_array().unwrap().last().unwrap()["fields"]
                .as_array()
                .unwrap()
                .iter()
                .find(|field| field["fieldNumber"] == 9 && field["role"] == "expanded")
                .unwrap();
            for attribute in [
                "value",
                "rawValue",
                "unit",
                "validity",
                "role",
                "componentParent",
                "sourceReference",
                "baseType",
                "profileType",
                "scale",
                "offset",
            ] {
                assert_eq!(
                    actual[attribute], original[attribute],
                    "lost decoded {attribute} with location={include_location}, device={include_device_identifiers}"
                );
            }
            let (normalized, _) = native_privacy_stream(Document::Normalized, &source, policy);
            let actual_interval = normalized["rr"]["intervals"]
                .as_array()
                .unwrap()
                .iter()
                .find(|interval| interval["sourceReference"] == interval_reference)
                .unwrap();
            for attribute in [
                "rrMs",
                "sourceReference",
                "timestamp",
                "elapsedSeconds",
                "startElapsedSeconds",
                "endElapsedSeconds",
                "timingEligible",
            ] {
                assert_eq!(
                    actual_interval[attribute], interval[attribute],
                    "lost normalized RR {attribute} with location={include_location}, device={include_device_identifiers}"
                );
            }
            for attribute in [
                "kind",
                "messageNumber",
                "fieldNumber",
                "timingMethod",
                "anchorTimestamp",
                "limitations",
            ] {
                assert_eq!(
                    actual_interval["provenance"][attribute],
                    interval["provenance"][attribute]
                );
            }
            for safe in [&decoded, &normalized] {
                assert!(
                    !serde_json::to_string(safe)
                        .unwrap()
                        .contains("\"nativeContext\""),
                    "dependency proof must not enter the export payload"
                );
            }
        }
    }
    assert_eq!(
        source, original_archive,
        "privacy projection must not mutate the archive"
    );
}

#[test]
fn native_privacy_wrong_wire_packed_parent_never_authorizes_components() {
    use garmin_fit_extractor_api::{fit::runs::decode_run_to_writer, runs::privacy::Document};
    // Both packed parents require Byte wire data, not UInt64. The SDK still
    // expands their bytes, so consumers must verify the actual physical parent.
    let mut extra = vec![0x40, 0, 0, 20, 0, 2, 253, 4, 134, 8, 8, 143, 0];
    extra.extend_from_slice(&1_000_000_000u32.to_le_bytes());
    extra.extend_from_slice(&0x1234_5678_9abcu64.to_le_bytes());
    extra.extend_from_slice(&[0x40, 0, 0, 132, 0, 2, 9, 4, 134, 10, 8, 143, 0]);
    extra.extend_from_slice(&4_294_967_294u32.to_le_bytes());
    extra.extend_from_slice(&0u64.to_le_bytes());
    let fixture = native_privacy_fixture(&extra);
    let original_fixture = fixture.clone();
    let mut archive = Vec::new();
    decode_run_to_writer(&fixture, &mut archive).unwrap();
    assert_eq!(fixture, original_fixture);
    let source: Value = serde_json::from_slice(&archive).unwrap();
    let original_archive = source.clone();
    let messages = source["decoded"]["messages"].as_array().unwrap();
    let record = messages
        .iter()
        .rev()
        .find(|message| message["globalMessageNumber"] == 20)
        .unwrap();
    let record_fields = record["fields"].as_array().unwrap();
    let record_parent = record_fields
        .iter()
        .find(|field| field["fieldNumber"] == 8)
        .unwrap();
    assert_eq!(record_parent["baseType"], 143);
    assert_eq!(record_parent["rawValue"], 0x1234_5678_9abcu64);
    for number in [5, 6, 73] {
        let field = record_fields
            .iter()
            .find(|field| field["fieldNumber"] == number && field["role"] == "expanded")
            .unwrap();
        assert!(
            field["value"]
                .as_f64()
                .is_some_and(|value| value.is_finite())
        );
        assert_eq!(field["componentParent"], 8);
        assert_eq!(field["sourceReference"]["componentParent"], 8);
        assert_eq!(
            field["sourceReference"]["byteOffset"],
            record_parent["sourceReference"]["byteOffset"]
        );
        assert_eq!(field["sourceReference"]["byteLength"], 8);
    }
    let speed = record_fields
        .iter()
        .find(|field| field["fieldNumber"] == 6 && field["role"] == "expanded")
        .unwrap();
    assert_eq!(speed["value"], f64::from(0xabcu16) / 100.0);
    let hr = messages.last().unwrap();
    assert_eq!(hr["globalMessageNumber"], 132);
    let hr_fields = hr["fields"].as_array().unwrap();
    let hr_parent = hr_fields
        .iter()
        .find(|field| field["fieldNumber"] == 10)
        .unwrap();
    assert_eq!(hr_parent["baseType"], 143);
    assert_eq!(hr_parent["rawValue"], 0);
    let native = hr_fields
        .iter()
        .find(|field| field["fieldNumber"] == 9 && field["role"] == "native")
        .unwrap();
    assert_eq!(native["baseType"], 134);
    assert_eq!(native["rawValue"], 4_294_967_294u64);
    let expanded = hr_fields
        .iter()
        .find(|field| field["fieldNumber"] == 9 && field["role"] == "expanded")
        .unwrap();
    assert_eq!(expanded["value"][0], 4_194_304.0);
    assert_eq!(expanded["componentParent"], 10);
    assert_eq!(expanded["sourceReference"]["componentParent"], 10);
    assert_eq!(
        expanded["sourceReference"]["byteOffset"],
        hr_parent["sourceReference"]["byteOffset"]
    );
    assert_eq!(expanded["sourceReference"]["byteLength"], 8);
    let rejected_references = record_fields
        .iter()
        .chain(hr_fields)
        .filter(|field| field["role"] == "expanded")
        .map(|field| field["sourceReference"].clone())
        .collect::<Vec<_>>();
    let sample = source["normalized"]["samples"]
        .as_array()
        .unwrap()
        .last()
        .unwrap();
    for metric in ["speedMps", "distanceMeters"] {
        assert_eq!(sample[metric], Value::Null);
        assert!(sample["sourceReferences"].get(metric).is_none());
    }
    assert!(
        source["normalized"]["rr"]["intervals"]
            .as_array()
            .unwrap()
            .iter()
            .all(
                |interval| interval["sourceReference"]["messageIndex"] != hr["index"]
                    || interval["sourceReference"]["componentParent"] != 10
            ),
        "wrong physical HR parent must not supply RR event counters"
    );
    for include_location in [false, true] {
        for include_device_identifiers in [false, true] {
            let policy = Policy {
                include_location,
                include_device_identifiers,
            };
            let (decoded, _) = native_privacy_stream(Document::Decoded, &source["decoded"], policy);
            let projected_messages = decoded["messages"].as_array().unwrap();
            for original in [record, hr] {
                let projected = projected_messages
                    .iter()
                    .find(|message| message["index"] == original["index"])
                    .unwrap();
                assert!(
                    projected["fields"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|field| field["role"] != "expanded"
                            && field["fieldNumber"]
                                != if original["globalMessageNumber"] == 20 {
                                    8
                                } else {
                                    10
                                }),
                    "wrong physical packed parent and its children must not enter Full decoded output"
                );
            }
            let projected_hr = projected_messages
                .iter()
                .find(|message| message["index"] == hr["index"])
                .unwrap();
            let projected_native = projected_hr["fields"]
                .as_array()
                .unwrap()
                .iter()
                .find(|field| field["fieldNumber"] == 9)
                .unwrap();
            for attribute in ["value", "rawValue", "role", "sourceReference"] {
                assert_eq!(
                    projected_native[attribute], native[attribute],
                    "legitimate native HR anchor must survive"
                );
            }
            let (normalized, _) = native_privacy_stream(Document::Normalized, &source, policy);
            let extensions = normalized["extensions"].as_array().unwrap();
            assert!(
                extensions
                    .iter()
                    .all(|field| !rejected_references.contains(&field["sourceReference"])),
                "wrong physical packed parent must not authorize normalized component extensions"
            );
            let sample = normalized["samples"].as_array().unwrap().last().unwrap();
            for metric in ["speedMps", "distanceMeters"] {
                assert_eq!(sample[metric], Value::Null);
                assert!(sample["sourceReferences"].get(metric).is_none());
            }
            assert!(
                normalized["rr"]["intervals"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(
                        |interval| interval["sourceReference"]["messageIndex"] != hr["index"]
                            || interval["sourceReference"]["componentParent"] != 10
                    )
            );
            assert!(
                !serde_json::to_string(&normalized)
                    .unwrap()
                    .contains("\"nativeContext\"")
            );
        }
    }
    assert_eq!(
        source, original_archive,
        "privacy projection must not mutate the archive"
    );
}
