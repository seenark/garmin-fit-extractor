use garmin_fit_extractor_api::{error::FitError, fit::runs::decode_run_to_writer};
use serde_json::{Value, json};

const RUN: &[u8] = include_bytes!("fixtures/runs/garmin_run.fit");
const PACKED: &[u8] = include_bytes!("fixtures/runs/garmin_packed_hr.fit");

#[derive(Debug, serde::Deserialize)]
struct DecodedRun {
    decoded: Value,
    normalized: Value,
}

// Consume the public writer as a small-fixture JSON client, not a second normalizer.
fn decode_run(bytes: &[u8]) -> Result<DecodedRun, FitError> {
    let mut output = Vec::new();
    decode_run_to_writer(bytes, &mut output)?;
    Ok(serde_json::from_slice(&output).expect("public writer emits the documented JSON document"))
}

fn field(decoded: &Value, global: u64, number: u64) -> &Value {
    decoded["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|message| message["globalMessageNumber"] == global)
        .flat_map(|message| message["fields"].as_array().unwrap())
        .find(|field| field["fieldNumber"] == number)
        .unwrap()
}

#[test]
fn native_packed_beats_use_real_counter_anchor_not_sampled_hr() {
    let run = decode_run(PACKED).unwrap();
    let rr = &run.normalized["rr"];
    assert_eq!(rr["alignmentEligible"], true);
    let intervals = rr["intervals"].as_array().unwrap();
    assert_eq!(
        intervals
            .iter()
            .map(|interval| interval["rrMs"].clone())
            .collect::<Vec<_>>(),
        vec![json!(1000.0), json!(1000.0)]
    );
    assert_eq!(intervals[0]["endElapsedSeconds"], 1.5);
    assert_eq!(intervals[0]["startElapsedSeconds"], 0.5);
    assert_eq!(intervals[1]["endElapsedSeconds"], 2.5);
    assert_eq!(intervals[0]["provenance"]["kind"], "recorded_rr");
    assert_eq!(intervals[0]["sourceReference"]["globalMessageNumber"], 132);
    let packed = field(&run.decoded, 132, 10);
    assert_eq!(packed["role"], "native");
    assert!(
        run.decoded["messages"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|message| message["fields"].as_array().unwrap())
            .any(|field| field["componentParent"] == 10
                && field["value"] == json!([101.0, 102.0])
                && field["role"] == "expanded")
    );
}

#[test]
fn running_metrics_preserve_resolution_precedence_and_cadence_units() {
    let run = decode_run(RUN).unwrap();
    let samples = run.normalized["samples"].as_array().unwrap();
    assert_eq!(samples[0]["speedMps"], 2.0);
    assert_eq!(samples[0]["paceSecondsPerKm"], 500.0);
    assert_eq!(samples[1]["speedMps"], 4.0);
    assert_eq!(samples[2]["speedMps"], 0.0);
    assert_eq!(samples[2]["paceSecondsPerKm"], Value::Null);
    assert_eq!(samples[0]["cadenceStepsPerMinute"], 170.0);
    assert_eq!(
        samples[0]["sourceReferences"]["speedMps"]["fieldNumber"],
        73
    );
    assert_eq!(run.normalized["summary"]["distanceMeters"], 1000.0);
    assert_eq!(run.normalized["summary"]["timerTimeSeconds"], 300.0);
    assert_eq!(run.normalized["summary"]["elapsedTimeSeconds"], 360.0);
    assert_eq!(run.normalized["summary"]["movingTimeSeconds"], Value::Null);
    let summary = &run.normalized["summary"];
    assert!((summary["derived"]["averageSpeedMps"].as_f64().unwrap() - 10.0 / 3.0).abs() < 1e-12);
    assert_eq!(
        summary["coverage"]["averageSpeedMps"]["coveredSeconds"],
        30.0
    );
    assert_eq!(
        summary["coverage"]["averageSpeedMps"]["windowSeconds"],
        360.0
    );
    assert_eq!(summary["method"]["gapThresholdSeconds"], 60.0);
    assert_eq!(
        samples
            .iter()
            .map(|sample| sample["elapsedSeconds"].clone())
            .collect::<Vec<_>>(),
        vec![json!(0.0), json!(10.0), json!(30.0), json!(360.0)]
    );
    assert_eq!(
        run.normalized["laps"][0]["summary"]["timerTimeSeconds"],
        120.0
    );
    assert_eq!(
        run.normalized["laps"][1]["summary"]["timerTimeSeconds"],
        180.0
    );
    assert_eq!(
        run.normalized["session"]["sourceLocalTimeContext"]["timeZoneOffsetsHours"],
        json!([8.0])
    );
}

#[test]
fn invalid_fit_never_returns_decoded_prefix() {
    for bytes in [b"not FIT".as_slice(), &RUN[..RUN.len() - 2]] {
        assert!(matches!(decode_run(bytes), Err(FitError::InvalidFit)));
    }
    let mut corrupt = RUN.to_vec();
    *corrupt.last_mut().unwrap() ^= 255;
    assert!(matches!(decode_run(&corrupt), Err(FitError::InvalidFit)));
}
#[test]
fn unsupported_activity_and_session_layout_have_safe_deterministic_reasons() {
    for (bytes, code) in [
        (
            include_bytes!("fixtures/runs/non_garmin_run.fit").as_slice(),
            "UNSUPPORTED_MANUFACTURER",
        ),
        (
            include_bytes!("fixtures/runs/cycling.fit").as_slice(),
            "UNSUPPORTED_SPORT",
        ),
        (
            include_bytes!("fixtures/runs/multiple_sessions.fit").as_slice(),
            "UNSUPPORTED_SESSION_LAYOUT",
        ),
    ] {
        match decode_run(bytes) {
            Err(FitError::UnsupportedRun { code: actual, .. }) => assert_eq!(actual, code),
            other => panic!("expected unsupported {code}, got {other:?}"),
        }
    }
}

#[test]
fn archive_preserves_invalid_zero_arrays_exact_integers_and_enum_identity() {
    let run = decode_run(include_bytes!("fixtures/runs/garmin_archive.fit")).unwrap();
    let decoded = &run.decoded;
    assert_eq!(field(decoded, 65280, 0)["value"], json!([0, null, 42]));
    assert_eq!(
        field(decoded, 65280, 0)["elementValidity"],
        json!(["valid", "invalid", "valid"])
    );
    assert_eq!(field(decoded, 65280, 1)["value"], Value::Null);
    assert_eq!(field(decoded, 65280, 1)["validity"], "invalid");
    assert_eq!(field(decoded, 65280, 2)["value"], "9007199254740993");
    assert_eq!(field(decoded, 65280, 3)["value"], "-9007199254740993");
    assert_eq!(field(decoded, 65280, 4)["value"], Value::Null);
    assert_eq!(field(decoded, 65280, 5)["value"], json!([null, null]));
    assert_eq!(field(decoded, 18, 5)["rawValue"], 1);
    assert_eq!(field(decoded, 18, 5)["value"], "running");
    assert_eq!(field(decoded, 18, 5)["enumCode"], 1);
    assert_eq!(field(decoded, 20, 240)["value"], 54321);
    assert_eq!(field(decoded, 20, 241)["value"], Value::Null);
    assert_eq!(field(decoded, 65281, 0)["value"], json!([1.25, null, null]));
    assert_eq!(field(decoded, 65281, 1)["value"], Value::Null);
    assert_eq!(field(decoded, 65281, 2)["value"], "abc");
    assert_eq!(field(decoded, 65281, 3)["value"], json!([0, null, 17]));
    assert_eq!(
        field(decoded, 65281, 3)["elementValidity"],
        json!(["valid", "invalid", "valid"])
    );
    assert_eq!(field(decoded, 65281, 4)["value"], Value::Null);
    let zero = decoded["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|message| message["globalMessageNumber"] == 20)
        .flat_map(|message| message["fields"].as_array().unwrap())
        .find(|field| field["fieldNumber"] == 3 && field["value"] == 0)
        .unwrap();
    assert_eq!(zero["validity"], "valid");
    let serialized = serde_json::to_string(decoded).unwrap();
    let restored: Value = serde_json::from_str(&serialized).unwrap();
    assert_eq!(field(&restored, 65280, 2)["value"], "9007199254740993");
}

#[test]
fn developer_collision_preserves_three_sources_and_application_metadata() {
    let run = decode_run(include_bytes!("fixtures/runs/garmin_archive.fit")).unwrap();
    let collision = run.decoded["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|message| {
            message["fields"]
                .as_array()
                .unwrap()
                .iter()
                .any(|field| field["developerIdentity"]["developerDataIndex"] == 0)
        })
        .unwrap();
    let heart_rates = collision["fields"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|field| field["fieldNumber"] == 3)
        .collect::<Vec<_>>();
    let native = heart_rates
        .iter()
        .find(|field| field["developerIdentity"].is_null())
        .unwrap();
    let first = heart_rates
        .iter()
        .find(|field| field["developerIdentity"]["developerDataIndex"] == 0)
        .unwrap();
    let second = heart_rates
        .iter()
        .find(|field| field["developerIdentity"]["developerDataIndex"] == 1)
        .unwrap();
    assert_eq!(native["value"], 120);
    assert_eq!(first["value"], 121.0);
    assert_eq!(second["value"], 122.0);
    assert_eq!(first["scale"], 2.0);
    assert_eq!(first["offset"], 3.0);
    assert_eq!(second["scale"], 4.0);
    assert_eq!(second["offset"], -2.0);
    assert_ne!(
        first["developerIdentity"]["applicationId"],
        second["developerIdentity"]["applicationId"]
    );
    assert_eq!(first["classification"], "unclassified");
    assert_eq!(
        first["sourceReference"]["developerIdentity"]["fieldDefinitionNumber"],
        3
    );
}

#[test]
fn native_hrv_keeps_invalid_elements_without_invented_alignment() {
    let run = decode_run(include_bytes!("fixtures/runs/garmin_archive.fit")).unwrap();
    let unanchored = run.normalized["rr"]["intervals"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|interval| interval["sourceReference"]["globalMessageNumber"] == 78)
        .collect::<Vec<_>>();
    assert_eq!(
        unanchored
            .iter()
            .map(|interval| interval["rrMs"].clone())
            .collect::<Vec<_>>(),
        vec![json!(800.0), Value::Null, json!(810.0)]
    );
    assert!(
        unanchored
            .iter()
            .all(|interval| interval["timestamp"].is_null() && interval["timingEligible"] == false)
    );
    assert_eq!(run.normalized["rr"]["alignmentEligible"], false);
}

#[test]
fn partial_packed_bits_never_create_beats_and_counter_rollover_keeps_zero() {
    for (bytes, expected) in [
        (
            include_bytes!("fixtures/runs/garmin_packed_hr_short_1.fit").as_slice(),
            json!([]),
        ),
        (
            include_bytes!("fixtures/runs/garmin_packed_hr_short_2.fit").as_slice(),
            json!([1000.0]),
        ),
        (
            include_bytes!("fixtures/runs/garmin_packed_hr_short_4.fit").as_slice(),
            json!([1000.0, 1000.0]),
        ),
        (
            include_bytes!("fixtures/runs/garmin_packed_hr_rollover.fit").as_slice(),
            json!([500.0, 1000.0]),
        ),
        (
            include_bytes!("fixtures/runs/garmin_packed_hr_u32_rollover.fit").as_slice(),
            json!([500.0, 1000.0]),
        ),
    ] {
        let run = decode_run(bytes).unwrap();
        let actual = run.normalized["rr"]["intervals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|interval| interval["rrMs"].clone())
            .collect::<Vec<_>>();
        assert_eq!(json!(actual), expected);
    }
    let unanchored = decode_run(include_bytes!(
        "fixtures/runs/garmin_packed_hr_unanchored.fit"
    ))
    .unwrap();
    assert_eq!(unanchored.normalized["rr"]["intervals"][0]["rrMs"], 1000.0);
    assert_eq!(
        unanchored.normalized["rr"]["intervals"][0]["timestamp"],
        Value::Null
    );
    assert_eq!(
        unanchored.normalized["rr"]["intervals"][0]["timingEligible"],
        false
    );
    assert_eq!(unanchored.normalized["rr"]["alignmentEligible"], false);
}

#[test]
fn compressed_timestamp_rollover_preserves_header_source_and_wire_order() {
    let run = decode_run(include_bytes!("fixtures/runs/garmin_archive.fit")).unwrap();
    let fields = run.decoded["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|message| message["globalMessageNumber"] == 20)
        .flat_map(|message| message["fields"].as_array().unwrap())
        .filter(|field| field["role"] == "reconstructed" && field["fieldNumber"] == 253)
        .collect::<Vec<_>>();
    assert_eq!(
        fields
            .iter()
            .map(|field| field["value"].clone())
            .collect::<Vec<_>>(),
        vec![
            json!("2021-09-08T01:47:12.000Z"),
            json!("2021-09-08T01:47:14.000Z")
        ]
    );
    assert_eq!(fields[0]["sourceReference"]["byteLength"], 1);
    assert_eq!(fields[0]["compressedTimeOffset"], 0);
    assert_eq!(fields[1]["compressedTimeOffset"], 2);
    assert!(
        run.normalized["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning["code"] == "TIMESTAMP_NON_MONOTONIC"
                || warning == "TIMESTAMP_NON_MONOTONIC")
    );
}

#[test]
fn packed_record_metrics_fall_back_to_encoded_components_with_lineage() {
    let run = decode_run(include_bytes!("fixtures/runs/garmin_components_only.fit")).unwrap();
    let sample = run.normalized["samples"]
        .as_array()
        .unwrap()
        .iter()
        .find(|sample| sample["elapsedSeconds"] == 35.0)
        .unwrap();
    assert_eq!(sample["speedMps"], 3.0);
    // Native1000m seeds16000ticks; packedlow160 rolls to16544ticks /16.
    assert_eq!(sample["distanceMeters"], 1034.0);
    assert_eq!(sample["sourceReferences"]["speedMps"]["componentParent"], 8);
    assert_eq!(
        sample["sourceReferences"]["distanceMeters"]["componentParent"],
        8
    );
}

#[test]
fn recorded_zero_rr_stays_zero_but_has_no_physiological_or_time_eligibility() {
    let run = decode_run(include_bytes!("fixtures/runs/garmin_hrv_zero.fit")).unwrap();
    let rr = &run.normalized["rr"];
    assert_eq!(
        rr["intervals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|interval| interval["rrMs"].clone())
            .collect::<Vec<_>>(),
        vec![json!(0.0), json!(1000.0), Value::Null]
    );
    assert_eq!(rr["intervals"][0]["timestamp"], Value::Null);
    assert_eq!(rr["intervals"][0]["timingEligible"], false);
    assert_eq!(
        field(&run.decoded, 78, 0)["elementValidity"],
        json!(["valid", "valid", "invalid"])
    );
}

#[test]
fn supported_header_forms_pass_and_unsupported_containers_publish_no_prefix() {
    for (bytes, header_size) in [
        (
            include_bytes!("fixtures/runs/garmin_header12.fit").as_slice(),
            12,
        ),
        (
            include_bytes!("fixtures/runs/garmin_zero_header_crc.fit").as_slice(),
            14,
        ),
        (
            include_bytes!("fixtures/runs/garmin_extended_header16.fit").as_slice(),
            16,
        ),
        (
            include_bytes!("fixtures/runs/garmin_extended_header16_crc.fit").as_slice(),
            16,
        ),
        (
            include_bytes!("fixtures/runs/garmin_extended_header15.fit").as_slice(),
            15,
        ),
        (
            include_bytes!("fixtures/runs/garmin_extended_header13.fit").as_slice(),
            13,
        ),
    ] {
        let run = decode_run(bytes).unwrap();
        assert_eq!(run.normalized["summary"]["distanceMeters"], 1000.0);
        assert_eq!(run.normalized["summary"]["timerTimeSeconds"], 300.0);
        assert_eq!(
            run.decoded["definitions"][0]["sourceReference"]["byteOffset"],
            header_size
        );
        assert_eq!(run.normalized["samples"][0]["speedMps"], 2.0);
    }
    for bytes in [
        include_bytes!("fixtures/runs/garmin_trailing_byte.fit").as_slice(),
        include_bytes!("fixtures/runs/garmin_chained.fit").as_slice(),
        include_bytes!("fixtures/runs/garmin_extended_header16_bad_header_crc.fit").as_slice(),
        include_bytes!("fixtures/runs/garmin_extended_header16_bad_extension_crc.fit").as_slice(),
        include_bytes!("fixtures/runs/garmin_extended_header16_truncated.fit").as_slice(),
    ] {
        assert!(matches!(decode_run(bytes), Err(FitError::InvalidFit)));
    }
}

#[test]
fn big_endian_native_unknown_arrays_and_unsafe_integers_keep_exact_values() {
    let run = decode_run(include_bytes!("fixtures/runs/garmin_big_endian.fit")).unwrap();
    assert_eq!(field(&run.decoded, 65280, 0)["value"], json!([0, null, 42]));
    assert_eq!(field(&run.decoded, 65280, 2)["value"], "9007199254740993");
    assert_eq!(field(&run.decoded, 65280, 3)["value"], "-9007199254740993");
    assert_eq!(field(&run.decoded, 20, 240)["value"], 54321);
}

#[test]
fn recorded_zero_summary_speed_cannot_select_a_positive_derived_pace() {
    let run = decode_run(include_bytes!(
        "fixtures/runs/garmin_summary_zero_speed.fit"
    ))
    .unwrap();
    assert_eq!(run.normalized["summary"]["averageSpeedMps"], 0.0);
    assert_eq!(
        run.normalized["summary"]["averagePaceSecondsPerKm"],
        Value::Null
    );
    assert_eq!(
        run.normalized["summary"]["derived"]["averagePaceSecondsPerKm"],
        300.0
    );
}

#[test]
fn encoded_string_sentinel_is_invalid_not_valid_empty_text() {
    let run = decode_run(include_bytes!("fixtures/runs/garmin_archive.fit")).unwrap();
    assert_eq!(field(&run.decoded, 65281, 5)["value"], Value::Null);
    assert_eq!(field(&run.decoded, 65281, 5)["validity"], "invalid");
    assert_eq!(field(&run.decoded, 65281, 5)["rawValue"], "");
    assert_eq!(field(&run.decoded, 65281, 2)["value"], "abc");
}

#[test]
fn native_fractional_rr_anchor_keeps_exact_timestamp_and_elapsed_precision() {
    let run = decode_run(include_bytes!("fixtures/runs/garmin_fractional_anchor.fit")).unwrap();
    let intervals = run.normalized["rr"]["intervals"].as_array().unwrap();
    assert_eq!(field(&run.decoded, 132, 0)["rawValue"], 9);
    assert_eq!(intervals[0]["endElapsedSeconds"], 1.0 + 9.0 / 32768.0);
    assert_eq!(intervals[1]["endElapsedSeconds"], 2.0 + 9.0 / 32768.0);
    assert_eq!(intervals[0]["timestamp"], "2021-09-08T01:46:41.000274658Z");
    assert_eq!(intervals[1]["timestamp"], "2021-09-08T01:46:42.000274658Z");
}

#[test]
fn both_native_progressive_rr_fixtures_match_independent_quantized_beat_bounds() {
    for (native, input) in [
        (
            include_bytes!("fixtures/runs/garmin_progressive_rr.fit").as_slice(),
            include_str!("fixtures/runs/progressive_rr_input.json"),
        ),
        (
            include_bytes!("fixtures/runs/progressive_rr_positive.fit").as_slice(),
            include_str!("fixtures/runs/progressive_rr_positive_input.json"),
        ),
    ] {
        let expected: Value = serde_json::from_str(input).unwrap();
        let run = decode_run(native).unwrap();
        let intervals = run.normalized["rr"]["intervals"].as_array().unwrap();
        let source = expected["rr"]["intervals"].as_array().unwrap();
        assert_eq!(intervals.len(), source.len());
        let mut previous_ticks = 0.0;
        for (actual, beat) in intervals.iter().zip(source) {
            let ticks = (beat["endElapsedSeconds"].as_f64().unwrap() * 1024.0).round();
            assert_eq!(actual["rrMs"], (ticks - previous_ticks) * 1000.0 / 1024.0);
            assert_eq!(actual["startElapsedSeconds"], previous_ticks / 1024.0);
            assert_eq!(actual["endElapsedSeconds"], ticks / 1024.0);
            assert_eq!(actual["timingEligible"], true);
            assert_eq!(
                actual["provenance"]["timingMethod"],
                "hr_event_counter_anchor"
            );
            previous_ticks = ticks;
        }
    }
}
