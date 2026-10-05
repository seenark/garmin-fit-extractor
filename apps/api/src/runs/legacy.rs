use crate::{db, error::ApiError, model::Analysis};
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

pub fn summary(analysis: Option<&Analysis>) -> Value {
    match analysis {
        Some(a) => json!({
            "distanceMeters":a.summary.distance.value,
            "timerTimeSeconds":a.summary.moving_time.value,
            "elapsedTimeSeconds":a.summary.duration.value,
            "movingTimeSeconds":null,
            "averageSpeedMps":null,
            "averagePaceSecondsPerKm":a.pace.average.value,
            "averageHeartRateBpm":a.heart_rate.average_bpm,
            "averagePowerWatts":a.power.average_watts,
            "averageCadenceStepsPerMinute":a.running_dynamics.cadence.average_steps_per_minute
        }),
        None => json!({"distanceMeters":null,"timerTimeSeconds":null,"elapsedTimeSeconds":null,"movingTimeSeconds":null,"averageSpeedMps":null,"averagePaceSecondsPerKm":null,"averageHeartRateBpm":null,"averagePowerWatts":null,"averageCadenceStepsPerMinute":null}),
    }
}

pub async fn insert_projection(transaction: &mut Transaction<'_, Postgres>, id: &str, owner: &str, start: Option<&str>, payload: Option<&str>, succeeded: bool, error_code: Option<&str>) -> Result<(), sqlx::Error> {
    let analysis = payload.and_then(|payload| serde_json::from_str::<Analysis>(payload).ok());
    let start = start.filter(|time| chrono::DateTime::parse_from_rfc3339(time).is_ok());
    let end = start.and_then(|start| {
        let seconds = analysis.as_ref()?.summary.duration.value?;
        if !seconds.is_finite() || seconds < 0.0 || seconds > 365.0 * 86400.0 { return None; }
        let start = chrono::DateTime::parse_from_rfc3339(start).ok()?;
        start.checked_add_signed(chrono::Duration::milliseconds((seconds * 1000.0).round() as i64))
            .map(|end| end.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
    });
    sqlx::query("INSERT INTO runs_legacy_summaries (id,owner_id,start_time,end_time,start_order,summary,processing_status,error_code) VALUES ($1,$2,$3,$4,$3::timestamptz,$5::jsonb,$6,$7) ON CONFLICT(id) DO NOTHING")
        .bind(id).bind(owner).bind(start).bind(end).bind(summary(analysis.as_ref()).to_string())
        .bind(if succeeded {"ready"} else {"failed"}).bind(error_code).execute(&mut **transaction).await?;
    Ok(())
}

pub async fn backfill(pool: &PgPool) -> Result<(), sqlx::Error> {
    loop {
        let rows = sqlx::query("SELECT e.id,e.user_id,e.activity_date,e.normalized_json,e.status,e.error_code FROM extractions e LEFT JOIN runs_legacy_summaries l ON l.id=e.id WHERE l.id IS NULL ORDER BY e.id LIMIT 100")
            .fetch_all(pool).await?;
        if rows.is_empty() { return Ok(()); }
        let mut transaction = pool.begin().await?;
        for row in rows {
            let id: String = row.try_get("id")?;
            let owner: String = row.try_get("user_id")?;
            let start: Option<String> = row.try_get("activity_date")?;
            let payload: Option<String> = row.try_get("normalized_json")?;
            let status: String = row.try_get("status")?;
            let error: Option<String> = row.try_get("error_code")?;
            insert_projection(&mut transaction,&id,&owner,start.as_deref(),payload.as_deref(),status=="succeeded",error.as_deref()).await?;
        }
        transaction.commit().await?;
    }
}

pub async fn delete(pool: &PgPool, owner: Uuid, id: Uuid) -> Result<bool, ApiError> {
    db::delete_one(pool, owner, id).await.map_err(|_| ApiError::database_error())
}

pub fn row_json(row: &sqlx::postgres::PgRow) -> Result<Value, ApiError> {
    let field = |name| row.try_get::<Option<String>,_>(name).map_err(|_| ApiError::database_error());
    let payload: String = row.try_get("summary").map_err(|_| ApiError::database_error())?;
    let status: String = row.try_get("processing_status").map_err(|_| ApiError::database_error())?;
    Ok(json!({"id":field("id")?,"startTime":field("start_time")?,"endTime":field("end_time")?,"summary":serde_json::from_str::<Value>(&payload).map_err(|_| ApiError::database_error())?,"processing":{"status":status,"stale":false,"updateFailed":false,"errorCode":field("error_code")?},"sourceUnavailable":true,"revisionId":null,"possibleDuplicate":false,"fidelityWarnings":["LEGACY_SOURCE_UNAVAILABLE"]}))
}
