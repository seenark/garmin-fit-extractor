use super::archive::{self, Frame};
use crate::error::FitError;
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{
    Deserialize, Serialize,
    de::{DeserializeOwned, DeserializeSeed, SeqAccess, Visitor},
};
use serde_json::{Map, Value, json};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::{
    cell::Cell,
    fs::{self, File, OpenOptions},
    io::{self, BufReader, BufWriter, Write},
    marker::PhantomData,
    path::PathBuf,
    rc::Rc,
};

const FIT_EPOCH: f64 = 631_065_600.0;
const SUMMARY_METRICS: &[(&str, &str)] = &[
    ("averageSpeedMps", "speedMps"),
    ("averageHeartRateBpm", "heartRateBpm"),
    ("averagePowerWatts", "powerWatts"),
    ("averageCadenceStepsPerMinute", "cadenceStepsPerMinute"),
];
fn spool_error() -> FitError {
    FitError::ProcessingFailed {
        code: "DECODE_SPOOL_FAILED",
        reason: "Decoder output could not be written",
    }
}
const MAX_DOCUMENT_BYTES: usize = 512 * 1024 * 1024;
fn resource_error() -> FitError {
    FitError::ProcessingFailed {
        code: "DECODE_RESOURCE_LIMIT",
        reason: "Decoder resource limit exceeded",
    }
}
fn io_error(error: io::Error) -> FitError {
    if error.kind() == io::ErrorKind::OutOfMemory {
        resource_error()
    } else {
        spool_error()
    }
}
fn emit<T: Serialize + ?Sized>(out: &mut dyn Write, value: &T) -> Result<(), FitError> {
    serde_json::to_writer(out, value).map_err(|error| {
        if error.io_error_kind() == Some(io::ErrorKind::OutOfMemory) {
            resource_error()
        } else {
            spool_error()
        }
    })
}
fn bytes(out: &mut dyn Write, value: &[u8]) -> Result<(), FitError> {
    out.write_all(value).map_err(io_error)
}

struct LimitedWriter<W> {
    writer: W,
    used: Rc<Cell<usize>>,
}
impl<W: Write> Write for LimitedWriter<W> {
    fn write(&mut self, value: &[u8]) -> io::Result<usize> {
        let used = self.used.get();
        if value.len() > MAX_DOCUMENT_BYTES - used {
            return Err(io::Error::new(
                io::ErrorKind::OutOfMemory,
                "Decoder resource limit exceeded",
            ));
        }
        let written = self.writer.write(value)?;
        self.used.set(used + written);
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}
struct Workspace {
    path: PathBuf,
    used: Rc<Cell<usize>>,
}
impl Workspace {
    fn new(used: Rc<Cell<usize>>) -> Result<Self, FitError> {
        for _ in 0..16 {
            let path = std::env::temp_dir().join(format!("fit-run-{}", uuid::Uuid::new_v4()));
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(Self { path, used }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(_) => return Err(spool_error()),
            }
        }
        Err(spool_error())
    }
    fn spool(&self, name: &'static str) -> Result<Spool, FitError> {
        let path = self.path.join(name);
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .map_err(|_| spool_error())?;
        let mut result = Spool {
            path,
            writer: BufWriter::new(LimitedWriter {
                writer: file,
                used: Rc::clone(&self.used),
            }),
            count: 0,
            closed: false,
        };
        bytes(&mut result.writer, b"[")?;
        Ok(result)
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
struct Spool {
    path: PathBuf,
    writer: BufWriter<LimitedWriter<File>>,
    count: usize,
    closed: bool,
}
impl Spool {
    fn push<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), FitError> {
        if self.count != 0 {
            bytes(&mut self.writer, b",")?;
        }
        emit(&mut self.writer, value)?;
        self.count += 1;
        Ok(())
    }
    fn finish(&mut self) -> Result<(), FitError> {
        if !self.closed {
            bytes(&mut self.writer, b"]")?;
            self.closed = true;
        }
        self.writer.flush().map_err(io_error)
    }
    fn copy(&mut self, out: &mut dyn Write) -> Result<(), FitError> {
        self.finish()?;
        let mut file = BufReader::new(File::open(&self.path).map_err(|_| spool_error())?);
        io::copy(&mut file, out).map_err(io_error)?;
        Ok(())
    }
    fn visit<T: DeserializeOwned, F: FnMut(T) -> Result<(), FitError>>(
        &mut self,
        callback: &mut F,
    ) -> Result<(), FitError> {
        self.finish()?;
        let input = BufReader::new(File::open(&self.path).map_err(|_| spool_error())?);
        let mut deserializer = serde_json::Deserializer::from_reader(input);
        let mut callback_error = None;
        let result = ArrayVisitor::<T, F> {
            callback,
            marker: PhantomData,
            error: &mut callback_error,
        }
        .deserialize(&mut deserializer);
        if let Some(error) = callback_error {
            return Err(error);
        }
        result.map_err(|_| spool_error())?;
        deserializer.end().map_err(|_| spool_error())
    }
}
struct ArrayVisitor<'a, T, F> {
    callback: &'a mut F,
    marker: PhantomData<T>,
    error: &'a mut Option<FitError>,
}
impl<'de, T: DeserializeOwned, F: FnMut(T) -> Result<(), FitError>> DeserializeSeed<'de>
    for ArrayVisitor<'_, T, F>
{
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_seq(self)
    }
}
impl<'de, T: DeserializeOwned, F: FnMut(T) -> Result<(), FitError>> Visitor<'de>
    for ArrayVisitor<'_, T, F>
{
    type Value = ();
    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("spooled JSON array")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while let Some(value) = seq.next_element::<T>()? {
            if let Err(error) = (self.callback)(value) {
                *self.error = Some(error);
                return Err(serde::de::Error::custom("spool output failed"));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceReference {
    message_index: usize,
    global_message_number: u64,
    field_number: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    component_parent: Option<u64>,
    byte_offset: usize,
    byte_length: usize,
}
impl SourceReference {
    fn field(f: &Value) -> Option<Self> {
        let reference = &f["sourceReference"];
        Some(Self {
            message_index: usize::try_from(reference["messageIndex"].as_u64()?).ok()?,
            global_message_number: reference["globalMessageNumber"].as_u64()?,
            field_number: reference["fieldNumber"].as_u64()?,
            component_parent: reference["componentParent"].as_u64(),
            byte_offset: usize::try_from(reference["byteOffset"].as_u64()?).ok()?,
            byte_length: usize::try_from(reference["byteLength"].as_u64()?).ok()?,
        })
    }
}
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct SampleReferences {
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp: Option<SourceReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    speed_mps: Option<SourceReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pace_seconds_per_km: Option<SourceReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    heart_rate_bpm: Option<SourceReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    power_watts: Option<SourceReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cadence_steps_per_minute: Option<SourceReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fractional_cadence_cycles_per_minute: Option<SourceReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    altitude_meters: Option<SourceReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    distance_meters: Option<SourceReference>,
}
struct Sample {
    time: Option<f64>,
    speed: Option<f64>,
    hr: Option<f64>,
    power: Option<f64>,
    cadence: Option<f64>,
    altitude: Option<f64>,
    distance: Option<f64>,
    refs: SampleReferences,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SampleView<'a> {
    index: usize,
    timestamp: Option<String>,
    elapsed_seconds: Option<f64>,
    speed_mps: Option<f64>,
    pace_seconds_per_km: Option<f64>,
    heart_rate_bpm: Option<f64>,
    power_watts: Option<f64>,
    cadence_steps_per_minute: Option<f64>,
    altitude_meters: Option<f64>,
    distance_meters: Option<f64>,
    source_references: &'a SampleReferences,
    timer_running: Option<bool>,
}
impl Sample {
    fn new(message: &Value) -> Self {
        let (speed, speed_ref) = metric(message, &[73, 6], false);
        let (altitude, altitude_ref) = metric(message, &[78, 2], false);
        let (hr, hr_ref) = metric(message, &[3], true);
        let (power, power_ref) = metric(message, &[7], true);
        let (distance, distance_ref) = metric(message, &[5], false);
        let (cadence, cadence_ref) = metric(message, &[4], false);
        let fractional = number(message, 53);
        Self {
            time: field_time(message, 253),
            speed,
            hr,
            power,
            cadence: cadence.map(|n| 2.0 * (n + fractional.unwrap_or(0.0))),
            altitude,
            distance,
            refs: SampleReferences {
                timestamp: field(message, 253).and_then(SourceReference::field),
                speed_mps: speed_ref,
                pace_seconds_per_km: speed_ref,
                heart_rate_bpm: hr_ref,
                power_watts: power_ref,
                cadence_steps_per_minute: cadence_ref,
                fractional_cadence_cycles_per_minute: cadence
                    .zip(fractional)
                    .and_then(|_| field(message, 53))
                    .and_then(SourceReference::field),
                altitude_meters: altitude_ref,
                distance_meters: distance_ref,
            },
        }
    }
    fn metric(&self, name: &str) -> Option<f64> {
        match name {
            "speedMps" => self.speed,
            "heartRateBpm" => self.hr,
            "powerWatts" => self.power,
            "cadenceStepsPerMinute" => self.cadence,
            _ => None,
        }
    }
    fn view<'a>(
        &'a self,
        index: usize,
        start: f64,
        timers: &[(f64, Option<bool>)],
    ) -> SampleView<'a> {
        SampleView {
            index,
            timestamp: self.time.and_then(utc_string),
            elapsed_seconds: self.time.map(|t| t - start),
            speed_mps: self.speed,
            pace_seconds_per_km: pace(self.speed),
            heart_rate_bpm: self.hr,
            power_watts: self.power,
            cadence_steps_per_minute: self.cadence,
            altitude_meters: self.altitude,
            distance_meters: self.distance,
            source_references: &self.refs,
            timer_running: self.time.and_then(|t| timer_state(timers, t)),
        }
    }
}
fn metric(
    message: &Value,
    candidates: &[u64],
    native_only: bool,
) -> (Option<f64>, Option<SourceReference>) {
    let Some(fields) = message["fields"].as_array() else {
        return (None, None);
    };
    for &number in candidates {
        for role in ["native", "expanded"] {
            if native_only && role == "expanded" {
                continue;
            }
            if let Some((f, value)) = fields
                .iter()
                .filter(|f| {
                    f["fieldNumber"] == number
                        && f["developerIdentity"].is_null()
                        && f["role"] == role
                        && valid(f)
                        && native_value(message, f)
                })
                .find_map(|f| {
                    f["value"]
                        .as_f64()
                        .filter(|n| n.is_finite())
                        .map(|n| (f, n))
                })
            {
                return (Some(value), SourceReference::field(f));
            }
        }
    }
    (None, None)
}

pub(super) fn decode_run_to_writer(input: &[u8], out: &mut dyn Write) -> Result<(), FitError> {
    let used = Rc::new(Cell::new(0));
    let workspace = Workspace::new(Rc::clone(&used))?;
    let mut limited = LimitedWriter { writer: out, used };
    let out: &mut dyn Write = &mut limited;
    let mut definitions = workspace.spool("definitions")?;
    let mut extensions = workspace.spool("extensions")?;
    let mut sensors = workspace.spool("sensors")?;
    let mut zones = workspace.spool("zones")?;
    let mut thresholds = workspace.spool("thresholds")?;
    let mut warnings = workspace.spool("warnings")?;
    let mut rr = RrState::new(workspace.spool("rr")?);
    let mut samples = Vec::new();
    let mut legacy = Vec::new();
    let mut file_ids = Vec::new();
    let mut sessions = Vec::new();
    let mut laps = Vec::new();
    let mut events = Vec::new();
    let mut activities = Vec::new();
    let mut contexts = Vec::new();
    bytes(out, b"{\"decoded\":{\"messages\":[")?;
    let mut message_count = 0;
    let metadata = archive::scan(input, &mut |frame| {
        match frame {
            Frame::Definition(value) => definitions.push(&value)?,
            Frame::Message(message, mut raw) => {
                if message_count != 0 {
                    bytes(out, b",")?;
                }
                emit(out, &message)?;
                message_count += 1;
                rr.consume(&message)?;
                match global(&message) {
                    Some(0) => {
                        if !file_ids.is_empty() {
                            return Err(layout_error());
                        }
                        file_ids.push(message.clone());
                    }
                    Some(18) => {
                        if !sessions.is_empty() {
                            return Err(layout_error());
                        }
                        sessions.push(message.clone());
                    }
                    Some(19) => laps.push(message.clone()),
                    Some(20) => samples.push(Sample::new(&message)),
                    Some(21) if enum_number(&message, 0) == Some(0) => events.push(message.clone()),
                    Some(34) => activities.push(message.clone()),
                    Some(2) => contexts.push(message.clone()),
                    _ => {}
                }
                let global = global(&message);
                let fields = message["fields"]
                    .as_array()
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                if global == Some(23)
                    || matches!(global, Some(7 | 8 | 9 | 10 | 12 | 53 | 131 | 216))
                {
                    let entries: Vec<_> = fields.iter().map(|f| extension(&message, f)).collect();
                    let target = if global == Some(23) {
                        &mut sensors
                    } else {
                        &mut zones
                    };
                    let mut row = json!({"index":target.count,"globalMessageNumber":global,"fields":entries,"sourceReference":message["sourceReference"]});
                    if global == Some(23) {
                        row["timestamp"] = json!(field_time(&message, 253).and_then(utc_string));
                    }
                    target.push(&row)?;
                } else {
                    for f in fields.iter().filter(|f| {
                        f["classification"] == "unclassified" || !f["developerIdentity"].is_null()
                    }) {
                        extensions.push(&extension(&message, f))?;
                    }
                }
                let threshold_fields: &[(u64, &str, &str)] = match global {
                    Some(7) => &[
                        (2, "thresholdHeartRateBpm", "bpm"),
                        (3, "functionalThresholdPowerWatts", "W"),
                    ],
                    Some(216) => &[
                        (13, "thresholdHeartRateBpm", "bpm"),
                        (15, "functionalThresholdPowerWatts", "W"),
                    ],
                    _ => &[],
                };
                for &(n, kind, unit) in threshold_fields {
                    if let Some(value) = number(&message, n) {
                        thresholds.push(&json!({"index":thresholds.count,"kind":kind,"value":value,"unit":unit,"timestamp":field_time(&message,253).and_then(utc_string),"provenance":"device_setting","sourceReference":source(field(&message,n).unwrap())}))?;
                    }
                }
                match raw.kind.as_str() {
                    "record" => {
                        raw.fields.retain(|f| {
                            matches!(
                                f.name.as_str(),
                                "timestamp"
                                    | "heart_rate"
                                    | "heartRate"
                                    | "hr"
                                    | "power"
                                    | "power_watts"
                                    | "powerWatts"
                                    | "temperature"
                            )
                        });
                        legacy.push(raw);
                    }
                    "session" | "activity" | "lap" | "hr_zone" | "power_zone" | "time_in_zone"
                    | "zones_target" => legacy.push(raw),
                    _ => {}
                }
            }
        }
        Ok(())
    })?;
    if file_ids.len() != 1 || sessions.len() != 1 {
        return Err(layout_error());
    }
    if enum_number(&file_ids[0], 1) != Some(1) {
        return Err(FitError::UnsupportedRun {
            code: "UNSUPPORTED_MANUFACTURER",
            reason: "Only Garmin activity files are supported",
        });
    }
    if enum_number(&file_ids[0], 0) != Some(4) {
        return Err(layout_error());
    }
    let session = &sessions[0];
    if enum_number(session, 5) != Some(1) {
        return Err(sport_error());
    }
    let subtype = match enum_number(session, 6) {
        Some(0) => "generic",
        Some(1) => "treadmill",
        Some(2) => "street",
        Some(3) => "trail",
        Some(4) => "track",
        _ => return Err(sport_error()),
    };
    let start = field_time(session, 2).ok_or_else(layout_error)?;
    let end = field_time(session, 253).ok_or_else(layout_error)?;
    if end < start {
        return Err(layout_error());
    }
    for warning in metadata["warnings"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[])
    {
        warnings.push(warning)?;
    }
    let mut timer_points = Vec::new();
    let timer_events:Vec<_>=events.iter().enumerate().map(|(index,message)| {
        let time=field_time(message,253);
        let event_type=enum_number(message,1);
        let running=match event_type { Some(0)=>Some(true),Some(1|4|8|9)=>Some(false),_=>None };
        if let Some(t)=time.filter(|t|*t>=start&&*t<=end) { timer_points.push((t,running)); }
        json!({"index":index,"timestamp":time.and_then(utc_string),"elapsedSeconds":time.map(|t|t-start),"event":0,"eventType":event_type,"sourceReferences":references(message,&[("timestamp",253),("event",0),("eventType",1)])})
    }).collect();
    if timer_points.windows(2).any(|p| p[1].0 < p[0].0) {
        warnings.push("TIMER_EVENT_NON_MONOTONIC")?;
    }
    let mut previous = None;
    let mut non_monotonic = false;
    let mut duplicate = false;
    let mut warning_error = None;
    samples.retain(|sample| {
        if warning_error.is_some() { return true; }
        if let Some(t)=sample.time {
            if t<start || t>end {
                if let Err(error)=warnings.push(&json!({"code":"OUT_OF_SESSION_RECORD","sourceReference":sample.refs.timestamp})) { warning_error=Some(error); }
                return false;
            }
            let reason=if previous.is_some_and(|p|t<p) && !non_monotonic { non_monotonic=true; Some("TIMESTAMP_NON_MONOTONIC") }
                else if previous==Some(t) && !duplicate { duplicate=true; Some("TIMESTAMP_DUPLICATE") } else { None };
            if let Some(reason)=reason && let Err(error)=warnings.push(reason) { warning_error=Some(error); }
            previous=Some(t);
        }
        true
    });
    if let Some(error) = warning_error {
        return Err(error);
    }
    let gap = gap_limit(&samples);
    let session_summary = summary(session, &samples, &timer_points, start, end, start, gap);
    let mut previous_lap_end = start;
    let mut normalized_laps = Vec::new();
    for message in &laps {
        let lap_start = field_time(message, 2).ok_or_else(layout_error)?;
        let lap_end = field_time(message, 253).ok_or_else(layout_error)?;
        if lap_start < start || lap_end > end || lap_end < lap_start || lap_start < previous_lap_end
        {
            return Err(layout_error());
        }
        previous_lap_end = lap_end;
        normalized_laps.push(json!({"index":normalized_laps.len(),"startTime":utc_string(lap_start),"endTime":utc_string(lap_end),"startElapsedSeconds":lap_start-start,"endElapsedSeconds":lap_end-start,"summary":summary(message,&samples,&timer_points,lap_start,lap_end,start,gap),"sourceReferences":references(message,&[("startTime",2),("endTime",253)])}));
    }
    let properties = session_properties(session, &activities, &contexts);
    bytes(out, b"]")?;
    for (key, value) in metadata.as_object().ok_or_else(spool_error)? {
        bytes(out, b",")?;
        emit(out, key)?;
        bytes(out, b":")?;
        emit(out, value)?;
    }
    bytes(out, b",\"definitions\":")?;
    definitions.copy(out)?;
    bytes(out, b"},\"normalized\":{")?;
    let head = json!({"schemaVersion":super::runs::NORMALIZED_SCHEMA_VERSION,"session":properties,"startTime":utc_string(start),"endTime":utc_string(end),"sport":"running","subtype":subtype,"summary":session_summary,"laps":normalized_laps,"timerEvents":timer_events});
    let mut first = true;
    for (key, value) in head.as_object().unwrap() {
        if !first {
            bytes(out, b",")?;
        }
        first = false;
        emit(out, key)?;
        bytes(out, b":")?;
        emit(out, value)?;
    }
    bytes(out, b",\"samples\":[")?;
    for (index, sample) in samples.iter().enumerate() {
        if index != 0 {
            bytes(out, b",")?;
        }
        emit(out, &sample.view(index, start, &timer_points))?;
    }
    bytes(out, b"],\"sensors\":")?;
    sensors.copy(out)?;
    bytes(out, b",\"zones\":")?;
    zones.copy(out)?;
    bytes(out, b",\"deviceReportedThresholds\":")?;
    thresholds.copy(out)?;
    bytes(out, b",\"extensions\":")?;
    extensions.copy(out)?;
    bytes(out, b",\"warnings\":")?;
    warnings.copy(out)?;
    bytes(out, b",\"rr\":")?;
    rr.output(out, start, end)?;
    drop(samples);
    bytes(out, b"},\"legacy\":")?;
    let old = super::normalize::normalize(&legacy, "");
    drop(legacy);
    emit(out, &old)?;
    bytes(out, b"}")
}

fn session_properties(session: &Value, activities: &[Value], contexts: &[Value]) -> Value {
    let mut properties = json!({"index":0,"sourceReferences":references(session,&[("startTime",2),("endTime",253),("sport",5),("subtype",6)])});
    let offsets: Vec<_> = activities
        .iter()
        .filter_map(|m| Some((raw_number(m, 5)? - raw_number(m, 253)?, m)))
        .collect();
    if let Some((offset, activity)) = offsets.first()
        && offsets.iter().all(|(other, _)| other == offset)
        && offset.abs() <= 86400.0
    {
        properties["localUtcOffsetSeconds"] = json!(offset);
        properties["localUtcOffsetSourceReferences"] =
            references(activity, &[("localTimestamp", 5), ("timestamp", 253)]);
    }
    let mut local:Vec<_>=contexts.iter().filter(|m|[0,2,5].iter().any(|n|field(m,*n).is_some())).map(|message| {
        let values=|n|field(message,n).filter(|f|valid(f)).map(|f|f["value"].as_array().cloned().unwrap_or_else(||vec![f["value"].clone()]));
        json!({"activeTimeZone":enum_number(message,0),"timeOffsetsSeconds":values(2),"timeZoneOffsetsHours":values(5),"sourceReferences":references(message,&[("activeTimeZone",0),("timeOffsetsSeconds",2),("timeZoneOffsetsHours",5)])})
    }).collect();
    if local.len() == 1 {
        properties["sourceLocalTimeContext"] = local.pop().unwrap();
    } else if !local.is_empty() {
        properties["sourceLocalTimeContexts"] = json!(local);
    }
    properties
}

fn layout_error() -> FitError {
    FitError::UnsupportedRun {
        code: "UNSUPPORTED_SESSION_LAYOUT",
        reason: "Activity does not contain one unambiguous running session",
    }
}
fn sport_error() -> FitError {
    FitError::UnsupportedRun {
        code: "UNSUPPORTED_SPORT",
        reason: "Only supported running profiles are accepted",
    }
}
fn global(message: &Value) -> Option<u64> {
    message["globalMessageNumber"].as_u64()
}
fn native(f: &Value) -> bool {
    f["developerIdentity"].is_null()
        && matches!(f["role"].as_str(), Some("native" | "reconstructed"))
}
fn valid(f: &Value) -> bool {
    matches!(f["validity"].as_str(), Some("valid" | "mixed"))
}
fn native_value(message: &Value, field: &Value) -> bool {
    global(message).is_some_and(|number| {
        super::raw::native_field_valid(
            number,
            field,
            message["fields"].as_array().map(Vec::as_slice),
        )
    })
}
fn field(message: &Value, n: u64) -> Option<&Value> {
    message["fields"]
        .as_array()?
        .iter()
        .find(|f| f["fieldNumber"].as_u64() == Some(n) && native(f) && native_value(message, f))
}
fn number(message: &Value, n: u64) -> Option<f64> {
    let f = field(message, n)?;
    valid(f)
        .then(|| f["value"].as_f64())
        .flatten()
        .filter(|n| n.is_finite())
}
fn raw_number(message: &Value, n: u64) -> Option<f64> {
    let f = field(message, n)?;
    valid(f)
        .then(|| f["rawValue"].as_f64())
        .flatten()
        .filter(|n| n.is_finite())
}
fn enum_number(message: &Value, n: u64) -> Option<u64> {
    let f = field(message, n)?;
    valid(f).then(|| f["rawValue"].as_u64()).flatten()
}
fn field_time(message: &Value, n: u64) -> Option<f64> {
    let f = field(message, n)?;
    if !valid(f) {
        return None;
    }
    if let Some(text) = f["value"].as_str() {
        let time = DateTime::parse_from_rfc3339(text).ok()?;
        Some(time.timestamp() as f64 + f64::from(time.timestamp_subsec_nanos()) / 1e9)
    } else {
        raw_number(message, n).map(|n| n + FIT_EPOCH)
    }
}
fn utc_string(seconds: f64) -> Option<String> {
    if !seconds.is_finite() {
        return None;
    }
    let whole = seconds.floor();
    let nanos = ((seconds - whole) * 1e9).round() as u32;
    DateTime::<Utc>::from_timestamp(
        whole as i64 + i64::from(nanos == 1_000_000_000),
        nanos % 1_000_000_000,
    )
    .map(|t| t.to_rfc3339_opts(SecondsFormat::AutoSi, true))
}
fn source(f: &Value) -> Value {
    f["sourceReference"].clone()
}
fn references(message: &Value, names: &[(&str, u64)]) -> Value {
    let mut result = Map::new();
    for &(name, n) in names {
        if let Some(f) = field(message, n) {
            result.insert(name.into(), source(f));
        }
    }
    Value::Object(result)
}
fn chosen_number(
    message: &Value,
    candidates: &[u64],
    name: &str,
    refs: &mut Map<String, Value>,
) -> Option<f64> {
    let fields = message["fields"].as_array()?;
    for &n in candidates {
        for role in ["native", "expanded"] {
            if let Some((field, value)) = fields
                .iter()
                .filter(|f| {
                    f["fieldNumber"].as_u64() == Some(n)
                        && f["developerIdentity"].is_null()
                        && f["role"] == role
                        && valid(f)
                        && native_value(message, f)
                })
                .find_map(|f| {
                    f["value"]
                        .as_f64()
                        .filter(|n| n.is_finite())
                        .map(|n| (f, n))
                })
            {
                refs.insert(name.into(), source(field));
                return Some(value);
            }
        }
    }
    None
}
fn pace(speed: Option<f64>) -> Option<f64> {
    speed
        .filter(|n| *n > 0.0)
        .map(|n| 1000.0 / n)
        .filter(|n| n.is_finite())
}
fn timer_state(points: &[(f64, Option<bool>)], time: f64) -> Option<bool> {
    if points.windows(2).any(|p| p[1].0 < p[0].0) {
        return None;
    }
    points
        .iter()
        .rev()
        .find(|(t, _)| *t <= time)
        .and_then(|(_, state)| *state)
}
fn gap_limit(samples: &[Sample]) -> f64 {
    let mut deltas: Vec<_> = samples
        .windows(2)
        .filter_map(|p| {
            let delta = p[1].time? - p[0].time?;
            (delta > 0.0).then_some(delta)
        })
        .collect();
    if deltas.is_empty() {
        return 5.0;
    }
    let middle = deltas.len() / 2;
    (3.0 * *deltas.select_nth_unstable_by(middle, f64::total_cmp).1).max(5.0)
}
fn active_seconds(points: &[(f64, Option<bool>)], from: f64, to: f64) -> f64 {
    if points.windows(2).any(|p| p[1].0 < p[0].0) {
        return 0.0;
    }
    let mut cursor = from;
    let mut running = timer_state(points, from);
    let mut result = 0.0;
    for &(time, next) in points.iter().filter(|(t, _)| *t > from && *t < to) {
        if running != Some(false) {
            result += time - cursor;
        }
        cursor = time;
        running = next;
    }
    if running != Some(false) {
        result += to - cursor;
    }
    result
}
fn summary(
    message: &Value,
    samples: &[Sample],
    timers: &[(f64, Option<bool>)],
    from: f64,
    to: f64,
    _origin: f64,
    gap: f64,
) -> Value {
    let lap = global(message) == Some(19);
    let mut refs = Map::new();
    let mut recorded = Map::new();
    for (name, n) in [
        ("distanceMeters", 9),
        ("timerTimeSeconds", 8),
        ("elapsedTimeSeconds", 7),
        ("movingTimeSeconds", if lap { 52 } else { 59 }),
        ("averageHeartRateBpm", if lap { 15 } else { 16 }),
        ("averagePowerWatts", if lap { 19 } else { 20 }),
    ] {
        recorded.insert(
            name.into(),
            json!(chosen_number(message, &[n], name, &mut refs)),
        );
    }
    let speed = chosen_number(
        message,
        &[if lap { 110 } else { 124 }, if lap { 13 } else { 14 }],
        "averageSpeedMps",
        &mut refs,
    );
    recorded.insert("averageSpeedMps".into(), json!(speed));
    recorded.insert("averagePaceSecondsPerKm".into(), json!(pace(speed)));
    if let Some(reference) = refs.get("averageSpeedMps").cloned() {
        refs.insert("averagePaceSecondsPerKm".into(), reference);
    }
    let fractional = if lap { 80 } else { 92 };
    let cadence = chosen_number(
        message,
        &[if lap { 17 } else { 18 }],
        "averageCadenceStepsPerMinute",
        &mut refs,
    )
    .map(|n| 2.0 * (n + number(message, fractional).unwrap_or(0.0)));
    recorded.insert("averageCadenceStepsPerMinute".into(), json!(cadence));
    if cadence.is_some() && number(message, fractional).is_some() {
        refs.insert(
            "averageFractionalCadenceCyclesPerMinute".into(),
            source(field(message, fractional).unwrap()),
        );
    }
    let mut derived = Map::new();
    let mut coverage = Map::new();
    for &(average, metric) in SUMMARY_METRICS {
        let mut weighted = 0.0;
        let mut seconds = 0.0;
        let mut covered_until = from;
        for pair in samples.windows(2) {
            let (Some(a), Some(b), Some(value), Some(_)) = (
                pair[0].time,
                pair[1].time,
                pair[0].metric(metric),
                pair[1].metric(metric),
            ) else {
                continue;
            };
            if b <= a || b - a > gap {
                continue;
            }
            let left = a.max(from).max(covered_until);
            let right = b.min(to);
            if right <= left {
                continue;
            }
            let weight = active_seconds(timers, left, right);
            weighted += value * weight;
            seconds += weight;
            covered_until = right;
        }
        derived.insert(
            average.into(),
            json!(
                (seconds > 0.0)
                    .then(|| weighted / seconds)
                    .filter(|n| n.is_finite())
            ),
        );
        coverage.insert(average.into(),json!({"coveredSeconds":seconds,"windowSeconds":to-from,"fraction":(to>from).then(||seconds/(to-from))}));
    }
    derived.insert(
        "averagePaceSecondsPerKm".into(),
        json!(pace(derived["averageSpeedMps"].as_f64())),
    );
    derived.insert("elapsedTimeSeconds".into(), json!(to - from));
    for name in ["distanceMeters", "timerTimeSeconds", "movingTimeSeconds"] {
        derived.insert(name.into(), Value::Null);
    }
    let mut result = recorded.clone();
    for (name, value) in &derived {
        if result.get(name).is_none_or(Value::is_null) {
            result.insert(name.clone(), value.clone());
        }
    }
    // Pace follows the selected speed, including a recorded zero; derived pace remains separately available.
    result.insert(
        "averagePaceSecondsPerKm".into(),
        json!(pace(result["averageSpeedMps"].as_f64())),
    );
    result.insert("recorded".into(), Value::Object(recorded));
    result.insert("derived".into(), Value::Object(derived));
    result.insert("sourceReferences".into(), Value::Object(refs));
    result.insert("method".into(),json!({"default":"recorded_summary_preferred","derived":"interval_weighted_left_sample","gapThresholdSeconds":gap,"pauseHandling":"exclude_recorded_timer_pauses","missingTimerState":"unknown_not_assumed_stopped","finalSample":"no_extrapolation"}));
    result.insert("coverage".into(), Value::Object(coverage));
    Value::Object(result)
}
fn extension(message: &Value, f: &Value) -> Value {
    let mut value = json!({"identity":{"globalMessageNumber":global(message),"fieldNumber":f["fieldNumber"],"developerIdentity":f["developerIdentity"]},
        "name":f["name"],"value":f["value"],"rawValue":f["rawValue"],"type":f["baseType"],"unit":f["unit"],"validity":f["validity"],
        "role":f["role"],"classification":f["classification"],"sourceReference":source(f),"componentParent":f["componentParent"]});
    if let Some(dependency) =
        global(message).and_then(|number| super::raw::native_context_dependency(number, f))
    {
        value["nativeContext"]=json!(message["fields"].as_array().into_iter().flatten()
            .filter(|field|super::raw::native_dependency_source(dependency,field))
            .map(|field|json!({"fieldNumber":field["fieldNumber"],"rawValue":field["rawValue"],"value":field["value"],"baseType":field["baseType"],"developerIdentity":field["developerIdentity"],"role":field["role"],"sourceReference":field["sourceReference"]})).collect::<Vec<_>>());
    }
    value
}

#[derive(Serialize, Deserialize)]
struct PendingRr {
    rr: Option<f64>,
    end_time: Option<f64>,
    reference: Value,
    message_number: u64,
    field_number: u64,
    method: String,
    anchor: Option<f64>,
    anchor_refs: Option<Value>,
    limitation: Option<String>,
}
struct RrState {
    spool: Spool,
    anchor: Option<(f64, f64)>,
    anchor_refs: Option<Value>,
    previous: Option<f64>,
    clock: Option<f64>,
}
impl RrState {
    fn new(spool: Spool) -> Self {
        Self {
            spool,
            anchor: None,
            anchor_refs: None,
            previous: None,
            clock: None,
        }
    }
    fn interval(
        &mut self,
        rr: Option<f64>,
        end_time: Option<f64>,
        provenance: (Value, u64, u64),
        method: &str,
        anchor: Option<f64>,
        limitation: Option<&str>,
    ) -> Result<(), FitError> {
        let (reference, message_number, field_number) = provenance;
        self.spool.push(&PendingRr {
            rr,
            end_time,
            reference,
            message_number,
            field_number,
            method: method.into(),
            anchor,
            anchor_refs: self.anchor_refs.clone(),
            limitation: limitation.map(str::to_owned),
        })
    }
    fn clear_anchor(&mut self) {
        self.anchor = None;
        self.anchor_refs = None;
    }
    fn consume(&mut self, message: &Value) -> Result<(), FitError> {
        if let Some(t) = field_time(message, 253) {
            self.clock = Some(t);
        }
        if global(message) == Some(78) {
            if let Some(f) = field(message, 0) {
                let elements = f["value"]
                    .as_array()
                    .map(Vec::as_slice)
                    .unwrap_or_else(|| std::slice::from_ref(&f["value"]));
                for (position, element) in elements.iter().enumerate() {
                    let rr = (valid(f)
                        && f["elementValidity"][position].as_str() != Some("invalid"))
                    .then(|| element.as_f64())
                    .flatten()
                    .filter(|n| n.is_finite() && *n >= 0.0)
                    .map(|n| n * 1000.0);
                    let mut reference = source(f);
                    if f["value"].is_array() {
                        reference["arrayIndex"] = json!(position);
                    }
                    // HRV records have no source time anchor, even when another message supplied one.
                    let saved = self.anchor_refs.take();
                    let result = self.interval(
                        rr,
                        None,
                        (reference, 78, 0),
                        "unanchored",
                        None,
                        Some(if rr == Some(0.0) {
                            "NON_POSITIVE_RECORDED_INTERVAL"
                        } else if rr.is_some() {
                            "NO_SOURCE_TIME_ANCHOR"
                        } else {
                            "INVALID_RECORDED_INTERVAL"
                        }),
                    );
                    self.anchor_refs = saved;
                    result?;
                }
            }
            return Ok(());
        }
        if global(message) != Some(132) {
            return Ok(());
        }
        let fields = message["fields"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let genuine_anchor = field_time(message, 253).zip(number(message, 0));
        let native_counter = field(message, 9).filter(|f| valid(f));
        let anchor_counter = native_counter.and_then(|f| {
            f["value"].as_f64().or_else(|| {
                f["value"]
                    .as_array()
                    .filter(|a| a.len() == 1)?
                    .first()?
                    .as_f64()
            })
        });
        if let (Some((timestamp, fractional)), Some(counter)) = (genuine_anchor, anchor_counter) {
            let utc = timestamp + fractional;
            if !counter.is_finite() || !utc.is_finite() {
                self.clear_anchor();
                self.previous = None;
                return Ok(());
            }
            if let (Some(last), Some((anchor_utc, base))) = (self.previous, self.anchor) {
                let delta = counter - last;
                let consistent = (utc - (anchor_utc + counter - base)).abs() <= 1.0 / 1024.0;
                let limitation = if delta <= 0.0 {
                    Some("EVENT_COUNTER_RESET")
                } else if delta > 10.0 || !consistent {
                    Some("EVENT_COUNTER_GAP_AMBIGUITY")
                } else {
                    None
                };
                self.interval(
                    (delta > 0.0).then_some(delta * 1000.0),
                    Some(utc),
                    (source(native_counter.unwrap()), 132, 9),
                    "hr_event_counter_anchor",
                    Some(anchor_utc),
                    limitation,
                )?;
            }
            self.anchor = Some((utc, counter));
            self.anchor_refs = Some(references(
                message,
                &[
                    ("timestamp", 253),
                    ("fractionalTimestamp", 0),
                    ("eventCounter", 9),
                ],
            ));
            self.previous = Some(counter);
        }
        for f in fields.iter().filter(|f| {
            f["fieldNumber"].as_u64() == Some(9)
                && f["developerIdentity"].is_null()
                && native_value(message, f)
        }) {
            if native(f) && genuine_anchor.is_some() && anchor_counter.is_some() {
                continue;
            }
            let expanded =
                f["role"].as_str() == Some("expanded") && f["componentParent"].as_u64() == Some(10);
            if !native(f) && !expanded {
                continue;
            }
            let elements = f["value"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_else(|| std::slice::from_ref(&f["value"]));
            for (position, element) in elements.iter().enumerate() {
                let counter = (valid(f)
                    && f["elementValidity"][position].as_str() != Some("invalid"))
                .then(|| element.as_f64())
                .flatten()
                .filter(|n| n.is_finite());
                let mut reference = source(f);
                if f["value"].is_array() {
                    reference["arrayIndex"] = json!(position);
                }
                let Some(counter) = counter else {
                    self.clear_anchor();
                    self.interval(
                        None,
                        None,
                        (reference, 132, 9),
                        "unanchored",
                        None,
                        Some("INVALID_EVENT_COUNTER"),
                    )?;
                    self.previous = None;
                    continue;
                };
                let Some(last) = self.previous else {
                    self.previous = Some(counter);
                    continue;
                };
                let delta = counter - last;
                let reset = delta <= 0.0;
                let too_large = delta > 10.0;
                let aligned = self.anchor.map(|(utc, base)| utc + counter - base);
                let clock_gap = aligned
                    .zip(self.clock)
                    .is_some_and(|(t, clock)| clock > t + 4.0);
                let limitation = if reset {
                    Some("EVENT_COUNTER_RESET")
                } else if too_large || clock_gap {
                    Some("EVENT_COUNTER_GAP_AMBIGUITY")
                } else if self.anchor.is_none() {
                    Some("NO_SOURCE_TIME_ANCHOR")
                } else {
                    None
                };
                if reset || too_large || clock_gap {
                    self.clear_anchor();
                }
                let timing = if self.anchor.is_some() { aligned } else { None };
                self.interval(
                    (!reset).then_some(delta * 1000.0),
                    timing,
                    (reference, 132, 9),
                    if timing.is_some() {
                        "hr_event_counter_anchor"
                    } else {
                        "unanchored"
                    },
                    self.anchor.map(|(utc, _)| utc),
                    limitation,
                )?;
                self.previous = Some(counter);
            }
        }
        Ok(())
    }
    fn output(&mut self, out: &mut dyn Write, start: f64, end: f64) -> Result<(), FitError> {
        bytes(out, b"{\"intervals\":[")?;
        let mut index = 0;
        let mut eligible = true;
        let mut reasons = Vec::<String>::new();
        self.spool.visit::<PendingRr,_>(&mut |row| {
            let elapsed=row.end_time.map(|t|t-start);
            let left=elapsed.zip(row.rr).map(|(t,rr)|t-rr/1000.0);
            let timing_eligible=row.rr.is_some_and(|n|n>0.0)&&row.limitation.is_none()&&row.anchor_refs.is_some()&&left.is_some_and(|t|t>=0.0)&&row.end_time.is_some_and(|t|t<=end);
            eligible&=timing_eligible;
            if let Some(reason)=&row.limitation && !reasons.contains(reason) { reasons.push(reason.clone()); }
            if row.end_time.is_some()&&!timing_eligible&&!reasons.iter().any(|r|r=="RR_OUTSIDE_SESSION") { reasons.push("RR_OUTSIDE_SESSION".into()); }
            if index!=0 { bytes(out,b",")?; }
            emit(out,&json!({"index":index,"rrMs":row.rr,"timestamp":row.end_time.and_then(utc_string),"elapsedSeconds":elapsed,"startElapsedSeconds":left,"endElapsedSeconds":elapsed,
                "sourceReference":row.reference,"timingEligible":timing_eligible,"provenance":{"kind":"recorded_rr","messageNumber":row.message_number,"fieldNumber":row.field_number,"timingMethod":row.method,"anchorTimestamp":row.anchor.and_then(utc_string),"anchorSourceReferences":row.anchor_refs,"limitations":row.limitation.into_iter().collect::<Vec<_>>()}}))?;
            index+=1;
            Ok(())
        })?;
        if index == 0 {
            eligible = false;
            reasons.push("NO_RECORDED_RR".into());
        }
        bytes(out, b"],\"alignmentEligible\":")?;
        emit(out, &eligible)?;
        bytes(out, b",\"reasons\":")?;
        emit(out, &reasons)?;
        bytes(out, b"}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn output_limit_rejects_write_before_any_bytes_escape() {
        let used = Rc::new(Cell::new(MAX_DOCUMENT_BYTES - 2));
        let mut output = Vec::new();
        let mut writer = LimitedWriter {
            writer: &mut output,
            used: Rc::clone(&used),
        };
        assert!(matches!(
            bytes(&mut writer, b"abc"),
            Err(FitError::ProcessingFailed {
                code: "DECODE_RESOURCE_LIMIT",
                ..
            })
        ));
        assert_eq!(used.get(), MAX_DOCUMENT_BYTES - 2);
        bytes(&mut writer, b"ab").unwrap();
        assert!(matches!(
            emit(&mut writer, &1),
            Err(FitError::ProcessingFailed {
                code: "DECODE_RESOURCE_LIMIT",
                ..
            })
        ));
        assert_eq!(output, b"ab");
    }

    #[test]
    fn private_spools_share_limit_and_cleanup_after_failure() {
        let path;
        {
            let workspace = Workspace::new(Rc::new(Cell::new(0))).unwrap();
            path = workspace.path.clone();
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o700
            );
            let mut first = workspace.spool("first").unwrap();
            let mut second = workspace.spool("second").unwrap();
            assert_eq!(
                fs::metadata(&first.path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            first.finish().unwrap();
            second.finish().unwrap();
            assert_eq!(workspace.used.get(), 4);
            workspace.used.set(MAX_DOCUMENT_BYTES - 1);
            first.writer.write_all(b"x").unwrap();
            first.writer.flush().unwrap();
            second.writer.write_all(b"y").unwrap();
            assert!(matches!(
                second.writer.flush().map_err(io_error),
                Err(FitError::ProcessingFailed {
                    code: "DECODE_RESOURCE_LIMIT",
                    ..
                })
            ));
        }
        assert!(!path.exists());
    }
}
