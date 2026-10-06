use garmin_fit_extractor_api::{error::FitError, fit::raw::decode_raw};

const ACTIVITY: &[u8] = include_bytes!("fixtures/activity.fit");

#[test]
fn public_fixture_preserves_ordered_activity_messages_and_running_profile() {
    let records = decode_raw(ACTIVITY).expect("MIT fixture has valid CRCs");
    let kinds = records
        .iter()
        .map(|record| record.kind.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        [
            "file_id",
            "file_creator",
            "event",
            "record",
            "record",
            "record",
            "record",
            "record",
            "record",
            "record",
            "record",
            "record",
            "record",
            "record",
            "record",
            "record",
            "record",
            "event",
            "lap",
            "event",
            "session",
            "activity",
        ]
    );
    let sport = records[20]
        .fields
        .iter()
        .find(|field| field.name == "sport")
        .unwrap();
    assert_eq!(sport.value, serde_json::json!("running"));
}

#[test]
fn invalid_or_crc_corrupted_bytes_are_invalid_fit() {
    assert!(matches!(
        decode_raw(b"not a fit file"),
        Err(FitError::InvalidFit)
    ));

    let mut corrupt = ACTIVITY.to_vec();
    *corrupt.last_mut().expect("fixture is nonempty") ^= 0xff;
    assert!(matches!(decode_raw(&corrupt), Err(FitError::InvalidFit)));
}

#[test]
fn packed_hr_reads_only_encoded_events_and_carries_full_anchor() {
    // CC0 synthetic protocol stream. FIT uint32 anchor 102400/1024 = 100 s.
    let bytes = [
        14, 32, 225, 82, 104, 0, 0, 0, 46, 70, 73, 84, 43, 251, 79, 0, 0, 0, 0, 2, 0, 1, 0, 1, 2,
        132, 15, 4, 1, 0, 64, 0, 0, 20, 0, 2, 253, 4, 134, 3, 1, 2, 0, 0, 202, 154, 59, 100, 65, 0,
        0, 132, 0, 4, 253, 4, 134, 0, 2, 132, 6, 1, 2, 9, 4, 134, 1, 0, 202, 154, 59, 0, 0, 120, 0,
        144, 1, 0, 66, 0, 0, 78, 0, 1, 0, 6, 132, 2, 32, 3, 255, 255, 42, 3, 0, 1, 202, 154, 59,
        101, 65, 0, 0, 132, 0, 2, 6, 2, 2, 10, 3, 13, 1, 121, 122, 0, 4, 128, 252, 220,
    ];
    let records = decode_raw(&bytes).expect("synthetic CRC-valid stream");
    let events = records
        .iter()
        .filter(|record| record.kind == "hr")
        .flat_map(|record| &record.fields)
        .filter(|field| field.name == "event_timestamp")
        .map(|field| field.value.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        events,
        vec![serde_json::json!(100.0), serde_json::json!([101.0, 102.0])]
    );
}
