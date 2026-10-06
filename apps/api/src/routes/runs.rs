use crate::{
    app::AppState,
    auth::AuthenticatedUser,
    error::ApiError,
    runs::{export, history, import, jobs, legacy, store, stream::OBSERVATION_RULE},
};
use axum::{
    Json, Router,
    extract::{Multipart, Path, State},
    http::{StatusCode, Uri},
    response::Response,
    routing::{get, post},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/runs", get(list))
        .route(
            "/api/v2/runs/imports",
            post(import_runs).layer(axum::extract::DefaultBodyLimit::max(210 * 1024 * 1024)),
        )
        .route("/api/v2/runs/thresholds/latest", get(latest))
        .route("/api/v2/runs/thresholds/trend", get(trend))
        .route("/api/v2/runs/exports", post(create_export))
        .route(
            "/api/v2/runs/exports/{token}",
            get(serve_export).head(serve_export_head),
        )
        .route("/api/v2/runs/{id}", get(detail).delete(delete))
        .route("/api/v2/runs/{id}/reprocess", post(reprocess))
        .route("/api/v2/runs/{id}/evidence", get(evidence))
        .route("/api/v2", axum::routing::any(api_not_found))
        .route("/api/v2/{*path}", axum::routing::any(api_not_found))
        .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("private, no-store"),
        ))
}

fn id(text: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(text).map_err(|_| ApiError::not_found())
}
fn database(_: sqlx::Error) -> ApiError {
    ApiError::database_error()
}
async fn api_not_found() -> ApiError {
    ApiError::api_route_not_found()
}

async fn import_runs(
    State(state): State<AppState>,
    AuthenticatedUser { user_id, .. }: AuthenticatedUser,
    multipart: Result<Multipart, axum::extract::multipart::MultipartRejection>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let multipart = multipart.map_err(|error| {
        if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
            ApiError::request_too_large()
        } else {
            ApiError::invalid_multipart()
        }
    })?;
    // The owned task drains bounded ZIP work and reaps a cancelled decoder
    // before releasing admission when the HTTP response future is dropped.
    let (cancel, cancellation) = tokio::sync::watch::channel(false);
    let result =
        tokio::spawn(async move { import_inner(state, user_id, multipart, cancellation).await })
            .await
            .map_err(|_| ApiError::processing_error())?;
    drop(cancel);
    result
}

async fn import_inner(
    state: AppState,
    user_id: Uuid,
    multipart: Multipart,
    mut cancellation: tokio::sync::watch::Receiver<bool>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let mut admission = jobs::import_admission(&state.db).await?;
    let staged = import::parse(multipart).await?;
    let batch = Uuid::now_v7();
    let mut items = Vec::with_capacity(staged.len());
    let mut counts = json!({"imported":0,"duplicate":0,"unsupported":0,"failed":0});
    for (index, item) in staged.into_iter().enumerate() {
        if admission.is_cancelled() || *cancellation.borrow() || cancellation.has_changed().is_err()
        {
            return Err(ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "IMPORT_LEASE_LOST",
                "Import admission expired.",
            ));
        }
        let mut status = match item.status {
            import::InputStatus::Ready => "imported",
            import::InputStatus::Unsupported => "unsupported",
            import::InputStatus::Failed => "failed",
        };
        let mut reason = item.reason.map(str::to_owned);
        let mut activity_id = None;
        let warnings = item.warnings;
        if let Some(bytes) = item.bytes {
            if let Some(duplicate) = store::find_duplicate(&state.db, user_id, &bytes).await? {
                activity_id = Some(duplicate.activity_id);
                status = "duplicate";
            } else {
                let decoder_capacity = admission.decoder_hold()?;
                let decoded = jobs::decode_import(
                    &state.db,
                    &bytes,
                    async {
                        tokio::select! {
                            _ = admission.cancelled() => {},
                            _ = jobs::cancelled(&mut cancellation) => {},
                        }
                    },
                    Some(decoder_capacity),
                )
                .await;
                if admission.is_cancelled()
                    || *cancellation.borrow()
                    || cancellation.has_changed().is_err()
                {
                    return Err(ApiError::new(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "IMPORT_LEASE_LOST",
                        "Import admission expired.",
                    ));
                }
                match decoded {
                    Ok(mut document) => {
                        document.protect_cpu(&admission);
                        match store::accept_spool(
                            &state.db,
                            user_id,
                            &bytes,
                            document,
                            admission.holder(),
                        )
                        .await
                        {
                            Ok(accepted) => {
                                activity_id = Some(accepted.activity_id);
                                if accepted.duplicate {
                                    status = "duplicate";
                                }
                            }
                            Err(error) => {
                                reason = Some(error.code().to_owned());
                                status = "failed";
                            }
                        }
                    }
                    Err(error) => {
                        reason = Some(error.code().to_owned());
                        status = if error.code().starts_with("UNSUPPORTED_") {
                            "unsupported"
                        } else {
                            "failed"
                        };
                    }
                }
            }
        }
        counts[status] = json!(counts[status].as_u64().unwrap_or(0) + 1);
        items.push(json!({"index":index,"name":item.name,"status":status,"reason":reason,"activityId":activity_id,"warnings":warnings}));
    }
    if admission.is_cancelled() || *cancellation.borrow() || cancellation.has_changed().is_err() {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "IMPORT_LEASE_LOST",
            "Import admission expired.",
        ));
    }
    let mut response = json!({"batchId":batch,"items":items,"counts":counts});
    crate::runs::persist_import_report(&state.db, user_id, &mut response).await?;
    Ok((StatusCode::CREATED, Json(response)))
}

fn pagination(uri: &Uri) -> Result<(i64, i64, bool), ApiError> {
    let (mut limit, mut offset, mut ascending) = (50, 0, false);
    for (key, value) in url::form_urlencoded::parse(uri.query().unwrap_or("").as_bytes()) {
        match key.as_ref() {
            "limit" => limit = value.parse().map_err(|_| ApiError::invalid_pagination())?,
            "offset" => offset = value.parse().map_err(|_| ApiError::invalid_pagination())?,
            "order" => {
                ascending = match value.as_ref() {
                    "asc" => true,
                    "desc" => false,
                    _ => return Err(ApiError::invalid_pagination()),
                }
            }
            "sort" if value == "startTime" => {}
            _ => return Err(ApiError::invalid_pagination()),
        }
    }
    if !(1..=100).contains(&limit) || offset < 0 {
        return Err(ApiError::invalid_pagination());
    }
    Ok((limit, offset, ascending))
}

async fn list(
    State(state): State<AppState>,
    AuthenticatedUser { user_id, .. }: AuthenticatedUser,
    uri: Uri,
) -> Result<Json<Value>, ApiError> {
    let (limit, offset, ascending) = pagination(&uri)?;
    let owner = user_id.to_string();
    let total:i64=sqlx::query_scalar("SELECT (SELECT COUNT(*) FROM runs_activities WHERE owner_id=$1)+(SELECT COUNT(*) FROM runs_legacy_summaries WHERE owner_id=$1)").bind(&owner).fetch_one(&state.db).await.map_err(database)?;
    let direction = if ascending { "ASC" } else { "DESC" };
    let mut query = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        "SELECT id,start_time,end_time,summary::text,processing_status,error_code,current_manifest_id,desired_generation,manifest_generation,source_unavailable,duplicate_evidence::text,history_status FROM (SELECT a.id,a.start_time,a.end_time,a.start_order,a.summary,a.processing_status,a.error_code,a.current_manifest_id,a.desired_generation,m.generation AS manifest_generation,false AS source_unavailable,a.duplicate_evidence,CASE WHEN m.versions->>'observationRule' IS DISTINCT FROM $4 THEN 'pending' WHEN h.id IS NOT NULL THEN 'ready' WHEN EXISTS(SELECT 1 FROM runs_jobs j WHERE j.owner_id=a.owner_id AND j.activity_id=a.id AND j.stage='history' AND j.desired_generation=a.history_generation AND j.status='failed') THEN 'failed' ELSE 'pending' END AS history_status FROM runs_activities a LEFT JOIN runs_manifests m ON m.id=a.current_manifest_id AND m.owner_id=a.owner_id LEFT JOIN runs_estimates h ON h.activity_id=a.id AND h.owner_id=a.owner_id AND h.generation=a.history_generation AND h.manifest_id=a.current_manifest_id AND m.versions->>'observationRule'=$4 WHERE a.owner_id=$1 UNION ALL SELECT id,start_time,end_time,start_order,summary,processing_status,error_code,NULL,0,NULL,true,'null'::jsonb,NULL FROM runs_legacy_summaries WHERE owner_id=$1) projected ORDER BY start_order ",
    );
    query
        .push(direction)
        .push(" NULLS LAST,id ")
        .push(direction)
        .push(" LIMIT $2 OFFSET $3");
    let rows = query
        .build()
        .bind(&owner)
        .bind(limit)
        .bind(offset)
        .bind(OBSERVATION_RULE)
        .fetch_all(&state.db)
        .await
        .map_err(database)?;
    let items = rows
        .iter()
        .map(|row| {
            let mut value = activity_row(row)?;
            if let Some(status) = row
                .try_get::<Option<String>, _>("history_status")
                .map_err(database)?
            {
                value["processing"]["historyStale"] =
                    json!(value["processing"]["stale"] == true || status != "ready");
                value["processing"]["historyStatus"] = json!(status);
            }
            Ok::<_, ApiError>(value)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(
        json!({"items":items,"total":total,"limit":limit,"offset":offset}),
    ))
}

fn activity_row(row: &sqlx::postgres::PgRow) -> Result<Value, ApiError> {
    let status: String = row.try_get("processing_status").map_err(database)?;
    let manifest: Option<String> = row.try_get("current_manifest_id").map_err(database)?;
    let desired: i64 = row.try_get("desired_generation").map_err(database)?;
    let current: Option<i64> = row.try_get("manifest_generation").map_err(database)?;
    let source_unavailable: bool = row.try_get("source_unavailable").map_err(database)?;
    let summary: String = row.try_get("summary").map_err(database)?;
    let duplicate: String = row.try_get("duplicate_evidence").map_err(database)?;
    let duplicate: Value =
        serde_json::from_str(&duplicate).map_err(|_| ApiError::database_error())?;
    let mut result = json!({
        "id":row.try_get::<String,_>("id").map_err(database)?,
        "startTime":row.try_get::<Option<String>,_>("start_time").map_err(database)?,
        "endTime":row.try_get::<Option<String>,_>("end_time").map_err(database)?,
        "summary":serde_json::from_str::<Value>(&summary).map_err(|_|ApiError::database_error())?,
        "processing":{"status":status,"stale":manifest.is_some()&&current!=Some(desired),"updateFailed":status=="failed"&&manifest.is_some(),"errorCode":row.try_get::<Option<String>,_>("error_code").map_err(database)?},
        "sourceUnavailable":source_unavailable,"revisionId":manifest,
        "possibleDuplicate":duplicate.as_array().is_some_and(|items|!items.is_empty())
    });
    if source_unavailable {
        result["fidelityWarnings"] = json!(["LEGACY_SOURCE_UNAVAILABLE"]);
    }
    Ok(result)
}

async fn detail_value(pool: &sqlx::PgPool, owner: Uuid, activity: Uuid) -> Result<Value, ApiError> {
    let row=sqlx::query("SELECT a.id,a.start_time,a.end_time,a.summary::text,a.processing_status,a.error_code,a.current_manifest_id,a.desired_generation,a.history_generation,m.generation AS manifest_generation,m.versions->>'observationRule'=$3 AS history_compatible,false AS source_unavailable,a.duplicate_evidence::text,z.payload::text AS analysis FROM runs_activities a LEFT JOIN runs_manifests m ON m.id=a.current_manifest_id AND m.owner_id=a.owner_id LEFT JOIN runs_revisions z ON z.id=m.analysis_revision_id WHERE a.owner_id=$1 AND a.id=$2")
        .bind(owner.to_string()).bind(activity.to_string()).bind(OBSERVATION_RULE).fetch_optional(pool).await.map_err(database)?;
    if let Some(row) = row {
        let mut detail = activity_row(&row)?;
        detail["normalized"] = Value::Null;
        detail["analysis"] = row
            .try_get::<Option<String>, _>("analysis")
            .map_err(database)?
            .map(|text| serde_json::from_str(&text).map_err(|_| ApiError::database_error()))
            .transpose()?
            .unwrap_or(Value::Null);
        let manifest: Option<String> = row.try_get("current_manifest_id").map_err(database)?;
        let generation: i64 = row.try_get("history_generation").map_err(database)?;
        detail["historicalThresholds"] =
            history::for_manifest(pool, owner, activity, manifest.as_deref(), generation).await?;
        let history_status = if !detail["historicalThresholds"].is_null() {
            "ready"
        } else if row
            .try_get::<Option<bool>, _>("history_compatible")
            .map_err(database)?
            != Some(true)
        {
            "pending"
        } else {
            let failed:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM runs_jobs WHERE owner_id=$1 AND activity_id=$2 AND stage='history' AND desired_generation=$3 AND status='failed')")
                .bind(owner.to_string()).bind(activity.to_string()).bind(generation).fetch_one(pool).await.map_err(database)?;
            if failed { "failed" } else { "pending" }
        };
        detail["processing"]["historyStatus"] = json!(history_status);
        detail["processing"]["historyStale"] =
            json!(detail["processing"]["stale"] == true || history_status != "ready");
        return Ok(detail);
    }
    let row=sqlx::query("SELECT id,start_time,end_time,summary::text,processing_status,error_code FROM runs_legacy_summaries WHERE owner_id=$1 AND id=$2")
        .bind(owner.to_string()).bind(activity.to_string()).fetch_optional(pool).await.map_err(database)?.ok_or_else(ApiError::not_found)?;
    let mut detail = legacy::row_json(&row)?;
    for field in ["normalized", "analysis", "historicalThresholds"] {
        detail[field] = Value::Null;
    }
    Ok(detail)
}

async fn detail(
    State(state): State<AppState>,
    AuthenticatedUser { user_id, .. }: AuthenticatedUser,
    Path(activity): Path<String>,
) -> Result<Response, ApiError> {
    use axum::response::IntoResponse;
    let activity = id(&activity)?;
    let mut value = detail_value(&state.db, user_id, activity).await?;
    let Some(manifest) = value["revisionId"].as_str() else {
        return Ok(Json(value).into_response());
    };
    // Pin the same immutable manifest used by summary, analysis, and evidence.
    let row=sqlx::query("SELECT n.id,(n.payload->'documents'->'archive')::text AS document FROM runs_manifests m JOIN runs_activities a ON a.id=m.activity_id AND a.owner_id=m.owner_id JOIN runs_revisions n ON n.id=m.normalized_revision_id AND n.owner_id=m.owner_id WHERE m.id=$1 AND m.activity_id=$2 AND m.owner_id=$3")
        .bind(manifest).bind(activity.to_string()).bind(user_id.to_string()).fetch_optional(&state.db).await.map_err(database)?.ok_or_else(ApiError::not_found)?;
    let document: String = row.try_get("document").map_err(database)?;
    let document: Value =
        serde_json::from_str(&document).map_err(|_| ApiError::processing_error())?;
    let expected_bytes = document["byteLength"]
        .as_u64()
        .filter(|size| *size > 0 && *size <= 512 * 1024 * 1024)
        .ok_or_else(ApiError::processing_error)?;
    let chunk_count = document["chunkCount"]
        .as_u64()
        .filter(|count| *count == expected_bytes.div_ceil(65536))
        .ok_or_else(ApiError::processing_error)?;
    let expected_hash = document["sha256"]
        .as_str()
        .filter(|hash| hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or_else(ApiError::processing_error)?
        .to_owned();
    value
        .as_object_mut()
        .ok_or_else(ApiError::processing_error)?
        .remove("normalized");
    let mut prefix = serde_json::to_vec(&value).map_err(|_| ApiError::processing_error())?;
    if prefix.pop() != Some(b'}') {
        return Err(ApiError::processing_error());
    }
    prefix.extend_from_slice(b",\"normalized\":");
    let content_length = prefix.len() as u64 + expected_bytes + 2;
    let stream = futures_util::stream::unfold(
        Some(DetailStream {
            pool: state.db,
            owner: user_id.to_string(),
            revision: row.try_get("id").map_err(database)?,
            prefix: Some(prefix),
            position: 0,
            chunk_count,
            expected_bytes,
            expected_hash,
            emitted_bytes: 0,
            hasher: Sha256::new(),
        }),
        |state| async move {
            let mut state = state?;
            if let Some(prefix) = state.prefix.take() {
                return Some((
                    Ok::<_, std::io::Error>(axum::body::Bytes::from(prefix)),
                    Some(state),
                ));
            }
            if state.position < state.chunk_count {
                let chunk=sqlx::query_scalar::<_,Vec<u8>>("SELECT payload FROM runs_revision_chunks WHERE owner_id=$1 AND revision_id=$2 AND document='archive' AND position=$3")
                .bind(&state.owner).bind(&state.revision).bind(state.position as i64).fetch_optional(&state.pool).await;
                let chunk = match chunk {
                    Ok(Some(chunk)) if !chunk.is_empty() && chunk.len() <= 65536 => chunk,
                    _ => {
                        return Some((
                            Err(std::io::Error::other(
                                "Run document is no longer available.",
                            )),
                            None,
                        ));
                    }
                };
                state.emitted_bytes += chunk.len() as u64;
                if state.emitted_bytes > state.expected_bytes {
                    return Some((
                        Err(std::io::Error::other(
                            "Run document failed integrity verification.",
                        )),
                        None,
                    ));
                }
                state.hasher.update(&chunk);
                state.position += 1;
                return Some((Ok(axum::body::Bytes::from(chunk)), Some(state)));
            }
            if state.emitted_bytes != state.expected_bytes
                || format!("{:x}", state.hasher.finalize()) != state.expected_hash
            {
                return Some((
                    Err(std::io::Error::other(
                        "Run document failed integrity verification.",
                    )),
                    None,
                ));
            }
            Some((Ok(axum::body::Bytes::from_static(b"}\n")), None))
        },
    );
    Response::builder()
        .status(StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .header(axum::http::header::CONTENT_LENGTH, content_length)
        .body(axum::body::Body::from_stream(stream))
        .map_err(|_| ApiError::processing_error())
}

struct DetailStream {
    pool: sqlx::PgPool,
    owner: String,
    revision: String,
    prefix: Option<Vec<u8>>,
    position: u64,
    chunk_count: u64,
    expected_bytes: u64,
    expected_hash: String,
    emitted_bytes: u64,
    hasher: Sha256,
}
async fn delete(
    State(state): State<AppState>,
    AuthenticatedUser { user_id, .. }: AuthenticatedUser,
    Path(activity): Path<String>,
) -> Result<StatusCode, ApiError> {
    let activity = id(&activity)?;
    if !store::delete(&state.db, user_id, activity).await?
        && !legacy::delete(&state.db, user_id, activity).await?
    {
        return Err(ApiError::not_found());
    }
    Ok(StatusCode::NO_CONTENT)
}
async fn reprocess(
    State(state): State<AppState>,
    AuthenticatedUser { user_id, .. }: AuthenticatedUser,
    Path(activity): Path<String>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let activity = id(&activity)?;
    let detail = detail_value(&state.db, user_id, activity).await?;
    if detail["sourceUnavailable"] == true {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "SOURCE_UNAVAILABLE",
            "Original FIT is unavailable for this legacy activity.",
        ));
    }
    store::reprocess(&state.db, user_id, activity).await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({"activityId":activity,"status":"queued"})),
    ))
}
async fn latest(
    State(state): State<AppState>,
    AuthenticatedUser { user_id, .. }: AuthenticatedUser,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(history::latest(&state.db, user_id).await?))
}
async fn trend(
    State(state): State<AppState>,
    AuthenticatedUser { user_id, .. }: AuthenticatedUser,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(history::trend(&state.db, user_id).await?))
}
async fn evidence(
    State(state): State<AppState>,
    AuthenticatedUser { user_id, .. }: AuthenticatedUser,
    Path(activity): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mut detail = detail_value(&state.db, user_id, id(&activity)?).await?;
    Ok(Json(detail["historicalThresholds"].take()))
}
async fn create_export(
    State(state): State<AppState>,
    AuthenticatedUser { user_id, .. }: AuthenticatedUser,
    request: Result<Json<export::ExportRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let Json(request) = request.map_err(|_| {
        ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_EXPORT_REQUEST",
            "Provide explicit activity IDs, coach or full mode, and valid privacy options.",
        )
    })?;
    Ok((
        StatusCode::CREATED,
        Json(export::create(&state.db, user_id, request).await?),
    ))
}
async fn serve_export(
    State(state): State<AppState>,
    AuthenticatedUser { user_id, .. }: AuthenticatedUser,
    Path(token): Path<String>,
) -> Result<Response, ApiError> {
    export::serve(&state.db, user_id, &token).await
}
async fn serve_export_head(
    State(state): State<AppState>,
    AuthenticatedUser { user_id, .. }: AuthenticatedUser,
    Path(token): Path<String>,
) -> Result<Response, ApiError> {
    export::serve_head(&state.db, user_id, &token).await
}
