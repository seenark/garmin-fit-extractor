use std::{cell::RefCell, collections::{BTreeMap, HashSet}, io::{self, Write}, rc::Rc, time::{Duration, Instant}};
use axum::{body::{Body, Bytes}, http::{HeaderValue, StatusCode, header}, response::Response};
use chrono::{SecondsFormat, Utc};
use futures_util::stream;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;
use crate::error::ApiError;
use super::{privacy::{self, Document, Policy, Projection}, stream::{CHUNK_BYTES, RevisionReader, project_document}};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExportMode { Coach, Full }

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportRequest {
    pub activity_ids: Vec<Uuid>,
    pub mode: ExportMode,
    #[serde(default)] pub include_location: bool,
    #[serde(default)] pub include_device_identifiers: bool,
}

fn invalid_selection() -> ApiError { ApiError::new(StatusCode::UNPROCESSABLE_ENTITY,"INVALID_EXPORT_SELECTION","Select distinct, owned activities with coherent ready revisions.") }
fn not_ready() -> ApiError { ApiError::new(StatusCode::UNPROCESSABLE_ENTITY,"EXPORT_NOT_READY","Wait for coherent analysis and the historical threshold snapshot before exporting.") }
fn unavailable() -> ApiError { ApiError::new(StatusCode::NOT_FOUND,"EXPORT_UNAVAILABLE","The export is unavailable, expired, or revoked.") }
fn internal() -> ApiError { ApiError::new(StatusCode::INTERNAL_SERVER_ERROR,"EXPORT_FAILED","The export could not be completed.") }
fn validate_selection(request: &ExportRequest) -> Result<(), ApiError> {
    if request.activity_ids.is_empty() || request.activity_ids.iter().collect::<HashSet<_>>().len()!=request.activity_ids.len() { return Err(invalid_selection()); }
    Ok(())
}

pub async fn create(pool: &PgPool, owner: Uuid, request: ExportRequest) -> Result<Value, ApiError> {
    validate_selection(&request)?;
    let ids: Vec<_> = request.activity_ids.iter().map(Uuid::to_string).collect();
    let owner = owner.to_string();
    let mut transaction = pool.begin().await.map_err(|_|internal())?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))").bind(&owner).execute(&mut *transaction).await.map_err(|_|internal())?;
    let rows = sqlx::query("SELECT a.id,m.id AS manifest_id,m.decoded_revision_id,m.normalized_revision_id,m.analysis_revision_id,e.id AS estimate_id FROM runs_activities a JOIN runs_manifests m ON m.id=a.current_manifest_id AND m.owner_id=a.owner_id AND m.activity_id=a.id JOIN runs_revisions d ON d.id=m.decoded_revision_id AND d.owner_id=a.owner_id AND d.activity_id=a.id AND d.stage='decoded' JOIN runs_revisions n ON n.id=m.normalized_revision_id AND n.owner_id=a.owner_id AND n.activity_id=a.id AND n.stage='normalized' JOIN runs_revisions r ON r.id=m.analysis_revision_id AND r.owner_id=a.owner_id AND r.activity_id=a.id AND r.stage='analysis' JOIN runs_estimates e ON e.activity_id=a.id AND e.owner_id=a.owner_id AND e.manifest_id=m.id AND e.generation=a.history_generation AND e.evidence_cutoff=a.end_time WHERE a.owner_id=$1 AND a.id=ANY($2) FOR SHARE OF a")
        .bind(&owner).bind(&ids).fetch_all(&mut *transaction).await.map_err(|_|internal())?;
    if rows.len()!=ids.len() {
        let owned:i64=sqlx::query_scalar("SELECT count(*) FROM (SELECT id FROM runs_activities WHERE owner_id=$1 AND id=ANY($2) UNION SELECT id FROM extractions WHERE user_id=$1 AND id=ANY($2)) AS selected")
            .bind(&owner).bind(&ids).fetch_one(&mut *transaction).await.map_err(|_|internal())?;
        if owned!=ids.len() as i64 {return Err(ApiError::not_found());}
        let legacy: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM extractions WHERE user_id=$1 AND id=ANY($2))").bind(&owner).bind(&ids).fetch_one(&mut *transaction).await.map_err(|_|internal())?;
        if legacy { return Err(ApiError::new(StatusCode::UNPROCESSABLE_ENTITY,"LEGACY_EXPORT_UNSUPPORTED","Original source and a compatible export revision are unavailable.")); }
        return Err(not_ready());
    }
    let mut by_id = BTreeMap::new();
    for row in rows { by_id.insert(row.try_get::<String,_>("id").map_err(|_|internal())?,row); }
    let mut pins = Vec::with_capacity(ids.len());
    let mut manifests = Vec::with_capacity(ids.len());
    for id in &ids {
        let row = by_id.remove(id).ok_or_else(invalid_selection)?;
        manifests.push(row.try_get::<String,_>("manifest_id").map_err(|_|internal())?);
        pins.push(Pin{id:id.clone(),decoded:row.try_get("decoded_revision_id").map_err(|_|internal())?,normalized:row.try_get("normalized_revision_id").map_err(|_|internal())?,analysis:row.try_get("analysis_revision_id").map_err(|_|internal())?,estimate:row.try_get("estimate_id").map_err(|_|internal())?});
    }
    let generated_at = Utc::now();
    let generated = generated_at.to_rfc3339_opts(SecondsFormat::Millis,true);
    let expires = (generated_at+chrono::Duration::minutes(15)).to_rfc3339_opts(SecondsFormat::Millis,true);
    let token = Uuid::new_v4().to_string();
    let mode=mode_name(request.mode);
    sqlx::query("INSERT INTO runs_exports(token,owner_id,activity_ids,manifest_ids,byte_length,mode,meta,generated_at,expires_at) VALUES($1,$2,$3,$4,0,$5,'{}'::jsonb,$6,$7::timestamptz)")
        .bind(&token).bind(&owner).bind(&ids).bind(&manifests).bind(mode).bind(&generated).bind(&expires).execute(&mut *transaction).await.map_err(|_|internal())?;
    let snapshot_token=token.clone();
    let snapshot_generated=generated.clone();
    let (mut transaction,byte_length,omissions)=tokio::task::spawn_blocking(move || {
        let mut writer=PrettyWriter::new(SnapshotWriter::new(transaction,snapshot_token));
        let omissions=write_snapshot(&mut writer,&owner,&pins,&request,&snapshot_generated)?;
        writer.inner.write_all(b"\n").map_err(|_|internal())?;
        writer.inner.flush().map_err(|_|internal())?;
        Ok::<_,ApiError>((Rc::try_unwrap(writer.inner.transaction).map_err(|_|internal())?.into_inner(),writer.inner.length,omissions))
    }).await.map_err(|_|internal())??;
    let meta=json!({"privacyOmissions":omissions});
    sqlx::query("UPDATE runs_exports SET byte_length=$2,meta=$3::jsonb WHERE token=$1")
        .bind(&token).bind(i64::try_from(byte_length).map_err(|_|internal())?).bind(meta.to_string()).execute(&mut *transaction).await.map_err(|_|internal())?;
    transaction.commit().await.map_err(|_|internal())?;
    Ok(json!({"token":token,"generatedAt":generated,"expiresAt":expires,"byteLength":byte_length,"privacyOmissions":meta["privacyOmissions"],"downloadUrl":format!("/api/v2/runs/exports/{token}")}))
}

fn mode_name(mode:ExportMode)->&'static str {match mode {ExportMode::Coach=>"coach",ExportMode::Full=>"full"}}
struct Pin {id:String,decoded:String,normalized:String,analysis:String,estimate:String}

/// Fixture/schema oracle. Runtime snapshots use bounded archive readers and the chunk writer below.
pub fn snapshot(selection: &[Uuid], activities: &[Value], request: &ExportRequest, generated_at: &str) -> Result<Vec<u8>,ApiError> {
    validate_selection(request)?;
    if selection!=request.activity_ids || activities.len()!=selection.len() { return Err(invalid_selection()); }
    let policy = Policy {include_location:request.include_location,include_device_identifiers:request.include_device_identifiers};
    let mut output = Vec::with_capacity(activities.len());
    let mut omissions = Vec::new();
    let mut transformations = Vec::new();
    for (id,activity) in selection.iter().zip(activities) {
        if activity["id"].as_str()!=Some(id.to_string().as_str()) || activity["decoded"]["schemaVersion"]!="2.0.0" || activity["normalized"]["schemaVersion"]!="2.0.0" || !activity["analysis"].is_object() { return Err(invalid_selection()); }
        if !activity["historicalThresholds"].is_object() || activity["historicalThresholds"]["evidenceCutoff"]!=activity["normalized"]["endTime"] {return Err(not_ready());}
        let (mut safe, removed) = privacy::project(activity,policy);
        if !safe["historicalThresholds"].is_null() { safe["historicalThresholds"] = super::thresholds::export_projection(&safe["historicalThresholds"]); }
        if !safe["analysis"]["thresholds"].is_null() { safe["analysis"]["thresholds"] = super::thresholds::export_projection(&safe["analysis"]["thresholds"]); }
        omissions.extend(removed.as_array().ok_or_else(internal)?.iter().cloned());
        if request.mode==ExportMode::Full { output.push(safe); }
        else {
            let normalized = &safe["normalized"];
            let analysis = &safe["analysis"];
            let (samples,transformation) = coach_samples(normalized,analysis);
            let mut coach = json!({"id":id,"startTime":normalized["startTime"],"endTime":normalized["endTime"],"subtype":normalized["subtype"],"summary":normalized["summary"],"laps":normalized["laps"],"segments":analysis["segments"],"samples":samples,"quality":analysis["quality"],"thresholds":analysis["thresholds"],"historicalThresholds":safe["historicalThresholds"],"transformations":analysis["transformations"].as_array().cloned().unwrap_or_default()});
            coach["transformations"].as_array_mut().unwrap().push(transformation.clone());
            transformations.push(json!({"activityId":id,"transformation":transformation}));
            output.push(coach);
        }
    }
    let mut document = json!({"schemaVersion":"2.0.0","generatedAt":generated_at,"mode":request.mode,"privacy":{"includeLocation":request.include_location,"includeDeviceIdentifiers":request.include_device_identifiers},"selection":selection});
    document["activities"]=Value::Array(output);
    document["privacyOmissions"]=Value::Array(omissions);
    document["transformations"]=Value::Array(transformations);
    let mut bytes = serde_json::to_vec_pretty(&document).map_err(|_|internal())?;
    bytes.push(b'\n');
    Ok(bytes)
}

struct ReadLease { transaction: Transaction<'static,Postgres>, token: String, owner: String, length:u64, offset:u64, position:i64, deadline:Instant }

async fn read_lease(pool:&PgPool,owner:Uuid,token:&str)->Result<(ReadLease,String),ApiError> {
    if Uuid::parse_str(token).is_err() { return Err(unavailable()); }
    let owner=owner.to_string();
    let mut transaction=pool.begin().await.map_err(|_|internal())?;
    sqlx::query("SET LOCAL statement_timeout='30s'").execute(&mut *transaction).await.map_err(|_|internal())?;
    sqlx::query("SET LOCAL idle_in_transaction_session_timeout='35s'").execute(&mut *transaction).await.map_err(|_|internal())?;
    sqlx::query("SELECT pg_advisory_xact_lock_shared(hashtextextended($1,0))").bind(&owner).execute(&mut *transaction).await.map_err(|_|internal())?;
    let row=sqlx::query("SELECT activity_ids,manifest_ids,byte_length,mode,extract(epoch FROM expires_at-clock_timestamp())::double precision AS remaining FROM runs_exports WHERE token=$1 AND owner_id=$2 AND NOT revoked AND expires_at>clock_timestamp()")
        .bind(token).bind(&owner).fetch_optional(&mut *transaction).await.map_err(|_|internal())?.ok_or_else(unavailable)?;
    let ids=row.try_get::<Vec<String>,_>("activity_ids").map_err(|_|internal())?;
    let pinned=row.try_get::<Vec<String>,_>("manifest_ids").map_err(|_|internal())?;
    let length=u64::try_from(row.try_get::<i64,_>("byte_length").map_err(|_|internal())?).map_err(|_|unavailable())?;
    let mode=row.try_get::<String,_>("mode").map_err(|_|internal())?;
    let activities=sqlx::query("SELECT id FROM runs_activities WHERE owner_id=$1 AND id=ANY($2) FOR SHARE").bind(&owner).bind(&ids).fetch_all(&mut *transaction).await.map_err(|_|internal())?;
    let count:i64=sqlx::query_scalar("SELECT count(*) FROM unnest($2::text[],$3::text[]) AS pin(activity_id,manifest_id) JOIN runs_manifests m ON m.owner_id=$1 AND m.activity_id=pin.activity_id AND m.id=pin.manifest_id")
        .bind(&owner).bind(&ids).bind(&pinned).fetch_one(&mut *transaction).await.map_err(|_|internal())?;
    if length==0 || ids.is_empty() || ids.iter().collect::<HashSet<_>>().len()!=ids.len() || activities.len()!=ids.len() || pinned.len()!=ids.len() || count!=ids.len() as i64 || !matches!(mode.as_str(),"coach"|"full") {return Err(unavailable());}
    // Transport budget, not a client throttle. TTL remains the upper bound.
    let seconds=(30+length.div_ceil(5*1024*1024)) as f64;
    let remaining=row.try_get::<f64,_>("remaining").map_err(|_|internal())?;
    if !remaining.is_finite() || remaining<=0.0 {return Err(unavailable());}
    let duration=Duration::from_secs_f64(seconds.min(remaining));
    sqlx::query("SELECT set_config('idle_in_transaction_session_timeout',$1,true)").bind(format!("{}ms",duration.as_millis()+5000)).execute(&mut *transaction).await.map_err(|_|internal())?;
    Ok((ReadLease{transaction,token:token.to_owned(),owner,length,offset:0,position:0,deadline:Instant::now()+duration},mode))
}

fn export_response(body:Body,length:u64,mode:&str)->Result<Response,ApiError> {
    let mut response=Response::new(body);
    response.headers_mut().insert(header::CONTENT_TYPE,HeaderValue::from_static("application/json; charset=utf-8"));
    response.headers_mut().insert(header::CACHE_CONTROL,HeaderValue::from_static("private, no-store"));
    response.headers_mut().insert(header::CONTENT_LENGTH,HeaderValue::from_str(&length.to_string()).map_err(|_|internal())?);
    response.headers_mut().insert(header::CONTENT_DISPOSITION,HeaderValue::from_str(&format!("attachment; filename=\"runs-{mode}.json\"")).map_err(|_|internal())?);
    Ok(response)
}

pub async fn serve_head(pool:&PgPool,owner:Uuid,token:&str)->Result<Response,ApiError> {
    let (lease,mode)=read_lease(pool,owner,token).await?;
    let length=lease.length;
    lease.transaction.rollback().await.map_err(|_|internal())?;
    export_response(Body::empty(),length,&mode)
}

pub async fn serve(pool: &PgPool, owner: Uuid, token: &str) -> Result<Response, ApiError> {
    let (lease,mode)=read_lease(pool,owner,token).await?;
    let length=lease.length;
    let body=Body::from_stream(stream::try_unfold(lease,|mut lease|async move {
        let remaining=lease.deadline.checked_duration_since(Instant::now()).ok_or_else(||io::Error::other("export lease expired"))?;
        let row=tokio::time::timeout(remaining,sqlx::query("SELECT e.byte_length,c.payload FROM runs_exports e LEFT JOIN runs_export_chunks c ON c.token=e.token AND c.position=$3 WHERE e.token=$1 AND e.owner_id=$2 AND NOT e.revoked AND e.expires_at>clock_timestamp()")
            .bind(&lease.token).bind(&lease.owner).bind(lease.position).fetch_optional(&mut *lease.transaction)).await
            .map_err(|_|io::Error::other("export lease expired"))?.map_err(|_|io::Error::other("export read failed"))?.ok_or_else(||io::Error::other("export revoked"))?;
        let declared=row.try_get::<i64,_>("byte_length").map_err(|_|io::Error::other("export integrity failed"))?;
        if u64::try_from(declared).ok()!=Some(lease.length) {return Err(io::Error::other("export integrity failed"));}
        let bytes=row.try_get::<Option<Vec<u8>>,_>("payload").map_err(|_|io::Error::other("export integrity failed"))?;
        if lease.offset==lease.length {
            if bytes.is_some() {return Err(io::Error::other("export integrity failed"));}
            lease.transaction.rollback().await.map_err(|_|io::Error::other("export lease closed"))?;
            return Ok(None);
        }
        let bytes=bytes.ok_or_else(||io::Error::other("export incomplete"))?;
        if bytes.is_empty() || bytes.len()>CHUNK_BYTES || bytes.len() as u64>lease.length-lease.offset {return Err(io::Error::other("export integrity failed"));}
        lease.offset+=bytes.len() as u64;
        lease.position+=1;
        Ok(Some((Bytes::from(bytes),lease)))
    }));
    export_response(body,length,&mode)
}

const METRICS: &[&str] = &["speedMps","paceSecondsPerKm","heartRateBpm","powerWatts","cadenceStepsPerMinute","altitudeMeters","distanceMeters"];

fn add_boundaries(boundaries:&mut Vec<f64>,entity:&Value) {
    for key in ["startElapsedSeconds","endElapsedSeconds","elapsedSeconds"] {
        if let Some(value)=entity[key].as_f64().filter(|n|n.is_finite()) {boundaries.push(value);}
    }
}
fn analysis_boundaries(analysis:&Value)->Vec<f64> {
    let mut boundaries=Vec::new();
    for values in [&analysis["segments"],&analysis["quality"]["gaps"],&analysis["quality"]["pauses"],&analysis["quality"]["timerPauseIntervals"]] {
        if let Some(values)=values.as_array() {for value in values {add_boundaries(&mut boundaries,value);}}
    }
    boundaries
}
fn aggregation_transformation(source_count:u64,output_count:u64)->Value {
    json!({"kind":"sampleAggregation","method":"left_hold_time_weighted_split_at_boundary","resolutionSeconds":30,"gapLimitSeconds":5,"sourceCount":source_count,"outputCount":output_count,"precision":"unrounded JSON numbers","boundaries":["lap","segment","repeat","pause","gap","timer_event"],"sourceCountDefinition":"Each interval counts distinct contributing source rows; a row split at a boundary contributes to both intervals. Root sourceCount counts original rows once.","missingHandling":"Missing metrics contribute no duration. No weight bridges pauses or gaps. Interval weights split at boundaries. Recorded end values remain separate; terminal rows gain no invented duration."})
}

#[derive(Default)]
struct Bucket {
    start:Option<f64>,end:Option<f64>,timer:Option<bool>,sources:u64,last_index:Option<u64>,
    first:[Option<f64>;7],last:[Option<f64>;7],sums:[f64;7],coverage:[f64;7],
}
impl Bucket {
    fn add(&mut self,index:u64,sample:&Value,start:Option<f64>,end:Option<f64>) {
        let metrics:[Option<f64>;7]=std::array::from_fn(|i|sample[METRICS[i]].as_f64());
        if self.sources==0 {self.start=start;self.timer=sample["timerRunning"].as_bool();self.first=metrics;}
        if self.last_index!=Some(index) {self.sources+=1;self.last_index=Some(index);}
        self.end=end;self.last=metrics;
        if let (Some(start),Some(end))=(start,end) {
            let duration=(end-start).max(0.0);
            for (i,value) in metrics.iter().enumerate() {if let Some(value)=value {self.sums[i]+=value*duration;self.coverage[i]+=duration;}}
        }
    }
    fn value(self)->Value {
        let mut value=json!({"startElapsedSeconds":self.start,"endElapsedSeconds":self.end,"timerRunning":self.timer,"sourceCount":self.sources,"metricCoverageSeconds":{},"recordedEndValues":{}});
        for (i,metric) in METRICS.iter().enumerate() {
            value[*metric]=json!(if self.coverage[i]>0.0 {Some(self.sums[i]/self.coverage[i])} else if self.sources==1 {self.first[i]} else {None});
            value["metricCoverageSeconds"][*metric]=json!(self.coverage[i]);
            value["recordedEndValues"][*metric]=json!(self.last[i]);
        }
        value
    }
}

struct Coach {boundaries:Vec<f64>,pending:Option<(u64,Value)>,bucket:Bucket,key:Option<(i64,usize,Option<bool>,u64)>,epoch:u64,sources:u64,outputs:u64}
impl Coach {
    fn new(mut boundaries:Vec<f64>)->Self {boundaries.sort_by(f64::total_cmp);boundaries.dedup();Self{boundaries,pending:None,bucket:Bucket::default(),key:None,epoch:0,sources:0,outputs:0}}
    fn flush(&mut self,emit:&mut dyn FnMut(Value)->Result<(),ApiError>)->Result<(),ApiError> {
        if self.bucket.sources>0 {emit(std::mem::take(&mut self.bucket).value())?;self.outputs+=1;}
        Ok(())
    }
    fn push(&mut self,sample:Value,emit:&mut dyn FnMut(Value)->Result<(),ApiError>)->Result<(),ApiError> {
        let index=self.sources;self.sources+=1;
        if let Some((old_index,previous))=self.pending.take() {self.interval(old_index,&previous,Some(&sample),emit)?;}
        self.pending=Some((index,sample));Ok(())
    }
    fn interval(&mut self,index:u64,sample:&Value,next:Option<&Value>,emit:&mut dyn FnMut(Value)->Result<(),ApiError>)->Result<(),ApiError> {
        let Some(mut cursor)=sample["elapsedSeconds"].as_f64() else {
            self.flush(emit)?;self.bucket.add(index,sample,None,None);self.flush(emit)?;self.key=None;self.epoch+=1;return Ok(());
        };
        let next_time=next.and_then(|sample|sample["elapsedSeconds"].as_f64());
        let continuous=next_time.is_some_and(|end|end>cursor && end-cursor<=5.0);
        let end=next_time.filter(|_|continuous && sample["timerRunning"].as_bool()!=Some(false)).unwrap_or(cursor);
        loop {
            let boundary=self.boundaries.partition_point(|value|*value<=cursor);
            let bucket=(cursor/30.0).floor() as i64;
            let key=Some((bucket,boundary,sample["timerRunning"].as_bool(),self.epoch));
            if self.key!=key {self.flush(emit)?;}
            let stop=end.min((bucket as f64+1.0)*30.0).min(self.boundaries.get(boundary).copied().unwrap_or(end));
            self.bucket.add(index,sample,Some(cursor),Some(stop.max(cursor)));self.key=key;
            if stop>=end || stop<=cursor {break;} cursor=stop;
        }
        if next.is_some() && (!continuous || next.is_some_and(|next|sample["timerRunning"]!=next["timerRunning"])) {self.flush(emit)?;self.key=None;self.epoch+=1;}
        Ok(())
    }
    fn finish(&mut self,emit:&mut dyn FnMut(Value)->Result<(),ApiError>)->Result<Value,ApiError> {
        if let Some((index,sample))=self.pending.take() {self.interval(index,&sample,None,emit)?;}
        self.flush(emit)?;Ok(aggregation_transformation(self.sources,self.outputs))
    }
}

fn coach_samples(normalized:&Value,analysis:&Value)->(Value,Value) {
    let mut boundaries=analysis_boundaries(analysis);
    for values in [&normalized["laps"],&normalized["timerEvents"]] {if let Some(values)=values.as_array() {for value in values {add_boundaries(&mut boundaries,value);}}}
    let mut coach=Coach::new(boundaries);let mut output=Vec::new();
    let mut emit=|value|{output.push(value);Ok(())};
    if let Some(samples)=normalized["samples"].as_array() {for sample in samples {coach.push(sample.clone(),&mut emit).expect("in-memory JSON emission");}}
    let transformation=coach.finish(&mut emit).expect("in-memory JSON emission");
    (Value::Array(output),transformation)
}

struct SnapshotWriter {
    transaction:Rc<RefCell<Transaction<'static,Postgres>>>,token:String,buffer:Vec<u8>,position:i64,length:u64,
}
impl SnapshotWriter {
    fn new(transaction:Transaction<'static,Postgres>,token:String)->Self {Self{transaction:Rc::new(RefCell::new(transaction)),token,buffer:Vec::with_capacity(CHUNK_BYTES),position:0,length:0}}
}
impl Write for SnapshotWriter {
    fn write(&mut self,mut bytes:&[u8])->io::Result<usize> {
        let total=bytes.len();
        while !bytes.is_empty() {
            let n=bytes.len().min(CHUNK_BYTES-self.buffer.len());
            self.buffer.extend_from_slice(&bytes[..n]);self.length=self.length.checked_add(n as u64).ok_or_else(||io::Error::other("export length overflow"))?;
            bytes=&bytes[n..];if self.buffer.len()==CHUNK_BYTES {self.flush()?;}
        }
        Ok(total)
    }
    fn flush(&mut self)->io::Result<()> {
        if self.buffer.is_empty() {return Ok(());}
        let mut transaction=self.transaction.try_borrow_mut().map_err(|_|io::Error::other("export transaction busy"))?;
        tokio::runtime::Handle::current().block_on(sqlx::query("INSERT INTO runs_export_chunks(token,position,payload) VALUES($1,$2,$3)")
            .bind(&self.token).bind(self.position).bind(self.buffer.as_slice()).execute(&mut **transaction)).map_err(|_|io::Error::other("export storage failed"))?;
        self.position+=1;self.buffer.clear();Ok(())
    }
}

// Format streamed tokens, not a materialized document. String bytes are copied in spans.
struct PrettyWriter<W> {inner:W,empty:Vec<bool>,quoted:bool,escaped:bool}
impl<W:Write> PrettyWriter<W> {
    fn new(inner:W)->Self {Self{inner,empty:Vec::new(),quoted:false,escaped:false}}
    fn newline(&mut self)->io::Result<()> {
        self.inner.write_all(b"\n")?;
        const SPACES:[u8;128]=[b' ';128];
        let mut spaces=self.empty.len()*2;
        while spaces>0 {let n=spaces.min(SPACES.len());self.inner.write_all(&SPACES[..n])?;spaces-=n;}
        Ok(())
    }
    fn begin(&mut self)->io::Result<()> {
        if self.empty.last()==Some(&true) {self.newline()?;*self.empty.last_mut().unwrap()=false;}
        Ok(())
    }
}
impl<W:Write> Write for PrettyWriter<W> {
    fn write(&mut self,bytes:&[u8])->io::Result<usize> {
        let mut i=0;
        while i<bytes.len() {
            if self.quoted {
                let start=i;
                while i<bytes.len() {
                    let byte=bytes[i];i+=1;
                    if self.escaped {self.escaped=false;}
                    else if byte==b'\\' {self.escaped=true;}
                    else if byte==b'"' {self.quoted=false;break;}
                }
                self.inner.write_all(&bytes[start..i])?;continue;
            }
            let byte=bytes[i];i+=1;
            match byte {
                b' '|b'\n'|b'\r'|b'\t'=>{},
                b'{'|b'['=>{self.begin()?;self.inner.write_all(&[byte])?;self.empty.push(true);},
                b'}'|b']'=>{let empty=self.empty.pop().ok_or_else(||io::Error::other("invalid export JSON"))?;if !empty {self.newline()?;}self.inner.write_all(&[byte])?;},
                b','=>{self.inner.write_all(b",")?;self.newline()?;},
                b':'=>self.inner.write_all(b": ")?,
                b'"'=>{self.begin()?;self.inner.write_all(b"\"")?;self.quoted=true;},
                _=>{self.begin()?;let start=i-1;while i<bytes.len() && !matches!(bytes[i],b' '|b'\n'|b'\r'|b'\t'|b'{'|b'}'|b'['|b']'|b','|b':'|b'"') {i+=1;}self.inner.write_all(&bytes[start..i])?;},
            }
        }
        Ok(bytes.len())
    }
    fn flush(&mut self)->io::Result<()> {self.inner.flush()}
}

fn encoded<W:Write+?Sized>(writer:&mut W,value:&Value)->Result<(),ApiError> {serde_json::to_writer(writer,value).map_err(|_|internal())}
fn literal<W:Write+?Sized>(writer:&mut W,bytes:&[u8])->Result<(),ApiError> {writer.write_all(bytes).map_err(|_|internal())}
fn merge_omissions(counts:&mut BTreeMap<(String,String),u64>,rows:Value)->Result<(),ApiError> {
    let Value::Array(rows)=rows else {return Err(internal());};
    for row in rows {
        let key=(row["category"].as_str().ok_or_else(internal)?.to_owned(),row["pathPattern"].as_str().ok_or_else(internal)?.to_owned());
        *counts.entry(key).or_default()+=row["count"].as_u64().ok_or_else(internal)?;
    }
    Ok(())
}
fn archive_reader(transaction:Rc<RefCell<Transaction<'static,Postgres>>>,owner:&str,revision:&str,metadata:&Value)->Result<RevisionReader,ApiError> {
    RevisionReader::new_transaction(transaction,owner.to_owned(),revision.to_owned(),"archive",&metadata["documents"]["archive"])
}
fn write_archive<W:Write>(writer:&mut W,reader:RevisionReader,document:Document,policy:Policy)->Result<Value,ApiError> {
    let projection=RefCell::new(Projection::new(document,policy));
    project_document(reader,writer,&mut |path|projection.borrow_mut().keep(path),&mut |path,value|Ok(projection.borrow_mut().value(path,value)))?;
    Ok(projection.into_inner().finish())
}

fn write_snapshot(writer:&mut PrettyWriter<SnapshotWriter>,owner:&str,pins:&[Pin],request:&ExportRequest,generated:&str)->Result<Value,ApiError> {
    let transaction=writer.inner.transaction.clone();
    let policy=Policy{include_location:request.include_location,include_device_identifiers:request.include_device_identifiers};
    let mut omissions=BTreeMap::new();let mut transformations=Vec::new();
    literal(writer,b"{\"activities\":[")?;
    for (index,pin) in pins.iter().enumerate() {
        let row={
            let mut transaction=transaction.try_borrow_mut().map_err(|_|internal())?;
            tokio::runtime::Handle::current().block_on(sqlx::query("SELECT d.payload::text AS decoded,n.payload::text AS normalized,r.payload::text AS analysis,e.payload::text AS history FROM runs_revisions d,runs_revisions n,runs_revisions r,runs_estimates e WHERE d.id=$1 AND n.id=$2 AND r.id=$3 AND e.id=$4 AND d.owner_id=$5 AND n.owner_id=$5 AND r.owner_id=$5 AND e.owner_id=$5")
                .bind(&pin.decoded).bind(&pin.normalized).bind(&pin.analysis).bind(&pin.estimate).bind(owner).fetch_one(&mut **transaction)).map_err(|_|internal())?
        };
        let parse=|key|->Result<Value,ApiError>{serde_json::from_str(&row.try_get::<String,_>(key).map_err(|_|internal())?).map_err(|_|internal())};
        let decoded=parse("decoded")?;let normalized=parse("normalized")?;let analysis=parse("analysis")?;let history=parse("history")?;
        if decoded["schemaVersion"]!="2.0.0" || normalized["schemaVersion"]!="2.0.0" || !analysis.is_object() {return Err(invalid_selection());}
        if !history.is_object() || history["evidenceCutoff"]!=normalized["endTime"] {return Err(not_ready());}
        let (mut safe,removed)=privacy::project(&json!({"id":pin.id,"analysis":analysis,"historicalThresholds":history}),policy);
        merge_omissions(&mut omissions,removed)?;
        safe["historicalThresholds"]=super::thresholds::export_projection(&safe["historicalThresholds"]);
        if !safe["analysis"]["thresholds"].is_null() {safe["analysis"]["thresholds"]=super::thresholds::export_projection(&safe["analysis"]["thresholds"]);}
        if index>0 {literal(writer,b",")?;}
        if request.mode==ExportMode::Full {
            literal(writer,b"{\"analysis\":")?;encoded(writer,&safe["analysis"])?;
            literal(writer,b",\"decoded\":")?;
            let removed=write_archive(writer,archive_reader(transaction.clone(),owner,&pin.decoded,&decoded)?,Document::Decoded,policy)?;
            merge_omissions(&mut omissions,removed)?;
            literal(writer,b",\"historicalThresholds\":")?;encoded(writer,&safe["historicalThresholds"])?;
            literal(writer,b",\"id\":")?;encoded(writer,&json!(pin.id))?;
            literal(writer,b",\"normalized\":")?;
            let removed=write_archive(writer,archive_reader(transaction.clone(),owner,&pin.normalized,&normalized)?,Document::Normalized,policy)?;
            merge_omissions(&mut omissions,removed)?;literal(writer,b"}")?;
        } else {
            let transformation=write_coach(writer,&transaction,owner,pin,&normalized,&safe,policy,&mut omissions)?;
            transformations.push(json!({"activityId":pin.id,"transformation":transformation}));
        }
    }
    let omissions=Value::Array(omissions.into_iter().map(|((category,path),count)|json!({"category":category,"pathPattern":path,"count":count,"reason":"Field omitted by the export schema and consent policy."})).collect());
    literal(writer,b"],\"generatedAt\":")?;encoded(writer,&json!(generated))?;
    literal(writer,b",\"mode\":")?;encoded(writer,&json!(request.mode))?;
    literal(writer,b",\"privacy\":")?;encoded(writer,&json!({"includeLocation":policy.include_location,"includeDeviceIdentifiers":policy.include_device_identifiers}))?;
    literal(writer,b",\"privacyOmissions\":")?;encoded(writer,&omissions)?;
    literal(writer,b",\"schemaVersion\":\"2.0.0\",\"selection\":")?;encoded(writer,&json!(request.activity_ids))?;
    literal(writer,b",\"transformations\":")?;encoded(writer,&Value::Array(transformations))?;literal(writer,b"}")?;
    Ok(omissions)
}

fn write_coach<W:Write>(writer:&mut W,transaction:&Rc<RefCell<Transaction<'static,Postgres>>>,owner:&str,pin:&Pin,normalized:&Value,safe:&Value,policy:Policy,omissions:&mut BTreeMap<(String,String),u64>)->Result<Value,ApiError> {
    let (root,removed)=privacy::project_normalized(&json!({"endTime":normalized["endTime"],"startTime":normalized["startTime"],"subtype":normalized["subtype"],"summary":normalized["summary"]}),policy);
    merge_omissions(omissions,removed)?;
    literal(writer,b"{\"endTime\":")?;encoded(writer,&root["endTime"])?;
    literal(writer,b",\"historicalThresholds\":")?;encoded(writer,&safe["historicalThresholds"])?;
    literal(writer,b",\"id\":")?;encoded(writer,&json!(pin.id))?;
    literal(writer,b",\"laps\":[")?;
    let mut boundaries=analysis_boundaries(&safe["analysis"]);let mut first=true;
    let mut projection=Projection::new(Document::Normalized,policy);
    project_document(archive_reader(transaction.clone(),owner,&pin.normalized,normalized)?,&mut io::sink(),&mut |path|path.first().is_some_and(|key|matches!(key.as_str(),"laps"|"timerEvents")),&mut |path,value|{
        if path.last().is_some_and(|key|key=="*") {
            if let Some(value)=projection.value(path,value) {
                add_boundaries(&mut boundaries,&value);
                if path[0]=="laps" {if !first {literal(writer,b",")?;}first=false;encoded(writer,&value)?;}
            }
            Ok(None)
        } else {Ok(Some(value))}
    })?;
    merge_omissions(omissions,projection.finish())?;
    literal(writer,b"],\"quality\":")?;encoded(writer,&safe["analysis"]["quality"])?;
    literal(writer,b",\"samples\":[")?;
    let mut coach=Coach::new(boundaries);let mut first=true;
    let mut emit=|value|{if !first {literal(writer,b",")?;}first=false;encoded(writer,&value)};
    let mut projection=Projection::new(Document::Normalized,policy);
    project_document(archive_reader(transaction.clone(),owner,&pin.normalized,normalized)?,&mut io::sink(),&mut |path|path.first().is_some_and(|key|key=="samples"),&mut |path,value|{
        if path.last().is_some_and(|key|key=="*") {if let Some(value)=projection.value(path,value) {coach.push(value,&mut emit)?;}Ok(None)}
        else {Ok(Some(value))}
    })?;
    let transformation=coach.finish(&mut emit)?;
    merge_omissions(omissions,projection.finish())?;
    literal(writer,b"],\"segments\":")?;encoded(writer,&safe["analysis"]["segments"])?;
    literal(writer,b",\"startTime\":")?;encoded(writer,&root["startTime"])?;
    literal(writer,b",\"subtype\":")?;encoded(writer,&root["subtype"])?;
    literal(writer,b",\"summary\":")?;encoded(writer,&root["summary"])?;
    literal(writer,b",\"thresholds\":")?;encoded(writer,&safe["analysis"]["thresholds"])?;
    literal(writer,b",\"transformations\":")?;
    let mut values=safe["analysis"]["transformations"].as_array().cloned().unwrap_or_default();values.push(transformation.clone());
    encoded(writer,&Value::Array(values))?;literal(writer,b"}")?;Ok(transformation)
}
