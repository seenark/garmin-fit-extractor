//! Bounded JSON stage transport. Only one native record or normalized row is resident.
use crate::{error::ApiError, model::Analysis};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{
    Deserialize,
    de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
    fs::{File, OpenOptions},
    io::{self, BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    rc::Rc,
};

pub const CHUNK_BYTES: usize = 65_536;
pub const PROJECTION_VERSION: &str = "runs-numerical-input-1.0.0";
pub const OBSERVATION_RULE: &str = "exact-stream-v3";
// A FIT record has at most 256 native fields. This bounds JSON expansion of one
// record, not the document, its number of rows, or any streamed unknown array.
const RECORD_BYTES: usize = 2 * 1024 * 1024;
fn invalid() -> ApiError {
    ApiError::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        "INVALID_RUN_DOCUMENT",
        "Run document failed semantic validation.",
    )
}
fn storage() -> ApiError {
    ApiError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "RUNS_STORAGE_FAILED",
        "Run storage operation failed.",
    )
}
fn integrity() -> ApiError {
    ApiError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "RUNS_DOCUMENT_INTEGRITY_FAILED",
        "Stored run document failed integrity verification.",
    )
}

#[derive(Debug)]
pub struct Document {
    pub path: PathBuf,
    pub offset: u64,
    pub byte_length: u64,
    pub sha256: String,
    pub metadata: Value,
}
#[derive(Debug)]
pub struct StagedRun {
    pub decoded: Document,
    pub normalized: Document,
    pub analysis_input: Document,
    pub normalized_metadata: Value,
    pub legacy: Analysis,
}

struct HashFile {
    file: BufWriter<File>,
    hash: Sha256,
    length: u64,
    limit: u64,
}
impl HashFile {
    fn create(path: &Path, limit: u64) -> Result<Self, ApiError> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        Ok(Self {
            file: BufWriter::with_capacity(CHUNK_BYTES, options.open(path).map_err(|_| storage())?),
            hash: Sha256::new(),
            length: 0,
            limit,
        })
    }
    fn finish(mut self, path: PathBuf, mut metadata: Value) -> Result<Document, ApiError> {
        self.file.flush().map_err(|_| storage())?;
        self.file.get_ref().sync_all().map_err(|_| storage())?;
        let sha256 = format!("{:x}", self.hash.finalize());
        metadata["byteLength"] = json!(self.length);
        metadata["sha256"] = json!(sha256);
        metadata["chunkCount"] = json!(self.length.div_ceil(CHUNK_BYTES as u64));
        Ok(Document {
            path,
            offset: 0,
            byte_length: self.length,
            sha256,
            metadata,
        })
    }
}
impl Write for HashFile {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() as u64 > self.limit.saturating_sub(self.length) {
            return Err(io::Error::other("RUNS_SPOOL_LIMIT"));
        }
        let n = self.file.write(bytes)?;
        self.hash.update(&bytes[..n]);
        self.length += n as u64;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

// serde's IoRead consumes one byte at a time. Resetting this shared allowance
// before an atomic value bounds its allocation before Value can grow.
struct BudgetRead<R> {
    inner: R,
    allowance: Rc<Cell<usize>>,
}
impl<R: Read> Read for BudgetRead<R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let remaining = self.allowance.get();
        if remaining == 0 {
            return Err(io::Error::other("RUNS_RECORD_LIMIT"));
        }
        let size = bytes.len().min(remaining);
        let n = self.inner.read(&mut bytes[..size])?;
        self.allowance.set(remaining - n);
        Ok(n)
    }
}

type Project<'a> = dyn FnMut(&[String], Value) -> Result<Option<Value>, ApiError> + 'a;
type Keep<'a> = dyn FnMut(&[String]) -> bool + 'a;
fn known_array(path: &[String]) -> bool {
    matches!(path, [key] if matches!(key.as_str(), "messages"|"warnings"|"samples"|"laps"|"timerEvents"|"sensors"|"zones"|"deviceReportedThresholds"|"extensions"))
        || matches!(path, [root,key] if root == "rr" && matches!(key.as_str(),"intervals"|"reasons"))
}
fn atomic(path: &[String]) -> bool {
    if path.last().is_some_and(|p| p == "*") {
        return known_array(&path[..path.len() - 1]);
    }
    matches!(path, [key] if matches!(key.as_str(), "schemaVersion"|"decoder"|"session"|"startTime"|"endTime"|"sport"|"subtype"|"summary"|"sourceRevision"|"decodedRevisionId"|"projectionVersion"|"normalizedDocumentHash"))
        || matches!(path, [root,key] if root == "rr" && key == "alignmentEligible")
}
struct Walk<'a> {
    out: &'a mut dyn Write,
    emit: bool,
    keep: &'a mut Keep<'a>,
    project: &'a mut Project<'a>,
    callback_failure: &'a mut Option<ApiError>,
    allowance: Rc<Cell<usize>>,
    counts: &'a mut BTreeMap<String, u64>,
}
struct Seed<'a, 'b> {
    walk: &'a mut Walk<'b>,
    path: Vec<String>,
    separator: &'static [u8],
}
impl<'de> DeserializeSeed<'de> for Seed<'_, '_> {
    type Value = bool;
    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<bool, D::Error> {
        self.walk.allowance.set(RECORD_BYTES);
        if atomic(&self.path) {
            let value = Value::deserialize(deserializer)?;
            let value = (self.walk.project)(&self.path, value).map_err(|error| {
                *self.walk.callback_failure = Some(error);
                de::Error::custom("RUNS_CALLBACK_FAILED")
            })?;
            if let Some(value) = value {
                self.walk
                    .out
                    .write_all(self.separator)
                    .map_err(de::Error::custom)?;
                if self.walk.emit {
                    serde_json::to_writer(&mut self.walk.out, &value).map_err(de::Error::custom)?;
                }
                return Ok(true);
            }
            return Ok(false);
        }
        deserializer.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Seed<'_, '_> {
    type Value = bool;
    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a streamed JSON value")
    }
    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<bool, M::Error> {
        if known_array(&self.path) {
            return Err(de::Error::custom("INVALID_RUN_DOCUMENT"));
        }
        self.walk
            .out
            .write_all(self.separator)
            .map_err(de::Error::custom)?;
        self.walk.out.write_all(b"{").map_err(de::Error::custom)?;
        let mut first = true;
        let mut seen = BTreeSet::new();
        loop {
            self.walk.allowance.set(RECORD_BYTES);
            let Some(key) = map.next_key::<String>()? else {
                break;
            };
            if !seen.insert(key.clone()) {
                return Err(de::Error::custom("INVALID_RUN_DOCUMENT"));
            }
            let mut path = self.path.clone();
            path.push(key.clone());
            if !(self.walk.keep)(&path) {
                map.next_value_seed(Discard {
                    allowance: self.walk.allowance.clone(),
                })?;
                continue;
            }
            // Object fields cannot disappear after their name has been emitted;
            // use keep for field omission, project(None) only for array items.
            if !first {
                self.walk.out.write_all(b",").map_err(de::Error::custom)?;
            }
            first = false;
            if self.walk.emit {
                serde_json::to_writer(&mut self.walk.out, &key).map_err(de::Error::custom)?;
            }
            self.walk.out.write_all(b":").map_err(de::Error::custom)?;
            let emitted = map.next_value_seed(Seed {
                walk: self.walk,
                path,
                separator: b"",
            })?;
            if !emitted {
                return Err(de::Error::custom("INVALID_RUN_DOCUMENT"));
            }
        }
        self.walk.out.write_all(b"}").map_err(de::Error::custom)?;
        Ok(true)
    }
    fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<bool, S::Error> {
        self.walk
            .out
            .write_all(self.separator)
            .map_err(de::Error::custom)?;
        self.walk.out.write_all(b"[").map_err(de::Error::custom)?;
        let mut path = self.path.clone();
        path.push("*".into());
        let mut first = true;
        let mut count = 0u64;
        while let Some(emitted) = seq.next_element_seed(Seed {
            walk: self.walk,
            path: path.clone(),
            separator: if first { b"" } else { b"," },
        })? {
            count += 1;
            if emitted {
                first = false;
            }
        }
        if known_array(&self.path) {
            self.walk.counts.insert(self.path.join("."), count);
        }
        self.walk.out.write_all(b"]").map_err(de::Error::custom)?;
        Ok(true)
    }
    fn visit_bool<E: de::Error>(self, value: bool) -> Result<bool, E> {
        self.scalar(json!(value))
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<bool, E> {
        self.scalar(json!(value))
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<bool, E> {
        self.scalar(json!(value))
    }
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<bool, E> {
        if !value.is_finite() {
            return Err(E::custom("INVALID_RUN_DOCUMENT"));
        }
        self.scalar(json!(value))
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<bool, E> {
        self.scalar(json!(value))
    }
    fn visit_string<E: de::Error>(self, value: String) -> Result<bool, E> {
        self.scalar(Value::String(value))
    }
    fn visit_unit<E: de::Error>(self) -> Result<bool, E> {
        self.scalar(Value::Null)
    }
}
impl Seed<'_, '_> {
    fn scalar<E: de::Error>(self, value: Value) -> Result<bool, E> {
        if known_array(&self.path) {
            return Err(E::custom("INVALID_RUN_DOCUMENT"));
        }
        let value = (self.walk.project)(&self.path, value).map_err(|error| {
            *self.walk.callback_failure = Some(error);
            E::custom("RUNS_CALLBACK_FAILED")
        })?;
        if let Some(value) = value {
            self.walk.out.write_all(self.separator).map_err(E::custom)?;
            if self.walk.emit {
                serde_json::to_writer(&mut self.walk.out, &value).map_err(E::custom)?;
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
// Skipping uses the same incremental visitor: IgnoredAny can allocate a giant
// escaped string, and one byte allowance for an entire array would cap rows.
struct Discard {
    allowance: Rc<Cell<usize>>,
}
impl<'de> DeserializeSeed<'de> for Discard {
    type Value = ();
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        self.allowance.set(RECORD_BYTES);
        d.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Discard {
    type Value = ();
    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("JSON")
    }
    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<(), M::Error> {
        loop {
            self.allowance.set(RECORD_BYTES);
            if map.next_key::<String>()?.is_none() {
                break;
            }
            map.next_value_seed(Discard {
                allowance: self.allowance.clone(),
            })?;
        }
        Ok(())
    }
    fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<(), S::Error> {
        while seq
            .next_element_seed(Discard {
                allowance: self.allowance.clone(),
            })?
            .is_some()
        {}
        Ok(())
    }
    fn visit_bool<E: de::Error>(self, _: bool) -> Result<(), E> {
        Ok(())
    }
    fn visit_i64<E: de::Error>(self, _: i64) -> Result<(), E> {
        Ok(())
    }
    fn visit_u64<E: de::Error>(self, _: u64) -> Result<(), E> {
        Ok(())
    }
    fn visit_f64<E: de::Error>(self, _: f64) -> Result<(), E> {
        Ok(())
    }
    fn visit_str<E: de::Error>(self, _: &str) -> Result<(), E> {
        Ok(())
    }
    fn visit_string<E: de::Error>(self, _: String) -> Result<(), E> {
        Ok(())
    }
    fn visit_unit<E: de::Error>(self) -> Result<(), E> {
        Ok(())
    }
}

// serde converts visitor write failures into deserializer errors. Remember
// their origin independently so resource exhaustion never blames source JSON.
struct OutputWrite<'a> {
    inner: &'a mut dyn Write,
    failed: bool,
}
impl Write for OutputWrite<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let result = self.inner.write(bytes);
        self.failed |= result
            .as_ref()
            .err()
            .is_some_and(|error| error.kind() != io::ErrorKind::Interrupted)
            || matches!(result,Ok(0) if !bytes.is_empty());
        result
    }
    fn flush(&mut self) -> io::Result<()> {
        let result = self.inner.flush();
        self.failed |= result.is_err();
        result
    }
}

// Capture errors below the record budget so actual storage failures do not
// become semantic JSON errors, while budget exhaustion remains input failure.
struct InputRead<'a, R> {
    inner: R,
    failure: &'a mut Option<io::ErrorKind>,
}
impl<R: Read> Read for InputRead<'_, R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let result = self.inner.read(bytes);
        if let Err(error) = &result
            && error.kind() != io::ErrorKind::Interrupted
        {
            *self.failure = Some(error.kind());
        }
        result
    }
}

/// Compact JSON output; export may wrap an indenting writer. Field omission is
/// decided before parsing. Atomic known-array items may be removed with None.
pub fn project_document<R: Read, W: Write>(
    reader: R,
    writer: &mut W,
    keep: &mut Keep<'_>,
    project: &mut Project<'_>,
) -> Result<(), ApiError> {
    let mut counts = BTreeMap::new();
    walk_document(reader, writer, keep, project, &mut counts, true)
}
fn walk_document<R: Read>(
    reader: R,
    writer: &mut dyn Write,
    keep: &mut Keep<'_>,
    project: &mut Project<'_>,
    counts: &mut BTreeMap<String, u64>,
    emit: bool,
) -> Result<(), ApiError> {
    let allowance = Rc::new(Cell::new(RECORD_BYTES));
    let mut input_failure = None;
    let mut callback_failure = None;
    let input = BudgetRead {
        inner: BufReader::with_capacity(
            CHUNK_BYTES,
            InputRead {
                inner: reader,
                failure: &mut input_failure,
            },
        ),
        allowance: allowance.clone(),
    };
    let mut deserializer = serde_json::Deserializer::from_reader(input);
    let mut output = OutputWrite {
        inner: writer,
        failed: false,
    };
    let mut walk = Walk {
        out: &mut output,
        emit,
        keep,
        project,
        callback_failure: &mut callback_failure,
        allowance,
        counts,
    };
    let result = Seed {
        walk: &mut walk,
        path: Vec::new(),
        separator: b"",
    }
    .deserialize(&mut deserializer)
    .and_then(|_| deserializer.end());
    drop(deserializer);
    // A callback may write to its own storage, outside OutputWrite. Move its
    // original typed error back across serde without exposing error contents.
    if let Some(error) = callback_failure {
        return Err(error);
    }
    result.map_err(|_| {
        if output.failed {
            storage()
        } else {
            match input_failure {
                Some(io::ErrorKind::InvalidData) => integrity(),
                Some(_) => storage(),
                None => invalid(),
            }
        }
    })
}

pub(super) fn visit_document<R: Read>(
    reader: R,
    project: &mut Project<'_>,
) -> Result<(), ApiError> {
    walk_document(
        reader,
        &mut io::sink(),
        &mut |_| true,
        project,
        &mut BTreeMap::new(),
        false,
    )
}

struct RrField {
    number: u8,
    offset: Option<u64>,
    component: Option<u64>,
    values: Box<[Option<f64>]>,
}
#[derive(Default)]
struct Native {
    global: u16,
    fields: [u64; 4],
    valid: [u64; 4],
    developers: Vec<(u8, usize, bool)>,
    timestamp: Option<DateTime<Utc>>,
    start_time: Option<DateTime<Utc>>,
    fractional_timestamp: Option<f64>,
    rr_fields: Option<Vec<RrField>>,
}
impl Native {
    fn bit(bitmap: &[u64; 4], number: u8) -> bool {
        bitmap[number as usize / 64] & (1u64 << (number % 64)) != 0
    }
    fn rr_value(&self, reference: &Value) -> Result<Option<f64>, ApiError> {
        let fields = self.rr_fields.as_ref().ok_or_else(invalid)?;
        let number = reference["fieldNumber"].as_u64().ok_or_else(invalid)?;
        let mut candidates = fields.iter().filter(|field| {
            field.number as u64 == number
                && reference["byteOffset"]
                    .as_u64()
                    .is_none_or(|offset| field.offset == Some(offset))
                && reference["componentParent"]
                    .as_u64()
                    .is_none_or(|parent| field.component == Some(parent))
        });
        let field = candidates.next().ok_or_else(invalid)?;
        if candidates.next().is_some() {
            return Err(invalid());
        }
        let index = match reference.get("arrayIndex") {
            Some(value) => value
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .ok_or_else(invalid)?,
            None => field.values.len().checked_sub(1).ok_or_else(invalid)?,
        };
        field.values.get(index).copied().ok_or_else(invalid)
    }
}
#[derive(Default)]
struct Lineage {
    messages: Vec<Native>,
    developer_values: Vec<Value>,
    developer_index: BTreeMap<[u8; 32], usize>,
}
impl Lineage {
    fn intern_developer(&mut self, value: &Value) -> Result<usize, ApiError> {
        let mut hash = Sha256::new();
        hash_value(&mut hash, value)?;
        let digest: [u8; 32] = hash.finalize().into();
        if let Some(index) = self.developer_index.get(&digest) {
            if self.developer_values[*index] != *value {
                return Err(invalid());
            }
            return Ok(*index);
        }
        let index = self.developer_values.len();
        self.developer_values.push(value.clone());
        self.developer_index.insert(digest, index);
        Ok(index)
    }
    fn message(&mut self, value: &Value) -> Result<(), ApiError> {
        if value["index"].as_u64() != Some(self.messages.len() as u64)
            || value["localMessageNumber"].as_u64().is_none_or(|n| n > 15)
        {
            return Err(invalid());
        }
        let global = value["globalMessageNumber"]
            .as_u64()
            .filter(|n| *n <= u16::MAX as u64)
            .ok_or_else(invalid)? as u16;
        let mut native = Native {
            global,
            ..Native::default()
        };
        let fields = value["fields"].as_array().ok_or_else(invalid)?;
        if fields.len() > 512 {
            return Err(invalid());
        }
        for field in fields {
            let number = field["fieldNumber"]
                .as_u64()
                .filter(|n| *n <= 255)
                .ok_or_else(invalid)? as u8;
            let valid = match field["validity"].as_str() {
                Some("valid" | "mixed") => !field["value"].is_null(),
                Some("invalid") => false,
                _ => return Err(invalid()),
            };
            let identity = field.get("developerIdentity").unwrap_or(&Value::Null);
            if !identity.is_null() {
                if identity["developerDataIndex"]
                    .as_u64()
                    .is_none_or(|n| n > 255)
                    || identity["fieldDefinitionNumber"]
                        .as_u64()
                        .is_none_or(|n| n > 255)
                {
                    return Err(invalid());
                }
                let identity = self.intern_developer(identity)?;
                native.developers.push((number, identity, valid));
            } else {
                let reconstructed =
                    global == 20 && number == 253 && field["role"] == "reconstructed";
                let expected_unit = match (global, number) {
                    (20, 6 | 73) => Some("m/s"),
                    (20, 3) => Some("bpm"),
                    (20, 7) => Some("watts"),
                    (20, 4 | 53) => Some("rpm"),
                    (20, 2 | 78 | 5) => Some("m"),
                    (20, 253) if !reconstructed => Some("s"),
                    (78, 0) | (132, 0 | 9) | (18, 253) => Some("s"),
                    _ => None,
                };
                if let Some(unit) = expected_unit
                    && field["unit"] != unit
                {
                    return Err(invalid());
                }
                if (reconstructed || matches!((global, number), (132, 253) | (18, 2)))
                    && !field["unit"].is_null()
                {
                    return Err(invalid());
                }
                native.fields[number as usize / 64] |= 1u64 << (number % 64);
                if valid {
                    native.valid[number as usize / 64] |= 1u64 << (number % 64);
                }
                if matches!(global, 18 | 20 | 132) && number == 253 && valid {
                    native.timestamp = field["value"]
                        .as_str()
                        .map(|_| timestamp(&field["value"]))
                        .transpose()?;
                }
                if global == 18 && number == 2 && valid {
                    native.start_time = Some(timestamp(&field["value"])?);
                }
                if global == 132 && number == 0 && valid {
                    native.fractional_timestamp = field["value"].as_f64();
                }
                if matches!((global, number), (78, 0) | (132, 9)) {
                    let values = match &field["value"] {
                        Value::Array(values) => {
                            values.iter().map(Value::as_f64).collect::<Vec<_>>()
                        }
                        value => vec![value.as_f64()],
                    };
                    if values.iter().flatten().any(|n| !n.is_finite() || *n < 0.0) {
                        return Err(invalid());
                    }
                    let reference = &field["sourceReference"];
                    native
                        .rr_fields
                        .get_or_insert_with(|| Vec::with_capacity(1))
                        .push(RrField {
                            number,
                            offset: reference["byteOffset"].as_u64(),
                            component: reference["componentParent"].as_u64(),
                            values: values.into_boxed_slice(),
                        });
                }
            }
            validate_native_reference(
                &field["sourceReference"],
                self.messages.len(),
                global,
                Some(number),
                identity,
            )?;
        }
        native
            .developers
            .sort_unstable_by_key(|(number, identity, _)| (*number, *identity));
        native.developers.dedup_by(|right, left| {
            if right.0 == left.0 && right.1 == left.1 {
                left.2 |= right.2;
                true
            } else {
                false
            }
        });
        validate_native_reference(
            &value["sourceReference"],
            self.messages.len(),
            global,
            None,
            &Value::Null,
        )?;
        self.messages.push(native);
        Ok(())
    }
    fn reference(&self, value: &Value, require_valid: bool) -> Result<&Native, ApiError> {
        let index = value["messageIndex"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or_else(invalid)?;
        let message = self.messages.get(index).ok_or_else(invalid)?;
        if value["globalMessageNumber"].as_u64() != Some(message.global as u64) {
            return Err(invalid());
        }
        if value.get("fieldNumber").is_none() {
            if require_valid {
                return Err(invalid());
            }
            return Ok(message);
        }
        let number = value["fieldNumber"]
            .as_u64()
            .filter(|n| *n <= 255)
            .ok_or_else(invalid)? as u8;
        let identity = value.get("developerIdentity").unwrap_or(&Value::Null);
        let exists = if identity.is_null() {
            Native::bit(
                if require_valid {
                    &message.valid
                } else {
                    &message.fields
                },
                number,
            )
        } else {
            let mut hash = Sha256::new();
            hash_value(&mut hash, identity)?;
            let digest: [u8; 32] = hash.finalize().into();
            self.developer_index.get(&digest).is_some_and(|id| {
                self.developer_values[*id] == *identity
                    && message
                        .developers
                        .binary_search_by_key(&(number, *id), |(number, id, _)| (*number, *id))
                        .ok()
                        .is_some_and(|index| !require_valid || message.developers[index].2)
            })
        };
        if !exists {
            return Err(invalid());
        }
        if matches!((message.global, number), (78, 0) | (132, 9))
            && value.get("arrayIndex").is_some()
        {
            if require_valid && message.rr_value(value)?.is_none() {
                return Err(invalid());
            }
            message.rr_value(value)?;
        }
        Ok(message)
    }
    fn references(&self, value: &Value) -> Result<(), ApiError> {
        match value {
            Value::Object(object) => {
                if object.contains_key("messageIndex") && object.contains_key("globalMessageNumber")
                {
                    self.reference(value, false)?;
                }
                for child in object.values() {
                    self.references(child)?;
                }
            }
            Value::Array(values) => {
                for child in values {
                    self.references(child)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

// Export approval is separate from immutable archive/import validation. This
// index keeps numeric coordinates and numeric/time facts, never archived fields.
#[derive(Default)]
pub struct SourceProof {
    messages: Vec<SourceMessage>,
    decoder_verified: bool,
    session: Option<usize>,
    session_count: usize,
    rr_previous: Option<f64>,
    rr_clock_safe: bool,
    unsafe_record_times: [BTreeSet<u64>; 8],
    unlocated_unsafe: u16,
    observed_samples: bool,
    observed_unsafe_times: [BTreeSet<u64>; 8],
    observed_unlocated: u16,
    observed_records: BTreeMap<u64, u16>,
    timer_states: BTreeMap<u64, bool>,
    unsafe_timer_times: BTreeSet<u64>,
    unlocated_timer: bool,
}
struct SourceMessage {
    global: u16,
    frame: (u64, u64),
    frame_valid: bool,
    facts: Option<Box<SourceFacts>>,
}
struct SourceFacts {
    global: u16,
    fields: Box<[SourceField]>,
    clock: Option<f64>,
    unsafe_metrics: u16,
    unsafe_fields: [u64; 4],
    rr: Option<Box<Native>>,
    timer: bool,
    timer_safe: bool,
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SourceCoordinates {
    number: u8,
    parent: Option<u8>,
    offset: u64,
    length: u64,
}
impl SourceCoordinates {
    fn read(value: &Value) -> Option<Self> {
        if !value["developerIdentity"].is_null() {
            return None;
        }
        let parent = match value.get("componentParent") {
            None | Some(Value::Null) => None,
            Some(value) => Some(u8::try_from(value.as_u64()?).ok()?),
        };
        Some(Self {
            number: u8::try_from(value["fieldNumber"].as_u64()?).ok()?,
            parent,
            offset: value["byteOffset"].as_u64()?,
            length: value["byteLength"].as_u64()?,
        })
    }
}
struct SourceField {
    coordinates: SourceCoordinates,
    value: SourceNumber,
    raw: Option<SourceNumber>,
    wire: Option<u16>,
    role: u8,
    previous: Option<f64>,
    clock_safe: bool,
}
enum SourceNumber {
    Scalar(Option<f64>),
    Array(Box<[Option<f64>]>),
    Date(DateTime<Utc>),
    Enum(fitparser::profile::FieldDataType, i64),
    Rr(usize, bool),
}
impl SourceProof {
    pub fn decoder(&mut self, value: &Value) -> Result<(), ApiError> {
        use crate::fit::runs;
        self.decoder_verified = value["profileVersion"] == runs::PROFILE_VERSION
            && value["profileSourceSha256"] == runs::PROFILE_SOURCE_SHA256
            && value["vendorSourceSha256"] == runs::VENDOR_SOURCE_SHA256;
        self.verify_decoder()
    }
    pub fn verify_decoder(&self) -> Result<(), ApiError> {
        if self.decoder_verified {
            Ok(())
        } else {
            Err(ApiError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "RUNS_EXPORT_SOURCE_UNPROVABLE",
                "Stored run source cannot prove a required export value.",
            ))
        }
    }
    pub fn message(&mut self, value: &Value) -> Result<(), ApiError> {
        let index = self.messages.len();
        if value["index"].as_u64() != Some(index as u64)
            || value["localMessageNumber"].as_u64().is_none_or(|n| n > 15)
        {
            return Err(invalid());
        }
        let global = value["globalMessageNumber"]
            .as_u64()
            .and_then(|n| u16::try_from(n).ok())
            .ok_or_else(invalid)?;
        let fields = value["fields"].as_array().ok_or_else(invalid)?;
        if fields.len() > 512 {
            return Err(invalid());
        }
        validate_native_reference(&value["sourceReference"], index, global, None, &Value::Null)?;
        let mut guarded = [false; 512];
        for (position, field) in fields.iter().enumerate() {
            guarded[position] =
                crate::fit::raw::native_field_valid(global as u64, field, Some(fields));
        }
        let mut coordinates_count = BTreeMap::<SourceCoordinates, usize>::new();
        for (_, field) in fields
            .iter()
            .enumerate()
            .filter(|(position, _)| guarded[*position])
        {
            if let Some(coordinates) = SourceCoordinates::read(&field["sourceReference"]) {
                *coordinates_count.entry(coordinates).or_default() += 1;
            }
        }
        for field in fields
            .iter()
            .enumerate()
            .filter(|(position, _)| !guarded[*position])
            .map(|(_, field)| field)
        {
            if let Some(count) = SourceCoordinates::read(&field["sourceReference"])
                .and_then(|coordinates| coordinates_count.get_mut(&coordinates))
            {
                *count += 1;
            }
        }
        let mut source = SourceFacts {
            global,
            fields: Box::default(),
            clock: None,
            unsafe_metrics: 0,
            unsafe_fields: [0; 4],
            rr: None,
            timer: false,
            timer_safe: true,
        };
        let mut approved = Vec::new();
        for (position, field) in fields.iter().enumerate() {
            let number = field["fieldNumber"]
                .as_u64()
                .and_then(|n| u8::try_from(n).ok())
                .ok_or_else(invalid)?;
            if !matches!(
                field["validity"].as_str(),
                Some("valid" | "mixed" | "invalid")
            ) {
                return Err(invalid());
            }
            validate_native_reference(
                &field["sourceReference"],
                index,
                global,
                Some(number),
                field.get("developerIdentity").unwrap_or(&Value::Null),
            )?;
            let coordinates = SourceCoordinates::read(&field["sourceReference"]);
            let physical = coordinates.is_some_and(|coordinates| {
                if field["sourceReference"].get("arrayIndex").is_some() {
                    return false;
                }
                if let Some(parent) = coordinates.parent {
                    if field["componentParent"]
                        .as_u64()
                        .is_some_and(|n| n != parent as u64)
                    {
                        return false;
                    }
                    let mut parents = fields.iter().enumerate().filter(|(_, candidate)| {
                        candidate["role"] != "expanded"
                            && candidate["role"] != "reconstructed"
                            && candidate["sourceReference"].get("arrayIndex").is_none()
                            && SourceCoordinates::read(&candidate["sourceReference"])
                                == Some(SourceCoordinates {
                                    number: parent,
                                    parent: None,
                                    ..coordinates
                                })
                    });
                    let Some((parent_position, _)) = parents.next() else {
                        return false;
                    };
                    guarded[parent_position] && parents.next().is_none()
                } else {
                    field["role"] != "expanded"
                }
            });
            let typed_null = field["value"].is_null()
                || field["value"]
                    .as_array()
                    .is_some_and(|items| items.iter().all(Value::is_null));
            let safe = guarded[position]
                && physical
                && (field["validity"] != "invalid" || typed_null)
                && coordinates
                    .is_some_and(|coordinates| coordinates_count.get(&coordinates) == Some(&1));
            if !safe && field["developerIdentity"].is_null() && field["role"] != "expanded" {
                source.unsafe_fields[number as usize / 64] |= 1u64 << (number % 64);
            }
            if global == 20 && field["developerIdentity"].is_null() {
                if number == 253 && source.clock.is_none() {
                    source.clock = source_seconds(&field["value"]);
                }
                if !safe && !typed_null {
                    source.unsafe_metrics |= source_metric(number);
                }
            }
            if global == 21 && field["developerIdentity"].is_null() {
                if number == 253 && source.clock.is_none() {
                    source.clock = source_seconds(&field["value"]);
                }
                if number == 0 && !typed_null {
                    let code = field["enumCode"]
                        .as_u64()
                        .or_else(|| field["rawValue"].as_u64())
                        .or_else(|| field["value"].as_u64());
                    source.timer |= !safe || code == Some(0);
                }
                if matches!(number, 0 | 1 | 253) && (!safe || typed_null) {
                    source.timer_safe = false;
                }
            }
            if safe {
                let coordinates = coordinates.ok_or_else(invalid)?;
                let value = if matches!((global, number), (78, 0) | (132, 9)) {
                    let values = match &field["value"] {
                        Value::Array(items) => items.iter().map(Value::as_f64).collect::<Vec<_>>(),
                        value => vec![value.as_f64()],
                    };
                    let rr = source.rr.get_or_insert_with(|| {
                        Box::new(Native {
                            global,
                            ..Native::default()
                        })
                    });
                    let rr_fields = rr.rr_fields.get_or_insert_with(|| Vec::with_capacity(1));
                    let position = rr_fields.len();
                    rr_fields.push(RrField {
                        number,
                        offset: Some(coordinates.offset),
                        component: coordinates.parent.map(u64::from),
                        values: values.into_boxed_slice(),
                    });
                    SourceNumber::Rr(position, field["value"].is_array())
                } else if let Some(items) = field["value"].as_array() {
                    SourceNumber::Array(items.iter().map(source_numeric).collect())
                } else if let Ok(time) = timestamp(&field["value"]) {
                    SourceNumber::Date(time)
                } else if let Some((kind, code)) = source_enum(global, number, field) {
                    SourceNumber::Enum(kind, code)
                } else {
                    SourceNumber::Scalar(source_numeric(&field["value"]))
                };
                if matches!((global, number), (132, 0 | 253)) {
                    source.rr.get_or_insert_with(|| {
                        Box::new(Native {
                            global,
                            ..Native::default()
                        })
                    });
                }
                if let Some(rr) = source.rr.as_mut() {
                    rr.fields[number as usize / 64] |= 1u64 << (number % 64);
                    if !typed_null {
                        rr.valid[number as usize / 64] |= 1u64 << (number % 64);
                    }
                    if number == 253 {
                        rr.timestamp = timestamp(&field["value"]).ok();
                    }
                    if number == 0 {
                        rr.fractional_timestamp = field["value"].as_f64();
                    }
                }
                let raw = (field["rawValue"] != field["value"]).then(|| match &field["rawValue"] {
                    Value::Array(values) => {
                        SourceNumber::Array(values.iter().map(source_numeric).collect())
                    }
                    value => SourceNumber::Scalar(source_numeric(value)),
                });
                let wire = field
                    .get("baseType")
                    .filter(|value| !value.is_null())
                    .or_else(|| field.get("type"))
                    .and_then(source_wire);
                let role = match field["role"].as_str() {
                    Some("expanded") => 1,
                    Some("reconstructed") => 2,
                    _ => 0,
                };
                approved.push(SourceField {
                    coordinates,
                    value,
                    raw,
                    wire,
                    role,
                    previous: None,
                    clock_safe: false,
                });
            }
        }
        // A new genuine counter anchor ends any earlier clock ambiguity. Other
        // source clocks contribute trust only; no SDK timing is recomputed.
        if fields.iter().enumerate().any(|(position, field)| {
            field["fieldNumber"] == 253
                && field["developerIdentity"].is_null()
                && !field["value"].is_null()
                && !guarded[position]
        }) {
            self.rr_clock_safe = false;
        }
        if global == 132 {
            let anchor = fields
                .iter()
                .position(|field| {
                    field["fieldNumber"] == 9
                        && field["developerIdentity"].is_null()
                        && field["role"] != "expanded"
                        && field["validity"] != "invalid"
                        && (field["value"].as_f64().is_some()
                            || field["value"].as_array().is_some_and(|values| {
                                values.len() == 1 && values[0].as_f64().is_some()
                            }))
                })
                .filter(|_| {
                    fields.iter().any(|field| {
                        field["fieldNumber"] == 253
                            && field["developerIdentity"].is_null()
                            && field["role"] != "expanded"
                            && field["validity"] != "invalid"
                            && source_seconds(&field["value"]).is_some()
                    }) && fields.iter().any(|field| {
                        field["fieldNumber"] == 0
                            && field["developerIdentity"].is_null()
                            && field["role"] != "expanded"
                            && field["validity"] != "invalid"
                            && field["value"].as_f64().is_some()
                    })
                });
            let order = anchor
                .into_iter()
                .chain((0..fields.len()).filter(|position| Some(*position) != anchor));
            for position in order {
                let field = &fields[position];
                if field["fieldNumber"] != 9
                    || !field["developerIdentity"].is_null()
                    || !(field["role"] != "expanded" || field["componentParent"] == 10)
                {
                    continue;
                }
                let coordinates = SourceCoordinates::read(&field["sourceReference"]);
                let candidate = approved
                    .iter_mut()
                    .find(|candidate| Some(candidate.coordinates) == coordinates);
                if let Some(candidate) = candidate {
                    candidate.previous = self.rr_previous;
                    if Some(position) == anchor {
                        self.rr_clock_safe = !Native::bit(&source.unsafe_fields, 253)
                            && !Native::bit(&source.unsafe_fields, 0);
                    }
                    candidate.clock_safe = self.rr_clock_safe;
                    if let SourceNumber::Rr(index, _) = candidate.value {
                        self.rr_previous = source
                            .rr
                            .as_ref()
                            .and_then(|rr| rr.rr_fields.as_ref())
                            .and_then(|values| values.get(index))
                            .and_then(|field| field.values.last())
                            .copied()
                            .flatten();
                    }
                } else {
                    self.rr_previous = None;
                }
            }
        }
        if source.timer {
            source.timer_safe &= [0, 1, 253].into_iter().all(|number| {
                approved.iter().any(|field| {
                    field.coordinates.number == number && field.coordinates.parent.is_none()
                })
            }) && source.clock.is_some();
        }
        if global == 20 && source.unsafe_metrics != 0 {
            source_bad_record(
                source.clock,
                source.unsafe_metrics,
                &mut self.unsafe_record_times,
                &mut self.unlocated_unsafe,
            );
        }
        if source.timer {
            if let Some(time) = source.clock {
                let time = source_time_key(time);
                self.timer_states
                    .entry(time)
                    .and_modify(|safe| *safe &= source.timer_safe)
                    .or_insert(source.timer_safe);
                if !source.timer_safe {
                    self.unsafe_timer_times.insert(time);
                }
            } else {
                self.unlocated_timer = true;
            }
        }
        approved.sort_unstable_by_key(|field| field.coordinates);
        source.fields = approved.into_boxed_slice();
        if global == 18 {
            self.session_count += 1;
            if self.session.is_none() {
                self.session = Some(index);
            }
        }
        let frame = value["sourceReference"]["byteOffset"]
            .as_u64()
            .zip(value["sourceReference"]["byteLength"].as_u64());
        let keep = !source.fields.is_empty() || (global == 20 && source.clock.is_some());
        self.messages.push(SourceMessage {
            global,
            frame: frame.unwrap_or_default(),
            frame_valid: frame.is_some(),
            facts: keep.then(|| Box::new(source)),
        });
        Ok(())
    }
    pub fn observe_sample(&mut self, value: &Value) -> Result<(), ApiError> {
        let sources = value["sourceReferences"].as_object().ok_or_else(invalid)?;
        self.observed_samples = true;
        let timestamp_reference = &value["sourceReferences"]["timestamp"];
        let selected_clock = self
            .field(timestamp_reference)
            .and_then(|(message, field)| {
                (message.global == 20 && field.coordinates.number == 253)
                    .then(|| self.timestamp(timestamp_reference))
                    .flatten()
            });
        let source_index = timestamp_reference["messageIndex"]
            .as_u64()
            .or_else(|| {
                sources
                    .values()
                    .find_map(|reference| reference["messageIndex"].as_u64())
            })
            .and_then(|index| usize::try_from(index).ok());
        let clock =
            selected_clock.or_else(|| self.messages.get(source_index?)?.facts.as_deref()?.clock);
        let mut unsafe_metrics = 0;
        if (!value["timestamp"].is_null() || !value["elapsedSeconds"].is_null())
            && selected_clock
                .zip(source_seconds(&value["timestamp"]))
                .is_none_or(|(source, row)| (source - row).abs() > 0.001)
        {
            unsafe_metrics |= 128;
        }
        let metrics = [
            ("heartRateBpm", 1),
            ("powerWatts", 2),
            ("speedMps", 4),
            ("paceSecondsPerKm", 4),
            ("cadenceStepsPerMinute", 8),
            ("altitudeMeters", 16),
            ("distanceMeters", 32),
            ("location", 64),
        ];
        for (key, mask) in metrics {
            if !value[key].is_null() && !super::privacy::sample_source_safe(value, key, self) {
                unsafe_metrics |= mask;
            }
        }
        if let Some(time) = clock {
            self.observed_records
                .entry(source_time_key(time))
                .and_modify(|mask| *mask |= unsafe_metrics)
                .or_insert(unsafe_metrics);
        }
        source_bad_record(
            clock,
            unsafe_metrics,
            &mut self.observed_unsafe_times,
            &mut self.observed_unlocated,
        );
        Ok(())
    }
    fn field(&self, reference: &Value) -> Option<(&SourceFacts, &SourceField)> {
        if !self.decoder_verified {
            return None;
        }
        let index = usize::try_from(reference["messageIndex"].as_u64()?).ok()?;
        let message = self.messages.get(index)?;
        if reference["globalMessageNumber"].as_u64() != Some(message.global as u64) {
            return None;
        }
        let message = message.facts.as_deref()?;
        let coordinates = SourceCoordinates::read(reference)?;
        let position = message
            .fields
            .binary_search_by_key(&coordinates, |field| field.coordinates)
            .ok()?;
        let field = &message.fields[position];
        if let Some(index) = reference.get("arrayIndex") {
            let index = usize::try_from(index.as_u64()?).ok()?;
            if source_array(message, field).is_none_or(|values| index >= values.len()) {
                return None;
            }
        }
        Some((message, field))
    }
    pub(super) fn reference(&self, reference: &Value) -> bool {
        self.field(reference).is_some()
    }
    pub(super) fn field_payload_safe(&self, reference: &Value, value: &Value) -> bool {
        let Some((message, field)) = self.field(reference) else {
            return false;
        };
        if reference.get("arrayIndex").is_some() {
            return false;
        }
        for identity in [value, &value["identity"]] {
            if !identity["developerIdentity"].is_null()
                || identity
                    .get("fieldNumber")
                    .filter(|value| !value.is_null())
                    .is_some_and(|number| number.as_u64() != Some(field.coordinates.number as u64))
                || identity
                    .get("globalMessageNumber")
                    .filter(|value| !value.is_null())
                    .is_some_and(|number| number.as_u64() != Some(message.global as u64))
            {
                return false;
            }
        }
        let role = match field.role {
            1 => "expanded",
            2 => "reconstructed",
            _ => "native",
        };
        let parent_matches = match field.coordinates.parent {
            Some(parent) => value["componentParent"].as_u64() == Some(parent as u64),
            None => value["componentParent"].is_null(),
        };
        if value["role"] != role || !parent_matches {
            return false;
        }
        if !value["enumCode"].is_null() {
            match &field.value {
                SourceNumber::Enum(_, code) if value["enumCode"].as_i64() == Some(*code) => {}
                _ => return false,
            }
        }
        if !value["validity"].is_null()
            && !matches!(
                value["validity"].as_str(),
                Some("valid" | "mixed" | "invalid")
            )
        {
            return false;
        }
        if !value["elementValidity"].is_null() {
            let Some(source) = source_array(message, field) else {
                return false;
            };
            if !value["elementValidity"].as_array().is_some_and(|validity| {
                validity.len() == source.len()
                    && source.iter().zip(validity).all(|(source, validity)| {
                        validity == if source.is_some() { "valid" } else { "invalid" }
                    })
            }) {
                return false;
            }
        }
        let metadata = [value.get("baseType"), value.get("type")];
        // Compressed-header timestamps have no primitive wire metadata.
        let has_metadata = metadata.into_iter().flatten().any(|value| !value.is_null());
        if (!has_metadata && (field.role != 2 || field.wire.is_some()))
            || metadata
                .into_iter()
                .flatten()
                .filter(|value| !value.is_null())
                .any(|value| field.wire.is_none() || source_wire(value) != field.wire)
        {
            return false;
        }
        crate::fit::raw::verified_primitive(
            message.global as u64,
            field.coordinates.number as u64,
            value,
            field.role == 2,
            crate::fit::raw::enum_profile(message.global as u64, field.coordinates.number as u64)
                .is_some(),
        ) && source_payload(&field.value, &value["value"], Some(message))
            && source_payload(
                field.raw.as_ref().unwrap_or(&field.value),
                &value["rawValue"],
                Some(message),
            )
    }
    pub(super) fn scalar(&self, reference: &Value) -> Option<f64> {
        let (message, field) = self.field(reference)?;
        match &field.value {
            SourceNumber::Scalar(value) => *value,
            SourceNumber::Array(values) => {
                let index = usize::try_from(reference.get("arrayIndex")?.as_u64()?).ok()?;
                values.get(index).copied().flatten()
            }
            SourceNumber::Rr(_, _) => {
                let values = source_values(message, field)?;
                let index = match reference.get("arrayIndex") {
                    Some(index) => usize::try_from(index.as_u64()?).ok()?,
                    None => values.len().checked_sub(1)?,
                };
                values.get(index).copied().flatten()
            }
            SourceNumber::Date(_) | SourceNumber::Enum(_, _) => None,
        }
    }
    pub(super) fn timestamp(&self, reference: &Value) -> Option<f64> {
        match &self.field(reference)?.1.value {
            SourceNumber::Date(time) => Some(source_utc_seconds(*time)),
            _ => None,
        }
    }
    pub(super) fn message_reference(&self, reference: &Value) -> bool {
        if reference.get("fieldNumber").is_some()
            || reference.get("componentParent").is_some()
            || reference.get("arrayIndex").is_some()
            || !reference["developerIdentity"].is_null()
        {
            return false;
        }
        let Some(index) = reference["messageIndex"]
            .as_u64()
            .and_then(|index| usize::try_from(index).ok())
        else {
            return false;
        };
        self.messages.get(index).is_some_and(|message| {
            reference["globalMessageNumber"].as_u64() == Some(message.global as u64)
                && reference["byteOffset"]
                    .as_u64()
                    .zip(reference["byteLength"].as_u64())
                    .is_some_and(|frame| message.frame_valid && message.frame == frame)
        })
    }
    fn native_field(&self, index: usize, number: u64) -> Option<&SourceField> {
        if !self.decoder_verified {
            return None;
        }
        let message = self.messages.get(index)?.facts.as_deref()?;
        if number > 255 || Native::bit(&message.unsafe_fields, number as u8) {
            return None;
        }
        let mut fields = message.fields.iter().filter(|field| {
            field.coordinates.number as u64 == number && field.coordinates.parent.is_none()
        });
        let field = fields.next()?;
        if fields.next().is_some() {
            return None;
        }
        Some(field)
    }
    fn native_reference(&self, index: usize, number: u64) -> Option<Value> {
        let message = self.messages.get(index)?;
        let field = self.native_field(index, number)?;
        Some(
            json!({"messageIndex":index,"globalMessageNumber":message.global,
            "fieldNumber":number,"byteOffset":field.coordinates.offset,"byteLength":field.coordinates.length}),
        )
    }
    pub(super) fn session_reference(&self, number: u64) -> Option<Value> {
        if self.session_count != 1 {
            return None;
        }
        self.native_reference(self.session?, number)
    }
    pub(super) fn sibling_reference(&self, reference: &Value, number: u64) -> Option<Value> {
        if !self.reference(reference) && !self.message_reference(reference) {
            return None;
        }
        self.native_reference(
            usize::try_from(reference["messageIndex"].as_u64()?).ok()?,
            number,
        )
    }
    pub(super) fn sibling_safe(&self, reference: &Value, number: u64) -> bool {
        number <= 255
            && self
                .field(reference)
                .is_some_and(|(message, _)| !Native::bit(&message.unsafe_fields, number as u8))
    }
    pub(super) fn rr_previous(&self, reference: &Value) -> Option<f64> {
        let (message, field) = self.field(reference)?;
        if message.global != 132 || field.coordinates.number != 9 {
            return None;
        }
        let values = source_values(message, field)?;
        let index = match reference.get("arrayIndex") {
            Some(index) => usize::try_from(index.as_u64()?).ok()?,
            None => values.len().checked_sub(1)?,
        };
        if index == 0 {
            field.previous
        } else {
            values.get(index - 1).copied().flatten()
        }
    }
    pub(super) fn rr_interval_safe(&self, value: &Value) -> bool {
        let source = &value["sourceReference"];
        let Some((message, field)) = self.field(source) else {
            return false;
        };
        if source.get("arrayIndex").is_none()
            && let Some(values) = source_array(message, field)
        {
            let anchor_row = message.global == 132
                && field.coordinates.parent.is_none()
                && values.len() == 1
                && values[0].is_some()
                && message.rr.as_ref().is_some_and(|native| {
                    native.timestamp.is_some() && native.fractional_timestamp.is_some()
                });
            if !anchor_row {
                return false;
            }
        }
        if message.global == 132 {
            match (
                value["rrMs"].as_f64(),
                self.scalar(source),
                self.rr_previous(source),
            ) {
                (Some(rr), Some(counter), Some(previous))
                    if counter > previous
                        && (rr / 1000.0 - (counter - previous)).abs() <= 0.000001 => {}
                (None, None, _) => {}
                (None, Some(counter), Some(previous)) if counter <= previous => {}
                _ => return false,
            }
            if value["provenance"]["timingMethod"] == "hr_event_counter_anchor" && !field.clock_safe
            {
                return false;
            }
        }
        if self.session_count != 1 {
            return false;
        }
        let Some(session) = self.session else {
            return false;
        };
        let Some(SourceField {
            value: SourceNumber::Date(start),
            ..
        }) = self.native_field(session, 2)
        else {
            return false;
        };
        let Some((_, end)) = self.session_window() else {
            return false;
        };
        let duration = end - source_utc_seconds(*start);
        let mut all_eligible = true;
        let mut previous_end = None;
        validate_rr(
            value,
            *start,
            duration,
            &mut all_eligible,
            &mut previous_end,
            |reference, require_valid| {
                let (message, field) = self.field(reference).ok_or_else(invalid)?;
                if require_valid {
                    let present = match &field.value {
                        SourceNumber::Date(_) => true,
                        SourceNumber::Scalar(value) => value.is_some(),
                        SourceNumber::Array(_) | SourceNumber::Rr(_, _) => {
                            self.scalar(reference).is_some()
                        }
                        SourceNumber::Enum(_, _) => false,
                    };
                    if !present {
                        return Err(invalid());
                    }
                }
                message.rr.as_deref().ok_or_else(invalid)
            },
            |index| {
                self.messages
                    .get(index)
                    .and_then(|message| message.facts.as_deref())
                    .and_then(|facts| facts.rr.as_deref())
            },
        )
        .is_ok()
    }
    pub(super) fn session_start(&self) -> Option<f64> {
        if self.session_count != 1 {
            return None;
        }
        match &self.native_field(self.session?, 2)?.value {
            SourceNumber::Date(time) => Some(source_utc_seconds(*time)),
            _ => None,
        }
    }
    pub(super) fn session_window(&self) -> Option<(f64, f64)> {
        let start = self.session_start()?;
        let end = match &self.native_field(self.session?, 253)?.value {
            SourceNumber::Date(time) => source_utc_seconds(*time),
            _ => return None,
        };
        (end >= start).then_some((start, end))
    }
    pub(super) fn record_window_safe(&self, metric: &str, from: f64, to: f64) -> bool {
        let mask = match metric {
            "heartRateBpm" => 1,
            "powerWatts" => 2,
            "speedMps" => 4,
            "cadenceRpm" | "cadenceStepsPerMinute" => 8,
            "altitudeMeters" => 16,
            "distanceMeters" => 32,
            "location" => 64,
            "clock" => 128,
            _ => return false,
        };
        if !self.decoder_verified || !from.is_finite() || !to.is_finite() || from > to {
            return false;
        }
        let mask = mask | 128;
        let (times, unlocated) = if self.observed_samples {
            (&self.observed_unsafe_times, self.observed_unlocated)
        } else {
            (&self.unsafe_record_times, self.unlocated_unsafe)
        };
        if unlocated & mask != 0 {
            return false;
        }
        let from = source_time_key(from);
        let to = source_time_key(to);
        if self.observed_samples {
            if self.observed_records.range(from..).next().is_some()
                && self
                    .observed_records
                    .range(..from)
                    .next_back()
                    .is_some_and(|(_, unsafe_metrics)| unsafe_metrics & mask != 0)
            {
                return false;
            }
            if self.observed_records.range(..=to).next_back().is_some()
                && self
                    .observed_records
                    .range((std::ops::Bound::Excluded(to), std::ops::Bound::Unbounded))
                    .next()
                    .is_some_and(|(_, unsafe_metrics)| unsafe_metrics & mask != 0)
            {
                return false;
            }
        }
        (0..8).all(|bit| mask & (1 << bit) == 0 || times[bit].range(from..=to).next().is_none())
    }
    pub(super) fn clock_window_safe(&self, from: f64, to: f64) -> bool {
        self.record_window_safe("clock", from, to)
    }
    pub(super) fn timer_window_safe(&self, from: f64, to: f64) -> bool {
        if !self.decoder_verified
            || !from.is_finite()
            || !to.is_finite()
            || from > to
            || self.unlocated_timer
        {
            return false;
        }
        let from = source_time_key(from);
        let to = source_time_key(to);
        self.unsafe_timer_times.range(from..=to).next().is_none()
            && self
                .timer_states
                .range(..from)
                .next_back()
                .is_none_or(|(_, safe)| *safe)
    }
}
fn source_wire(value: &Value) -> Option<u16> {
    if let Some(number) = value.as_u64() {
        return u16::try_from(number).ok().filter(|number| *number <= 255);
    }
    let name = value.as_str()?;
    if name == "date_time" {
        return Some(256);
    }
    matches!(
        name,
        "enum"
            | "sint8"
            | "uint8"
            | "sint16"
            | "uint16"
            | "sint32"
            | "uint32"
            | "sint64"
            | "uint64"
            | "uint8z"
            | "uint16z"
            | "uint32z"
            | "uint64z"
            | "float32"
            | "float64"
            | "byte"
    )
    .then(|| fitparser::profile::field_types::FitBaseType::from(name).as_i64() as u16)
}
fn source_enum(
    global: u16,
    number: u8,
    field: &Value,
) -> Option<(fitparser::profile::FieldDataType, i64)> {
    let text = field["value"].as_str()?;
    let code = field["enumCode"]
        .as_i64()
        .or_else(|| field["rawValue"].as_i64())?;
    crate::fit::raw::enum_profile(global as u64, number as u64)?
        .0
        .iter()
        .find(|kind| {
            kind.is_named_variant(code)
                && fitparser::profile::get_field_variant_as_string(**kind, code) == text
        })
        .map(|kind| (*kind, code))
}
fn source_payload(source: &SourceNumber, value: &Value, message: Option<&SourceFacts>) -> bool {
    match source {
        SourceNumber::Scalar(None) => value.is_null(),
        SourceNumber::Scalar(Some(number)) => value.as_f64() == Some(*number),
        SourceNumber::Date(time) => timestamp(value).ok() == Some(*time),
        SourceNumber::Enum(kind, code) => value.as_str().is_some_and(|text| {
            fitparser::profile::get_field_variant_as_string(*kind, *code) == text
        }),
        SourceNumber::Array(values) => source_numeric_array(values, value),
        SourceNumber::Rr(index, array) => {
            let Some(values) = message
                .and_then(|message| message.rr.as_ref())
                .and_then(|native| native.rr_fields.as_ref())
                .and_then(|fields| fields.get(*index))
            else {
                return false;
            };
            if *array {
                source_numeric_array(&values.values, value)
            } else {
                match values.values.first().copied().flatten() {
                    Some(number) => value.as_f64() == Some(number),
                    None => value.is_null(),
                }
            }
        }
    }
}
fn source_numeric_array(source: &[Option<f64>], value: &Value) -> bool {
    value.as_array().is_some_and(|values| {
        values.len() == source.len()
            && source
                .iter()
                .zip(values)
                .all(|(source, value)| match source {
                    Some(number) => value.as_f64() == Some(*number),
                    None => value.is_null(),
                })
    })
}
fn source_bad_record(
    time: Option<f64>,
    mask: u16,
    times: &mut [BTreeSet<u64>; 8],
    unlocated: &mut u16,
) {
    if mask == 0 {
        return;
    }
    if let Some(time) = time {
        for (bit, entries) in times.iter_mut().enumerate() {
            if mask & (1 << bit) != 0 {
                entries.insert(source_time_key(time));
            }
        }
    } else {
        *unlocated |= mask;
    }
}
fn source_time_key(time: f64) -> u64 {
    let bits = if time == 0.0 { 0 } else { time.to_bits() };
    if bits >> 63 != 0 {
        !bits
    } else {
        bits ^ (1 << 63)
    }
}
fn source_array<'a>(message: &'a SourceFacts, field: &'a SourceField) -> Option<&'a [Option<f64>]> {
    if matches!(field.value, SourceNumber::Rr(_, false)) {
        return None;
    }
    source_values(message, field)
}
fn source_values<'a>(
    message: &'a SourceFacts,
    field: &'a SourceField,
) -> Option<&'a [Option<f64>]> {
    match &field.value {
        SourceNumber::Array(values) => Some(values),
        SourceNumber::Rr(index, _) => message
            .rr
            .as_ref()?
            .rr_fields
            .as_ref()?
            .get(*index)
            .map(|field| field.values.as_ref()),
        _ => None,
    }
}
fn source_seconds(value: &Value) -> Option<f64> {
    timestamp(value).ok().map(source_utc_seconds)
}
fn source_utc_seconds(time: DateTime<Utc>) -> f64 {
    time.timestamp() as f64 + f64::from(time.timestamp_subsec_nanos()) / 1_000_000_000.0
}
fn source_numeric(value: &Value) -> Option<f64> {
    value.as_f64().filter(|value| value.is_finite())
}
fn source_metric(number: u8) -> u16 {
    match number {
        3 => 1,
        7 => 2,
        6 | 73 => 4,
        4 | 53 => 8,
        2 | 78 => 16,
        5 => 32,
        0 | 1 => 64,
        253 => 128,
        _ => 0,
    }
}
fn validate_native_reference(
    value: &Value,
    index: usize,
    global: u16,
    field: Option<u8>,
    developer: &Value,
) -> Result<(), ApiError> {
    if value["messageIndex"].as_u64() != Some(index as u64)
        || value["globalMessageNumber"].as_u64() != Some(global as u64)
    {
        return Err(invalid());
    }
    if let Some(field) = field
        && (value["fieldNumber"].as_u64() != Some(field as u64)
            || value.get("developerIdentity").unwrap_or(&Value::Null) != developer)
    {
        return Err(invalid());
    }
    Ok(())
}
fn timestamp(value: &Value) -> Result<DateTime<Utc>, ApiError> {
    let text = value.as_str().ok_or_else(invalid)?;
    if !text.ends_with('Z') && !text.ends_with("+00:00") {
        return Err(invalid());
    }
    DateTime::parse_from_rfc3339(text)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|_| invalid())
}
fn numeric(value: &Value, negative: bool) -> Result<(), ApiError> {
    if !value.is_null()
        && value
            .as_f64()
            .is_none_or(|n| !n.is_finite() || (!negative && n < 0.0))
    {
        return Err(invalid());
    }
    Ok(())
}
fn bound(value: &Value, duration: f64) -> Result<(), ApiError> {
    numeric(value, false)?;
    if value.as_f64().is_some_and(|n| n > duration + 0.001) {
        return Err(invalid());
    }
    Ok(())
}
fn strip_unused(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.remove("sourceReferences");
            map.remove("sourceReference");
            map.remove("extensions");
            for child in map.values_mut() {
                strip_unused(child);
            }
        }
        Value::Array(array) => {
            for child in array {
                strip_unused(child);
            }
        }
        _ => {}
    }
}

struct Validation<'a> {
    lineage: &'a Lineage,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    duration: f64,
    indices: BTreeMap<String, u64>,
    next_sample_source: usize,
    rr_all_eligible: bool,
    rr_previous_end: Option<f64>,
    sample_hash: Sha256,
    lap_hash: Sha256,
    event_hash: Sha256,
    metric_samples: u64,
}
impl<'a> Validation<'a> {
    fn next_sample(&mut self) -> Option<usize> {
        while let Some(message) = self.lineage.messages.get(self.next_sample_source) {
            let index = self.next_sample_source;
            self.next_sample_source += 1;
            if message.global == 20
                && message
                    .timestamp
                    .is_none_or(|time| time >= self.start && time <= self.end)
            {
                return Some(index);
            }
        }
        None
    }
    fn row(&mut self, path: &[String], value: &Value) -> Result<(), ApiError> {
        self.lineage.references(value)?;
        let kind = if path == ["rr", "intervals", "*"] {
            "rrIntervals"
        } else if path.len() == 2 && path[1] == "*" {
            path[0].as_str()
        } else {
            return Ok(());
        };
        if !matches!(kind, "samples" | "laps" | "timerEvents" | "rrIntervals") {
            return Ok(());
        }
        let index = self.indices.entry(kind.into()).or_default();
        if value["index"].as_u64() != Some(*index) {
            return Err(invalid());
        }
        *index += 1;
        for key in ["timestamp", "startTime", "endTime"] {
            if !value[key].is_null() {
                let t = timestamp(&value[key])?;
                if kind != "rrIntervals" && (t < self.start || t > self.end) {
                    return Err(invalid());
                }
            }
        }
        for key in ["elapsedSeconds", "startElapsedSeconds", "endElapsedSeconds"] {
            if kind == "rrIntervals" {
                numeric(&value[key], true)?;
            } else {
                bound(&value[key], self.duration)?;
            }
        }
        if let (Some(from), Some(to)) = (
            value["startElapsedSeconds"].as_f64(),
            value["endElapsedSeconds"].as_f64(),
        ) && to < from
        {
            return Err(invalid());
        }
        match kind {
            "samples" => {
                let expected_source = self.next_sample().ok_or_else(invalid)? as u64;
                if value.get("timestamp").is_none() || value.get("elapsedSeconds").is_none() {
                    return Err(invalid());
                }
                let sources = value["sourceReferences"].as_object().ok_or_else(invalid)?;
                let mut source_index = None;
                for reference in sources.values() {
                    let native = self.lineage.reference(reference, false)?;
                    let index = reference["messageIndex"].as_u64().ok_or_else(invalid)?;
                    if native.global != 20 || source_index.is_some_and(|previous| previous != index)
                    {
                        return Err(invalid());
                    }
                    source_index = Some(index);
                }
                if source_index.is_some_and(|index| index != expected_source) {
                    return Err(invalid());
                }
                for key in [
                    "speedMps",
                    "paceSecondsPerKm",
                    "heartRateBpm",
                    "powerWatts",
                    "cadenceStepsPerMinute",
                    "altitudeMeters",
                    "distanceMeters",
                ] {
                    if value.get(key).is_none() {
                        return Err(invalid());
                    }
                    numeric(&value[key], key == "altitudeMeters")?;
                    if value[key].is_number() {
                        let source = &value["sourceReferences"][key];
                        let native = self.lineage.reference(source, true)?;
                        let field = source["fieldNumber"].as_u64().ok_or_else(invalid)?;
                        let fields: &[u64] = match key {
                            "speedMps" | "paceSecondsPerKm" => &[73, 6],
                            "heartRateBpm" => &[3],
                            "powerWatts" => &[7],
                            "cadenceStepsPerMinute" => &[4],
                            "altitudeMeters" => &[78, 2],
                            "distanceMeters" => &[5],
                            _ => &[],
                        };
                        if native.global != 20
                            || !source["developerIdentity"].is_null()
                            || !fields.contains(&field)
                        {
                            return Err(invalid());
                        }
                    }
                }
                if !value["timerRunning"].is_null() && !value["timerRunning"].is_boolean() {
                    return Err(invalid());
                }
                if value["timestamp"].is_string() {
                    let source = &value["sourceReferences"]["timestamp"];
                    let native = self.lineage.reference(source, true)?;
                    let time = timestamp(&value["timestamp"])?;
                    if native.global != 20
                        || source["fieldNumber"] != 253
                        || native.timestamp != Some(time)
                    {
                        return Err(invalid());
                    }
                    let elapsed = (time - self.start).num_microseconds().ok_or_else(invalid)?
                        as f64
                        / 1_000_000.0;
                    if value["elapsedSeconds"]
                        .as_f64()
                        .is_none_or(|v| (v - elapsed).abs() > 0.001)
                    {
                        return Err(invalid());
                    }
                } else if !value["elapsedSeconds"].is_null() {
                    return Err(invalid());
                }
                if ["speedMps", "heartRateBpm", "powerWatts", "distanceMeters"]
                    .iter()
                    .any(|key| value[*key].is_number())
                {
                    self.metric_samples += 1;
                }
                for key in [
                    "timestamp",
                    "elapsedSeconds",
                    "speedMps",
                    "heartRateBpm",
                    "powerWatts",
                    "cadenceStepsPerMinute",
                    "distanceMeters",
                    "altitudeMeters",
                ] {
                    hash_value(&mut self.sample_hash, &value[key])?;
                }
            }
            "laps" => {
                if !value["summary"].is_object() {
                    return Err(invalid());
                }
                for key in [
                    "distanceMeters",
                    "timerTimeSeconds",
                    "elapsedTimeSeconds",
                    "movingTimeSeconds",
                    "averageSpeedMps",
                    "averagePaceSecondsPerKm",
                    "averageHeartRateBpm",
                    "averagePowerWatts",
                    "averageCadenceStepsPerMinute",
                ] {
                    numeric(&value["summary"][key], false)?;
                }
                let mut compact = value.clone();
                strip_unused(&mut compact);
                hash_value(&mut self.lap_hash, &compact)?;
            }
            "timerEvents" => {
                let mut compact = value.clone();
                strip_unused(&mut compact);
                hash_value(&mut self.event_hash, &compact)?;
            }
            "rrIntervals" => self.rr(value)?,
            _ => {}
        }
        Ok(())
    }
    fn rr(&mut self, value: &Value) -> Result<(), ApiError> {
        validate_rr(
            value,
            self.start,
            self.duration,
            &mut self.rr_all_eligible,
            &mut self.rr_previous_end,
            |reference, valid| self.lineage.reference(reference, valid),
            |index| self.lineage.messages.get(index),
        )
    }
}
fn validate_rr<'a>(
    value: &Value,
    start: DateTime<Utc>,
    duration: f64,
    rr_all_eligible: &mut bool,
    rr_previous_end: &mut Option<f64>,
    reference: impl Fn(&Value, bool) -> Result<&'a Native, ApiError>,
    message: impl Fn(usize) -> Option<&'a Native>,
) -> Result<(), ApiError> {
    for key in [
        "rrMs",
        "timestamp",
        "elapsedSeconds",
        "startElapsedSeconds",
        "endElapsedSeconds",
        "sourceReference",
        "provenance",
        "timingEligible",
    ] {
        if value.get(key).is_none() {
            return Err(invalid());
        }
    }
    numeric(&value["rrMs"], false)?;
    let rr = value["rrMs"].as_f64();
    let source = &value["sourceReference"];
    let native = reference(source, rr.is_some_and(|n| n > 0.0))?;
    if !source["developerIdentity"].is_null() {
        return Err(invalid());
    }
    let source_field = source["fieldNumber"].as_u64().ok_or_else(invalid)?;
    if !matches!((native.global, source_field), (78, 0) | (132, 9))
        || value["provenance"]["kind"] != "recorded_rr"
        || value["provenance"]["messageNumber"].as_u64() != Some(native.global as u64)
        || value["provenance"]["fieldNumber"].as_u64() != Some(source_field)
    {
        return Err(invalid());
    }
    if native.global == 78 {
        let recorded = native.rr_value(source)?;
        match (rr, recorded) {
            (Some(rr), Some(recorded)) if (rr / 1000.0 - recorded).abs() <= 0.000001 => {}
            (None, None) => {}
            _ => return Err(invalid()),
        }
    }
    let eligible = value["timingEligible"].as_bool().ok_or_else(invalid)?;
    if !eligible {
        *rr_all_eligible = false;
        *rr_previous_end = None;
    }
    let method = value["provenance"]["timingMethod"]
        .as_str()
        .ok_or_else(invalid)?;
    if method == "unanchored" {
        if eligible
            || !value["timestamp"].is_null()
            || !value["elapsedSeconds"].is_null()
            || !value["startElapsedSeconds"].is_null()
            || !value["endElapsedSeconds"].is_null()
            || !value["provenance"]["anchorTimestamp"].is_null()
            || !value["provenance"]["anchorSourceReferences"].is_null()
        {
            return Err(invalid());
        }
        return Ok(());
    }
    if method != "hr_event_counter_anchor" || native.global != 132 {
        return Err(invalid());
    }
    if eligible && rr.is_none_or(|rr| rr <= 0.0) {
        return Err(invalid());
    }
    let anchor = timestamp(&value["provenance"]["anchorTimestamp"])?;
    let references = &value["provenance"]["anchorSourceReferences"];
    let mut anchor_index = None;
    for (key, field) in [
        ("timestamp", 253),
        ("fractionalTimestamp", 0),
        ("eventCounter", 9),
    ] {
        let reference_value = &references[key];
        let proof = reference(reference_value, true)?;
        if proof.global != 132
            || reference_value["fieldNumber"] != field
            || !reference_value["developerIdentity"].is_null()
        {
            return Err(invalid());
        }
        let index = reference_value["messageIndex"]
            .as_u64()
            .ok_or_else(invalid)?;
        if anchor_index.is_some_and(|previous| previous != index) {
            return Err(invalid());
        }
        anchor_index = Some(index);
    }
    if anchor_index.is_none_or(|index| {
        source["messageIndex"]
            .as_u64()
            .is_none_or(|source| index > source)
    }) {
        return Err(invalid());
    }
    let anchor_native = message(anchor_index.ok_or_else(invalid)? as usize).ok_or_else(invalid)?;
    let base = anchor_native.timestamp.ok_or_else(invalid)?;
    let fraction = anchor_native.fractional_timestamp.ok_or_else(invalid)?;
    if !fraction.is_finite() || fraction < 0.0 {
        return Err(invalid());
    }
    let proved = base
        .checked_add_signed(chrono::Duration::microseconds(
            (fraction * 1_000_000.0).round() as i64,
        ))
        .ok_or_else(invalid)?;
    if (proved - anchor)
        .num_microseconds()
        .ok_or_else(invalid)?
        .abs()
        > 1000
    {
        return Err(invalid());
    }
    let from = value["startElapsedSeconds"].as_f64();
    let to = value["endElapsedSeconds"].as_f64().ok_or_else(invalid)?;
    match (rr, from) {
        (Some(rr), Some(from)) if (to - from - rr / 1000.0).abs() <= 0.001 => {}
        (None, None) if !eligible => {}
        _ => return Err(invalid()),
    }
    if value["elapsedSeconds"]
        .as_f64()
        .is_none_or(|v| (v - to).abs() > 0.001)
    {
        return Err(invalid());
    }
    let end = timestamp(&value["timestamp"])?;
    let elapsed = (end - start).num_microseconds().ok_or_else(invalid)? as f64 / 1_000_000.0;
    if (elapsed - to).abs() > 0.001 {
        return Err(invalid());
    }
    let counter = native.rr_value(source)?.ok_or_else(invalid)?;
    let anchor_counter = anchor_native
        .rr_value(&references["eventCounter"])?
        .ok_or_else(invalid)?;
    let counter_elapsed =
        (anchor - start).num_microseconds().ok_or_else(invalid)? as f64 / 1_000_000.0 + counter
            - anchor_counter;
    if (counter_elapsed - to).abs() > 0.001 {
        // A fresh native timestamp proves an ineligible reset/gap endpoint.
        // It cannot establish counter continuity or make that row eligible.
        let native_counter = native.rr_fields.as_ref().is_some_and(|fields| {
            fields.iter().any(|field| {
                field.number == 9 && field.component.is_none() && field.values.len() == 1
            })
        });
        let fresh =
            native
                .timestamp
                .zip(native.fractional_timestamp)
                .and_then(|(timestamp, fraction)| {
                    timestamp.checked_add_signed(chrono::Duration::microseconds(
                        (fraction * 1_000_000.0).round() as i64,
                    ))
                });
        let fresh_elapsed = fresh
            .and_then(|timestamp| (timestamp - start).num_microseconds())
            .map(|micros| micros as f64 / 1_000_000.0);
        if eligible
            || !native_counter
            || fresh_elapsed.is_none_or(|elapsed| (elapsed - to).abs() > 0.001)
        {
            return Err(invalid());
        }
    }
    if eligible {
        let from = from.ok_or_else(invalid)?;
        if from < 0.0 || to > duration + 0.001 {
            return Err(invalid());
        }
        if rr_previous_end.is_some_and(|last| (last - from).abs() > 0.001) {
            *rr_all_eligible = false;
        }
        *rr_previous_end = Some(to);
    }
    Ok(())
}
struct HashSink<'a>(&'a mut Sha256);
impl Write for HashSink<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn hash_value(hash: &mut Sha256, value: &Value) -> Result<(), ApiError> {
    serde_json::to_writer(HashSink(hash), value).map_err(|_| invalid())?;
    hash.update([0]);
    Ok(())
}

const WORKSPACE_BYTES: u64 = 512 * 1024 * 1024;
struct Scan {
    input: BufReader<File>,
    position: u64,
}
impl Scan {
    fn peek(&mut self) -> Result<Option<u8>, ApiError> {
        Ok(self
            .input
            .fill_buf()
            .map_err(|_| storage())?
            .first()
            .copied())
    }
    fn byte(&mut self) -> Result<u8, ApiError> {
        let value = self.peek()?.ok_or_else(invalid)?;
        self.input.consume(1);
        self.position += 1;
        Ok(value)
    }
    fn whitespace(&mut self) -> Result<(), ApiError> {
        while self.peek()?.is_some_and(|b| b.is_ascii_whitespace()) {
            self.byte()?;
        }
        Ok(())
    }
    fn value(&mut self) -> Result<(u64, u64), ApiError> {
        self.whitespace()?;
        let start = self.position;
        let first = self.byte()?;
        match first {
            b'{' | b'[' => {
                let mut stack = vec![if first == b'{' { b'}' } else { b']' }];
                let mut string = false;
                let mut escape = false;
                while !stack.is_empty() {
                    let byte = self.byte()?;
                    if string {
                        if escape {
                            escape = false;
                        } else if byte == b'\\' {
                            escape = true;
                        } else if byte == b'"' {
                            string = false;
                        }
                        continue;
                    }
                    match byte {
                        b'"' => string = true,
                        b'{' => stack.push(b'}'),
                        b'[' => stack.push(b']'),
                        b'}' | b']' if stack.pop() != Some(byte) => {
                            return Err(invalid());
                        }
                        b'}' | b']' => {}
                        _ => {}
                    }
                    if stack.len() > 128 {
                        return Err(invalid());
                    }
                }
            }
            b'"' => {
                let mut escape = false;
                loop {
                    let byte = self.byte()?;
                    if escape {
                        escape = false;
                    } else if byte == b'\\' {
                        escape = true;
                    } else if byte == b'"' {
                        break;
                    }
                }
            }
            _ => {
                while self
                    .peek()?
                    .is_some_and(|b| !b.is_ascii_whitespace() && b != b',' && b != b'}')
                {
                    self.byte()?;
                }
            }
        }
        Ok((start, self.position - start))
    }
}
fn root_spans(path: &Path) -> Result<BTreeMap<String, (u64, u64)>, ApiError> {
    let mut scan = Scan {
        input: BufReader::with_capacity(CHUNK_BYTES, File::open(path).map_err(|_| storage())?),
        position: 0,
    };
    scan.whitespace()?;
    if scan.byte()? != b'{' {
        return Err(invalid());
    }
    let mut spans = BTreeMap::new();
    loop {
        scan.whitespace()?;
        if scan.peek()? == Some(b'}') {
            scan.byte()?;
            break;
        }
        let (offset, length) = scan.value()?;
        if length > RECORD_BYTES as u64 {
            return Err(invalid());
        }
        let key: String =
            serde_json::from_reader(span_reader(path, offset, length)?).map_err(|_| invalid())?;
        if !matches!(key.as_str(), "decoded" | "normalized" | "legacy") || spans.contains_key(&key)
        {
            return Err(invalid());
        }
        scan.whitespace()?;
        if scan.byte()? != b':' {
            return Err(invalid());
        }
        spans.insert(key, scan.value()?);
        scan.whitespace()?;
        match scan.byte()? {
            b',' => {
                scan.whitespace()?;
                if scan.peek()? == Some(b'}') {
                    return Err(invalid());
                }
            }
            b'}' => break,
            _ => return Err(invalid()),
        }
    }
    scan.whitespace()?;
    if scan.peek()?.is_some() || spans.len() != 3 {
        return Err(invalid());
    }
    Ok(spans)
}
fn span_reader(path: &Path, offset: u64, length: u64) -> Result<io::Take<File>, ApiError> {
    let mut file = File::open(path).map_err(|_| storage())?;
    let end = offset.checked_add(length).ok_or_else(invalid)?;
    if end > file.metadata().map_err(|_| storage())?.len() {
        return Err(invalid());
    }
    file.seek(SeekFrom::Start(offset)).map_err(|_| storage())?;
    Ok(file.take(length))
}
struct HashRead<R> {
    inner: R,
    hash: Rc<RefCell<Sha256>>,
}
impl<R: Read> Read for HashRead<R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(bytes)?;
        self.hash.borrow_mut().update(&bytes[..n]);
        Ok(n)
    }
}
fn archive(
    path: &Path,
    span: (u64, u64),
    lineage: &mut Lineage,
    decoded: bool,
) -> Result<Document, ApiError> {
    let hash = Rc::new(RefCell::new(Sha256::new()));
    let reader = HashRead {
        inner: span_reader(path, span.0, span.1)?,
        hash: hash.clone(),
    };
    let mut metadata = json!({});
    let mut counts = BTreeMap::new();
    let mut keep = |_: &[String]| true;
    let mut project = |path: &[String], value: Value| -> Result<Option<Value>, ApiError> {
        if decoded && path == ["messages", "*"] {
            lineage.message(&value)?;
        }
        if !decoded {
            lineage.references(&value)?;
        }
        if path.len() == 1 && atomic(path) {
            metadata[&path[0]] = value.clone();
        }
        if path == ["rr", "alignmentEligible"] {
            metadata["rrAlignmentEligible"] = value.clone();
        }
        Ok(Some(value))
    };
    // Archives are already immutable raw spans. Validate without serializing
    // every record a second time into a discarded output buffer.
    walk_document(
        reader,
        &mut io::sink(),
        &mut keep,
        &mut project,
        &mut counts,
        false,
    )?;
    let sha256 = format!("{:x}", hash.borrow().clone().finalize());
    metadata["counts"] = json!(counts);
    metadata["byteLength"] = json!(span.1);
    metadata["sha256"] = json!(sha256);
    metadata["chunkCount"] = json!(span.1.div_ceil(CHUNK_BYTES as u64));
    Ok(Document {
        path: path.to_owned(),
        offset: span.0,
        byte_length: span.1,
        sha256,
        metadata,
    })
}

/// Runs on a blocking worker. Archives are exact spans in the owning private
/// run.json; only the numerical input is written to a new immutable file.
pub fn split_run(path: &Path, directory: &Path) -> Result<StagedRun, ApiError> {
    let source_length = std::fs::metadata(path).map_err(|_| storage())?.len();
    if source_length > WORKSPACE_BYTES {
        return Err(storage());
    }
    let spans = root_spans(path)?;
    let mut lineage = Lineage::default();
    let decoded = archive(path, spans["decoded"], &mut lineage, true)?;
    let normalized = archive(path, spans["normalized"], &mut lineage, false)?;
    let legacy: Analysis = serde_json::from_reader(BufReader::with_capacity(
        CHUNK_BYTES,
        span_reader(path, spans["legacy"].0, spans["legacy"].1)?,
    ))
    .map_err(|_| invalid())?;
    if decoded.metadata["schemaVersion"] != "2.0.0"
        || !decoded.metadata["decoder"].is_object()
        || !decoded.metadata["counts"]["messages"].is_u64()
        || !decoded.metadata["counts"]["warnings"].is_u64()
    {
        return Err(invalid());
    }
    let mut metadata = normalized.metadata.clone();
    if metadata["schemaVersion"] != "2.0.0"
        || metadata["sport"] != "running"
        || metadata["session"]["index"] != 0
        || !metadata["summary"].is_object()
    {
        return Err(invalid());
    }
    lineage.references(&metadata)?;
    for key in [
        "samples",
        "laps",
        "timerEvents",
        "sensors",
        "zones",
        "deviceReportedThresholds",
        "extensions",
        "warnings",
        "rr.intervals",
        "rr.reasons",
    ] {
        if !metadata["counts"][key].is_u64() {
            return Err(invalid());
        }
    }
    if !metadata["rrAlignmentEligible"].is_boolean() {
        return Err(invalid());
    }
    let start = timestamp(&metadata["startTime"])?;
    let end = timestamp(&metadata["endTime"])?;
    if end < start {
        return Err(invalid());
    }
    let start_source = &metadata["session"]["sourceReferences"]["startTime"];
    let end_source = &metadata["session"]["sourceReferences"]["endTime"];
    let native_start = lineage.reference(start_source, true)?;
    let native_end = lineage.reference(end_source, true)?;
    if native_start.global != 18
        || native_end.global != 18
        || start_source["fieldNumber"] != 2
        || end_source["fieldNumber"] != 253
        || start_source["messageIndex"] != end_source["messageIndex"]
        || native_start.start_time != Some(start)
        || native_end.timestamp != Some(end)
    {
        return Err(invalid());
    }
    for key in [
        "distanceMeters",
        "timerTimeSeconds",
        "elapsedTimeSeconds",
        "movingTimeSeconds",
        "averageSpeedMps",
        "averagePaceSecondsPerKm",
        "averageHeartRateBpm",
        "averagePowerWatts",
        "averageCadenceStepsPerMinute",
    ] {
        if metadata["summary"].get(key).is_none() {
            return Err(invalid());
        }
        numeric(&metadata["summary"][key], false)?;
    }
    let duration = (end - start).num_microseconds().ok_or_else(invalid)? as f64 / 1_000_000.0;
    let mut validation = Validation {
        lineage: &lineage,
        start,
        end,
        duration,
        indices: BTreeMap::new(),
        next_sample_source: 0,
        rr_all_eligible: true,
        rr_previous_end: None,
        sample_hash: Sha256::new(),
        lap_hash: Sha256::new(),
        event_hash: Sha256::new(),
        metric_samples: 0,
    };
    let analysis_path = directory.join("analysis-input.stage.json");
    let mut output = HashFile::create(&analysis_path, WORKSPACE_BYTES - source_length)?;
    let mut keep = |path: &[String]| !matches!(path,[key] if !matches!(key.as_str(),"schemaVersion"|"startTime"|"endTime"|"sport"|"subtype"|"summary"|"samples"|"laps"|"timerEvents"|"sensors"|"rr"));
    let mut project = |path: &[String], mut value: Value| -> Result<Option<Value>, ApiError> {
        validation.row(path, &value)?;
        // RR provenance is used by the engine. Other repeated source identities
        // and extension metadata are archived, not numerical engine inputs.
        if !matches!(path,[rr,intervals,_] if rr=="rr" && intervals=="intervals") {
            strip_unused(&mut value);
        }
        Ok(Some(value))
    };
    let mut projection_counts = BTreeMap::new();
    walk_document(
        span_reader(&normalized.path, normalized.offset, normalized.byte_length)?,
        &mut output,
        &mut keep,
        &mut project,
        &mut projection_counts,
        true,
    )?;
    if validation.next_sample().is_some() {
        return Err(invalid());
    }
    if metadata["rrAlignmentEligible"] == true
        && (!validation.rr_all_eligible || metadata["counts"]["rr.intervals"] == 0)
    {
        return Err(invalid());
    }
    let mut observation = Sha256::new();
    observation.update(OBSERVATION_RULE.as_bytes());
    // Native byte coordinates are provenance, not observation identity. Move
    // them out temporarily so the immutable summary stays exact without copying.
    let summary_source_references = metadata
        .get_mut("summary")
        .and_then(|summary| summary.get_mut("sourceReferences"))
        .map(Value::take);
    for key in ["startTime", "endTime", "summary"] {
        hash_value(&mut observation, &metadata[key])?;
    }
    if let Some(references) = summary_source_references {
        *metadata
            .get_mut("summary")
            .and_then(|summary| summary.get_mut("sourceReferences"))
            .ok_or_else(invalid)? = references;
    }
    observation.update(validation.sample_hash.finalize());
    observation.update(validation.lap_hash.finalize());
    observation.update(validation.event_hash.finalize());
    metadata["observationFingerprint"] = json!(format!("{:x}", observation.finalize()));
    metadata["observationStrong"] = json!(validation.metric_samples >= 2);
    metadata["observationRule"] = json!(OBSERVATION_RULE);
    metadata["projectionVersion"] = json!(PROJECTION_VERSION);
    metadata["normalizedSha256"] = json!(normalized.sha256);
    metadata["fullDocumentBytes"] = json!(normalized.byte_length);
    metadata
        .as_object_mut()
        .ok_or_else(invalid)?
        .remove("session");
    let analysis_input=output.finish(analysis_path,json!({"projectionVersion":PROJECTION_VERSION,"counts":projection_counts,"omittedMetadata":["sourceReferences","extensions","warnings","session","zones","deviceReportedThresholds","unknown-root-fields"],"retainedResolution":"all-original-samples-and-rr"}))?;
    metadata["analysisInputSha256"] = json!(analysis_input.sha256);
    metadata["analysisInputBytes"] = json!(analysis_input.byte_length);
    Ok(StagedRun {
        decoded,
        normalized,
        analysis_input,
        normalized_metadata: metadata,
        legacy,
    })
}

/// Integrity-checked immutable PG document reader. Construct and use ONLY from
/// spawn_blocking: the Tokio handle drives one bounded chunk query at a time.
enum RevisionSource {
    Pool(PgPool),
    Transaction(Rc<RefCell<sqlx::Transaction<'static, sqlx::Postgres>>>),
}
pub struct RevisionReader {
    source: RevisionSource,
    handle: tokio::runtime::Handle,
    owner: String,
    revision: String,
    document: String,
    position: u64,
    chunk_count: u64,
    expected_bytes: u64,
    expected_hash: String,
    bytes: Vec<u8>,
    offset: usize,
    read_bytes: u64,
    hash: Sha256,
    finished: bool,
}
impl RevisionReader {
    pub fn new(
        pool: PgPool,
        owner: String,
        revision: String,
        document: &str,
        metadata: &Value,
    ) -> Result<Self, ApiError> {
        Self::from_source(
            RevisionSource::Pool(pool),
            owner,
            revision,
            document,
            metadata,
        )
    }
    /// Reuse the export guard transaction; never acquire another pool connection
    /// while all connections may be held by concurrent owner-lock waiters.
    pub fn new_transaction(
        transaction: Rc<RefCell<sqlx::Transaction<'static, sqlx::Postgres>>>,
        owner: String,
        revision: String,
        document: &str,
        metadata: &Value,
    ) -> Result<Self, ApiError> {
        Self::from_source(
            RevisionSource::Transaction(transaction),
            owner,
            revision,
            document,
            metadata,
        )
    }
    fn from_source(
        source: RevisionSource,
        owner: String,
        revision: String,
        document: &str,
        metadata: &Value,
    ) -> Result<Self, ApiError> {
        if !matches!(document, "archive" | "analysisInput") {
            return Err(integrity());
        }
        let expected_bytes = metadata["byteLength"].as_u64().ok_or_else(integrity)?;
        let chunk_count = metadata["chunkCount"].as_u64().ok_or_else(integrity)?;
        let expected_hash = metadata["sha256"]
            .as_str()
            .filter(|s| {
                s.len() == 64
                    && s.bytes()
                        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            })
            .ok_or_else(integrity)?
            .to_owned();
        if expected_bytes == 0 || chunk_count != expected_bytes.div_ceil(CHUNK_BYTES as u64) {
            return Err(integrity());
        }
        let handle = tokio::runtime::Handle::try_current().map_err(|_| storage())?;
        Ok(Self {
            source,
            handle,
            owner,
            revision,
            document: document.into(),
            position: 0,
            chunk_count,
            expected_bytes,
            expected_hash,
            bytes: Vec::new(),
            offset: 0,
            read_bytes: 0,
            hash: Sha256::new(),
            finished: false,
        })
    }
    fn next_chunk(&mut self) -> io::Result<bool> {
        if self.finished {
            return Ok(false);
        }
        if self.position == self.chunk_count {
            let query=sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM runs_revision_chunks WHERE owner_id=$1 AND revision_id=$2 AND document=$3 AND position >= $4)").bind(&self.owner).bind(&self.revision).bind(&self.document).bind(self.position as i64);
            let extra = match &self.source {
                RevisionSource::Pool(pool) => self.handle.block_on(query.fetch_one(pool)),
                RevisionSource::Transaction(transaction) => {
                    let mut transaction = transaction
                        .try_borrow_mut()
                        .map_err(|_| io::Error::other("RUNS_STORAGE_FAILED"))?;
                    self.handle.block_on(query.fetch_one(&mut **transaction))
                }
            }
            .map_err(|_| io::Error::new(io::ErrorKind::ConnectionAborted, "RUNS_STORAGE_FAILED"))?;
            if extra
                || self.read_bytes != self.expected_bytes
                || format!("{:x}", self.hash.clone().finalize()) != self.expected_hash
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "RUNS_DOCUMENT_INTEGRITY_FAILED",
                ));
            }
            self.finished = true;
            return Ok(false);
        }
        let query=sqlx::query("SELECT position,payload FROM runs_revision_chunks WHERE owner_id=$1 AND revision_id=$2 AND document=$3 AND position=$4").bind(&self.owner).bind(&self.revision).bind(&self.document).bind(self.position as i64);
        let row = match &self.source {
            RevisionSource::Pool(pool) => self.handle.block_on(query.fetch_optional(pool)),
            RevisionSource::Transaction(transaction) => {
                let mut transaction = transaction
                    .try_borrow_mut()
                    .map_err(|_| io::Error::other("RUNS_STORAGE_FAILED"))?;
                self.handle
                    .block_on(query.fetch_optional(&mut **transaction))
            }
        }
        .map_err(|_| io::Error::new(io::ErrorKind::ConnectionAborted, "RUNS_STORAGE_FAILED"))?
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "RUNS_DOCUMENT_INTEGRITY_FAILED")
        })?;
        let position: i64 = row.try_get("position").map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "RUNS_DOCUMENT_INTEGRITY_FAILED")
        })?;
        let bytes: Vec<u8> = row.try_get("payload").map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "RUNS_DOCUMENT_INTEGRITY_FAILED")
        })?;
        let remaining = self
            .expected_bytes
            .checked_sub(self.read_bytes)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "RUNS_DOCUMENT_INTEGRITY_FAILED")
            })?;
        let expected = remaining.min(CHUNK_BYTES as u64) as usize;
        if position < 0
            || position as u64 != self.position
            || bytes.len() != expected
            || bytes.is_empty()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "RUNS_DOCUMENT_INTEGRITY_FAILED",
            ));
        }
        self.hash.update(&bytes);
        self.read_bytes += bytes.len() as u64;
        self.bytes = bytes;
        self.offset = 0;
        self.position += 1;
        Ok(true)
    }
}
impl Read for RevisionReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        if self.offset == self.bytes.len() && !self.next_chunk()? {
            return Ok(0);
        }
        let n = out.len().min(self.bytes.len() - self.offset);
        out[..n].copy_from_slice(&self.bytes[self.offset..self.offset + n]);
        self.offset += n;
        Ok(n)
    }
}
