use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Values are intentionally typed before hashing so null, empty text, numbers, and binary
/// values cannot collapse to the same representation.
#[derive(Clone, Debug)]
pub enum CanonicalValue {
    Null,
    Text(String),
    Integer(i64),
    Real(f64),
    Blob(Vec<u8>),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TableDigest {
    pub table: String,
    pub row_count: u64,
    pub digest: String,
}

pub fn canonical_value_bytes(value: &CanonicalValue) -> Vec<u8> {
    match value {
        CanonicalValue::Null => length_delimited(b'N', &[]),
        CanonicalValue::Text(text) => length_delimited(b'T', text.as_bytes()),
        CanonicalValue::Integer(number) => length_delimited(b'I', &number.to_be_bytes()),
        CanonicalValue::Real(number) => length_delimited(b'R', &number.to_bits().to_be_bytes()),
        CanonicalValue::Blob(bytes) => length_delimited(b'B', bytes),
    }
}

pub fn canonical_row_bytes(row: &[CanonicalValue]) -> Vec<u8> {
    let mut encoded = Vec::new();
    encoded.extend_from_slice(&(row.len() as u64).to_be_bytes());
    for value in row {
        let value_bytes = canonical_value_bytes(value);
        encoded.extend_from_slice(&(value_bytes.len() as u64).to_be_bytes());
        encoded.extend_from_slice(&value_bytes);
    }
    encoded
}

/// Hash a table after sorting rows by the encoded primary-key values rather than a database
/// collation. `primary_key_indices` is normally one element, but supports composite keys.
pub fn table_digest(
    table: &str,
    columns: &[&str],
    rows: &[Vec<CanonicalValue>],
    primary_key_indices: &[usize],
) -> TableDigest {
    let mut ordered: Vec<(&Vec<CanonicalValue>, Vec<u8>)> = rows
        .iter()
        .map(|row| {
            let mut key = Vec::new();
            for index in primary_key_indices {
                if let Some(value) = row.get(*index) {
                    let encoded = canonical_value_bytes(value);
                    key.extend_from_slice(&(encoded.len() as u64).to_be_bytes());
                    key.extend_from_slice(&encoded);
                }
            }
            (row, key)
        })
        .collect();
    ordered.sort_by(|(_, left), (_, right)| left.cmp(right));

    let mut hasher = Sha256::new();
    feed_length_delimited(&mut hasher, b'T', table.as_bytes());
    hasher.update((columns.len() as u64).to_be_bytes());
    for column in columns {
        feed_length_delimited(&mut hasher, b'C', column.as_bytes());
    }
    hasher.update((ordered.len() as u64).to_be_bytes());
    for (row, _) in ordered {
        let encoded = canonical_row_bytes(row);
        hasher.update((encoded.len() as u64).to_be_bytes());
        hasher.update(encoded);
    }

    TableDigest {
        table: table.to_owned(),
        row_count: rows.len() as u64,
        digest: hex_digest(hasher.finalize()),
    }
}

/// Hash a sorted list of table digests. This is the stable source identity recorded in the
/// PostgreSQL import ledger.
pub fn manifest_digest(manifests: &[TableDigest]) -> String {
    let mut ordered = manifests.to_vec();
    ordered.sort_by(|left, right| left.table.cmp(&right.table));
    let mut hasher = Sha256::new();
    hasher.update(b"legacy-data-manifest\0");
    hasher.update((ordered.len() as u64).to_be_bytes());
    for manifest in ordered {
        feed_length_delimited(&mut hasher, b'T', manifest.table.as_bytes());
        hasher.update(manifest.row_count.to_be_bytes());
        feed_length_delimited(&mut hasher, b'D', manifest.digest.as_bytes());
    }
    hex_digest(hasher.finalize())
}

pub fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    let bytes = bytes.as_ref();
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

fn length_delimited(tag: u8, bytes: &[u8]) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(1 + 8 + bytes.len());
    encoded.push(tag);
    encoded.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    encoded.extend_from_slice(bytes);
    encoded
}

fn feed_length_delimited(hasher: &mut Sha256, tag: u8, bytes: &[u8]) {
    hasher.update([tag]);
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}
