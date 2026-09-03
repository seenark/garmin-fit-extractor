use axum::{
    Json, Router,
    extract::{Path, State},
    http::Uri,
    routing::get,
};
use uuid::Uuid;

use crate::{
    app::AppState,
    auth::CoachAuthenticatedUser,
    db::{self, StoredActivity},
    error::ApiError,
    model::{
        Activity as CoachActivity, ActivitySummary, Analysis, CoachDerivedMetrics,
        CoachHeartRateZone, CoachLap,
    },
};

const DEFAULT_LIMIT: u32 = 10;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/activities/latest", get(latest_activity))
        .route("/api/v1/activities", get(list_activities))
        .route("/api/v1/activities/{activity_id}", get(get_activity))
}

async fn latest_activity(
    State(state): State<AppState>,
    CoachAuthenticatedUser { user_id }: CoachAuthenticatedUser,
    uri: Uri,
) -> Result<Json<CoachActivity>, ApiError> {
    let detail = parse_detail(uri.query())?;
    let stored = db::latest_activity(&state.db, user_id)
        .await
        .map_err(|_| ApiError::coach_processing_error())?
        .ok_or_else(ApiError::activity_not_found)?;
    let analysis = parse_analysis(&stored.activity_data)?;
    Ok(Json(to_activity(&stored, &analysis, detail)))
}

async fn list_activities(
    State(state): State<AppState>,
    CoachAuthenticatedUser { user_id }: CoachAuthenticatedUser,
    uri: Uri,
) -> Result<Json<Vec<ActivitySummary>>, ApiError> {
    let limit = parse_limit(uri.query())?;
    let stored = db::list_activities(&state.db, user_id, limit)
        .await
        .map_err(|_| ApiError::coach_processing_error())?;
    stored
        .iter()
        .map(|activity| {
            let analysis = parse_analysis(&activity.activity_data)?;
            Ok(to_summary(activity, &analysis))
        })
        .collect::<Result<Vec<_>, ApiError>>()
        .map(Json)
}

async fn get_activity(
    State(state): State<AppState>,
    CoachAuthenticatedUser { user_id }: CoachAuthenticatedUser,
    Path(activity_id): Path<String>,
    uri: Uri,
) -> Result<Json<CoachActivity>, ApiError> {
    let activity_id = Uuid::parse_str(&activity_id).map_err(|_| ApiError::activity_not_found())?;
    let detail = parse_detail(uri.query())?;
    let stored = db::get_activity(&state.db, user_id, activity_id)
        .await
        .map_err(|_| ApiError::coach_processing_error())?
        .ok_or_else(ApiError::activity_not_found)?;
    let analysis = parse_analysis(&stored.activity_data)?;
    Ok(Json(to_activity(&stored, &analysis, detail)))
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum ActivityDetail {
    Summary,
    Laps,
}

fn parse_detail(query: Option<&str>) -> Result<ActivityDetail, ApiError> {
    let Some(query) = query else {
        return Ok(ActivityDetail::Summary);
    };
    for pair in query.split('&') {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        if key != "detail" {
            continue;
        }
        return match value {
            "summary" => Ok(ActivityDetail::Summary),
            "laps" => Ok(ActivityDetail::Laps),
            _ => Err(ApiError::invalid_activity_detail()),
        };
    }
    Ok(ActivityDetail::Summary)
}

fn parse_limit(query: Option<&str>) -> Result<u32, ApiError> {
    let Some(query) = query else {
        return Ok(DEFAULT_LIMIT);
    };
    for pair in query.split('&') {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        if key != "limit" {
            continue;
        }
        let limit = value
            .parse::<u32>()
            .map_err(|_| ApiError::invalid_activity_limit())?;
        return (1..=20)
            .contains(&limit)
            .then_some(limit)
            .ok_or_else(ApiError::invalid_activity_limit);
    }
    Ok(DEFAULT_LIMIT)
}

fn parse_analysis(value: &str) -> Result<Analysis, ApiError> {
    serde_json::from_str(value).map_err(|error| {
        tracing::error!(%error, "stored coach activity JSON is invalid");
        ApiError::coach_processing_error()
    })
}

fn to_summary(stored: &StoredActivity, analysis: &Analysis) -> ActivitySummary {
    ActivitySummary {
        activity_id: stored.id,
        sport: stored.sport.clone(),
        started_at: stored.started_at.clone(),
        distance_m: finite(analysis.summary.distance.value),
        duration_s: rounded_seconds(analysis.summary.duration.value),
    }
}

fn to_activity(
    stored: &StoredActivity,
    analysis: &Analysis,
    detail: ActivityDetail,
) -> CoachActivity {
    let summary = to_summary(stored, analysis);
    let (laps, heart_rate_zones, derived_metrics) = match detail {
        ActivityDetail::Summary => (
            Vec::new(),
            Vec::new(),
            CoachDerivedMetrics {
                heart_rate_drift_percent: None,
            },
        ),
        ActivityDetail::Laps => (
            analysis.laps.iter().map(to_lap).collect(),
            analysis.heart_rate.zones.iter().map(to_zone).collect(),
            CoachDerivedMetrics {
                heart_rate_drift_percent: heart_rate_drift(analysis),
            },
        ),
    };
    CoachActivity {
        summary,
        average_heart_rate: analysis.heart_rate.average_bpm,
        maximum_heart_rate: analysis.heart_rate.maximum_bpm,
        average_pace_s_per_km: finite(analysis.pace.average.value),
        elevation_gain_m: finite(analysis.elevation.ascent.value),
        laps,
        heart_rate_zones,
        derived_metrics,
    }
}

fn to_lap(lap: &crate::model::Lap) -> CoachLap {
    CoachLap {
        index: lap.index,
        start_time: lap.start_time.clone(),
        distance_m: finite(lap.distance.value),
        duration_s: rounded_seconds(lap.duration.value),
        moving_time_s: rounded_seconds(lap.moving_time.value),
        pace_s_per_km: finite(lap.pace.value),
        average_heart_rate: lap.heart_rate.average_bpm,
        maximum_heart_rate: lap.heart_rate.maximum_bpm,
        average_power_w: lap.power.average_watts,
        maximum_power_w: lap.power.maximum_watts,
        average_cadence_spm: finite(lap.cadence.average_steps_per_minute),
        maximum_cadence_spm: finite(lap.cadence.maximum_steps_per_minute),
    }
}

fn to_zone(zone: &crate::model::HeartRateZone) -> CoachHeartRateZone {
    CoachHeartRateZone {
        zone: zone.zone,
        bucket_index: zone.bucket_index,
        label: zone.label.clone(),
        mapping_state: zone.mapping_state,
        zone_count: zone.zone_count,
        lower_bound_bpm: zone.lower_bound_bpm,
        upper_bound_bpm_exclusive: zone.upper_bound_bpm_exclusive,
        duration_s: finite(zone.duration_seconds),
    }
}

fn heart_rate_drift(analysis: &Analysis) -> Option<f64> {
    let midpoint = analysis.laps.len() / 2;
    if midpoint == 0 || midpoint == analysis.laps.len() {
        return None;
    }
    let average = |laps: &[crate::model::Lap]| {
        let values = laps
            .iter()
            .filter_map(|lap| lap.heart_rate.average_bpm)
            .filter(|value| *value > 0)
            .map(|value| value as f64)
            .collect::<Vec<_>>();
        (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
    };
    let first = average(&analysis.laps[..midpoint])?;
    let second = average(&analysis.laps[midpoint..])?;
    (first > 0.0).then(|| ((second - first) / first * 100.0 * 10.0).round() / 10.0)
}

fn finite(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite())
}

fn rounded_seconds(value: Option<f64>) -> Option<i64> {
    finite(value).map(|value| value.round() as i64)
}
