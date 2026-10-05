use garmin_fit_extractor_api::runs::privacy::{Policy, project};
use serde_json::json;

#[test]
fn numeric_identity_and_recursive_schema_keep_metrics_not_canaries() {
    let activity = json!({
        "id":"selected", "decoded":{"schemaVersion":"2.0.0","messages":[
            {"index":0,"globalMessageNumber":20,"localMessageNumber":0,"fields":[
                {"fieldNumber":3,"name":"heart_rate","value":151,"classification":"metric"},
                {"fieldNumber":0,"name":"heart_rate","value":123456789,"classification":"metric"},
                {"fieldNumber":3,"name":"heart_rate","value":987654321,"developerIdentity":{"developerDataIndex":0,"fieldDefinitionNumber":3},"classification":"metric"},
                {"fieldNumber":250,"name":"heart_rate","value":"opaque-canary","classification":"metric"}
            ]}
        ]},
        "normalized":{"schemaVersion":"2.0.0","summary":{"distanceMeters":1000,"device":{"serial":"serial-canary"}},"samples":[{"index":0,"elapsedSeconds":0,"heartRateBpm":151,"latitude":55.4321,"unknown":{"heartRateBpm":"nested-canary"}}],"extensions":[{"value":"extension-canary"}],"filename":"private-canary.fit"},
        "analysis":{"quality":{"usableDurationSeconds":42,"unknown":{"value":"analysis-canary"}}},
        "historicalThresholds":{"lt1":{"status":"low_confidence","value":{"heartRateBpm":150},"evidence":[{"activityId":"unselected-canary","samples":[{"heartRateBpm":199}]}]}}
    });
    let (safe, omissions) = project(&activity, Policy::default());
    assert_eq!(safe["decoded"]["messages"][0]["fields"][0]["value"], 151);
    assert_eq!(safe["normalized"]["summary"]["distanceMeters"], 1000);
    assert_eq!(safe["normalized"]["samples"][0]["heartRateBpm"], 151);
    assert_eq!(safe["historicalThresholds"]["lt1"]["value"]["heartRateBpm"], 150);
    let output = serde_json::to_string(&(safe, omissions)).unwrap();
    for canary in ["123456789", "987654321", "opaque-canary", "serial-canary", "55.4321", "nested-canary", "extension-canary", "private-canary", "analysis-canary", "unselected-canary"] {
        assert!(!output.contains(canary), "leaked {canary}");
    }
}

#[test]
fn location_consent_never_promotes_identifiers_or_compound_developer_sources() {
    let activity=json!({"decoded":{"messages":[
        {"globalMessageNumber":20,"fields":[{"fieldNumber":0,"name":"private-label-canary.fit","value":123456789,"classification":"metric"},
            {"fieldNumber":3,"value":987654321,"sourceReference":{"globalMessageNumber":20,"fieldNumber":3,"developerIdentity":{"developerDataIndex":4,"fieldDefinitionNumber":3}}}]},
        {"globalMessageNumber":23,"fields":[{"fieldNumber":3,"value":444444444}]}
    ]},"normalized":{"samples":[{"heartRateBpm":777777777,"latitudeDegrees":55.4321,"sourceReferences":{"heartRateBpm":{"globalMessageNumber":20,"fieldNumber":0},"latitudeDegrees":{"globalMessageNumber":20,"fieldNumber":0}}}],"opaque":{"value":"unknown-canary"}}});
    let (safe,omissions)=project(&activity,Policy{include_location:true,include_device_identifiers:false});
    assert_eq!(safe["decoded"]["messages"][0]["fields"][0]["value"],123456789);
    assert_eq!(safe["normalized"]["samples"][0]["latitudeDegrees"],55.4321);
    let output=serde_json::to_string(&(safe,omissions)).unwrap();
    for secret in ["private-label-canary","987654321","444444444","777777777","unknown-canary"] {assert!(!output.contains(secret),"leaked {secret}");}
}

use garmin_fit_extractor_api::runs::export::{ExportMode,ExportRequest,snapshot};
use serde_json::Value;
use uuid::Uuid;

fn request(id:Uuid,mode:ExportMode)->ExportRequest {
    ExportRequest{activity_ids:vec![id],mode,include_location:false,include_device_identifiers:false}
}
fn activity(id:Uuid,samples:Vec<Value>)->Value {
    json!({"id":id,"decoded":{"schemaVersion":"2.0.0","messages":[]},"normalized":{"schemaVersion":"2.0.0","startTime":"2026-01-01T00:00:00Z","endTime":"2026-01-01T01:00:00Z","samples":samples,"laps":[],"timerEvents":[],"summary":{"distanceMeters":1234}},"analysis":{"schemaVersion":"2.0.0","quality":{"sampleCount":5},"segments":[],"thresholds":{"lt1":{"status":"low_confidence","method":{"id":"running-dfa-a1-075","version":"real-version"},"value":{"heartRateBpm":150},"evidence":{"activityId":"unselected-canary","independentActivityCount":1},"trace":{"crossing":150,"regression":{"slope":-0.01,"intercept":2.25},"windows":[{"samples":["trace-canary"]}]}}},"transformations":[]},"historicalThresholds":garmin_fit_extractor_api::runs::thresholds::estimate_history("2026-01-01T01:00:00Z",&[])})
}

#[test]
fn full_snapshot_keeps_all_100000_samples_values_order_and_exact_bytes() {
    let id=Uuid::new_v4();
    let samples=(0..100000).map(|index|json!({"index":index,"elapsedSeconds":index,"heartRateBpm":100+index%80,"powerWatts":index})).collect();
    let mut original=activity(id,samples);
    original["analysis"]["thresholds"]["lt1"]["contextUnverified"]=json!(["fatigueState","treadmillCalibration"]);
    original["analysis"]["thresholds"]["lt1"]["trace"]["parameters"]=json!({"lambda":500,"maximumRrContinuityErrorSeconds":0.005,"scales":[4,5,6,7,8,9,10,11,12,13,14,15,16]});
    original["analysis"]["transformations"]=json!([{"type":"derivedWorkloadBlocks","derivedTimelineSorted":true,"originalSamplesChanged":false}]);
    let request=request(id,ExportMode::Full);
    let bytes=snapshot(&[id],&[original.clone()],&request,"2026-01-01T00:00:00Z").unwrap();
    assert_eq!(bytes.last(),Some(&b'\n'));
    assert_eq!(bytes,snapshot(&[id],&[original],&request,"2026-01-01T00:00:00Z").unwrap());
    let value:Value=serde_json::from_slice(&bytes).unwrap();
    let samples=value["activities"][0]["normalized"]["samples"].as_array().unwrap();
    assert_eq!(samples.len(),100000);
    for (index,sample) in samples.iter().enumerate() {assert_eq!(sample["powerWatts"],index);assert_eq!(sample["index"],index);}
    assert_eq!(value["activities"][0]["analysis"]["thresholds"]["lt1"]["value"]["heartRateBpm"],150);
    assert_eq!(value["activities"][0]["analysis"]["thresholds"]["lt1"]["trace"]["crossing"],150);
    assert_eq!(value["activities"][0]["analysis"]["thresholds"]["lt1"]["contextUnverified"],json!(["fatigueState","treadmillCalibration"]));
    assert_eq!(value["activities"][0]["analysis"]["thresholds"]["lt1"]["trace"]["parameters"]["maximumRrContinuityErrorSeconds"],0.005);
    assert_eq!(value["activities"][0]["analysis"]["transformations"][0]["derivedTimelineSorted"],true);
    let text=String::from_utf8(bytes).unwrap();
    assert!(!text.contains("unselected-canary"));assert!(!text.contains("trace-canary"));
}

#[test]
fn explicit_invalid_selection_never_materializes_remaining_activities() {
    let id=Uuid::new_v4();let other=Uuid::new_v4();
    let mut selected=request(id,ExportMode::Full);selected.activity_ids.push(other);
    assert!(snapshot(&[id,other],&[activity(id,vec![])],&selected,"2026-01-01T00:00:00Z").is_err());
    selected.activity_ids=vec![id,id];
    assert!(snapshot(&[id,id],&[activity(id,vec![]),activity(id,vec![])],&selected,"2026-01-01T00:00:00Z").is_err());
    let mut pending=activity(id,vec![]);pending["historicalThresholds"]=Value::Null;
    assert_eq!(snapshot(&[id],&[pending],&request(id,ExportMode::Full),"2026-01-01T00:00:00Z").unwrap_err().code(),"EXPORT_NOT_READY");
    assert!(snapshot(&[id],&[activity(other,vec![])],&request(id,ExportMode::Full),"2026-01-01T00:00:00Z").is_err());
}

#[test]
fn coach_aggregation_preserves_lap_pause_gap_repeats_and_missing_metric_weights() {
    let id=Uuid::new_v4();
    let mut run=activity(id,vec![
        json!({"index":0,"elapsedSeconds":0,"heartRateBpm":100,"powerWatts":null,"timerRunning":true}),
        json!({"index":1,"elapsedSeconds":1,"heartRateBpm":200,"powerWatts":300,"timerRunning":true}),
        json!({"index":2,"elapsedSeconds":3,"heartRateBpm":null,"powerWatts":600,"timerRunning":true}),
        json!({"index":3,"elapsedSeconds":4,"heartRateBpm":180,"timerRunning":false}),
        json!({"index":4,"elapsedSeconds":20,"heartRateBpm":190,"timerRunning":true})
    ]);
    run["normalized"]["laps"]=json!([{"index":0,"startElapsedSeconds":0,"endElapsedSeconds":4}]);
    run["analysis"]["segments"]=json!([{"index":0,"kind":"repeatedsurge","startElapsedSeconds":0,"endElapsedSeconds":4},{"index":1,"kind":"repeatedsurge","startElapsedSeconds":20,"endElapsedSeconds":21}]);
    let bytes=snapshot(&[id],&[run],&request(id,ExportMode::Coach),"2026-01-01T00:00:00Z").unwrap();
    let value:Value=serde_json::from_slice(&bytes).unwrap();
    let samples=value["activities"][0]["samples"].as_array().unwrap();
    assert_eq!(samples.iter().map(|v|v["sourceCount"].as_u64().unwrap()).sum::<u64>(),5);
    assert!((samples[0]["heartRateBpm"].as_f64().unwrap()-500.0/3.0).abs()<1e-10);
    assert_eq!(samples[0]["metricCoverageSeconds"]["heartRateBpm"].as_f64(),Some(3.0));
    // Power coverage: 300 W for two seconds, then 600 W for one second.
    assert_eq!(samples[0]["powerWatts"].as_f64(),Some(400.0));
    assert_eq!(samples[0]["metricCoverageSeconds"]["powerWatts"].as_f64(),Some(3.0));
    assert_eq!(samples.len(),3);
    assert_eq!(samples[1]["startElapsedSeconds"].as_f64(),Some(4.0));
    assert_eq!(samples[1]["timerRunning"],false);
    assert_eq!(samples[2]["startElapsedSeconds"].as_f64(),Some(20.0));
    assert_eq!(value["activities"][0]["laps"][0]["endElapsedSeconds"],4);
    assert_eq!(value["activities"][0]["segments"][1]["kind"],"repeatedsurge");
    assert!(!String::from_utf8(bytes).unwrap().contains("unselected-canary"));
}

#[test]
fn both_modes_apply_independent_consents_to_nested_canaries() {
    let id=Uuid::new_v4();
    let mut run=activity(id,vec![json!({"index":0,"elapsedSeconds":0,"heartRateBpm":151,"latitude":55.4321})]);
    run["decoded"]["messages"]=json!([
        {"index":0,"globalMessageNumber":20,"fields":[
            {"fieldNumber":3,"name":"name-canary.fit","value":151},
            {"fieldNumber":0,"value":123456789},
            {"fieldNumber":3,"value":987654321,"developerIdentity":{"developerDataIndex":0,"fieldDefinitionNumber":3}},
            {"fieldNumber":251,"value":"opaque-canary"}]},
        {"index":1,"globalMessageNumber":23,"fields":[{"fieldNumber":3,"value":444444444}]}
    ]);
    run["normalized"]["laps"]=json!([{"index":0,"startElapsedSeconds":0,"endElapsedSeconds":1,"startLatitude":66.1234,"unknown":{"value":"lap-canary"}}]);
    run["normalized"]["extensions"]=json!([{"value":"extension-canary"}]);
    run["analysis"]["quality"]["unknown"]=json!({"value":"quality-canary"});
    run["historicalThresholds"]=json!({"evidenceCutoff":"2026-01-01T01:00:00Z","lt1":{"status":"low_confidence","value":{"heartRateBpm":150},"trace":{"crossing":150,"location":{"value":"history-canary"}},"evidence":{"activityId":"unselected-history-canary","sampleCount":1}}});
    for mode in [ExportMode::Coach,ExportMode::Full] {
        let bytes=snapshot(&[id],&[run.clone()],&request(id,mode),"2026-01-01T00:00:00Z").unwrap();
        let text=String::from_utf8(bytes).unwrap();
        for secret in ["123456789","987654321","444444444","55.4321","66.1234","name-canary","opaque-canary","lap-canary","extension-canary","quality-canary","history-canary"] {assert!(!text.contains(secret),"leaked {secret} in {mode:?}");}
        let mut consent=request(id,mode);consent.include_location=true;
        let text=String::from_utf8(snapshot(&[id],&[run.clone()],&consent,"2026-01-01T00:00:00Z").unwrap()).unwrap();
        for secret in ["987654321","444444444","opaque-canary","extension-canary"] {assert!(!text.contains(secret));}
        if mode==ExportMode::Full {assert!(text.contains("123456789"));assert!(text.contains("55.4321"));}
    }
}

#[test]
fn coach_splits_recorded_intervals_at_boundaries_without_discarding_time_coverage() {
    let id=Uuid::new_v4();
    let mut run=activity(id,vec![
        json!({"index":0,"elapsedSeconds":28,"heartRateBpm":100,"powerWatts":100,"timerRunning":true}),
        json!({"index":1,"elapsedSeconds":31,"heartRateBpm":200,"powerWatts":null,"timerRunning":true}),
        json!({"index":2,"elapsedSeconds":33,"heartRateBpm":300,"powerWatts":400,"timerRunning":true})
    ]);
    run["normalized"]["laps"]=json!([{"index":0,"startElapsedSeconds":0,"endElapsedSeconds":30},{"index":1,"startElapsedSeconds":30,"endElapsedSeconds":33}]);
    let bytes=snapshot(&[id],&[run],&request(id,ExportMode::Coach),"2026-01-01T00:00:00Z").unwrap();
    let value:Value=serde_json::from_slice(&bytes).unwrap();
    let samples=value["activities"][0]["samples"].as_array().unwrap();
    assert_eq!(samples[0]["endElapsedSeconds"].as_f64(),Some(30.0));
    assert_eq!(samples[0]["metricCoverageSeconds"]["heartRateBpm"].as_f64(),Some(2.0));
    assert_eq!(samples[1]["startElapsedSeconds"].as_f64(),Some(30.0));
    assert_eq!(samples[1]["metricCoverageSeconds"]["heartRateBpm"].as_f64(),Some(3.0));
    assert!((samples[1]["heartRateBpm"].as_f64().unwrap()-500.0/3.0).abs()<1e-10);
    assert_eq!(samples[1]["powerWatts"].as_f64(),Some(100.0));
}

#[tokio::test]
async fn pinned_transport_rejects_revocation_releases_drop_guard_and_matches_copy_download_bytes() {
    use futures_util::StreamExt;
    use garmin_fit_extractor_api::runs::export::{create,serve,serve_head};
    use sha2::{Digest,Sha256};
    use sqlx::{PgPool,postgres::PgPoolOptions};
    let url=std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL must point to a disposable PostgreSQL database");
    let admin=PgPool::connect(&url).await.unwrap();
    let directory:String=sqlx::query_scalar("SHOW data_directory").fetch_one(&admin).await.unwrap();
    let expected=std::env::var("PGDATA").expect("PGDATA must identify the disposable PostgreSQL directory");
    assert!(directory.starts_with("/tmp/") && directory==expected,"Refuse a non-disposable database");
    let schema=format!("runs_export_test_{}",Uuid::new_v4().simple());
    // Schema names contain only a fixed prefix and system-generated UUID hex.
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE SCHEMA {schema}"))).execute(&admin).await.unwrap();
    let connect_schema=schema.clone();
    let pool=PgPoolOptions::new().max_connections(5).after_connect(move |connection,_|{
        let statement=format!("SET search_path TO {connect_schema}");
        Box::pin(async move {sqlx::query(sqlx::AssertSqlSafe(statement)).execute(connection).await?;Ok(())})
    }).connect(&url).await.unwrap();
    for migration in [
        include_str!("../migrations/0001_extractions.sql"),
        include_str!("../migrations/0002_google_users_sessions_zip_history.sql"),
        include_str!("../migrations/0003_fit_coach.sql"),
        include_str!("../migrations/0004_transcript_entries.sql"),
        include_str!("../migrations/0005_legacy_imports.sql"),
        include_str!("../migrations/0006_runs.sql"),
        include_str!("../migrations/0007_runs_legacy_summary.sql")
    ] {sqlx::raw_sql(migration).execute(&pool).await.unwrap();}
    let owner=Uuid::new_v4();let id=Uuid::new_v4();let source=Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,google_subject,email,created_at,updated_at) VALUES($1,$1,'export-test@example.test','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')").bind(owner.to_string()).execute(&pool).await.unwrap();
    let bytes=include_bytes!("fixtures/activity.fit");
    let legacy=garmin_fit_extractor_api::fit::normalize::normalize(&garmin_fit_extractor_api::fit::raw::decode_raw(bytes).unwrap(),"");
    sqlx::query("INSERT INTO runs_sources(id,owner_id,sha256,size_bytes,bytes,integrity) VALUES($1,$2,$3,$4,$5,'verified')").bind(source.to_string()).bind(owner.to_string()).bind(Sha256::digest(bytes).to_vec()).bind(bytes.len() as i64).bind(bytes.as_slice()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO runs_activities(id,owner_id,source_id,session_index,start_time,end_time,start_order,end_order,summary,observation_group_id,observation_fingerprint,desired_versions,processing_status) VALUES($1,$2,$3,0,'2026-01-01T00:00:00Z','2026-01-02T03:46:40Z','2026-01-01T00:00:00Z','2026-01-02T03:46:40Z','{}',$1,$4,'{}','ready')").bind(id.to_string()).bind(owner.to_string()).bind(source.to_string()).bind(vec![1u8;32]).execute(&pool).await.unwrap();
    let mut run=activity(id,(0..100000).map(|index|json!({"index":index,"elapsedSeconds":index,"heartRateBpm":100+index%80,"powerWatts":index})).collect());
    run["normalized"]["endTime"]=json!("2026-01-02T03:46:40Z");
    run["analysis"]["quality"]["sampleCount"]=json!(100000);
    run["historicalThresholds"]=json!({"evidenceCutoff":"2026-01-02T03:46:40Z","computedAt":"2026-01-02T04:00:00Z","lt1":{"status":"low_confidence","value":{"heartRateBpm":143},"evidence":{"independentActivityCount":1,"activityId":"unselected-canary"}}});
    run["decoded"]["decoder"]=json!({"library":"fitparser","libraryVersion":"0.11.0","version":"0.11.0+runs.1","profileVersion":"21.202.0","archiveSchemaVersion":"2.0.0","normalizedSchemaVersion":"2.0.0","normalizerVersion":"native-runs-stream.1","vendorSourceSha256":"a".repeat(64),"profileSourceSha256":"b".repeat(64),"normalizerSourceSha256":"c".repeat(64),"archiveSourceSha256":"d".repeat(64),"rawProjectionSourceSha256":"e".repeat(64),"jsonFloatRoundtrip":true,"sourceSha256":"private-source-canary","sourceByteLength":123456789});
    run["normalized"]["private-canary-name.fit"]=json!({"samples":[{"value":"private-opaque-canary"}]});
    let mut revisions=Vec::new();
    for stage in ["decoded","normalized","analysis"] {
        let revision=Uuid::new_v4().to_string();
        let legacy_projection=if stage=="normalized" {Some(serde_json::to_string(&legacy).unwrap())} else {None};
        let archive=if stage=="analysis" {None} else {Some(serde_json::to_vec(&run[stage]).unwrap())};
        let payload=if let Some(bytes)=archive.as_ref() {
            let mut metadata=if stage=="decoded" {json!({"schemaVersion":"2.0.0"})} else {json!({"schemaVersion":"2.0.0","startTime":run[stage]["startTime"],"endTime":run[stage]["endTime"],"summary":run[stage]["summary"],"subtype":run[stage]["subtype"]})};
            metadata["documents"]=json!({"archive":{"byteLength":bytes.len(),"sha256":format!("{:x}",Sha256::digest(bytes)),"chunkCount":bytes.len().div_ceil(65536)}});
            metadata
        } else {run[stage].clone()};
        sqlx::query("INSERT INTO runs_revisions(id,owner_id,activity_id,stage,generation,versions,input_revision_ids,payload,legacy_projection,validation) VALUES($1,$2,$3,$4,1,'{}','[]',$5::jsonb,$6::jsonb,'{}')").bind(&revision).bind(owner.to_string()).bind(id.to_string()).bind(stage).bind(payload.to_string()).bind(legacy_projection).execute(&pool).await.unwrap();
        if let Some(bytes)=archive {for (position,chunk) in bytes.chunks(65536).enumerate() {
            sqlx::query("INSERT INTO runs_revision_chunks(revision_id,owner_id,document,position,payload) VALUES($1,$2,'archive',$3,$4)").bind(&revision).bind(owner.to_string()).bind(position as i64).bind(chunk).execute(&pool).await.unwrap();
        }}
        revisions.push(revision);
    }
    let manifest=Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO runs_manifests(id,owner_id,activity_id,decoded_revision_id,normalized_revision_id,analysis_revision_id,generation,versions) VALUES($1,$2,$3,$4,$5,$6,1,'{}')").bind(&manifest).bind(owner.to_string()).bind(id.to_string()).bind(&revisions[0]).bind(&revisions[1]).bind(&revisions[2]).execute(&pool).await.unwrap();
    sqlx::query("UPDATE runs_activities SET current_manifest_id=$1,history_generation=2 WHERE id=$2").bind(&manifest).bind(id.to_string()).execute(&pool).await.unwrap();
    assert_eq!(create(&pool,owner,request(id,ExportMode::Full)).await.unwrap_err().code(),"EXPORT_NOT_READY");
    sqlx::query("INSERT INTO runs_estimates(id,owner_id,activity_id,evidence_cutoff,computed_at,generation,manifest_id,payload) VALUES($1,$2,$3,'2026-01-02T03:46:40Z','2026-01-02T04:00:00Z',2,$4,$5::jsonb)").bind(Uuid::new_v4().to_string()).bind(owner.to_string()).bind(id.to_string()).bind(&manifest).bind(run["historicalThresholds"].to_string()).execute(&pool).await.unwrap();
    assert_eq!(create(&pool,Uuid::new_v4(),request(id,ExportMode::Full)).await.unwrap_err().code(),"NOT_FOUND");
    assert_eq!(create(&pool,owner,request(Uuid::new_v4(),ExportMode::Full)).await.unwrap_err().code(),"NOT_FOUND");
    let mut invalid=request(id,ExportMode::Full);invalid.activity_ids.push(Uuid::new_v4());
    assert_eq!(create(&pool,owner,invalid).await.unwrap_err().code(),"NOT_FOUND");
    let mut duplicate=request(id,ExportMode::Full);duplicate.activity_ids.push(id);
    assert_eq!(create(&pool,owner,duplicate).await.unwrap_err().code(),"INVALID_EXPORT_SELECTION");
    let mut empty=request(id,ExportMode::Full);empty.activity_ids.clear();
    assert_eq!(create(&pool,owner,empty).await.unwrap_err().code(),"INVALID_EXPORT_SELECTION");
    let legacy=Uuid::new_v4();
    sqlx::query("INSERT INTO extractions(id,user_id,file_name,file_size_bytes,status,error_code,error_message,created_at) VALUES($1,$2,'legacy-private-canary.fit',1,'failed','INVALID_FIT','Unsupported fixture','2026-01-01T00:00:00Z')").bind(legacy.to_string()).bind(owner.to_string()).execute(&pool).await.unwrap();
    assert_eq!(create(&pool,owner,request(legacy,ExportMode::Full)).await.unwrap_err().code(),"LEGACY_EXPORT_UNSUPPORTED");
    let mut mixed=request(legacy,ExportMode::Full);mixed.activity_ids.push(Uuid::new_v4());
    assert_eq!(create(&pool,owner,mixed).await.unwrap_err().code(),"NOT_FOUND");
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM runs_exports WHERE owner_id=$1").bind(owner.to_string()).fetch_one(&pool).await.unwrap(),0);
    let created=create(&pool,owner,request(id,ExportMode::Full)).await.unwrap();
    let token=created["token"].as_str().unwrap();
    assert!(serve(&pool,Uuid::new_v4(),token).await.is_err());
    assert!(serve_head(&pool,Uuid::new_v4(),token).await.is_err());
    let head=serve_head(&pool,owner,token).await.unwrap();
    assert_eq!(head.headers()["content-length"],created["byteLength"].as_u64().unwrap().to_string());
    assert_eq!(head.headers()["content-disposition"],"attachment; filename=\"runs-full.json\"");
    assert!(axum::body::to_bytes(head.into_body(),0).await.unwrap().is_empty());
    let generated=chrono::DateTime::parse_from_rfc3339(created["generatedAt"].as_str().unwrap()).unwrap();
    let expires=chrono::DateTime::parse_from_rfc3339(created["expiresAt"].as_str().unwrap()).unwrap();
    assert_eq!((expires-generated).num_seconds(),900);
    let response=serve(&pool,owner,token).await.unwrap();
    assert_eq!(response.headers()["cache-control"],"private, no-store");
    let copy=axum::body::to_bytes(response.into_body(),usize::MAX).await.unwrap();
    let download=axum::body::to_bytes(serve(&pool,owner,token).await.unwrap().into_body(),usize::MAX).await.unwrap();
    assert_eq!(copy,download);assert_eq!(copy.len() as u64,created["byteLength"].as_u64().unwrap());
    let expected=snapshot(&[id],&[run.clone()],&request(id,ExportMode::Full),created["generatedAt"].as_str().unwrap()).unwrap();
    assert_eq!(serde_json::from_slice::<Value>(&copy).unwrap(),serde_json::from_slice::<Value>(&expected).unwrap());
    assert_eq!(copy.last(),Some(&b'\n'));
    let full:Value=serde_json::from_slice(&copy).unwrap();
    assert_eq!(copy.as_ref(),serde_json::to_vec_pretty(&full).unwrap().into_iter().chain([b'\n']).collect::<Vec<_>>().as_slice());
    assert_eq!(full["privacyOmissions"],created["privacyOmissions"]);
    let text=std::str::from_utf8(&copy).unwrap();
    for secret in ["private-source-canary","123456789","private-canary-name","private-opaque-canary","unselected-canary","trace-canary"] {assert!(!text.contains(secret));}
    assert_eq!(full["activities"][0]["decoded"]["decoder"]["profileSourceSha256"],"b".repeat(64));
    let coach=create(&pool,owner,request(id,ExportMode::Coach)).await.unwrap();
    let coach_bytes=axum::body::to_bytes(serve(&pool,owner,coach["token"].as_str().unwrap()).await.unwrap().into_body(),usize::MAX).await.unwrap();
    let coach_value:Value=serde_json::from_slice(&coach_bytes).unwrap();
    let expected:Value=serde_json::from_slice(&snapshot(&[id],&[run],&request(id,ExportMode::Coach),coach["generatedAt"].as_str().unwrap()).unwrap()).unwrap();
    assert_eq!(coach_value["activities"],expected["activities"]);
    assert_eq!(coach_value["privacyOmissions"],coach["privacyOmissions"]);
    let document:Value=serde_json::from_slice(&copy).unwrap();
    assert_eq!(document["activities"][0]["historicalThresholds"]["lt1"]["value"]["heartRateBpm"],143);
    // Corrupt backing storage must fail the body, never masquerade as clean EOF.
    let damaged=create(&pool,owner,request(id,ExportMode::Full)).await.unwrap();
    sqlx::query("DELETE FROM runs_export_chunks WHERE token=$1 AND position=1").bind(damaged["token"].as_str().unwrap()).execute(&pool).await.unwrap();
    let mut damaged_body=serve(&pool,owner,damaged["token"].as_str().unwrap()).await.unwrap().into_body().into_data_stream();
    assert_eq!(damaged_body.next().await.unwrap().unwrap().len(),65536);
    assert!(damaged_body.next().await.unwrap().is_err());drop(damaged_body);
    // TTL is checked after headers and on every chunk, not frozen at transaction start.
    let expiring=create(&pool,owner,request(id,ExportMode::Full)).await.unwrap();
    let mut expiring_body=serve(&pool,owner,expiring["token"].as_str().unwrap()).await.unwrap().into_body().into_data_stream();
    assert!(expiring_body.next().await.unwrap().is_ok());
    sqlx::query("UPDATE runs_exports SET expires_at=clock_timestamp()-interval '1 second' WHERE token=$1").bind(expiring["token"].as_str().unwrap()).execute(&pool).await.unwrap();
    assert!(expiring_body.next().await.unwrap().is_err());drop(expiring_body);
    // PostgreSQL must release guards even if a client never polls the body again.
    let idle=create(&pool,owner,request(id,ExportMode::Full)).await.unwrap();
    sqlx::query("UPDATE runs_exports SET expires_at=clock_timestamp()+interval '1 second' WHERE token=$1").bind(idle["token"].as_str().unwrap()).execute(&pool).await.unwrap();
    let unpolled=serve(&pool,owner,idle["token"].as_str().unwrap()).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(8)).await;
    let mut guard=pool.begin().await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(3),sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))").bind(owner.to_string()).execute(&mut *guard)).await.unwrap().unwrap();
    guard.rollback().await.unwrap();
    let mut expired_body=unpolled.into_body().into_data_stream();
    assert!(expired_body.next().await.unwrap().is_err());drop(expired_body);
    let expired=create(&pool,owner,request(id,ExportMode::Full)).await.unwrap();
    sqlx::query("UPDATE runs_exports SET expires_at=now()-interval '1 second' WHERE token=$1").bind(expired["token"].as_str().unwrap()).execute(&pool).await.unwrap();
    assert!(serve(&pool,owner,expired["token"].as_str().unwrap()).await.is_err());
    assert!(serve_head(&pool,owner,expired["token"].as_str().unwrap()).await.is_err());
    sqlx::query("UPDATE runs_activities SET current_manifest_id=NULL,processing_status='processing' WHERE id=$1").bind(id.to_string()).execute(&pool).await.unwrap();
    assert!(create(&pool,owner,request(id,ExportMode::Full)).await.is_err());
    let pinned=axum::body::to_bytes(serve(&pool,owner,token).await.unwrap().into_body(),usize::MAX).await.unwrap();
    assert_eq!(pinned,copy);
    // Body drop must release both the transaction and owner deletion guard.
    let dropped=serve(&pool,owner,token).await.unwrap();drop(dropped);
    let mut guard=pool.begin().await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(3),sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))").bind(owner.to_string()).execute(&mut *guard)).await.unwrap().unwrap();
    guard.rollback().await.unwrap();
    // Deterministic midstream barrier: first chunk is read before revocation commits.
    let mut stream=serve(&pool,owner,token).await.unwrap().into_body().into_data_stream();
    let first=stream.next().await.unwrap().unwrap();assert!(first.len()<copy.len());
    sqlx::query("UPDATE runs_exports SET revoked=true WHERE token=$1").bind(token).execute(&pool).await.unwrap();
    assert!(serve_head(&pool,owner,token).await.is_err());
    let delete_pool=pool.clone();
    let deletion=tokio::spawn(async move {
        let mut transaction=delete_pool.begin().await.unwrap();
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))").bind(owner.to_string()).execute(&mut *transaction).await.unwrap();
        sqlx::query("DELETE FROM runs_exports WHERE owner_id=$1 AND $2=ANY(activity_ids)").bind(owner.to_string()).bind(id.to_string()).execute(&mut *transaction).await.unwrap();
        sqlx::query("DELETE FROM runs_activities WHERE id=$1 AND owner_id=$2").bind(id.to_string()).bind(owner.to_string()).execute(&mut *transaction).await.unwrap();
        transaction.commit().await.unwrap();
    });
    assert!(stream.next().await.unwrap().is_err());drop(stream);
    tokio::time::timeout(std::time::Duration::from_secs(3),deletion).await.unwrap().unwrap();
    assert!(serve(&pool,owner,token).await.is_err());
    pool.close().await;
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE"))).execute(&admin).await.unwrap();
    admin.close().await;
}

#[test]
fn native_metric_types_keep_exact_integer_strings_zero_and_invalid_state_not_opaque_strings() {
    let activity=json!({"decoded":{"messages":[{"globalMessageNumber":20,"fields":[
        {"fieldNumber":5,"value":"18446744073709551614","rawValue":"18446744073709551614","validity":"valid","unit":"m"},
        {"fieldNumber":3,"value":0,"validity":"valid","unit":"bpm"},
        {"fieldNumber":7,"value":null,"validity":"invalid"},
        {"fieldNumber":3,"value":"opaque-safe-looking-canary","classification":"metric"}
    ]}]}});
    let (safe,omissions)=project(&activity,Policy::default());
    let fields=safe["decoded"]["messages"][0]["fields"].as_array().unwrap();
    assert_eq!(fields[0]["value"],"18446744073709551614");
    assert_eq!(fields[0]["rawValue"],"18446744073709551614");
    assert_eq!(fields[1]["value"],0);
    assert_eq!(fields[2]["value"],Value::Null);
    assert_eq!(fields[2]["validity"],"invalid");
    assert!(!serde_json::to_string(&(safe,omissions)).unwrap().contains("opaque-safe-looking-canary"));
}

#[test]
fn verified_native_extension_identity_keeps_metric_not_named_developer_duplicates() {
    let activity=json!({"normalized":{"extensions":[
        {"identity":{"globalMessageNumber":20,"fieldNumber":3,"developerIdentity":null},"name":"private-name-canary","value":151,"unit":"bpm","sourceReference":{"messageIndex":4,"globalMessageNumber":20,"fieldNumber":3}},
        {"identity":{"globalMessageNumber":20,"fieldNumber":3,"developerIdentity":{"developerDataIndex":0,"fieldDefinitionNumber":3}},"name":"heart_rate","value":777777777},
        {"identity":{"globalMessageNumber":20,"fieldNumber":250},"name":"heart_rate","value":888888888}
    ]}});
    let (safe,omissions)=project(&activity,Policy::default());
    assert_eq!(safe["normalized"]["extensions"][0]["value"],151);
    assert_eq!(safe["normalized"]["extensions"][0]["identity"]["fieldNumber"],3);
    let text=serde_json::to_string(&(safe,omissions)).unwrap();
    for secret in ["private-name-canary","777777777","888888888"] {assert!(!text.contains(secret));}
}

#[test]
fn streaming_privacy_preserves_native_float64_bits_including_subnormals() {
    use std::cell::RefCell;
    use garmin_fit_extractor_api::runs::{privacy::{Document,Projection},stream::project_document};
    let numbers=[f64::MIN_POSITIVE,f64::from_bits(1),f64::from_bits(0x0010000000000001),0.8455124082255701,9.903520314283041e27];
    let decoded=json!({"schemaVersion":"2.0.0","messages":numbers.iter().enumerate().map(|(index,value)|json!({"index":index,"globalMessageNumber":20,"fields":[{"fieldNumber":6,"value":value,"rawValue":value,"baseType":"float64","validity":"valid","unit":"m/s"}]})).collect::<Vec<_>>()});
    let normalized=json!({"schemaVersion":"2.0.0","samples":numbers.iter().enumerate().map(|(index,value)|json!({"index":index,"elapsedSeconds":index,"speedMps":value})).collect::<Vec<_>>()});
    for (document,input,key) in [(Document::Decoded,decoded,"messages"),(Document::Normalized,normalized,"samples")] {
        let bytes=serde_json::to_vec(&input).unwrap();let mut output=Vec::new();
        let projection=RefCell::new(Projection::new(document,Policy::default()));
        project_document(bytes.as_slice(),&mut output,&mut |path|projection.borrow_mut().keep(path),&mut |path,value|Ok(projection.borrow_mut().value(path,value))).unwrap();
        let value:Value=serde_json::from_slice(&output).unwrap();
        for (index,expected) in numbers.iter().enumerate() {
            let row=&value[key][index];
            let actual=if key=="messages" {&row["fields"][0]["value"]} else {&row["speedMps"]};
            assert_eq!(actual.as_f64().unwrap().to_bits(),expected.to_bits(),"lost native float64 precision at {key}[{index}]");
            if key=="messages" {assert_eq!(row["fields"][0]["rawValue"].as_f64().unwrap().to_bits(),expected.to_bits());}
        }
    }
}

#[test]
fn streaming_rr_keeps_native_array_positions_not_developer_or_location_compound_values() {
    use std::cell::RefCell;
    use garmin_fit_extractor_api::runs::{privacy::{Document,Projection},stream::project_document};
    let normalized=json!({"schemaVersion":"2.0.0","samples":[{"index":0,"heartRateBpm":987654321,"sourceReferences":{"heartRateBpm":{"globalMessageNumber":20,"fieldNumber":3,"componentParent":0}}}],"rr":{"alignmentEligible":false,"intervals":[
        {"index":0,"rrMs":800,"sourceReference":{"globalMessageNumber":78,"fieldNumber":0,"arrayIndex":0}},
        {"index":1,"rrMs":805,"sourceReference":{"globalMessageNumber":132,"fieldNumber":9,"componentParent":10,"arrayIndex":7}},
        {"index":2,"rrMs":123456789,"sourceReference":{"globalMessageNumber":78,"fieldNumber":0,"arrayIndex":2,"developerIdentity":{"developerDataIndex":0,"fieldDefinitionNumber":0}}}
    ]}});
    let bytes=serde_json::to_vec(&normalized).unwrap();let mut output=Vec::new();
    let projection=RefCell::new(Projection::new(Document::Normalized,Policy::default()));
    project_document(bytes.as_slice(),&mut output,&mut |path|projection.borrow_mut().keep(path),&mut |path,value|Ok(projection.borrow_mut().value(path,value))).unwrap();
    let value:Value=serde_json::from_slice(&output).unwrap();
    assert_eq!(value["rr"]["intervals"][0]["rrMs"],800);
    assert_eq!(value["rr"]["intervals"][0]["sourceReference"]["arrayIndex"],0);
    assert_eq!(value["rr"]["intervals"][1]["sourceReference"]["componentParent"],10);
    assert_eq!(value["rr"]["intervals"][1]["sourceReference"]["arrayIndex"],7);
    let text=std::str::from_utf8(&output).unwrap();
    assert!(!text.contains("123456789"));assert!(!text.contains("987654321"));
}

#[test]
fn summary_retains_recorded_derived_coverage_and_literal_method_without_reference_promotion() {
    use std::cell::RefCell;
    use garmin_fit_extractor_api::runs::{privacy::{Document,Projection},stream::project_document};
    let summary=json!({
        "distanceMeters":1000,"timerTimeSeconds":300,"elapsedTimeSeconds":360,"averageHeartRateBpm":126.66666666666667,
        "recorded":{"distanceMeters":1000,"timerTimeSeconds":300,"elapsedTimeSeconds":360,"averageHeartRateBpm":null,"averagePowerWatts":123456789,"sourceReferences":{"averagePowerWatts":{"globalMessageNumber":18,"fieldNumber":20}}},
        "derived":{"averageHeartRateBpm":126.66666666666667,"elapsedTimeSeconds":360,"distanceMeters":null,"private-canary.fit":"opaque-canary"},
        "coverage":{"averageHeartRateBpm":{"coveredSeconds":30,"windowSeconds":360,"fraction":30.0/360.0,"filename":"coverage-canary"}},
        "method":{"default":"recorded_summary_preferred","derived":"interval_weighted_left_sample","gapThresholdSeconds":30,"pauseHandling":"exclude_recorded_timer_pauses","missingTimerState":"unknown_not_assumed_stopped","finalSample":"no_extrapolation","filename":"method-canary"},
        "sourceReferences":{"distanceMeters":{"messageIndex":5,"globalMessageNumber":18,"fieldNumber":9,"byteOffset":100},"timerTimeSeconds":{"globalMessageNumber":18,"fieldNumber":8},"elapsedTimeSeconds":{"globalMessageNumber":18,"fieldNumber":7},"averageFractionalCadenceCyclesPerMinute":{"globalMessageNumber":18,"fieldNumber":92},"averagePowerWatts":{"globalMessageNumber":18,"fieldNumber":20,"developerIdentity":{"developerDataIndex":0,"fieldDefinitionNumber":20}}}
    });
    let input=json!({"schemaVersion":"2.0.0","summary":summary,"laps":[{"index":0,"summary":summary}]});
    let bytes=serde_json::to_vec(&input).unwrap();let mut output=Vec::new();
    let projection=RefCell::new(Projection::new(Document::Normalized,Policy::default()));
    project_document(bytes.as_slice(),&mut output,&mut |path|projection.borrow_mut().keep(path),&mut |path,value|Ok(projection.borrow_mut().value(path,value))).unwrap();
    let value:Value=serde_json::from_slice(&output).unwrap();
    for summary in [&value["summary"],&value["laps"][0]["summary"]] {
        assert_eq!(summary["recorded"]["distanceMeters"],1000);
        assert_eq!(summary["recorded"]["timerTimeSeconds"],300);
        assert_eq!(summary["recorded"]["elapsedTimeSeconds"],360);
        assert_eq!(summary["recorded"]["averageHeartRateBpm"],Value::Null);
        assert_eq!(summary["derived"]["averageHeartRateBpm"].as_f64(),Some(126.66666666666667));
        assert_eq!(summary["coverage"]["averageHeartRateBpm"]["coveredSeconds"],30);
        assert_eq!(summary["coverage"]["averageHeartRateBpm"]["windowSeconds"],360);
        assert_eq!(summary["method"]["derived"],"interval_weighted_left_sample");
        assert_eq!(summary["method"]["gapThresholdSeconds"],30);
        assert_eq!(summary["sourceReferences"]["distanceMeters"]["globalMessageNumber"],18);
        assert_eq!(summary["sourceReferences"]["averageFractionalCadenceCyclesPerMinute"]["fieldNumber"],92);
    }
    let text=std::str::from_utf8(&output).unwrap();
    for secret in ["123456789","private-canary","opaque-canary","coverage-canary","method-canary","developerIdentity"] {assert!(!text.contains(secret));}
    let invalid_method=json!({"normalized":{"summary":{"method":{"derived":"known-key-private-filename.fit","gapThresholdSeconds":"opaque-gap"}}}});
    let (safe,omissions)=project(&invalid_method,Policy::default());
    let text=serde_json::to_string(&(safe,omissions)).unwrap();
    assert!(!text.contains("known-key-private"));assert!(!text.contains("opaque-gap"));
}
