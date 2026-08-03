use garmin_fit_extractor_api::{error::FitError, fit::raw::decode_raw};

const ACTIVITY: &[u8] = include_bytes!("fixtures/activity.fit");

#[test]
fn activity_fixture_decodes_with_default_crc_validation_and_preserves_decoder_order() {
    let records = decode_raw(ACTIVITY).expect("the licensed Activity.fit fixture has valid CRCs");

    assert!(!records.is_empty());
    assert!(records.iter().any(|record| record.kind == "session"));
    assert!(records.iter().all(|record| !record.fields.is_empty()));

    let serialized = serde_json::to_value(&records).expect("raw records serialize");
    assert!(
        serialized
            .as_array()
            .is_some_and(|records| !records.is_empty())
    );
    assert!(
        records
            .iter()
            .flat_map(|record| &record.fields)
            .all(|field| {
                field.value.is_number()
                    || field.value.is_string()
                    || field.value.is_array()
                    || field.value.is_null()
            })
    );
}

#[test]
fn fields_with_empty_fit_units_omit_the_units_property() {
    let records = decode_raw(ACTIVITY).expect("fixture decodes");
    let field = records
        .iter()
        .flat_map(|record| &record.fields)
        .find(|field| field.units.is_none())
        .expect("fixture has a field without FIT units");

    assert!(
        serde_json::to_value(field)
            .expect("field serializes")
            .get("units")
            .is_none()
    );
}

#[test]
fn known_profile_enums_are_strings_when_present_and_unknown_enums_remain_numeric() {
    let records = decode_raw(ACTIVITY).expect("fixture decodes");
    let sport = records
        .iter()
        .find(|record| record.kind == "session")
        .and_then(|record| record.fields.iter().find(|field| field.name == "sport"));

    assert!(sport.is_some_and(|field| field.value.is_string()));
    assert!(sport.is_some_and(|field| field.value != serde_json::json!(0)));
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
