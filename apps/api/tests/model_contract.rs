use garmin_fit_extractor_api::model::{
    Activity, ActivitySummary, Analysis, Calories, CoachDerivedMetrics, CoachHeartRateZone,
    CoachLap, CurrentUserResponse, Metric, Source, UserProfile,
};

#[test]
fn analysis_serializes_required_nulls_and_fixed_units() {
    let analysis = Analysis::empty("activity.fit");
    let json = serde_json::to_value(&analysis).expect("analysis must serialize");

    assert_eq!(json["schemaVersion"], "1.0.0");
    assert_eq!(json["source"]["fileName"], "activity.fit");
    assert!(json["activity"]["type"].is_null());
    assert!(json["summary"]["calories"]["value"].is_null());
    assert_eq!(json["summary"]["calories"]["unit"], "kcal");
}

#[test]
fn analysis_accepts_pre_chart_normalized_json_with_missing_chart_fields() {
    let mut json =
        serde_json::to_value(Analysis::empty("activity.fit")).expect("analysis must serialize");
    json.as_object_mut()
        .expect("analysis object")
        .remove("samples");
    json["power"]
        .as_object_mut()
        .expect("power object")
        .remove("zones");

    let parsed: Analysis = serde_json::from_value(json).expect("legacy analysis must parse");
    assert!(parsed.samples.is_empty());
    assert!(parsed.power.zones.is_empty());
}

#[test]
fn analysis_rejects_an_incompatible_schema_version() {
    let value = serde_json::json!({
        "schemaVersion": "2.0.0",
        "source": { "fileName": "activity.fit" }
    });

    assert!(serde_json::from_value::<Analysis>(value).is_err());
}

#[test]
fn metric_and_calories_keep_explicit_null_values() {
    let metric = Metric {
        value: None,
        unit: "meters".into(),
    };
    let calories = Calories {
        value: None,
        unit: "kcal".into(),
    };

    assert_eq!(
        serde_json::to_value(metric).unwrap(),
        serde_json::json!({"value": null, "unit": "meters"})
    );
    assert_eq!(
        serde_json::to_value(calories).unwrap(),
        serde_json::json!({"value": null, "unit": "kcal"})
    );
}

#[test]
fn source_is_camel_cased() {
    assert_eq!(
        serde_json::to_value(Source {
            file_name: "activity.fit".into()
        })
        .unwrap(),
        serde_json::json!({"fileName": "activity.fit"})
    );
}

#[test]
fn current_user_serializes_with_camel_case_profile_fields() {
    let user = CurrentUserResponse {
        user: Some(UserProfile {
            id: uuid::Uuid::from_u128(1),
            email: "alice@example.test".into(),
            display_name: Some("Alice".into()),
        }),
    };

    assert_eq!(
        serde_json::to_value(user).unwrap(),
        serde_json::json!({
            "user": {
                "id": "00000000-0000-0000-0000-000000000001",
                "email": "alice@example.test",
                "displayName": "Alice"
            }
        })
    );
}

#[test]
fn coach_activity_serializes_snake_case_and_safe_shape() {
    let activity = Activity {
        summary: ActivitySummary {
            activity_id: uuid::Uuid::from_u128(2),
            sport: Some("running".into()),
            started_at: "2025-01-02T03:04:05Z".into(),
            distance_m: Some(10_000.5),
            duration_s: Some(3_600),
        },
        average_heart_rate: Some(145),
        maximum_heart_rate: Some(170),
        average_pace_s_per_km: Some(360.0),
        elevation_gain_m: Some(120.0),
        laps: vec![CoachLap {
            index: 1,
            start_time: Some("2025-01-02T03:04:05Z".into()),
            distance_m: Some(1_000.0),
            duration_s: Some(360),
            moving_time_s: Some(350),
            pace_s_per_km: Some(350.0),
            average_heart_rate: Some(140),
            maximum_heart_rate: Some(155),
            average_power_w: Some(250),
            maximum_power_w: Some(300),
            average_cadence_spm: Some(185.0),
            maximum_cadence_spm: Some(190.0),
        }],
        heart_rate_zones: vec![CoachHeartRateZone {
            zone: 3,
            min_bpm: Some(140),
            max_bpm: Some(159),
            duration_s: Some(900.0),
        }],
        derived_metrics: CoachDerivedMetrics {
            heart_rate_drift_percent: Some(2.5),
        },
    };

    let value = serde_json::to_value(activity).expect("coach activity must serialize");
    assert_eq!(value["activity_id"], "00000000-0000-0000-0000-000000000002");
    assert_eq!(value["average_heart_rate"], 145);
    assert_eq!(value["average_pace_s_per_km"], 360.0);
    assert_eq!(value["laps"][0]["start_time"], "2025-01-02T03:04:05Z");
    assert_eq!(value["laps"][0]["average_cadence_spm"], 185.0);
    assert_eq!(value["heart_rate_zones"][0]["min_bpm"], 140);
    assert_eq!(value["derived_metrics"]["heart_rate_drift_percent"], 2.5);
    assert!(value.get("activityId").is_none());
}

#[test]
fn coach_activity_omits_optional_scalars_but_keeps_collections() {
    let value = serde_json::to_value(Activity {
        summary: ActivitySummary {
            activity_id: uuid::Uuid::from_u128(3),
            sport: None,
            started_at: "2025-01-02T03:04:05Z".into(),
            distance_m: None,
            duration_s: None,
        },
        average_heart_rate: None,
        maximum_heart_rate: None,
        average_pace_s_per_km: None,
        elevation_gain_m: None,
        laps: vec![CoachLap {
            index: 1,
            start_time: None,
            distance_m: None,
            duration_s: None,
            moving_time_s: None,
            pace_s_per_km: None,
            average_heart_rate: None,
            maximum_heart_rate: None,
            average_power_w: None,
            maximum_power_w: None,
            average_cadence_spm: None,
            maximum_cadence_spm: None,
        }],
        heart_rate_zones: vec![CoachHeartRateZone {
            zone: 1,
            min_bpm: None,
            max_bpm: None,
            duration_s: None,
        }],
        derived_metrics: CoachDerivedMetrics {
            heart_rate_drift_percent: None,
        },
    })
    .expect("coach activity must serialize");

    for field in [
        "sport",
        "distance_m",
        "duration_s",
        "average_heart_rate",
        "maximum_heart_rate",
        "average_pace_s_per_km",
        "elevation_gain_m",
    ] {
        assert!(value.get(field).is_none(), "{field} should be omitted");
    }
    assert_eq!(value["laps"][0]["index"], 1);
    assert!(value["laps"][0].get("distance_m").is_none());
    assert_eq!(value["heart_rate_zones"][0]["zone"], 1);
    assert!(value["heart_rate_zones"][0].get("min_bpm").is_none());
    assert_eq!(value["derived_metrics"], serde_json::json!({}));
}

#[test]
fn coach_activity_contains_no_identity_or_token_fields() {
    let value = serde_json::to_string(&Activity {
        summary: ActivitySummary {
            activity_id: uuid::Uuid::from_u128(4),
            sport: None,
            started_at: "2025-01-02T03:04:05Z".into(),
            distance_m: None,
            duration_s: None,
        },
        average_heart_rate: None,
        maximum_heart_rate: None,
        average_pace_s_per_km: None,
        elevation_gain_m: None,
        laps: vec![],
        heart_rate_zones: vec![],
        derived_metrics: CoachDerivedMetrics {
            heart_rate_drift_percent: None,
        },
    })
    .expect("coach activity must serialize");
    for forbidden in [
        "owner_id",
        "user_id",
        "email",
        "google_subject",
        "access_token",
        "refresh_token",
    ] {
        assert!(!value.contains(forbidden), "forbidden field {forbidden}");
    }
}
