use chrono::SecondsFormat;
use fitparser::Value;
use serde_json::{Number, Value as JsonValue};

use crate::{
    error::FitError,
    model::{RawFitField, RawFitRecord},
};

const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

pub fn decode_raw(bytes: &[u8]) -> Result<Vec<RawFitRecord>, FitError> {
    fitparser::from_bytes(bytes)
        .map(|records| {
            records
                .into_iter()
                .map(|record| RawFitRecord {
                    kind: record.kind().to_string(),
                    fields: record
                        .fields()
                        .iter()
                        .map(|field| RawFitField {
                            name: field.name().into(),
                            value: json_value(field.value()),
                            units: (!field.units().is_empty()).then(|| field.units().into()),
                        })
                        .collect(),
                })
                .collect()
        })
        .map_err(|error| {
            tracing::debug!(error = %error, "FIT decoding failed");
            FitError::InvalidFit
        })
}

fn json_value(value: &Value) -> JsonValue {
    match value {
        Value::Timestamp(value) => {
            JsonValue::String(value.to_utc().to_rfc3339_opts(SecondsFormat::Millis, true))
        }
        Value::Byte(value) => integer_json(*value as i64),
        Value::Enum(value) => integer_json(*value as i64),
        Value::SInt8(value) => integer_json(*value as i64),
        Value::UInt8(value) => integer_json(*value as i64),
        Value::SInt16(value) => integer_json(*value as i64),
        Value::UInt16(value) => integer_json(*value as i64),
        Value::SInt32(value) => integer_json(*value as i64),
        Value::UInt32(value) => integer_json(*value as i64),
        Value::UInt8z(value) => integer_json(*value as i64),
        Value::UInt16z(value) => integer_json(*value as i64),
        Value::UInt32z(value) => integer_json(*value as i64),
        Value::SInt64(value) => integer_json(*value),
        Value::UInt64(value) => unsigned_integer_json(*value),
        Value::UInt64z(value) => unsigned_integer_json(*value),
        Value::Float32(value) if value.is_finite() => Number::from_f64(*value as f64)
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null),
        Value::Float64(value) if value.is_finite() => Number::from_f64(*value)
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null),
        Value::Float32(_) | Value::Float64(_) | Value::Invalid => JsonValue::Null,
        Value::String(value) => JsonValue::String(value.clone()),
        Value::Array(values) => JsonValue::Array(values.iter().map(json_value).collect()),
    }
}

fn integer_json(value: i64) -> JsonValue {
    if (-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&value) {
        JsonValue::Number(value.into())
    } else {
        JsonValue::String(value.to_string())
    }
}

fn unsigned_integer_json(value: u64) -> JsonValue {
    if value <= MAX_SAFE_INTEGER as u64 {
        JsonValue::Number(value.into())
    } else {
        JsonValue::String(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Local, TimeZone, Timelike, Utc};
    use fitparser::Value;
    use serde_json::json;

    use super::json_value;

    #[test]
    fn converter_preserves_every_safe_integer_variant_as_a_json_number() {
        let values = [
            (Value::Byte(1), json!(1)),
            (Value::Enum(2), json!(2)),
            (Value::SInt8(-3), json!(-3)),
            (Value::UInt8(4), json!(4)),
            (Value::SInt16(-5), json!(-5)),
            (Value::UInt16(6), json!(6)),
            (Value::SInt32(-7), json!(-7)),
            (Value::UInt32(8), json!(8)),
            (Value::UInt8z(9), json!(9)),
            (Value::UInt16z(10), json!(10)),
            (Value::UInt32z(11), json!(11)),
            (
                Value::SInt64(-9_007_199_254_740_991),
                json!(-9_007_199_254_740_991_i64),
            ),
            (
                Value::UInt64(9_007_199_254_740_991),
                json!(9_007_199_254_740_991_u64),
            ),
            (Value::UInt64z(12), json!(12)),
        ];

        for (value, expected) in values {
            assert_eq!(json_value(&value), expected);
        }
    }

    #[test]
    fn converter_emits_unsafe_integers_as_decimal_strings() {
        assert_eq!(
            json_value(&Value::SInt64(-9_007_199_254_740_992)),
            json!("-9007199254740992")
        );
        assert_eq!(
            json_value(&Value::UInt64(9_007_199_254_740_992)),
            json!("9007199254740992")
        );
        assert_eq!(
            json_value(&Value::UInt64z(u64::MAX)),
            json!("18446744073709551615")
        );
    }

    #[test]
    fn converter_formats_timestamps_in_utc_with_three_fractional_digits() {
        let timestamp = Utc
            .with_ymd_and_hms(2026, 7, 19, 12, 34, 56)
            .single()
            .expect("valid timestamp")
            .with_nanosecond(789_000_000)
            .expect("valid nanoseconds")
            .with_timezone(&Local);

        assert_eq!(
            json_value(&Value::Timestamp(timestamp)),
            json!("2026-07-19T12:34:56.789Z")
        );
    }

    #[test]
    fn converter_handles_floats_strings_arrays_and_invalid_values() {
        assert_eq!(json_value(&Value::Float32(1.25)), json!(1.25));
        assert_eq!(json_value(&Value::Float64(-2.5)), json!(-2.5));
        assert_eq!(
            json_value(&Value::Float32(f32::NAN)),
            serde_json::Value::Null
        );
        assert_eq!(
            json_value(&Value::Float64(f64::INFINITY)),
            serde_json::Value::Null
        );
        assert_eq!(json_value(&Value::String("run".into())), json!("run"));
        assert_eq!(
            json_value(&Value::Array(vec![
                Value::UInt64(9_007_199_254_740_992),
                Value::Invalid,
                Value::Array(vec![Value::SInt8(-1)]),
            ])),
            json!(["9007199254740992", null, [-1]])
        );
        assert_eq!(json_value(&Value::Invalid), serde_json::Value::Null);
    }
}
