use garmin_fit_extractor_api::runs::history::latest_projection;
use serde_json::json;

#[test]
fn latest_preserves_each_targets_actual_prior_result_and_cutoff() {
    let older = json!({"activityId":"older","evidenceCutoff":"2026-10-01T10:00:00Z","computedAt":"2026-10-02T12:00:00Z","lt1":{"status":"low_confidence","value":{"heartRateBpm":150.0}},"lt2":{"status":"insufficient_data","value":null}});
    let newer = json!({"activityId":"newer","evidenceCutoff":"2026-10-03T10:00:00Z","computedAt":"2026-10-04T12:00:00Z","lt1":{"status":"insufficient_data","value":null},"lt2":{"status":"low_confidence","value":{"heartRateBpm":170.0}}});
    let attempt = json!({"activityId":null,"evidenceCutoff":"2026-10-05T10:00:00Z","computedAt":"2026-10-05T10:00:00Z","lt1":{"status":"insufficient_data","value":null},"lt2":{"status":"insufficient_data","value":null}});
    let result = latest_projection("2026-10-05T10:00:00Z", attempt.clone(), &[older.clone(), newer.clone()]);
    assert_eq!(result["lastAvailable"]["lt1"]["value"]["heartRateBpm"], 150.0);
    assert_eq!(result["lastAvailable"]["lt1"]["activityId"], "older");
    assert_eq!(result["lastAvailable"]["lt1"]["evidenceCutoff"], "2026-10-01T10:00:00Z");
    assert_eq!(result["lastAvailable"]["lt1"]["computedAt"], "2026-10-02T12:00:00Z");
    assert_eq!(result["lastAvailable"]["lt2"]["value"]["heartRateBpm"], 170.0);
    assert_eq!(result["lastAvailable"]["lt1"]["stale"], true);
    assert_eq!(result["latestAttempt"], attempt);
    let reverse_older = json!({"activityId":"older","evidenceCutoff":"2026-10-01T10:00:00Z","computedAt":"2026-10-02T12:00:00Z","lt1":{"status":"insufficient_data","value":null},"lt2":{"status":"low_confidence","value":{"heartRateBpm":165.0}}});
    let reverse_newer = json!({"activityId":"newer","evidenceCutoff":"2026-10-03T10:00:00Z","computedAt":"2026-10-04T12:00:00Z","lt1":{"status":"low_confidence","value":{"heartRateBpm":155.0}},"lt2":{"status":"insufficient_data","value":null}});
    let reverse = latest_projection("2026-10-05T10:00:00Z", attempt, &[reverse_newer, reverse_older]);
    assert_eq!(reverse["lastAvailable"]["lt1"]["value"]["heartRateBpm"], 155.0);
    assert_eq!(reverse["lastAvailable"]["lt2"]["value"]["heartRateBpm"], 165.0);
    assert_eq!(reverse["lastAvailable"]["lt2"]["activityId"], "older");
    assert_eq!(reverse["lastAvailable"]["lt2"]["evidenceCutoff"], "2026-10-01T10:00:00Z");
}

#[test]
fn latest_numeric_attempt_replaces_only_its_target_without_false_staleness(){
    let previous=json!({"activityId":"older","evidenceCutoff":"2026-10-01T10:00:00Z","computedAt":"2026-10-02T12:00:00Z","lt1":{"status":"low_confidence","value":{"heartRateBpm":150.0}},"lt2":{"status":"low_confidence","value":{"heartRateBpm":165.0}}});
    let attempt=json!({"activityId":null,"evidenceCutoff":"2026-10-05T10:00:00Z","computedAt":"2026-10-05T10:00:01Z","lt1":{"status":"insufficient_data","value":null},"lt2":{"status":"low_confidence","value":{"heartRateBpm":170.0}}});
    let result=latest_projection("2026-10-05T10:00:00Z",attempt,&[previous]);
    assert_eq!(result["lastAvailable"]["lt1"]["value"]["heartRateBpm"],150.0);
    assert_eq!(result["lastAvailable"]["lt1"]["computedAt"],"2026-10-02T12:00:00Z");
    assert_eq!(result["lastAvailable"]["lt1"]["stale"],true);
    assert_eq!(result["lastAvailable"]["lt2"]["value"]["heartRateBpm"],170.0);
    assert!(result["lastAvailable"]["lt2"]["activityId"].is_null());
    assert_eq!(result["lastAvailable"]["lt2"]["evidenceCutoff"],"2026-10-05T10:00:00Z");
    assert_eq!(result["lastAvailable"]["lt2"]["computedAt"],"2026-10-05T10:00:01Z");
    assert_eq!(result["lastAvailable"]["lt2"]["stale"],false);
}

#[test]
fn superseded_history_at_same_cutoff_keeps_number_but_is_stale(){
    let previous=json!({"activityId":"same-run","evidenceCutoff":"2026-10-05T10:00:00Z","computedAt":"2026-10-05T10:00:00Z","_historyGeneration":1,"_historyStale":true,"lt1":{"status":"low_confidence","value":{"heartRateBpm":150.0}},"lt2":{"status":"insufficient_data","value":null}});
    let attempt=json!({"activityId":null,"evidenceCutoff":"2026-10-05T10:00:00Z","computedAt":"2026-10-05T10:00:01Z","lt1":{"status":"insufficient_data","value":null},"lt2":{"status":"insufficient_data","value":null}});
    let result=latest_projection("2026-10-05T10:00:00Z",attempt.clone(),&[previous.clone()]);
    assert_eq!(result["lastAvailable"]["lt1"]["value"]["heartRateBpm"],150.0);
    assert_eq!(result["lastAvailable"]["lt1"]["computedAt"],"2026-10-05T10:00:00Z");
    assert_eq!(result["lastAvailable"]["lt1"]["stale"],true);
    assert_eq!(result["stale"],true);
    let mut current=previous.clone();
    current["_historyGeneration"]=json!(2);
    current["_historyStale"]=json!(false);
    current["lt1"]["value"]["heartRateBpm"]=json!(155.0);
    let result=latest_projection("2026-10-05T10:00:00Z",attempt,&[current,previous]);
    assert_eq!(result["lastAvailable"]["lt1"]["value"]["heartRateBpm"],155.0);
    assert_eq!(result["lastAvailable"]["lt1"]["stale"],false);
}
