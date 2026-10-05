use chrono::{DateTime, Duration};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub const VERSION:&str="runs-history-2.0.0";
pub(crate) fn source_hash()->String{
    format!("{:x}",Sha256::digest(include_bytes!("history.rs")))
}

/// Preserve each target's most recent numeric estimate and its original event-time cutoff.
pub fn latest_projection(cutoff: &str, attempt: Value, history: &[Value]) -> Value {
    let now = DateTime::parse_from_rfc3339(cutoff).ok();
    let mut available = json!({"lt1":null,"lt2":null});
    let mut stale = false;
    for target in ["lt1", "lt2"] {
        let newest = history.iter().chain(std::iter::once(&attempt)).filter(|row| {
            row[target]["value"]["heartRateBpm"].as_f64().is_some_and(f64::is_finite)
                && row["evidenceCutoff"].as_str().and_then(|time| DateTime::parse_from_rfc3339(time).ok()).is_some_and(|time| now.is_some_and(|now| time <= now))
        }).max_by_key(|row| (
            row["evidenceCutoff"].as_str().and_then(|time| DateTime::parse_from_rfc3339(time).ok()),
            row["computedAt"].as_str().and_then(|time| DateTime::parse_from_rfc3339(time).ok()),
            row["_historyGeneration"].as_u64().unwrap_or(u64::MAX),
        ));
        if let Some(row) = newest {
            let mut result = row[target].clone();
            let date = row["evidenceCutoff"].as_str().and_then(|time| DateTime::parse_from_rfc3339(time).ok());
            let old = match (now, date) {
                (Some(now), Some(date)) => date < now || now.signed_duration_since(date) > Duration::days(7),
                _ => true,
            }||row["_historyStale"].as_bool().unwrap_or(false);
            result["activityId"] = row["activityId"].clone();
            result["evidenceCutoff"] = row["evidenceCutoff"].clone();
            result["computedAt"] = row["computedAt"].clone();
            result["stale"] = json!(old);
            available[target] = result;
            stale |= old;
        }
    }
    let engine_status = attempt.get("engineStatus").cloned()
        .or_else(|| attempt["lt1"].get("engineStatus").cloned())
        .unwrap_or(Value::String("unavailable".into()));
    json!({"evidenceCutoff":cutoff,"latestAttempt":attempt,"lastAvailable":available,"stale":stale,"engineStatus":engine_status})
}

use crate::{db, error::ApiError, runs::thresholds};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

fn database(_: sqlx::Error) -> ApiError { ApiError::database_error() }

async fn evidence(transaction: &mut Transaction<'_, Postgres>, owner: &str, cutoff: &str) -> Result<Vec<Value>, ApiError> {
    let rows = sqlx::query(
        "SELECT a.id,a.observation_group_id,a.start_time,a.end_time,a.current_manifest_id,n.id AS normalized_revision_id,n.payload::text AS normalized,z.payload::text AS analysis,z.validation::text AS analysis_validation
         FROM runs_activities a
         JOIN runs_manifests m ON m.id=a.current_manifest_id AND m.owner_id=a.owner_id
         JOIN runs_revisions n ON n.id=m.normalized_revision_id
         JOIN runs_revisions z ON z.id=m.analysis_revision_id
         WHERE a.owner_id=$1 AND a.end_order<=$2::timestamptz AND a.end_order>=$2::timestamptz-INTERVAL '7 days'
         ORDER BY a.observation_group_id,a.id ASC")
        .bind(owner).bind(cutoff).fetch_all(&mut **transaction).await.map_err(database)?;
    rows.into_iter().map(|row| {
        let normalized: String = row.try_get("normalized").map_err(database)?;
        let analysis: String = row.try_get("analysis").map_err(database)?;
        let validation:String=row.try_get("analysis_validation").map_err(database)?;
        let validation:Value=serde_json::from_str(&validation).map_err(|_|ApiError::database_error())?;
        let mut normalized:Value=serde_json::from_str(&normalized).map_err(|_|ApiError::database_error())?;
        let revision:String=row.try_get("normalized_revision_id").map_err(database)?;
        normalized["sourceRevision"]=json!(revision);
        normalized["projectionVersion"]=json!(thresholds::PROJECTION_VERSION);
        normalized["normalizedDocumentHash"]=normalized["documents"]["archive"]["sha256"].clone();
        if validation["normalizedRevisionId"]!=json!(revision)
            ||validation["projectionVersion"]!=json!(thresholds::PROJECTION_VERSION)
            ||validation["normalizedDocumentHash"]!=normalized["normalizedDocumentHash"]
            ||validation["inputHash"].as_str().is_none_or(|hash|hash.len()!=64||!hash.bytes().all(|byte|byte.is_ascii_hexdigit()))
        {
            return Err(ApiError::new(axum::http::StatusCode::SERVICE_UNAVAILABLE,"RUNS_EVIDENCE_INCOHERENT","Historical evidence does not match its immutable numerical input."));
        }
        let verification=json!({"kind":"immutableNumericalProjection","revisionId":revision,"inputHash":validation["inputHash"],"projectionVersion":validation["projectionVersion"]});
        Ok(json!({
            "activityId":row.try_get::<String,_>("id").map_err(database)?,
            "observationGroupId":row.try_get::<String,_>("observation_group_id").map_err(database)?,
            "manifestId":row.try_get::<String,_>("current_manifest_id").map_err(database)?,
            "startTime":row.try_get::<String,_>("start_time").map_err(database)?,
            "endTime":row.try_get::<String,_>("end_time").map_err(database)?,
            "normalized":normalized,
            "analysis":serde_json::from_str::<Value>(&analysis).map_err(|_|ApiError::database_error())?,
            "evidenceVerification":verification
        }))
    }).collect()
}

/// Publish an event-time estimate only while its history generation and manifest remain live.
pub async fn recompute(pool: &PgPool, owner: Uuid, activity: Uuid, generation: i64,lease_owner:&str) -> Result<(), ApiError> {
    let owner = owner.to_string();
    let activity = activity.to_string();
    let mut transaction = pool.begin().await.map_err(database)?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))").bind(&owner)
        .execute(&mut *transaction).await.map_err(database)?;
    let row = sqlx::query("SELECT end_time,current_manifest_id FROM runs_activities a WHERE owner_id=$1 AND id=$2 AND history_generation=$3 AND current_manifest_id IS NOT NULL AND EXISTS(SELECT 1 FROM runs_jobs j WHERE j.owner_id=$1 AND j.activity_id=$2 AND j.stage='history' AND j.desired_generation=$3 AND j.status='processing' AND j.lease_owner=$4 AND j.lease_until>clock_timestamp()) AND EXISTS(SELECT 1 FROM runs_slots WHERE stage='worker' AND lease_owner=$4 AND lease_until>clock_timestamp()) FOR UPDATE OF a")
        .bind(&owner).bind(&activity).bind(generation).bind(lease_owner).fetch_optional(&mut *transaction).await.map_err(database)?;
    let Some(row) = row else { return Err(ApiError::new(axum::http::StatusCode::SERVICE_UNAVAILABLE,"JOB_LEASE_LOST","Historical processing lease expired or was cancelled.")); };
    let cutoff: String = row.try_get("end_time").map_err(database)?;
    let manifest: String = row.try_get("current_manifest_id").map_err(database)?;
    let inputs = evidence(&mut transaction, &owner, &cutoff).await?;
    let mut result = thresholds::estimate_history(&cutoff, &inputs);
    let computed_at = db::created_at_now();
    result["activityId"] = json!(activity);
    result["evidenceCutoff"] = json!(cutoff);
    result["computedAt"] = json!(computed_at);
    validate_result(&result)?;
    let estimate = Uuid::now_v7().to_string();
    let inserted = sqlx::query("INSERT INTO runs_estimates(id,owner_id,activity_id,evidence_cutoff,computed_at,generation,manifest_id,payload) SELECT $1,$2,$3,$4,$5,$6,$7,$8::jsonb WHERE EXISTS(SELECT 1 FROM runs_activities WHERE owner_id=$2 AND id=$3 AND history_generation=$6 AND current_manifest_id=$7) AND NOT EXISTS(SELECT 1 FROM runs_tombstones WHERE activity_id=$3) AND EXISTS(SELECT 1 FROM runs_jobs WHERE owner_id=$2 AND activity_id=$3 AND stage='history' AND desired_generation=$6 AND status='processing' AND lease_owner=$9 AND lease_until>clock_timestamp()) AND EXISTS(SELECT 1 FROM runs_slots WHERE stage='worker' AND lease_owner=$9 AND lease_until>clock_timestamp()) ON CONFLICT(activity_id,generation) DO NOTHING")
        .bind(&estimate).bind(&owner).bind(&activity).bind(&cutoff).bind(&computed_at).bind(generation).bind(&manifest).bind(result.to_string()).bind(lease_owner)
        .execute(&mut *transaction).await.map_err(database)?.rows_affected();
    if inserted == 1 {
        for input in &inputs {
            sqlx::query("INSERT INTO runs_estimate_dependencies(estimate_id,owner_id,activity_id,manifest_id) VALUES($1,$2,$3,$4)")
                .bind(&estimate).bind(&owner).bind(input["activityId"].as_str()).bind(input["manifestId"].as_str())
                .execute(&mut *transaction).await.map_err(database)?;
        }
    } else {
        let existing:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM runs_estimates WHERE owner_id=$1 AND activity_id=$2 AND generation=$3 AND manifest_id=$4)")
            .bind(&owner).bind(&activity).bind(generation).bind(&manifest).fetch_one(&mut *transaction).await.map_err(database)?;
        if !existing {
            return Err(ApiError::new(axum::http::StatusCode::SERVICE_UNAVAILABLE,"JOB_LEASE_LOST","Historical processing lease expired or was cancelled."));
        }
    }
    transaction.commit().await.map_err(database)?;
    Ok(())
}

fn validate_result(result: &Value) -> Result<(), ApiError> {
    for target in ["lt1","lt2"] {
        let target = &result[target];
        let valid = match target["status"].as_str() {
            Some("insufficient_data") => target["value"].is_null(),
            Some("estimated"|"low_confidence") => target["value"]["heartRateBpm"].as_f64().is_some_and(|value| value.is_finite() && value > 0.0),
            _ => false,
        };
        if !valid || !target["reasons"].is_array() || !target["evidence"].is_object() {
            return Err(ApiError::new(axum::http::StatusCode::INTERNAL_SERVER_ERROR,"ANALYSIS_VALIDATION_FAILED","Threshold result failed publication validation."));
        }
    }
    Ok(())
}

pub async fn for_manifest(pool: &PgPool, owner: Uuid, activity: Uuid, manifest:Option<&str>,generation:i64) -> Result<Value, ApiError> {
    let result: Option<String> = sqlx::query_scalar("SELECT payload::text FROM runs_estimates WHERE owner_id=$1 AND activity_id=$2 AND generation=$3 AND manifest_id=$4")
        .bind(owner.to_string()).bind(activity.to_string()).bind(generation).bind(manifest).fetch_optional(pool).await.map_err(database)?;
    result.map(|text| serde_json::from_str(&text).map_err(|_|ApiError::database_error())).transpose().map(|result| result.unwrap_or(Value::Null))
}

pub async fn trend(pool: &PgPool, owner: Uuid) -> Result<Value, ApiError> {
    let rows: Vec<String> = sqlx::query_scalar("SELECT e.payload::text FROM runs_estimates e JOIN runs_activities a ON a.id=e.activity_id AND a.owner_id=e.owner_id WHERE a.owner_id=$1 AND e.generation=a.history_generation AND e.manifest_id=a.current_manifest_id ORDER BY a.end_order ASC,a.id ASC")
        .bind(owner.to_string()).fetch_all(pool).await.map_err(database)?;
    let items = rows.into_iter().map(|text|serde_json::from_str::<Value>(&text).map_err(|_|ApiError::database_error())).collect::<Result<Vec<_>,_>>()?;
    Ok(json!({"items":items}))
}

pub async fn latest(pool: &PgPool, owner: Uuid) -> Result<Value, ApiError> {
    let cutoff = db::created_at_now();
    let owner = owner.to_string();
    let mut transaction = pool.begin().await.map_err(database)?;
    sqlx::query("SELECT pg_advisory_xact_lock_shared(hashtextextended($1,0))").bind(&owner)
        .execute(&mut *transaction).await.map_err(database)?;
    let inputs = evidence(&mut transaction,&owner,&cutoff).await?;
    let mut attempt = thresholds::estimate_history(&cutoff,&inputs);
    attempt["activityId"] = Value::Null;
    attempt["computedAt"] = json!(db::created_at_now());
    validate_result(&attempt)?;
    let rows=sqlx::query("SELECT e.payload::text,e.generation,(e.generation<>a.history_generation OR e.manifest_id IS DISTINCT FROM a.current_manifest_id) AS history_stale FROM runs_estimates e JOIN runs_activities a ON a.id=e.activity_id AND a.owner_id=e.owner_id WHERE a.owner_id=$1 AND a.end_order<=$2::timestamptz ORDER BY a.end_order DESC,e.computed_at DESC,e.generation DESC")
        .bind(&owner).bind(&cutoff).fetch_all(&mut *transaction).await.map_err(database)?;
    let history=rows.into_iter().map(|row|{
        let text:String=row.try_get("payload").map_err(database)?;
        let mut value:Value=serde_json::from_str(&text).map_err(|_|ApiError::database_error())?;
        value["_historyGeneration"]=json!(row.try_get::<i64,_>("generation").map_err(database)?);
        value["_historyStale"]=json!(row.try_get::<bool,_>("history_stale").map_err(database)?);
        Ok::<_,ApiError>(value)
    }).collect::<Result<Vec<_>,_>>()?;
    transaction.commit().await.map_err(database)?;
    Ok(latest_projection(&cutoff,attempt,&history))
}
