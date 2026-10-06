use std::io::{Cursor, Write};

use garmin_fit_extractor_api::runs::import::{InputStatus, Upload, stage};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

fn fit() -> Vec<u8> {
    let mut bytes = vec![0; 14];
    bytes[0] = 14;
    bytes[8..12].copy_from_slice(b".FIT");
    bytes
}

fn archive(members: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in members {
        writer
            .start_file(
                *name,
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
            )
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn direct_and_archive_stage_identical_bytes_with_safe_partial_reports() {
    let bytes = fit();
    let zip = archive(&[
        ("folder/run.FiT", &bytes),
        ("../bad.fit", &bytes),
        ("notes.txt", b"text"),
        ("broken.fit", b"not FIT"),
    ]);
    let items = stage(vec![
        Upload {
            name: "direct.FIT".into(),
            bytes: bytes.clone(),
        },
        Upload {
            name: "runs.ZIP".into(),
            bytes: zip,
        },
    ])
    .unwrap();
    assert_eq!(items[0].bytes.as_deref(), Some(bytes.as_slice()));
    assert_eq!(items[1].bytes.as_deref(), Some(bytes.as_slice()));
    assert_eq!(items[2].reason, Some("UNSAFE_ARCHIVE_PATH"));
    assert_eq!(items[3].status, InputStatus::Unsupported);
    assert_eq!(items[4].reason, Some("FIT_HEADER_INVALID"));
    assert!(items[2..].iter().all(|item| item.bytes.is_none()));
}

#[test]
fn duplicate_member_names_preserve_both_original_bytes_in_order() {
    let first = fit();
    let mut second = fit();
    second.push(42);
    let mut zip = archive(&[("a.fit", &first), ("b.fit", &second)]);
    // ZIP writer rejects duplicate names; encode the valid duplicate layout manually.
    for index in 0..zip.len().saturating_sub(4) {
        if &zip[index..index + 5] == b"b.fit" {
            zip[index] = b'a';
        }
    }
    let items = stage(vec![Upload {
        name: "duplicates.zip".into(),
        bytes: zip,
    }])
    .unwrap();
    assert_eq!(items[0].bytes.as_deref(), Some(first.as_slice()));
    assert_eq!(items[1].bytes.as_deref(), Some(second.as_slice()));
    assert_ne!(items[0].name, items[1].name);
}

#[test]
fn local_header_mismatch_fails_only_bad_member() {
    let bytes = fit();
    let mut zip = archive(&[("good.fit", &bytes), ("bad.fit", &bytes)]);
    let local = zip
        .windows(4)
        .enumerate()
        .filter(|(_, bytes)| *bytes == b"PK\x03\x04")
        .nth(1)
        .unwrap()
        .0;
    zip[local + 30] = b'x';
    let items = stage(vec![Upload {
        name: "mixed.zip".into(),
        bytes: zip,
    }])
    .unwrap();
    assert_eq!(items[0].bytes.as_deref(), Some(bytes.as_slice()));
    assert_eq!(items[1].reason, Some("ARCHIVE_MEMBER_INVALID"));
    assert!(items[1].bytes.is_none());
}

#[test]
fn archive_attacks_never_stage_bytes_and_keep_valid_sibling() {
    let bytes = fit();
    let nested = archive(&[("inside.fit", &bytes)]);
    let zip = archive(&[
        ("good.fit", &bytes),
        ("/absolute.fit", &bytes),
        ("C:\\bad.fit", &bytes),
        ("dir\\..\\bad.fit", &bytes),
        ("nested.ZIP", &nested),
        ("hidden.fit", &nested),
    ]);
    let items = stage(vec![Upload {
        name: "attacks.zip".into(),
        bytes: zip,
    }])
    .unwrap();
    assert_eq!(items[0].bytes.as_deref(), Some(bytes.as_slice()));
    for item in &items[1..4] {
        assert_eq!(item.reason, Some("UNSAFE_ARCHIVE_PATH"));
        assert!(!item.name.contains('\\'));
    }
    for item in &items[4..] {
        assert_eq!(item.reason, Some("NESTED_ARCHIVE"));
    }
    assert!(items[1..].iter().all(|item| item.bytes.is_none()));
}

#[test]
fn symlink_attributes_fail_without_following_target() {
    let bytes = fit();
    let mut zip = archive(&[("link.fit", &bytes), ("good.fit", &bytes)]);
    let central = zip
        .windows(4)
        .position(|bytes| bytes == b"PK\x01\x02")
        .unwrap();
    zip[central + 5] = 3; // Unix made-by platform.
    zip[central + 38..central + 42].copy_from_slice(&(0o120777_u32 << 16).to_le_bytes());
    let items = stage(vec![Upload {
        name: "symlink.zip".into(),
        bytes: zip,
    }])
    .unwrap();
    assert_eq!(items[0].reason, Some("ARCHIVE_SYMLINK"));
    assert!(items[0].bytes.is_none());
    assert_eq!(items[1].bytes.as_deref(), Some(bytes.as_slice()));
}

#[test]
fn crc_failure_is_per_member_even_for_unsupported_content() {
    let bytes = fit();
    let mut zip = archive(&[
        ("good.fit", &bytes),
        ("bad.fit", &bytes),
        ("notes.txt", b"some text"),
    ]);
    let locals: Vec<_> = zip
        .windows(4)
        .enumerate()
        .filter(|(_, bytes)| *bytes == b"PK\x03\x04")
        .map(|(index, _)| index)
        .collect();
    for local in &locals[1..] {
        let length = u16::from_le_bytes([zip[*local + 26], zip[*local + 27]]) as usize;
        zip[*local + 30 + length] ^= 1;
    }
    let items = stage(vec![Upload {
        name: "crc.zip".into(),
        bytes: zip,
    }])
    .unwrap();
    assert_eq!(items[0].bytes.as_deref(), Some(bytes.as_slice()));
    assert_eq!(items[1].reason, Some("ARCHIVE_MEMBER_INVALID"));
    assert_eq!(items[2].reason, Some("ARCHIVE_MEMBER_INVALID"));
    assert!(items[1..].iter().all(|item| item.bytes.is_none()));
}

#[test]
fn compressed_bomb_and_forged_expanded_size_are_bounded() {
    let mut payload = fit();
    payload.resize(2 * 1024 * 1024, 0);
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file(
            "bomb.fit",
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
        )
        .unwrap();
    writer.write_all(&payload).unwrap();
    let zip = writer.finish().unwrap().into_inner();
    let items = stage(vec![Upload {
        name: "bomb.zip".into(),
        bytes: zip.clone(),
    }])
    .unwrap();
    assert_eq!(items[0].reason, Some("ARCHIVE_RATIO_EXCEEDED"));
    assert!(items[0].bytes.is_none());
    let mut forged = zip;
    let central = forged
        .windows(4)
        .position(|bytes| bytes == b"PK\x01\x02")
        .unwrap();
    forged[central + 24..central + 28].copy_from_slice(&14_u32.to_le_bytes());
    let items = stage(vec![Upload {
        name: "forged.zip".into(),
        bytes: forged,
    }])
    .unwrap();
    assert_eq!(items[0].reason, Some("ARCHIVE_RATIO_EXCEEDED"));
    assert!(items[0].bytes.is_none());
}

#[test]
fn request_wide_counts_reject_before_staged_result_can_be_committed() {
    let bytes = fit();
    let uploads = (0..11)
        .map(|index| Upload {
            name: format!("{index}.fit"),
            bytes: bytes.clone(),
        })
        .collect();
    assert_eq!(stage(uploads).unwrap_err().code(), "TOO_MANY_FILES");
    let names: Vec<_> = (0..51).map(|index| format!("{index}.fit")).collect();
    let members: Vec<_> = names
        .iter()
        .map(|name| (name.as_str(), bytes.as_slice()))
        .collect();
    assert_eq!(
        stage(vec![Upload {
            name: "many-fit.zip".into(),
            bytes: archive(&members)
        }])
        .unwrap_err()
        .code(),
        "ARCHIVE_LIMIT_EXCEEDED"
    );
    let names: Vec<_> = (0..1001).map(|index| format!("{index}.txt")).collect();
    let members: Vec<_> = names
        .iter()
        .map(|name| (name.as_str(), b"".as_slice()))
        .collect();
    assert_eq!(
        stage(vec![Upload {
            name: "many-members.zip".into(),
            bytes: archive(&members)
        }])
        .unwrap_err()
        .code(),
        "ARCHIVE_LIMIT_EXCEEDED"
    );
}

#[test]
fn member_count_is_actual_and_shared_across_archives() {
    let names: Vec<_> = (0..600).map(|index| format!("{index}.txt")).collect();
    let members: Vec<_> = names
        .iter()
        .map(|name| (name.as_str(), b"".as_slice()))
        .collect();
    let zip = archive(&members);
    assert_eq!(
        stage(vec![
            Upload {
                name: "one.zip".into(),
                bytes: zip.clone()
            },
            Upload {
                name: "two.zip".into(),
                bytes: zip.clone()
            }
        ])
        .unwrap_err()
        .code(),
        "ARCHIVE_LIMIT_EXCEEDED"
    );
    let mut forged = zip;
    let end = forged.len() - 22;
    forged[end + 8..end + 12].copy_from_slice(&[0; 4]);
    let items = stage(vec![Upload {
        name: "forged.zip".into(),
        bytes: forged,
    }])
    .unwrap();
    assert_eq!(items[0].reason, Some("INVALID_ZIP"));
    assert!(items[0].bytes.is_none());
}

#[test]
fn input_byte_bounds_and_filename_validation_never_stage_rejected_bytes() {
    let mut bytes = fit();
    bytes.resize(20 * 1024 * 1024 + 1, 0);
    let items = stage(vec![
        Upload {
            name: "huge.fit".into(),
            bytes,
        },
        Upload {
            name: format!("{}.fit", "x".repeat(252)),
            bytes: fit(),
        },
        Upload {
            name: "bad\n.fit".into(),
            bytes: fit(),
        },
        Upload {
            name: "empty.fit".into(),
            bytes: Vec::new(),
        },
        Upload {
            name: "renamed.bin".into(),
            bytes: fit(),
        },
    ])
    .unwrap();
    assert_eq!(items[0].reason, Some("FILE_TOO_LARGE"));
    assert_eq!(items[1].reason, Some("INVALID_FILE_NAME"));
    assert_eq!(items[2].reason, Some("INVALID_FILE_NAME"));
    assert!(!items[2].name.contains('\n'));
    assert_eq!(items[3].reason, Some("FIT_HEADER_INVALID"));
    assert!(items[..4].iter().all(|item| item.bytes.is_none()));
    assert_eq!(items[4].bytes.as_deref(), Some(fit().as_slice()));
}

#[test]
fn aggregate_expanded_byte_limit_returns_no_partial_staging_result() {
    let mut bytes = fit();
    bytes.resize(17 * 1024 * 1024, 0);
    let uploads = (0..6)
        .map(|index| Upload {
            name: format!("{index}.fit"),
            bytes: bytes.clone(),
        })
        .collect();
    assert_eq!(stage(uploads).unwrap_err().code(), "ARCHIVE_LIMIT_EXCEEDED");
}

#[tokio::test]
async fn multipart_stages_valid_sibling_after_oversized_part_without_partial_bytes() {
    use axum::{
        body::Body,
        extract::{FromRequest, Multipart, Request},
    };
    use garmin_fit_extractor_api::runs::import::parse;

    let mut body = b"--runs-boundary\r\nContent-Disposition: form-data; name=\"files\"; filename=\"too-large.fit\"\r\nContent-Type: application/octet-stream\r\n\r\n".to_vec();
    body.resize(body.len() + 20 * 1024 * 1024 + 1, 0);
    body.extend_from_slice(b"\r\n--runs-boundary\r\nContent-Disposition: form-data; name=\"files\"; filename=\"good.fit\"\r\nContent-Type: application/octet-stream\r\n\r\n");
    body.extend_from_slice(&fit());
    body.extend_from_slice(b"\r\n--runs-boundary--\r\n");
    let mut request = Request::builder()
        .header(
            "content-type",
            "multipart/form-data; boundary=runs-boundary",
        )
        .body(Body::from(body))
        .unwrap();
    axum::extract::DefaultBodyLimit::max(210 * 1024 * 1024).apply(&mut request);
    let multipart = Multipart::from_request(request, &()).await.unwrap();
    let items = parse(multipart).await.unwrap();
    assert_eq!(items[0].reason, Some("FILE_TOO_LARGE"));
    assert!(items[0].bytes.is_none());
    assert_eq!(items[1].bytes.as_deref(), Some(fit().as_slice()));
}

#[tokio::test]
async fn multipart_rejects_unknown_fields_and_eleventh_input() {
    use axum::{
        body::Body,
        extract::{FromRequest, Multipart, Request},
    };
    use garmin_fit_extractor_api::runs::import::parse;

    let body = b"--runs-boundary\r\nContent-Disposition: form-data; name=\"wrong\"; filename=\"run.fit\"\r\n\r\nbytes\r\n--runs-boundary--\r\n";
    let request = Request::builder()
        .header(
            "content-type",
            "multipart/form-data; boundary=runs-boundary",
        )
        .body(Body::from(body.as_slice()))
        .unwrap();
    assert_eq!(
        parse(Multipart::from_request(request, &()).await.unwrap())
            .await
            .unwrap_err()
            .code(),
        "UNKNOWN_FIELD"
    );
    let mut body = Vec::new();
    for index in 0..11 {
        body.extend_from_slice(format!("--runs-boundary\r\nContent-Disposition: form-data; name=\"files\"; filename=\"{index}.fit\"\r\n\r\n").as_bytes());
        body.extend_from_slice(&fit());
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(b"--runs-boundary--\r\n");
    let request = Request::builder()
        .header(
            "content-type",
            "multipart/form-data; boundary=runs-boundary",
        )
        .body(Body::from(body))
        .unwrap();
    assert_eq!(
        parse(Multipart::from_request(request, &()).await.unwrap())
            .await
            .unwrap_err()
            .code(),
        "TOO_MANY_FILES"
    );
}

#[test]
fn forged_small_declared_size_does_not_hide_actual_expanded_bytes() {
    let bytes = fit();
    let mut zip = archive(&[("run.fit", &bytes)]);
    let central = zip
        .windows(4)
        .position(|bytes| bytes == b"PK\x01\x02")
        .unwrap();
    zip[central + 24..central + 28].copy_from_slice(&1_u32.to_le_bytes());
    let items = stage(vec![Upload {
        name: "forged.zip".into(),
        bytes: zip,
    }])
    .unwrap();
    assert_eq!(items[0].reason, Some("ARCHIVE_MEMBER_INVALID"));
    assert!(items[0].bytes.is_none());
}

#[test]
fn actual_member_read_limit_overrides_forged_small_declared_size() {
    let mut bytes = fit();
    let mut seed = 0x12345678_u32;
    for index in bytes.len()..21 * 1024 * 1024 {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        bytes.push(if index % 4 == 0 { 0 } else { seed as u8 });
    }
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file(
            "oversized.fit",
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
        )
        .unwrap();
    writer.write_all(&bytes).unwrap();
    let mut zip = writer.finish().unwrap().into_inner();
    assert!(
        zip.len() < 20 * 1024 * 1024,
        "Synthetic compressed input must fit the upload limit."
    );
    let central = zip
        .windows(4)
        .position(|bytes| bytes == b"PK\x01\x02")
        .unwrap();
    zip[central + 24..central + 28].copy_from_slice(&14_u32.to_le_bytes());
    let items = stage(vec![
        Upload {
            name: "forged.zip".into(),
            bytes: zip,
        },
        Upload {
            name: "good.fit".into(),
            bytes: fit(),
        },
    ])
    .unwrap();
    assert_eq!(items[0].reason, Some("FILE_TOO_LARGE"));
    assert!(items[0].bytes.is_none());
    assert_eq!(items[1].bytes.as_deref(), Some(fit().as_slice()));
}

#[test]
fn exact_fit_and_total_member_limits_are_inclusive() {
    let bytes = fit();
    let names: Vec<_> = (0..50).map(|index| format!("{index}.fit")).collect();
    let members: Vec<_> = names
        .iter()
        .map(|name| (name.as_str(), bytes.as_slice()))
        .collect();
    let items = stage(vec![Upload {
        name: "fifty.zip".into(),
        bytes: archive(&members),
    }])
    .unwrap();
    assert!(
        items
            .iter()
            .all(|item| item.bytes.as_deref() == Some(bytes.as_slice()))
    );
    assert_eq!(items.len(), 50);
    let names: Vec<_> = (0..1000).map(|index| format!("{index}.txt")).collect();
    let members: Vec<_> = names
        .iter()
        .map(|name| (name.as_str(), b"".as_slice()))
        .collect();
    let items = stage(vec![Upload {
        name: "thousand.zip".into(),
        bytes: archive(&members),
    }])
    .unwrap();
    assert_eq!(items.len(), 1000);
    assert!(
        items
            .iter()
            .all(|item| item.status == InputStatus::Unsupported && item.bytes.is_none())
    );
}

#[test]
fn exact_file_and_aggregate_byte_limits_are_inclusive() {
    let mut bytes = fit();
    bytes.resize(20 * 1024 * 1024, 0);
    let uploads = (0..5)
        .map(|index| Upload {
            name: format!("{index}.fit"),
            bytes: bytes.clone(),
        })
        .collect();
    let items = stage(uploads).unwrap();
    assert_eq!(items.len(), 5);
    assert!(
        items
            .iter()
            .all(|item| item.bytes.as_deref() == Some(bytes.as_slice()))
    );
}

#[test]
fn extended_fit_headers_are_admitted_by_content_for_actual_decoder_validation() {
    let extended = include_bytes!("fixtures/runs/garmin_extended_header16.fit");
    let items = stage(vec![
        Upload {
            name: "extended.FIT".into(),
            bytes: extended.to_vec(),
        },
        Upload {
            name: "renamed.bin".into(),
            bytes: extended.to_vec(),
        },
        Upload {
            name: "extended.ZIP".into(),
            bytes: archive(&[("extended.fit", extended)]),
        },
    ])
    .unwrap();
    assert!(items.iter().all(|item| item.status == InputStatus::Ready
        && item.bytes.as_deref() == Some(extended.as_slice())));
}

#[test]
fn malformed_or_incomplete_fit_headers_never_reach_decoder_admission() {
    let mut below_minimum = fit();
    below_minimum[0] = 11;
    let mut short_extended = fit();
    short_extended[0] = 16;
    let mut short_maximum = fit();
    short_maximum[0] = 255;
    let mut wrong_magic = fit();
    wrong_magic[8] = b'!';
    let items = stage(vec![
        Upload {
            name: "minimum.fit".into(),
            bytes: below_minimum,
        },
        Upload {
            name: "truncated.fit".into(),
            bytes: short_extended,
        },
        Upload {
            name: "oversized-header.fit".into(),
            bytes: short_maximum,
        },
        Upload {
            name: "wrong-magic.fit".into(),
            bytes: wrong_magic,
        },
    ])
    .unwrap();
    assert!(items.iter().all(|item| item.status == InputStatus::Failed
        && item.reason == Some("FIT_HEADER_INVALID")
        && item.bytes.is_none()));
}
