use garmin_fit_extractor_api::{db,runs::{jobs,store}};
use sqlx::PgPool;
use uuid::Uuid;
use garmin_fit_extractor_api::runs::{export,history};
use serde_json::{Value,json};
use sha2::{Digest,Sha256};
use sqlx::postgres::PgPoolOptions;
use tokio::time::{Duration,timeout};

async fn isolated_store_pool()->(PgPool,PgPool,String,Uuid){
    let url=std::env::var("TEST_DATABASE_URL").expect("disposable TEST_DATABASE_URL");
    let expected=std::env::var("PGDATA").expect("disposable PGDATA identity");
    assert!(expected.starts_with("/tmp/garmin-runs-core-pg."));
    let admin=PgPool::connect(&url).await.unwrap();
    let directory:String=sqlx::query_scalar("SHOW data_directory").fetch_one(&admin).await.unwrap();
    assert_eq!(directory,expected,"refuse to migrate any other PostgreSQL instance");
    let version:String=sqlx::query_scalar("SHOW server_version_num").fetch_one(&admin).await.unwrap();
    assert!((180000..190000).contains(&version.parse::<i32>().unwrap()));
    let schema=format!("runs_store_test_{}",Uuid::new_v4().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE SCHEMA {schema}"))).execute(&admin).await.unwrap();
    let selected=schema.clone();
    let pool=PgPoolOptions::new().max_connections(6).after_connect(move|connection,_|{
        let schema=selected.clone();
        Box::pin(async move{
            sqlx::query("SELECT set_config('search_path',$1,false),set_config('application_name',$1,false)")
                .bind(schema).execute(connection).await?;
            Ok(())
        })
    }).connect(&url).await.unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    let owner=Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,google_subject,email,created_at,updated_at) VALUES($1,$1,$2,$3,$3)")
        .bind(owner.to_string()).bind(format!("{owner}@example.test")).bind(db::created_at_now()).execute(&pool).await.unwrap();
    (admin,pool,schema,owner)
}

async fn close_store_pool(admin:PgPool,pool:PgPool,schema:String){
    pool.close().await;
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE"))).execute(&admin).await.unwrap();
    admin.close().await;
}

// Only deletion's database graph is synthetic. Source and revision immutability
// triggers, foreign keys, export transport, and the actual deletion stay enabled.
async fn deletion_graph_run(pool:&PgPool,owner:Uuid,end:&str,bytes:&[u8])->(Uuid,String){
    let activity=Uuid::new_v4();let source=Uuid::new_v4();
    sqlx::query("INSERT INTO runs_sources(id,owner_id,sha256,size_bytes,bytes,integrity) VALUES($1,$2,$3,$4,$5,'verified')")
        .bind(source.to_string()).bind(owner.to_string()).bind(Sha256::digest(bytes).to_vec()).bind(bytes.len() as i64).bind(bytes).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO runs_activities(id,owner_id,source_id,session_index,start_time,end_time,start_order,end_order,summary,observation_group_id,observation_fingerprint,desired_versions,processing_status) VALUES($1,$2,$3,0,$4,$4,$4::timestamptz,$4::timestamptz,'{}',$1,$5,$6::jsonb,'ready')")
        .bind(activity.to_string()).bind(owner.to_string()).bind(source.to_string()).bind(end).bind(vec![1u8;32]).bind(store::versions().to_string()).execute(pool).await.unwrap();
    let mut revisions=Vec::new();
    for stage in ["decoded","normalized","analysis"]{
        let revision=Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO runs_revisions(id,owner_id,activity_id,stage,generation,versions,input_revision_ids,payload,legacy_projection,validation) VALUES($1,$2,$3,$4,1,$5::jsonb,'[]','{}',$6::jsonb,'{}')")
            .bind(&revision).bind(owner.to_string()).bind(activity.to_string()).bind(stage).bind(store::versions().to_string()).bind((stage=="normalized").then_some("{}")).execute(pool).await.unwrap();
        revisions.push(revision);
    }
    let manifest=Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO runs_manifests(id,owner_id,activity_id,decoded_revision_id,normalized_revision_id,analysis_revision_id,generation,versions) VALUES($1,$2,$3,$4,$5,$6,1,$7::jsonb)")
        .bind(&manifest).bind(owner.to_string()).bind(activity.to_string()).bind(&revisions[0]).bind(&revisions[1]).bind(&revisions[2]).bind(store::versions().to_string()).execute(pool).await.unwrap();
    sqlx::query("UPDATE runs_activities SET current_manifest_id=$2 WHERE id=$1").bind(activity.to_string()).bind(&manifest).execute(pool).await.unwrap();
    (activity,manifest)
}

async fn insert_graph_export(tx:&mut sqlx::Transaction<'_,sqlx::Postgres>,owner:Uuid,activity:Uuid,manifest:&str,token:&str,bytes:&[u8]){
    sqlx::query("INSERT INTO runs_exports(token,owner_id,activity_ids,manifest_ids,byte_length,mode,meta,generated_at,expires_at) VALUES($1,$2,$3,$4,$5,'full','{}',$6,clock_timestamp()+interval '15 minutes')")
        .bind(token).bind(owner.to_string()).bind(vec![activity.to_string()]).bind(vec![manifest.to_owned()]).bind(bytes.len() as i64).bind(db::created_at_now()).execute(&mut **tx).await.unwrap();
    for(position,chunk)in bytes.chunks(65536).enumerate(){
        sqlx::query("INSERT INTO runs_export_chunks(token,position,payload) VALUES($1,$2,$3)")
            .bind(token).bind(position as i64).bind(chunk).execute(&mut **tx).await.unwrap();
    }
}

#[tokio::test(flavor="multi_thread",worker_threads=2)]
async fn delete_erases_dependent_export_committed_while_waiting_for_owner_lock(){
    let(admin,pool,schema,owner)=isolated_store_pool().await;
    let(deleted,deleted_manifest)=deletion_graph_run(&pool,owner,"2026-01-01T00:00:00Z",b"deleted source").await;
    let(selected,selected_manifest)=deletion_graph_run(&pool,owner,"2026-01-09T00:00:00Z",b"selected source").await;
    // Eight days later deliberately excludes the seven-day invalidation arm.
    let estimate=Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO runs_estimates(id,owner_id,activity_id,evidence_cutoff,computed_at,generation,manifest_id,payload) VALUES($1,$2,$3,'2026-01-09T00:00:00Z','2026-01-09T01:00:00Z',1,$4,'{}')")
        .bind(&estimate).bind(owner.to_string()).bind(selected.to_string()).bind(&selected_manifest).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO runs_estimate_dependencies(estimate_id,owner_id,activity_id,manifest_id) VALUES($1,$2,$3,$4)")
        .bind(&estimate).bind(owner.to_string()).bind(deleted.to_string()).bind(&deleted_manifest).execute(&pool).await.unwrap();
    let before_token=Uuid::new_v4().to_string();
    let late_token=Uuid::new_v4().to_string();
    let bytes=serde_json::to_vec(&json!({"selected":selected,"dependency":deleted,"padding":"x".repeat(65536)})).unwrap();
    let mut before=pool.begin().await.unwrap();
    insert_graph_export(&mut before,owner,selected,&selected_manifest,&before_token,&bytes).await;
    before.commit().await.unwrap();
    let response=export::serve(&pool,owner,&before_token).await.unwrap();
    assert_eq!(axum::body::to_bytes(response.into_body(),bytes.len()).await.unwrap().as_ref(),bytes.as_slice());

    let mut publisher=pool.begin().await.unwrap();
    let publisher_pid:i32=sqlx::query_scalar("SELECT pg_backend_pid()").fetch_one(&mut *publisher).await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))").bind(owner.to_string()).execute(&mut *publisher).await.unwrap();
    let delete_pool=pool.clone();
    let deletion=tokio::spawn(async move{store::delete(&delete_pool,owner,deleted).await});
    let waiting=timeout(Duration::from_secs(10),async{
        loop{
            let waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity a WHERE a.application_name=$1 AND a.wait_event_type='Lock' AND lower(a.wait_event)='advisory' AND $2=ANY(pg_blocking_pids(a.pid)))")
                .bind(&schema).bind(publisher_pid).fetch_one(&admin).await.unwrap();
            if waiting{break;}
            assert!(!deletion.is_finished(),"deletion must wait for the publisher's owner lock");
            tokio::task::yield_now().await;
        }
    }).await;
    if waiting.is_err(){
        deletion.abort();
        publisher.rollback().await.unwrap();
        panic!("delete never reached the observed PostgreSQL advisory lock wait");
    }
    let revoked:bool=sqlx::query_scalar("SELECT revoked FROM runs_exports WHERE token=$1").bind(&before_token).fetch_one(&pool).await.unwrap();
    assert!(revoked,"pre-lock dependency revocation must have completed");
    insert_graph_export(&mut publisher,owner,selected,&selected_manifest,&late_token,&bytes).await;
    publisher.commit().await.unwrap();
    assert!(timeout(Duration::from_secs(10),deletion).await.unwrap().unwrap().unwrap());
    let remaining:(i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM runs_exports WHERE token=ANY($1)),(SELECT count(*) FROM runs_export_chunks WHERE token=ANY($1))")
        .bind(vec![before_token.clone(),late_token.clone()]).fetch_one(&pool).await.unwrap();
    assert_eq!(remaining,(0,0),"delete must atomically erase late dependent token and every chunk");
    for token in [&before_token,&late_token]{
        assert_eq!(export::serve(&pool,owner,token).await.unwrap_err().code(),"EXPORT_UNAVAILABLE");
        assert_eq!(export::serve_head(&pool,owner,token).await.unwrap_err().code(),"EXPORT_UNAVAILABLE");
    }
    let graph:(bool,bool,bool)=sqlx::query_as("SELECT EXISTS(SELECT 1 FROM runs_activities WHERE id=$1),EXISTS(SELECT 1 FROM runs_manifests WHERE id=$2),EXISTS(SELECT 1 FROM runs_estimates WHERE id=$3)")
        .bind(deleted.to_string()).bind(&selected_manifest).bind(&estimate).fetch_one(&pool).await.unwrap();
    assert_eq!(graph,(false,true,false),"erase B and its dependent estimate without deleting A's coherent manifest");
    close_store_pool(admin,pool,schema).await;
}

async fn run_detail(pool:&PgPool,owner:Uuid,activity:Uuid)->Value{
    use std::{path::PathBuf,sync::Arc};
    use axum::{body::Body,http::Request};
    use garmin_fit_extractor_api::{app::{AppState,router},auth::{AuthState,hash_token}};
    use tower::ServiceExt;
    let token=Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO sessions(token_hash,user_id,created_at,expires_at) VALUES($1,$2,$3,'2099-01-01T00:00:00Z')")
        .bind(hash_token(&token)).bind(owner.to_string()).bind(db::created_at_now()).execute(pool).await.unwrap();
    let app=router(AppState{db:pool.clone(),auth:Arc::new(AuthState::new(None,None)),app_origin:None},PathBuf::from("nonexistent-test-static"));
    let response=Box::pin(app.oneshot(Request::builder().uri(format!("/api/v2/runs/{activity}")).header("cookie",format!("garmin_fit_session={token}")).body(Body::empty()).unwrap())).await.unwrap();
    assert_eq!(response.status(),axum::http::StatusCode::OK);
    serde_json::from_slice(&Box::pin(axum::body::to_bytes(response.into_body(),10*1024*1024)).await.unwrap()).unwrap()
}

// Stage a new generation with intact transport integrity but invalid numerical
// JSON. Never update original bytes, published revisions, or immutable chunks.
async fn stage_invalid_current_generation(pool:&PgPool,activity:Uuid,generation:i64,manifest:&str){
    let(decoded,normalized):(String,String)=sqlx::query_as("SELECT decoded_revision_id,normalized_revision_id FROM runs_manifests WHERE id=$1").bind(manifest).fetch_one(pool).await.unwrap();
    let new_decoded=Uuid::new_v4().to_string();let new_normalized=Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO runs_revisions(id,owner_id,activity_id,stage,generation,versions,input_revision_ids,payload,legacy_projection,validation) SELECT $1,owner_id,activity_id,stage,$2,versions,input_revision_ids,payload,legacy_projection,validation FROM runs_revisions WHERE id=$3 AND activity_id=$4")
        .bind(&new_decoded).bind(generation).bind(&decoded).bind(activity.to_string()).execute(pool).await.unwrap();
    let invalid=b"{";
    let descriptor=json!({"byteLength":invalid.len(),"sha256":format!("{:x}",Sha256::digest(invalid)),"chunkCount":1});
    sqlx::query("INSERT INTO runs_revisions(id,owner_id,activity_id,stage,generation,versions,input_revision_ids,payload,legacy_projection,validation) SELECT $1,owner_id,activity_id,stage,$2,versions,$3::jsonb,jsonb_set(payload,'{documents,analysisInput}',$4::jsonb),legacy_projection,validation FROM runs_revisions WHERE id=$5 AND activity_id=$6")
        .bind(&new_normalized).bind(generation).bind(json!([new_decoded]).to_string()).bind(descriptor.to_string()).bind(&normalized).bind(activity.to_string()).execute(pool).await.unwrap();
    for(old,new)in [(&decoded,&new_decoded),(&normalized,&new_normalized)]{
        sqlx::query("INSERT INTO runs_revision_chunks(revision_id,owner_id,document,position,payload) SELECT $1,owner_id,document,position,payload FROM runs_revision_chunks WHERE revision_id=$2 AND document='archive'")
            .bind(new).bind(old).execute(pool).await.unwrap();
    }
    sqlx::query("INSERT INTO runs_revision_chunks(revision_id,owner_id,document,position,payload) SELECT $1,owner_id,'analysisInput',0,$2 FROM runs_revisions WHERE id=$1")
        .bind(&new_normalized).bind(invalid.as_slice()).execute(pool).await.unwrap();
}

async fn retained_manifest_history_survives_processing_failure(version_scan:bool){
    let(admin,pool,schema,owner)=isolated_store_pool().await;
    let bytes=include_bytes!("fixtures/runs/garmin_run.fit");
    let mut document=Box::pin(jobs::decode_import(&pool,bytes)).await.unwrap();
    let admission=jobs::import_admission(&pool).await.unwrap();
    document.protect_cpu(&admission);
    let activity=Box::pin(store::accept_spool(&pool,owner,bytes,document,admission.holder())).await.unwrap().activity_id;
    drop(admission);
    // Keep debug-only nested worker futures off the test harness's stack.
    assert!(Box::pin(jobs::run_once(&pool)).await.unwrap(),"publish the real initial analysis manifest");
    assert!(Box::pin(jobs::run_once(&pool)).await.unwrap(),"publish the real initial historical result");
    assert!(!Box::pin(jobs::run_once(&pool)).await.unwrap(),"initial durable queue must drain");
    let original=Box::pin(run_detail(&pool,owner,activity)).await;
    let manifest=original["revisionId"].as_str().unwrap().to_owned();
    assert!(original["historicalThresholds"].is_object());
    let old_estimate:(String,String)=sqlx::query_as("SELECT id,payload::text FROM runs_estimates WHERE activity_id=$1").bind(activity.to_string()).fetch_one(&pool).await.unwrap();
    let pin=Box::pin(export::create(&pool,owner,export::ExportRequest{activity_ids:vec![activity],mode:export::ExportMode::Full,include_location:false,include_device_identifiers:false})).await.unwrap();
    let pin_token=pin["token"].as_str().unwrap();
    let pinned_bytes=axum::body::to_bytes(export::serve(&pool,owner,pin_token).await.unwrap().into_body(),10*1024*1024).await.unwrap();

    let mut tx=pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))").bind(owner.to_string()).execute(&mut *tx).await.unwrap();
    store::invalidate_cutoffs(&mut tx,&owner.to_string(),original["endTime"].as_str().unwrap()).await.unwrap();
    tx.commit().await.unwrap();
    let(history_job,history_generation):(String,i64)=sqlx::query_as("SELECT id,desired_generation FROM runs_jobs WHERE activity_id=$1 AND stage='history' AND status='queued'").bind(activity.to_string()).fetch_one(&pool).await.unwrap();
    // Schedule processing first without sleeps or a fabricated history result.
    sqlx::query("UPDATE runs_jobs SET next_attempt_at='infinity' WHERE id=$1").bind(&history_job).execute(&pool).await.unwrap();
    assert!(Box::pin(run_detail(&pool,owner,activity)).await["historicalThresholds"].is_null());
    let processing_generation:i64=sqlx::query_scalar("SELECT desired_generation+1 FROM runs_activities WHERE id=$1").bind(activity.to_string()).fetch_one(&pool).await.unwrap();
    stage_invalid_current_generation(&pool,activity,processing_generation,&manifest).await;
    if version_scan{
        let mut outdated=store::versions();outdated["analysis"]=json!("prior-analysis-version");
        sqlx::query("UPDATE runs_activities SET desired_versions=$2::jsonb WHERE id=$1").bind(activity.to_string()).bind(outdated.to_string()).execute(&pool).await.unwrap();
        jobs::scan_versions(&pool).await.unwrap();
    }else{
        store::reprocess(&pool,owner,activity).await.unwrap();
    }
    let queued:(String,i64,i64)=sqlx::query_as("SELECT j.status,a.history_generation,a.desired_generation FROM runs_jobs j JOIN runs_activities a ON a.id=j.activity_id WHERE j.id=$1").bind(&history_job).fetch_one(&pool).await.unwrap();
    assert_eq!(queued,("queued".into(),history_generation,processing_generation),"processing generations must not cancel independent queued history");
    let failure=Box::pin(jobs::run_once(&pool)).await.unwrap_err();
    assert_eq!(failure.code(),"INVALID_RUN_DOCUMENT","consume the actual invalid current-generation numerical input");
    let failed=Box::pin(run_detail(&pool,owner,activity)).await;
    assert_eq!(failed["processing"]["status"],"failed");
    assert_eq!(failed["processing"]["errorCode"],"INVALID_RUN_DOCUMENT");
    assert_eq!(failed["processing"]["updateFailed"],true);
    assert_eq!(failed["processing"]["historyStatus"],"pending");
    assert_eq!(failed["revisionId"],manifest);
    assert_eq!(failed["normalized"],original["normalized"]);
    assert_eq!(failed["analysis"],original["analysis"]);
    assert!(failed["historicalThresholds"].is_null());
    sqlx::query("UPDATE runs_jobs SET next_attempt_at=now() WHERE id=$1").bind(&history_job).execute(&pool).await.unwrap();
    assert!(Box::pin(jobs::run_once(&pool)).await.unwrap(),"consume the retained queued historical job");
    assert!(!Box::pin(jobs::run_once(&pool)).await.unwrap(),"no cancelled or failed processing may republish a manifest");
    let completed=Box::pin(run_detail(&pool,owner,activity)).await;
    assert_eq!(completed["revisionId"],manifest);
    assert_eq!(completed["processing"]["status"],"failed");
    assert_eq!(completed["processing"]["historyStatus"],"ready");
    let historical=&completed["historicalThresholds"];
    assert_eq!(historical["activityId"],activity.to_string());
    assert_eq!(historical["evidenceCutoff"],original["endTime"]);
    assert!(historical["computedAt"].as_str().is_some());
    for target in ["lt1","lt2"]{
        assert_eq!(historical[target],original["historicalThresholds"][target],"real history engine must still consume the retained coherent evidence");
        assert_eq!(historical[target]["status"],"insufficient_data");
    }
    let preserved:String=sqlx::query_scalar("SELECT payload::text FROM runs_estimates WHERE id=$1").bind(&old_estimate.0).fetch_one(&pool).await.unwrap();
    assert_eq!(preserved,old_estimate.1,"old historical snapshots remain immutable");
    let current=history::for_manifest(&pool,owner,activity,Some(&manifest),history_generation).await.unwrap();
    assert_eq!(current,*historical);
    let fresh=Box::pin(export::create(&pool,owner,export::ExportRequest{activity_ids:vec![activity],mode:export::ExportMode::Full,include_location:false,include_device_identifiers:false}))
        .await.expect("completed history must unblock new exports despite failed reprocessing");
    let fresh_bytes=axum::body::to_bytes(export::serve(&pool,owner,fresh["token"].as_str().unwrap()).await.unwrap().into_body(),10*1024*1024).await.unwrap();
    let fresh_document:Value=serde_json::from_slice(&fresh_bytes).unwrap();
    let exported_history=&fresh_document["activities"][0]["historicalThresholds"];
    assert_eq!(exported_history["computedAt"],historical["computedAt"]);
    assert_eq!(exported_history["evidenceCutoff"],historical["evidenceCutoff"]);
    assert_eq!(fresh_document["activities"][0]["id"],activity.to_string());
    let pins:Vec<String>=sqlx::query_scalar("SELECT manifest_ids FROM runs_exports WHERE token=$1").bind(pin_token).fetch_one(&pool).await.unwrap();
    assert_eq!(pins,vec![manifest]);
    let after=axum::body::to_bytes(export::serve(&pool,owner,pin_token).await.unwrap().into_body(),10*1024*1024).await.unwrap();
    assert_eq!(after,pinned_bytes,"failed update and independent history must not alter an existing export pin");
    let archived:Vec<u8>=sqlx::query_scalar("SELECT bytes FROM runs_sources WHERE owner_id=$1").bind(owner.to_string()).fetch_one(&pool).await.unwrap();
    assert_eq!(archived.as_slice(),bytes,"failure setup never weakens or mutates immutable source storage");
    close_store_pool(admin,pool,schema).await;
}

#[tokio::test(flavor="multi_thread",worker_threads=2)]
async fn reprocess_failure_preserves_queued_history_coherent_manifest_and_export_pin(){
    Box::pin(retained_manifest_history_survives_processing_failure(false)).await;
}

#[tokio::test(flavor="multi_thread",worker_threads=2)]
async fn nonhistory_version_scan_failure_preserves_queued_history_coherent_manifest_and_export_pin(){
    Box::pin(retained_manifest_history_survives_processing_failure(true)).await;
}

#[tokio::test(flavor="multi_thread",worker_threads=2)]
async fn publication_commits_while_heartbeat_waits_for_its_job_row_lock(){
    let(admin,pool,schema,owner)=isolated_store_pool().await;
    let bytes=include_bytes!("fixtures/runs/garmin_run.fit");
    let mut document=Box::pin(jobs::decode_import(&pool,bytes)).await.unwrap();
    let admission=jobs::import_admission(&pool).await.unwrap();
    document.protect_cpu(&admission);
    let activity=Box::pin(store::accept_spool(&pool,owner,bytes,document,admission.holder())).await.unwrap().activity_id;
    drop(admission);
    // Hold the real publication transaction after it locks and completes the
    // processing job. The normal heartbeat must then contend for that job row.
    sqlx::raw_sql("CREATE FUNCTION runs_test_publication_gate() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN
            IF OLD.status='processing' AND NEW.status='ready' AND NEW.stage<>'history' THEN
                PERFORM pg_advisory_xact_lock(hashtextextended(TG_TABLE_SCHEMA,1));
            END IF;
            RETURN NEW;
        END $$;
        CREATE TRIGGER runs_test_publication_gate AFTER UPDATE ON runs_jobs
        FOR EACH ROW EXECUTE FUNCTION runs_test_publication_gate();").execute(&pool).await.unwrap();
    let mut gate=pool.begin().await.unwrap();
    let gate_pid:i32=sqlx::query_scalar("SELECT pg_backend_pid()").fetch_one(&mut *gate).await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,1))").bind(&schema).execute(&mut *gate).await.unwrap();
    let worker_pool=pool.clone();
    let mut worker=tokio::spawn(async move{Box::pin(jobs::run_once(&worker_pool)).await});
    let execution=timeout(Duration::from_secs(30),async{
        loop{
            let pid:Option<i32>=sqlx::query_scalar("SELECT a.pid FROM pg_stat_activity a WHERE a.application_name=$1 AND a.wait_event_type='Lock' AND lower(a.wait_event)='advisory' AND $2=ANY(pg_blocking_pids(a.pid)) ORDER BY a.pid LIMIT 1")
                .bind(&schema).bind(gate_pid).fetch_optional(&admin).await.unwrap();
            if let Some(pid)=pid{break pid;}
            assert!(!worker.is_finished(),"worker must reach the actual publication transaction gate");
            tokio::task::yield_now().await;
        }
    }).await;
    let execution_pid=match execution{
        Ok(pid)=>pid,
        Err(_)=>{worker.abort();gate.rollback().await.unwrap();panic!("publication never reached its observed advisory lock wait");}
    };
    let heartbeat=timeout(Duration::from_secs(30),async{
        loop{
            let blocked:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity a WHERE a.application_name=$1 AND a.wait_event_type='Lock' AND a.query LIKE 'UPDATE runs_jobs j SET lease_until=%' AND $2=ANY(pg_blocking_pids(a.pid)))")
                .bind(&schema).bind(execution_pid).fetch_one(&admin).await.unwrap();
            if blocked{break;}
            assert!(!worker.is_finished(),"worker must remain live while its heartbeat waits for publication");
            tokio::task::yield_now().await;
        }
    }).await;
    if heartbeat.is_err(){
        worker.abort();
        gate.rollback().await.unwrap();
        panic!("actual worker heartbeat never contended for the publication job row");
    }
    gate.commit().await.unwrap();
    let completed=timeout(Duration::from_secs(5),&mut worker).await;
    if completed.is_err(){
        worker.abort();
        panic!("publication stopped being polled while heartbeat waited for its job row");
    }
    match completed.unwrap().unwrap(){
        Ok(processed)=>assert!(processed,"the worker claimed the real processing job"),
        // A heartbeat released after atomic job completion can no longer renew
        // the original holder. Consumer readiness below is the durable result.
        Err(error)=>assert_eq!(error.code(),"JOB_LEASE_LOST"),
    }
    let ready:(String,String,i64,i64)=sqlx::query_as("SELECT j.status,a.processing_status,j.desired_generation,m.generation FROM runs_jobs j JOIN runs_activities a ON a.id=j.activity_id JOIN runs_manifests m ON m.id=a.current_manifest_id WHERE j.activity_id=$1 AND j.stage='analysis'")
        .bind(activity.to_string()).fetch_one(&pool).await.unwrap();
    assert_eq!(ready.0,"ready");
    assert_eq!(ready.1,"ready");
    assert_eq!(ready.2,ready.3,"completed job must publish the matching coherent manifest generation");
    assert!(Box::pin(jobs::run_once(&pool)).await.unwrap(),"publication must leave real history ready to process");
    assert!(!Box::pin(jobs::run_once(&pool)).await.unwrap());
    let detail=Box::pin(run_detail(&pool,owner,activity)).await;
    assert_eq!(detail["processing"]["status"],"ready");
    assert_eq!(detail["processing"]["historyStatus"],"ready");
    let created=Box::pin(export::create(&pool,owner,export::ExportRequest{activity_ids:vec![activity],mode:export::ExportMode::Full,include_location:false,include_device_identifiers:false})).await.unwrap();
    let response=export::serve(&pool,owner,created["token"].as_str().unwrap()).await.unwrap();
    let served=Box::pin(axum::body::to_bytes(response.into_body(),10*1024*1024)).await.unwrap();
    let document:Value=serde_json::from_slice(&served).unwrap();
    assert_eq!(document["activities"][0]["id"],activity.to_string());
    assert_eq!(document["activities"][0]["historicalThresholds"]["computedAt"],detail["historicalThresholds"]["computedAt"]);
    assert_eq!(document["activities"][0]["normalized"]["startTime"],detail["normalized"]["startTime"]);
    close_store_pool(admin,pool,schema).await;
}

#[tokio::test(flavor="multi_thread",worker_threads=2)]
async fn revision_query_timeout_retries_real_processing_without_failing_the_run(){
    async fn configure_statement_timeout(pool:&PgPool,value:&str){
        // Hold every connection together so checkout cannot configure the same
        // session twice while leaving a reader session without the test bound.
        let mut connections=Vec::new();
        for _ in 0..6{connections.push(pool.acquire().await.unwrap());}
        for connection in &mut connections{
            sqlx::query("SELECT set_config('statement_timeout',$1,false)").bind(value).execute(&mut **connection).await.unwrap();
        }
    }
    let(admin,pool,schema,owner)=isolated_store_pool().await;
    let bytes=include_bytes!("fixtures/runs/garmin_run.fit");
    let mut document=Box::pin(jobs::decode_import(&pool,bytes)).await.unwrap();
    let admission=jobs::import_admission(&pool).await.unwrap();
    document.protect_cpu(&admission);
    let activity=Box::pin(store::accept_spool(&pool,owner,bytes,document,admission.holder())).await.unwrap().activity_id;
    drop(admission);
    let job:String=sqlx::query_scalar("SELECT id FROM runs_jobs WHERE activity_id=$1 AND stage='analysis' AND status='queued'")
        .bind(activity.to_string()).fetch_one(&pool).await.unwrap();
    let original_timeout:String=sqlx::query_scalar("SHOW statement_timeout").fetch_one(&pool).await.unwrap();
    configure_statement_timeout(&pool,"100ms").await;
    // A genuine PostgreSQL lock blocks only revision chunks. The worker can
    // claim, read metadata, and finish normally; its actual reader SELECT fails.
    let mut blocker=admin.begin().await.unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!("LOCK TABLE {schema}.runs_revision_chunks IN ACCESS EXCLUSIVE MODE"))).execute(&mut *blocker).await.unwrap();
    let failure=timeout(Duration::from_secs(5),Box::pin(jobs::run_once(&pool))).await.expect("actual revision query must hit its test statement timeout").unwrap_err();
    blocker.rollback().await.unwrap();
    configure_statement_timeout(&pool,&original_timeout).await;
    assert_eq!(failure.code(),"RUNS_STORAGE_FAILED","a PostgreSQL reader failure is retryable storage failure, not malformed JSON");
    let queued:(String,i32,Option<String>,String,Option<String>)=sqlx::query_as("SELECT j.status,j.attempts,j.error_code,a.processing_status,a.current_manifest_id FROM runs_jobs j JOIN runs_activities a ON a.id=j.activity_id WHERE j.id=$1")
        .bind(&job).fetch_one(&pool).await.unwrap();
    assert_eq!(queued,("queued".into(),1,Some("RUNS_STORAGE_FAILED".into()),"queued".into(),None),"storage fault must retain the original durable job for retry without a failed run or partial manifest");
    let pending=Box::pin(run_detail(&pool,owner,activity)).await;
    assert_eq!(pending["processing"]["status"],"queued");
    assert!(pending["revisionId"].is_null());
    assert!(pending["analysis"].is_null());
    assert!(pending["historicalThresholds"].is_null());
    sqlx::query("UPDATE runs_jobs SET next_attempt_at=now() WHERE id=$1").bind(&job).execute(&pool).await.unwrap();
    assert!(Box::pin(jobs::run_once(&pool)).await.unwrap(),"retry must consume the same real processing job");
    let ready:(String,i32,String,Option<String>,i64,i64)=sqlx::query_as("SELECT j.status,j.attempts,a.processing_status,j.error_code,j.desired_generation,m.generation FROM runs_jobs j JOIN runs_activities a ON a.id=j.activity_id JOIN runs_manifests m ON m.id=a.current_manifest_id WHERE j.id=$1")
        .bind(&job).fetch_one(&pool).await.unwrap();
    assert_eq!(ready.0,"ready");
    assert_eq!(ready.1,2,"the original job must succeed on its second attempt");
    assert_eq!(ready.2,"ready");
    assert_eq!(ready.3,None);
    assert_eq!(ready.4,ready.5,"retry must publish its coherent processing generation");
    assert!(Box::pin(jobs::run_once(&pool)).await.unwrap(),"retry must leave actual history to process");
    assert!(!Box::pin(jobs::run_once(&pool)).await.unwrap());
    let detail=Box::pin(run_detail(&pool,owner,activity)).await;
    assert_eq!(detail["processing"]["status"],"ready");
    assert_eq!(detail["processing"]["historyStatus"],"ready");
    let created=Box::pin(export::create(&pool,owner,export::ExportRequest{activity_ids:vec![activity],mode:export::ExportMode::Full,include_location:false,include_device_identifiers:false})).await.unwrap();
    let response=export::serve(&pool,owner,created["token"].as_str().unwrap()).await.unwrap();
    let served=Box::pin(axum::body::to_bytes(response.into_body(),10*1024*1024)).await.unwrap();
    let document:Value=serde_json::from_slice(&served).unwrap();
    assert_eq!(document["activities"][0]["id"],activity.to_string());
    assert_eq!(document["activities"][0]["historicalThresholds"]["computedAt"],detail["historicalThresholds"]["computedAt"]);
    assert_eq!(document["activities"][0]["normalized"]["startTime"],detail["normalized"]["startTime"]);
    close_store_pool(admin,pool,schema).await;
}

#[tokio::test]
async fn simultaneous_same_owner_bytes_keep_one_immutable_source_and_stable_activity(){
    let(admin,pool,schema,owner)=isolated_store_pool().await;
    let bytes=include_bytes!("fixtures/runs/garmin_run.fit");
    let mut first_document=Box::pin(jobs::decode_import(&pool,bytes)).await.unwrap();
    let mut second_document=Box::pin(jobs::decode_import(&pool,bytes)).await.unwrap();
    let admission=jobs::import_admission(&pool).await.unwrap();
    first_document.protect_cpu(&admission);
    second_document.protect_cpu(&admission);
    let(first,second)=tokio::join!(
        Box::pin(store::accept_spool(&pool,owner,bytes,first_document,admission.holder())),
        Box::pin(store::accept_spool(&pool,owner,bytes,second_document,admission.holder()))
    );
    let first=first.unwrap();let second=second.unwrap();
    assert_eq!(first.activity_id,second.activity_id);
    assert_ne!(first.duplicate,second.duplicate);
    let original:Vec<u8>=sqlx::query_scalar("SELECT bytes FROM runs_sources WHERE owner_id=$1").bind(owner.to_string()).fetch_one(&pool).await.unwrap();
    assert_eq!(original.as_slice(),bytes);
    let sources:i64=sqlx::query_scalar("SELECT COUNT(*) FROM runs_sources WHERE owner_id=$1").bind(owner.to_string()).fetch_one(&pool).await.unwrap();
    assert_eq!(sources,1);
    let immutable=sqlx::query("UPDATE runs_sources SET bytes=bytes WHERE owner_id=$1").bind(owner.to_string()).execute(&pool).await;
    assert!(immutable.is_err());
    assert!(store::delete(&pool,owner,first.activity_id).await.unwrap());
    let _ = Box::pin(jobs::run_once(&pool)).await.unwrap();
    let live:i64=sqlx::query_scalar("SELECT COUNT(*) FROM runs_activities WHERE id=$1").bind(first.activity_id.to_string()).fetch_one(&pool).await.unwrap();
    assert_eq!(live,0,"cancelled durable work cannot recreate a deleted run");
    let sources:i64=sqlx::query_scalar("SELECT COUNT(*) FROM runs_sources WHERE owner_id=$1").bind(owner.to_string()).fetch_one(&pool).await.unwrap();
    assert_eq!(sources,0,"erasure removes archived original bytes");
    sqlx::query("DELETE FROM users WHERE id=$1").bind(owner.to_string()).execute(&pool).await.unwrap();
    drop(admission);
    close_store_pool(admin,pool,schema).await;
}
