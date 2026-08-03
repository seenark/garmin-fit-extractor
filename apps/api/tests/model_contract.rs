use garmin_fit_extractor_api::model::{Analysis, Calories, Metric, Source};

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
