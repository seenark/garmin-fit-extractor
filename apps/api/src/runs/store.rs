use super::{
    process::DecodedSpool,
    stream::{Document, StagedRun},
};
use crate::{error::ApiError, model::Analysis};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Row, Transaction};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use uuid::Uuid;

pub struct Imported {
    pub activity_id: Uuid,
    pub duplicate: bool,
}
pub fn versions() -> Value {
    static DESIRED: std::sync::LazyLock<Value> = std::sync::LazyLock::new(|| {
        let config = format!(
            "{:x}",
            Sha256::digest(
                crate::runs::thresholds::configuration()
                    .to_string()
                    .as_bytes()
            )
        );
        let decoder = crate::fit::runs::decoder_metadata();
        let mut desired = json!({"schema":crate::fit::runs::NORMALIZED_SCHEMA_VERSION,"analysis":crate::runs::analysis::VERSION,"method":crate::runs::thresholds::METHOD_VERSION,"config":config,"history":crate::runs::history::VERSION,"historySourceHash":crate::runs::history::source_hash(),"observationRule":super::stream::OBSERVATION_RULE,"projectionVersion":crate::runs::thresholds::PROJECTION_VERSION});
        desired["decoder"] = decoder;
        desired
    });
    DESIRED.clone()
}
pub(crate) fn database_error(_: sqlx::Error) -> ApiError {
    ApiError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "RUNS_STORAGE_FAILED",
        "Run storage operation failed.",
    )
}
pub(crate) fn invalid() -> ApiError {
    ApiError::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        "INVALID_RUN_DOCUMENT",
        "Run document failed semantic validation.",
    )
}
pub(crate) fn parse(text: &str) -> Result<Value, ApiError> {
    serde_json::from_str(text).map_err(|_| invalid())
}
pub(crate) fn timestamp(value: &Value) -> Result<DateTime<Utc>, ApiError> {
    let text = value.as_str().ok_or_else(invalid)?;
    if !text.ends_with('Z') && !text.ends_with("+00:00") {
        return Err(invalid());
    }
    DateTime::parse_from_rfc3339(text)
        .map(|v| v.with_timezone(&Utc))
        .map_err(|_| invalid())
}
pub(crate) async fn owner_lock(
    tx: &mut Transaction<'_, Postgres>,
    owner: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(owner)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
pub async fn find_duplicate(
    pool: &PgPool,
    owner: Uuid,
    bytes: &[u8],
) -> Result<Option<Imported>, ApiError> {
    if bytes.is_empty() || bytes.len() > super::process::MAX_INPUT {
        return Err(ApiError::file_too_large());
    }
    let hash = Sha256::digest(bytes).to_vec();
    let row=sqlx::query("SELECT a.id,s.bytes,s.size_bytes FROM runs_sources s JOIN runs_activities a ON a.source_id=s.id AND a.owner_id=s.owner_id WHERE s.owner_id=$1 AND s.sha256=$2 AND a.session_index=0")
        .bind(owner.to_string()).bind(hash).fetch_optional(pool).await.map_err(database_error)?;
    let Some(row) = row else { return Ok(None) };
    let stored: &[u8] = row.try_get("bytes").map_err(database_error)?;
    if row
        .try_get::<i64, _>("size_bytes")
        .map_err(database_error)?
        != bytes.len() as i64
        || stored != bytes
    {
        return Err(integrity());
    }
    let id: &str = row.try_get("id").map_err(database_error)?;
    Ok(Some(Imported {
        activity_id: Uuid::parse_str(id).map_err(|_| invalid())?,
        duplicate: true,
    }))
}
fn integrity() -> ApiError {
    ApiError::new(
        StatusCode::CONFLICT,
        "SOURCE_INTEGRITY_FAILED",
        "Stored source integrity verification failed.",
    )
}

/// Only a successful bounded child control and full streaming semantic validation
/// can reach source acceptance. Staging never constructs the decoded archive Value.
pub async fn accept_spool(
    pool: &PgPool,
    owner: Uuid,
    bytes: &[u8],
    spool: DecodedSpool,
    admission_holder: &str,
) -> Result<Imported, ApiError> {
    if bytes.is_empty() || bytes.len() > super::process::MAX_INPUT {
        return Err(ApiError::file_too_large());
    }
    if spool.source_sha256 != format!("{:x}", Sha256::digest(bytes)) {
        return Err(integrity());
    }
    let (spool, staged) = tokio::task::spawn_blocking(move || {
        let staged = super::stream::split_run(&spool.run, &spool.directory)?;
        Ok::<_, ApiError>((spool, staged))
    })
    .await
    .map_err(|_| invalid())??;
    let result = accept_staged(
        pool,
        owner,
        bytes,
        &staged,
        &spool.source_sha256,
        admission_holder,
    )
    .await;
    drop(spool);
    result
}
/// Select current identity and reconcile mutable reciprocal evidence under the owner lock.
pub(crate) async fn observation_identity(
    tx: &mut Transaction<'_, Postgres>,
    owner: &str,
    activity: &str,
    normalized: &Value,
) -> Result<(Vec<u8>, String, Value), ApiError> {
    if normalized["observationRule"] != super::stream::OBSERVATION_RULE {
        return Err(invalid());
    }
    let fingerprint = decode_hash(
        normalized["observationFingerprint"]
            .as_str()
            .ok_or_else(invalid)?,
    )?;
    let strong = normalized["observationStrong"]
        .as_bool()
        .ok_or_else(invalid)?;
    let peer = if strong {
        sqlx::query("SELECT id,observation_group_id FROM runs_activities WHERE owner_id=$1 AND observation_fingerprint=$2 AND id<>$3 ORDER BY id LIMIT 1")
            .bind(owner).bind(&fingerprint).bind(activity).fetch_optional(&mut **tx).await.map_err(database_error)?
    } else {
        None
    };
    let mut group = activity.to_owned();
    let mut evidence = json!([]);
    if let Some(peer) = peer {
        group = peer.try_get("observation_group_id").map_err(database_error)?;
        evidence = json!([{"rule":super::stream::OBSERVATION_RULE,"strength":"strong","activityId":peer.try_get::<&str,_>("id").map_err(database_error)?}]);
    } else if let Some(peer) = sqlx::query_scalar::<_, String>("SELECT id FROM runs_activities WHERE owner_id=$1 AND start_order=$2::timestamptz AND end_order=$3::timestamptz AND id<>$4 ORDER BY id LIMIT 1")
        .bind(owner).bind(normalized["startTime"].as_str()).bind(normalized["endTime"].as_str()).bind(activity).fetch_optional(&mut **tx).await.map_err(database_error)?
    {
        evidence = json!([{"rule":"exact-boundaries-v1","strength":"weak","activityId":peer}]);
    }
    let peer = evidence[0]["activityId"].as_str();
    let reciprocal = json!([{"rule":evidence[0]["rule"],"strength":evidence[0]["strength"],"activityId":activity}]).to_string();
    // Preserve every compatible reciprocal claim, not only the selected peer.
    sqlx::query("UPDATE runs_activities a SET duplicate_evidence=COALESCE((SELECT jsonb_agg(item) FROM jsonb_array_elements(a.duplicate_evidence) item WHERE item->>'activityId'<>$2 OR (item->>'rule'=$4 AND item->>'strength'='strong' AND $5 AND a.observation_fingerprint=$6) OR (item->>'rule'='exact-boundaries-v1' AND item->>'strength'='weak' AND a.start_order=$7::timestamptz AND a.end_order=$8::timestamptz AND NOT ($5 AND a.observation_fingerprint=$6))),'[]'::jsonb) WHERE a.owner_id=$1 AND a.id<>$2 AND a.duplicate_evidence @> $3::jsonb")
        .bind(owner).bind(activity).bind(json!([{"activityId":activity}]).to_string()).bind(super::stream::OBSERVATION_RULE).bind(strong).bind(&fingerprint).bind(normalized["startTime"].as_str()).bind(normalized["endTime"].as_str()).execute(&mut **tx).await.map_err(database_error)?;
    if let Some(peer) = peer {
        sqlx::query("UPDATE runs_activities SET duplicate_evidence=duplicate_evidence||$3::jsonb WHERE owner_id=$1 AND id=$2 AND NOT duplicate_evidence @> $3::jsonb")
            .bind(owner).bind(peer).bind(reciprocal).execute(&mut **tx).await.map_err(database_error)?;
    }
    // Publication replaces this activity's cache, so carry forward compatible
    // incoming and outgoing claims already recorded in its evidence array.
    let retained: Vec<String> = sqlx::query_scalar("SELECT item::text FROM runs_activities a CROSS JOIN LATERAL jsonb_array_elements(a.duplicate_evidence) item JOIN runs_activities peer ON peer.owner_id=a.owner_id AND peer.id=item->>'activityId' WHERE a.owner_id=$1 AND a.id=$2 AND peer.id<>$2 AND ((item->>'rule'=$3 AND item->>'strength'='strong' AND $4 AND peer.observation_fingerprint=$5) OR (item->>'rule'='exact-boundaries-v1' AND item->>'strength'='weak' AND peer.start_order=$6::timestamptz AND peer.end_order=$7::timestamptz AND NOT ($4 AND peer.observation_fingerprint=$5)))")
        .bind(owner).bind(activity).bind(super::stream::OBSERVATION_RULE).bind(strong).bind(&fingerprint).bind(normalized["startTime"].as_str()).bind(normalized["endTime"].as_str()).fetch_all(&mut **tx).await.map_err(database_error)?;
    let claims = evidence.as_array_mut().ok_or_else(invalid)?;
    for claim in retained {
        let claim = parse(&claim)?;
        if !claims.contains(&claim) {
            claims.push(claim);
        }
    }
    Ok((fingerprint, group, evidence))
}

async fn accept_staged(
    pool: &PgPool,
    owner: Uuid,
    bytes: &[u8],
    staged: &StagedRun,
    source_hash: &str,
    admission_holder: &str,
) -> Result<Imported, ApiError> {
    let normalized = &staged.normalized_metadata;
    let start = timestamp(&normalized["startTime"])?;
    let end = timestamp(&normalized["endTime"])?;
    if end < start || !normalized["summary"].is_object() {
        return Err(invalid());
    }
    let owner = owner.to_string();
    let hash = decode_hash(source_hash)?;
    let mut tx = pool.begin().await.map_err(database_error)?;
    owner_lock(&mut tx, &owner).await.map_err(database_error)?;
    admission_fence(&mut tx, admission_holder).await?;
    sqlx::query("INSERT INTO runs_sources(id,owner_id,sha256,size_bytes,bytes,integrity) VALUES($1,$2,$3,$4,$5,'verified') ON CONFLICT(owner_id,sha256) DO NOTHING")
        .bind(Uuid::now_v7().to_string()).bind(&owner).bind(&hash).bind(bytes.len() as i64).bind(bytes).execute(&mut *tx).await.map_err(database_error)?;
    let source =
        sqlx::query("SELECT id,size_bytes,bytes FROM runs_sources WHERE owner_id=$1 AND sha256=$2")
            .bind(&owner)
            .bind(hash)
            .fetch_one(&mut *tx)
            .await
            .map_err(database_error)?;
    if source
        .try_get::<i64, _>("size_bytes")
        .map_err(database_error)?
        != bytes.len() as i64
        || source
            .try_get::<&[u8], _>("bytes")
            .map_err(database_error)?
            != bytes
    {
        return Err(integrity());
    }
    let source: &str = source.try_get("id").map_err(database_error)?;
    if let Some(id) = sqlx::query_scalar::<_, String>(
        "SELECT id FROM runs_activities WHERE owner_id=$1 AND source_id=$2 AND session_index=0",
    )
    .bind(&owner)
    .bind(source)
    .fetch_optional(&mut *tx)
    .await
    .map_err(database_error)?
    {
        admission_fence(&mut tx, admission_holder).await?;
        tx.commit().await.map_err(database_error)?;
        return Ok(Imported {
            activity_id: Uuid::parse_str(&id).map_err(|_| invalid())?,
            duplicate: true,
        });
    }
    let id = Uuid::now_v7();
    let id_text = id.to_string();
    let (fingerprint, group, evidence) =
        observation_identity(&mut tx, &owner, &id_text, normalized).await?;
    sqlx::query("INSERT INTO runs_activities(id,owner_id,source_id,session_index,start_time,end_time,start_order,end_order,subtype,summary,observation_group_id,observation_fingerprint,duplicate_evidence,desired_versions) VALUES($1,$2,$3,0,$4,$5,$4::timestamptz,$5::timestamptz,$6,$7::jsonb,$8,$9,$10::jsonb,$11::jsonb)")
        .bind(&id_text).bind(&owner).bind(source).bind(normalized["startTime"].as_str()).bind(normalized["endTime"].as_str()).bind(normalized["subtype"].as_str()).bind(normalized["summary"].to_string()).bind(group).bind(fingerprint).bind(evidence.to_string()).bind(versions().to_string()).execute(&mut *tx).await.map_err(database_error)?;
    let (decoded_id, normalized_id) =
        stage_documents(&mut tx, &owner, &id_text, 1, staged, source_hash).await?;
    enqueue(
        &mut tx,
        &owner,
        &id_text,
        "analysis",
        1,
        &json!([decoded_id, normalized_id]),
    )
    .await
    .map_err(database_error)?;
    invalidate_cutoffs(
        &mut tx,
        &owner,
        normalized["endTime"].as_str().ok_or_else(invalid)?,
    )
    .await
    .map_err(database_error)?;
    admission_fence(&mut tx, admission_holder).await?;
    tx.commit().await.map_err(database_error)?;
    Ok(Imported {
        activity_id: id,
        duplicate: false,
    })
}
async fn admission_fence(tx: &mut Transaction<'_, Postgres>, holder: &str) -> Result<(), ApiError> {
    let live=sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM runs_slots WHERE stage='import' AND lease_owner=$1 AND lease_until>clock_timestamp())")
        .bind(holder).fetch_one(&mut **tx).await.map_err(database_error)?;
    if !live {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "IMPORT_LEASE_LOST",
            "Import admission lease expired.",
        ));
    }
    Ok(())
}
pub(crate) fn decode_hash(text: &str) -> Result<Vec<u8>, ApiError> {
    if text.len() != 64 || !text.is_ascii() {
        return Err(invalid());
    }
    (0..64)
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16).map_err(|_| invalid()))
        .collect()
}

pub(crate) async fn stage_documents(
    tx: &mut Transaction<'_, Postgres>,
    owner: &str,
    activity: &str,
    generation: i64,
    staged: &StagedRun,
    source_hash: &str,
) -> Result<(String, String), ApiError> {
    let target = (owner, activity, generation);
    let decoded_payload = json!({"documents":{"archive":staged.decoded.metadata},"counts":staged.decoded.metadata["counts"],"schemaVersion":crate::fit::runs::ARCHIVE_SCHEMA_VERSION,"decoder":crate::fit::runs::decoder_metadata(),"sourceHash":source_hash});
    let decoded = insert_revision(
        tx,
        target,
        "decoded",
        &json!([]),
        &decoded_payload,
        None,
        &validated(),
    )
    .await?;
    let mut normalized = staged.normalized_metadata.clone();
    normalized["sourceHash"] = json!(source_hash);
    normalized["documents"] = json!({"archive":staged.normalized.metadata,"analysisInput":staged.analysis_input.metadata});
    normalized["projectionVersion"] = json!(crate::runs::thresholds::PROJECTION_VERSION);
    let normalized_id = insert_revision(
        tx,
        target,
        "normalized",
        &json!([decoded]),
        &normalized,
        Some(&staged.legacy),
        &validated(),
    )
    .await?;
    copy_documents(
        tx,
        owner,
        &[
            (&decoded, "archive", &staged.decoded),
            (&normalized_id, "archive", &staged.normalized),
            (&normalized_id, "analysisInput", &staged.analysis_input),
        ],
    )
    .await?;
    Ok((decoded, normalized_id))
}
fn validated() -> Value {
    json!({"status":"validated","schema":"2.0.0","semanticBounds":true,"lineage":true})
}
pub(crate) async fn insert_revision(
    tx: &mut Transaction<'_, Postgres>,
    target: (&str, &str, i64),
    stage: &str,
    inputs: &Value,
    payload: &Value,
    legacy: Option<&Analysis>,
    validation: &Value,
) -> Result<String, ApiError> {
    let (owner, activity, generation) = target;
    let id = Uuid::now_v7().to_string();
    let legacy = legacy
        .map(serde_json::to_string)
        .transpose()
        .map_err(|_| invalid())?;
    sqlx::query("INSERT INTO runs_revisions(id,owner_id,activity_id,stage,generation,versions,input_revision_ids,payload,validation,legacy_projection) VALUES($1,$2,$3,$4,$5,$6::jsonb,$7::jsonb,$8::jsonb,$9::jsonb,$10::jsonb)")
        .bind(&id).bind(owner).bind(activity).bind(stage).bind(generation).bind(versions().to_string()).bind(inputs.to_string()).bind(payload.to_string()).bind(validation.to_string()).bind(legacy).execute(&mut **tx).await.map_err(database_error)?;
    Ok(id)
}

async fn copy_documents(
    tx: &mut Transaction<'_, Postgres>,
    owner: &str,
    documents: &[(&str, &str, &Document)],
) -> Result<(), ApiError> {
    let mut copy=tx.copy_in_raw("COPY runs_revision_chunks(revision_id,owner_id,document,position,payload) FROM STDIN WITH (FORMAT binary)").await.map_err(database_error)?;
    copy.send(&b"PGCOPY\n\xff\r\n\0\0\0\0\0\0\0\0\0"[..])
        .await
        .map_err(database_error)?;
    let work = async {
        let mut buffer = [0u8; 65536];
        let mut header = Vec::with_capacity(160);
        for (revision, kind, document) in documents {
            let mut options = tokio::fs::OpenOptions::new();
            options.read(true);
            #[cfg(unix)]
            {
                options.custom_flags(libc::O_NOFOLLOW);
            }
            let mut file = options.open(&document.path).await.map_err(|_| invalid())?;
            let metadata = file.metadata().await.map_err(|_| invalid())?;
            super::process::verify_metadata(&metadata, false)?;
            if document.byte_length == 0
                || document.byte_length > super::process::MAX_SPOOL
                || document
                    .offset
                    .checked_add(document.byte_length)
                    .is_none_or(|end| end > metadata.len())
                || document.metadata["byteLength"].as_u64() != Some(document.byte_length)
                || document.metadata["sha256"].as_str() != Some(document.sha256.as_str())
                || document.metadata["chunkCount"].as_u64()
                    != Some(document.byte_length.div_ceil(65536))
            {
                return Err(invalid());
            }
            file.seek(std::io::SeekFrom::Start(document.offset))
                .await
                .map_err(|_| invalid())?;
            let mut remaining = document.byte_length;
            let mut position = 0i64;
            let mut hash = Sha256::new();
            while remaining > 0 {
                let count = remaining.min(65536) as usize;
                file.read_exact(&mut buffer[..count])
                    .await
                    .map_err(|_| invalid())?;
                hash.update(&buffer[..count]);
                header.clear();
                header.extend_from_slice(&5i16.to_be_bytes());
                copy_field(&mut header, revision.as_bytes());
                copy_field(&mut header, owner.as_bytes());
                copy_field(&mut header, kind.as_bytes());
                copy_field(&mut header, &position.to_be_bytes());
                header.extend_from_slice(&(count as i32).to_be_bytes());
                copy.send(header.as_slice()).await.map_err(database_error)?;
                copy.send(&buffer[..count]).await.map_err(database_error)?;
                remaining -= count as u64;
                position += 1;
            }
            if format!("{:x}", hash.finalize()) != document.sha256
                || document.metadata["chunkCount"].as_u64() != Some(position as u64)
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
    .await;
    if let Err(error) = work {
        let _ = copy.abort("Runs stage validation failed").await;
        return Err(error);
    }
    copy.send(&(-1i16).to_be_bytes()[..])
        .await
        .map_err(database_error)?;
    copy.finish().await.map_err(database_error)?;
    Ok(())
}
fn copy_field(header: &mut Vec<u8>, bytes: &[u8]) {
    header.extend_from_slice(&(bytes.len() as i32).to_be_bytes());
    header.extend_from_slice(bytes);
}

pub(crate) async fn enqueue(
    tx: &mut Transaction<'_, Postgres>,
    owner: &str,
    activity: &str,
    stage: &str,
    generation: i64,
    inputs: &Value,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO runs_jobs(id,owner_id,activity_id,stage,desired_generation,input_revision_ids,versions) VALUES($1,$2,$3,$4,$5,$6::jsonb,$7::jsonb) ON CONFLICT DO NOTHING")
        .bind(Uuid::now_v7().to_string()).bind(owner).bind(activity).bind(stage).bind(generation).bind(inputs.to_string()).bind(versions().to_string()).execute(&mut **tx).await?;
    Ok(())
}
pub async fn invalidate_cutoffs(
    tx: &mut Transaction<'_, Postgres>,
    owner: &str,
    event_end: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE runs_jobs SET status='cancelled',lease_owner=NULL,lease_until=NULL WHERE stage='history' AND status IN ('queued','processing') AND activity_id IN (SELECT id FROM runs_activities WHERE owner_id=$1 AND end_order BETWEEN $2::timestamptz AND $2::timestamptz+interval '7 days')").bind(owner).bind(event_end).execute(&mut **tx).await?;
    sqlx::query("UPDATE runs_activities SET history_generation=history_generation+1 WHERE owner_id=$1 AND end_order BETWEEN $2::timestamptz AND $2::timestamptz+interval '7 days'").bind(owner).bind(event_end).execute(&mut **tx).await?;
    sqlx::query("INSERT INTO runs_jobs(id,owner_id,activity_id,stage,desired_generation,input_revision_ids,versions) SELECT gen_random_uuid()::text,owner_id,id,'history',history_generation,'[]'::jsonb,desired_versions FROM runs_activities WHERE owner_id=$1 AND current_manifest_id IS NOT NULL AND end_order BETWEEN $2::timestamptz AND $2::timestamptz+interval '7 days' ON CONFLICT DO NOTHING").bind(owner).bind(event_end).execute(&mut **tx).await?;
    Ok(())
}
pub async fn reprocess(pool: &PgPool, owner: Uuid, id: Uuid) -> Result<(), ApiError> {
    let mut tx = pool.begin().await.map_err(database_error)?;
    let owner = owner.to_string();
    let id = id.to_string();
    owner_lock(&mut tx, &owner).await.map_err(database_error)?;
    let generation=sqlx::query_scalar::<_,i64>("UPDATE runs_activities SET desired_generation=desired_generation+1,desired_versions=$3::jsonb,processing_status='queued',error_code=NULL WHERE owner_id=$1 AND id=$2 RETURNING desired_generation")
        .bind(&owner).bind(&id).bind(versions().to_string()).fetch_optional(&mut *tx).await.map_err(database_error)?.ok_or_else(||ApiError::new(StatusCode::NOT_FOUND,"RUN_NOT_FOUND","Run was not found."))?;
    sqlx::query("UPDATE runs_jobs SET status='cancelled',lease_owner=NULL,lease_until=NULL WHERE activity_id=$1 AND stage<>'history' AND status IN ('queued','processing')").bind(&id).execute(&mut *tx).await.map_err(database_error)?;
    enqueue(&mut tx, &owner, &id, "process", generation, &json!([]))
        .await
        .map_err(database_error)?;
    tx.commit().await.map_err(database_error)
}
const REVOKE_DEPENDENT_EXPORTS: &str = "UPDATE runs_exports e SET revoked=true WHERE e.owner_id=$1 AND (e.activity_ids @> ARRAY[$2]::text[] OR e.activity_ids && ARRAY(SELECT affected.id FROM runs_activities deleted JOIN runs_activities affected ON affected.owner_id=deleted.owner_id AND affected.end_order BETWEEN deleted.end_order AND deleted.end_order+interval '7 days' WHERE deleted.owner_id=$1 AND deleted.id=$2) OR e.activity_ids && ARRAY(SELECT estimate.activity_id FROM runs_estimates estimate JOIN runs_estimate_dependencies dependency ON dependency.estimate_id=estimate.id AND dependency.owner_id=estimate.owner_id WHERE estimate.owner_id=$1 AND dependency.activity_id=$2))";
pub(crate) fn delete_incomplete() -> ApiError {
    ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "RUNS_DELETE_INCOMPLETE",
        "Deletion is not complete. Active export transport or storage is unavailable; retry after it drains.",
    )
}
const ACTIVE_ERASURE_READERS: &str = "SELECT EXISTS(SELECT 1 FROM runs_exports WHERE owner_id=$1 AND (revoked OR activity_ids @> ARRAY[$2]::text[]) AND COALESCE(meta->'activeReads','[]'::jsonb)<>'[]'::jsonb)";
pub async fn delete(pool: &PgPool, owner: Uuid, id: Uuid) -> Result<bool, ApiError> {
    let owner = owner.to_string();
    let id = id.to_string();
    tokio::time::timeout(std::time::Duration::from_secs(22),async{
        // Publish cancellation before waiting for the publisher lock. The
        // tombstone also fences still-uncommitted snapshot generators.
        let mut revoke=pool.begin().await.map_err(|_|delete_incomplete())?;
        sqlx::query("SET LOCAL statement_timeout='5s'").execute(&mut *revoke).await.map_err(|_|delete_incomplete())?;
        sqlx::query("INSERT INTO runs_tombstones(activity_id,owner_id) SELECT id,owner_id FROM runs_activities WHERE owner_id=$1 AND id=$2 ON CONFLICT DO NOTHING")
            .bind(&owner).bind(&id).execute(&mut *revoke).await.map_err(|_|delete_incomplete())?;
        sqlx::query(REVOKE_DEPENDENT_EXPORTS).bind(&owner).bind(&id).execute(&mut *revoke).await.map_err(|_|delete_incomplete())?;
        revoke.commit().await.map_err(|_|delete_incomplete())?;
        tokio::time::timeout(std::time::Duration::from_secs(10),async{
            loop{
                super::export::recover_readers(pool,Some(&owner)).await?;
                let active:bool=sqlx::query_scalar(ACTIVE_ERASURE_READERS).bind(&owner).bind(&id).fetch_one(pool).await.map_err(|_|delete_incomplete())?;
                if !active{return Ok::<_,ApiError>(());}
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }).await.map_err(|_|delete_incomplete())??;
        tokio::time::timeout(std::time::Duration::from_secs(5),delete_final(pool,&owner,&id)).await.map_err(|_|delete_incomplete())?.map_err(|_|delete_incomplete())
    }).await.map_err(|_|delete_incomplete())?
}
async fn delete_final(pool: &PgPool, owner: &str, id: &str) -> Result<bool, ApiError> {
    let mut tx = pool.begin().await.map_err(|_| delete_incomplete())?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(&mut *tx)
        .await
        .map_err(|_| delete_incomplete())?;
    owner_lock(&mut tx, owner)
        .await
        .map_err(|_| delete_incomplete())?;
    let row = sqlx::query(
        "SELECT source_id,end_time FROM runs_activities WHERE owner_id=$1 AND id=$2 FOR UPDATE",
    )
    .bind(owner)
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(database_error)?;
    let Some(row) = row else { return Ok(false) };
    let source: &str = row.try_get("source_id").map_err(database_error)?;
    let end: &str = row.try_get("end_time").map_err(database_error)?;
    // Catch export publishers that committed while deletion waited for the owner lock.
    sqlx::query(REVOKE_DEPENDENT_EXPORTS)
        .bind(owner)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(database_error)?;
    let active: bool = sqlx::query_scalar(ACTIVE_ERASURE_READERS)
        .bind(owner)
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| delete_incomplete())?;
    if active {
        // A late registration must be cancelled durably, not erased while its
        // Body or delivered frames remain alive. Never wait here inside a tx.
        tx.commit().await.map_err(|_| delete_incomplete())?;
        return Err(delete_incomplete());
    }
    sqlx::query(
        "INSERT INTO runs_tombstones(activity_id,owner_id) VALUES($1,$2) ON CONFLICT DO NOTHING",
    )
    .bind(id)
    .bind(owner)
    .execute(&mut *tx)
    .await
    .map_err(database_error)?;
    sqlx::query("DELETE FROM runs_exports WHERE owner_id=$1 AND (revoked OR activity_ids @> ARRAY[$2]::text[]) AND COALESCE(meta->'activeReads','[]'::jsonb)='[]'::jsonb").bind(owner).bind(id).execute(&mut *tx).await.map_err(database_error)?;
    sqlx::query("UPDATE runs_activities SET current_manifest_id=NULL,desired_generation=desired_generation+1 WHERE id=$1").bind(id).execute(&mut *tx).await.map_err(database_error)?;
    sqlx::query("DELETE FROM runs_estimates WHERE owner_id=$1 AND (id IN (SELECT estimate_id FROM runs_estimate_dependencies WHERE activity_id=$2) OR evidence_cutoff::timestamptz BETWEEN $3::timestamptz AND $3::timestamptz+interval '7 days')").bind(owner).bind(id).bind(end).execute(&mut *tx).await.map_err(database_error)?;
    sqlx::query("DELETE FROM runs_activities WHERE owner_id=$1 AND id=$2")
        .bind(owner)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(database_error)?;
    crate::runs::delete_import_receipts(&mut tx, owner, id)
        .await
        .map_err(database_error)?;
    sqlx::query("UPDATE runs_activities a SET duplicate_evidence=COALESCE((SELECT jsonb_agg(item) FROM jsonb_array_elements(a.duplicate_evidence) item WHERE item->>'activityId'<>$2),'[]'::jsonb) WHERE a.owner_id=$1 AND a.duplicate_evidence @> $3::jsonb")
        .bind(owner).bind(id).bind(json!([{"activityId":id}]).to_string()).execute(&mut *tx).await.map_err(database_error)?;
    sqlx::query("DELETE FROM activities WHERE owner_id=$1 AND id=$2")
        .bind(owner)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(database_error)?;
    sqlx::query("DELETE FROM runs_sources WHERE owner_id=$1 AND id=$2 AND NOT EXISTS(SELECT 1 FROM runs_activities WHERE source_id=$2)").bind(owner).bind(source).execute(&mut *tx).await.map_err(database_error)?;
    invalidate_cutoffs(&mut tx, owner, end)
        .await
        .map_err(database_error)?;
    tx.commit().await.map_err(database_error)?;
    Ok(true)
}
