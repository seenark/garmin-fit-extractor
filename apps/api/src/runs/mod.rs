pub mod history;
pub mod import;
pub mod legacy;
pub mod analysis;
pub mod thresholds;
pub mod export;
pub mod privacy;
pub mod store;
pub mod jobs;
pub mod process;
pub mod stream;

use serde_json::{Value,json};
use sqlx::{PgPool,Postgres,Row,Transaction};
use uuid::Uuid;
use crate::error::ApiError;

fn report_counts(items:&[Value])->Value {
    let mut counts=json!({"imported":0,"duplicate":0,"unsupported":0,"failed":0});
    for item in items {
        if let Some(status)=item["status"].as_str().filter(|status|matches!(*status,"imported"|"duplicate"|"unsupported"|"failed")){
            counts[status]=json!(counts[status].as_u64().unwrap_or(0)+1);
        }
    }
    counts
}

pub async fn persist_import_report(pool:&PgPool,owner:Uuid,report:&mut Value)->Result<(),ApiError>{
    let owner=owner.to_string();
    let mut transaction=pool.begin().await.map_err(|_|ApiError::database_error())?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))").bind(&owner).execute(&mut *transaction).await.map_err(|_|ApiError::database_error())?;
    let items=report["items"].as_array_mut().ok_or_else(ApiError::processing_error)?;
    for item in items.iter_mut(){
        if let Some(activity)=item["activityId"].as_str(){
            let live:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM runs_activities WHERE owner_id=$1 AND id=$2)")
                .bind(&owner).bind(activity).fetch_one(&mut *transaction).await.map_err(|_|ApiError::database_error())?;
            if !live {
                *item=json!({"index":item["index"],"name":"deleted-input","status":"failed","reason":"IMPORT_ACTIVITY_DELETED","activityId":null,"warnings":[]});
            }
        }
    }
    report["counts"]=report_counts(items);
    sqlx::query("INSERT INTO runs_import_reports(id,owner_id,payload) VALUES($1,$2,$3::jsonb)")
        .bind(report["batchId"].as_str()).bind(&owner).bind(report.to_string()).execute(&mut *transaction).await.map_err(|_|ApiError::database_error())?;
    transaction.commit().await.map_err(|_|ApiError::database_error())?;
    Ok(())
}

/// Erase deleted activity provenance without dropping valid sibling import receipts.
pub async fn delete_import_receipts(transaction:&mut Transaction<'_,Postgres>,owner:&str,activity:&str)->Result<(),sqlx::Error>{
    let rows=sqlx::query("SELECT id,payload::text FROM runs_import_reports WHERE owner_id=$1 AND payload @> $2::jsonb FOR UPDATE")
        .bind(owner).bind(json!({"items":[{"activityId":activity}]}).to_string()).fetch_all(&mut **transaction).await?;
    for row in rows{
        let id:String=row.try_get("id")?;
        let text:String=row.try_get("payload")?;
        let mut report:Value=serde_json::from_str(&text).map_err(|_|sqlx::Error::Protocol("Invalid stored import report".into()))?;
        let items=report["items"].as_array_mut().ok_or_else(||sqlx::Error::Protocol("Invalid stored import items".into()))?;
        items.retain(|item|item["activityId"].as_str()!=Some(activity));
        if items.is_empty(){
            sqlx::query("DELETE FROM runs_import_reports WHERE owner_id=$1 AND id=$2").bind(owner).bind(&id).execute(&mut **transaction).await?;
        }else{
            report["counts"]=report_counts(items);
            sqlx::query("UPDATE runs_import_reports SET payload=$3::jsonb WHERE owner_id=$1 AND id=$2").bind(owner).bind(&id).bind(report.to_string()).execute(&mut **transaction).await?;
        }
    }
    Ok(())
}
