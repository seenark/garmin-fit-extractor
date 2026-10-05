use fitparser::de::{from_bytes_with_options, DecodeOption};
use fitparser::{from_bytes, Value};
use std::collections::HashSet;

fn crc(bytes: &[u8]) -> u16 {
    let mut crc = 0u16;
    for &byte in bytes {
        crc ^= u16::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xa001 } else { crc >> 1 };
        }
    }
    crc
}

fn hr_rr_order(anchor: u32, packed: &[u8]) -> Vec<u8> {
    let mut body = vec![0x40, 0, 0, 132, 0, 1, 9, 4, 0x86, 0];
    body.extend_from_slice(&anchor.to_le_bytes());
    body.extend_from_slice(&[0x40, 0, 0, 132, 0, 1, 10, packed.len() as u8, 0x0d, 0]);
    body.extend_from_slice(packed);
    envelope(&body)
}

fn envelope(body: &[u8]) -> Vec<u8> {
    let mut fit = vec![12, 0x20, 0xea, 0x07];
    fit.extend_from_slice(&(body.len() as u32).to_le_bytes());
    fit.extend_from_slice(b".FIT");
    fit.extend_from_slice(&body);
    fit.extend_from_slice(&crc(&fit).to_le_bytes());
    fit
}

fn events(record: &fitparser::FitDataRecord) -> &Value {
    record.fields().iter().find(|field| field.name() == "event_timestamp").unwrap().value()
}

#[test]
fn hr_rr_order_uses_full_anchor_and_only_complete_components() {
    let fit = hr_rr_order(102400, &[0x00, 0x04, 0x80]);
    let records = from_bytes(&fit).unwrap();
    assert_eq!(events(&records[0]), &Value::Float64(100.0));
    assert_eq!(events(&records[1]), &Value::Array(vec![Value::Float64(101.0), Value::Float64(102.0)]));
    let expanded = records[1].fields().iter().find(|field| field.name() == "event_timestamp").unwrap();
    assert_eq!(expanded.component_parent(), Some(10));
    assert_eq!(expanded.scale(), Some(1024.0));
    assert_eq!(expanded.profile_type(), Some("uint32"));
    assert_eq!(records[0].fields()[0].component_parent(), None);
    let options = HashSet::from([DecodeOption::KeepCompositeFields]);
    let records = from_bytes_with_options(&fit, &options).unwrap();
    assert!(records[1].fields().iter().find(|field| field.name() == "event_timestamp_12").unwrap().is_composite());
    assert_eq!(records[1].fields().iter().find(|field| field.name() == "event_timestamp_12").unwrap().value(),
        &Value::Array(vec![Value::Byte(0), Value::Byte(4), Value::Byte(128)]));
}

#[test]
fn zero_component_rolls_over_and_incomplete_tail_is_absent() {
    let records = from_bytes(&hr_rr_order(105472, &[0x00, 0x00, 0x40])).unwrap();
    assert_eq!(events(&records[1]), &Value::Array(vec![Value::Float64(104.0), Value::Float64(105.0)]));
    let records = from_bytes(&hr_rr_order(102400, &[0x00, 0x04])).unwrap();
    assert_eq!(events(&records[1]), &Value::Array(vec![Value::Float64(101.0)]));
}

#[test]
fn packed_ff_bytes_are_not_removed_or_zero_filled() {
    let records = from_bytes(&hr_rr_order(102400, &[0xff, 0x03, 0x00])).unwrap();
    assert_eq!(events(&records[1]), &Value::Array(vec![Value::Float64(100.9990234375), Value::Float64(104.0)]));
}

#[test]
fn invalid_native_presence_is_opt_in_and_never_becomes_epoch() {
    let mut body = vec![0x40, 0, 0, 20, 0, 3, 253, 4, 0x86, 3, 2, 2, 5, 8, 0x86, 0];
    body.extend_from_slice(&u32::MAX.to_le_bytes());
    body.extend_from_slice(&[255, 255]);
    body.extend_from_slice(&100u32.to_le_bytes());
    body.extend_from_slice(&u32::MAX.to_le_bytes());
    let fit = envelope(&body);
    let legacy = from_bytes(&fit).unwrap();
    assert!(legacy[0].fields().iter().all(|field| field.number() != 253 && field.number() != 3));
    let options = HashSet::from([DecodeOption::PreserveInvalidValues]);
    let archived = from_bytes_with_options(&fit, &options).unwrap();
    assert_eq!(archived[0].fields().iter().find(|field| field.number() == 253).unwrap().value(), &Value::Invalid);
    assert_eq!(archived[0].fields().iter().find(|field| field.number() == 3).unwrap().value(), &Value::Array(vec![Value::Invalid, Value::Invalid]));
    assert_eq!(archived[0].fields().iter().find(|field| field.number() == 5).unwrap().value(), &Value::Array(vec![Value::Float64(1.0), Value::Invalid]));
}

#[test]
fn undescribed_developer_bytes_are_opt_in_and_keep_developer_identity() {
    let fit = envelope(&[0x60, 0, 0, 20, 0, 0, 1, 2, 3, 7, 0, 0xff, 0, 9]);
    assert!(from_bytes(&fit).is_err());
    let options = HashSet::from([DecodeOption::PreserveUnknownDeveloperFields]);
    let archived = from_bytes_with_options(&fit, &options).unwrap();
    let field = &archived[0].fields()[0];
    assert_eq!(field.developer_data_index(), Some(7));
    assert_eq!(field.value(), &Value::Array(vec![Value::Byte(255), Value::Byte(0), Value::Byte(9)]));
    assert_eq!(field.scale(), None);
    assert_eq!(field.profile_type(), None);
    let opaque_ff = envelope(&[0x60, 0, 0, 20, 0, 0, 1, 2, 1, 7, 0, 255]);
    let archived = from_bytes_with_options(&opaque_ff, &options).unwrap();
    assert_eq!(archived[0].fields()[0].value(), &Value::Byte(255));
}

#[test]
fn accumulated_distance_uses_native_scale_and_rejects_partial_component() {
    let mut body = vec![0x40, 0, 0, 20, 0, 1, 5, 4, 0x86, 0];
    body.extend_from_slice(&10000u32.to_le_bytes());
    body.extend_from_slice(&[0x40, 0, 0, 20, 0, 1, 8, 3, 0x0d, 0, 0, 0, 0x65]);
    let records = from_bytes(&envelope(&body)).unwrap();
    let distance = records[1].fields().iter().find(|field| field.name() == "distance").unwrap();
    assert_eq!(distance.value(), &Value::Float64(101.0));
    assert_eq!(distance.component_parent(), Some(8));
    body.extend_from_slice(&[0x40, 0, 0, 20, 0, 1, 8, 2, 0x0d, 0, 0, 0]);
    let records = from_bytes(&envelope(&body)).unwrap();
    assert!(records[2].fields().iter().all(|field| field.name() != "distance"));
}

#[test]
fn carry_continues_between_packed_messages() {
    let mut body = vec![0x40, 0, 0, 132, 0, 1, 9, 4, 0x86, 0];
    body.extend_from_slice(&102400u32.to_le_bytes());
    body.extend_from_slice(&[0x40, 0, 0, 132, 0, 1, 10, 3, 0x0d, 0, 0, 4, 128, 0, 0, 12, 0]);
    let records = from_bytes(&envelope(&body)).unwrap();
    assert_eq!(events(&records[2]), &Value::Array(vec![Value::Float64(103.0), Value::Float64(104.0)]));
}

#[test]
fn archived_ff_composite_remains_packed_bits_even_when_no_event_is_complete() {
    let options = HashSet::from([DecodeOption::KeepCompositeFields, DecodeOption::PreserveInvalidValues]);
    let records = from_bytes_with_options(&hr_rr_order(102400, &[255]), &options).unwrap();
    assert_eq!(records[1].fields()[0].value(), &Value::Byte(255));
    assert!(records[1].fields()[0].is_composite());
    assert_eq!(records[1].fields()[0].name(), "event_timestamp_12");
    let records = from_bytes_with_options(&hr_rr_order(102400, &[255, 255, 255]), &options).unwrap();
    assert_eq!(events(&records[1]), &Value::Array(vec![Value::Float64(103.9990234375), Value::Float64(103.9990234375)]));
}
