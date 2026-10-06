use super::stream::SourceProof;
use crate::error::ApiError;
use crate::fit::raw::{enum_profile, native_field_valid, numeric_value};
use axum::http::StatusCode;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

pub const POLICY_VERSION: &str = "native-source-proof-v1";

#[derive(Clone, Copy, Debug, Default)]
pub struct Policy {
    pub include_location: bool,
    pub include_device_identifiers: bool,
}

/// Project snapshots against the same decoded source used by streaming exports.
pub fn project(activity: &Value, policy: Policy) -> Result<(Value, Value), ApiError> {
    let mut proof = SourceProof::default();
    let source = if let Some(messages) = activity["decoded"]["messages"].as_array() {
        proof
            .decoder(&activity["decoded"]["decoder"])
            .map_err(|_| source_unprovable())?;
        for message in messages {
            proof.message(message).map_err(|_| source_unprovable())?;
        }
        if let Some(samples) = activity["normalized"]["samples"].as_array() {
            for sample in samples {
                proof
                    .observe_sample(sample)
                    .map_err(|_| source_unprovable())?;
            }
        }
        Some(&proof)
    } else {
        None
    };
    let mut omissions = BTreeMap::new();
    let scope = Scope::new(source);
    let value = object_with_references(
        activity,
        "activity",
        "activities.*",
        policy,
        &mut omissions,
        scope,
    );
    if required_missing(activity, &value, "activity") {
        return Err(source_unprovable());
    }
    Ok((value, omission_rows(omissions)))
}

type Omissions = BTreeMap<&'static str, BTreeMap<String, u64>>;
fn omit(omissions: &mut Omissions, category: &'static str, path: &str) {
    let counts = omissions.entry(category).or_default();
    if let Some(count) = counts.get_mut(path) {
        *count += 1;
    } else {
        counts.insert(path.to_owned(), 1);
    }
}

fn object(
    value: &Value,
    schema: &str,
    path: &str,
    policy: Policy,
    omissions: &mut Omissions,
) -> Value {
    object_with_references(value, schema, path, policy, omissions, Scope::new(None))
}

fn object_with_references<'a>(
    value: &'a Value,
    schema: &str,
    path: &str,
    policy: Policy,
    omissions: &mut Omissions,
    mut scope: Scope<'a>,
) -> Value {
    if value.is_null() {
        return Value::Null;
    }
    let Some(input) = value.as_object() else {
        omit(omissions, "unclassified", path);
        return Value::Null;
    };
    if schema == "lap" {
        scope.window = source_window(value, scope.source);
    }
    if schema == "interval" {
        scope.rr_safe = Some(
            scope
                .source
                .is_some_and(|source| source.rr_interval_safe(value)),
        );
    }
    if schema == "summary" {
        scope.summary = Some(value);
        scope.summary_kind = SummaryKind::Selected;
        scope.derived_mask = summary_source_mask(scope);
    }
    let mut output = Map::new();
    for (key, value) in input {
        let rule = rule(schema, key);
        if rule.is_empty() {
            if !value.is_null()
                && !value.as_array().is_some_and(Vec::is_empty)
                && !value.as_object().is_some_and(Map::is_empty)
            {
                omit(omissions, "unclassified", &format!("{path}.*"));
            }
            continue;
        }
        let child_path = format!("{path}.{key}");
        if schema == "summaryCoverage" && !value.is_null() && !derived_safe(key, scope) {
            omit(omissions, "unclassified", &child_path);
            continue;
        }
        if native_dependent(schema, key) && !canonical_safe(input, schema, key, value, scope) {
            omit(omissions, "unclassified", &child_path);
            continue;
        }
        if rule == "location" || rule == "deviceIdentifiers" {
            let allowed = if rule == "location" {
                policy.include_location
            } else {
                policy.include_device_identifiers
            };
            if !allowed {
                omit(
                    omissions,
                    if rule == "location" {
                        "location"
                    } else {
                        "deviceIdentifiers"
                    },
                    &child_path,
                );
                continue;
            }
            if value.is_null() || value.is_number() {
                output.insert(key.clone(), value.clone());
            } else {
                omit(omissions, "unclassified", &child_path);
            }
        } else if rule == "scalar" {
            if scalar(value) {
                output.insert(key.clone(), value.clone());
            } else {
                omit(omissions, "unclassified", &child_path);
            }
        } else if rule == "number" {
            if value.is_null() || value.is_number() {
                output.insert(key.clone(), value.clone());
            } else {
                omit(omissions, "unclassified", &child_path);
            }
        } else if let Some(expected) = rule.strip_prefix("literal:") {
            if value.as_str() == Some(expected) {
                output.insert(key.clone(), value.clone());
            } else {
                omit(omissions, "unclassified", &child_path);
            }
        } else if rule == "hash" {
            if hash(value) {
                output.insert(key.clone(), value.clone());
            } else {
                omit(omissions, "unclassified", &child_path);
            }
        } else if rule == "messages" {
            output.insert(key.clone(), messages(value, &child_path, policy, omissions));
        } else if rule == "nativeFields" || rule == "extensions" {
            let fields = value
                .as_array()
                .map(|values| {
                    values
                        .iter()
                        .filter_map(|field| {
                            let number = if rule == "nativeFields" {
                                input.get("globalMessageNumber").and_then(Value::as_u64)
                            } else {
                                field["identity"]["globalMessageNumber"].as_u64()
                            };
                            let field_path = format!("{child_path}.*");
                            let Some(number) = number else {
                                omit(omissions, "unclassified", &field_path);
                                return None;
                            };
                            project_field(
                                number,
                                field,
                                &field_path,
                                policy,
                                omissions,
                                None,
                                scope.source,
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            output.insert(key.clone(), Value::Array(fields));
        } else if rule == "references" {
            output.insert(
                key.clone(),
                project_references(input, schema, key, &child_path, policy, omissions, scope),
            );
        } else if rule == "reference" && schema != "references" {
            let safe = if schema == "sensor" || schema == "zone" {
                scope
                    .source
                    .is_some_and(|source| source.message_reference(value))
            } else {
                scope.source.is_some_and(|source| source.reference(value))
                    && safe_reference(value, "metric")
            };
            if safe {
                output.insert(
                    key.clone(),
                    object(value, "reference", &child_path, policy, omissions),
                );
            } else {
                omit(omissions, "unclassified", &child_path);
            }
        } else if rule == "array:number" {
            if value.is_null()
                || value.as_array().is_some_and(|items| {
                    items.iter().all(|item| item.is_null() || item.is_number())
                })
            {
                output.insert(key.clone(), value.clone());
            } else {
                omit(omissions, "unclassified", &child_path);
            }
        } else if let Some(child) = rule.strip_prefix("array:") {
            if value.is_null() {
                output.insert(key.clone(), Value::Null);
                continue;
            }
            if let Some(values) = value.as_array() {
                let items = values
                    .iter()
                    .filter_map(|item| {
                        if child == "scalar" {
                            if scalar(item) {
                                Some(item.clone())
                            } else {
                                omit(omissions, "unclassified", &format!("{child_path}.*"));
                                None
                            }
                        } else {
                            Some(object_with_references(
                                item,
                                child,
                                &format!("{child_path}.*"),
                                policy,
                                omissions,
                                scope,
                            ))
                        }
                    })
                    .collect();
                output.insert(key.clone(), Value::Array(items));
            } else {
                omit(omissions, "unclassified", &child_path);
            }
        } else {
            let mut child_scope = scope;
            if schema == "summary" && matches!(key.as_str(), "recorded" | "derived") {
                child_scope.references = input.get("sourceReferences");
                child_scope.summary_kind = if key == "recorded" {
                    SummaryKind::Recorded
                } else {
                    SummaryKind::Derived
                };
            }
            output.insert(
                key.clone(),
                object_with_references(value, rule, &child_path, policy, omissions, child_scope),
            );
        }
    }
    Value::Object(output)
}

#[derive(Clone, Copy)]
enum SummaryKind {
    Selected,
    Recorded,
    Derived,
}

#[derive(Clone, Copy)]
struct Scope<'a> {
    source: Option<&'a SourceProof>,
    references: Option<&'a Value>,
    summary: Option<&'a Value>,
    summary_kind: SummaryKind,
    window: Option<(f64, f64)>,
    rr_safe: Option<bool>,
    derived_mask: u8,
}
impl<'a> Scope<'a> {
    fn new(source: Option<&'a SourceProof>) -> Self {
        Self {
            source,
            references: None,
            summary: None,
            summary_kind: SummaryKind::Selected,
            window: source.and_then(SourceProof::session_window),
            rr_safe: None,
            derived_mask: 0,
        }
    }
}

fn source_window(value: &Value, source: Option<&SourceProof>) -> Option<(f64, f64)> {
    let source = source?;
    let references = &value["sourceReferences"];
    let from = &references["startTime"];
    let to = &references["endTime"];
    if !reference_matches("lap", "startTime", from)
        || !reference_matches("lap", "endTime", to)
        || from["messageIndex"] != to["messageIndex"]
    {
        return None;
    }
    let from = source.timestamp(from)?;
    let to = source.timestamp(to)?;
    (from <= to).then_some((from, to))
}

fn native_dependent(schema: &str, key: &str) -> bool {
    match schema {
        "normalized" => matches!(key, "startTime" | "endTime" | "sport" | "subtype"),
        "sample" => key != "index" && rule(schema, key) != "references",
        "summary" | "summaryMetrics" => rule(schema, key) == "number",
        "lap" => {
            matches!(
                key,
                "startTime" | "endTime" | "startElapsedSeconds" | "endElapsedSeconds"
            ) || rule(schema, key) == "location"
        }
        "timer" => matches!(key, "timestamp" | "elapsedSeconds" | "event" | "eventType"),
        "session" => key == "localUtcOffsetSeconds" || rule(schema, key) == "location",
        "localTime" => matches!(
            key,
            "activeTimeZone" | "timeOffsetsSeconds" | "timeZoneOffsetsHours"
        ),
        "sensor" => {
            matches!(
                key,
                "sensorType" | "type" | "sourceType" | "manufacturer" | "product" | "timestamp"
            ) || rule(schema, key) == "deviceIdentifiers"
        }
        "zone" => matches!(key, "minimum" | "maximum" | "low" | "high"),
        "recordedThreshold" => matches!(
            key,
            "value" | "heartRateBpm" | "speedMps" | "powerWatts" | "timestamp"
        ),
        "interval" => matches!(
            key,
            "rrMs"
                | "timestamp"
                | "elapsedSeconds"
                | "startElapsedSeconds"
                | "endElapsedSeconds"
                | "timingEligible"
        ),
        "provenance" => key == "anchorTimestamp",
        _ => false,
    }
}

fn reference_matches(schema: &str, key: &str, reference: &Value) -> bool {
    let Some(message) = reference["globalMessageNumber"].as_u64() else {
        return false;
    };
    let Some(field) = reference["fieldNumber"].as_u64() else {
        return false;
    };
    match (schema, key) {
        ("sample", "timestamp" | "elapsedSeconds" | "timerRunning") => {
            message == 20 && field == 253
        }
        ("sample", "heartRateBpm") => message == 20 && field == 3,
        ("sample", "powerWatts") => message == 20 && field == 7,
        ("sample", "speedMps" | "paceSecondsPerKm") => message == 20 && matches!(field, 6 | 73),
        ("sample", "distanceMeters") => message == 20 && field == 5,
        ("sample", "altitudeMeters") => message == 20 && matches!(field, 2 | 78),
        ("sample", "cadenceStepsPerMinute") => message == 20 && field == 4,
        ("sample", "fractionalCadenceCyclesPerMinute") => message == 20 && field == 53,
        ("sample", "latitude" | "latitudeDegrees") => message == 20 && field == 0,
        ("sample", "longitude" | "longitudeDegrees") => message == 20 && field == 1,
        ("summary" | "summaryMetrics", metric) => match (message, metric) {
            (18 | 19, "distanceMeters") => field == 9,
            (18 | 19, "timerTimeSeconds") => field == 8,
            (18 | 19, "elapsedTimeSeconds") => field == 7,
            (18, "movingTimeSeconds") => field == 59,
            (19, "movingTimeSeconds") => field == 52,
            (18, "averageSpeedMps" | "averagePaceSecondsPerKm") => matches!(field, 14 | 124),
            (19, "averageSpeedMps" | "averagePaceSecondsPerKm") => matches!(field, 13 | 110),
            (18, "averageHeartRateBpm") => field == 16,
            (19, "averageHeartRateBpm") => field == 15,
            (18, "averagePowerWatts") => field == 20,
            (19, "averagePowerWatts") => field == 19,
            (18, "averageCadenceStepsPerMinute") => field == 18,
            (19, "averageCadenceStepsPerMinute") => field == 17,
            (18, "averageFractionalCadenceCyclesPerMinute") => field == 92,
            (19, "averageFractionalCadenceCyclesPerMinute") => field == 80,
            _ => false,
        },
        ("lap", "startTime" | "startElapsedSeconds") => message == 19 && field == 2,
        ("lap", "endTime" | "endElapsedSeconds") => message == 19 && field == 253,
        ("lap", "startLatitude") => message == 19 && field == 3,
        ("lap", "startLongitude") => message == 19 && field == 4,
        ("lap", "endLatitude") => message == 19 && field == 5,
        ("lap", "endLongitude") => message == 19 && field == 6,
        ("session", "startTime") => message == 18 && field == 2,
        ("session", "endTime") => message == 18 && field == 253,
        ("session", "sport") => message == 18 && field == 5,
        ("session", "subtype") => message == 18 && field == 6,
        ("session", "startLatitude") => message == 18 && field == 3,
        ("session", "startLongitude") => message == 18 && field == 4,
        ("session", "endLatitude") => message == 18 && field == 29,
        ("session", "endLongitude") => message == 18 && field == 30,
        ("timer", "timestamp" | "elapsedSeconds") => message == 21 && field == 253,
        ("timer", "event") => message == 21 && field == 0,
        ("timer", "eventType") => message == 21 && field == 1,
        ("localTime", "activeTimeZone") => message == 2 && field == 0,
        ("localTime", "timeOffsetsSeconds") => message == 2 && field == 2,
        ("localTime", "timeZoneOffsetsHours") => message == 2 && field == 5,
        ("offset", "localTimestamp") => message == 34 && field == 5,
        ("offset", "timestamp") => message == 34 && field == 253,
        ("sensor", "serialNumber") => message == 23 && field == 3,
        ("sensor", "antDeviceNumber" | "deviceId" | "unitId") => message == 23 && field == 21,
        ("sensor", "manufacturer") => message == 23 && field == 2,
        ("sensor", "product") => message == 23 && field == 4,
        ("sensor", "sensorType" | "type") => message == 23 && field == 1,
        ("sensor", "sourceType") => message == 23 && field == 25,
        ("sensor", "timestamp") => message == 23 && field == 253,
        ("recordedThreshold", "value") => matches!((message, field), (7, 2 | 3) | (216, 13 | 15)),
        ("recordedThreshold", "heartRateBpm") => matches!((message, field), (7, 2) | (216, 13)),
        ("recordedThreshold", "powerWatts") => matches!((message, field), (7, 3) | (216, 15)),
        ("recordedThreshold", "timestamp") => matches!(message, 7 | 216) && field == 253,
        ("anchor", "timestamp") => message == 132 && field == 253,
        ("anchor", "fractionalTimestamp") => message == 132 && field == 0,
        ("anchor", "eventCounter") => message == 132 && field == 9,
        _ => false,
    }
}

fn direct_safe(schema: &str, key: &str, reference: &Value, source: &SourceProof) -> bool {
    let category = match rule(schema, key) {
        "location" => "location",
        "deviceIdentifiers" => "deviceIdentifiers",
        _ => "metric",
    };
    reference_matches(schema, key, reference)
        && safe_reference(reference, category)
        && source.reference(reference)
}

fn derived_safe(key: &str, scope: Scope<'_>) -> bool {
    let mask = match key {
        "averageSpeedMps" | "averagePaceSecondsPerKm" => 1,
        "averageHeartRateBpm" => 2,
        "averagePowerWatts" => 4,
        "averageCadenceStepsPerMinute" => 8,
        "elapsedTimeSeconds" => 16,
        _ => return false,
    };
    scope.derived_mask & mask != 0
}

fn summary_source_mask(scope: Scope<'_>) -> u8 {
    let Some(source) = scope.source else {
        return 0;
    };
    let Some((from, to)) = scope.window else {
        return 0;
    };
    let mut mask = if source.clock_window_safe(from, to) {
        16
    } else {
        0
    };
    if source.timer_window_safe(from, to) {
        for (metric, bit) in [
            ("speedMps", 1),
            ("heartRateBpm", 2),
            ("powerWatts", 4),
            ("cadenceStepsPerMinute", 8),
        ] {
            if source.record_window_safe(metric, from, to) {
                mask |= bit;
            }
        }
    }
    mask
}

pub(super) fn sample_source_safe(value: &Value, key: &str, source: &SourceProof) -> bool {
    value.as_object().is_some_and(|input| {
        canonical_safe(
            input,
            "sample",
            key,
            &value[key],
            Scope {
                source: Some(source),
                ..Scope::new(None)
            },
        )
    })
}

fn canonical_safe(
    input: &Map<String, Value>,
    schema: &str,
    key: &str,
    value: &Value,
    scope: Scope<'_>,
) -> bool {
    if matches!(schema, "interval" | "provenance") {
        return scope.rr_safe == Some(true);
    }
    let references = scope.references.or_else(|| input.get("sourceReferences"));
    let reference_key = match (schema, key) {
        ("sample", "elapsedSeconds" | "timerRunning") | ("timer", "elapsedSeconds") => "timestamp",
        ("lap", "startElapsedSeconds") => "startTime",
        ("lap", "endElapsedSeconds") => "endTime",
        _ => key,
    };
    let reference = references
        .and_then(|references| references.get(reference_key))
        .filter(|reference| !reference.is_null())
        .or_else(|| {
            input
                .get("sourceReference")
                .filter(|reference| !reference.is_null())
        });
    // An absent metric remains absent; null is never replaced with a manufactured value.
    if value.is_null() {
        return true;
    }
    let Some(source) = scope.source else {
        return false;
    };
    if schema == "normalized" {
        let field = match key {
            "startTime" => 2,
            "endTime" => 253,
            "sport" => 5,
            "subtype" => 6,
            _ => return false,
        };
        if matches!(key, "sport" | "subtype") {
            return source
                .session_reference(2)
                .is_some_and(|reference| source.sibling_safe(&reference, field));
        }
        return source.session_reference(field).is_some_and(|reference| {
            direct_safe("session", key, &reference, source)
                && source.timestamp(&reference).is_some()
        });
    }
    if schema == "summary" || schema == "summaryMetrics" {
        let kind = match scope.summary_kind {
            SummaryKind::Selected
                if scope.summary.is_some_and(|summary| {
                    summary["recorded"]
                        .get(key)
                        .is_some_and(|recorded| !recorded.is_null())
                        || summary.get("recorded").is_none() && reference.is_some()
                }) =>
            {
                SummaryKind::Recorded
            }
            SummaryKind::Selected => SummaryKind::Derived,
            kind => kind,
        };
        if matches!(kind, SummaryKind::Derived) {
            return derived_safe(key, scope);
        }
    }
    if schema == "session" && key == "localUtcOffsetSeconds" {
        let Some(references) = input.get("localUtcOffsetSourceReferences") else {
            return false;
        };
        return ["localTimestamp", "timestamp"]
            .into_iter()
            .all(|key| direct_safe("offset", key, &references[key], source))
            && references["localTimestamp"]["messageIndex"]
                == references["timestamp"]["messageIndex"];
    }
    let Some(reference) = reference else {
        return false;
    };
    if schema == "sensor" && reference.get("fieldNumber").is_none() {
        let field = match key {
            "sensorType" | "type" => 1,
            "manufacturer" => 2,
            "serialNumber" => 3,
            "product" => 4,
            "antDeviceNumber" | "deviceId" | "unitId" => 21,
            "sourceType" => 25,
            "timestamp" => 253,
            _ => return false,
        };
        return source.message_reference(reference)
            && source
                .sibling_reference(reference, field)
                .is_some_and(|reference| {
                    direct_safe(schema, key, &reference, source)
                        && (key != "timestamp" || source.timestamp(&reference).is_some())
                });
    }
    if schema == "recordedThreshold" && key == "timestamp" {
        return source
            .sibling_reference(reference, 253)
            .is_some_and(|reference| source.timestamp(&reference).is_some());
    }
    if schema == "recordedThreshold" && key == "value" {
        let metric = match input.get("kind").and_then(Value::as_str) {
            Some("thresholdHeartRateBpm") => "heartRateBpm",
            Some("functionalThresholdPowerWatts") => "powerWatts",
            _ => return false,
        };
        return direct_safe(schema, metric, reference, source)
            && source.scalar(reference).is_some();
    }
    if !direct_safe(schema, reference_key, reference, source) {
        return false;
    }
    if matches!(key, "timestamp" | "startTime" | "endTime") {
        return source.timestamp(reference).is_some();
    }
    if matches!(
        key,
        "elapsedSeconds" | "startElapsedSeconds" | "endElapsedSeconds"
    ) {
        return source.timestamp(reference).is_some() && source.session_start().is_some();
    }
    if key == "timerRunning" {
        return source.timestamp(reference).is_some_and(|time| {
            source
                .session_start()
                .is_some_and(|start| source.timer_window_safe(start, time))
        });
    }
    if matches!(
        schema,
        "sample" | "summary" | "summaryMetrics" | "recordedThreshold"
    ) && matches!(rule(schema, key), "number" | "location")
        && source.scalar(reference).is_none_or(|native| {
            matches!(key, "paceSecondsPerKm" | "averagePaceSecondsPerKm") && native <= 0.0
        })
    {
        return false;
    }
    if matches!(
        key,
        "cadenceStepsPerMinute" | "averageCadenceStepsPerMinute"
    ) {
        let (field, fractional_key) = match reference["globalMessageNumber"].as_u64() {
            Some(20) => (53, "fractionalCadenceCyclesPerMinute"),
            Some(18) => (92, "averageFractionalCadenceCyclesPerMinute"),
            Some(19) => (80, "averageFractionalCadenceCyclesPerMinute"),
            _ => return false,
        };
        return source.sibling_safe(reference, field)
            && references
                .and_then(|references| references.get(fractional_key))
                .filter(|reference| !reference.is_null())
                .is_none_or(|fractional| {
                    fractional["messageIndex"] == reference["messageIndex"]
                        && direct_safe(schema, fractional_key, fractional, source)
                });
    }
    true
}

fn project_references(
    owner: &Map<String, Value>,
    schema: &str,
    container: &str,
    path: &str,
    policy: Policy,
    omissions: &mut Omissions,
    scope: Scope<'_>,
) -> Value {
    let references = &owner[container];
    if references.is_null() {
        return Value::Null;
    }
    let Some(references) = references.as_object() else {
        omit(omissions, "unclassified", path);
        return json!({});
    };
    let mut output = Map::new();
    for (key, reference) in references {
        if rule("references", key).is_empty() {
            omit(omissions, "unclassified", &format!("{path}.*"));
            continue;
        }
        let category = match rule(schema, key) {
            "location" => "location",
            "deviceIdentifiers" => "deviceIdentifiers",
            _ => "metric",
        };
        let allowed = (category != "location" || policy.include_location)
            && (category != "deviceIdentifiers" || policy.include_device_identifiers);
        let owner_key = match key.as_str() {
            "fractionalCadenceCyclesPerMinute" => "cadenceStepsPerMinute",
            "averageFractionalCadenceCyclesPerMinute" => "averageCadenceStepsPerMinute",
            _ => key,
        };
        if reference.is_null()
            && allowed
            && owner
                .get(owner_key)
                .is_some_and(|value| canonical_safe(owner, schema, owner_key, value, scope))
        {
            output.insert(key.clone(), Value::Null);
            continue;
        }
        let safe = scope.source.is_some_and(|source| {
            let reference_schema = match container {
                "localUtcOffsetSourceReferences" => "offset",
                "anchorSourceReferences" => "anchor",
                _ => schema,
            };
            direct_safe(reference_schema, key, reference, source)
                && match container {
                    "localUtcOffsetSourceReferences" => {
                        owner.get("localUtcOffsetSeconds").is_some_and(|value| {
                            canonical_safe(owner, schema, "localUtcOffsetSeconds", value, scope)
                        })
                    }
                    "anchorSourceReferences" => scope.rr_safe == Some(true),
                    _ if schema == "summary" => owner
                        .get("recorded")
                        .and_then(|recorded| recorded.get(owner_key))
                        .or_else(|| owner.get(owner_key))
                        .is_some_and(|value| {
                            canonical_safe(
                                owner
                                    .get("recorded")
                                    .and_then(Value::as_object)
                                    .unwrap_or(owner),
                                "summaryMetrics",
                                owner_key,
                                value,
                                Scope {
                                    references: owner.get("sourceReferences"),
                                    summary_kind: SummaryKind::Recorded,
                                    ..scope
                                },
                            )
                        }),
                    _ if schema == "session"
                        && matches!(
                            key.as_str(),
                            "startTime" | "endTime" | "sport" | "subtype"
                        ) =>
                    {
                        true
                    }
                    _ => owner.get(owner_key).is_some_and(|value| {
                        canonical_safe(owner, schema, owner_key, value, scope)
                    }),
                }
        });
        let child_path = format!("{path}.{key}");
        if allowed && safe {
            output.insert(
                key.clone(),
                object(reference, "reference", &child_path, policy, omissions),
            );
        } else {
            omit(
                omissions,
                if allowed { "unclassified" } else { category },
                &child_path,
            );
        }
    }
    Value::Object(output)
}
fn scalar(value: &Value) -> bool {
    value.is_null() || value.is_string() || value.is_number() || value.is_boolean()
}
fn hash(value: &Value) -> bool {
    value.as_str().is_some_and(|text| {
        text.len() == 64
            && text
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    })
}

fn rule<'a>(schema: &str, key: &'a str) -> &'a str {
    match (schema, key) {
        ("activity", "id") => "scalar",
        ("activity", "decoded" | "normalized" | "analysis" | "historicalThresholds") => match key {
            "historicalThresholds" => "history",
            _ => key,
        },
        ("decoded", "schemaVersion") => "scalar",
        ("decoded", "decoder") => "decoder",
        ("decoded", "messages") => "messages",
        ("decoded" | "normalized", "warnings") => "array:warning",
        (
            "decoder",
            "name"
            | "library"
            | "libraryVersion"
            | "version"
            | "profileVersion"
            | "options"
            | "hrMerge"
            | "archiveSchemaVersion"
            | "normalizedSchemaVersion"
            | "normalizerVersion"
            | "jsonFloatRoundtrip",
        ) => {
            if key == "options" {
                "array:scalar"
            } else {
                "scalar"
            }
        }
        (
            "decoder",
            "vendorSourceSha256"
            | "profileSourceSha256"
            | "normalizerSourceSha256"
            | "archiveSourceSha256"
            | "rawProjectionSourceSha256",
        ) => "hash",
        ("parameters", "windowEnumeration" | "nativeRrTimingMethod") => "scalar",
        ("parameters", "requiredAnchorFields") => "array:scalar",
        ("parameters", "floatingBoundToleranceSeconds") => "number",
        ("session", "localUtcOffsetSeconds") => "number",
        ("session", "localUtcOffsetSourceReferences") => "references",
        ("session", "sourceLocalTimeContext") => "localTime",
        ("session", "sourceLocalTimeContexts") => "array:localTime",
        ("localTime", "activeTimeZone") => "number",
        ("localTime", "timeOffsetsSeconds" | "timeZoneOffsetsHours") => "array:number",
        ("localTime", "sourceReferences") => "references",
        ("normalized", "schemaVersion" | "startTime" | "endTime" | "sport" | "subtype") => "scalar",
        ("normalized", "summary") | ("lap", "summary") => "summary",
        ("normalized", "session") => "session",
        ("normalized", "samples") => "array:sample",
        ("normalized", "laps") => "array:lap",
        ("normalized", "timerEvents") => "array:timer",
        ("normalized", "sensors") => "array:sensor",
        ("normalized", "zones") => "array:zone",
        ("normalized", "deviceReportedThresholds") => "array:recordedThreshold",
        ("normalized", "rr") => "rr",
        ("normalized", "extensions") => "extensions",
        (
            "summary" | "summaryMetrics",
            "distanceMeters"
            | "timerTimeSeconds"
            | "elapsedTimeSeconds"
            | "movingTimeSeconds"
            | "averageSpeedMps"
            | "averagePaceSecondsPerKm"
            | "averageHeartRateBpm"
            | "averagePowerWatts"
            | "averageCadenceStepsPerMinute",
        ) => "number",
        ("summary", "recorded" | "derived") => "summaryMetrics",
        ("summary", "sourceReferences") => "references",
        ("summary", "coverage") => "summaryCoverage",
        ("summary", "method") => "summaryMethod",
        (
            "summaryCoverage",
            "averageSpeedMps"
            | "averagePaceSecondsPerKm"
            | "averageHeartRateBpm"
            | "averagePowerWatts"
            | "averageCadenceStepsPerMinute",
        ) => "summaryMetricCoverage",
        ("summaryMetricCoverage", "coveredSeconds" | "windowSeconds" | "fraction") => "number",
        ("summaryMethod", "gapThresholdSeconds") => "number",
        ("summaryMethod", "default") => "literal:recorded_summary_preferred",
        ("summaryMethod", "derived") => "literal:interval_weighted_left_sample",
        ("summaryMethod", "pauseHandling") => "literal:exclude_recorded_timer_pauses",
        ("summaryMethod", "missingTimerState") => "literal:unknown_not_assumed_stopped",
        ("summaryMethod", "finalSample") => "literal:no_extrapolation",
        (
            "sample",
            "index"
            | "elapsedSeconds"
            | "speedMps"
            | "paceSecondsPerKm"
            | "heartRateBpm"
            | "powerWatts"
            | "cadenceStepsPerMinute"
            | "altitudeMeters"
            | "distanceMeters",
        ) => "number",
        ("sample", "timestamp" | "timerRunning") => "scalar",
        (
            "sample" | "session" | "lap" | "timer" | "sensor" | "recordedThreshold",
            "sourceReferences",
        ) => "references",
        (
            "sample" | "lap" | "session",
            "latitude" | "longitude" | "latitudeDegrees" | "longitudeDegrees" | "startLatitude"
            | "startLongitude" | "endLatitude" | "endLongitude",
        ) => "location",
        ("sensor", "serialNumber" | "deviceId" | "unitId" | "antDeviceNumber") => {
            "deviceIdentifiers"
        }
        ("lap", "index" | "startElapsedSeconds" | "endElapsedSeconds") => "number",
        ("lap", "startTime" | "endTime") => "scalar",
        ("timer", "index" | "elapsedSeconds") | ("session", "index") => "number",
        ("timer", "timestamp" | "event" | "eventType") => "scalar",
        (
            "sensor",
            "index" | "sensorType" | "type" | "sourceType" | "manufacturer" | "product" | "unit"
            | "timestamp",
        ) => "scalar",
        ("zone", "index" | "minimum" | "maximum" | "low" | "high") => "number",
        ("zone", "metric" | "unit" | "source" | "definition") => "scalar",
        ("sensor" | "zone", "globalMessageNumber") => "number",
        ("sensor" | "zone", "fields") => "nativeFields",
        ("sensor" | "zone" | "recordedThreshold", "sourceReference") => "reference",
        ("recordedThreshold", "index" | "value") => "number",
        ("recordedThreshold", "kind" | "unit" | "provenance") => "scalar",
        ("recordedThreshold", "heartRateBpm" | "speedMps" | "powerWatts") => "number",
        ("recordedThreshold", "type" | "timestamp" | "definition") => "scalar",
        (
            "references",
            "timestamp"
            | "startTime"
            | "endTime"
            | "elapsedSeconds"
            | "speedMps"
            | "paceSecondsPerKm"
            | "heartRateBpm"
            | "powerWatts"
            | "cadenceStepsPerMinute"
            | "altitudeMeters"
            | "distanceMeters"
            | "timerTimeSeconds"
            | "elapsedTimeSeconds"
            | "averageSpeedMps"
            | "averageHeartRateBpm"
            | "averagePowerWatts"
            | "averageCadenceStepsPerMinute"
            | "event"
            | "eventType",
        ) => "reference",
        (
            "references",
            "movingTimeSeconds"
            | "averagePaceSecondsPerKm"
            | "averageFractionalCadenceCyclesPerMinute",
        ) => "reference",
        (
            "references",
            "fractionalCadenceCyclesPerMinute"
            | "sport"
            | "subtype"
            | "fractionalTimestamp"
            | "eventCounter"
            | "latitude"
            | "longitude"
            | "latitudeDegrees"
            | "longitudeDegrees"
            | "startLatitude"
            | "startLongitude"
            | "endLatitude"
            | "endLongitude"
            | "serialNumber"
            | "deviceId"
            | "unitId"
            | "antDeviceNumber"
            | "manufacturer"
            | "product"
            | "sensorType"
            | "type"
            | "sourceType"
            | "value",
        ) => "reference",
        (
            "references",
            "activeTimeZone" | "timeOffsetsSeconds" | "timeZoneOffsetsHours" | "localTimestamp",
        ) => "reference",
        (
            "reference",
            "messageIndex"
            | "globalMessageNumber"
            | "fieldNumber"
            | "byteOffset"
            | "byteLength"
            | "componentParent"
            | "arrayIndex",
        ) => "number",
        ("rr", "intervals") => "array:interval",
        ("rr", "alignmentEligible") => "scalar",
        ("rr" | "provenance", "reasons" | "limitations") => "array:scalar",
        (
            "interval",
            "index" | "rrMs" | "elapsedSeconds" | "startElapsedSeconds" | "endElapsedSeconds",
        ) => "number",
        ("interval", "timestamp" | "timingEligible") => "scalar",
        ("interval", "sourceReference") => "reference",
        ("interval", "provenance") => "provenance",
        (
            "provenance",
            "kind" | "messageNumber" | "fieldNumber" | "timingMethod" | "anchorTimestamp",
        ) => "scalar",
        ("provenance", "anchorSourceReferences") => "references",
        ("warning", "code" | "reason" | "messageIndex" | "globalMessageNumber" | "fieldNumber") => {
            "scalar"
        }
        ("analysis", "schemaVersion" | "version") => "scalar",
        ("analysis", "quality") => "quality",
        ("analysis", "segments") => "array:segment",
        ("analysis", "thresholds") => "history",
        ("analysis", "transformations") => "array:transformation",
        (
            "quality",
            "coverage"
            | "counts"
            | "usableDurationSeconds"
            | "pauseRatio"
            | "gapCount"
            | "duplicateTimestampCount"
            | "outOfOrderTimestampCount"
            | "artifactCount"
            | "sampleCount"
            | "timerTimeSeconds"
            | "elapsedTimeSeconds"
            | "elapsedSeconds"
            | "duplicateTimestamps"
            | "outOfOrder"
            | "invalidTimes"
            | "pausedSeconds",
        ) => {
            if key == "coverage" || key == "counts" {
                "qualityMetrics"
            } else {
                "number"
            }
        }
        (
            "quality",
            "recordedTimerTimeSeconds"
            | "derivedTimerTimeSeconds"
            | "timerTotalsDifferenceSeconds"
            | "timerCoverage",
        ) => "number",
        ("quality", "pauseTimeBasis") => "scalar",
        ("quality", "timerPauseIntervals") => "array:boundary",
        ("quality", "warnings" | "reasons") => "array:scalar",
        ("quality", "gaps" | "pauses") => "array:boundary",
        (
            "qualityMetrics",
            "heartRateBpm" | "powerWatts" | "speedMps" | "rr" | "samples" | "valid" | "missing"
            | "durationSeconds" | "coverageRatio" | "heartRate" | "speed" | "power",
        ) => "number",
        (
            "boundary",
            "startElapsedSeconds"
            | "endElapsedSeconds"
            | "durationSeconds"
            | "startIndex"
            | "endIndex",
        ) => "number",
        ("boundary", "reason") => "scalar",
        ("quality", "artifacts") => "array:artifact",
        ("artifact", "elapsedSeconds") => "number",
        ("artifact", "metric" | "reason") => "scalar",
        (
            "segment",
            "index"
            | "startElapsedSeconds"
            | "endElapsedSeconds"
            | "durationSeconds"
            | "repeatIndex"
            | "repeatGroup"
            | "startIndex"
            | "endIndex",
        ) => "number",
        ("segment", "kind" | "type" | "timeBasis" | "version" | "accepted") => "scalar",
        ("segment", "eligibility") => "eligibility",
        ("eligibility", "lt1" | "lt2") => "verdict",
        ("verdict", "accepted") => "scalar",
        ("verdict", "reasons") => "array:scalar",
        ("segment", "features") => "features",
        ("segment", "reasons" | "acceptReasons" | "rejectReasons") => "array:scalar",
        (
            "features",
            "averageSpeedMps"
            | "averageHeartRateBpm"
            | "averagePowerWatts"
            | "speedCv"
            | "powerCv"
            | "hrDrift"
            | "durationSeconds"
            | "sourceCount"
            | "coverageRatio"
            | "speedMeanMps"
            | "speedSlopeMpsPerSecond"
            | "heartRateDriftBpm"
            | "powerMeanWatts"
            | "sampleCount",
        ) => "number",
        ("features", "workloadSurgeCount") => "number",
        ("features", "repeatPatternObserved") => "scalar",
        ("features", "coverage") => "qualityMetrics",
        (
            "history",
            "evidenceCutoff" | "computedAt" | "schemaVersion" | "engineStatus" | "stale"
            | "policyVersion" | "researchBlocked",
        ) => "scalar",
        ("history", "lt1" | "lt2") => "target",
        (
            "target",
            "status"
            | "engineStatus"
            | "methodVersion"
            | "targetDefinition"
            | "availability"
            | "experimentalEstimate"
            | "researchBlocked"
            | "requiredContextUnprovable",
        ) => "scalar",
        ("target", "contextUnverified") => "array:scalar",
        ("target", "method") => "method",
        ("method", "id" | "version") => "scalar",
        ("method", "configurationHash") => "hash",
        ("target", "freshness") => "freshness",
        ("freshness", "stale") => "scalar",
        ("freshness", "ageDays") => "number",
        ("target", "value" | "uncertainty") => "value",
        ("target", "reasons" | "limits" | "limitations") => "array:scalar",
        ("target", "evidence") => "evidence",
        ("target", "trace") => "trace",
        ("target", "suggestions") => "array:suggestion",
        (
            "value",
            "heartRateBpm"
            | "speedMps"
            | "paceSecondsPerKm"
            | "powerWatts"
            | "lower"
            | "upper"
            | "value"
            | "alpha"
            | "crossingHeartRateBpm",
        ) => "number",
        ("value", "unit" | "type" | "definition" | "method") => "scalar",
        ("value", "interval") => "array:scalar",
        ("value", "reason") => "scalar",
        (
            "evidence",
            "activityCount"
            | "independentActivityCount"
            | "windowCount"
            | "acceptedCount"
            | "rejectedCount"
            | "ageDays"
            | "sampleCount"
            | "sourceCount",
        ) => "number",
        ("evidence", "startTime" | "endTime" | "dateRangeStart" | "dateRangeEnd" | "lineage") => {
            "scalar"
        }
        ("trace", "parameters") => "parameters",
        ("trace", "regression" | "counts") => "calculation",
        ("trace", "crossing" | "independentActivityCount") => "number",
        ("trace", "evidenceCutoff" | "lineage" | "computedAt") => "scalar",
        ("trace", "formula" | "methodVersion" | "limitations") => {
            if key == "limitations" {
                "array:scalar"
            } else {
                "scalar"
            }
        }
        (
            "parameters",
            "windowSeconds"
            | "stepSeconds"
            | "alphaTarget"
            | "minimumBeats"
            | "artifactLimit"
            | "lambda"
            | "lookbackDays"
            | "minimumHeartRateSpanBpm"
            | "scaleMinimum"
            | "scaleMaximum",
        ) => "number",
        ("parameters", "detrending" | "interpolation" | "version") => "scalar",
        ("parameters", "scales") => "array:scalar",
        (
            "parameters",
            "localTrendOrder"
            | "artifactCeiling"
            | "minimumInnerWindows"
            | "minimumHrCoverage"
            | "maximumSampleGapSeconds"
            | "maximumRrContinuityErrorSeconds",
        ) => "number",
        (
            "parameters",
            "workloadBlockSeconds"
            | "steadySpeedCvCeiling"
            | "minimumProgressiveSpeedSlopeMpsPerSecond"
            | "surgeRelativeChange"
            | "minimumSpeedCoverage"
            | "minimumRepeatedSurges",
        ) => "number",
        ("parameters", "inputProjectionVersion") => "scalar",
        (
            "parameters",
            "segmentVersion" | "samplingConvention" | "overlap" | "scaleTail" | "fluctuation"
            | "fit" | "windowClosure" | "artifactPolicy" | "selector" | "comparability",
        ) => "scalar",
        (
            "calculation",
            "slope"
            | "intercept"
            | "rSquared"
            | "crossingHeartRateBpm"
            | "alphaTarget"
            | "pointCount"
            | "accepted"
            | "rejected"
            | "total"
            | "minimumHeartRateBpm"
            | "maximumHeartRateBpm"
            | "hrMin"
            | "hrMax"
            | "alphaMin"
            | "alphaMax"
            | "candidate"
            | "windows"
            | "acceptedWindows"
            | "rejectedWindows",
        ) => "number",
        (
            "suggestion",
            "code" | "purpose" | "duration" | "durationSeconds" | "rationale" | "expectedData"
            | "safety" | "optional",
        ) => "scalar",
        ("suggestion", "prerequisites" | "steps" | "limits") => "array:scalar",
        (
            "transformation",
            "kind"
            | "method"
            | "version"
            | "timeBasis"
            | "precision"
            | "description"
            | "resolutionSeconds"
            | "sourceCount"
            | "outputCount"
            | "type"
            | "blockSeconds"
            | "originalSamplesChanged"
            | "hrStabilityFilter"
            | "derivedTimelineSorted",
        ) => "scalar",
        _ => "",
    }
}

// Native FIT profile identity, never a supplied display name or classification.
fn native_category(message: u64, field: u64) -> Option<&'static str> {
    match (message, field) {
        (20, 0 | 1) | (18, 3 | 4 | 29 | 30 | 31 | 32 | 38 | 39) | (19, 3..=6) => Some("location"),
        (0, 3 | 5) | (23, 3 | 21) => Some("deviceIdentifiers"),
        (
            20,
            2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 13 | 30 | 31 | 32 | 39 | 40 | 41 | 53 | 73 | 78 | 83
            | 84 | 85 | 253,
        ) => Some("metric"),
        (
            18,
            0 | 1 | 2 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 13 | 14 | 15 | 16 | 17 | 18 | 19 | 20 | 21
            | 22 | 23 | 24 | 25 | 26 | 27 | 28 | 34 | 35 | 42 | 43 | 44 | 45 | 46 | 47 | 48 | 49
            | 57 | 58 | 59 | 60 | 65 | 66 | 67 | 68 | 69 | 70 | 71 | 86 | 87 | 88 | 89 | 90 | 91
            | 92 | 124 | 253,
        ) => Some("metric"),
        (
            19,
            0 | 1 | 2 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18 | 19 | 20 | 21 | 22
            | 23 | 24 | 25 | 26 | 32 | 33 | 35 | 37 | 38 | 39 | 40 | 52 | 57 | 58 | 59 | 60 | 71
            | 74 | 75 | 76 | 77 | 78 | 80 | 110 | 253,
        ) => Some("metric"),
        (0, 0 | 1 | 2 | 4)
        | (2, 0 | 1 | 2 | 4 | 5 | 39)
        | (23, 0 | 1 | 2 | 4 | 5 | 6 | 25 | 253)
        | (21, 0 | 1 | 3 | 4 | 253)
        | (34, 0 | 1 | 2 | 3 | 4 | 5 | 6 | 253)
        | (78, 0)
        | (132, 0 | 1 | 6 | 9 | 10 | 253)
        | (7, 1 | 2 | 3 | 5 | 7)
        | (8, 1 | 254)
        | (9, 1 | 254)
        | (53, 0 | 254)
        | (216, 13 | 15) => Some("metric"),
        _ => None,
    }
}

fn messages(value: &Value, path: &str, policy: Policy, omissions: &mut Omissions) -> Value {
    let Some(messages) = value.as_array() else {
        omit(omissions, "unclassified", path);
        return json!([]);
    };
    Value::Array(
        messages
            .iter()
            .filter_map(|message| project_message(message, &format!("{path}.*"), policy, omissions))
            .collect(),
    )
}

fn project_message(
    message: &Value,
    path: &str,
    policy: Policy,
    omissions: &mut Omissions,
) -> Option<Value> {
    let number = message.get("globalMessageNumber")?.as_u64()?;
    let mut output = Map::new();
    for key in ["index", "globalMessageNumber", "localMessageNumber"] {
        if let Some(value) = message.get(key).filter(|v| v.is_number()) {
            output.insert(key.into(), value.clone());
        }
    }
    if let Some(reference) = message.get("sourceReference") {
        output.insert(
            "sourceReference".into(),
            object(
                reference,
                "reference",
                &format!("{path}.sourceReference"),
                policy,
                omissions,
            ),
        );
    }
    let field_path = format!("{path}.fields.*");
    let fields = message
        .get("fields")
        .and_then(Value::as_array)
        .map(|fields| {
            fields
                .iter()
                .filter_map(|field| {
                    project_field(
                        number,
                        field,
                        &field_path,
                        policy,
                        omissions,
                        Some(fields),
                        None,
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    output.insert("fields".into(), Value::Array(fields));
    Some(Value::Object(output))
}

fn project_field(
    number: u64,
    field: &Value,
    path: &str,
    policy: Policy,
    omissions: &mut Omissions,
    siblings: Option<&[Value]>,
    source: Option<&SourceProof>,
) -> Option<Value> {
    let normalized = siblings.is_none();
    let identity = &field["identity"];
    let Some(field_number) = field
        .get("fieldNumber")
        .or_else(|| identity.get("fieldNumber"))
        .and_then(Value::as_u64)
    else {
        omit(omissions, "unclassified", path);
        return None;
    };
    let reference = &field["sourceReference"];
    let developer = field.get("developerIdentity").is_some_and(|v| !v.is_null())
        || identity
            .get("developerIdentity")
            .is_some_and(|v| !v.is_null())
        || reference
            .get("developerIdentity")
            .is_some_and(|v| !v.is_null());
    let category = if developer
        || identity
            .get("globalMessageNumber")
            .and_then(Value::as_u64)
            .is_some_and(|n| n != number)
        || reference
            .get("globalMessageNumber")
            .and_then(Value::as_u64)
            .is_some_and(|n| n != number)
        || reference
            .get("fieldNumber")
            .and_then(Value::as_u64)
            .is_some_and(|n| n != field_number)
    {
        None
    } else {
        native_category(number, field_number)
    };
    let Some(category) = category else {
        omit(omissions, "unclassified", path);
        return None;
    };
    let parent = field
        .get("componentParent")
        .or_else(|| reference.get("componentParent"))
        .and_then(Value::as_u64);
    if normalized
        && field
            .get("componentParent")
            .filter(|value| !value.is_null())
            .is_some_and(|parent| reference.get("componentParent") != Some(parent))
    {
        omit(omissions, "unclassified", path);
        return None;
    }
    if parent.is_some_and(|n| native_category(number, n) != Some(category)) {
        omit(omissions, "unclassified", path);
        return None;
    }
    if (category == "location" && !policy.include_location)
        || (category == "deviceIdentifiers" && !policy.include_device_identifiers)
    {
        omit(omissions, category, path);
        return None;
    }
    let value = field.get("value")?;
    if if normalized {
        !source.is_some_and(|source| source.field_payload_safe(reference, field))
    } else {
        !native_field_valid(number, field, siblings)
    } {
        omit(omissions, "unclassified", path);
        return None;
    }
    let named_enum = value.is_string() && enum_profile(number, field_number).is_some();
    let mut safe = Map::new();
    safe.insert("fieldNumber".into(), json!(field_number));
    safe.insert("value".into(), value.clone());
    if identity.is_object() {
        safe.insert(
            "identity".into(),
            json!({"globalMessageNumber":number,"fieldNumber":field_number}),
        );
    }
    if let Some(name) = verified_name(number, field_number) {
        safe.insert("name".into(), json!(name));
    }
    safe.insert("classification".into(), json!(category));
    for key in ["rawValue", "scale", "offset"] {
        if let Some(value) = field.get(key).filter(|v| numeric_value(v)) {
            safe.insert(key.into(), value.clone());
        }
    }
    for key in [
        "baseType",
        "type",
        "byteSize",
        "profileType",
        "unit",
        "validity",
        "role",
        "developerIdentity",
        "componentParent",
        "compressedTimeOffset",
        "enumCode",
    ] {
        if let Some(value) = field.get(key) {
            let safe_text = value.as_str().is_some_and(|text| {
                matches!(
                    text,
                    "valid"
                        | "invalid"
                        | "mixed"
                        | "native"
                        | "expanded"
                        | "reconstructed"
                        | "enum"
                        | "sint8"
                        | "uint8"
                        | "sint16"
                        | "uint16"
                        | "sint32"
                        | "uint32"
                        | "sint64"
                        | "uint64"
                        | "float32"
                        | "float64"
                        | "date_time"
                        | "local_date_time"
                        | "m"
                        | "m/s"
                        | "s"
                        | "bpm"
                        | "watts"
                        | "rpm"
                        | "cycles"
                        | "semicircles"
                        | "C"
                        | "%"
                        | "ms"
                )
            });
            if value.is_null() || value.is_number() || safe_text {
                safe.insert(key.into(), value.clone());
            }
        }
    }
    if named_enum {
        safe.insert("enumCode".into(), field["rawValue"].clone());
    }
    if !reference.is_null() {
        safe.insert(
            "sourceReference".into(),
            object(
                reference,
                "reference",
                &format!("{path}.sourceReference"),
                policy,
                omissions,
            ),
        );
    }
    Some(Value::Object(safe))
}

pub fn project_normalized(
    value: &Value,
    policy: Policy,
    source: &SourceProof,
) -> Result<(Value, Value), ApiError> {
    let mut omissions = BTreeMap::new();
    let projected = object_with_references(
        value,
        "normalized",
        "activities.*.normalized",
        policy,
        &mut omissions,
        Scope::new(Some(source)),
    );
    if required_missing(value, &projected, "normalized") {
        return Err(source_unprovable());
    }
    Ok((projected, omission_rows(omissions)))
}

fn source_unprovable() -> ApiError {
    ApiError::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        "RUNS_EXPORT_SOURCE_UNPROVABLE",
        "Stored run source cannot prove a required export value.",
    )
}

fn required_missing(original: &Value, projected: &Value, schema: &str) -> bool {
    let required: &[&str] = match schema {
        "normalized" => &["startTime", "endTime"],
        "sample" | "timer" => &["timestamp", "elapsedSeconds"],
        "lap" => &[
            "startTime",
            "endTime",
            "startElapsedSeconds",
            "endElapsedSeconds",
        ],
        "interval" => &["timingEligible", "sourceReference", "provenance"],
        _ => &[],
    };
    if required
        .iter()
        .any(|key| original.get(key).is_some() && projected.get(key).is_none())
    {
        return true;
    }
    let Some(original) = original.as_object() else {
        return false;
    };
    original.iter().any(|(key, value)| {
        let child = rule(schema, key);
        if let Some(child) = child.strip_prefix("array:") {
            value
                .as_array()
                .zip(projected[key].as_array())
                .is_some_and(|(before, after)| {
                    before
                        .iter()
                        .zip(after)
                        .any(|(before, after)| required_missing(before, after, child))
                })
        } else if value.is_object() && projected[key].is_object() {
            required_missing(value, &projected[key], child)
        } else {
            false
        }
    })
}

fn omission_rows(omissions: Omissions) -> Value {
    Value::Array(omissions.into_iter().flat_map(|(category,paths)|paths.into_iter().map(move |(path,count)|json!({
        "category":category,"pathPattern":path,"count":count,"reason":"Field omitted by the export schema and consent policy."
    }))).collect())
}

#[derive(Clone, Copy)]
pub enum Document {
    Decoded,
    Normalized,
}

/// One bounded archive item at a time. Paths contain only schema keys and wildcard array positions.
pub struct Projection<'a> {
    document: Document,
    policy: Policy,
    omissions: Omissions,
    scope: Scope<'a>,
    required_error: bool,
}
impl<'source> Projection<'source> {
    pub fn new(document: Document, policy: Policy) -> Self {
        Self {
            document,
            policy,
            omissions: BTreeMap::new(),
            scope: Scope::new(None),
            required_error: false,
        }
    }
    pub fn with_source(document: Document, policy: Policy, source: &'source SourceProof) -> Self {
        Self {
            scope: Scope::new(Some(source)),
            ..Self::new(document, policy)
        }
    }
    fn resolve<'a>(&self, path: &'a [String]) -> (&'a str, String) {
        let root = match self.document {
            Document::Decoded => "decoded",
            Document::Normalized => "normalized",
        };
        let mut schema = root;
        let mut safe = format!("activities.*.{root}");
        for key in path {
            let next = if key == "*" {
                match schema {
                    "messages" => "message",
                    "extensions" | "nativeFields" => "extension",
                    _ => schema.strip_prefix("array:").unwrap_or(""),
                }
            } else {
                rule(schema, key)
            };
            if next.is_empty() {
                safe.push_str(".*");
                return ("", safe);
            }
            safe.push('.');
            safe.push_str(key);
            schema = next;
        }
        (schema, safe)
    }
    pub fn keep(&mut self, path: &[String]) -> bool {
        let (schema, safe) = self.resolve(path);
        if matches!(self.document, Document::Normalized)
            && path.len() == 1
            && native_dependent("normalized", &path[0])
            && !canonical_safe(
                &Map::new(),
                "normalized",
                &path[0],
                &Value::Bool(true),
                self.scope,
            )
        {
            omit(&mut self.omissions, "unclassified", &safe);
            self.required_error |= matches!(path[0].as_str(), "startTime" | "endTime");
            return false;
        }
        let category = match schema {
            "location" if !self.policy.include_location => Some("location"),
            "deviceIdentifiers" if !self.policy.include_device_identifiers => {
                Some("deviceIdentifiers")
            }
            "" => Some("unclassified"),
            _ => None,
        };
        if let Some(category) = category {
            omit(&mut self.omissions, category, &safe);
            false
        } else {
            true
        }
    }
    pub fn value(&mut self, path: &[String], value: Value) -> Option<Value> {
        let (schema, safe) = self.resolve(path);
        let policy = self.policy;
        let scope = self.scope;
        if matches!(self.document, Document::Normalized)
            && path.len() == 1
            && native_dependent("normalized", &path[0])
            && !canonical_safe(&Map::new(), "normalized", &path[0], &value, scope)
        {
            omit(&mut self.omissions, "unclassified", &safe);
            self.required_error |= matches!(path[0].as_str(), "startTime" | "endTime");
            return None;
        }
        match schema {
            "" => None,
            "message" => project_message(&value, &safe, policy, &mut self.omissions),
            "extension" => value["identity"]["globalMessageNumber"]
                .as_u64()
                .and_then(|number| {
                    project_field(
                        number,
                        &value,
                        &safe,
                        policy,
                        &mut self.omissions,
                        None,
                        self.scope.source,
                    )
                }),
            "scalar" if scalar(&value) => Some(value),
            "number" | "location" | "deviceIdentifiers" if value.is_null() || value.is_number() => {
                Some(value)
            }
            "hash" if hash(&value) => Some(value),
            "scalar" | "location" | "deviceIdentifiers" | "number" | "hash" => {
                omit(&mut self.omissions, "unclassified", &safe);
                None
            }
            _ => {
                let projected = object_with_references(
                    &value,
                    schema,
                    &safe,
                    policy,
                    &mut self.omissions,
                    scope,
                );
                self.required_error |= required_missing(&value, &projected, schema);
                Some(projected)
            }
        }
    }
    pub fn try_value(&mut self, path: &[String], value: Value) -> Result<Option<Value>, ApiError> {
        let projected = self.value(path, value);
        match self.error() {
            Some(error) => Err(error),
            None => Ok(projected),
        }
    }
    pub fn error(&self) -> Option<ApiError> {
        self.required_error.then(source_unprovable)
    }
    pub fn finish_checked(self) -> Result<Value, ApiError> {
        if self.required_error {
            Err(source_unprovable())
        } else {
            Ok(omission_rows(self.omissions))
        }
    }
    pub fn finish(self) -> Value {
        omission_rows(self.omissions)
    }
}

fn verified_name(message: u64, field: u64) -> Option<&'static str> {
    if field == 253 {
        return Some("timestamp");
    }
    match (message, field) {
        (20, 0) => Some("position_lat"),
        (20, 1) => Some("position_long"),
        (20, 2) => Some("altitude"),
        (20, 3) => Some("heart_rate"),
        (20, 4) => Some("cadence"),
        (20, 5) => Some("distance"),
        (20, 6) => Some("speed"),
        (20, 7) => Some("power"),
        (20, 13) => Some("temperature"),
        (20, 53) => Some("fractional_cadence"),
        (20, 73) => Some("enhanced_speed"),
        (20, 78) => Some("enhanced_altitude"),
        (18 | 19, 2) => Some("start_time"),
        (18 | 19, 7) => Some("total_elapsed_time"),
        (18 | 19, 8) => Some("total_timer_time"),
        (18 | 19, 9) => Some("total_distance"),
        (18, 14) | (19, 13) => Some("avg_speed"),
        (18, 16) | (19, 15) => Some("avg_heart_rate"),
        (18, 18) | (19, 17) => Some("avg_cadence"),
        (18, 20) | (19, 19) => Some("avg_power"),
        (78, 0) => Some("time"),
        (132, 9) => Some("event_timestamp"),
        (132, 10) => Some("event_timestamp_12"),
        (0 | 23, 3) => Some("serial_number"),
        _ => None,
    }
}

fn safe_reference(reference: &Value, category: &str) -> bool {
    if reference
        .get("developerIdentity")
        .is_some_and(|d| !d.is_null())
    {
        return false;
    }
    reference
        .get("globalMessageNumber")
        .and_then(Value::as_u64)
        .zip(reference.get("fieldNumber").and_then(Value::as_u64))
        .is_some_and(|(message, field)| {
            native_category(message, field) == Some(category)
                && reference
                    .get("componentParent")
                    .filter(|value| !value.is_null())
                    .is_none_or(|value| {
                        value.as_u64().is_some_and(|parent| {
                            native_category(message, parent) == Some(category)
                        })
                    })
        })
}
