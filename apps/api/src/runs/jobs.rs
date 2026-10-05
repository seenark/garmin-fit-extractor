use axum::http::StatusCode;
use std::sync::atomic::{AtomicBool,Ordering};
use serde_json::{Value,json};
use sha2::{Digest,Sha256};
use sqlx::{PgPool,Row};
use tokio::time::{Duration,Instant,interval,sleep};
use uuid::Uuid;
use crate::error::ApiError;
use super::{process,store};

fn busy()->ApiError{ApiError::new(StatusCode::SERVICE_UNAVAILABLE,"DECODER_BUSY","The decoder is busy. Retry the import later.")}
fn lost()->ApiError{ApiError::new(StatusCode::SERVICE_UNAVAILABLE,"JOB_LEASE_LOST","Processing lease expired or was cancelled.")}
async fn acquire_slot(pool:&PgPool,stage:&str,holder:&str)->Result<bool,ApiError>{
    Ok(sqlx::query("UPDATE runs_slots SET lease_owner=$2,lease_until=now()+interval '90 seconds' WHERE stage=$1 AND slot=1 AND (lease_until IS NULL OR lease_until<now())")
        .bind(stage).bind(holder).execute(pool).await.map_err(store::database_error)?.rows_affected()==1)
}
async fn release_slot(pool:&PgPool,stage:&str,holder:&str)->Result<(),ApiError>{
    sqlx::query("UPDATE runs_slots SET lease_owner=NULL,lease_until=NULL WHERE stage=$1 AND lease_owner=$2")
        .bind(stage).bind(holder).execute(pool).await.map_err(store::database_error)?;Ok(())
}
async fn heartbeat_slot(pool:&PgPool,stage:&str,holder:&str)->Result<(),ApiError>{
    let updated=sqlx::query("UPDATE runs_slots SET lease_until=now()+interval '90 seconds' WHERE stage=$1 AND lease_owner=$2 AND lease_until>now()")
        .bind(stage).bind(holder).execute(pool).await.map_err(store::database_error)?.rows_affected();
    if updated!=1{return Err(lost());}Ok(())
}

pub(crate) struct SlotLease {
    pool:PgPool,
    holder:String,
    stage:&'static str,
    released:AtomicBool,
    heartbeat:tokio::task::JoinHandle<()>,
}
pub struct LeaseGuard {
    lease:std::sync::Arc<SlotLease>,
    cancelled:tokio::sync::watch::Receiver<bool>,
}
impl LeaseGuard {
    pub async fn cancelled(&mut self) {
        while !*self.cancelled.borrow() {
            if self.cancelled.changed().await.is_err(){return;}
        }
    }
    pub fn holder(&self)->&str{&self.lease.holder}
    pub(crate) fn cpu_hold(&self)->std::sync::Arc<SlotLease>{self.lease.clone()}
    async fn release(&mut self)->Result<(),ApiError>{
        if std::sync::Arc::strong_count(&self.lease)!=1{return Err(lost());}
        self.lease.heartbeat.abort();
        let result=tokio::time::timeout(Duration::from_secs(5),release_slot(&self.lease.pool,self.lease.stage,&self.lease.holder))
            .await.map_err(|_|lost())?;
        if result.is_ok(){self.lease.released.store(true,Ordering::Release);}result
    }
}
impl Drop for SlotLease {
    fn drop(&mut self){
        self.heartbeat.abort();
        if self.released.load(Ordering::Acquire){return;}
        let pool=self.pool.clone();let holder=self.holder.clone();let stage=self.stage;
        if let Ok(runtime)=tokio::runtime::Handle::try_current(){
            runtime.spawn(async move{
                let _=tokio::time::timeout(Duration::from_secs(5),release_slot(&pool,stage,&holder)).await;
            });
        }
    }
}
pub async fn import_admission(pool:&PgPool)->Result<LeaseGuard,ApiError>{
    let holder=Uuid::new_v4().to_string();
    if !acquire_slot(pool,"import",&holder).await?{
        return Err(ApiError::new(StatusCode::SERVICE_UNAVAILABLE,"IMPORT_BUSY","Another import is being staged. Retry later."));
    }
    Ok(lease_guard(pool,"import",holder))
}
fn lease_guard(pool:&PgPool,stage:&'static str,holder:String)->LeaseGuard{
    let (signal,cancelled)=tokio::sync::watch::channel(false);
    let heartbeat_pool=pool.clone();let heartbeat_holder=holder.clone();
    let heartbeat=tokio::spawn(async move{
        let mut timer=interval(Duration::from_secs(10));timer.tick().await;
        loop {
            timer.tick().await;
            if heartbeat_slot(&heartbeat_pool,stage,&heartbeat_holder).await.is_err(){
                let _=signal.send(true);
            }
        }
    });
    LeaseGuard{lease:std::sync::Arc::new(SlotLease{pool:pool.clone(),holder,stage,released:AtomicBool::new(false),heartbeat}),cancelled}
}
/// Import, restore, and reprocess share the same PostgreSQL decoder lease.
pub async fn decode_import(pool:&PgPool,bytes:&[u8])->Result<process::DecodedSpool,ApiError>{
    if bytes.is_empty() || bytes.len()>process::MAX_INPUT{return Err(ApiError::file_too_large());}
    let holder=Uuid::new_v4().to_string();let deadline=Instant::now()+Duration::from_secs(65);
    while !acquire_slot(pool,"decode",&holder).await? {
        if Instant::now()>=deadline{return Err(busy());}sleep(Duration::from_millis(100)).await;
    }
    let mut guard=lease_guard(pool,"decode",holder);
    let result={
        let future=process::decode(bytes);tokio::pin!(future);
        tokio::select!{
            result=&mut future=>result,
            _=guard.cancelled()=>Err(lost())
        }
    };
    let release=guard.release().await;
    match result{Ok(value)=>{release?;Ok(value)},Err(error)=>Err(error)}
}

struct Job {id:String,owner:String,activity:String,stage:String,generation:i64,holder:String,attempts:i32,analysis_running:AtomicBool,cancelled:AtomicBool}
pub fn start_worker(pool:PgPool){
    tokio::spawn(async move{
        let _=tokio::task::spawn_blocking(process::sweep_spools).await;
        let mut next_sweep=Instant::now()+Duration::from_secs(3600);
        let mut next_scan=Instant::now();
        loop {
            if Instant::now()>=next_sweep{let _=tokio::task::spawn_blocking(process::sweep_spools).await;next_sweep=Instant::now()+Duration::from_secs(3600);}
            if Instant::now()>=next_scan{
                if let Err(error)=scan_versions(&pool).await{tracing::warn!(code=error.code(),"Runs version scan failed");}
                next_scan=Instant::now()+Duration::from_secs(60);
            }
            match run_once(&pool).await{
                Ok(true)=>{},Ok(false)=>sleep(Duration::from_secs(1)).await,
                Err(error)=>{tracing::warn!(code=error.code(),"Runs job failed");sleep(Duration::from_secs(1)).await;}
            }
        }
    });
}
async fn recover(pool:&PgPool)->Result<(),ApiError>{
    let mut tx=pool.begin().await.map_err(store::database_error)?;
    sqlx::query("UPDATE runs_jobs SET status=CASE WHEN attempts<3 THEN 'queued' ELSE 'failed' END,error_code='WORKER_LEASE_EXPIRED',lease_owner=NULL,lease_until=NULL,next_attempt_at=now()+make_interval(secs=>least(30,attempts*attempts)) WHERE status='processing' AND lease_until<now()")
        .execute(&mut *tx).await.map_err(store::database_error)?;
    sqlx::query("UPDATE runs_activities a SET processing_status='failed',error_code='WORKER_LEASE_EXPIRED' WHERE EXISTS(SELECT 1 FROM runs_jobs j WHERE j.activity_id=a.id AND j.desired_generation=a.desired_generation AND j.stage<>'history' AND j.status='failed' AND j.error_code='WORKER_LEASE_EXPIRED')")
        .execute(&mut *tx).await.map_err(store::database_error)?;
    tx.commit().await.map_err(store::database_error)
}
async fn claim(pool:&PgPool,holder:&str)->Result<Option<Job>,ApiError>{
    let mut tx=pool.begin().await.map_err(store::database_error)?;
    let row=sqlx::query("SELECT j.id,j.owner_id,j.activity_id,j.stage,j.desired_generation,j.attempts FROM runs_jobs j JOIN runs_activities a ON a.id=j.activity_id AND (CASE WHEN j.stage='history' THEN a.history_generation ELSE a.desired_generation END)=j.desired_generation WHERE j.status='queued' AND j.next_attempt_at<=now() AND j.attempts<3 AND NOT EXISTS(SELECT 1 FROM runs_tombstones t WHERE t.activity_id=j.activity_id) ORDER BY j.created_at,j.id FOR UPDATE OF j SKIP LOCKED LIMIT 1")
        .fetch_optional(&mut *tx).await.map_err(store::database_error)?;
    let Some(row)=row else{return Ok(None)};
    let job=Job{id:row.get("id"),owner:row.get("owner_id"),activity:row.get("activity_id"),stage:row.get("stage"),generation:row.get("desired_generation"),holder:holder.to_owned(),attempts:row.get::<i32,_>("attempts")+1,analysis_running:AtomicBool::new(false),cancelled:AtomicBool::new(false)};
    sqlx::query("UPDATE runs_jobs SET status='processing',attempts=attempts+1,lease_owner=$2,lease_until=now()+interval '90 seconds',heartbeat_at=now(),error_code=NULL WHERE id=$1")
        .bind(&job.id).bind(holder).execute(&mut *tx).await.map_err(store::database_error)?;
    if job.stage!="history"{sqlx::query("UPDATE runs_activities SET processing_status='processing' WHERE id=$1 AND desired_generation=$2").bind(&job.activity).bind(job.generation).execute(&mut *tx).await.map_err(store::database_error)?;}
    tx.commit().await.map_err(store::database_error)?;Ok(Some(job))
}
async fn heartbeat_job(pool:&PgPool,job:&Job)->Result<(),ApiError>{
    let updated=sqlx::query("UPDATE runs_jobs j SET lease_until=now()+interval '90 seconds',heartbeat_at=now() WHERE j.id=$1 AND j.lease_owner=$2 AND j.status='processing' AND j.lease_until>clock_timestamp() AND EXISTS(SELECT 1 FROM runs_slots WHERE stage='worker' AND lease_owner=$2 AND lease_until>clock_timestamp()) AND EXISTS(SELECT 1 FROM runs_activities a WHERE a.id=j.activity_id AND (CASE WHEN j.stage='history' THEN a.history_generation ELSE a.desired_generation END)=j.desired_generation) AND NOT EXISTS(SELECT 1 FROM runs_tombstones t WHERE t.activity_id=j.activity_id)")
        .bind(&job.id).bind(&job.holder).execute(pool).await.map_err(store::database_error)?.rows_affected();
    if updated!=1{return Err(lost());}Ok(())
}
pub async fn run_once(pool:&PgPool)->Result<bool,ApiError>{
    recover(pool).await?;let holder=Uuid::new_v4().to_string();
    if !acquire_slot(pool,"worker",&holder).await?{return Ok(false);}
    let mut guard=lease_guard(pool,"worker",holder.clone());
    let claim=claim(pool,&holder).await;
    let job=match claim{Ok(Some(job))=>job,Ok(None)=>{guard.release().await?;return Ok(false);},Err(error)=>{let _=guard.release().await;return Err(error);}};
    let result={
        let lease=guard.cpu_hold();let future=execute(pool,&job,&lease);tokio::pin!(future);
        // Keep execution polled while its publication row lock delays a heartbeat.
        let heartbeat=async{
            let mut timer=interval(Duration::from_secs(10));timer.tick().await;
            loop{
                timer.tick().await;
                if let Err(error)=heartbeat_job(pool,&job).await{break error;}
            }
        };tokio::pin!(heartbeat);
        tokio::select!{
            result=&mut future=>result,
            error=&mut heartbeat=>{
                job.cancelled.store(true,Ordering::Release);
                // A blocking engine call cannot be aborted. Retain its global slot
                // until it drains; decoder futures remain immediately killable.
                if job.analysis_running.load(Ordering::Acquire){
                    let _=(&mut future).await;
                }
                Err(error)
            }
        }
    };
    let finish=finish(pool,&job,result.as_ref().err()).await;
    let release=guard.release().await;
    finish?;release?;
    match result{Ok(())=>Ok(true),Err(error)=>Err(error)}
}
async fn finish(pool:&PgPool,job:&Job,error:Option<&ApiError>)->Result<(),ApiError>{
    let transient=error.is_some_and(|e|matches!(e.code(),"DECODER_UNAVAILABLE"|"DECODER_BUSY"|"RUNS_STORAGE_FAILED"|"JOB_LEASE_LOST"));
    let state=if error.is_none(){"ready"}else if transient && job.attempts<3{"queued"}else{"failed"};
    let mut tx=pool.begin().await.map_err(store::database_error)?;
    let changed=sqlx::query("UPDATE runs_jobs SET status=$3,error_code=$4,lease_owner=NULL,lease_until=NULL,next_attempt_at=now()+make_interval(secs=>$5) WHERE id=$1 AND lease_owner=$2 AND status='processing'")
        .bind(&job.id).bind(&job.holder).bind(state).bind(error.map(ApiError::code)).bind(f64::from(job.attempts*job.attempts)).execute(&mut *tx).await.map_err(store::database_error)?.rows_affected();
    if changed==1 && job.stage!="history" && error.is_some(){sqlx::query("UPDATE runs_activities SET processing_status=$3,error_code=$4 WHERE id=$1 AND desired_generation=$2")
        .bind(&job.activity).bind(job.generation).bind(state).bind(error.map(ApiError::code)).execute(&mut *tx).await.map_err(store::database_error)?;}
    tx.commit().await.map_err(store::database_error)
}
async fn execute(pool:&PgPool,job:&Job,lease:&std::sync::Arc<SlotLease>)->Result<(),ApiError>{
    if job.stage=="history"{
        let owner=Uuid::parse_str(&job.owner).map_err(|_|lost())?;
        let activity=Uuid::parse_str(&job.activity).map_err(|_|lost())?;
        return crate::runs::history::recompute(pool,owner,activity,job.generation,&job.holder).await;
    }
    let staged=sqlx::query("SELECT d.id AS decoded_id,n.id AS normalized_id,n.payload::text AS normalized,n.legacy_projection::text AS legacy FROM runs_revisions n JOIN runs_revisions d ON d.id=n.input_revision_ids->>0 WHERE n.activity_id=$1 AND n.stage='normalized' AND n.generation=$2")
        .bind(&job.activity).bind(job.generation).fetch_optional(pool).await.map_err(store::database_error)?;
    let (decoded_id,normalized_id,metadata,legacy)=if let Some(row)=staged{
        let metadata=store::parse(row.try_get::<&str,_>("normalized").map_err(store::database_error)?)?;
        let legacy:crate::model::Analysis=serde_json::from_str(row.try_get::<&str,_>("legacy").map_err(store::database_error)?).map_err(|_|store::invalid())?;
        (row.try_get::<String,_>("decoded_id").map_err(store::database_error)?,row.try_get::<String,_>("normalized_id").map_err(store::database_error)?,metadata,legacy)
    }else if job.stage=="analysis"{
        let row=sqlx::query("SELECT m.decoded_revision_id,m.normalized_revision_id,n.payload::text AS normalized,n.legacy_projection::text AS legacy FROM runs_activities a JOIN runs_manifests m ON m.id=a.current_manifest_id JOIN runs_revisions n ON n.id=m.normalized_revision_id WHERE a.id=$1 AND a.desired_generation=$2")
            .bind(&job.activity).bind(job.generation).fetch_optional(pool).await.map_err(store::database_error)?.ok_or_else(lost)?;
        let metadata=store::parse(row.try_get::<&str,_>("normalized").map_err(store::database_error)?)?;
        let legacy:crate::model::Analysis=serde_json::from_str(row.try_get::<&str,_>("legacy").map_err(store::database_error)?).map_err(|_|store::invalid())?;
        (row.try_get("decoded_revision_id").map_err(store::database_error)?,row.try_get("normalized_revision_id").map_err(store::database_error)?,metadata,legacy)
    }else{
        let row=sqlx::query("SELECT s.bytes,s.sha256,s.size_bytes FROM runs_sources s JOIN runs_activities a ON a.source_id=s.id AND a.owner_id=s.owner_id WHERE a.id=$1 AND a.desired_generation=$2")
            .bind(&job.activity).bind(job.generation).fetch_optional(pool).await.map_err(store::database_error)?.ok_or_else(lost)?;
        let bytes:&[u8]=row.try_get("bytes").map_err(store::database_error)?;let hash:&[u8]=row.try_get("sha256").map_err(store::database_error)?;
        let actual=Sha256::digest(bytes);
        if row.try_get::<i64,_>("size_bytes").map_err(store::database_error)?!=bytes.len() as i64 || actual[..]!=*hash{return Err(ApiError::new(StatusCode::CONFLICT,"SOURCE_INTEGRITY_FAILED","Stored source integrity verification failed."));}
        let source_hash=format!("{actual:x}");
        let mut spool=decode_import(pool,bytes).await?;
        spool.hold_cpu(lease.clone());
        job.analysis_running.store(true,Ordering::Release);
        let (_spool,staged)=tokio::task::spawn_blocking(move||{let staged=super::stream::split_run(&spool.run,&spool.directory)?;Ok::<_,ApiError>((spool,staged))}).await.map_err(|_|store::invalid())??;
        job.analysis_running.store(false,Ordering::Release);
        if job.cancelled.load(Ordering::Acquire){return Err(lost());}
        let mut tx=pool.begin().await.map_err(store::database_error)?;store::owner_lock(&mut tx,&job.owner).await.map_err(store::database_error)?;fence(&mut tx,job).await?;
        let (decoded,normalized)=store::stage_documents(&mut tx,&job.owner,&job.activity,job.generation,&staged,&source_hash).await?;
        let mut metadata=staged.normalized_metadata;
        metadata["sourceHash"]=json!(source_hash);
        metadata["documents"]=json!({"archive":staged.normalized.metadata,"analysisInput":staged.analysis_input.metadata});
        metadata["projectionVersion"]=json!(crate::runs::thresholds::PROJECTION_VERSION);
        fence(&mut tx,job).await?;tx.commit().await.map_err(store::database_error)?;
        (decoded,normalized,metadata,staged.legacy)
    };
    let normalized_hash=metadata["documents"]["archive"]["sha256"].as_str().ok_or_else(store::invalid)?.to_owned();
    if metadata["projectionVersion"]!=crate::runs::thresholds::PROJECTION_VERSION{return Err(store::invalid());}
    let descriptor=metadata["documents"]["analysisInput"].clone();let owner=job.owner.clone();let revision=normalized_id.clone();let read_pool=pool.clone();
    let cpu_hold=lease.clone();
    let source_hash=metadata["sourceHash"].as_str().ok_or_else(store::invalid)?.to_owned();
    job.analysis_running.store(true,Ordering::Release);
    let (analysis,input_hash)=tokio::task::spawn_blocking(move||{
        let _cpu_hold=cpu_hold;
        let reader=super::stream::RevisionReader::new(read_pool,owner,revision.clone(),"analysisInput",&descriptor)?;
        let mut input:Value=serde_json::from_reader(reader).map_err(|error|{
            if error.io_error_kind()==Some(std::io::ErrorKind::ConnectionAborted){
                ApiError::new(StatusCode::INTERNAL_SERVER_ERROR,"RUNS_STORAGE_FAILED","Run storage operation failed.")
            }else{store::invalid()}
        })?;
        input["sourceRevision"]=json!(revision);
        input["sourceHash"]=json!(source_hash);
        input["normalizedDocumentHash"]=json!(normalized_hash);
        input["projectionVersion"]=json!(crate::runs::thresholds::PROJECTION_VERSION);
        let mut hash=Sha256::new();serde_json::to_writer(&mut hash,&input).map_err(|_|store::invalid())?;
        let digest=format!("{:x}",hash.finalize());
        let analysis=crate::runs::analysis::analyze(&input);
        Ok::<_,ApiError>((analysis,digest))
    }).await.map_err(|_|ApiError::new(StatusCode::INTERNAL_SERVER_ERROR,"ANALYSIS_FAILED","Run analysis failed."))??;
    job.analysis_running.store(false,Ordering::Release);
    if job.cancelled.load(Ordering::Acquire){return Err(lost());}
    validate_analysis(&analysis,&metadata)?;
    let normalized_hash=metadata["documents"]["archive"]["sha256"].as_str().ok_or_else(store::invalid)?;
    let config=store::versions()["config"].as_str().ok_or_else(store::invalid)?.to_owned();
    for (target,method) in [("lt1","running-dfa-a1-075"),("lt2","running-dfa-a1-050")]{
        let value=&analysis["thresholds"][target];
        if value["trace"]["inputHash"]!=input_hash || value["trace"]["inputRevision"]!=normalized_id ||
            value["trace"]["inputNormalizedHash"]!=normalized_hash || value["trace"]["inputProjectionVersion"]!=crate::runs::thresholds::PROJECTION_VERSION ||
            value["method"]["id"]!=method || value["method"]["version"]!=crate::runs::thresholds::METHOD_VERSION || value["method"]["configurationHash"]!=config{return Err(store::invalid());}
    }
    let validation=json!({"status":"validated","inputHash":input_hash,"projectionVersion":crate::runs::thresholds::PROJECTION_VERSION,"normalizedRevisionId":normalized_id,"normalizedDocumentHash":normalized_hash,"methodVersion":crate::runs::thresholds::METHOD_VERSION,"configurationHash":config});
    let mut tx=pool.begin().await.map_err(store::database_error)?;store::owner_lock(&mut tx,&job.owner).await.map_err(store::database_error)?;fence(&mut tx,job).await?;
    let analysis_id=store::insert_revision(&mut tx,&job.owner,&job.activity,"analysis",job.generation,&json!([normalized_id]),&analysis,None,&validation).await?;
    let manifest=Uuid::now_v7().to_string();
    sqlx::query("INSERT INTO runs_manifests(id,owner_id,activity_id,decoded_revision_id,normalized_revision_id,analysis_revision_id,generation,versions) VALUES($1,$2,$3,$4,$5,$6,$7,$8::jsonb)")
        .bind(&manifest).bind(&job.owner).bind(&job.activity).bind(&decoded_id).bind(&normalized_id).bind(analysis_id).bind(job.generation).bind(store::versions().to_string()).execute(&mut *tx).await.map_err(store::database_error)?;
    let changed=sqlx::query("UPDATE runs_activities SET current_manifest_id=$3,summary=$4::jsonb,start_time=$5,end_time=$6,start_order=$5::timestamptz,end_order=$6::timestamptz,subtype=$7,processing_status='ready',error_code=NULL WHERE id=$1 AND desired_generation=$2 AND NOT EXISTS(SELECT 1 FROM runs_tombstones WHERE activity_id=$1)")
        .bind(&job.activity).bind(job.generation).bind(manifest).bind(metadata["summary"].to_string()).bind(metadata["startTime"].as_str()).bind(metadata["endTime"].as_str()).bind(metadata["subtype"].as_str()).execute(&mut *tx).await.map_err(store::database_error)?.rows_affected();
    if changed!=1{return Err(lost());}
    sqlx::query("INSERT INTO activities(id,owner_id,sport,started_at,activity_data,created_at) VALUES($1,$2,'running',$3,$4,$5) ON CONFLICT(id) DO UPDATE SET sport=EXCLUDED.sport,started_at=EXCLUDED.started_at,activity_data=EXCLUDED.activity_data WHERE activities.owner_id=EXCLUDED.owner_id")
        .bind(&job.activity).bind(&job.owner).bind(metadata["startTime"].as_str()).bind(serde_json::to_string(&legacy).map_err(|_|lost())?).bind(crate::db::created_at_now()).execute(&mut *tx).await.map_err(store::database_error)?;
    store::invalidate_cutoffs(&mut tx,&job.owner,metadata["endTime"].as_str().ok_or_else(lost)?).await.map_err(store::database_error)?;
    let completed=sqlx::query("UPDATE runs_jobs j SET status='ready',error_code=NULL,lease_owner=NULL,lease_until=NULL WHERE j.id=$1 AND j.lease_owner=$2 AND j.status='processing' AND j.lease_until>clock_timestamp() AND EXISTS(SELECT 1 FROM runs_activities a WHERE a.id=j.activity_id AND a.desired_generation=j.desired_generation) AND EXISTS(SELECT 1 FROM runs_slots s WHERE s.stage='worker' AND s.lease_owner=$2 AND s.lease_until>clock_timestamp()) AND NOT EXISTS(SELECT 1 FROM runs_tombstones t WHERE t.activity_id=j.activity_id)")
        .bind(&job.id).bind(&job.holder).execute(&mut *tx).await.map_err(store::database_error)?.rows_affected();
    if completed!=1 || job.cancelled.load(Ordering::Acquire){return Err(lost());}
    tx.commit().await.map_err(store::database_error)
}
async fn fence(tx:&mut sqlx::Transaction<'_,sqlx::Postgres>,job:&Job)->Result<(),ApiError>{
    let live=sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM runs_activities a JOIN runs_jobs j ON j.activity_id=a.id WHERE a.id=$1 AND a.desired_generation=$2 AND j.id=$3 AND j.status='processing' AND j.lease_owner=$4 AND j.lease_until>clock_timestamp() AND EXISTS(SELECT 1 FROM runs_slots s WHERE s.stage='worker' AND s.lease_owner=$4 AND s.lease_until>clock_timestamp()) AND NOT EXISTS(SELECT 1 FROM runs_tombstones t WHERE t.activity_id=a.id))")
        .bind(&job.activity).bind(job.generation).bind(&job.id).bind(&job.holder).fetch_one(&mut **tx).await.map_err(store::database_error)?;
    if !live{return Err(lost());}Ok(())
}
fn validate_analysis(analysis:&Value,normalized:&Value)->Result<(),ApiError>{
    let error=||ApiError::new(StatusCode::UNPROCESSABLE_ENTITY,"INVALID_ANALYSIS_DOCUMENT","Analysis failed semantic validation.");
    if analysis["schemaVersion"]!="2.0.0" || !analysis["quality"].is_object() || !analysis["thresholds"].is_object() || !analysis["transformations"].is_array(){return Err(error());}
    let duration=(store::timestamp(&normalized["endTime"])?-store::timestamp(&normalized["startTime"])?).num_milliseconds() as f64/1000.0;
    for segment in analysis["segments"].as_array().ok_or_else(error)?{
        let start=segment["startElapsedSeconds"].as_f64().ok_or_else(error)?;
        let end=segment["endElapsedSeconds"].as_f64().ok_or_else(error)?;
        if !start.is_finite() || !end.is_finite() || start<0.0 || end<start || end>duration+0.001{return Err(error());}
    }
    for target in ["lt1","lt2"] {
        let target=&analysis["thresholds"][target];
        if !matches!(target["status"].as_str(),Some("estimated"|"low_confidence"|"insufficient_data")){return Err(error());}
        if !target["value"].is_null() && target["value"]["heartRateBpm"].as_f64().is_none_or(|n|!n.is_finite() || n<=0.0){return Err(error());}
    } Ok(())
}
pub async fn scan_versions(pool:&PgPool)->Result<(),ApiError>{
    let rows=sqlx::query("SELECT id,owner_id,current_manifest_id FROM runs_activities WHERE desired_versions<>$1::jsonb ORDER BY id LIMIT 64")
        .bind(store::versions().to_string()).fetch_all(pool).await.map_err(store::database_error)?;
    for row in rows{
        let id:String=row.get("id");let owner:String=row.get("owner_id");let manifest:Option<String>=row.get("current_manifest_id");
        let mut tx=pool.begin().await.map_err(store::database_error)?;store::owner_lock(&mut tx,&owner).await.map_err(store::database_error)?;
        let current=sqlx::query("SELECT desired_versions::text AS versions,desired_generation FROM runs_activities WHERE id=$1 AND desired_versions<>$2::jsonb FOR UPDATE")
            .bind(&id).bind(store::versions().to_string()).fetch_optional(&mut *tx).await.map_err(store::database_error)?;
        let Some(current)=current else{continue};let old=store::parse(current.get::<&str,_>("versions"))?;let desired=store::versions();
        let reuse=manifest.is_some() && ["schema","decoder","observationRule","projectionVersion"].iter().all(|key|old[*key]==desired[*key]);
        let history_only=desired.as_object().is_some_and(|versions|versions.iter().all(|(key,value)|key=="history" || key=="historySourceHash" || old[key]==*value));
        if history_only {
            let generation=sqlx::query_scalar::<_,i64>("UPDATE runs_activities SET desired_versions=$2::jsonb,history_generation=history_generation+1 WHERE id=$1 RETURNING history_generation")
                .bind(&id).bind(desired.to_string()).fetch_one(&mut *tx).await.map_err(store::database_error)?;
            sqlx::query("UPDATE runs_jobs SET status='cancelled',lease_owner=NULL,lease_until=NULL WHERE activity_id=$1 AND stage='history' AND status IN ('queued','processing')").bind(&id).execute(&mut *tx).await.map_err(store::database_error)?;
            if manifest.is_some(){store::enqueue(&mut tx,&owner,&id,"history",generation,&json!([])).await.map_err(store::database_error)?;}
            tx.commit().await.map_err(store::database_error)?;continue;
        }
        let generation=current.get::<i64,_>("desired_generation")+1;
        sqlx::query("UPDATE runs_jobs SET status='cancelled',lease_owner=NULL,lease_until=NULL WHERE activity_id=$1 AND stage<>'history' AND status IN ('queued','processing')").bind(&id).execute(&mut *tx).await.map_err(store::database_error)?;
        sqlx::query("UPDATE runs_activities SET desired_generation=$2,desired_versions=$3::jsonb,processing_status='queued',error_code=NULL WHERE id=$1")
            .bind(&id).bind(generation).bind(desired.to_string()).execute(&mut *tx).await.map_err(store::database_error)?;
        store::enqueue(&mut tx,&owner,&id,if reuse{"analysis"}else{"process"},generation,&json!([])).await.map_err(store::database_error)?;
        tx.commit().await.map_err(store::database_error)?;
    } Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor="multi_thread",worker_threads=2)]
    #[ignore="Requires the verified disposable Runs PostgreSQL environment."]
    async fn cpu_hold_retains_original_slot_until_blocking_work_finishes(){
        let url=std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL");
        let expected=std::env::var("PGDATA").expect("PGDATA");
        assert!(expected.starts_with("/tmp/garmin-runs-core-pg."));
        let root=PgPool::connect(&url).await.unwrap();
        let actual:String=sqlx::query_scalar("SHOW data_directory").fetch_one(&root).await.unwrap();
        assert_eq!(actual,expected);
        let schema=format!("runs_lease_{}",Uuid::new_v4().simple());
        sqlx::QueryBuilder::<sqlx::Postgres>::new(format!("CREATE SCHEMA {schema}")).build().execute(&root).await.unwrap();
        let selected=schema.clone();
        let pool=sqlx::postgres::PgPoolOptions::new().max_connections(3).after_connect(move|connection,_|{
            let schema=selected.clone();
            Box::pin(async move{sqlx::query("SELECT set_config('search_path',$1,false)").bind(schema).execute(connection).await?;Ok(())})
        }).connect(&url).await.unwrap();
        sqlx::query("CREATE TABLE runs_slots(stage TEXT PRIMARY KEY,slot INTEGER NOT NULL,lease_owner TEXT,lease_until TIMESTAMPTZ)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO runs_slots(stage,slot) VALUES('import',1)").execute(&pool).await.unwrap();
        let admission=import_admission(&pool).await.unwrap();
        let original=admission.holder().to_owned();let hold=admission.cpu_hold();
        let (started,ready)=tokio::sync::oneshot::channel();
        let (finish,gate)=std::sync::mpsc::channel();
        let cpu=tokio::task::spawn_blocking(move||{let _hold=hold;started.send(()).unwrap();gate.recv().unwrap();});
        ready.await.unwrap();drop(admission);
        let rejection=match import_admission(&pool).await{Err(error)=>error,Ok(_)=>panic!("CPU work released admission early")};
        assert_eq!(rejection.code(),"IMPORT_BUSY");
        let holder:Option<String>=sqlx::query_scalar("SELECT lease_owner FROM runs_slots WHERE stage='import'").fetch_one(&pool).await.unwrap();
        assert_eq!(holder.as_deref(),Some(original.as_str()));
        finish.send(()).unwrap();cpu.await.unwrap();
        let mut next=tokio::time::timeout(Duration::from_secs(5),async{
            loop{match import_admission(&pool).await{Ok(guard)=>break guard,Err(error)=>{assert_eq!(error.code(),"IMPORT_BUSY");sleep(Duration::from_millis(5)).await;}}}
        }).await.expect("CPU cleanup did not release admission");
        release_slot(&pool,"import",&original).await.unwrap();
        let holder:Option<String>=sqlx::query_scalar("SELECT lease_owner FROM runs_slots WHERE stage='import'").fetch_one(&pool).await.unwrap();
        assert_eq!(holder.as_deref(),Some(next.holder()),"Old cleanup released a new holder");
        next.release().await.unwrap();drop(next);
        pool.close().await;
        sqlx::QueryBuilder::<sqlx::Postgres>::new(format!("DROP SCHEMA {schema} CASCADE")).build().execute(&root).await.unwrap();
        root.close().await;
    }

    #[tokio::test(flavor="multi_thread",worker_threads=2)]
    #[ignore="Requires the verified disposable Runs PostgreSQL environment and decoder binary."]
    async fn published_generation_survives_worker_loss_before_finish(){
        let url=std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL");
        assert_eq!(url,"postgresql://atiwatseenark@127.0.0.1:55201/garmin_runs_core_test");
        let expected=std::env::var("PGDATA").expect("PGDATA");
        assert!(expected.starts_with("/tmp/garmin-runs-core-pg."));
        let root=PgPool::connect(&url).await.unwrap();
        let actual:String=sqlx::query_scalar("SHOW data_directory").fetch_one(&root).await.unwrap();
        assert_eq!(actual,expected);
        let schema=format!("runs_publish_{}",Uuid::new_v4().simple());
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE SCHEMA {schema}"))).execute(&root).await.unwrap();
        let selected=schema.clone();
        let pool=sqlx::postgres::PgPoolOptions::new().max_connections(5).after_connect(move|connection,_|{
            let schema=selected.clone();
            Box::pin(async move{sqlx::query("SELECT set_config('search_path',$1,false)").bind(schema).execute(connection).await?;Ok(())})
        }).connect(&url).await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let owner=Uuid::new_v4();
        sqlx::query("INSERT INTO users(id,google_subject,email,created_at,updated_at) VALUES($1,$2,$3,$4,$4)")
            .bind(owner.to_string()).bind(format!("publish-test:{owner}")).bind(format!("{owner}@example.test")).bind(crate::db::created_at_now()).execute(&pool).await.unwrap();
        let bytes=include_bytes!("../../tests/fixtures/runs/garmin_run.fit");
        let document=decode_import(&pool,bytes).await.unwrap();
        let mut admission=import_admission(&pool).await.unwrap();
        let imported=store::accept_spool(&pool,owner,bytes,document,admission.holder()).await.unwrap();
        admission.release().await.unwrap();
        let holder=Uuid::new_v4().to_string();
        assert!(acquire_slot(&pool,"worker",&holder).await.unwrap());
        let mut guard=lease_guard(&pool,"worker",holder.clone());
        let job=claim(&pool,&holder).await.unwrap().unwrap();
        assert_eq!(job.activity,imported.activity_id.to_string());
        let hold=guard.cpu_hold();
        execute(&pool,&job,&hold).await.unwrap();
        drop(hold);
        // Simulate a dead original worker after publication, without ever calling finish.
        guard.release().await.unwrap();
        sqlx::query("UPDATE runs_jobs SET lease_until=clock_timestamp()-interval '1 second' WHERE id=$1 AND status='processing' AND lease_owner=$2")
            .bind(&job.id).bind(&holder).execute(&pool).await.unwrap();
        recover(&pool).await.unwrap();
        sqlx::query("UPDATE runs_jobs SET next_attempt_at=clock_timestamp() WHERE status='queued'").execute(&pool).await.unwrap();
        assert!(run_once(&pool).await.unwrap(),"Recovery must compute real history, not retry the published analysis");
        assert!(!run_once(&pool).await.unwrap(),"Published generation must not be requeued");
        let row=sqlx::query("SELECT a.current_manifest_id,a.processing_status,j.status AS job_status,j.lease_owner FROM runs_activities a JOIN runs_jobs j ON j.activity_id=a.id WHERE j.id=$1")
            .bind(&job.id).fetch_one(&pool).await.unwrap();
        assert_eq!(row.get::<String,_>("processing_status"),"ready");
        assert_eq!(row.get::<String,_>("job_status"),"ready");
        assert!(row.get::<Option<String>,_>("lease_owner").is_none());
        let history=super::super::history::for_manifest(&pool,owner,imported.activity_id,row.get::<Option<String>,_>("current_manifest_id").as_deref(),
            sqlx::query_scalar("SELECT history_generation FROM runs_activities WHERE id=$1").bind(&job.activity).fetch_one(&pool).await.unwrap()).await.unwrap();
        assert_eq!(history["activityId"],job.activity);
        assert_eq!(history["lt1"]["status"],"insufficient_data");
        let request=super::super::export::ExportRequest{activity_ids:vec![imported.activity_id],mode:super::super::export::ExportMode::Full,include_location:false,include_device_identifiers:false};
        let created=super::super::export::create(&pool,owner,request).await.unwrap();
        let response=super::super::export::serve(&pool,owner,created["token"].as_str().unwrap()).await.unwrap();
        let exported:Value=serde_json::from_slice(&axum::body::to_bytes(response.into_body(),usize::MAX).await.unwrap()).unwrap();
        assert_eq!(exported["activities"][0]["historicalThresholds"]["computedAt"],history["computedAt"]);
        pool.close().await;
        sqlx::query(sqlx::AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE"))).execute(&root).await.unwrap();
        root.close().await;
    }
}
