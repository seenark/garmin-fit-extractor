use chrono::SecondsFormat;
use fitparser::Value;
use serde_json::{Number, Value as JsonValue};
use std::collections::BTreeMap;

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
                .map(|record| raw_record(&record))
                .collect()
        })
        .map_err(|_| FitError::InvalidFit)
}

pub(super) fn raw_record(record: &fitparser::FitDataRecord) -> RawFitRecord {
    RawFitRecord {
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
    }
}

pub(super) fn json_value(value: &Value) -> JsonValue {
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

pub(crate) fn verified_wire(field: &JsonValue, expected: i64) -> bool {
    // Decoded native fields use baseType; normalized native extensions use type.
    let base = field.get("baseType").filter(|value| !value.is_null());
    let extension = field.get("type").filter(|value| !value.is_null());
    base.or(extension).and_then(JsonValue::as_i64) == Some(expected)
        && base
            .into_iter()
            .chain(extension)
            .all(|value| value.as_i64() == Some(expected))
}

struct NativeProfileSpec {
    wire: i64,
    timestamp: bool,
    reconstructed: bool,
    scale: f64,
    offset: f64,
    components: BTreeMap<u64, (f64, f64)>,
}

fn native_profile(message: u64, number: u64) -> Option<&'static NativeProfileSpec> {
    use fitparser::{Value as FitValue, de::DecodeOption, profile::MesgNum};
    use std::collections::{HashMap, HashSet};
    static PROFILE: std::sync::LazyLock<BTreeMap<(u64, u64), NativeProfileSpec>> =
        std::sync::LazyLock::new(|| {
            let mut profile = BTreeMap::new();
            let options = HashSet::from([DecodeOption::KeepCompositeFields]);
            let messages = [
                0, 2, 7, 8, 9, 10, 12, 18, 19, 20, 21, 23, 34, 53, 78, 131, 132, 216,
            ];
            // Only actual pinned profile metadata authorizes a candidate field. Invalid
            // probes avoid converting values or choosing an untrusted subfield.
            for message in messages {
                for number in 0..=u8::MAX as u64 {
                    let mut values = HashMap::from([(number as u8, FitValue::Invalid)]);
                    let Some(decoded) = MesgNum::from(message as u16)
                        .decode_message(&mut values, &mut HashMap::new(), &options)
                        .ok()
                        .and_then(|fields| {
                            fields.into_iter().find(|field| {
                                field.number() == number as u8 && field.component_parent().is_none()
                            })
                        })
                    else {
                        continue;
                    };
                    let (Some(kind), Some(scale), Some(offset)) =
                        (decoded.profile_type(), decoded.scale(), decoded.offset())
                    else {
                        continue;
                    };
                    let timestamp = matches!(kind, "date_time" | "local_date_time");
                    let Some(wire) = primitive_wire(kind).or_else(|| {
                        timestamp.then_some(
                            fitparser::profile::field_types::FitBaseType::Uint32.as_i64(),
                        )
                    }) else {
                        continue;
                    };
                    profile.insert(
                        (message, number),
                        NativeProfileSpec {
                            wire,
                            timestamp,
                            reconstructed: kind == "date_time",
                            scale,
                            offset,
                            components: BTreeMap::new(),
                        },
                    );
                }
            }
            // Eight zero bytes exercise the pinned component declarations, not archive
            // names or role tags. Components extract/accumulate UInt64, and can exceed
            // a native UInt32 anchor's range or use a different scale.
            for message in messages {
                for parent in 0..=u8::MAX as u64 {
                    if !profile.contains_key(&(message, parent)) {
                        continue;
                    }
                    let mut values = HashMap::from([(parent as u8, FitValue::UInt64(0))]);
                    let Ok(fields) = MesgNum::from(message as u16).decode_message(
                        &mut values,
                        &mut HashMap::new(),
                        &options,
                    ) else {
                        continue;
                    };
                    for field in fields {
                        if field.component_parent() != Some(parent as u8) {
                            continue;
                        }
                        let Some(spec) = profile.get_mut(&(message, field.number() as u64)) else {
                            continue;
                        };
                        if field.profile_type().and_then(primitive_wire) != Some(spec.wire) {
                            continue;
                        }
                        if let (Some(scale), Some(offset)) = (field.scale(), field.offset()) {
                            spec.components.insert(parent, (scale, offset));
                        }
                    }
                }
            }
            profile
        });
    PROFILE.get(&(message, number))
}

fn primitive_wire(kind: &str) -> Option<i64> {
    use fitparser::profile::field_types::FitBaseType;
    matches!(
        kind,
        "sint8"
            | "uint8"
            | "sint16"
            | "uint16"
            | "sint32"
            | "uint32"
            | "sint64"
            | "uint64"
            | "uint8z"
            | "uint16z"
            | "uint32z"
            | "uint64z"
            | "float32"
            | "float64"
            | "byte"
    )
    .then(|| FitBaseType::from(kind).as_i64())
}

fn primitive_bounds(wire: i64) -> Option<(i128, i128)> {
    Some(match wire {
        1 => (i8::MIN as i128, i8::MAX as i128),
        2 | 10 | 13 => (0, u8::MAX as i128),
        131 => (i16::MIN as i128, i16::MAX as i128),
        132 | 139 => (0, u16::MAX as i128),
        133 => (i32::MIN as i128, i32::MAX as i128),
        134 | 140 => (0, u32::MAX as i128),
        142 => (i64::MIN as i128, i64::MAX as i128),
        143 | 144 => (0, u64::MAX as i128),
        _ => return None,
    })
}

fn primitive_raw(value: &JsonValue, wire: i64) -> bool {
    if value.is_null() {
        return true;
    }
    if let Some((minimum, maximum)) = primitive_bounds(wire) {
        let integer = value
            .as_i64()
            .map(i128::from)
            .or_else(|| value.as_u64().map(i128::from))
            .or_else(|| {
                value
                    .as_str()
                    .filter(|_| matches!(wire, 142..=144) && numeric_value(value))
                    .and_then(|text| text.parse::<i128>().ok())
            });
        return integer.is_some_and(|integer| {
            integer >= minimum
                && integer <= maximum
                && (value.is_string() || integer.unsigned_abs() <= 9_007_199_254_740_991)
        });
    }
    value.as_f64().is_some_and(|number| {
        number.is_finite() && (wire == 137 || (wire == 136 && (number as f32) as f64 == number))
    })
}

fn primitive_decoded(value: &JsonValue, wire: i64, scale: f64, offset: f64) -> bool {
    if value.is_null() {
        return true;
    }
    if scale == 1.0 && offset == 0.0 {
        return primitive_raw(value, wire);
    }
    let Some(number) = value.as_f64().filter(|number| number.is_finite()) else {
        return false;
    };
    let (minimum, maximum) = primitive_bounds(wire)
        .map(|(minimum, maximum)| (minimum as f64, maximum as f64))
        .unwrap_or_else(|| {
            if wire == 136 {
                (-(f32::MAX as f64), f32::MAX as f64)
            } else {
                (-f64::MAX, f64::MAX)
            }
        });
    let low = minimum / scale - offset;
    let high = maximum / scale - offset;
    number >= low.min(high) && number <= low.max(high)
}

fn primitive_pair(raw: &JsonValue, value: &JsonValue, wire: i64, spec: &NativeProfileSpec) -> bool {
    if !primitive_raw(raw, wire) || !primitive_decoded(value, wire, spec.scale, spec.offset) {
        return false;
    }
    if raw.is_null() {
        return value.is_null();
    }
    // Byte's 0xff can remain in rawValue while the archive projects its invalid
    // decoded element as null. Typed invalid integers arrive as raw null.
    if value.is_null() {
        return wire == 13 && raw.as_u64() == Some(255);
    }
    if spec.scale == 1.0 && spec.offset == 0.0 {
        return raw == value;
    }
    raw.as_f64()
        .zip(value.as_f64())
        .is_some_and(|(raw, value)| raw / spec.scale - spec.offset == value)
}

pub(crate) fn verified_primitive(
    message: u64,
    number: u64,
    field: &JsonValue,
    reconstructed: bool,
    semantic: bool,
) -> bool {
    // Enum and dependent subfield fidelity is checked separately.
    if semantic {
        return !reconstructed
            && field["role"] != "expanded"
            && field["componentParent"].is_null()
            && field["sourceReference"]["componentParent"].is_null();
    }
    let Some(spec) = native_profile(message, number) else {
        return false;
    };
    let wire = spec.wire;
    if reconstructed {
        return spec.reconstructed;
    }
    let parent = field
        .get("componentParent")
        .or_else(|| field["sourceReference"].get("componentParent"))
        .and_then(JsonValue::as_u64);
    let expanded = field["role"] == "expanded";
    let component = if expanded {
        parent.and_then(|parent| spec.components.get(&parent))
    } else {
        None
    };
    if expanded && component.is_none() {
        return false;
    }
    let metadata = field
        .get("baseType")
        .filter(|value| !value.is_null())
        .into_iter()
        .chain(field.get("type").filter(|value| !value.is_null()));
    let mut present = false;
    for value in metadata {
        present = true;
        if value.as_i64() != Some(wire)
            && !(expanded
                && !spec.timestamp
                && value.as_str().and_then(primitive_wire) == Some(wire))
        {
            return false;
        }
    }
    if !present {
        return false;
    }
    let value = &field["value"];
    let flat = |value: &JsonValue, check: &dyn Fn(&JsonValue) -> bool| {
        check(value)
            || value
                .as_array()
                .is_some_and(|items| items.iter().all(check))
    };
    if let Some(&(scale, offset)) = component {
        return field["rawValue"].is_null()
            && flat(value, &|value| {
                (!value.is_string() || matches!(wire, 142..=144))
                    && primitive_decoded(value, 143, scale, offset)
            });
    }
    if spec.timestamp && value.is_string() {
        return field
            .get("rawValue")
            .is_none_or(|raw| primitive_raw(raw, wire) && !raw.is_null());
    }
    if !flat(value, &|value| {
        primitive_decoded(value, wire, spec.scale, spec.offset)
    }) {
        return false;
    }
    let Some(raw) = field.get("rawValue") else {
        return true;
    };
    match (raw.as_array(), value.as_array()) {
        (Some(raw), Some(values)) => {
            raw.len() == values.len()
                && raw
                    .iter()
                    .zip(values)
                    .all(|(raw, value)| primitive_pair(raw, value, wire, spec))
        }
        (None, None) => primitive_pair(raw, value, wire, spec),
        _ => false,
    }
}

pub(crate) fn numeric_wire(value: Option<&JsonValue>) -> bool {
    use fitparser::profile::field_types::FitBaseType;
    match value {
        None | Some(JsonValue::Null) => true,
        Some(value) => {
            value
                .as_i64()
                .is_some_and(|code| code != 7 && FitBaseType::is_named_variant(code))
                || value.as_str().is_some_and(|name| {
                    matches!(
                        name,
                        "enum"
                            | "sint8"
                            | "uint8"
                            | "sint16"
                            | "uint16"
                            | "sint32"
                            | "uint32"
                            | "sint64"
                            | "uint64"
                            | "uint8z"
                            | "uint16z"
                            | "uint32z"
                            | "uint64z"
                            | "float32"
                            | "float64"
                            | "byte"
                    )
                })
        }
    }
}

pub(crate) fn numeric_value(value: &JsonValue) -> bool {
    fn number(value: &JsonValue) -> bool {
        value.is_null()
            || value.is_number()
            || value.as_str().is_some_and(|text| {
                // The archive uses decimal strings only for integers outside JSON's exact range.
                let digits = text.strip_prefix('-').unwrap_or(text);
                !digits.is_empty()
                    && digits.bytes().all(|byte| byte.is_ascii_digit())
                    && !digits.starts_with('0')
                    && text.parse::<i128>().is_ok_and(|n| {
                        n >= i128::from(i64::MIN)
                            && n <= i128::from(u64::MAX)
                            && n.unsigned_abs() > 9_007_199_254_740_991
                    })
            })
    }
    number(value)
        || value
            .as_array()
            .is_some_and(|items| items.iter().all(number))
}

fn native_array_capable(message: u64, number: u64) -> bool {
    // Factual array flags for the callback-covered messages, not SDK runtime code.
    // Source: garmin/fit-python-sdk tag 21.202.0, garmin_fit_sdk/profile.py;
    // commit 06bd8910bcf84d832d6cc98071a467f5a13e2339,
    // SHA256 43168b75f7db62c3f16dfd68fbf54722048eeedf6d4fbfa21fd50affaaff9eb4.
    // The MIT-generated callbacks omit dimensions. Boolean declarations establish
    // array capability only, not fixed counts or SDK behavioral parity.
    matches!(
        (message, number),
        (0, 8)
            | (2, 2 | 4 | 5 | 40 | 57)
            | (8, 2)
            | (9, 2)
            | (12, 3)
            | (
                18,
                65 | 66
                    | 67
                    | 68
                    | 84
                    | 85
                    | 86
                    | 95
                    | 96
                    | 97
                    | 98
                    | 99
                    | 100
                    | 110
                    | 116
                    | 117
                    | 118
                    | 119
                    | 120
                    | 121
                    | 122
                    | 123
            )
            | (
                19,
                57 | 58
                    | 59
                    | 60
                    | 75
                    | 76
                    | 84
                    | 85
                    | 86
                    | 87
                    | 88
                    | 89
                    | 102
                    | 103
                    | 104
                    | 105
                    | 106
                    | 107
                    | 108
                    | 109
            )
            | (20, 8 | 17 | 69 | 70 | 71 | 72)
            | (23, 19 | 27)
            | (53, 1)
            | (78, 0)
            | (131, 1)
            | (132, 6 | 9 | 10)
            | (216, 2..=9)
    )
}
pub(crate) fn enum_profile(
    message: u64,
    number: u64,
) -> Option<(&'static [fitparser::profile::FieldDataType], i64)> {
    use fitparser::profile::FieldDataType as T;
    // Profile field/subfield types from the pinned FIT 21.202 profile. Supplied
    // names, classifications, and profileType strings never authorize values.
    let (types, wire): (&[T], i64) = match (message, number) {
        (0, 0) => (&[T::File], 0),
        (0, 1) | (23, 2) => (&[T::Manufacturer], 132),
        (0, 2) | (23, 4) => (&[T::GarminProduct, T::FaveroProduct], 132),
        (2, 4) => (&[T::TimeMode], 0),
        (7, 5) => (&[T::HrZoneCalc], 0),
        (7, 7) => (&[T::PwrZoneCalc], 0),
        (8 | 9 | 53, 254) | (19, 71) => (&[T::MessageIndex], 132),
        (18 | 19 | 21, 0) | (34, 3) => (&[T::Event], 0),
        (18 | 19 | 21, 1) | (34, 4) => (&[T::EventType], 0),
        (18, 5) | (19, 25) => (&[T::Sport], 0),
        (18, 6) | (19, 39) => (&[T::SubSport], 0),
        (18, 28) => (&[T::SessionTrigger], 0),
        (18, 43) | (19, 38) => (&[T::SwimStroke], 0),
        (18, 46) => (&[T::DisplayMeasure], 0),
        (19, 23) => (&[T::Intensity], 0),
        (19, 24) => (&[T::LapTrigger], 0),
        (20, 30) => (&[T::LeftRightBalance], 2),
        (21, 3) => (
            &[
                T::TimerTrigger,
                T::FitnessEquipmentState,
                T::RiderPositionType,
                T::CommTimeoutType,
                T::DiveAlert,
                T::MessageIndex,
            ],
            134,
        ),
        (23, 0) => (&[T::DeviceIndex], 2),
        (23, 1) => (
            &[T::AntplusDeviceType, T::BleDeviceType, T::LocalDeviceType],
            2,
        ),
        (23, 25) => (&[T::SourceType], 0),
        (34, 2) => (&[T::Activity], 0),
        _ => return None,
    };
    Some((types, wire))
}

pub(crate) fn native_dependency(message: u64, number: u64) -> Option<u64> {
    match (message, number) {
        (0, 2) => Some(1),
        (23, 4) => Some(2),
        (23, 1) => Some(25),
        (21, 3) => Some(0),
        _ => None,
    }
}

pub(crate) fn native_context_dependency(message: u64, field: &JsonValue) -> Option<u64> {
    if field["role"] == "expanded" {
        field
            .get("componentParent")
            .filter(|value| !value.is_null())
            .or_else(|| field["sourceReference"].get("componentParent"))
            .and_then(JsonValue::as_u64)
    } else {
        field
            .get("fieldNumber")
            .or_else(|| field["identity"].get("fieldNumber"))
            .and_then(JsonValue::as_u64)
            .and_then(|number| native_dependency(message, number))
    }
}

pub(crate) fn native_dependency_source(number: u64, field: &JsonValue) -> bool {
    field["role"] != "expanded"
        && field["role"] != "reconstructed"
        && field
            .get("fieldNumber")
            .or_else(|| field["identity"].get("fieldNumber"))
            .and_then(JsonValue::as_u64)
            == Some(number)
        && [field, &field["identity"], &field["sourceReference"]]
            .into_iter()
            .all(|identity| {
                identity
                    .get("developerIdentity")
                    .is_none_or(JsonValue::is_null)
            })
}

fn native_number(message: u64, field: &JsonValue) -> Option<u64> {
    let number = field
        .get("fieldNumber")
        .or_else(|| field["identity"].get("fieldNumber"))?
        .as_u64()?;
    if number > u8::MAX as u64 {
        return None;
    }
    for identity in [field, &field["identity"], &field["sourceReference"]] {
        if identity
            .get("developerIdentity")
            .is_some_and(|value| !value.is_null())
        {
            return None;
        }
        if identity
            .get("globalMessageNumber")
            .filter(|value| !value.is_null())
            .is_some_and(|value| value.as_u64() != Some(message))
            || identity
                .get("fieldNumber")
                .filter(|value| !value.is_null())
                .is_some_and(|value| value.as_u64() != Some(number))
        {
            return None;
        }
    }
    Some(number)
}

fn reconstructed_timestamp(message: u64, number: u64, field: &JsonValue) -> bool {
    number == 253
        && field["role"] == "reconstructed"
        && field["rawValue"].is_null()
        && field["sourceReference"]["globalMessageNumber"].as_u64() == Some(message)
        && field["sourceReference"]["fieldNumber"].as_u64() == Some(number)
        && field["sourceReference"]["byteLength"].as_u64() == Some(1)
        && field
            .get("compressedTimeOffset")
            .filter(|value| !value.is_null())
            .is_none_or(|value| value.as_u64().is_some_and(|offset| offset <= 31))
        && [field.get("baseType"), field.get("type")]
            .into_iter()
            .flatten()
            .all(|value| value.is_null() || value.as_str() == Some("date_time"))
        && field["value"]
            .as_str()
            .is_some_and(|text| chrono::DateTime::parse_from_rfc3339(text).is_ok())
}

pub(crate) fn native_field_valid(
    message: u64,
    field: &JsonValue,
    siblings: Option<&[JsonValue]>,
) -> bool {
    let Some(number) = native_number(message, field) else {
        return false;
    };
    let Some(value) = field.get("value") else {
        return false;
    };
    if field["role"] != "expanded"
        && (value.is_array() || field["rawValue"].is_array())
        && !native_array_capable(message, number)
    {
        return false;
    }
    let reconstructed = reconstructed_timestamp(message, number, field);
    if field
        .get("role")
        .filter(|value| !value.is_null())
        .is_some_and(|value| {
            !matches!(
                value.as_str(),
                Some("native" | "expanded" | "reconstructed")
            )
        })
        || (field["role"] == "reconstructed" && !reconstructed)
    {
        return false;
    }
    if field["role"] != "expanded"
        && (!field["componentParent"].is_null()
            || !field["sourceReference"]["componentParent"].is_null())
    {
        return false;
    }
    if (!numeric_wire(field.get("baseType")) || !numeric_wire(field.get("type"))) && !reconstructed
    {
        return false;
    }
    let semantic = enum_profile(message, number);
    if !verified_primitive(message, number, field, reconstructed, semantic.is_some()) {
        return false;
    }
    if field["role"] == "expanded" {
        let Some(parent) = native_context_dependency(message, field) else {
            return false;
        };
        let Some(source) = dependency_source(message, parent, field, siblings) else {
            return false;
        };
        let Some(source) = source else {
            return false;
        };
        if !native_field_valid(message, source, siblings) {
            return false;
        }
    }
    if let Some((types, wire)) = semantic {
        if !verified_wire(field, wire) {
            return false;
        }
        if native_dependency(message, number).is_some() {
            return selected_subfield(message, number, field, siblings, types);
        }
        return enum_value(field, types, wire);
    }
    reconstructed
        || numeric_value(value)
        || value
            .as_str()
            .is_some_and(|text| chrono::DateTime::parse_from_rfc3339(text).is_ok())
}

fn enum_value(field: &JsonValue, types: &[fitparser::profile::FieldDataType], wire: i64) -> bool {
    use fitparser::profile::{FieldDataType as T, get_field_variant_as_string};
    let maximum = if types.iter().any(|kind| matches!(kind, T::MessageIndex)) {
        u16::MAX as i64
    } else {
        match wire {
            0 | 2 => u8::MAX as i64,
            132 => u16::MAX as i64,
            134 => u32::MAX as i64,
            _ => return false,
        }
    };
    let indices = types
        .iter()
        .any(|kind| matches!(kind, T::MessageIndex | T::DeviceIndex | T::LeftRightBalance));
    if let Some(text) = field["value"].as_str() {
        let code = if field["enumCode"].is_null() {
            field["rawValue"].as_i64()
        } else {
            field["enumCode"].as_i64()
        };
        return code.is_some_and(|code| {
            code >= 0
                && code < maximum
                && field["rawValue"].as_i64() == Some(code)
                && types.iter().any(|kind| {
                    kind.is_named_variant(code) && get_field_variant_as_string(*kind, code) == text
                })
        });
    }
    if field["rawValue"] != field["value"] {
        return false;
    }
    let valid = |value: &JsonValue| {
        value.is_null()
            || value.as_i64().is_some_and(|code| {
                code >= 0
                    && code < maximum
                    && (indices || types.iter().any(|kind| kind.is_named_variant(code)))
            })
    };
    valid(&field["value"])
        || field["value"]
            .as_array()
            .is_some_and(|items| items.iter().all(valid))
}

fn dependency_source<'a>(
    message: u64,
    dependency: u64,
    field: &'a JsonValue,
    siblings: Option<&'a [JsonValue]>,
) -> Option<Option<&'a JsonValue>> {
    let context = if let Some(siblings) = siblings {
        siblings
    } else {
        let context = field.get("nativeContext")?.as_array()?;
        if context.len() > 1
            || context.iter().any(|source| {
                native_number(message, source) != Some(dependency)
                    || !native_dependency_source(dependency, source)
            })
        {
            return None;
        }
        context
    };
    let mut dependencies = context
        .iter()
        .filter(|source| native_dependency_source(dependency, source));
    let source = dependencies.next();
    dependencies.next().is_none().then_some(source)
}

fn raw_numeric(value: &JsonValue) -> Option<Value> {
    if value.is_null() {
        Some(Value::Invalid)
    } else if let Some(number) = value.as_i64() {
        Some(Value::SInt64(number))
    } else {
        value
            .as_array()
            .and_then(|values| values.iter().map(raw_numeric).collect::<Option<Vec<_>>>())
            .map(Value::Array)
    }
}

fn selected_subfield(
    message: u64,
    number: u64,
    field: &JsonValue,
    siblings: Option<&[JsonValue]>,
    types: &[fitparser::profile::FieldDataType],
) -> bool {
    use fitparser::profile::MesgNum;
    use std::collections::{HashMap, HashSet};
    let Some(dependency) = native_dependency(message, number) else {
        return false;
    };
    let Some(source) = dependency_source(message, dependency, field, siblings) else {
        return false;
    };
    let Some(raw) = raw_numeric(&field["rawValue"]) else {
        return false;
    };
    let mut values = HashMap::from([(number as u8, raw)]);
    if let Some(source) = source {
        if !native_field_valid(message, source, siblings) {
            return false;
        }
        let Some(raw) = raw_numeric(&source["rawValue"]) else {
            return false;
        };
        values.insert(dependency as u8, raw);
    }
    let Some(decoded) = MesgNum::from(message as u16)
        .decode_message(&mut values, &mut HashMap::new(), &HashSet::new())
        .ok()
        .and_then(|fields| {
            fields
                .into_iter()
                .find(|candidate| candidate.number() == number as u8)
        })
    else {
        return false;
    };
    if let Some(kind) = types
        .iter()
        .find(|kind| Some(kind.as_str()) == decoded.profile_type())
    {
        return enum_value(
            field,
            std::slice::from_ref(kind),
            enum_profile(message, number).unwrap().1,
        ) && matches_native_value(&field["value"], decoded.value());
    }
    let Some(wire) = decoded.profile_type().and_then(primitive_wire) else {
        return false;
    };
    let valid = |value: &JsonValue| primitive_raw(value, wire);
    (valid(&field["rawValue"])
        || field["rawValue"]
            .as_array()
            .is_some_and(|values| values.iter().all(valid)))
        && matches_native_value(&field["value"], decoded.value())
}

fn matches_native_value(actual: &JsonValue, expected: &Value) -> bool {
    match expected {
        Value::Invalid => actual.is_null(),
        Value::String(text) => actual.as_str() == Some(text),
        Value::Float64(number) => actual
            .as_f64()
            .is_some_and(|actual| actual.to_bits() == number.to_bits()),
        Value::Float32(number) => actual
            .as_f64()
            .is_some_and(|actual| actual.to_bits() == f64::from(*number).to_bits()),
        Value::Array(values) => actual.as_array().is_some_and(|actual| {
            actual.len() == values.len()
                && actual
                    .iter()
                    .zip(values)
                    .all(|(actual, expected)| matches_native_value(actual, expected))
        }),
        _ => {
            let number: Result<i64, _> = expected.try_into();
            number.is_ok_and(|number| actual.as_i64() == Some(number))
        }
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
