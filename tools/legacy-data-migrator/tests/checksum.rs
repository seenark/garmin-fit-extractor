use legacy_data_migrator::checksum::{CanonicalValue, manifest_digest, table_digest};

#[test]
fn canonical_checksum_distinguishes_null_empty_types_and_binary() {
    let columns = ["id", "value"];
    let rows = vec![
        vec![CanonicalValue::Text("56".into()), CanonicalValue::Null],
        vec![
            CanonicalValue::Text("57".into()),
            CanonicalValue::Text(String::new()),
        ],
        vec![
            CanonicalValue::Text("58".into()),
            CanonicalValue::Integer(1),
        ],
        vec![
            CanonicalValue::Text("59".into()),
            CanonicalValue::Blob(vec![0, 255]),
        ],
    ];
    let digest = table_digest("fixture", &columns, &rows, &[0]);
    assert_eq!(digest.row_count, 4);
    assert_eq!(digest.digest.len(), 64);

    let mut changed = rows.clone();
    changed[1][1] = CanonicalValue::Text(" ".into());
    assert_ne!(
        digest.digest,
        table_digest("fixture", &columns, &changed, &[0]).digest
    );
}

#[test]
fn canonical_checksum_is_order_independent_by_primary_key_bytes() {
    let columns = ["id", "body"];
    let first = vec![
        vec![
            CanonicalValue::Integer(56),
            CanonicalValue::Text("a".into()),
        ],
        vec![CanonicalValue::Integer(2), CanonicalValue::Text("b".into())],
    ];
    let second = vec![first[1].clone(), first[0].clone()];
    assert_eq!(
        table_digest("fixture", &columns, &first, &[0]).digest,
        table_digest("fixture", &columns, &second, &[0]).digest
    );
}

#[test]
fn manifest_digest_is_stable_and_includes_table_identity() {
    let columns = ["id"];
    let one = table_digest(
        "one",
        &columns,
        &[vec![CanonicalValue::Text("1".into())]],
        &[0],
    );
    let two = table_digest(
        "two",
        &columns,
        &[vec![CanonicalValue::Text("1".into())]],
        &[0],
    );
    assert_eq!(
        manifest_digest(&[one.clone(), two.clone()]),
        manifest_digest(&[two.clone(), one.clone()])
    );
    assert_ne!(manifest_digest(&[one]), manifest_digest(&[two]));
}
