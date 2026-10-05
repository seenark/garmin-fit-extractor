//! Bounded JSON stage transport. Only one native record or normalized row is resident.
use std::{cell::{Cell,RefCell}, collections::{BTreeMap, BTreeSet}, fs::{File, OpenOptions}, io::{self, BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write}, path::{Path, PathBuf}, rc::Rc};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor}};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use crate::{error::ApiError, model::Analysis};

pub const CHUNK_BYTES: usize = 65_536;
pub const PROJECTION_VERSION: &str = "runs-numerical-input-1.0.0";
// A FIT record has at most 256 native fields. This bounds JSON expansion of one
// record, not the document, its number of rows, or any streamed unknown array.
const RECORD_BYTES: usize = 2 * 1024 * 1024;
fn invalid() -> ApiError { ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, "INVALID_RUN_DOCUMENT", "Run document failed semantic validation.") }
fn storage() -> ApiError { ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "RUNS_STORAGE_FAILED", "Run storage operation failed.") }
fn integrity() -> ApiError { ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "RUNS_DOCUMENT_INTEGRITY_FAILED", "Stored run document failed integrity verification.") }

#[derive(Debug)]
pub struct Document { pub path: PathBuf, pub offset:u64, pub byte_length: u64, pub sha256: String, pub metadata: Value }
#[derive(Debug)]
pub struct StagedRun { pub decoded: Document, pub normalized: Document, pub analysis_input: Document, pub normalized_metadata: Value, pub legacy: Analysis }

struct HashFile { file: BufWriter<File>, hash: Sha256, length: u64, limit:u64 }
impl HashFile {
    fn create(path: &Path, limit:u64) -> Result<Self, ApiError> {
        let mut options = OpenOptions::new(); options.write(true).create_new(true);
        #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
        Ok(Self { file: BufWriter::with_capacity(CHUNK_BYTES, options.open(path).map_err(|_| storage())?), hash: Sha256::new(), length: 0, limit })
    }
    fn finish(mut self, path: PathBuf, mut metadata: Value) -> Result<Document, ApiError> {
        self.file.flush().map_err(|_| storage())?;
        self.file.get_ref().sync_all().map_err(|_| storage())?;
        let sha256 = format!("{:x}", self.hash.finalize());
        metadata["byteLength"] = json!(self.length); metadata["sha256"] = json!(sha256);
        metadata["chunkCount"] = json!(self.length.div_ceil(CHUNK_BYTES as u64));
        Ok(Document { path, offset:0, byte_length: self.length, sha256, metadata })
    }
}
impl Write for HashFile {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> { if bytes.len() as u64>self.limit.saturating_sub(self.length){return Err(io::Error::other("RUNS_SPOOL_LIMIT"));}let n = self.file.write(bytes)?; self.hash.update(&bytes[..n]); self.length += n as u64; Ok(n) }
    fn flush(&mut self) -> io::Result<()> { self.file.flush() }
}

// serde's IoRead consumes one byte at a time. Resetting this shared allowance
// before an atomic value bounds its allocation before Value can grow.
struct BudgetRead<R> { inner: R, allowance: Rc<Cell<usize>> }
impl<R: Read> Read for BudgetRead<R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if bytes.is_empty() { return Ok(0); }
        let remaining = self.allowance.get();
        if remaining == 0 { return Err(io::Error::other("RUNS_RECORD_LIMIT")); }
        let size = bytes.len().min(remaining); let n = self.inner.read(&mut bytes[..size])?;
        self.allowance.set(remaining - n); Ok(n)
    }
}

type Project<'a> = dyn FnMut(&[String], Value) -> Result<Option<Value>, ApiError> + 'a;
type Keep<'a> = dyn FnMut(&[String]) -> bool + 'a;
fn known_array(path: &[String]) -> bool {
    matches!(path, [key] if matches!(key.as_str(), "messages"|"warnings"|"samples"|"laps"|"timerEvents"|"sensors"|"zones"|"deviceReportedThresholds"|"extensions"))
        || matches!(path, [root,key] if root == "rr" && matches!(key.as_str(),"intervals"|"reasons"))
}
fn atomic(path: &[String]) -> bool {
    if path.last().is_some_and(|p| p == "*") { return known_array(&path[..path.len()-1]); }
    matches!(path, [key] if matches!(key.as_str(), "schemaVersion"|"decoder"|"session"|"startTime"|"endTime"|"sport"|"subtype"|"summary"|"sourceRevision"|"decodedRevisionId"|"projectionVersion"|"normalizedDocumentHash"))
        || matches!(path, [root,key] if root == "rr" && key == "alignmentEligible")
}
struct Walk<'a> { out: &'a mut dyn Write, emit:bool, keep: &'a mut Keep<'a>, project: &'a mut Project<'a>, allowance: Rc<Cell<usize>>, counts: &'a mut BTreeMap<String,u64> }
struct Seed<'a,'b> { walk: &'a mut Walk<'b>, path: Vec<String>, separator: &'static [u8] }
impl<'de> DeserializeSeed<'de> for Seed<'_,'_> {
    type Value = bool;
    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<bool,D::Error> {
        self.walk.allowance.set(RECORD_BYTES);
        if atomic(&self.path) {
            let value = Value::deserialize(deserializer)?;
            let value = (self.walk.project)(&self.path, value).map_err(|_| de::Error::custom("INVALID_RUN_DOCUMENT"))?;
            if let Some(value) = value {
                self.walk.out.write_all(self.separator).map_err(de::Error::custom)?;
                if self.walk.emit{serde_json::to_writer(&mut self.walk.out, &value).map_err(de::Error::custom)?;}
                return Ok(true);
            }
            return Ok(false);
        }
        deserializer.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Seed<'_,'_> {
    type Value = bool;
    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { f.write_str("a streamed JSON value") }
    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<bool,M::Error> {
        if known_array(&self.path) { return Err(de::Error::custom("INVALID_RUN_DOCUMENT")); }
        self.walk.out.write_all(self.separator).map_err(de::Error::custom)?;
        self.walk.out.write_all(b"{").map_err(de::Error::custom)?;
        let mut first = true; let mut seen = BTreeSet::new();
        loop {
            self.walk.allowance.set(RECORD_BYTES);
            let Some(key) = map.next_key::<String>()? else { break; };
            if !seen.insert(key.clone()) { return Err(de::Error::custom("INVALID_RUN_DOCUMENT")); }
            let mut path = self.path.clone(); path.push(key.clone());
            if !(self.walk.keep)(&path) {
                map.next_value_seed(Discard { allowance: self.walk.allowance.clone() })?;
                continue;
            }
            // Object fields cannot disappear after their name has been emitted;
            // use keep for field omission, project(None) only for array items.
            if !first { self.walk.out.write_all(b",").map_err(de::Error::custom)?; } first = false;
            if self.walk.emit{serde_json::to_writer(&mut self.walk.out, &key).map_err(de::Error::custom)?;}
            self.walk.out.write_all(b":").map_err(de::Error::custom)?;
            let emitted = map.next_value_seed(Seed { walk: self.walk, path, separator: b"" })?;
            if !emitted { return Err(de::Error::custom("INVALID_RUN_DOCUMENT")); }
        }
        self.walk.out.write_all(b"}").map_err(de::Error::custom)?; Ok(true)
    }
    fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<bool,S::Error> {
        self.walk.out.write_all(self.separator).map_err(de::Error::custom)?;
        self.walk.out.write_all(b"[").map_err(de::Error::custom)?;
        let mut path = self.path.clone(); path.push("*".into());
        let mut first = true; let mut count = 0u64;
        while let Some(emitted) = seq.next_element_seed(Seed { walk: self.walk, path: path.clone(), separator: if first { b"" } else { b"," } })? {
            count += 1; if emitted { first = false; }
        }
        if known_array(&self.path) { self.walk.counts.insert(self.path.join("."), count); }
        self.walk.out.write_all(b"]").map_err(de::Error::custom)?; Ok(true)
    }
    fn visit_bool<E: de::Error>(self, value: bool) -> Result<bool,E> { self.scalar(json!(value)) }
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<bool,E> { self.scalar(json!(value)) }
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<bool,E> { self.scalar(json!(value)) }
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<bool,E> { if !value.is_finite() { return Err(E::custom("INVALID_RUN_DOCUMENT")); } self.scalar(json!(value)) }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<bool,E> { self.scalar(json!(value)) }
    fn visit_string<E: de::Error>(self, value: String) -> Result<bool,E> { self.scalar(Value::String(value)) }
    fn visit_unit<E: de::Error>(self) -> Result<bool,E> { self.scalar(Value::Null) }
}
impl Seed<'_,'_> {
    fn scalar<E: de::Error>(self, value: Value) -> Result<bool,E> {
        if known_array(&self.path) { return Err(E::custom("INVALID_RUN_DOCUMENT")); }
        let value = (self.walk.project)(&self.path,value).map_err(|_| E::custom("INVALID_RUN_DOCUMENT"))?;
        if let Some(value) = value {
            self.walk.out.write_all(self.separator).map_err(E::custom)?;
            if self.walk.emit{serde_json::to_writer(&mut self.walk.out,&value).map_err(E::custom)?;} Ok(true)
        } else { Ok(false) }
    }
}
// Skipping uses the same incremental visitor: IgnoredAny can allocate a giant
// escaped string, and one byte allowance for an entire array would cap rows.
struct Discard { allowance: Rc<Cell<usize>> }
impl<'de> DeserializeSeed<'de> for Discard {
    type Value = ();
    fn deserialize<D: de::Deserializer<'de>>(self, d:D)->Result<(),D::Error> { self.allowance.set(RECORD_BYTES); d.deserialize_any(self) }
}
impl<'de> Visitor<'de> for Discard {
    type Value=();
    fn expecting(&self,f:&mut std::fmt::Formatter)->std::fmt::Result {f.write_str("JSON")}
    fn visit_map<M:MapAccess<'de>>(self,mut map:M)->Result<(),M::Error>{loop{self.allowance.set(RECORD_BYTES);if map.next_key::<String>()?.is_none(){break;}map.next_value_seed(Discard{allowance:self.allowance.clone()})?;}Ok(())}
    fn visit_seq<S:SeqAccess<'de>>(self,mut seq:S)->Result<(),S::Error>{while seq.next_element_seed(Discard{allowance:self.allowance.clone()})?.is_some(){}Ok(())}
    fn visit_bool<E:de::Error>(self,_:bool)->Result<(),E>{Ok(())}
    fn visit_i64<E:de::Error>(self,_:i64)->Result<(),E>{Ok(())}
    fn visit_u64<E:de::Error>(self,_:u64)->Result<(),E>{Ok(())}
    fn visit_f64<E:de::Error>(self,_:f64)->Result<(),E>{Ok(())}
    fn visit_str<E:de::Error>(self,_:&str)->Result<(),E>{Ok(())}
    fn visit_string<E:de::Error>(self,_:String)->Result<(),E>{Ok(())}
    fn visit_unit<E:de::Error>(self)->Result<(),E>{Ok(())}
}

// serde converts visitor write failures into deserializer errors. Remember
// their origin independently so resource exhaustion never blames source JSON.
struct OutputWrite<'a> { inner: &'a mut dyn Write, failed: bool }
impl Write for OutputWrite<'_> {
    fn write(&mut self,bytes:&[u8])->io::Result<usize>{
        let result=self.inner.write(bytes);
        self.failed|=result.as_ref().err().is_some_and(|error|error.kind()!=io::ErrorKind::Interrupted) || matches!(result,Ok(0) if !bytes.is_empty());
        result
    }
    fn flush(&mut self)->io::Result<()>{
        let result=self.inner.flush();self.failed|=result.is_err();result
    }
}

/// Compact JSON output; export may wrap an indenting writer. Field omission is
/// decided before parsing. Atomic known-array items may be removed with None.
pub fn project_document<R:Read,W:Write>(reader:R,writer:&mut W,keep:&mut Keep<'_>,project:&mut Project<'_>)->Result<(),ApiError>{
    let mut counts=BTreeMap::new(); walk_document(reader,writer,keep,project,&mut counts,true)
}
fn walk_document<R:Read>(reader:R,writer:&mut dyn Write,keep:&mut Keep<'_>,project:&mut Project<'_>,counts:&mut BTreeMap<String,u64>,emit:bool)->Result<(),ApiError>{
    let allowance=Rc::new(Cell::new(RECORD_BYTES));
    let input=BudgetRead{inner:BufReader::with_capacity(CHUNK_BYTES,reader),allowance:allowance.clone()};
    let mut deserializer=serde_json::Deserializer::from_reader(input);
    let mut output=OutputWrite{inner:writer,failed:false};
    let mut walk=Walk{out:&mut output,emit,keep,project,allowance,counts};
    let result=Seed{walk:&mut walk,path:Vec::new(),separator:b""}.deserialize(&mut deserializer).and_then(|_|deserializer.end());
    result.map_err(|_|if output.failed{storage()}else{invalid()})
}

struct RrField { number:u8, offset:Option<u64>, component:Option<u64>, values:Box<[Option<f64>]> }
#[derive(Default)]
struct Native { global:u16, fields:[u64;4], valid:[u64;4], developers:Vec<(u8,usize,bool)>, timestamp:Option<DateTime<Utc>>, start_time:Option<DateTime<Utc>>, fractional_timestamp:Option<f64>, rr_fields:Option<Box<Vec<RrField>>> }
impl Native {
    fn bit(bitmap:&[u64;4],number:u8)->bool { bitmap[number as usize/64] & (1u64 << (number%64)) != 0 }
    fn rr_value(&self,reference:&Value)->Result<Option<f64>,ApiError>{
        let fields=self.rr_fields.as_ref().ok_or_else(invalid)?;
        let number=reference["fieldNumber"].as_u64().ok_or_else(invalid)?;
        let mut candidates=fields.iter().filter(|field|field.number as u64==number && reference["byteOffset"].as_u64().is_none_or(|offset|field.offset==Some(offset)) && reference["componentParent"].as_u64().is_none_or(|parent|field.component==Some(parent)));
        let field=candidates.next().ok_or_else(invalid)?;
        if candidates.next().is_some(){return Err(invalid());}
        let index=match reference.get("arrayIndex"){Some(value)=>value.as_u64().and_then(|n|usize::try_from(n).ok()).ok_or_else(invalid)?,None=>field.values.len().checked_sub(1).ok_or_else(invalid)?};
        field.values.get(index).copied().ok_or_else(invalid)
    }
}
#[derive(Default)]
struct Lineage { messages:Vec<Native>, developer_values:Vec<Value>, developer_index:BTreeMap<[u8;32],usize> }
impl Lineage {
    fn intern_developer(&mut self,value:&Value)->Result<usize,ApiError>{
        let mut hash=Sha256::new();hash_value(&mut hash,value)?;let digest:[u8;32]=hash.finalize().into();
        if let Some(index)=self.developer_index.get(&digest){if self.developer_values[*index]!=*value{return Err(invalid());}return Ok(*index);}
        let index=self.developer_values.len();self.developer_values.push(value.clone());self.developer_index.insert(digest,index);Ok(index)
    }
    fn message(&mut self,value:&Value)->Result<(),ApiError>{
        if value["index"].as_u64()!=Some(self.messages.len() as u64) || value["localMessageNumber"].as_u64().is_none_or(|n|n>15){return Err(invalid());}
        let global=value["globalMessageNumber"].as_u64().filter(|n|*n<=u16::MAX as u64).ok_or_else(invalid)? as u16;
        let mut native=Native{global,..Native::default()};
        let fields=value["fields"].as_array().ok_or_else(invalid)?;
        if fields.len()>512{return Err(invalid());}
        for field in fields {
            let number=field["fieldNumber"].as_u64().filter(|n|*n<=255).ok_or_else(invalid)? as u8;
            let valid=match field["validity"].as_str(){Some("valid"|"mixed")=>!field["value"].is_null(),Some("invalid")=>false,_=>return Err(invalid())};
            let identity=field.get("developerIdentity").unwrap_or(&Value::Null);
            if !identity.is_null(){
                if identity["developerDataIndex"].as_u64().is_none_or(|n|n>255) || identity["fieldDefinitionNumber"].as_u64().is_none_or(|n|n>255){return Err(invalid());}
                let identity=self.intern_developer(identity)?;
                native.developers.push((number,identity,valid));
            }else{
                let reconstructed=global==20 && number==253 && field["role"]=="reconstructed";
                let expected_unit=match (global,number){(20,6|73)=>Some("m/s"),(20,3)=>Some("bpm"),(20,7)=>Some("watts"),(20,4|53)=>Some("rpm"),(20,2|78|5)=>Some("m"),(20,253) if !reconstructed=>Some("s"),(78,0)|(132,0|9)|(18,253)=>Some("s"),_=>None};
                if let Some(unit)=expected_unit{if field["unit"]!=unit{return Err(invalid());}}
                if (reconstructed || matches!((global,number),(132,253)|(18,2))) && !field["unit"].is_null(){return Err(invalid());}
                native.fields[number as usize/64]|=1u64<<(number%64);
                if valid{native.valid[number as usize/64]|=1u64<<(number%64);}
                if matches!(global,18|20|132) && number==253 && valid { native.timestamp=field["value"].as_str().map(|_|timestamp(&field["value"])).transpose()?; }
                if global==18 && number==2 && valid{native.start_time=Some(timestamp(&field["value"])?);}
                if global==132 && number==0 && valid {native.fractional_timestamp=field["value"].as_f64();}
                if matches!((global,number),(78,0)|(132,9)){
                    let values=match &field["value"]{Value::Array(values)=>values.iter().map(Value::as_f64).collect::<Vec<_>>(),value=>vec![value.as_f64()]};
                    if values.iter().flatten().any(|n|!n.is_finite() || *n<0.0){return Err(invalid());}
                    let reference=&field["sourceReference"];
                    native.rr_fields.get_or_insert_with(||Box::new(Vec::with_capacity(1))).push(RrField{number,offset:reference["byteOffset"].as_u64(),component:reference["componentParent"].as_u64(),values:values.into_boxed_slice()});
                }
            }
            validate_native_reference(&field["sourceReference"],self.messages.len(),global,Some(number),identity)?;
        }
        native.developers.sort_unstable_by_key(|(number,identity,_)|(*number,*identity));
        native.developers.dedup_by(|right,left|if right.0==left.0 && right.1==left.1{left.2|=right.2;true}else{false});
        validate_native_reference(&value["sourceReference"],self.messages.len(),global,None,&Value::Null)?;
        self.messages.push(native); Ok(())
    }
    fn reference(&self,value:&Value,require_valid:bool)->Result<&Native,ApiError>{
        let index=value["messageIndex"].as_u64().and_then(|n|usize::try_from(n).ok()).ok_or_else(invalid)?;
        let message=self.messages.get(index).ok_or_else(invalid)?;
        if value["globalMessageNumber"].as_u64()!=Some(message.global as u64){return Err(invalid());}
        if value.get("fieldNumber").is_none() {if require_valid{return Err(invalid());}return Ok(message);}
        let number=value["fieldNumber"].as_u64().filter(|n|*n<=255).ok_or_else(invalid)? as u8;
        let identity=value.get("developerIdentity").unwrap_or(&Value::Null);
        let exists=if identity.is_null(){Native::bit(if require_valid{&message.valid}else{&message.fields},number)}else{
            let mut hash=Sha256::new();hash_value(&mut hash,identity)?;let digest:[u8;32]=hash.finalize().into();
            self.developer_index.get(&digest).is_some_and(|id|self.developer_values[*id]==*identity && message.developers.binary_search_by_key(&(number,*id),|(number,id,_)|(*number,*id)).ok().is_some_and(|index|!require_valid || message.developers[index].2))
        };
        if !exists{return Err(invalid());}
        if matches!((message.global,number),(78,0)|(132,9)) && value.get("arrayIndex").is_some(){if require_valid && message.rr_value(value)?.is_none(){return Err(invalid());}message.rr_value(value)?;}
        Ok(message)
    }
    fn references(&self,value:&Value)->Result<(),ApiError>{
        match value {
            Value::Object(object)=>{
                if object.contains_key("messageIndex") && object.contains_key("globalMessageNumber") { self.reference(value,false)?; }
                for child in object.values(){self.references(child)?;}
            },
            Value::Array(values)=>for child in values{self.references(child)?;},
            _=>{}
        } Ok(())
    }
}
fn validate_native_reference(value:&Value,index:usize,global:u16,field:Option<u8>,developer:&Value)->Result<(),ApiError>{
    if value["messageIndex"].as_u64()!=Some(index as u64) || value["globalMessageNumber"].as_u64()!=Some(global as u64){return Err(invalid());}
    if let Some(field)=field {if value["fieldNumber"].as_u64()!=Some(field as u64) || value.get("developerIdentity").unwrap_or(&Value::Null)!=developer{return Err(invalid());}}
    Ok(())
}
fn timestamp(value:&Value)->Result<DateTime<Utc>,ApiError>{let text=value.as_str().ok_or_else(invalid)?;if !text.ends_with('Z') && !text.ends_with("+00:00"){return Err(invalid());}DateTime::parse_from_rfc3339(text).map(|t|t.with_timezone(&Utc)).map_err(|_|invalid())}
fn numeric(value:&Value,negative:bool)->Result<(),ApiError>{if !value.is_null() && value.as_f64().is_none_or(|n|!n.is_finite() || (!negative && n<0.0)){return Err(invalid());}Ok(())}
fn bound(value:&Value,duration:f64)->Result<(),ApiError>{numeric(value,false)?;if value.as_f64().is_some_and(|n|n>duration+0.001){return Err(invalid());}Ok(())}
fn strip_unused(value:&mut Value){match value{Value::Object(map)=>{map.remove("sourceReferences");map.remove("sourceReference");map.remove("extensions");for child in map.values_mut(){strip_unused(child);}},Value::Array(array)=>for child in array{strip_unused(child);},_=>{}}}

struct Validation<'a> { lineage:&'a Lineage, start:DateTime<Utc>, end:DateTime<Utc>, duration:f64, indices:BTreeMap<String,u64>, next_sample_source:usize, rr_all_eligible:bool, rr_previous_end:Option<f64>, sample_hash:Sha256, lap_hash:Sha256, event_hash:Sha256, metric_samples:u64 }
impl<'a> Validation<'a>{
    fn next_sample(&mut self)->Option<usize>{
        while let Some(message)=self.lineage.messages.get(self.next_sample_source){
            let index=self.next_sample_source;self.next_sample_source+=1;
            if message.global==20 && message.timestamp.is_none_or(|time|time>=self.start && time<=self.end){return Some(index);}
        }None
    }
    fn row(&mut self,path:&[String],value:&Value)->Result<(),ApiError>{
        self.lineage.references(value)?;
        let kind=if path==["rr","intervals","*"]{"rrIntervals"}else if path.len()==2 && path[1]=="*"{path[0].as_str()}else{return Ok(());};
        if !matches!(kind,"samples"|"laps"|"timerEvents"|"rrIntervals"){return Ok(());}
        let index=self.indices.entry(kind.into()).or_default();
        if value["index"].as_u64()!=Some(*index){return Err(invalid());}*index+=1;
        for key in ["timestamp","startTime","endTime"] {if !value[key].is_null(){let t=timestamp(&value[key])?;if kind!="rrIntervals" && (t<self.start || t>self.end){return Err(invalid());}}}
        for key in ["elapsedSeconds","startElapsedSeconds","endElapsedSeconds"] {if kind=="rrIntervals"{numeric(&value[key],true)?;}else{bound(&value[key],self.duration)?;}}
        if let (Some(from),Some(to))=(value["startElapsedSeconds"].as_f64(),value["endElapsedSeconds"].as_f64()){if to<from{return Err(invalid());}}
        match kind {
            "samples"=>{
                let expected_source=self.next_sample().ok_or_else(invalid)? as u64;
                if value.get("timestamp").is_none() || value.get("elapsedSeconds").is_none(){return Err(invalid());}
                let sources=value["sourceReferences"].as_object().ok_or_else(invalid)?;
                let mut source_index=None;
                for reference in sources.values(){
                    let native=self.lineage.reference(reference,false)?;
                    let index=reference["messageIndex"].as_u64().ok_or_else(invalid)?;
                    if native.global!=20 || source_index.is_some_and(|previous|previous!=index){return Err(invalid());}source_index=Some(index);
                }
                if source_index.is_some_and(|index|index!=expected_source){return Err(invalid());}
                for key in ["speedMps","paceSecondsPerKm","heartRateBpm","powerWatts","cadenceStepsPerMinute","altitudeMeters","distanceMeters"]{
                    if value.get(key).is_none(){return Err(invalid());}numeric(&value[key],key=="altitudeMeters")?;
                    if value[key].is_number(){
                        let source=&value["sourceReferences"][key];let native=self.lineage.reference(source,true)?;
                        let field=source["fieldNumber"].as_u64().ok_or_else(invalid)?;
                        let fields:&[u64]=match key {"speedMps"|"paceSecondsPerKm"=>&[73,6],"heartRateBpm"=>&[3],"powerWatts"=>&[7],"cadenceStepsPerMinute"=>&[4],"altitudeMeters"=>&[78,2],"distanceMeters"=>&[5],_=>&[]};
                        if native.global!=20 || !source["developerIdentity"].is_null() || !fields.contains(&field){return Err(invalid());}
                    }
                }
                if !value["timerRunning"].is_null() && !value["timerRunning"].is_boolean(){return Err(invalid());}
                if value["timestamp"].is_string(){
                    let source=&value["sourceReferences"]["timestamp"];let native=self.lineage.reference(source,true)?;
                    let time=timestamp(&value["timestamp"])?;
                    if native.global!=20 || source["fieldNumber"]!=253 || native.timestamp!=Some(time){return Err(invalid());}
                    let elapsed=(time-self.start).num_microseconds().ok_or_else(invalid)? as f64/1_000_000.0;
                    if value["elapsedSeconds"].as_f64().is_none_or(|v|(v-elapsed).abs()>0.001){return Err(invalid());}
                }else if !value["elapsedSeconds"].is_null(){return Err(invalid());}
                if ["speedMps","heartRateBpm","powerWatts","distanceMeters"].iter().any(|key|value[*key].is_number()){self.metric_samples+=1;}
                for key in ["timestamp","elapsedSeconds","speedMps","heartRateBpm","powerWatts","cadenceStepsPerMinute","distanceMeters","altitudeMeters"]{hash_value(&mut self.sample_hash,&value[key])?;}
            },
            "laps"=>{if !value["summary"].is_object(){return Err(invalid());}for key in ["distanceMeters","timerTimeSeconds","elapsedTimeSeconds","movingTimeSeconds","averageSpeedMps","averagePaceSecondsPerKm","averageHeartRateBpm","averagePowerWatts","averageCadenceStepsPerMinute"]{numeric(&value["summary"][key],false)?;}let mut compact=value.clone();strip_unused(&mut compact);hash_value(&mut self.lap_hash,&compact)?;},
            "timerEvents"=>{let mut compact=value.clone();strip_unused(&mut compact);hash_value(&mut self.event_hash,&compact)?;},
            "rrIntervals"=>self.rr(value)?,_=>{}
        }Ok(())
    }
    fn rr(&mut self,value:&Value)->Result<(),ApiError>{
        for key in ["rrMs","timestamp","elapsedSeconds","startElapsedSeconds","endElapsedSeconds","sourceReference","provenance","timingEligible"]{if value.get(key).is_none(){return Err(invalid());}}
        numeric(&value["rrMs"],false)?;
        let rr=value["rrMs"].as_f64();
        let source=&value["sourceReference"];let native=self.lineage.reference(source,rr.is_some_and(|n|n>0.0))?;
        if !source["developerIdentity"].is_null(){return Err(invalid());}
        let source_field=source["fieldNumber"].as_u64().ok_or_else(invalid)?;
        if !matches!((native.global,source_field),(78,0)|(132,9)) || value["provenance"]["kind"]!="recorded_rr" || value["provenance"]["messageNumber"].as_u64()!=Some(native.global as u64) || value["provenance"]["fieldNumber"].as_u64()!=Some(source_field){return Err(invalid());}
        if native.global==78{
            let recorded=native.rr_value(source)?;
            match (rr,recorded){(Some(rr),Some(recorded)) if (rr/1000.0-recorded).abs()<=0.000001=>{},(None,None)=>{},_=>return Err(invalid())}
        }
        let eligible=value["timingEligible"].as_bool().ok_or_else(invalid)?;
        if !eligible {self.rr_all_eligible=false;self.rr_previous_end=None;}
        let method=value["provenance"]["timingMethod"].as_str().ok_or_else(invalid)?;
        if method=="unanchored" {if eligible || !value["timestamp"].is_null() || !value["elapsedSeconds"].is_null() || !value["startElapsedSeconds"].is_null() || !value["endElapsedSeconds"].is_null() || !value["provenance"]["anchorTimestamp"].is_null() || !value["provenance"]["anchorSourceReferences"].is_null(){return Err(invalid());}return Ok(());}
        if method!="hr_event_counter_anchor" || native.global!=132{return Err(invalid());}
        if eligible && rr.is_none_or(|rr|rr<=0.0){return Err(invalid());}
        let anchor=timestamp(&value["provenance"]["anchorTimestamp"])?;
        let references=&value["provenance"]["anchorSourceReferences"];
        let mut anchor_index=None;
        for (key,field) in [("timestamp",253),("fractionalTimestamp",0),("eventCounter",9)] {
            let reference=&references[key];let proof=self.lineage.reference(reference,true)?;
            if proof.global!=132 || reference["fieldNumber"]!=field || !reference["developerIdentity"].is_null(){return Err(invalid());}
            let index=reference["messageIndex"].as_u64().ok_or_else(invalid)?;
            if anchor_index.is_some_and(|previous|previous!=index){return Err(invalid());}anchor_index=Some(index);
        }
        if anchor_index.is_none_or(|index|source["messageIndex"].as_u64().is_none_or(|source|index>source)){return Err(invalid());}
        let anchor_native=&self.lineage.messages[anchor_index.ok_or_else(invalid)? as usize];
        let base=anchor_native.timestamp.ok_or_else(invalid)?;
        let fraction=anchor_native.fractional_timestamp.ok_or_else(invalid)?;
        if !fraction.is_finite() || fraction<0.0{return Err(invalid());}
        let proved=base.checked_add_signed(chrono::Duration::microseconds((fraction*1_000_000.0).round() as i64)).ok_or_else(invalid)?;
        if (proved-anchor).num_microseconds().ok_or_else(invalid)?.abs()>1000{return Err(invalid());}
        let from=value["startElapsedSeconds"].as_f64();
        let to=value["endElapsedSeconds"].as_f64().ok_or_else(invalid)?;
        match (rr,from){
            (Some(rr),Some(from)) if (to-from-rr/1000.0).abs()<=0.001=>{},
            (None,None) if !eligible=>{},
            _=>return Err(invalid())
        }
        if value["elapsedSeconds"].as_f64().is_none_or(|v|(v-to).abs()>0.001){return Err(invalid());}
        let end=timestamp(&value["timestamp"])?;
        let elapsed=(end-self.start).num_microseconds().ok_or_else(invalid)? as f64/1_000_000.0;
        if (elapsed-to).abs()>0.001{return Err(invalid());}
        let counter=native.rr_value(source)?.ok_or_else(invalid)?;
        let anchor_counter=anchor_native.rr_value(&references["eventCounter"])?.ok_or_else(invalid)?;
        let counter_elapsed=(anchor-self.start).num_microseconds().ok_or_else(invalid)? as f64/1_000_000.0+counter-anchor_counter;
        if (counter_elapsed-to).abs()>0.001{
            // A fresh native timestamp proves an ineligible reset/gap endpoint.
            // It cannot establish counter continuity or make that row eligible.
            let native_counter=native.rr_fields.as_ref().is_some_and(|fields|fields.iter().any(|field|field.number==9 && field.component.is_none() && field.values.len()==1));
            let fresh=native.timestamp.zip(native.fractional_timestamp).and_then(|(timestamp,fraction)|timestamp.checked_add_signed(chrono::Duration::microseconds((fraction*1_000_000.0).round() as i64)));
            let fresh_elapsed=fresh.and_then(|timestamp|(timestamp-self.start).num_microseconds()).map(|micros|micros as f64/1_000_000.0);
            if eligible || !native_counter || fresh_elapsed.is_none_or(|elapsed|(elapsed-to).abs()>0.001){return Err(invalid());}
        }
        if eligible {let from=from.ok_or_else(invalid)?;if from<0.0 || to>self.duration+0.001{return Err(invalid());}if self.rr_previous_end.is_some_and(|last|(last-from).abs()>0.001){self.rr_all_eligible=false;}self.rr_previous_end=Some(to);}
        Ok(())
    }
}
struct HashSink<'a>(&'a mut Sha256);
impl Write for HashSink<'_>{fn write(&mut self,bytes:&[u8])->io::Result<usize>{self.0.update(bytes);Ok(bytes.len())}fn flush(&mut self)->io::Result<()>{Ok(())}}
fn hash_value(hash:&mut Sha256,value:&Value)->Result<(),ApiError>{serde_json::to_writer(HashSink(hash),value).map_err(|_|invalid())?;hash.update([0]);Ok(())}

const WORKSPACE_BYTES:u64=512*1024*1024;
struct Scan { input:BufReader<File>, position:u64 }
impl Scan {
    fn peek(&mut self)->Result<Option<u8>,ApiError>{Ok(self.input.fill_buf().map_err(|_|storage())?.first().copied())}
    fn byte(&mut self)->Result<u8,ApiError>{let value=self.peek()?.ok_or_else(invalid)?;self.input.consume(1);self.position+=1;Ok(value)}
    fn whitespace(&mut self)->Result<(),ApiError>{while self.peek()?.is_some_and(|b|b.is_ascii_whitespace()){self.byte()?;}Ok(())}
    fn value(&mut self)->Result<(u64,u64),ApiError>{
        self.whitespace()?;let start=self.position;let first=self.byte()?;
        match first {
            b'{'|b'['=>{
                let mut stack=vec![if first==b'{'{b'}'}else{b']'}];let mut string=false;let mut escape=false;
                while !stack.is_empty(){
                    let byte=self.byte()?;
                    if string {if escape{escape=false;}else if byte==b'\\'{escape=true;}else if byte==b'"'{string=false;}continue;}
                    match byte {b'"'=>string=true,b'{'=>stack.push(b'}'),b'['=>stack.push(b']'),b'}'|b']'=>if stack.pop()!=Some(byte){return Err(invalid());},_=>{}}
                    if stack.len()>128{return Err(invalid());}
                }
            },
            b'"'=>{let mut escape=false;loop{let byte=self.byte()?;if escape{escape=false;}else if byte==b'\\'{escape=true;}else if byte==b'"'{break;}}},
            _=>{while self.peek()?.is_some_and(|b|!b.is_ascii_whitespace() && b!=b',' && b!=b'}'){self.byte()?;}}
        }
        Ok((start,self.position-start))
    }
}
fn root_spans(path:&Path)->Result<BTreeMap<String,(u64,u64)>,ApiError>{
    let mut scan=Scan{input:BufReader::with_capacity(CHUNK_BYTES,File::open(path).map_err(|_|storage())?),position:0};
    scan.whitespace()?;if scan.byte()?!=b'{'{return Err(invalid());}
    let mut spans=BTreeMap::new();
    loop {
        scan.whitespace()?;if scan.peek()?==Some(b'}'){scan.byte()?;break;}
        let (offset,length)=scan.value()?;if length>RECORD_BYTES as u64{return Err(invalid());}
        let key:String=serde_json::from_reader(span_reader(path,offset,length)?).map_err(|_|invalid())?;
        if !matches!(key.as_str(),"decoded"|"normalized"|"legacy") || spans.contains_key(&key){return Err(invalid());}
        scan.whitespace()?;if scan.byte()?!=b':'{return Err(invalid());}
        spans.insert(key,scan.value()?);
        scan.whitespace()?;match scan.byte()?{b','=>{scan.whitespace()?;if scan.peek()?==Some(b'}'){return Err(invalid());}},b'}'=>break,_=>return Err(invalid())}
    }
    scan.whitespace()?;if scan.peek()?.is_some() || spans.len()!=3{return Err(invalid());}Ok(spans)
}
fn span_reader(path:&Path,offset:u64,length:u64)->Result<io::Take<File>,ApiError>{
    let mut file=File::open(path).map_err(|_|storage())?;
    let end=offset.checked_add(length).ok_or_else(invalid)?;
    if end>file.metadata().map_err(|_|storage())?.len(){return Err(invalid());}
    file.seek(SeekFrom::Start(offset)).map_err(|_|storage())?;Ok(file.take(length))
}
struct HashRead<R>{inner:R,hash:Rc<RefCell<Sha256>>}
impl<R:Read> Read for HashRead<R>{fn read(&mut self,bytes:&mut[u8])->io::Result<usize>{let n=self.inner.read(bytes)?;self.hash.borrow_mut().update(&bytes[..n]);Ok(n)}}
fn archive(path:&Path,span:(u64,u64),lineage:&mut Lineage,decoded:bool)->Result<Document,ApiError>{
    let hash=Rc::new(RefCell::new(Sha256::new()));let reader=HashRead{inner:span_reader(path,span.0,span.1)?,hash:hash.clone()};
    let mut metadata=json!({});let mut counts=BTreeMap::new();
    let mut keep=|_:&[String]|true;
    let mut project=|path:&[String],value:Value|->Result<Option<Value>,ApiError>{
        if decoded && path==["messages","*"]{lineage.message(&value)?;}
        if !decoded{lineage.references(&value)?;}
        if path.len()==1 && atomic(path){metadata[&path[0]]=value.clone();}
        if path==["rr","alignmentEligible"]{metadata["rrAlignmentEligible"]=value.clone();}
        Ok(Some(value))
    };
    // Archives are already immutable raw spans. Validate without serializing
    // every record a second time into a discarded output buffer.
    walk_document(reader,&mut io::sink(),&mut keep,&mut project,&mut counts,false)?;
    let sha256=format!("{:x}",hash.borrow().clone().finalize());
    metadata["counts"]=json!(counts);metadata["byteLength"]=json!(span.1);metadata["sha256"]=json!(sha256);metadata["chunkCount"]=json!(span.1.div_ceil(CHUNK_BYTES as u64));
    Ok(Document{path:path.to_owned(),offset:span.0,byte_length:span.1,sha256,metadata})
}

/// Runs on a blocking worker. Archives are exact spans in the owning private
/// run.json; only the numerical input is written to a new immutable file.
pub fn split_run(path:&Path,directory:&Path)->Result<StagedRun,ApiError>{
    let source_length=std::fs::metadata(path).map_err(|_|storage())?.len();
    if source_length>WORKSPACE_BYTES{return Err(storage());}
    let spans=root_spans(path)?;let mut lineage=Lineage::default();
    let decoded=archive(path,spans["decoded"],&mut lineage,true)?;
    let normalized=archive(path,spans["normalized"],&mut lineage,false)?;
    let legacy:Analysis=serde_json::from_reader(BufReader::with_capacity(CHUNK_BYTES,span_reader(path,spans["legacy"].0,spans["legacy"].1)?)).map_err(|_|invalid())?;
    if decoded.metadata["schemaVersion"]!="2.0.0" || !decoded.metadata["decoder"].is_object() || !decoded.metadata["counts"]["messages"].is_u64() || !decoded.metadata["counts"]["warnings"].is_u64(){return Err(invalid());}
    let mut metadata=normalized.metadata.clone();
    if metadata["schemaVersion"]!="2.0.0" || metadata["sport"]!="running" || metadata["session"]["index"]!=0 || !metadata["summary"].is_object(){return Err(invalid());}
    lineage.references(&metadata)?;
    for key in ["samples","laps","timerEvents","sensors","zones","deviceReportedThresholds","extensions","warnings","rr.intervals","rr.reasons"]{if !metadata["counts"][key].is_u64(){return Err(invalid());}}
    if !metadata["rrAlignmentEligible"].is_boolean(){return Err(invalid());}
    let start=timestamp(&metadata["startTime"])?;let end=timestamp(&metadata["endTime"])?;if end<start{return Err(invalid());}
    let start_source=&metadata["session"]["sourceReferences"]["startTime"];
    let end_source=&metadata["session"]["sourceReferences"]["endTime"];
    let native_start=lineage.reference(start_source,true)?;let native_end=lineage.reference(end_source,true)?;
    if native_start.global!=18 || native_end.global!=18 || start_source["fieldNumber"]!=2 || end_source["fieldNumber"]!=253 || start_source["messageIndex"]!=end_source["messageIndex"] || native_start.start_time!=Some(start) || native_end.timestamp!=Some(end){return Err(invalid());}
    for key in ["distanceMeters","timerTimeSeconds","elapsedTimeSeconds","movingTimeSeconds","averageSpeedMps","averagePaceSecondsPerKm","averageHeartRateBpm","averagePowerWatts","averageCadenceStepsPerMinute"]{if metadata["summary"].get(key).is_none(){return Err(invalid());}numeric(&metadata["summary"][key],false)?;}
    let duration=(end-start).num_microseconds().ok_or_else(invalid)? as f64/1_000_000.0;
    let mut validation=Validation{lineage:&lineage,start,end,duration,indices:BTreeMap::new(),next_sample_source:0,rr_all_eligible:true,rr_previous_end:None,sample_hash:Sha256::new(),lap_hash:Sha256::new(),event_hash:Sha256::new(),metric_samples:0};
    let analysis_path=directory.join("analysis-input.stage.json");let mut output=HashFile::create(&analysis_path,WORKSPACE_BYTES-source_length)?;
    let mut keep=|path:&[String]|{!matches!(path,[key] if !matches!(key.as_str(),"schemaVersion"|"startTime"|"endTime"|"sport"|"subtype"|"summary"|"samples"|"laps"|"timerEvents"|"sensors"|"rr"))};
    let mut project=|path:&[String],mut value:Value|->Result<Option<Value>,ApiError>{
        validation.row(path,&value)?;
        // RR provenance is used by the engine. Other repeated source identities
        // and extension metadata are archived, not numerical engine inputs.
        if !matches!(path,[rr,intervals,_] if rr=="rr" && intervals=="intervals"){strip_unused(&mut value);}
        Ok(Some(value))
    };
    let mut projection_counts=BTreeMap::new();
    walk_document(span_reader(&normalized.path,normalized.offset,normalized.byte_length)?,&mut output,&mut keep,&mut project,&mut projection_counts,true)?;
    if validation.next_sample().is_some(){return Err(invalid());}
    if metadata["rrAlignmentEligible"]==true && (!validation.rr_all_eligible || metadata["counts"]["rr.intervals"]==0){return Err(invalid());}
    let mut observation=Sha256::new();observation.update(b"exact-stream-v2");
    for key in ["startTime","endTime","summary"]{hash_value(&mut observation,&metadata[key])?;}
    observation.update(validation.sample_hash.finalize());observation.update(validation.lap_hash.finalize());observation.update(validation.event_hash.finalize());
    metadata["observationFingerprint"]=json!(format!("{:x}",observation.finalize()));metadata["observationStrong"]=json!(validation.metric_samples>=2);metadata["observationRule"]=json!("exact-stream-v2");
    metadata["projectionVersion"]=json!(PROJECTION_VERSION);metadata["normalizedSha256"]=json!(normalized.sha256);metadata["fullDocumentBytes"]=json!(normalized.byte_length);
    metadata.as_object_mut().ok_or_else(invalid)?.remove("session");
    let analysis_input=output.finish(analysis_path,json!({"projectionVersion":PROJECTION_VERSION,"counts":projection_counts,"omittedMetadata":["sourceReferences","extensions","warnings","session","zones","deviceReportedThresholds","unknown-root-fields"],"retainedResolution":"all-original-samples-and-rr"}))?;
    metadata["analysisInputSha256"]=json!(analysis_input.sha256);metadata["analysisInputBytes"]=json!(analysis_input.byte_length);
    Ok(StagedRun{decoded,normalized,analysis_input,normalized_metadata:metadata,legacy})
}

/// Integrity-checked immutable PG document reader. Construct and use ONLY from
/// spawn_blocking: the Tokio handle drives one bounded chunk query at a time.
enum RevisionSource { Pool(PgPool), Transaction(Rc<RefCell<sqlx::Transaction<'static,sqlx::Postgres>>>) }
pub struct RevisionReader { source:RevisionSource, handle:tokio::runtime::Handle, owner:String, revision:String, document:String, position:u64, chunk_count:u64, expected_bytes:u64, expected_hash:String, bytes:Vec<u8>, offset:usize, read_bytes:u64, hash:Sha256, finished:bool }
impl RevisionReader {
    pub fn new(pool:PgPool,owner:String,revision:String,document:&str,metadata:&Value)->Result<Self,ApiError>{
        Self::from_source(RevisionSource::Pool(pool),owner,revision,document,metadata)
    }
    /// Reuse the export guard transaction; never acquire another pool connection
    /// while all connections may be held by concurrent owner-lock waiters.
    pub fn new_transaction(transaction:Rc<RefCell<sqlx::Transaction<'static,sqlx::Postgres>>>,owner:String,revision:String,document:&str,metadata:&Value)->Result<Self,ApiError>{
        Self::from_source(RevisionSource::Transaction(transaction),owner,revision,document,metadata)
    }
    fn from_source(source:RevisionSource,owner:String,revision:String,document:&str,metadata:&Value)->Result<Self,ApiError>{
        if !matches!(document,"archive"|"analysisInput"){return Err(integrity());}
        let expected_bytes=metadata["byteLength"].as_u64().ok_or_else(integrity)?;
        let chunk_count=metadata["chunkCount"].as_u64().ok_or_else(integrity)?;
        let expected_hash=metadata["sha256"].as_str().filter(|s|s.len()==64 && s.bytes().all(|b|b.is_ascii_hexdigit() && !b.is_ascii_uppercase())).ok_or_else(integrity)?.to_owned();
        if expected_bytes==0 || chunk_count!=expected_bytes.div_ceil(CHUNK_BYTES as u64){return Err(integrity());}
        let handle=tokio::runtime::Handle::try_current().map_err(|_|storage())?;
        Ok(Self{source,handle,owner,revision,document:document.into(),position:0,chunk_count,expected_bytes,expected_hash,bytes:Vec::new(),offset:0,read_bytes:0,hash:Sha256::new(),finished:false})
    }
    fn next_chunk(&mut self)->io::Result<bool>{
        if self.finished{return Ok(false);}
        if self.position==self.chunk_count {
            let query=sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM runs_revision_chunks WHERE owner_id=$1 AND revision_id=$2 AND document=$3 AND position >= $4)").bind(&self.owner).bind(&self.revision).bind(&self.document).bind(self.position as i64);
            let extra=match &self.source{
                RevisionSource::Pool(pool)=>self.handle.block_on(query.fetch_one(pool)),
                RevisionSource::Transaction(transaction)=>{let mut transaction=transaction.try_borrow_mut().map_err(|_|io::Error::other("RUNS_STORAGE_FAILED"))?;self.handle.block_on(query.fetch_one(&mut **transaction))}
            }.map_err(|_|io::Error::new(io::ErrorKind::ConnectionAborted,"RUNS_STORAGE_FAILED"))?;
            if extra || self.read_bytes!=self.expected_bytes || format!("{:x}",self.hash.clone().finalize())!=self.expected_hash{return Err(io::Error::other("RUNS_DOCUMENT_INTEGRITY_FAILED"));}
            self.finished=true;return Ok(false);
        }
        let query=sqlx::query("SELECT position,payload FROM runs_revision_chunks WHERE owner_id=$1 AND revision_id=$2 AND document=$3 AND position=$4").bind(&self.owner).bind(&self.revision).bind(&self.document).bind(self.position as i64);
        let row=match &self.source{
            RevisionSource::Pool(pool)=>self.handle.block_on(query.fetch_optional(pool)),
            RevisionSource::Transaction(transaction)=>{let mut transaction=transaction.try_borrow_mut().map_err(|_|io::Error::other("RUNS_STORAGE_FAILED"))?;self.handle.block_on(query.fetch_optional(&mut **transaction))}
        }.map_err(|_|io::Error::new(io::ErrorKind::ConnectionAborted,"RUNS_STORAGE_FAILED"))?.ok_or_else(||io::Error::other("RUNS_DOCUMENT_INTEGRITY_FAILED"))?;
        let position:i64=row.try_get("position").map_err(|_|io::Error::other("RUNS_DOCUMENT_INTEGRITY_FAILED"))?;
        let bytes:Vec<u8>=row.try_get("payload").map_err(|_|io::Error::other("RUNS_DOCUMENT_INTEGRITY_FAILED"))?;
        let remaining=self.expected_bytes.checked_sub(self.read_bytes).ok_or_else(||io::Error::other("RUNS_DOCUMENT_INTEGRITY_FAILED"))?;
        let expected=remaining.min(CHUNK_BYTES as u64) as usize;
        if position<0 || position as u64!=self.position || bytes.len()!=expected || bytes.is_empty(){return Err(io::Error::other("RUNS_DOCUMENT_INTEGRITY_FAILED"));}
        self.hash.update(&bytes);self.read_bytes+=bytes.len() as u64;self.bytes=bytes;self.offset=0;self.position+=1;Ok(true)
    }
}
impl Read for RevisionReader {
    fn read(&mut self,out:&mut [u8])->io::Result<usize>{
        if out.is_empty(){return Ok(0);}
        if self.offset==self.bytes.len() && !self.next_chunk()?{return Ok(0);}
        let n=out.len().min(self.bytes.len()-self.offset);out[..n].copy_from_slice(&self.bytes[self.offset..self.offset+n]);self.offset+=n;Ok(n)
    }
}
