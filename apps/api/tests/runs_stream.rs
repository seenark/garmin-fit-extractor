use garmin_fit_extractor_api::{
    model::Analysis,
    runs::stream::{Document, RevisionReader, project_document, split_run},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};
use uuid::Uuid;

struct PrivateDirectory(PathBuf);
impl PrivateDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("runs-stream-test-{}", Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        Self(path)
    }
    fn root(&self, value: &Value) -> PathBuf {
        let path = self.0.join("run.json");
        fs::write(&path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
        path
    }
}
impl Drop for PrivateDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn reference(index: u64, global: u64, field: u64) -> Value {
    json!({"messageIndex":index,"globalMessageNumber":global,"fieldNumber":field,"byteOffset":12+index*20,"byteLength":4})
}
fn field(index: u64, global: u64, number: u64, value: Value, unit: &str) -> Value {
    let unit = if (global == 132 && number == 253) || (global == 18 && number == 2) {
        Value::Null
    } else {
        json!(unit)
    };
    json!({"fieldNumber":number,"name":"synthetic-native-field","value":value,"unit":unit,"baseType":"uint32","validity":"valid","role":"native","developerIdentity":null,"classification":"metric","sourceReference":reference(index,global,number)})
}
fn message(index: u64, global: u64, fields: Vec<Value>) -> Value {
    json!({"index":index,"globalMessageNumber":global,"localMessageNumber":0,"sourceReference":{"messageIndex":index,"globalMessageNumber":global,"byteOffset":12+index*20,"byteLength":20},"fields":fields})
}
fn base_fixture() -> Value {
    let sample = |index: u64| json!({"index":index,"timestamp":format!("2026-01-01T00:00:0{}Z",index+1),"elapsedSeconds":index+1,"speedMps":3.125,"paceSecondsPerKm":192.0,"heartRateBpm":if index==0{json!(0)}else{Value::Null},"powerWatts":null,"cadenceStepsPerMinute":null,"altitudeMeters":-12.5,"distanceMeters":null,"timerRunning":null,"sourceReferences":{"timestamp":reference(index,20,253),"speedMps":reference(index,20,6),"paceSecondsPerKm":reference(index,20,6),"altitudeMeters":reference(index,20,2),"heartRateBpm":reference(index,20,3)}});
    let rr = |index: u64| json!({"index":index,"rrMs":1000.0,"timestamp":format!("2026-01-01T00:00:0{}Z",index+1),"elapsedSeconds":index+1,"startElapsedSeconds":index,"endElapsedSeconds":index+1,"sourceReference":reference(2,132,9),"timingEligible":true,"provenance":{"kind":"recorded_rr","messageNumber":132,"fieldNumber":9,"timingMethod":"hr_event_counter_anchor","anchorTimestamp":"2026-01-01T00:00:02Z","anchorSourceReferences":{"timestamp":reference(2,132,253),"fractionalTimestamp":reference(2,132,0),"eventCounter":reference(2,132,9)},"limitations":[]}});
    let mut summary = json!({});
    for key in [
        "distanceMeters",
        "timerTimeSeconds",
        "elapsedTimeSeconds",
        "movingTimeSeconds",
        "averageSpeedMps",
        "averagePaceSecondsPerKm",
        "averageHeartRateBpm",
        "averagePowerWatts",
        "averageCadenceStepsPerMinute",
    ] {
        summary[key] = Value::Null;
    }
    summary["elapsedTimeSeconds"] = json!(10.0);
    json!({"decoded":{"schemaVersion":"2.0.0","decoder":{"name":"synthetic-cc0"},"messages":[message(0,20,vec![field(0,20,253,json!("2026-01-01T00:00:01Z"),"s"),field(0,20,6,json!(3.125),"m/s"),field(0,20,3,json!(0),"bpm"),field(0,20,2,json!(-12.5),"m"),field(0,20,250,json!([0,255,null]),"unknown")]),message(1,20,vec![field(1,20,253,json!("2026-01-01T00:00:02Z"),"s"),field(1,20,6,json!(3.125),"m/s"),field(1,20,3,Value::Null,"bpm"),field(1,20,2,json!(-12.5),"m")]),message(2,132,vec![field(2,132,253,json!("2026-01-01T00:00:02Z"),"s"),field(2,132,0,json!(0.0),"s"),field(2,132,9,json!([1024,2048]),"s")])],"warnings":[],"unknownRootArray":[{"nested":["escaped quote: \" and bracket }",null,0]}]},"normalized":{"schemaVersion":"2.0.0","session":{"index":0,"sourceReferences":{}},"startTime":"2026-01-01T00:00:00Z","endTime":"2026-01-01T00:00:10Z","sport":"running","subtype":null,"summary":summary,"samples":[sample(0),sample(1)],"laps":[],"timerEvents":[],"sensors":[],"zones":[],"deviceReportedThresholds":[],"extensions":[{"opaque":[null,0,"unchanged"]}],"rr":{"intervals":[rr(0),rr(1)],"alignmentEligible":true,"reasons":[]},"warnings":[],"unknownRootArray":[[1,2],{"unknown":"preserved"}]},"legacy":serde_json::to_value(Analysis::empty("synthetic-recorded-source.fit")).unwrap()})
}
fn fixture() -> Value {
    let mut value = base_fixture();
    value["decoded"]["messages"]
        .as_array_mut()
        .unwrap()
        .push(message(
            3,
            18,
            vec![
                field(3, 18, 2, json!("2026-01-01T00:00:00Z"), "s"),
                field(3, 18, 253, json!("2026-01-01T00:00:10Z"), "s"),
            ],
        ));
    value["normalized"]["session"]["sourceReferences"] =
        json!({"startTime":reference(3,18,2),"endTime":reference(3,18,253)});
    let identity = json!({"developerDataIndex":0,"fieldDefinitionNumber":3,"applicationId":"synthetic-cc0-identity"});
    let mut developer = field(0, 20, 3, json!(991), "unknown");
    developer["developerIdentity"] = identity.clone();
    developer["sourceReference"]["developerIdentity"] = identity;
    value["decoded"]["messages"][0]["fields"]
        .as_array_mut()
        .unwrap()
        .push(developer);
    value["decoded"]["messages"][2]["fields"][2]["value"] = json!([101.0, 102.0]);
    for (index, interval) in value["normalized"]["rr"]["intervals"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        interval["sourceReference"]["arrayIndex"] = json!(index);
        interval["provenance"]["anchorSourceReferences"]["eventCounter"]["arrayIndex"] = json!(1);
    }
    value
}
fn bytes(document: &Document) -> Vec<u8> {
    let mut file = File::open(&document.path).unwrap();
    file.seek(SeekFrom::Start(document.offset)).unwrap();
    let mut output = Vec::new();
    file.take(document.byte_length)
        .read_to_end(&mut output)
        .unwrap();
    output
}
fn value(document: &Document) -> Value {
    serde_json::from_slice(&bytes(document)).unwrap()
}

#[test]
fn native_header_variant_preserves_strong_observation_identity_and_archived_provenance() {
    use garmin_fit_extractor_api::fit::runs::decode_run_to_writer;

    fn crc(data: &[u8]) -> u16 {
        let mut value = 0u16;
        for &byte in data {
            value ^= u16::from(byte);
            for _ in 0..8 {
                value = (value >> 1) ^ if value & 1 != 0 { 0xA001 } else { 0 };
            }
        }
        value
    }

    fn assert_header_shift(before: &Value, after: &Value, delta: u64) {
        match (before, after) {
            (Value::Object(before), Value::Object(after)) => {
                assert!(before.keys().eq(after.keys()));
                for (key, value) in before {
                    if key == "byteOffset" {
                        assert_eq!(
                            after[key].as_u64().unwrap(),
                            value.as_u64().unwrap() + delta
                        );
                    } else {
                        assert_header_shift(value, &after[key], delta);
                    }
                }
            }
            (Value::Array(before), Value::Array(after)) => {
                assert_eq!(before.len(), after.len());
                for (before, after) in before.iter().zip(after) {
                    assert_header_shift(before, after, delta);
                }
            }
            _ => assert_eq!(before, after),
        }
    }

    // Original CC0 native corpus, generated by fixtures/runs/generate.py.
    // Change only the container header, not any FIT definition or data message.
    let original = include_bytes!("fixtures/runs/garmin_run.fit").as_slice();
    let original_header = usize::from(original[0]);
    let mut alternate = original[..12].to_vec();
    alternate[0] = 16;
    alternate.extend_from_slice(&crc(&alternate).to_le_bytes());
    alternate.extend_from_slice(&[0xA5, 0x5A]);
    alternate.extend_from_slice(&original[original_header..original.len() - 2]);
    alternate.extend_from_slice(&crc(&alternate).to_le_bytes());
    let alternate_header = usize::from(alternate[0]);
    let delta = (alternate_header - original_header) as u64;
    assert_eq!(delta, 2);
    assert_eq!(
        &alternate[alternate_header..alternate.len() - 2],
        &original[original_header..original.len() - 2]
    );

    let mut observations = Vec::new();
    for source in [original, alternate.as_slice()] {
        let directory = PrivateDirectory::new();
        let path = directory.0.join("run.json");
        let mut document = Vec::new();
        // The real decoder validates both header CRC and complete file CRC.
        decode_run_to_writer(source, &mut document).unwrap();
        let mut decoded: Value = serde_json::from_slice(&document).unwrap();
        fs::write(&path, &document).unwrap();
        let staged = split_run(&path, &directory.0).unwrap();
        let normalized = &decoded["normalized"];
        assert_eq!(normalized["summary"]["recorded"]["distanceMeters"], 1000.0);
        assert_eq!(normalized["summary"]["recorded"]["timerTimeSeconds"], 300.0);
        assert_eq!(normalized["samples"][0]["speedMps"], 2.0);
        assert_eq!(normalized["samples"][1]["speedMps"], 4.0);
        assert!(normalized["summary"]["sourceReferences"]["distanceMeters"]["byteOffset"].is_u64());
        assert_eq!(staged.normalized_metadata["observationStrong"], true);
        assert_eq!(staged.normalized_metadata["summary"], normalized["summary"]);
        for (archive, key) in [
            (&staged.decoded, "decoded"),
            (&staged.normalized, "normalized"),
        ] {
            assert_eq!(
                bytes(archive),
                document[archive.offset as usize..(archive.offset + archive.byte_length) as usize]
            );
            assert_eq!(value(archive), decoded[key]);
        }
        observations.push((staged.normalized_metadata, decoded["normalized"].take()));
    }

    let (original_metadata, original_normalized) = &observations[0];
    let (alternate_metadata, alternate_normalized) = &observations[1];
    assert_header_shift(original_normalized, alternate_normalized, delta);
    assert_ne!(
        original_normalized["summary"]["sourceReferences"],
        alternate_normalized["summary"]["sourceReferences"]
    );
    assert_eq!(
        original_metadata["observationFingerprint"]
            .as_str()
            .unwrap(),
        alternate_metadata["observationFingerprint"]
            .as_str()
            .unwrap(),
        "changing native byte coordinates must not split identical strong observations"
    );
}

#[test]
fn stage_spans_preserve_archive_bytes_values_order_and_full_resolution_projection() {
    let original = fixture();
    let directory = PrivateDirectory::new();
    let path = directory.root(&original);
    let root = fs::read(&path).unwrap();
    let staged = split_run(&path, &directory.0).unwrap();
    assert_eq!(staged.decoded.path, path);
    assert_eq!(staged.normalized.path, path);
    for (document, key) in [
        (&staged.decoded, "decoded"),
        (&staged.normalized, "normalized"),
    ] {
        let output = bytes(document);
        assert_eq!(
            &root[document.offset as usize..(document.offset + document.byte_length) as usize],
            output
        );
        assert_eq!(document.sha256, format!("{:x}", Sha256::digest(&output)));
        assert_eq!(value(document), original[key]);
    }
    let input = value(&staged.analysis_input);
    assert_eq!(
        input["samples"].as_array().unwrap().len(),
        original["normalized"]["samples"].as_array().unwrap().len()
    );
    for (before, after) in original["normalized"]["samples"]
        .as_array()
        .unwrap()
        .iter()
        .zip(input["samples"].as_array().unwrap())
    {
        for key in [
            "index",
            "timestamp",
            "elapsedSeconds",
            "speedMps",
            "paceSecondsPerKm",
            "heartRateBpm",
            "powerWatts",
            "cadenceStepsPerMinute",
            "altitudeMeters",
            "distanceMeters",
            "timerRunning",
        ] {
            assert_eq!(after[key], before[key], "{key}");
        }
        assert!(after.get("sourceReferences").is_none());
    }
    assert_eq!(input["rr"], original["normalized"]["rr"]);
    assert_eq!(input["samples"][0]["heartRateBpm"], 0);
    assert!(input["samples"][1]["heartRateBpm"].is_null());
    assert!(input.get("unknownRootArray").is_none());
    assert_eq!(
        staged.legacy.source.file_name,
        "synthetic-recorded-source.fit"
    );
    assert_eq!(staged.normalized_metadata["counts"]["samples"], 2);
    assert_eq!(staged.normalized_metadata["observationStrong"], true);
    assert!(!Path::new(&directory.0.join("decoded.stage.json")).exists());
    assert!(!Path::new(&directory.0.join("normalized.stage.json")).exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(staged.analysis_input.path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}

#[test]
fn malformed_identity_metric_source_missing_array_and_rr_proof_fail_before_publish() {
    let cases: [fn(&mut Value); 9] = [
        |v| v["decoded"]["messages"][1]["index"] = json!(9),
        |v| v["normalized"]["samples"][0]["sourceReferences"]["speedMps"]["fieldNumber"] = json!(3),
        |v| {
            v["normalized"]["samples"][0]["sourceReferences"]["speedMps"]["messageIndex"] =
                json!(99)
        },
        |v| v["decoded"]["messages"][0]["fields"][2]["validity"] = json!("invalid"),
        |v| {
            v["normalized"].as_object_mut().unwrap().remove("laps");
        },
        |v| v["normalized"]["rr"]["intervals"][0]["startElapsedSeconds"] = json!(-0.1),
        |v| v["normalized"]["rr"]["intervals"][1]["endElapsedSeconds"] = json!(11.0),
        |v| {
            v["normalized"]["rr"]["intervals"][0]["provenance"]["timingMethod"] =
                json!("unanchored")
        },
        |v| {
            v["normalized"]["rr"]["intervals"][0]["provenance"]["anchorSourceReferences"]["timestamp"]
                ["messageIndex"] = json!(0)
        },
    ];
    for change in cases {
        let directory = PrivateDirectory::new();
        let mut original = fixture();
        change(&mut original);
        let path = directory.root(&original);
        assert_eq!(
            split_run(&path, &directory.0).unwrap_err().code(),
            "INVALID_RUN_DOCUMENT"
        );
    }
}

#[test]
fn source_proven_outside_session_rr_stays_archived_but_not_timing_eligible() {
    let directory = PrivateDirectory::new();
    let mut original = fixture();
    let interval = &mut original["normalized"]["rr"]["intervals"][0];
    interval["startElapsedSeconds"] = json!(-1.0);
    interval["endElapsedSeconds"] = json!(0.0);
    interval["elapsedSeconds"] = json!(0.0);
    interval["timestamp"] = json!("2026-01-01T00:00:00Z");
    interval["timingEligible"] = json!(false);
    original["normalized"]["rr"]["alignmentEligible"] = json!(false);
    original["normalized"]["rr"]["reasons"] = json!(["RR_OUTSIDE_SESSION"]);
    original["decoded"]["messages"][2]["fields"][2]["value"][0] = json!(100.0);
    let path = directory.root(&original);
    let staged = split_run(&path, &directory.0).unwrap();
    assert_eq!(
        value(&staged.normalized)["rr"],
        original["normalized"]["rr"]
    );
    assert_eq!(
        value(&staged.analysis_input)["rr"],
        original["normalized"]["rr"]
    );
}

#[test]
fn wrong_native_unit_and_unproven_session_bounds_fail() {
    for wrong_unit in [true, false] {
        let directory = PrivateDirectory::new();
        let mut original = fixture();
        if wrong_unit {
            original["decoded"]["messages"][0]["fields"][1]["unit"] = json!("km/h");
        } else {
            original["normalized"]["startTime"] = json!("2026-01-01T00:00:00.5Z");
        }
        let path = directory.root(&original);
        assert_eq!(
            split_run(&path, &directory.0).unwrap_err().code(),
            "INVALID_RUN_DOCUMENT"
        );
    }
}

#[test]
fn expanded_native_alias_compressed_clock_and_invalid_recorded_rr_remain_distinct() {
    let directory = PrivateDirectory::new();
    let mut original = fixture();
    let mut expanded = field(0, 20, 73, json!(3.125), "m/s");
    expanded["role"] = json!("expanded");
    expanded["sourceReference"]["componentParent"] = json!(6);
    original["decoded"]["messages"][0]["fields"]
        .as_array_mut()
        .unwrap()
        .push(expanded);
    original["decoded"]["messages"][0]["fields"]
        .as_array_mut()
        .unwrap()
        .push(field(0, 20, 73, json!(3.125), "m/s"));
    original["decoded"]["messages"][1]["fields"][0]["role"] = json!("reconstructed");
    original["decoded"]["messages"][1]["fields"][0]["unit"] = Value::Null;
    let mut raw_rr = field(4, 78, 0, json!([0.8, null, 0.81]), "s");
    raw_rr["validity"] = json!("mixed");
    original["decoded"]["messages"]
        .as_array_mut()
        .unwrap()
        .push(message(4, 78, vec![raw_rr]));
    let mut source = reference(4, 78, 0);
    source["arrayIndex"] = json!(1);
    original["normalized"]["rr"]["intervals"].as_array_mut().unwrap().push(json!({"index":2,"rrMs":null,"timestamp":null,"elapsedSeconds":null,"startElapsedSeconds":null,"endElapsedSeconds":null,"sourceReference":source,"timingEligible":false,"provenance":{"kind":"recorded_rr","messageNumber":78,"fieldNumber":0,"timingMethod":"unanchored","anchorTimestamp":null,"anchorSourceReferences":null,"limitations":["INVALID_RECORDED_INTERVAL"]}}));
    original["normalized"]["rr"]["alignmentEligible"] = json!(false);
    let path = directory.root(&original);
    let staged = split_run(&path, &directory.0).unwrap();
    assert_eq!(value(&staged.decoded), original["decoded"]);
    assert_eq!(value(&staged.normalized), original["normalized"]);
    assert_eq!(
        value(&staged.analysis_input)["rr"],
        original["normalized"]["rr"]
    );
}

#[test]
fn rr_timing_cannot_ignore_actual_native_counter_value() {
    let directory = PrivateDirectory::new();
    let mut original = fixture();
    original["decoded"]["messages"][2]["fields"][2]["value"][0] = json!(103.0);
    let path = directory.root(&original);
    assert_eq!(
        split_run(&path, &directory.0).unwrap_err().code(),
        "INVALID_RUN_DOCUMENT"
    );
}

#[test]
fn every_in_session_native_record_must_reach_projection() {
    for drop_first in [false, true] {
        let directory = PrivateDirectory::new();
        let mut original = fixture();
        original["normalized"]["samples"]
            .as_array_mut()
            .unwrap()
            .remove(if drop_first { 0 } else { 1 });
        original["normalized"]["samples"][0]["index"] = json!(0);
        let path = directory.root(&original);
        assert_eq!(
            split_run(&path, &directory.0).unwrap_err().code(),
            "INVALID_RUN_DOCUMENT"
        );
    }
}

#[test]
fn fractional_anchors_and_aliased_counters_preserve_distinct_source_values() {
    let directory = PrivateDirectory::new();
    let mut original = fixture();
    original["decoded"]["messages"][2]["fields"][1]["value"] = json!(1.25);
    let mut expanded = field(2, 132, 9, json!([999.0]), "s");
    expanded["role"] = json!("expanded");
    expanded["componentParent"] = json!(10);
    expanded["sourceReference"]["componentParent"] = json!(10);
    expanded["sourceReference"]["byteOffset"] = json!(999);
    original["decoded"]["messages"][2]["fields"]
        .as_array_mut()
        .unwrap()
        .push(expanded);
    original["decoded"]["messages"]
        .as_array_mut()
        .unwrap()
        .push(message(
            4,
            60000,
            vec![field(
                4,
                60000,
                253,
                json!("unclassified source text"),
                "unknown",
            )],
        ));
    for (index, interval) in original["normalized"]["rr"]["intervals"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        let end = index as f64 + 2.25;
        interval["timestamp"] = json!(format!("2026-01-01T00:00:0{}.250Z", index + 2));
        interval["elapsedSeconds"] = json!(end);
        interval["startElapsedSeconds"] = json!(end - 1.0);
        interval["endElapsedSeconds"] = json!(end);
        interval["provenance"]["anchorTimestamp"] = json!("2026-01-01T00:00:03.250Z");
    }
    let path = directory.root(&original);
    let staged = split_run(&path, &directory.0).unwrap();
    assert_eq!(value(&staged.normalized), original["normalized"]);
    assert_eq!(
        value(&staged.analysis_input)["rr"],
        original["normalized"]["rr"]
    );
}

#[test]
fn source_proven_counter_reset_retains_unknown_duration_without_eligibility() {
    let directory = PrivateDirectory::new();
    let mut original = fixture();
    original["decoded"]["messages"]
        .as_array_mut()
        .unwrap()
        .push(message(
            4,
            132,
            vec![
                field(4, 132, 253, json!("2026-01-01T00:00:03Z"), "s"),
                field(4, 132, 0, json!(0.0), "s"),
                field(4, 132, 9, json!(50.0), "s"),
            ],
        ));
    let mut reset = original["normalized"]["rr"]["intervals"][1].clone();
    reset["index"] = json!(2);
    reset["rrMs"] = Value::Null;
    reset["startElapsedSeconds"] = Value::Null;
    reset["elapsedSeconds"] = json!(3.0);
    reset["endElapsedSeconds"] = json!(3.0);
    reset["timestamp"] = json!("2026-01-01T00:00:03Z");
    reset["timingEligible"] = json!(false);
    reset["sourceReference"] = reference(4, 132, 9);
    reset["provenance"]["limitations"] = json!(["EVENT_COUNTER_RESET"]);
    original["normalized"]["rr"]["intervals"]
        .as_array_mut()
        .unwrap()
        .push(reset);
    original["normalized"]["rr"]["alignmentEligible"] = json!(false);
    let path = directory.root(&original);
    let staged = split_run(&path, &directory.0).unwrap();
    assert_eq!(
        value(&staged.normalized)["rr"],
        original["normalized"]["rr"]
    );
    assert_eq!(
        value(&staged.analysis_input)["rr"],
        original["normalized"]["rr"]
    );
}

#[test]
fn unknown_arrays_stream_past_record_budget_and_field_omission_remains_schema_closed() {
    let mut input = b"{\"unknown\":[".to_vec();
    for index in 0..400_000 {
        if index != 0 {
            input.push(b',');
        }
        input.extend_from_slice(b"[1,2,3,4]");
    }
    input.extend_from_slice(b"],\"samples\":[{\"index\":0,\"heartRateBpm\":0},{\"index\":1,\"heartRateBpm\":null}],\"summary\":{\"distanceMeters\":0}}");
    let mut output = Vec::new();
    let mut keep = |path: &[String]| path != ["unknown"];
    let mut project = |_: &[String], value: Value| Ok(Some(value));
    project_document(input.as_slice(), &mut output, &mut keep, &mut project).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&output).unwrap(),
        json!({"samples":[{"index":0,"heartRateBpm":0},{"index":1,"heartRateBpm":null}],"summary":{"distanceMeters":0}})
    );
}

#[test]
fn projection_writer_failures_remain_storage_errors_not_invalid_documents() {
    use axum::{http::StatusCode, response::IntoResponse};
    use std::io::{self, Write};
    struct FailingWriter {
        remaining: usize,
        disk_full: bool,
    }
    impl Write for FailingWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.remaining == 0 {
                return Err(if self.disk_full {
                    io::Error::from_raw_os_error(libc::ENOSPC)
                } else {
                    io::Error::other("RUNS_SPOOL_LIMIT")
                });
            }
            let length = bytes.len().min(self.remaining);
            self.remaining -= length;
            Ok(length)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let input =
        br#"{"samples":[{"index":0}],"unknown":[true,"text",null],"summary":{"distanceMeters":1}}"#;
    for disk_full in [false, true] {
        for remaining in 0..input.len() {
            let mut writer = FailingWriter {
                remaining,
                disk_full,
            };
            let error = project_document(
                input.as_slice(),
                &mut writer,
                &mut |_| true,
                &mut |_, value| Ok(Some(value)),
            )
            .unwrap_err();
            assert_eq!(
                error.code(),
                "RUNS_STORAGE_FAILED",
                "writer failed after {remaining} bytes"
            );
            assert_eq!(
                error.into_response().status(),
                StatusCode::INTERNAL_SERVER_ERROR
            );
        }
    }
    for invalid in [
        br#"{"samples":[}"#.as_slice(),
        br#"{"samples":{}}"#.as_slice(),
    ] {
        let error = project_document(invalid, &mut Vec::new(), &mut |_| true, &mut |_, value| {
            Ok(Some(value))
        })
        .unwrap_err();
        assert_eq!(error.code(), "INVALID_RUN_DOCUMENT");
        assert_eq!(
            error.into_response().status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
}

#[tokio::test]
async fn malformed_chunk_manifest_is_rejected_without_loading_payload() {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://localhost/unused")
        .unwrap();
    for metadata in [
        json!({}),
        json!({"byteLength":1,"sha256":"0".repeat(64),"chunkCount":2}),
        json!({"byteLength":1,"sha256":"not-a-digest","chunkCount":1}),
    ] {
        assert_eq!(
            RevisionReader::new(
                pool.clone(),
                "owner".into(),
                "revision".into(),
                "archive",
                &metadata
            )
            .err()
            .unwrap()
            .code(),
            "RUNS_DOCUMENT_INTEGRITY_FAILED"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn pg_reader_rejects_missing_chunk_wrong_hash_and_foreign_owner() {
    let url = std::env::var("TEST_DATABASE_URL").expect("disposable database URL");
    let expected = std::env::var("PGDATA").expect("disposable database identity");
    assert!(
        !expected.is_empty(),
        "disposable PostgreSQL must have a nonempty PGDATA identity"
    );
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .unwrap();
    let directory: String = sqlx::query_scalar("SHOW data_directory")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(directory, expected);
    let version: String = sqlx::query_scalar("SHOW server_version_num")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        (180000..190000).contains(&version.parse::<i32>().unwrap()),
        "disposable database must run PostgreSQL 18"
    );
    // An isolated temporary relation exercises the real PG transport without
    // publishing synthetic FIT or editing production immutable revisions.
    sqlx::query("CREATE TEMP TABLE runs_revision_chunks(revision_id TEXT NOT NULL,owner_id TEXT NOT NULL,document TEXT NOT NULL,position BIGINT NOT NULL,payload BYTEA NOT NULL CHECK(octet_length(payload)<=65536),PRIMARY KEY(revision_id,document,position))").execute(&pool).await.unwrap();
    let content = br#"{"samples":[]}"#.to_vec();
    sqlx::query("INSERT INTO runs_revision_chunks VALUES('revision','owner','archive',0,$1)")
        .bind(&content)
        .execute(&pool)
        .await
        .unwrap();
    let manifest = json!({"byteLength":content.len(),"chunkCount":1,"sha256":format!("{:x}",Sha256::digest(&content))});
    for (owner, revision, metadata, success) in [
        ("owner", "revision", manifest.clone(), true),
        ("foreign", "revision", manifest.clone(), false),
        ("owner", "missing", manifest.clone(), false),
        (
            "owner",
            "revision",
            json!({"byteLength":content.len(),"chunkCount":1,"sha256":"0".repeat(64)}),
            false,
        ),
    ] {
        let pool = pool.clone();
        let owner = owner.to_owned();
        let revision = revision.to_owned();
        let result = tokio::task::spawn_blocking(move || {
            let reader = RevisionReader::new(pool, owner, revision, "archive", &metadata).unwrap();
            let mut bytes = Vec::new();
            project_document(reader, &mut bytes, &mut |_| true, &mut |_, value| {
                Ok(Some(value))
            })
            .map(|_| bytes)
        })
        .await
        .unwrap();
        if success {
            assert_eq!(result.unwrap(), content);
        } else {
            use axum::{http::StatusCode, response::IntoResponse};
            let error = result.unwrap_err();
            assert_eq!(error.code(), "RUNS_DOCUMENT_INTEGRITY_FAILED");
            assert_eq!(
                error.into_response().status(),
                StatusCode::INTERNAL_SERVER_ERROR
            );
        }
    }
    let transport_manifest = manifest.clone();
    let transaction = pool.begin().await.unwrap();
    let (output, transaction) = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::task::spawn_blocking(move || {
            let transaction = std::rc::Rc::new(std::cell::RefCell::new(transaction));
            let mut reader = RevisionReader::new_transaction(
                transaction.clone(),
                "owner".into(),
                "revision".into(),
                "archive",
                &manifest,
            )
            .unwrap();
            let mut output = Vec::new();
            reader.read_to_end(&mut output).unwrap();
            drop(reader);
            let transaction = std::rc::Rc::try_unwrap(transaction)
                .ok()
                .unwrap()
                .into_inner();
            (output, transaction)
        }),
    )
    .await
    .expect("a guarded reader must not acquire a second pool connection")
    .unwrap();
    assert_eq!(output, content);
    transaction.rollback().await.unwrap();
    // Only this test connection's temporary relation changes. The valid JSON
    // remains intact while a real PG query loses its required owner column.
    sqlx::query(
        "ALTER TABLE pg_temp.runs_revision_chunks RENAME COLUMN owner_id TO unavailable_owner_id",
    )
    .execute(&pool)
    .await
    .unwrap();
    let failed_pool = pool.clone();
    tokio::task::spawn_blocking(move || {
        use axum::{http::StatusCode, response::IntoResponse};
        let mut reader = RevisionReader::new(
            failed_pool.clone(),
            "owner".into(),
            "revision".into(),
            "archive",
            &transport_manifest,
        )
        .unwrap();
        let error = reader.read_to_end(&mut Vec::new()).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::ConnectionAborted);
        assert_eq!(error.to_string(), "RUNS_STORAGE_FAILED");
        let reader = RevisionReader::new(
            failed_pool,
            "owner".into(),
            "revision".into(),
            "archive",
            &transport_manifest,
        )
        .unwrap();
        let error = project_document(reader, &mut Vec::new(), &mut |_| true, &mut |_, value| {
            Ok(Some(value))
        })
        .unwrap_err();
        assert_eq!(error.code(), "RUNS_STORAGE_FAILED");
        assert_eq!(
            error.into_response().status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    })
    .await
    .unwrap();
    pool.close().await;
}
