use std::collections::BTreeMap;
use serde_json::{Map, Value, json};

#[derive(Clone, Copy, Debug, Default)]
pub struct Policy {
    pub include_location: bool,
    pub include_device_identifiers: bool,
}

/// Project only documented schema positions. Unknown keys never enter omission paths.
pub fn project(activity: &Value, policy: Policy) -> (Value, Value) {
    let mut omissions = BTreeMap::new();
    let value = object(activity, "activity", "activities.*", policy, &mut omissions);
    (value, omission_rows(omissions))
}

type Omissions = BTreeMap<&'static str, BTreeMap<String, u64>>;
fn omit(omissions: &mut Omissions, category: &'static str, path: &str) {
    let counts = omissions.entry(category).or_default();
    if let Some(count) = counts.get_mut(path) { *count += 1; }
    else { counts.insert(path.to_owned(), 1); }
}

fn object(value: &Value, schema: &str, path: &str, policy: Policy, omissions: &mut Omissions) -> Value {
    object_with_references(value,schema,path,policy,omissions,None)
}

fn object_with_references(value:&Value,schema:&str,path:&str,policy:Policy,omissions:&mut Omissions,parent_references:Option<&Value>)->Value {
    if value.is_null() { return Value::Null; }
    let Some(input) = value.as_object() else { omit(omissions,"unclassified",path); return Value::Null; };
    let mut output = Map::new();
    for (key, value) in input {
        let rule = rule(schema, key);
        if rule.is_empty() {
            if !value.is_null() && !value.as_array().is_some_and(Vec::is_empty) && !value.as_object().is_some_and(Map::is_empty) {
                omit(omissions, "unclassified", &format!("{path}.*"));
            }
            continue;
        }
        let child_path = format!("{path}.{key}");
        if matches!(schema,"sample"|"summary"|"summaryMetrics"|"recordedThreshold"|"interval") {
            let reference = parent_references.or_else(||input.get("sourceReferences")).and_then(|r|r.get(key)).or_else(||input.get("sourceReference"));
            let category=if rule=="location" {"location"} else if rule=="deviceIdentifiers" {"deviceIdentifiers"} else {"metric"};
            if reference.is_some_and(|r| !safe_reference(r,category)) {
                omit(omissions,"unclassified",&child_path);
                continue;
            }
        }
        if rule == "location" || rule == "deviceIdentifiers" {
            let allowed = if rule == "location" { policy.include_location } else { policy.include_device_identifiers };
            if !allowed { omit(omissions, if rule == "location" {"location"} else {"deviceIdentifiers"}, &child_path); continue; }
            // Consented fields are scalar coordinates or identifiers, never opaque containers.
            if scalar(value) { output.insert(key.clone(), value.clone()); }
            else { omit(omissions,"unclassified",&child_path); }
        } else if rule == "scalar" {
            if scalar(value) { output.insert(key.clone(), value.clone()); }
            else { omit(omissions,"unclassified",&child_path); }
        } else if rule == "number" {
            if value.is_null() || value.is_number() { output.insert(key.clone(),value.clone()); }
            else { omit(omissions,"unclassified",&child_path); }
        } else if let Some(expected)=rule.strip_prefix("literal:") {
            if value.as_str()==Some(expected) {output.insert(key.clone(),value.clone());}
            else {omit(omissions,"unclassified",&child_path);}
        } else if rule == "hash" {
            if hash(value) { output.insert(key.clone(),value.clone()); }
            else { omit(omissions,"unclassified",&child_path); }
        } else if rule == "messages" {
            output.insert(key.clone(), messages(value, &child_path, policy, omissions));
        } else if rule == "nativeFields" {
            let fields = value.as_array().map(|values|values.iter().filter_map(|field|
                input.get("globalMessageNumber").and_then(Value::as_u64).and_then(|number|
                    project_field(number,field,&format!("{child_path}.*"),policy,omissions))
            ).collect()).unwrap_or_default();
            output.insert(key.clone(),Value::Array(fields));
        } else if rule == "extensions" {
            let fields=value.as_array().map(|values|values.iter().filter_map(|field| {
                let number=field["identity"]["globalMessageNumber"].as_u64();
                if let Some(number)=number {project_field(number,field,&format!("{child_path}.*"),policy,omissions)}
                else {omit(omissions,"unclassified",&format!("{child_path}.*"));None}
            }).collect()).unwrap_or_default();
            output.insert(key.clone(),Value::Array(fields));
        } else if let Some(child) = rule.strip_prefix("array:") {
            if value.is_null() { output.insert(key.clone(),Value::Null); continue; }
            if let Some(values) = value.as_array() {
                let items = values.iter().filter_map(|item| {
                    if child == "scalar" {
                        if scalar(item) { Some(item.clone()) } else { omit(omissions,"unclassified",&format!("{child_path}.*")); None }
                    } else { Some(object(item, child, &format!("{child_path}.*"), policy, omissions)) }
                }).collect();
                output.insert(key.clone(),Value::Array(items));
            } else { omit(omissions,"unclassified",&child_path); }
        } else {
            if schema=="references" && !safe_reference(value,"metric") {omit(omissions,"unclassified",&child_path);continue;}
            let references=if schema=="summary" && key=="recorded" {input.get("sourceReferences")} else {None};
            output.insert(key.clone(), object_with_references(value,rule,&child_path,policy,omissions,references));
        }
    }
    Value::Object(output)
}
fn scalar(value: &Value) -> bool { value.is_null() || value.is_string() || value.is_number() || value.is_boolean() }
fn hash(value:&Value)->bool {value.as_str().is_some_and(|text|text.len()==64 && text.bytes().all(|b|b.is_ascii_hexdigit() && !b.is_ascii_uppercase()))}

fn rule<'a>(schema: &str, key: &'a str) -> &'a str {
    match (schema, key) {
        ("activity", "id") => "scalar",
        ("activity", "decoded"|"normalized"|"analysis"|"historicalThresholds") => match key {"historicalThresholds"=>"history",_=>key},
        ("decoded", "schemaVersion") => "scalar",
        ("decoded", "decoder") => "decoder",
        ("decoded", "messages") => "messages",
        ("decoded"|"normalized", "warnings") => "array:warning",
        ("decoder", "name"|"library"|"libraryVersion"|"version"|"profileVersion"|"options"|"hrMerge"|"archiveSchemaVersion"|"normalizedSchemaVersion"|"normalizerVersion"|"jsonFloatRoundtrip") => if key=="options" {"array:scalar"} else {"scalar"},
        ("decoder", "vendorSourceSha256"|"profileSourceSha256"|"normalizerSourceSha256"|"archiveSourceSha256"|"rawProjectionSourceSha256") => "hash",
        ("parameters", "windowEnumeration"|"nativeRrTimingMethod") => "scalar",
        ("parameters", "requiredAnchorFields") => "array:scalar",
        ("parameters", "floatingBoundToleranceSeconds") => "number",
        ("session", "localUtcOffsetSeconds") => "number",
        ("session", "localUtcOffsetSourceReferences") => "references",
        ("session", "sourceLocalTimeContext") => "localTime",
        ("session", "sourceLocalTimeContexts") => "array:localTime",
        ("localTime", "activeTimeZone") => "number",
        ("localTime", "timeOffsetsSeconds"|"timeZoneOffsetsHours") => "array:scalar",
        ("localTime", "sourceReferences") => "references",
        ("normalized", "schemaVersion"|"startTime"|"endTime"|"sport"|"subtype") => "scalar",
        ("normalized", "summary") | ("lap", "summary") => "summary",
        ("normalized", "session") => "session",
        ("normalized", "samples") => "array:sample",
        ("normalized", "laps") => "array:lap",
        ("normalized", "timerEvents") => "array:timer",
        ("normalized", "sensors") => "array:sensor",
        ("normalized", "zones") => "array:zone",
        ("normalized", "deviceReportedThresholds") => "array:recordedThreshold",
        ("normalized", "rr") => "rr",
        ("normalized", "extensions") => "extensions",
        ("summary"|"summaryMetrics", "distanceMeters"|"timerTimeSeconds"|"elapsedTimeSeconds"|"movingTimeSeconds"|"averageSpeedMps"|"averagePaceSecondsPerKm"|"averageHeartRateBpm"|"averagePowerWatts"|"averageCadenceStepsPerMinute") => "number",
        ("summary", "recorded"|"derived") => "summaryMetrics",
        ("summary", "sourceReferences") => "references",
        ("summary", "coverage") => "summaryCoverage",
        ("summary", "method") => "summaryMethod",
        ("summaryCoverage", "averageSpeedMps"|"averagePaceSecondsPerKm"|"averageHeartRateBpm"|"averagePowerWatts"|"averageCadenceStepsPerMinute") => "summaryMetricCoverage",
        ("summaryMetricCoverage", "coveredSeconds"|"windowSeconds"|"fraction") => "number",
        ("summaryMethod", "gapThresholdSeconds") => "number",
        ("summaryMethod", "default") => "literal:recorded_summary_preferred",
        ("summaryMethod", "derived") => "literal:interval_weighted_left_sample",
        ("summaryMethod", "pauseHandling") => "literal:exclude_recorded_timer_pauses",
        ("summaryMethod", "missingTimerState") => "literal:unknown_not_assumed_stopped",
        ("summaryMethod", "finalSample") => "literal:no_extrapolation",
        ("sample", "index"|"elapsedSeconds"|"speedMps"|"paceSecondsPerKm"|"heartRateBpm"|"powerWatts"|"cadenceStepsPerMinute"|"altitudeMeters"|"distanceMeters") => "number",
        ("sample", "timestamp"|"timerRunning") => "scalar",
        ("sample"|"session"|"lap"|"timer"|"sensor"|"recordedThreshold", "sourceReferences") => "references",
        ("sample"|"lap"|"session", "latitude"|"longitude"|"latitudeDegrees"|"longitudeDegrees"|"startLatitude"|"startLongitude"|"endLatitude"|"endLongitude") => "location",
        ("sensor", "serialNumber"|"deviceId"|"unitId"|"antDeviceNumber") => "deviceIdentifiers",
        ("lap", "index"|"startElapsedSeconds"|"endElapsedSeconds") => "number",
        ("lap", "startTime"|"endTime") => "scalar",
        ("timer", "index"|"elapsedSeconds") | ("session", "index") => "number",
        ("timer", "timestamp"|"event"|"eventType") => "scalar",
        ("sensor", "index"|"sensorType"|"type"|"sourceType"|"manufacturer"|"product"|"unit") => "scalar",
        ("zone", "index"|"minimum"|"maximum"|"low"|"high") => "number",
        ("zone", "metric"|"unit"|"source"|"definition") => "scalar",
        ("sensor"|"zone", "globalMessageNumber") => "number",
        ("sensor"|"zone", "fields") => "nativeFields",
        ("sensor"|"zone"|"recordedThreshold", "sourceReference") => "reference",
        ("recordedThreshold", "index"|"value") => "number",
        ("recordedThreshold", "kind"|"unit"|"provenance") => "scalar",
        ("recordedThreshold", "heartRateBpm"|"speedMps"|"powerWatts") => "number",
        ("recordedThreshold", "type"|"timestamp"|"definition") => "scalar",
        ("references", "timestamp"|"startTime"|"endTime"|"elapsedSeconds"|"speedMps"|"paceSecondsPerKm"|"heartRateBpm"|"powerWatts"|"cadenceStepsPerMinute"|"altitudeMeters"|"distanceMeters"|"timerTimeSeconds"|"elapsedTimeSeconds"|"averageSpeedMps"|"averageHeartRateBpm"|"averagePowerWatts"|"averageCadenceStepsPerMinute"|"event"|"eventType") => "reference",
        ("references", "movingTimeSeconds"|"averagePaceSecondsPerKm"|"averageFractionalCadenceCyclesPerMinute") => "reference",
        ("reference", "messageIndex"|"globalMessageNumber"|"fieldNumber"|"byteOffset"|"byteLength"|"componentParent"|"arrayIndex") => "number",
        ("rr", "intervals") => "array:interval",
        ("rr", "alignmentEligible") => "scalar",
        ("rr"|"provenance", "reasons"|"limitations") => "array:scalar",
        ("interval", "index"|"rrMs"|"elapsedSeconds"|"startElapsedSeconds"|"endElapsedSeconds") => "number",
        ("interval", "timestamp"|"timingEligible") => "scalar",
        ("interval", "sourceReference") => "reference",
        ("interval", "provenance") => "provenance",
        ("provenance", "kind"|"messageNumber"|"fieldNumber"|"timingMethod"|"anchorTimestamp") => "scalar",
        ("warning", "code"|"reason"|"messageIndex"|"globalMessageNumber"|"fieldNumber") => "scalar",
        ("analysis", "schemaVersion"|"version") => "scalar",
        ("analysis", "quality") => "quality",
        ("analysis", "segments") => "array:segment",
        ("analysis", "thresholds") => "history",
        ("analysis", "transformations") => "array:transformation",
        ("quality", "coverage"|"counts"|"usableDurationSeconds"|"pauseRatio"|"gapCount"|"duplicateTimestampCount"|"outOfOrderTimestampCount"|"artifactCount"|"sampleCount"|"timerTimeSeconds"|"elapsedTimeSeconds"|"elapsedSeconds"|"duplicateTimestamps"|"outOfOrder"|"invalidTimes"|"pausedSeconds") => if key=="coverage" || key=="counts" {"qualityMetrics"} else {"number"},
        ("quality", "recordedTimerTimeSeconds"|"derivedTimerTimeSeconds"|"timerTotalsDifferenceSeconds"|"timerCoverage") => "number",
        ("quality", "pauseTimeBasis") => "scalar",
        ("quality", "timerPauseIntervals") => "array:boundary",
        ("quality", "warnings"|"reasons") => "array:scalar",
        ("quality", "gaps"|"pauses") => "array:boundary",
        ("qualityMetrics", "heartRateBpm"|"powerWatts"|"speedMps"|"rr"|"samples"|"valid"|"missing"|"durationSeconds"|"coverageRatio"|"heartRate"|"speed"|"power") => "number",
        ("boundary", "startElapsedSeconds"|"endElapsedSeconds"|"durationSeconds"|"startIndex"|"endIndex") => "number",
        ("boundary", "reason") => "scalar",
        ("quality", "artifacts") => "array:artifact",
        ("artifact", "elapsedSeconds") => "number",
        ("artifact", "metric"|"reason") => "scalar",
        ("segment", "index"|"startElapsedSeconds"|"endElapsedSeconds"|"durationSeconds"|"repeatIndex"|"repeatGroup"|"startIndex"|"endIndex") => "number",
        ("segment", "kind"|"type"|"timeBasis"|"version"|"accepted") => "scalar",
        ("segment", "eligibility") => "eligibility",
        ("eligibility", "lt1"|"lt2") => "verdict",
        ("verdict", "accepted") => "scalar",
        ("verdict", "reasons") => "array:scalar",
        ("segment", "features") => "features",
        ("segment", "reasons"|"acceptReasons"|"rejectReasons") => "array:scalar",
        ("features", "averageSpeedMps"|"averageHeartRateBpm"|"averagePowerWatts"|"speedCv"|"powerCv"|"hrDrift"|"durationSeconds"|"sourceCount"|"coverageRatio"|"speedMeanMps"|"speedSlopeMpsPerSecond"|"heartRateDriftBpm"|"powerMeanWatts"|"sampleCount") => "number",
        ("features", "workloadSurgeCount") => "number",
        ("features", "repeatPatternObserved") => "scalar",
        ("features", "coverage") => "qualityMetrics",
        ("history", "evidenceCutoff"|"computedAt"|"schemaVersion"|"engineStatus"|"stale"|"policyVersion"|"researchBlocked") => "scalar",
        ("history", "lt1"|"lt2") => "target",
        ("target", "status"|"engineStatus"|"methodVersion"|"targetDefinition"|"availability"|"experimentalEstimate"|"researchBlocked"|"requiredContextUnprovable") => "scalar",
        ("target", "contextUnverified") => "array:scalar",
        ("target", "method") => "method",
        ("method", "id"|"version") => "scalar",
        ("method", "configurationHash") => "hash",
        ("target", "freshness") => "freshness",
        ("freshness", "stale") => "scalar",
        ("freshness", "ageDays") => "number",
        ("target", "value"|"uncertainty") => "value",
        ("target", "reasons"|"limits"|"limitations") => "array:scalar",
        ("target", "evidence") => "evidence",
        ("target", "trace") => "trace",
        ("target", "suggestions") => "array:suggestion",
        ("value", "heartRateBpm"|"speedMps"|"paceSecondsPerKm"|"powerWatts"|"lower"|"upper"|"value"|"alpha"|"crossingHeartRateBpm") => "number",
        ("value", "unit"|"type"|"definition"|"method") => "scalar",
        ("value", "interval") => "array:scalar",
        ("value", "reason") => "scalar",
        ("evidence", "activityCount"|"independentActivityCount"|"windowCount"|"acceptedCount"|"rejectedCount"|"ageDays"|"sampleCount"|"sourceCount") => "number",
        ("evidence", "startTime"|"endTime"|"dateRangeStart"|"dateRangeEnd"|"lineage") => "scalar",
        ("trace", "parameters") => "parameters",
        ("trace", "regression"|"counts") => "calculation",
        ("trace", "crossing"|"independentActivityCount") => "number",
        ("trace", "evidenceCutoff"|"lineage"|"computedAt") => "scalar",
        ("trace", "formula"|"methodVersion"|"limitations") => if key=="limitations" {"array:scalar"} else {"scalar"},
        ("parameters", "windowSeconds"|"stepSeconds"|"alphaTarget"|"minimumBeats"|"artifactLimit"|"lambda"|"lookbackDays"|"minimumHeartRateSpanBpm"|"scaleMinimum"|"scaleMaximum") => "number",
        ("parameters", "detrending"|"interpolation"|"version") => "scalar",
        ("parameters", "scales") => "array:scalar",
        ("parameters", "localTrendOrder"|"artifactCeiling"|"minimumInnerWindows"|"minimumHrCoverage"|"maximumSampleGapSeconds"|"maximumRrContinuityErrorSeconds") => "number",
        ("parameters", "workloadBlockSeconds"|"steadySpeedCvCeiling"|"minimumProgressiveSpeedSlopeMpsPerSecond"|"surgeRelativeChange"|"minimumSpeedCoverage"|"minimumRepeatedSurges") => "number",
        ("parameters", "inputProjectionVersion") => "scalar",
        ("parameters", "segmentVersion"|"samplingConvention"|"overlap"|"scaleTail"|"fluctuation"|"fit"|"windowClosure"|"artifactPolicy"|"selector"|"comparability") => "scalar",
        ("calculation", "slope"|"intercept"|"rSquared"|"crossingHeartRateBpm"|"alphaTarget"|"pointCount"|"accepted"|"rejected"|"total"|"minimumHeartRateBpm"|"maximumHeartRateBpm"|"hrMin"|"hrMax"|"alphaMin"|"alphaMax"|"candidate"|"windows"|"acceptedWindows"|"rejectedWindows") => "number",
        ("suggestion", "code"|"purpose"|"duration"|"durationSeconds"|"rationale"|"expectedData"|"safety"|"optional") => "scalar",
        ("suggestion", "prerequisites"|"steps"|"limits") => "array:scalar",
        ("transformation", "kind"|"method"|"version"|"timeBasis"|"precision"|"description"|"resolutionSeconds"|"sourceCount"|"outputCount"|"type"|"blockSeconds"|"originalSamplesChanged"|"hrStabilityFilter"|"derivedTimelineSorted") => "scalar",
        _ => "",
    }
}

// Native FIT profile identity, never a supplied display name or classification.
fn native_category(message: u64, field: u64) -> Option<&'static str> {
    match (message, field) {
        (20, 0|1) | (18, 3|4|29|30|31|32|38|39) | (19, 3|4|5|6) => Some("location"),
        (0, 3|5) | (23, 3|21) => Some("deviceIdentifiers"),
        (20, 2|3|4|5|6|7|8|9|13|30|31|32|39|40|41|53|73|78|83|84|85|253) => Some("metric"),
        (18, 0|1|2|5|6|7|8|9|10|11|13|14|15|16|17|18|19|20|21|22|23|24|25|26|27|28|34|35|42|43|44|45|46|47|48|49|57|58|59|60|65|66|67|68|69|70|71|86|87|88|89|90|91|92|124|253) => Some("metric"),
        (19, 0|1|2|7|8|9|10|11|12|13|14|15|16|17|18|19|20|21|22|23|24|25|26|32|33|35|37|38|39|40|52|57|58|59|60|71|74|75|76|77|78|80|110|253) => Some("metric"),
        (0, 0|1|2|4) | (2,0|1|2|4|5|39) | (23, 0|1|2|4|5|6|25|253) | (21, 0|1|3|4|253) | (34, 0|1|2|3|4|5|6|253) | (78, 0) | (132, 0|1|6|9|10|253) | (7,1|2|3|5|7) | (8,1|254) | (9,1|254) | (53,0|254) | (216,13|15) => Some("metric"),
        _ => None,
    }
}

fn messages(value: &Value, path: &str, policy: Policy, omissions: &mut Omissions) -> Value {
    let Some(messages) = value.as_array() else { omit(omissions,"unclassified",path); return json!([]); };
    Value::Array(messages.iter().filter_map(|message| project_message(message,&format!("{path}.*"),policy,omissions)).collect())
}

fn project_message(message:&Value,path:&str,policy:Policy,omissions:&mut Omissions)->Option<Value> {
    let number = message.get("globalMessageNumber")?.as_u64()?;
    let mut output = Map::new();
    for key in ["index","globalMessageNumber","localMessageNumber"] {
        if let Some(value) = message.get(key).filter(|v| v.is_number()) { output.insert(key.into(),value.clone()); }
    }
    if let Some(reference)=message.get("sourceReference") {
        output.insert("sourceReference".into(),object(reference,"reference",&format!("{path}.sourceReference"),policy,omissions));
    }
    let field_path=format!("{path}.fields.*");
    let fields = message.get("fields").and_then(Value::as_array).map(|fields|fields.iter().filter_map(|field|
        project_field(number,field,&field_path,policy,omissions)
    ).collect()).unwrap_or_default();
    output.insert("fields".into(),Value::Array(fields));
    Some(Value::Object(output))
}

fn project_field(number:u64, field:&Value, path:&str, policy:Policy, omissions:&mut Omissions)->Option<Value> {
    let identity=&field["identity"];
    let Some(field_number)=field.get("fieldNumber").or_else(||identity.get("fieldNumber")).and_then(Value::as_u64) else {
        omit(omissions,"unclassified",path);return None;
    };
    let reference=&field["sourceReference"];
    let developer=field.get("developerIdentity").is_some_and(|v|!v.is_null()) ||
        identity.get("developerIdentity").is_some_and(|v|!v.is_null()) ||
        reference.get("developerIdentity").is_some_and(|v|!v.is_null());
    let category = if developer || identity.get("globalMessageNumber").and_then(Value::as_u64).is_some_and(|n|n!=number) ||
        reference.get("globalMessageNumber").and_then(Value::as_u64).is_some_and(|n|n!=number) ||
        reference.get("fieldNumber").and_then(Value::as_u64).is_some_and(|n|n!=field_number) {None}
        else {native_category(number,field_number)};
    let Some(category)=category else {omit(omissions,"unclassified",path);return None;};
    let parent=field.get("componentParent").or_else(||reference.get("componentParent")).and_then(Value::as_u64);
    if parent.is_some_and(|n|native_category(number,n)!=Some(category)) {omit(omissions,"unclassified",path);return None;}
    if (category=="location" && !policy.include_location) || (category=="deviceIdentifiers" && !policy.include_device_identifiers) {
        omit(omissions,category,path);return None;
    }
    let value=field.get("value")?;
    let numeric=|v:&Value|v.is_null() || v.is_number() || v.is_boolean() || v.as_str().is_some_and(|s|s.parse::<i128>().is_ok());
    let safe_string=value.as_str().is_some_and(|text|
        (matches!((number,field_number),(0,4)|(18|19,2)|(18|19|20|21|23|34|132,253)) && chrono::DateTime::parse_from_rfc3339(text).is_ok()) ||
        (field["enumCode"].is_number() && matches!(text,"activity"|"garmin"|"running"|"generic"|"treadmill"|"street"|"trail"|"track"|"timer"|"start"|"stop"|"stop_all"|"stop_disable"|"stop_disable_all"|"manual"|"auto"|"heart_rate"|"antplus"|"bluetooth"|"bluetooth_low_energy"|"local"))
    );
    if !(numeric(value) || value.as_array().is_some_and(|a|a.iter().all(numeric)) || safe_string) {omit(omissions,"unclassified",path);return None;}
    let mut safe=Map::new();
    safe.insert("fieldNumber".into(),json!(field_number));
    safe.insert("value".into(),value.clone());
    if identity.is_object() {safe.insert("identity".into(),json!({"globalMessageNumber":number,"fieldNumber":field_number}));}
    if let Some(name)=verified_name(number,field_number) {safe.insert("name".into(),json!(name));}
    safe.insert("classification".into(),json!(category));
    for key in ["rawValue","scale","offset"] {
        if let Some(value)=field.get(key).filter(|v|numeric(v) || v.as_array().is_some_and(|a|a.iter().all(numeric))) {safe.insert(key.into(),value.clone());}
    }
    for key in ["baseType","type","byteSize","profileType","unit","validity","role","componentParent","compressedTimeOffset","enumCode"] {
        if let Some(value)=field.get(key) {
            let safe_text=value.as_str().is_some_and(|text|matches!(text,
                "valid"|"invalid"|"mixed"|"native"|"expanded"|"reconstructed"|
                "enum"|"sint8"|"uint8"|"sint16"|"uint16"|"sint32"|"uint32"|"sint64"|"uint64"|"float32"|"float64"|
                "date_time"|"local_date_time"|"m"|"m/s"|"s"|"bpm"|"watts"|"rpm"|"cycles"|"semicircles"|"C"|"%"|"ms"));
            if numeric(value) || safe_text {safe.insert(key.into(),value.clone());}
        }
    }
    if !reference.is_null() {safe.insert("sourceReference".into(),object(reference,"reference",&format!("{path}.sourceReference"),policy,omissions));}
    Some(Value::Object(safe))
}

/// Bounded decoded-message chunk projection, using the same compound identity gate as full exports.
pub fn project_decoded_messages(value:&Value,policy:Policy)->(Value,Value) {
    let mut omissions=BTreeMap::new();
    let projected=messages(value,"activities.*.decoded.messages",policy,&mut omissions);
    (projected,omission_rows(omissions))
}

/// Bounded sample chunk projection; no surrounding archive or activity document is required.
pub fn project_normalized_samples(value:&Value,policy:Policy)->(Value,Value) {
    let mut omissions=BTreeMap::new();
    let projected=Value::Array(value.as_array().map(|samples|samples.iter().map(|sample|
        object(sample,"sample","activities.*.normalized.samples.*",policy,&mut omissions)
    ).collect()).unwrap_or_default());
    (projected,omission_rows(omissions))
}

pub fn project_normalized(value:&Value,policy:Policy)->(Value,Value) {
    let mut omissions=BTreeMap::new();
    let projected=object(value,"normalized","activities.*.normalized",policy,&mut omissions);
    (projected,omission_rows(omissions))
}

fn omission_rows(omissions:Omissions)->Value {
    Value::Array(omissions.into_iter().flat_map(|(category,paths)|paths.into_iter().map(move |(path,count)|json!({
        "category":category,"pathPattern":path,"count":count,"reason":"Field omitted by the export schema and consent policy."
    }))).collect())
}

#[derive(Clone, Copy)]
pub enum Document { Decoded, Normalized }

/// One bounded archive item at a time. Paths contain only schema keys and wildcard array positions.
pub struct Projection { document:Document, policy:Policy, omissions:Omissions }
impl Projection {
    pub fn new(document:Document,policy:Policy)->Self {Self{document,policy,omissions:BTreeMap::new()}}
    fn resolve<'a>(&self,path:&'a [String])->(&'a str,String) {
        let root=match self.document {Document::Decoded=>"decoded",Document::Normalized=>"normalized"};
        let mut schema=root;
        let mut safe=format!("activities.*.{root}");
        for key in path {
            let next=if key=="*" {
                match schema {"messages"=>"message","extensions"|"nativeFields"=>"extension",_=>schema.strip_prefix("array:").unwrap_or("")}
            } else {rule(schema,key)};
            if next.is_empty() {safe.push_str(".*");return ("",safe);}
            safe.push('.');safe.push_str(key);
            schema=next;
        }
        (schema,safe)
    }
    pub fn keep(&mut self,path:&[String])->bool {
        let (schema,safe)=self.resolve(path);
        let category=match schema {"location" if !self.policy.include_location=>Some("location"),"deviceIdentifiers" if !self.policy.include_device_identifiers=>Some("deviceIdentifiers"),""=>Some("unclassified"),_=>None};
        if let Some(category)=category {omit(&mut self.omissions,category,&safe);false} else {true}
    }
    pub fn value(&mut self,path:&[String],value:Value)->Option<Value> {
        let (schema,safe)=self.resolve(path);
        let policy=self.policy;
        match schema {
            ""=>None,
            "message"=>project_message(&value,&safe,policy,&mut self.omissions),
            "extension"=>value["identity"]["globalMessageNumber"].as_u64().and_then(|number|project_field(number,&value,&safe,policy,&mut self.omissions)),
            "scalar"|"location"|"deviceIdentifiers" if scalar(&value)=>Some(value),
            "number" if value.is_null()||value.is_number()=>Some(value),
            "hash" if hash(&value)=>Some(value),
            "scalar"|"location"|"deviceIdentifiers"|"number"|"hash"=>{omit(&mut self.omissions,"unclassified",&safe);None},
            _=>Some(object(&value,schema,&safe,policy,&mut self.omissions)),
        }
    }
    pub fn finish(self)->Value {omission_rows(self.omissions)}
}

fn verified_name(message: u64, field: u64) -> Option<&'static str> {
    if field == 253 { return Some("timestamp"); }
    match (message,field) {
        (20,0)=>Some("position_lat"), (20,1)=>Some("position_long"),
        (20,2)=>Some("altitude"), (20,3)=>Some("heart_rate"), (20,4)=>Some("cadence"),
        (20,5)=>Some("distance"), (20,6)=>Some("speed"), (20,7)=>Some("power"),
        (20,13)=>Some("temperature"), (20,53)=>Some("fractional_cadence"),
        (20,73)=>Some("enhanced_speed"), (20,78)=>Some("enhanced_altitude"),
        (18|19,2)=>Some("start_time"), (18|19,7)=>Some("total_elapsed_time"),
        (18|19,8)=>Some("total_timer_time"), (18|19,9)=>Some("total_distance"),
        (18,14)|(19,13)=>Some("avg_speed"), (18,16)|(19,15)=>Some("avg_heart_rate"),
        (18,18)|(19,17)=>Some("avg_cadence"), (18,20)|(19,19)=>Some("avg_power"),
        (78,0)=>Some("time"), (132,9)=>Some("event_timestamp"),
        (132,10)=>Some("event_timestamp_12"), (0|23,3)=>Some("serial_number"),
        _=>None,
    }
}

fn safe_reference(reference: &Value,category:&str) -> bool {
    if reference.get("developerIdentity").is_some_and(|d|!d.is_null()) {return false;}
    reference.get("globalMessageNumber").and_then(Value::as_u64)
        .zip(reference.get("fieldNumber").and_then(Value::as_u64))
        .is_some_and(|(message,field)|native_category(message,field)==Some(category) &&
            reference.get("componentParent").filter(|value|!value.is_null()).is_none_or(|value|value.as_u64().is_some_and(|parent|native_category(message,parent)==Some(category))))
}
