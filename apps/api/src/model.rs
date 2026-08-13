use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserProfile {
    pub id: Uuid,
    pub email: String,
    pub display_name: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CurrentUserResponse {
    pub user: Option<UserProfile>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metric {
    pub value: Option<f64>,
    pub unit: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub file_name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedActivity {
    pub r#type: Option<String>,
    pub sub_type: Option<String>,
    pub date: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Calories {
    pub value: Option<i64>,
    pub unit: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub duration: Metric,
    pub moving_time: Metric,
    pub distance: Metric,
    pub calories: Calories,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeartRateZone {
    pub zone: u32,
    pub min_bpm: Option<i64>,
    pub max_bpm: Option<i64>,
    pub duration_seconds: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeartRate {
    pub average_bpm: Option<i64>,
    pub maximum_bpm: Option<i64>,
    pub zones: Vec<HeartRateZone>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pace {
    pub average: Metric,
    pub moving: Metric,
    pub best: Metric,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Power {
    pub average_watts: Option<i64>,
    pub maximum_watts: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cadence {
    pub average_steps_per_minute: Option<f64>,
    pub maximum_steps_per_minute: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningDynamics {
    pub cadence: Cadence,
    pub stride_length: Metric,
    pub ground_contact_time: Metric,
    pub vertical_oscillation: Metric,
    pub vertical_ratio: Metric,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Elevation {
    pub ascent: Metric,
    pub descent: Metric,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Temperature {
    pub average_celsius: Option<f64>,
    pub minimum_celsius: Option<f64>,
    pub maximum_celsius: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LapHeartRate {
    pub average_bpm: Option<i64>,
    pub maximum_bpm: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lap {
    pub index: u32,
    pub start_time: Option<String>,
    pub distance: Metric,
    pub duration: Metric,
    pub moving_time: Metric,
    pub pace: Metric,
    pub heart_rate: LapHeartRate,
    pub power: Power,
    pub cadence: Cadence,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Analysis {
    pub schema_version: String,
    pub source: Source,
    pub activity: NormalizedActivity,
    pub summary: Summary,
    pub heart_rate: HeartRate,
    pub pace: Pace,
    pub power: Power,
    pub running_dynamics: RunningDynamics,
    pub elevation: Elevation,
    pub temperature: Temperature,
    pub laps: Vec<Lap>,
}

impl Analysis {
    pub const SCHEMA_VERSION: &'static str = "1.0.0";

    pub fn empty(file_name: impl Into<String>) -> Self {
        Self {
            schema_version: Self::SCHEMA_VERSION.into(),
            source: Source {
                file_name: file_name.into(),
            },
            activity: NormalizedActivity {
                r#type: None,
                sub_type: None,
                date: None,
            },
            summary: Summary {
                duration: metric(None, "seconds"),
                moving_time: metric(None, "seconds"),
                distance: metric(None, "meters"),
                calories: Calories {
                    value: None,
                    unit: "kcal".into(),
                },
            },
            heart_rate: HeartRate {
                average_bpm: None,
                maximum_bpm: None,
                zones: Vec::new(),
            },
            pace: Pace {
                average: metric(None, "seconds_per_kilometer"),
                moving: metric(None, "seconds_per_kilometer"),
                best: metric(None, "seconds_per_kilometer"),
            },
            power: Power {
                average_watts: None,
                maximum_watts: None,
            },
            running_dynamics: RunningDynamics {
                cadence: Cadence {
                    average_steps_per_minute: None,
                    maximum_steps_per_minute: None,
                },
                stride_length: metric(None, "meters"),
                ground_contact_time: metric(None, "milliseconds"),
                vertical_oscillation: metric(None, "millimeters"),
                vertical_ratio: metric(None, "percent"),
            },
            elevation: Elevation {
                ascent: metric(None, "meters"),
                descent: metric(None, "meters"),
            },
            temperature: Temperature {
                average_celsius: None,
                minimum_celsius: None,
                maximum_celsius: None,
            },
            laps: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema_version != Self::SCHEMA_VERSION {
            return Err("schemaVersion must be 1.0.0");
        }
        if self.summary.duration.unit != "seconds"
            || self.summary.moving_time.unit != "seconds"
            || self.summary.distance.unit != "meters"
            || self.summary.calories.unit != "kcal"
            || self.pace.average.unit != "seconds_per_kilometer"
            || self.pace.moving.unit != "seconds_per_kilometer"
            || self.pace.best.unit != "seconds_per_kilometer"
            || self.running_dynamics.stride_length.unit != "meters"
            || self.running_dynamics.ground_contact_time.unit != "milliseconds"
            || self.running_dynamics.vertical_oscillation.unit != "millimeters"
            || self.running_dynamics.vertical_ratio.unit != "percent"
            || self.elevation.ascent.unit != "meters"
            || self.elevation.descent.unit != "meters"
            || self.laps.iter().any(|lap| {
                lap.distance.unit != "meters"
                    || lap.duration.unit != "seconds"
                    || lap.moving_time.unit != "seconds"
                    || lap.pace.unit != "seconds_per_kilometer"
            })
        {
            return Err("analysis contains an invalid unit");
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for Analysis {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Wire {
            schema_version: String,
            source: Source,
            activity: NormalizedActivity,
            summary: Summary,
            heart_rate: HeartRate,
            pace: Pace,
            power: Power,
            running_dynamics: RunningDynamics,
            elevation: Elevation,
            temperature: Temperature,
            laps: Vec<Lap>,
        }

        let wire = Wire::deserialize(deserializer)?;
        let analysis = Analysis {
            schema_version: wire.schema_version,
            source: wire.source,
            activity: wire.activity,
            summary: wire.summary,
            heart_rate: wire.heart_rate,
            pace: wire.pace,
            power: wire.power,
            running_dynamics: wire.running_dynamics,
            elevation: wire.elevation,
            temperature: wire.temperature,
            laps: wire.laps,
        };
        analysis.validate().map_err(serde::de::Error::custom)?;
        Ok(analysis)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ActivitySummary {
    pub activity_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sport: Option<String>,
    pub started_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance_m: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_s: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Activity {
    #[serde(flatten)]
    pub summary: ActivitySummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub average_heart_rate: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum_heart_rate: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub average_pace_s_per_km: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elevation_gain_m: Option<f64>,
    pub laps: Vec<CoachLap>,
    pub heart_rate_zones: Vec<CoachHeartRateZone>,
    pub derived_metrics: CoachDerivedMetrics,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CoachLap {
    pub index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance_m: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_s: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moving_time_s: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pace_s_per_km: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub average_heart_rate: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum_heart_rate: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub average_power_w: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum_power_w: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub average_cadence_spm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum_cadence_spm: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CoachHeartRateZone {
    pub zone: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_bpm: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_bpm: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_s: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CoachDerivedMetrics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heart_rate_drift_percent: Option<f64>,
}

fn metric(value: Option<f64>, unit: &str) -> Metric {
    Metric {
        value,
        unit: unit.into(),
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawFitRecord {
    pub kind: String,
    pub fields: Vec<RawFitField>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawFitField {
    pub name: String,
    pub value: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub units: Option<String>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExtractionStatus {
    Succeeded,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiErrorBody {
    pub error: ApiErrorDetail,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiErrorDetail {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractionSummary {
    pub id: Uuid,
    pub file_name: String,
    pub file_size_bytes: u64,
    pub status: ExtractionStatus,
    pub activity_type: Option<String>,
    pub activity_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiErrorDetail>,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchCreateResponse {
    pub items: Vec<ExtractionSummary>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractionPage {
    pub items: Vec<ExtractionSummary>,
    pub total: u64,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractionDetail {
    #[serde(flatten)]
    pub summary: ExtractionSummary,
    pub normalized: Option<Analysis>,
    pub raw: Option<Vec<RawFitRecord>>,
}
