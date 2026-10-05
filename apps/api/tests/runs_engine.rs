#[path = "../src/runs/analysis.rs"]
mod analysis;
#[path = "../src/runs/thresholds.rs"]
mod thresholds;
use serde_json::{Value, json};

#[test]
fn full_dfa_matches_independent_scipy_nolds_oracle() {
    let fixture: Value = serde_json::from_str(include_str!("fixtures/runs-engine-dfa-oracle.json")).unwrap();
    let rr: Vec<f64> = fixture["rrMs"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    let result = thresholds::dfa_window(&rr).unwrap();
    assert!((result["alpha"].as_f64().unwrap() - fixture["reference"]["alpha"].as_f64().unwrap()).abs() < 1e-9);
    for (actual, expected) in result["fluctuations"].as_array().unwrap().iter().zip(fixture["reference"]["fluctuations"].as_array().unwrap()) {
        assert!((actual.as_f64().unwrap() - expected.as_f64().unwrap()).abs() < 1e-8);
    }
    let shifted: Vec<f64> = rr.iter().map(|x| x + 1000.0).collect();
    let scaled: Vec<f64> = rr.iter().map(|x| x / 1000.0).collect();
    for values in [&shifted, &scaled] {
        assert!((thresholds::dfa_window(values).unwrap()["alpha"].as_f64().unwrap() - result["alpha"].as_f64().unwrap()).abs() < 1e-9);
    }
    assert!(thresholds::dfa_window(&vec![700.0; 240]).is_none());
}

#[test]
fn steady_workload_preserves_heart_rate_drift_and_recorded_laps() {
    let normalized = json!({"sport":"running","summary":{"elapsedTimeSeconds":300},"samples":
        (0..=300).map(|i|json!({"elapsedSeconds":i,"speedMps":3.0,"heartRateBpm":120.0+i as f64/10.0,"timerRunning":true})).collect::<Vec<_>>(),
        "laps":[{"index":0,"startElapsedSeconds":0,"endElapsedSeconds":300}], "rr":{"intervals":[],"alignmentEligible":false}});
    let result = analysis::analyze(&normalized);
    assert_eq!(result["segments"][0]["kind"], "steady");
    assert!(result["segments"][0]["features"]["heartRateDriftBpm"].as_f64().unwrap() > 25.0);
    assert_eq!(result["quality"]["coverage"]["power"], 0.0);
    assert_eq!(result["thresholds"]["lt1"]["status"], "insufficient_data");
    assert!(result["thresholds"]["lt1"]["reasons"].as_array().unwrap().contains(&json!("noExerciseRR")));
    assert_eq!(normalized["laps"].as_array().unwrap().len(), 1);
}

fn progressive() -> Value {
    serde_json::from_str(include_str!("fixtures/runs-engine-progressive.json")).unwrap()
}

#[test]
fn real_rr_pipeline_estimates_both_proxies_with_actual_boundary_trace() {
    let result = analysis::analyze(&progressive());
    for slot in ["lt1", "lt2"] {
        assert_eq!(result["thresholds"][slot]["status"], "low_confidence", "{slot}: {}", result["thresholds"][slot]["reasons"]);
        assert_eq!(result["thresholds"][slot]["engineStatus"], "experimental");
        assert_eq!(result["thresholds"][slot]["researchBlocked"], true);
        let hr = result["thresholds"][slot]["value"]["heartRateBpm"].as_f64().unwrap();
        assert!((100.0..180.0).contains(&hr));
        assert_eq!(result["thresholds"][slot]["trace"]["counts"]["accepted"], 1);
        assert_eq!(result["thresholds"][slot]["trace"]["windows"][0]["fluctuations"].as_array().unwrap().len(), 13);
    }
    let lt2 = &result["thresholds"]["lt2"];
    let regression = &lt2["trace"]["regression"];
    assert!(regression["alphaMin"].as_f64().unwrap() < 0.5);
    assert!(lt2["trace"]["boundaryWindowIndices"].as_array().unwrap().len() <= 2);
    assert!(result["thresholds"]["lt1"]["value"]["heartRateBpm"].as_f64().unwrap() < lt2["value"]["heartRateBpm"].as_f64().unwrap());
}

#[test]
fn history_uses_event_cutoff_independent_observations_and_comparable_sensor() {
    let normalized = progressive();
    let result = analysis::analyze(&normalized);
    let mut duplicate = json!({"activityId":"a","observationGroupId":"same-run","startTime":normalized["startTime"],"endTime":normalized["endTime"],"normalized":normalized,"analysis":result});
    let mut another = duplicate.clone();
    another["activityId"] = json!("b");
    let mut future = duplicate.clone();
    future["activityId"] = json!("future");
    future["endTime"] = json!("2026-01-02T00:08:00Z");
    let history = thresholds::estimate_history("2026-01-01T00:08:00Z", &[another, duplicate.clone(), future]);
    assert_eq!(history["lt1"]["evidence"]["independentActivityCount"], 1);
    assert_eq!(history["lt1"]["evidence"]["activityId"], "a");
    assert_eq!(history["lt2"]["status"], "low_confidence");
    duplicate["activityId"] = json!("new-sensor");
    duplicate["observationGroupId"] = json!("new-run");
    duplicate["endTime"] = json!("2026-01-02T00:08:00Z");
    duplicate["normalized"]["endTime"] = duplicate["endTime"].clone();
    duplicate["startTime"] = json!("2026-01-02T00:00:00Z");
    duplicate["normalized"]["startTime"] = duplicate["startTime"].clone();
    duplicate["normalized"]["sensors"][0]["type"] = json!("different-sensor");
    duplicate["analysis"]["thresholds"]["lt1"]["value"] = Value::Null;
    duplicate["analysis"]["thresholds"]["lt1"]["status"] = json!("insufficient_data");
    let old = json!({"activityId":"old","startTime":normalized["startTime"],"endTime":normalized["endTime"],"normalized":normalized,"analysis":result});
    let changed = thresholds::estimate_history("2026-01-02T00:08:00Z", &[old, duplicate]);
    assert!(changed["lt1"]["value"].is_null());
    assert!(changed["lt1"]["reasons"].as_array().unwrap().contains(&json!("noRecentComparableEvidence")));
}

#[test]
fn all_real_rr_windows_and_crossings_match_independent_oracle() {
    let golden: Value = serde_json::from_str(include_str!("fixtures/runs-engine-progressive-oracle.json")).unwrap();
    let result = analysis::analyze(&progressive());
    let windows = result["thresholds"]["lt1"]["trace"]["windows"].as_array().unwrap();
    for (actual, expected) in windows.iter().zip(golden["raw"]["windows"].as_array().unwrap()) {
        assert_eq!(actual["accepted"], expected["accepted"]);
        if let Some(alpha) = actual["alpha"].as_f64() {
            assert!((alpha - expected["alpha"].as_f64().unwrap()).abs() < 1e-9, "window {}", actual["index"]);
            for (a, e) in actual["fluctuations"].as_array().unwrap().iter().zip(expected["fluctuations"].as_array().unwrap()) {
                assert!((a.as_f64().unwrap() - e.as_f64().unwrap()).abs() < 1e-8);
            }
        }
    }
    for (slot,key) in [("lt1","lt1HeartRateBpm"),("lt2","lt2HeartRateBpm")] {
        assert!((result["thresholds"][slot]["value"]["heartRateBpm"].as_f64().unwrap() - golden["raw"]["candidates"][0][key].as_f64().unwrap()).abs() < 1e-8);
    }
}

fn analytical_windows(alphas: &[f64]) -> Vec<Value> {
    alphas.iter().enumerate().map(|(i,alpha)|json!({"alpha":alpha,"heartRateBpm":130.0+i as f64*10.0,"accepted":true,"segmentIndex":0,"centerElapsedSeconds":60.0+i as f64*5.0})).collect()
}

#[test]
fn selector_observes_both_boundaries_and_exact_half_without_extrapolation() {
    let exact = analytical_windows(&[1.0,0.875,0.75,0.625,0.5]);
    assert!((thresholds::select_decline(&exact,0.75)["value"].as_f64().unwrap()-150.0).abs()<1e-9);
    assert!((thresholds::select_decline(&exact,0.5)["value"].as_f64().unwrap()-170.0).abs()<1e-9);
    let boundary = analytical_windows(&[1.125,1.0,0.875,0.75,0.625,0.5,0.375]);
    let result = thresholds::select_decline(&boundary,0.5);
    assert!((result["value"].as_f64().unwrap()-180.0).abs()<1e-9);
    assert_eq!(result["candidates"][0]["boundaryWindowIndices"], json!([0,6]));
    let only_lt1 = analytical_windows(&[1.0,0.875,0.75,0.625]);
    assert!(thresholds::select_decline(&only_lt1,0.75)["value"].is_number());
    assert!(thresholds::select_decline(&only_lt1,0.5)["value"].is_null());
}

#[test]
fn selector_rejects_constant_hr_nondecline_and_competing_sections() {
    let mut constant_hr = analytical_windows(&[1.0,0.875,0.75,0.625,0.5]);
    for row in &mut constant_hr { row["heartRateBpm"] = json!(150.0); }
    let result = thresholds::select_decline(&constant_hr,0.75);
    assert!(result["value"].is_null());
    assert!(result["candidates"][0]["reasons"].as_array().unwrap().contains(&json!("noHrVariance")));
    let increasing = analytical_windows(&[0.5,0.625,0.75,0.875,1.0]);
    assert!(thresholds::select_decline(&increasing,0.75)["value"].is_null());
    let competing = analytical_windows(&[1.0,0.875,0.75,0.625,0.5,0.3,1.2,1.0,0.875,0.75,0.625,0.5]);
    let result = thresholds::select_decline(&competing,0.75);
    assert!(result["value"].is_null());
    assert_eq!(result["reasons"], json!(["ambiguousCrossing"]));
}

#[test]
fn timing_pause_gap_and_carryover_do_not_create_thresholds() {
    for mutation in ["unanchored","pause","gap","constantHr","nondecliningHr","carryover"] {
        let mut normalized = progressive();
        match mutation {
            "unanchored" => normalized["rr"]["intervals"][0]["provenance"]["timingMethod"] = json!("message_arrival"),
            "pause" => for row in normalized["samples"].as_array_mut().unwrap() { if (180.0..300.0).contains(&row["elapsedSeconds"].as_f64().unwrap()) {row["timerRunning"] = json!(false);} },
            "gap" => normalized["rr"]["intervals"].as_array_mut().unwrap().retain(|b| !(120.0..360.0).contains(&b["elapsedSeconds"].as_f64().unwrap())),
            "constantHr" => for row in normalized["samples"].as_array_mut().unwrap() { row["heartRateBpm"] = json!(150.0); },
            "nondecliningHr" => for row in normalized["samples"].as_array_mut().unwrap() { row["heartRateBpm"] = json!(180.0-row["elapsedSeconds"].as_f64().unwrap()/6.0); },
            "carryover" => for row in normalized["samples"].as_array_mut().unwrap() { let t=row["elapsedSeconds"].as_f64().unwrap();row["speedMps"] = json!(if (30.0..60.0).contains(&t){5.0}else{2.2+t/480.0}); },
            _ => unreachable!(),
        }
        let result = analysis::analyze(&normalized);
        if mutation=="pause" {
            for window in result["thresholds"]["lt2"]["trace"]["windows"].as_array().unwrap() {
                let start=window["startElapsedSeconds"].as_f64().unwrap();
                let end=window["endElapsedSeconds"].as_f64().unwrap();
                if start<300.0 && end>180.0 {assert_eq!(window["accepted"],false);}
            }
        } else { assert!(result["thresholds"]["lt2"]["value"].is_null(), "{mutation}"); }
        if mutation=="unanchored" {assert_eq!(result["thresholds"]["lt1"]["requiredContextUnprovable"],true);}
        if mutation=="carryover" {assert!(result["segments"].as_array().unwrap().iter().any(|s|s["eligibility"]["lt1"]["reasons"].as_array().unwrap().contains(&json!("fatigueCarryover"))));}
    }
}

#[test]
fn real_rr_lt1_only_and_short_support_remain_independent() {
    let mut normalized = progressive();
    normalized["summary"]["elapsedTimeSeconds"] = json!(340.0);
    normalized["endTime"] = json!("2026-01-01T00:05:40Z");
    normalized["samples"].as_array_mut().unwrap().retain(|s|s["elapsedSeconds"].as_f64().unwrap()<=340.0);
    normalized["rr"]["intervals"].as_array_mut().unwrap().retain(|s|s["endElapsedSeconds"].as_f64().unwrap()<=340.0);
    let result = analysis::analyze(&normalized);
    assert!(result["thresholds"]["lt1"]["value"].is_object());
    assert!(result["thresholds"]["lt2"]["value"].is_null());
    normalized["summary"]["elapsedTimeSeconds"] = json!(100.0);
    normalized["endTime"] = json!("2026-01-01T00:01:40Z");
    normalized["samples"].as_array_mut().unwrap().retain(|s|s["elapsedSeconds"].as_f64().unwrap()<=100.0);
    normalized["rr"]["intervals"].as_array_mut().unwrap().retain(|s|s["endElapsedSeconds"].as_f64().unwrap()<=100.0);
    assert!(analysis::analyze(&normalized)["thresholds"]["lt1"]["reasons"].as_array().unwrap().contains(&json!("insufficientContinuousWindow")));
}

#[test]
fn exact_duplicate_signature_excludes_labels_but_not_real_stream_changes() {
    let normalized = progressive();
    let mut reexport = normalized.clone();
    reexport["filename"] = json!("different-name.fit");
    reexport["workoutLabel"] = json!("arbitrary");
    assert_eq!(analysis::duplicate_evidence(&normalized),analysis::duplicate_evidence(&reexport));
    reexport["samples"][100]["speedMps"] = json!(3.9);
    assert_ne!(analysis::duplicate_evidence(&normalized)["exactTemporalStreamSignature"],analysis::duplicate_evidence(&reexport)["exactTemporalStreamSignature"]);
}

#[test]
fn stale_history_never_falls_back_and_exports_have_no_unselected_lineage() {
    let normalized = progressive();
    let result = analysis::analyze(&normalized);
    let row=json!({"activityId":"private-unselected","observationGroupId":"private-group","startTime":normalized["startTime"],"endTime":normalized["endTime"],"normalized":normalized,"analysis":result});
    let historical=thresholds::estimate_history("2026-01-09T00:08:00Z", &[row]);
    assert!(historical["lt1"]["value"].is_null());
    assert!(result["thresholds"]["lt1"]["value"].is_object());
    assert!(historical["lt1"].get("lastGood").is_none());
    let export=thresholds::export_projection(&historical);
    let text=export.to_string();
    assert!(!text.contains("private-unselected"));
    assert!(!text.contains("private-group"));
    assert!(export["lt1"]["trace"].get("windows").is_none());
    assert!(export["lt1"].get("lastGood").is_none());
}

#[test]
fn later_strides_do_not_discard_an_earlier_eligible_progression() {
    let mut normalized=progressive();
    normalized["summary"]["elapsedTimeSeconds"]=json!(600.0);
    normalized["endTime"]=json!("2026-01-01T00:10:00Z");
    for i in 481..=600 {
        normalized["samples"].as_array_mut().unwrap().push(json!({"index":i,"elapsedSeconds":i,"heartRateBpm":180.0,"speedMps":if (i/30)%2==0 {5.0}else{2.0},"timerRunning":true}));
    }
    let result=analysis::analyze(&normalized);
    assert_eq!(result["thresholds"]["lt1"]["status"],"low_confidence");
    assert_eq!(result["thresholds"]["lt2"]["status"],"low_confidence");
    assert!(result["segments"].as_array().unwrap().iter().any(|s|s["kind"]=="repeatedsurge"));
}

fn artifact_boundary_recording(count:usize)->Value {
    let mut normalized=progressive();
    normalized["summary"]["elapsedTimeSeconds"]=json!(120.0);
    normalized["endTime"]=json!("2026-01-01T00:02:00Z");
    normalized["samples"].as_array_mut().unwrap().retain(|s|s["elapsedSeconds"].as_f64().unwrap()<=120.0);
    let mut values:Vec<f64>=(0..100).map(|i|1200.0+2.0*(i as f64*1.73).sin()).collect();
    for i in 0..count {values[10+i*12]=1700.0;}
    let total=values.iter().sum::<f64>();
    let adjustment=(total-120000.0)/(100-count) as f64;
    for (i,v) in values.iter_mut().enumerate() {if !(0..count).any(|n|i==10+n*12) {*v-=adjustment;}}
    let provenance=normalized["rr"]["intervals"][0]["provenance"].clone();
    let mut elapsed=0.0;
    let intervals:Vec<Value>=values.into_iter().enumerate().map(|(i,rr)|{
        let start=elapsed;elapsed+=rr/1000.0;
        json!({"index":i,"rrMs":rr,"startElapsedSeconds":start,"endElapsedSeconds":elapsed,"elapsedSeconds":elapsed,"timingEligible":true,"provenance":provenance})
    }).collect();
    normalized["rr"]["intervals"]=json!(intervals);
    normalized
}

#[test]
fn actual_annotated_copy_obeys_three_percent_not_five_percent_boundary() {
    for corrected in [3,4,5,6] {
        let normalized=artifact_boundary_recording(corrected);
        let original=normalized.clone();
        let result=analysis::analyze(&normalized);
        assert_eq!(normalized,original);
        let window=&result["thresholds"]["lt1"]["trace"]["windows"][0];
        assert_eq!(window["rawBeatCount"],100);
        assert_eq!(window["correctedBeatCount"],corrected);
        assert_eq!(window["artifactAnnotations"].as_array().unwrap().len(),corrected);
        for annotation in window["artifactAnnotations"].as_array().unwrap() {
            assert_ne!(annotation["rawRrMs"],annotation["correctedRrMs"]);
            assert!(annotation["reasons"].as_array().unwrap().contains(&json!("linearTimeInterpolation")));
        }
        assert_eq!(window["accepted"],corrected==3,"{corrected}%: {}",window["reasons"]);
        assert_eq!(window["reasons"].as_array().unwrap().contains(&json!("excessArtifact")),corrected>3);
    }
}

#[test]
fn window_selector_never_bridges_a_missing_time_step() {
    let mut windows=analytical_windows(&[1.0,0.875,0.625,0.5]);
    for row in &mut windows[2..] {
        row["centerElapsedSeconds"]=json!(row["centerElapsedSeconds"].as_f64().unwrap()+300.0);
    }
    let result=thresholds::select_decline(&windows,0.75);
    assert!(result["value"].is_null());
}

#[test]
fn sparse_years_of_elapsed_time_do_not_expand_empty_analysis_windows() {
    let mut normalized=progressive();
    let start=chrono::DateTime::parse_from_rfc3339(normalized["startTime"].as_str().unwrap()).unwrap();
    let end=start+chrono::Duration::days(3650);
    let duration=(end-start).num_seconds() as f64;
    normalized["endTime"]=json!(end.to_rfc3339());
    normalized["summary"]["elapsedTimeSeconds"]=json!(duration);
    normalized["samples"]=json!([{"elapsedSeconds":0,"speedMps":3,"heartRateBpm":120},{"elapsedSeconds":1,"speedMps":3,"heartRateBpm":120},{"elapsedSeconds":duration,"speedMps":3,"heartRateBpm":120}]);
    normalized["rr"]["intervals"]=json!([]);
    let result=analysis::analyze(&normalized);
    assert_eq!(result["segments"][0]["kind"],"unknown");
    assert_eq!(result["segments"][0]["startElapsedSeconds"],0.0);
    assert_eq!(result["segments"][0]["endElapsedSeconds"],duration);
    assert!(result["thresholds"]["lt1"]["value"].is_null());
    assert_eq!(result["thresholds"]["lt1"]["trace"]["counts"]["windows"],0);
}

#[test]
fn timer_event_pause_is_respected_without_sample_timer_flags() {
    let mut normalized=progressive();
    for row in normalized["samples"].as_array_mut().unwrap(){row.as_object_mut().unwrap().remove("timerRunning");}
    normalized["timerEvents"]=json!([{"event":0,"eventType":0,"elapsedSeconds":0},{"event":0,"eventType":1,"elapsedSeconds":180},{"event":0,"eventType":0,"elapsedSeconds":300},{"event":0,"eventType":4,"elapsedSeconds":480}]);
    let result=analysis::analyze(&normalized);
    assert_eq!(result["quality"]["pausedSeconds"],120.0);
    assert_eq!(result["quality"]["timerPauseIntervals"],json!([{"startElapsedSeconds":180.0,"endElapsedSeconds":300.0}]));
    for window in result["thresholds"]["lt1"]["trace"]["windows"].as_array().unwrap() {
        if window["startElapsedSeconds"].as_f64().unwrap()<300.0&&window["endElapsedSeconds"].as_f64().unwrap()>180.0 {assert_eq!(window["accepted"],false);}
    }
}

#[test]
fn cutoff_checks_source_bounds_revision_hash_and_deleted_sources() {
    let normalized=progressive();
    let result=analysis::analyze(&normalized);
    let row=json!({"activityId":"a","startTime":normalized["startTime"],"endTime":normalized["endTime"],"normalized":normalized,"analysis":result});
    for mutation in ["deleted","unavailable","hashMismatch","futureBounds","futureOuter","sourceRevisionMismatch"] {
        let mut changed=row.clone();
        match mutation {
            "deleted"=>changed["deleted"]=json!(true),
            "unavailable"=>changed["sourceUnavailable"]=json!(true),
            "hashMismatch"=>changed["normalized"]["samples"][100]["heartRateBpm"]=json!(200.0),
            "futureBounds"=>changed["normalized"]["endTime"]=json!("2026-01-02T00:08:00Z"),
            "futureOuter"=>changed["endTime"]=json!("2026-01-02T00:08:00Z"),
            "sourceRevisionMismatch"=>changed["normalized"]["sourceRevision"]=json!("unpublished-new-revision"),
            _=>unreachable!(),
        }
        assert!(thresholds::estimate_history("2026-01-01T00:08:00Z",&[changed])["lt1"]["value"].is_null(),"{mutation}");
    }
}

#[test]
fn selected_export_projection_rejects_nested_unselected_lineage() {
    let mut result=analysis::analyze(&progressive())["thresholds"].clone();
    result["lt1"]["method"]["privateHistory"]=json!({"activityId":"unselected-secret"});
    result["lt1"]["value"]["privateHistory"]=json!(["unselected-secret"]);
    result["lt1"]["trace"]["regression"]["privateHistory"]=json!({"samples":[1,2,3]});
    result["lt1"]["trace"]["parameters"]["privateHistory"]=json!("unselected-secret");
    let exported=thresholds::export_projection(&result);
    assert!(!exported.to_string().contains("unselected-secret"));
    assert!(!exported.to_string().contains("privateHistory"));
    assert!(exported["lt1"]["value"]["heartRateBpm"].is_number());
    assert!(exported["lt1"]["trace"]["regression"]["slope"].is_number());
}

fn counter_quantized(mut normalized:Value)->Value {
    let mut previous=0.0;
    for beat in normalized["rr"]["intervals"].as_array_mut().unwrap() {
        let tick=(beat["endElapsedSeconds"].as_f64().unwrap()*1024.0).round();
        beat["rrMs"]=json!((tick-previous)*1000.0/1024.0);
        beat["startElapsedSeconds"]=json!(previous/1024.0);
        beat["endElapsedSeconds"]=json!(tick/1024.0);
        beat["elapsedSeconds"]=json!(tick/1024.0);
        previous=tick;
    }
    for sample in normalized["samples"].as_array_mut().unwrap() {sample["heartRateBpm"]=json!(sample["heartRateBpm"].as_f64().unwrap().round());}
    normalized
}

#[test]
fn original_native_counter_quantization_does_not_force_an_lt2_result() {
    let result=analysis::analyze(&counter_quantized(progressive()));
    let lt1=&result["thresholds"]["lt1"];
    let lt2=&result["thresholds"]["lt2"];
    assert!((lt1["value"]["heartRateBpm"].as_f64().unwrap()-134.71544856745652).abs()<1e-8);
    assert!(lt2["value"].is_null());
    assert_eq!(lt2["reasons"],json!(["noExtrapolation"]));
    let candidate=&lt2["trace"]["candidates"][0];
    assert!((candidate["crossing"].as_f64().unwrap()-157.41934068491915).abs()<1e-8);
    assert_eq!(candidate["regression"]["hrMax"],156.75);
    assert_eq!(candidate["boundaryWindowIndices"],json!([10,56]));
}

#[test]
fn distinct_quantized_positive_rr_matches_both_independent_target_oracles() {
    let source:Value=serde_json::from_str(include_str!("fixtures/runs-engine-progressive-positive.json")).unwrap();
    let golden:Value=serde_json::from_str(include_str!("fixtures/runs-engine-progressive-positive-oracle.json")).unwrap();
    let result=analysis::analyze(&counter_quantized(source));
    let expected=&golden["encodedCounterAndIntegerHr"];
    for slot in ["lt1","lt2"] {
        let target=&result["thresholds"][slot];
        assert_eq!(target["status"],"low_confidence","{slot}: {}",target["reasons"]);
        assert!((target["value"]["heartRateBpm"].as_f64().unwrap()-expected["results"][slot]["value"].as_f64().unwrap()).abs()<1e-8);
        assert_eq!(target["trace"]["counts"]["accepted"],1);
        for (window,oracle) in target["trace"]["windows"].as_array().unwrap().iter().zip(expected["windows"].as_array().unwrap()) {
            if let (Some(a),Some(b))=(window["alpha"].as_f64(),oracle["alpha"].as_f64()) {
                assert!((a-b).abs()<1e-9);
                for (actual,reference) in window["fluctuations"].as_array().unwrap().iter().zip(oracle["fluctuations"].as_array().unwrap()) {
                    assert!((actual.as_f64().unwrap()-reference.as_f64().unwrap()).abs()<1e-8);
                }
            }
        }
    }
    assert_eq!(result["thresholds"]["lt2"]["trace"]["boundaryWindowIndices"],json!([61]));
}

#[test]
fn history_retains_the_real_recent_failed_target_attempt_and_its_calculations() {
    let normalized=counter_quantized(progressive());
    let result=analysis::analyze(&normalized);
    let row=json!({"activityId":"native-original","observationGroupId":"one-run","startTime":normalized["startTime"],"endTime":normalized["endTime"],"normalized":normalized,"analysis":result});
    let history=thresholds::estimate_history("2026-01-01T00:08:00Z",&[row]);
    assert!((history["lt1"]["value"]["heartRateBpm"].as_f64().unwrap()-134.71544856745652).abs()<1e-8);
    let lt2=&history["lt2"];
    assert!(lt2["value"].is_null());
    assert_eq!(lt2["reasons"],json!(["noExtrapolation"]));
    assert_eq!(lt2["evidence"]["activityId"],"native-original");
    assert_eq!(lt2["evidence"]["ageDays"],0.0);
    assert_eq!(lt2["trace"]["counts"]["candidate"],1);
    assert_eq!(lt2["trace"]["counts"]["rejected"],1);
    let candidate=&lt2["trace"]["candidates"][0];
    assert!((candidate["crossing"].as_f64().unwrap()-157.41934068491915).abs()<1e-8);
    assert_eq!(candidate["regression"]["hrMax"],156.75);
    assert!((lt2["trace"]["windows"][56]["alpha"].as_f64().unwrap()-0.49958672767735507).abs()<1e-9);
}

#[test]
fn native_timer_stop_creates_thirty_seconds_pause_without_sample_flags() {
    let mut normalized=progressive();
    for row in normalized["samples"].as_array_mut().unwrap(){row.as_object_mut().unwrap().remove("timerRunning");}
    normalized["summary"]["timerTimeSeconds"]=json!(480.0);
    normalized["timerEvents"]=json!([{"event":0,"eventType":0,"elapsedSeconds":0},{"event":0,"eventType":4,"elapsedSeconds":450}]);
    let original=normalized.clone();
    let result=analysis::analyze(&normalized);
    assert_eq!(normalized,original);
    assert_eq!(result["quality"]["pausedSeconds"],30.0);
    assert_eq!(result["quality"]["recordedTimerTimeSeconds"],480.0);
    assert_eq!(result["quality"]["derivedTimerTimeSeconds"],450.0);
    assert!(result["quality"]["reasons"].as_array().unwrap().contains(&json!("timerTotalsDisagree")));
    let pause=result["segments"].as_array().unwrap().iter().find(|s|s["kind"]=="pause").unwrap();
    assert_eq!(pause["startElapsedSeconds"],450.0);
    assert_eq!(pause["endElapsedSeconds"],480.0);
    for window in result["thresholds"]["lt1"]["trace"]["windows"].as_array().unwrap() {
        if window["endElapsedSeconds"].as_f64().unwrap()>450.0 {assert_eq!(window["accepted"],false);}
    }
}

#[test]
fn immutable_projection_receipt_preserves_real_cached_numerics_without_sample_copy() {
    let mut normalized=counter_quantized(progressive());
    normalized["sourceRevision"]=json!("00000000-0000-4000-8000-000000000001");
    normalized["projectionVersion"]=json!(thresholds::PROJECTION_VERSION);
    let result=analysis::analyze(&normalized);
    let hash=result["thresholds"]["lt1"]["trace"]["inputHash"].clone();
    let mut context=normalized.clone();
    context.as_object_mut().unwrap().remove("samples");
    context.as_object_mut().unwrap().remove("rr");
    let receipt=json!({"kind":"immutableNumericalProjection","revisionId":normalized["sourceRevision"],"inputHash":hash,"projectionVersion":thresholds::PROJECTION_VERSION});
    let row=json!({"activityId":"a","startTime":normalized["startTime"],"endTime":normalized["endTime"],"normalized":context,"analysis":result,"evidenceVerification":receipt});
    let history=thresholds::estimate_history("2026-01-01T00:08:00Z",&[row.clone()]);
    assert!((history["lt1"]["value"]["heartRateBpm"].as_f64().unwrap()-134.71544856745652).abs()<1e-8);
    assert_eq!(history["lt2"]["reasons"],json!(["noExtrapolation"]));
    assert_eq!(history["lt2"]["trace"]["counts"]["rejected"],1);
    for mutation in ["revisionId","projectionVersion","inputHash","kind"] {
        let mut changed=row.clone();
        changed["evidenceVerification"][mutation]=json!("invalid-proof");
        assert!(thresholds::estimate_history("2026-01-01T00:08:00Z",&[changed])["lt1"]["value"].is_null(),"{mutation}");
    }
    let mut full=row.clone();
    full["normalized"]=normalized;
    full["normalized"]["samples"][100]["heartRateBpm"]=json!(200.0);
    assert!(thresholds::estimate_history("2026-01-01T00:08:00Z",&[full])["lt1"]["value"].is_null());
}

#[test]
fn a_new_failed_target_does_not_remove_older_eligible_independent_target() {
    let mut older:Value=serde_json::from_str(include_str!("fixtures/runs-engine-progressive-positive.json")).unwrap();
    older=counter_quantized(older);
    older["startTime"]=json!("2025-12-31T00:00:00Z");
    older["endTime"]=json!("2025-12-31T00:08:00Z");
    let old_analysis=analysis::analyze(&older);
    let recent=counter_quantized(progressive());
    let recent_analysis=analysis::analyze(&recent);
    let rows=[
        json!({"activityId":"older","observationGroupId":"older-run","startTime":older["startTime"],"endTime":older["endTime"],"normalized":older,"analysis":old_analysis}),
        json!({"activityId":"recent","observationGroupId":"recent-run","startTime":recent["startTime"],"endTime":recent["endTime"],"normalized":recent,"analysis":recent_analysis}),
    ];
    let result=thresholds::estimate_history("2026-01-01T00:08:00Z",&rows);
    assert_eq!(result["lt1"]["evidence"]["activityId"],"recent");
    assert_eq!(result["lt2"]["evidence"]["activityId"],"older");
    assert!((result["lt2"]["value"]["heartRateBpm"].as_f64().unwrap()-160.42106261369022).abs()<1e-8);
    assert_eq!(result["lt2"]["evidence"]["ageDays"],1.0);
}

#[test]
fn independent_historical_targets_report_conflict_without_clamping() {
    let mut older:Value=serde_json::from_str(include_str!("fixtures/runs-engine-progressive-positive.json")).unwrap();
    older=counter_quantized(older);
    older["startTime"]=json!("2025-12-31T00:00:00Z");
    older["endTime"]=json!("2025-12-31T00:08:00Z");
    for sample in older["samples"].as_array_mut().unwrap() {
        sample["heartRateBpm"]=json!(sample["heartRateBpm"].as_f64().unwrap()-30.0);
    }
    let recent=counter_quantized(progressive());
    let result=thresholds::estimate_history("2026-01-01T00:08:00Z",&[
        json!({"activityId":"older","startTime":older["startTime"],"endTime":older["endTime"],"analysis":analysis::analyze(&older),"normalized":older}),
        json!({"activityId":"recent","startTime":recent["startTime"],"endTime":recent["endTime"],"analysis":analysis::analyze(&recent),"normalized":recent}),
    ]);
    assert!((result["lt1"]["value"]["heartRateBpm"].as_f64().unwrap()-134.71544856745652).abs()<1e-8);
    assert!((result["lt2"]["value"]["heartRateBpm"].as_f64().unwrap()-130.42106261369022).abs()<1e-8);
    for slot in ["lt1","lt2"] {
        assert!(result[slot]["reasons"].as_array().unwrap().contains(&json!("conflictingTargetOrder")));
        assert_eq!(result[slot]["status"],"low_confidence");
    }
}

#[test]
fn an_isolated_out_of_range_rr_keeps_its_correctable_continuous_window() {
    let mut normalized=progressive();
    normalized["rr"]["intervals"][400]["rrMs"]=json!(200.0);
    let mut cursor=0.0;
    for beat in normalized["rr"]["intervals"].as_array_mut().unwrap() {
        beat["startElapsedSeconds"]=json!(cursor);
        cursor+=beat["rrMs"].as_f64().unwrap()/1000.0;
        beat["endElapsedSeconds"]=json!(cursor);
        beat["elapsedSeconds"]=json!(cursor);
    }
    let result=analysis::analyze(&normalized);
    let window=result["thresholds"]["lt1"]["trace"]["windows"].as_array().unwrap().iter().find(|w|w["centerElapsedSeconds"]==240.0).expect("continuous corrected RR window at 240 seconds");
    assert_eq!(window["accepted"],true,"{}",window["reasons"]);
    assert_eq!(window["invalidBeatCount"],1);
    assert_eq!(window["correctedBeatCount"],1);
    let artifact=window["artifactAnnotations"].as_array().unwrap().iter().find(|a|a["beatIndex"]==400).unwrap();
    assert_eq!(artifact["rawRrMs"],200.0);
    assert!(artifact["correctedRrMs"].as_f64().unwrap()>250.0);
    assert!(artifact["reasons"].as_array().unwrap().contains(&json!("linearTimeInterpolation")));
    assert_eq!(normalized["rr"]["intervals"][400]["rrMs"],200.0);
}

#[test]
fn out_of_range_speed_endpoints_do_not_create_observed_progression_support() {
    let mut normalized=progressive();
    for (index,sample) in normalized["samples"].as_array_mut().unwrap().iter_mut().enumerate() {
        if index%30!=0 {sample["speedMps"]=json!(16.0);}
    }
    let result=analysis::analyze(&normalized);
    for slot in ["lt1","lt2"] {
        assert!(result["thresholds"][slot]["value"].is_null());
        assert!(result["thresholds"][slot]["reasons"].as_array().unwrap().contains(&json!("unsupportedProtocol")));
    }
    assert_eq!(result["quality"]["coverage"]["speed"],0.0);
    for segment in result["segments"].as_array().unwrap() {
        assert_eq!(segment["features"]["coverage"]["speed"],0.0);
        assert_eq!(segment["eligibility"]["lt1"]["accepted"],false);
    }
    assert_eq!(normalized["samples"][1]["speedMps"],16.0);
}

#[test]
fn unavailable_timer_state_abstains_and_partial_support_is_window_local() {
    let mut normalized=progressive();
    for sample in normalized["samples"].as_array_mut().unwrap() {sample.as_object_mut().unwrap().remove("timerRunning");}
    let result=analysis::analyze(&normalized);
    assert_eq!(result["quality"]["timerCoverage"],"unavailable");
    assert!(result["thresholds"]["lt1"]["value"].is_null());
    assert!(result["thresholds"]["lt1"]["reasons"].as_array().unwrap().contains(&json!("requiredContextUnprovable")));
    for sample in &mut normalized["samples"].as_array_mut().unwrap()[..180] {sample["timerRunning"]=json!(true);}
    let partial=analysis::analyze(&normalized);
    assert_eq!(partial["quality"]["timerCoverage"],"partial");
    let windows=partial["thresholds"]["lt1"]["trace"]["windows"].as_array().unwrap();
    assert_eq!(windows.iter().find(|w|w["centerElapsedSeconds"]==120.0).unwrap()["accepted"],true);
    assert!(windows.iter().find(|w|w["centerElapsedSeconds"]==125.0).unwrap()["reasons"].as_array().unwrap().contains(&json!("requiredContextUnprovable")));
}

#[test]
fn stale_duplicate_cache_does_not_hide_current_eligible_representative() {
    let normalized=counter_quantized(progressive());
    let evaluated=analysis::analyze(&normalized);
    let mut stale=evaluated.clone();
    stale["thresholds"]["lt1"]["trace"]["inputHash"]=json!("stale");
    stale["thresholds"]["lt2"]["trace"]["inputHash"]=json!("stale");
    let result=thresholds::estimate_history("2026-01-01T00:08:00Z",&[
        json!({"activityId":"a","observationGroupId":"same-run","startTime":normalized["startTime"],"endTime":normalized["endTime"],"normalized":normalized,"analysis":stale}),
        json!({"activityId":"b","observationGroupId":"same-run","startTime":normalized["startTime"],"endTime":normalized["endTime"],"normalized":normalized,"analysis":evaluated}),
    ]);
    assert!((result["lt1"]["value"]["heartRateBpm"].as_f64().unwrap()-134.71544856745652).abs()<1e-8);
    assert_eq!(result["lt1"]["evidence"]["activityId"],"b");
    assert_eq!(result["lt1"]["evidence"]["independentActivityCount"],1);
    assert_eq!(result["lt2"]["reasons"],json!(["noExtrapolation"]));
    assert_eq!(result["lt2"]["evidence"]["activityId"],"b");
}

#[test]
fn regression_candidate_counts_unique_invalid_and_missing_beats() {
    let result=analysis::analyze(&counter_quantized(progressive()));
    let candidate=&result["thresholds"]["lt2"]["trace"]["candidates"][0];
    assert_eq!(candidate["accepted"],false);
    assert_eq!(candidate["reasons"],json!(["noExtrapolation"]));
    assert_eq!(candidate["invalidBeatCount"],0);
    assert_eq!(candidate["missingBeatCount"],0);
    assert_eq!(candidate["correctedBeatCount"],0);
}

#[test]
fn export_preserves_unattempted_target_and_real_failed_target_nulls() {
    let actual=analysis::analyze(&counter_quantized(progressive()));
    let result=thresholds::export_projection(&json!({"lt1":null,"lt2":actual["thresholds"]["lt2"]}));
    assert!(result["lt1"].is_null());
    assert!(result["lt2"]["value"].is_null());
    assert_eq!(result["lt2"]["status"],"insufficient_data");
    assert_eq!(result["lt2"]["reasons"],json!(["noExtrapolation"]));
    assert_eq!(result["lt2"]["trace"]["counts"]["rejected"],1);
}
