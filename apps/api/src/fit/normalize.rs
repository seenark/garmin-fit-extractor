use chrono::{DateTime, SecondsFormat};
use serde_json::Value;

use crate::model::RawFitRecord;
use crate::model::{
    ActivitySample, Analysis, Cadence, Calories, Elevation, HeartRate, HeartRateZone, Lap,
    LapHeartRate, LapPower, Metric, NormalizedActivity, Pace, Power, PowerZone, RunningDynamics,
    Summary, Temperature,
};

pub fn normalize(records: &[RawFitRecord], file_name: &str) -> Analysis {
    let session = first_record(records, "session");
    let activity = first_record(records, "activity");
    let laps = matching_records(records, "lap");
    let samples = matching_records(records, "record");
    let zones = matching_records(records, "hrzone");
    let power_zone_messages = matching_records(records, "powerzone");
    let zones_target = first_record(records, "zonestarget");

    let distance = session.number(&["totalDistance"]);
    let duration = session.number(&["totalElapsedTime"]);
    let moving_time = session.number(&["totalTimerTime"]);
    let average_speed = session.number(&["enhancedAvgSpeed", "avgSpeed"]);
    let maximum_speed = session.number(&["enhancedMaxSpeed", "maxSpeed"]);

    let average_temperature = session
        .number(&["avgTemperature"])
        .or_else(|| average_record_value(&samples, &["temperature"]));
    let minimum_temperature = session
        .number(&["minTemperature"])
        .or_else(|| min_record_value(&samples, &["temperature"]));
    let maximum_temperature = session
        .number(&["maxTemperature"])
        .or_else(|| max_record_value(&samples, &["temperature"]));

    let average_cadence = normalize_cadence(session.number(&["avgRunningCadence", "avgCadence"]));
    let maximum_cadence = normalize_cadence(session.number(&["maxRunningCadence", "maxCadence"]));
    let normalized_power_zones = power_zones(session, zones_target, &power_zone_messages, &laps);
    let normalized_samples = activity_samples(session, &samples);
    let mut analysis = Analysis {
        schema_version: Analysis::SCHEMA_VERSION.into(),
        source: crate::model::Source {
            file_name: file_name.into(),
        },
        activity: NormalizedActivity {
            r#type: session
                .string(&["sport"])
                .or_else(|| activity.string(&["type"])),
            sub_type: session.string(&["subSport"]),
            date: date_or_null(
                session
                    .value(&["startTime"])
                    .or_else(|| activity.value(&["timestamp"]))
                    .or_else(|| session.value(&["timestamp"])),
            ),
        },
        summary: Summary {
            duration: metric(round(duration, 2), "seconds"),
            moving_time: metric(round(moving_time, 2), "seconds"),
            distance: metric(round(distance, 2), "meters"),
            calories: Calories {
                value: integer(session.number(&["totalCalories"])),
                unit: "kcal".into(),
            },
        },
        heart_rate: HeartRate {
            average_bpm: integer(session.number(&["avgHeartRate"])),
            maximum_bpm: integer(session.number(&["maxHeartRate"])),
            zones: heart_rate_zones(session, zones),
        },
        pace: Pace {
            average: metric(
                speed_to_pace(average_speed).or_else(|| derive_pace(distance, duration)),
                "seconds_per_kilometer",
            ),
            moving: metric(derive_pace(distance, moving_time), "seconds_per_kilometer"),
            best: metric(speed_to_pace(maximum_speed), "seconds_per_kilometer"),
        },
        power: Power {
            average_watts: integer(session.number(&["avgPower"])),
            maximum_watts: integer(session.number(&["maxPower"])),
            zones: normalized_power_zones,
        },
        running_dynamics: RunningDynamics {
            cadence: Cadence {
                average_steps_per_minute: round(average_cadence, 2),
                maximum_steps_per_minute: round(maximum_cadence, 2),
            },
            stride_length: metric(round(session.number(&["avgStrideLength"]), 3), "meters"),
            ground_contact_time: metric(
                round(
                    session.number(&["avgStanceTime", "avgGroundContactTime"]),
                    2,
                ),
                "milliseconds",
            ),
            vertical_oscillation: metric(
                round(session.number(&["avgVerticalOscillation"]), 2),
                "millimeters",
            ),
            vertical_ratio: metric(round(session.number(&["avgVerticalRatio"]), 2), "percent"),
        },
        elevation: Elevation {
            ascent: metric(round(session.number(&["totalAscent"]), 2), "meters"),
            descent: metric(round(session.number(&["totalDescent"]), 2), "meters"),
        },
        temperature: Temperature {
            average_celsius: round(average_temperature, 2),
            minimum_celsius: round(minimum_temperature, 2),
            maximum_celsius: round(maximum_temperature, 2),
        },
        samples: normalized_samples,
        laps: Vec::new(),
    };

    analysis.laps = laps
        .into_iter()
        .enumerate()
        .map(|(index, lap)| normalize_lap(lap, index as u32 + 1))
        .collect();
    analysis
}

#[derive(Copy, Clone)]
struct Record<'a> {
    record: Option<&'a RawFitRecord>,
}

impl<'a> Record<'a> {
    const EMPTY: Self = Self { record: None };

    fn value(self, aliases: &[&str]) -> Option<&'a Value> {
        let record = self.record?;
        for alias in aliases {
            let alias = canonical(alias);
            if let Some(field) = record
                .fields
                .iter()
                .find(|field| canonical(&field.name) == alias)
            {
                return Some(&field.value);
            }
        }
        None
    }

    fn number(self, aliases: &[&str]) -> Option<f64> {
        let record = self.record?;
        aliases.iter().find_map(|alias| {
            let alias = canonical(alias);
            record
                .fields
                .iter()
                .find(|field| canonical(&field.name) == alias)
                .and_then(|field| field.value.as_f64())
                .filter(|value| value.is_finite())
        })
    }

    fn string(self, aliases: &[&str]) -> Option<String> {
        let record = self.record?;
        aliases.iter().find_map(|alias| {
            let alias = canonical(alias);
            record
                .fields
                .iter()
                .find(|field| canonical(&field.name) == alias)
                .and_then(|field| field.value.as_str())
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
        })
    }

    fn numbers(self, aliases: &[&str]) -> Vec<Option<f64>> {
        self.value(aliases)
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .map(|value| value.as_f64().filter(|value| value.is_finite()))
                    .collect()
            })
            .unwrap_or_default()
    }
}

fn first_record<'a>(records: &'a [RawFitRecord], kind: &str) -> Record<'a> {
    Record {
        record: records
            .iter()
            .find(|record| canonical(&record.kind) == canonical(kind)),
    }
}

fn matching_records<'a>(records: &'a [RawFitRecord], kind: &str) -> Vec<Record<'a>> {
    records
        .iter()
        .filter(|record| canonical(&record.kind) == canonical(kind))
        .map(|record| Record {
            record: Some(record),
        })
        .collect()
}

fn canonical(value: &str) -> String {
    value
        .chars()
        .filter(|character| !matches!(character, '_' | '-' | ' '))
        .flat_map(char::to_lowercase)
        .collect()
}

fn metric(value: Option<f64>, unit: &str) -> Metric {
    Metric {
        value,
        unit: unit.into(),
    }
}

fn round(value: Option<f64>, digits: u32) -> Option<f64> {
    value.filter(|value| value.is_finite()).map(|value| {
        let factor = 10_f64.powi(digits as i32);
        (value * factor + 0.5).floor() / factor
    })
}

fn integer(value: Option<f64>) -> Option<i64> {
    value
        .filter(|value| value.is_finite() && *value >= i64::MIN as f64 && *value <= i64::MAX as f64)
        .map(|value| (value + 0.5).floor() as i64)
}

fn speed_to_pace(speed: Option<f64>) -> Option<f64> {
    speed
        .filter(|speed| *speed > 0.0)
        .and_then(|speed| round(Some(1000.0 / speed), 2))
}

fn derive_pace(distance: Option<f64>, seconds: Option<f64>) -> Option<f64> {
    match (distance, seconds) {
        (Some(distance), Some(seconds)) if distance > 0.0 && seconds > 0.0 => {
            round(Some(seconds / distance * 1000.0), 2)
        }
        _ => None,
    }
}

fn normalize_cadence(value: Option<f64>) -> Option<f64> {
    value.map(|value| if value < 130.0 { value * 2.0 } else { value })
}

fn average_record_value(records: &[Record<'_>], aliases: &[&str]) -> Option<f64> {
    let values = records
        .iter()
        .filter_map(|record| record.number(aliases))
        .collect::<Vec<_>>();
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

fn min_record_value(records: &[Record<'_>], aliases: &[&str]) -> Option<f64> {
    records
        .iter()
        .filter_map(|record| record.number(aliases))
        .reduce(f64::min)
}

fn max_record_value(records: &[Record<'_>], aliases: &[&str]) -> Option<f64> {
    records
        .iter()
        .filter_map(|record| record.number(aliases))
        .reduce(f64::max)
}

fn power_zones(
    session: Record<'_>,
    zones_target: Record<'_>,
    zone_messages: &[Record<'_>],
    laps: &[Record<'_>],
) -> Vec<PowerZone> {
    let mut durations = session.numbers(&["timeInPowerZone"]);
    if durations.is_empty() {
        durations = aggregate_zone_durations(laps, &["timeInPowerZone"]);
    }

    let mut sorted_zones = zone_messages.to_vec();
    sorted_zones.sort_by(|left, right| {
        left.number(&["messageIndex"])
            .unwrap_or(0.0)
            .total_cmp(&right.number(&["messageIndex"]).unwrap_or(0.0))
    });

    let mut boundaries: Vec<Option<i64>> = sorted_zones
        .iter()
        .map(|zone| {
            zone.number(&["highValue"])
                .and_then(|value| integer(Some(value)))
        })
        .collect();
    if boundaries.is_empty() {
        boundaries = zones_target
            .numbers(&["powerZoneHighBoundary"])
            .into_iter()
            .map(|value| value.and_then(|value| integer(Some(value))))
            .collect();
    }

    let count = durations.len().max(boundaries.len());
    (0..count)
        .map(|index| PowerZone {
            zone: index as u32 + 1,
            min_watts: if index == 0 {
                Some(0)
            } else {
                boundaries
                    .get(index - 1)
                    .copied()
                    .flatten()
                    .and_then(|value| value.checked_add(1))
            },
            max_watts: boundaries.get(index).copied().flatten(),
            duration_seconds: round(durations.get(index).copied().flatten(), 2),
        })
        .collect()
}

fn aggregate_zone_durations(records: &[Record<'_>], aliases: &[&str]) -> Vec<Option<f64>> {
    let count = records
        .iter()
        .map(|record| record.numbers(aliases).len())
        .max()
        .unwrap_or(0);
    (0..count)
        .map(|index| {
            let values = records
                .iter()
                .filter_map(|record| record.numbers(aliases).get(index).copied().flatten())
                .collect::<Vec<_>>();
            (!values.is_empty()).then(|| values.iter().sum())
        })
        .collect()
}

fn activity_samples(session: Record<'_>, records: &[Record<'_>]) -> Vec<ActivitySample> {
    let origin = session
        .value(&["startTime", "timestamp"])
        .and_then(|value| parse_date(Some(value)))
        .or_else(|| {
            records
                .iter()
                .find_map(|record| parse_date(record.value(&["timestamp"])))
        });

    records
        .iter()
        .enumerate()
        .filter_map(|(index, record)| {
            let sample_date = parse_date(record.value(&["timestamp"]));
            let timestamp = sample_date.map(format_date);
            let elapsed_seconds = sample_date
                .as_ref()
                .zip(origin.as_ref())
                .map(|(time, start)| {
                    (*time).signed_duration_since(*start).num_milliseconds() as f64 / 1000.0
                })
                .filter(|value| value.is_finite() && *value >= 0.0);
            let heart_rate_bpm = integer(record.number(&["heartRate", "hr"]));
            let power_watts = integer(record.number(&["power", "powerWatts"]));

            (heart_rate_bpm.is_some() || power_watts.is_some()).then(|| ActivitySample {
                index: index as u32,
                timestamp,
                elapsed_seconds: round(elapsed_seconds, 3),
                heart_rate_bpm,
                power_watts,
            })
        })
        .collect()
}

fn heart_rate_zones(session: Record<'_>, mut zones: Vec<Record<'_>>) -> Vec<HeartRateZone> {
    let durations: Vec<Option<f64>> = session
        .value(&["timeInHrZone"])
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .map(|value| Value::as_f64(value).filter(|value| value.is_finite()))
                .collect()
        })
        .unwrap_or_default();
    zones.sort_by(|left, right| {
        left.number(&["messageIndex"])
            .unwrap_or(0.0)
            .total_cmp(&right.number(&["messageIndex"]).unwrap_or(0.0))
    });

    let count = durations.len().max(zones.len());
    (0..count)
        .map(|index| {
            let zone = zones.get(index).copied().unwrap_or(Record::EMPTY);
            let next_minimum = zones
                .get(index + 1)
                .copied()
                .unwrap_or(Record::EMPTY)
                .number(&["lowBpm", "minHeartRate"])
                .and_then(|value| integer(Some(value)));
            let direct_maximum = zone
                .number(&["highBpm", "maxHeartRate"])
                .and_then(|value| integer(Some(value)));

            HeartRateZone {
                zone: index as u32 + 1,
                min_bpm: zone
                    .number(&["lowBpm", "minHeartRate"])
                    .and_then(|value| integer(Some(value))),
                max_bpm: direct_maximum.or_else(|| next_minimum.map(|value| value - 1)),
                duration_seconds: round(durations.get(index).copied().flatten(), 2),
            }
        })
        .collect()
}

fn normalize_lap(lap: Record<'_>, index: u32) -> Lap {
    let distance = lap.number(&["totalDistance"]);
    let duration = lap.number(&["totalElapsedTime"]);
    let moving_time = lap.number(&["totalTimerTime"]);
    let speed = lap.number(&["enhancedAvgSpeed", "avgSpeed"]);

    Lap {
        index,
        start_time: date_or_null(lap.value(&["startTime", "timestamp"])),
        distance: metric(round(distance, 2), "meters"),
        duration: metric(round(duration, 2), "seconds"),
        moving_time: metric(round(moving_time, 2), "seconds"),
        pace: metric(
            speed_to_pace(speed).or_else(|| {
                derive_pace(
                    distance,
                    moving_time.filter(|seconds| *seconds > 0.0).or(duration),
                )
            }),
            "seconds_per_kilometer",
        ),
        heart_rate: LapHeartRate {
            average_bpm: integer(lap.number(&["avgHeartRate"])),
            maximum_bpm: integer(lap.number(&["maxHeartRate"])),
        },
        power: LapPower {
            average_watts: integer(lap.number(&["avgPower"])),
            maximum_watts: integer(lap.number(&["maxPower"])),
        },
        cadence: Cadence {
            average_steps_per_minute: round(
                normalize_cadence(lap.number(&["avgRunningCadence", "avgCadence"])),
                2,
            ),
            maximum_steps_per_minute: round(
                normalize_cadence(lap.number(&["maxRunningCadence", "maxCadence"])),
                2,
            ),
        },
    }
}

fn date_or_null(value: Option<&Value>) -> Option<String> {
    parse_date(value).map(format_date)
}

fn parse_date(value: Option<&Value>) -> Option<DateTime<chrono::FixedOffset>> {
    DateTime::parse_from_rfc3339(value?.as_str()?).ok()
}

fn format_date(value: DateTime<chrono::FixedOffset>) -> String {
    value.to_utc().to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use crate::model::{RawFitField, RawFitRecord};

    use super::normalize;

    fn record<'a>(kind: &str, fields: impl IntoIterator<Item = (&'a str, Value)>) -> RawFitRecord {
        RawFitRecord {
            kind: kind.into(),
            fields: fields
                .into_iter()
                .map(|(name, value)| RawFitField {
                    name: name.into(),
                    value,
                    units: None,
                })
                .collect(),
        }
    }

    #[test]
    fn normalization_canonicalizes_kinds_aliases_and_duplicate_fields() {
        let records = vec![
            record(
                "SESSION",
                [
                    ("sport", json!("running")),
                    ("sub_sport", json!("road")),
                    ("start time", json!("2026-07-19T00:00:00.000Z")),
                    ("total-distance", json!(10_000)),
                    ("total_elapsed_time", json!(3600)),
                    ("total_timer_time", json!(3540)),
                    ("total_calories", json!(700)),
                    ("avg_heart_rate", json!(150)),
                    ("max_heart_rate", json!(180)),
                    ("enhanced_avg_speed", json!(2.824858757)),
                    ("avgSpeed", json!(99)),
                    ("enhancedMaxSpeed", json!(4)),
                    ("max_speed", json!(99)),
                    ("avg_power", json!(250)),
                    ("maxPower", json!(410)),
                    ("avg_running_cadence", json!(85)),
                    ("avgCadence", json!(100)),
                    ("max-running-cadence", json!(95)),
                    ("maxCadence", json!(100)),
                    ("avg_stride_length", json!(1.23456)),
                    ("avg_stance_time", json!(201.234)),
                    ("avgGroundContactTime", json!(999)),
                    ("avg_vertical_oscillation", json!(89.123)),
                    ("avg_vertical_ratio", json!(7.891)),
                    ("total_ascent", json!(40)),
                    ("total_descent", json!(38)),
                    ("time_in_hr_zone", json!([300, 900])),
                ],
            ),
            record(
                "activity",
                [
                    ("type", json!("ignored-fallback")),
                    ("timestamp", json!("2026-07-20T00:00:00Z")),
                ],
            ),
            record(
                "hr_zone",
                [("message_index", json!(1)), ("low_bpm", json!(120))],
            ),
            record(
                "HR-ZONE",
                [("messageIndex", json!(0)), ("min_heart_rate", json!(100))],
            ),
            record(
                "lap",
                [
                    ("start_time", json!("2026-07-19T00:00:00Z")),
                    ("total_distance", json!(1000)),
                    ("total_elapsed_time", json!(360)),
                    ("total_timer_time", json!(350)),
                    ("avg_heart_rate", json!(145)),
                    ("max_heart_rate", json!(155)),
                    ("avg_power", json!(245)),
                    ("max_power", json!(300)),
                    ("avg_running_cadence", json!(84)),
                    ("max_running_cadence", json!(86)),
                ],
            ),
        ];

        let result = normalize(&records, "activity.fit");

        assert_eq!(result.schema_version, "1.0.0");
        assert_eq!(result.activity.r#type.as_deref(), Some("running"));
        assert_eq!(result.activity.sub_type.as_deref(), Some("road"));
        assert_eq!(
            result.activity.date.as_deref(),
            Some("2026-07-19T00:00:00.000Z")
        );
        assert_eq!(result.summary.distance.value, Some(10_000.0));
        assert_eq!(result.pace.average.value, Some(354.0));
        assert_eq!(result.pace.best.value, Some(250.0));
        assert_eq!(
            result.running_dynamics.cadence.average_steps_per_minute,
            Some(170.0)
        );
        assert_eq!(
            result.running_dynamics.cadence.maximum_steps_per_minute,
            Some(190.0)
        );
        assert_eq!(result.running_dynamics.stride_length.value, Some(1.235));
        assert_eq!(
            result.running_dynamics.ground_contact_time.value,
            Some(201.23)
        );
        assert_eq!(
            result.running_dynamics.vertical_oscillation.value,
            Some(89.12)
        );
        assert_eq!(result.running_dynamics.vertical_ratio.value, Some(7.89));
        assert_eq!(result.heart_rate.zones[0].min_bpm, Some(100));
        assert_eq!(result.heart_rate.zones[0].max_bpm, Some(119));
        assert_eq!(result.heart_rate.zones[1].duration_seconds, Some(900.0));
        assert_eq!(result.laps[0].index, 1);
        assert_eq!(result.laps[0].pace.value, Some(350.0));
        assert_eq!(result.laps[0].cadence.average_steps_per_minute, Some(168.0));
        assert_eq!(result.laps[0].cadence.maximum_steps_per_minute, Some(172.0));
    }

    #[test]
    fn normalization_uses_activity_then_session_timestamp_and_record_temperatures() {
        let activity_date = normalize(
            &[
                record(
                    "session",
                    [("timestamp", json!("2026-07-19T03:00:00+03:00"))],
                ),
                record("activity", [("timestamp", json!("2026-07-18T00:00:00Z"))]),
                record("record", [("temperature", json!(10.123))]),
                record("RECORD", [("temperature", json!(20.456))]),
            ],
            "activity.fit",
        );
        assert_eq!(
            activity_date.activity.date.as_deref(),
            Some("2026-07-18T00:00:00.000Z")
        );
        assert_eq!(activity_date.temperature.average_celsius, Some(15.29));
        assert_eq!(activity_date.temperature.minimum_celsius, Some(10.12));
        assert_eq!(activity_date.temperature.maximum_celsius, Some(20.46));

        let fall_back = normalize(
            &[record(
                "activity",
                [("timestamp", json!("2026-07-18T00:00:00Z"))],
            )],
            "activity.fit",
        );
        assert_eq!(
            fall_back.activity.date.as_deref(),
            Some("2026-07-18T00:00:00.000Z")
        );

        let missing = normalize(&[], "activity.fit");
        assert!(missing.activity.date.is_none());
        assert!(missing.summary.distance.value.is_none());
        assert!(missing.pace.average.value.is_none());
    }

    #[test]
    fn normalization_handles_cadence_boundaries_pace_fallbacks_and_mismatched_zones() {
        let result = normalize(
            &[
                record(
                    "session",
                    [
                        ("start_time", json!("2026-07-19T00:00:00Z")),
                        ("total_distance", json!(0)),
                        ("total_elapsed_time", json!(600)),
                        ("total_timer_time", json!(0)),
                        ("avg_speed", json!(0)),
                        ("avg_cadence", json!(129)),
                        ("max_cadence", json!(130)),
                        ("time_in_hr_zone", json!([10, 20, 30])),
                        ("time_in_power_zone", json!([5, 15, 25, 35, 45, 55, 65])),
                    ],
                ),
                record(
                    "power_zone",
                    [("message_index", json!(0)), ("high_value", json!(150))],
                ),
                record(
                    "power_zone",
                    [("message_index", json!(1)), ("high_value", json!(220))],
                ),
                record(
                    "power_zone",
                    [("message_index", json!(2)), ("high_value", json!(280))],
                ),
                record(
                    "power_zone",
                    [("message_index", json!(3)), ("high_value", json!(340))],
                ),
                record(
                    "power_zone",
                    [("message_index", json!(4)), ("high_value", json!(410))],
                ),
                record(
                    "power_zone",
                    [("message_index", json!(5)), ("high_value", json!(500))],
                ),
                record(
                    "record",
                    [
                        ("timestamp", json!("2026-07-19T00:00:00Z")),
                        ("heart_rate", json!(140)),
                        ("power", json!(0)),
                    ],
                ),
                record(
                    "record",
                    [
                        ("timestamp", json!("2026-07-19T00:00:01Z")),
                        ("heart_rate", json!(145)),
                        ("power", json!(245)),
                    ],
                ),
                record("record", [("timestamp", json!("2026-07-19T00:00:02Z"))]),
                record(
                    "hrzone",
                    [("message_index", json!(0)), ("low_bpm", json!(100))],
                ),
                record(
                    "lap",
                    [
                        ("total_distance", json!(1000)),
                        ("total_elapsed_time", json!(360)),
                        ("avg_speed", json!(0)),
                    ],
                ),
            ],
            "activity.fit",
        );

        assert_eq!(
            result.running_dynamics.cadence.average_steps_per_minute,
            Some(258.0)
        );
        assert_eq!(
            result.running_dynamics.cadence.maximum_steps_per_minute,
            Some(130.0)
        );
        assert!(result.pace.average.value.is_none());
        assert!(result.pace.moving.value.is_none());
        assert_eq!(result.heart_rate.zones.len(), 3);
        assert_eq!(result.heart_rate.zones[0].duration_seconds, Some(10.0));
        assert_eq!(result.heart_rate.zones[1].min_bpm, None);
        assert_eq!(result.power.zones.len(), 7);
        assert_eq!(result.power.zones[0].min_watts, Some(0));
        assert_eq!(result.power.zones[0].max_watts, Some(150));
        assert_eq!(result.power.zones[6].min_watts, Some(501));
        assert_eq!(result.power.zones[6].max_watts, None);
        assert_eq!(result.power.zones[3].duration_seconds, Some(35.0));
        assert_eq!(result.samples.len(), 2);
        assert_eq!(result.samples[0].elapsed_seconds, Some(0.0));
        assert_eq!(result.samples[0].power_watts, Some(0));
        assert_eq!(result.samples[1].elapsed_seconds, Some(1.0));
        assert_eq!(result.samples[1].heart_rate_bpm, Some(145));
        assert_eq!(result.samples[1].power_watts, Some(245));
        assert_eq!(result.laps[0].pace.value, Some(360.0));
    }

    #[test]
    fn invalid_heart_rate_zone_durations_keep_their_original_indexes() {
        let result = normalize(
            &[record(
                "session",
                [("time_in_hr_zone", json!([10, "invalid", 30]))],
            )],
            "activity.fit",
        );

        assert_eq!(result.heart_rate.zones.len(), 3);
        assert_eq!(result.heart_rate.zones[0].duration_seconds, Some(10.0));
        assert_eq!(result.heart_rate.zones[1].duration_seconds, None);
        assert_eq!(result.heart_rate.zones[2].duration_seconds, Some(30.0));
    }

    #[test]
    fn session_temperature_overrides_record_aggregates_independently() {
        let result = normalize(
            &[
                record(
                    "session",
                    [
                        ("avg_temperature", json!(15)),
                        ("min_temperature", json!(8)),
                    ],
                ),
                record("record", [("temperature", json!(10))]),
                record("record", [("temperature", json!(20))]),
            ],
            "activity.fit",
        );

        assert_eq!(result.temperature.average_celsius, Some(15.0));
        assert_eq!(result.temperature.minimum_celsius, Some(8.0));
        assert_eq!(result.temperature.maximum_celsius, Some(20.0));
    }

    #[test]
    fn invalid_primary_aliases_fall_back_to_the_next_alias() {
        let result = normalize(
            &[record(
                "session",
                [
                    ("enhanced_avg_speed", json!("invalid")),
                    ("avg_speed", json!(4)),
                    ("avg_running_cadence", json!("invalid")),
                    ("avg_cadence", json!(85)),
                    ("avg_stance_time", json!("invalid")),
                    ("avg_ground_contact_time", json!(200)),
                ],
            )],
            "activity.fit",
        );

        assert_eq!(result.pace.average.value, Some(250.0));
        assert_eq!(
            result.running_dynamics.cadence.average_steps_per_minute,
            Some(170.0)
        );
        assert_eq!(
            result.running_dynamics.ground_contact_time.value,
            Some(200.0)
        );
    }
}
