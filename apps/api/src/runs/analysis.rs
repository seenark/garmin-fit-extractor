use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use super::thresholds;

pub const VERSION: &str = thresholds::SEGMENT_VERSION;
const MAX_SAMPLE_GAP: f64 = 5.0;
const WORKLOAD_BLOCK_SECONDS: f64 = 30.0;

pub(crate) fn number(value: &Value, key: &str) -> Option<f64> {
    value.get(key)?.as_f64().filter(|v| v.is_finite())
}

fn signal_number(value:&Value,key:&str)->Option<f64> {
    let (low,high)=match key {
        "heartRateBpm" => (30.0,240.0),
        "speedMps" => (0.0,15.0),
        "powerWatts" => (0.0,3000.0),
        _ => return None,
    };
    number(value,key).filter(|v|*v>=low&&*v<=high)
}

pub(crate) fn elapsed_duration(normalized: &Value) -> Option<f64> {
    let recorded = number(&normalized["summary"], "elapsedTimeSeconds").filter(|v| *v > 0.0);
    let bounded = normalized["startTime"].as_str().zip(normalized["endTime"].as_str()).and_then(|(start,end)| {
        let start=chrono::DateTime::parse_from_rfc3339(start).ok()?;
        let end=chrono::DateTime::parse_from_rfc3339(end).ok()?;
        Some((end-start).num_milliseconds() as f64 / 1000.0)
    });
    match (recorded,bounded) {
        (_,Some(seconds)) if seconds<=0.0 => None,
        (Some(recorded),Some(seconds)) if (recorded-seconds).abs()>1.0 => None,
        (_,Some(seconds)) => Some(seconds),
        (recorded,None) => recorded,
    }
}

fn in_bounds<'a>(timeline:&'a [&Value],start:f64,end:f64)->&'a [&'a Value] {
    let left=timeline.partition_point(|s|number(s,"elapsedSeconds").unwrap_or(0.0)<start);
    let right=timeline.partition_point(|s|number(s,"elapsedSeconds").unwrap_or(0.0)<=end);
    &timeline[left..right]
}

pub(crate) struct TimerTimeline {
    pauses:Vec<(f64,f64)>,
    support:Vec<(f64,f64)>,
}

impl TimerTimeline {
    pub(crate) fn covers(&self,start:f64,end:f64)->bool {
        let index=self.support.partition_point(|(_,stop)|*stop<=start);
        self.support.get(index).is_some_and(|(a,b)|*a<=start+1e-9&&*b>=end-1e-9)
    }

    pub(crate) fn overlaps_pause(&self,start:f64,end:f64)->bool {
        let index=self.pauses.partition_point(|(_,stop)|*stop<=start);
        self.pauses.get(index).is_some_and(|(a,_)|*a<end)
    }
}

fn merge_intervals(intervals:&mut Vec<(f64,f64)>) {
    if !intervals.is_sorted_by(|a,b|a.0<=b.0){intervals.sort_unstable_by(|a,b|a.0.total_cmp(&b.0));}
    let mut count=0;
    for i in 0..intervals.len() {
        let current=intervals[i];
        if count>0&&current.0<=intervals[count-1].1 {intervals[count-1].1=intervals[count-1].1.max(current.1);}
        else {intervals[count]=current;count+=1;}
    }
    intervals.truncate(count);
}

pub(crate) fn timer_timeline(normalized:&Value)->Result<TimerTimeline,&'static str> {
    let duration=elapsed_duration(normalized).unwrap_or(0.0);
    let mut timeline=TimerTimeline{pauses:Vec::new(),support:Vec::new()};
    let mut stopped=None;
    let mut previous=-1.0;
    let mut first_event=None;
    if let Some(events)=normalized["timerEvents"].as_array() {
        for event in events.iter().filter(|e|e["event"]==0&&matches!(e["eventType"].as_i64(),Some(0|1|4))) {
            let time=number(event,"elapsedSeconds").filter(|t|*t>=0.0&&*t<=duration).ok_or("timerTimeUncertain")?;
            if time<previous{return Err("timerTimeUncertain");}
            previous=time;
            if first_event.is_none(){first_event=Some(time);}
            match event["eventType"].as_i64() {
                Some(1|4) => {if stopped.is_none(){stopped=Some(time);}},
                Some(0) => {if let Some(start)=stopped.take(){if time>start{timeline.pauses.push((start,time));}}},
                _ => {},
            }
        }
    }
    if let Some(start)=stopped.filter(|start|*start<duration){timeline.pauses.push((start,duration));}
    if let Some(samples)=normalized["samples"].as_array() {
        for pair in samples.windows(2).filter(|p|p[0]["timerRunning"].is_boolean()) {
            if let (Some(start),Some(end))=(number(&pair[0],"elapsedSeconds"),number(&pair[1],"elapsedSeconds")) {
                if start>=0.0&&end<=duration&&end>start&&end-start<=MAX_SAMPLE_GAP {
                    timeline.support.push((start,end));
                    if pair[0]["timerRunning"]==false {timeline.pauses.push((start,end));}
                }
            }
        }
    }
    merge_intervals(&mut timeline.support);
    if let Some(start)=first_event.filter(|start|*start<duration) {
        let index=timeline.support.partition_point(|(a,_)|*a<start);
        timeline.support.insert(index,(start,duration));
        merge_intervals(&mut timeline.support);
    }
    merge_intervals(&mut timeline.pauses);
    Ok(timeline)
}

fn metrics(samples: &[&Value], start: f64, end: f64) -> Value {
    let mut speed = Vec::new();
    let mut hr = Vec::new();
    let mut power = Vec::new();
    for sample in samples {
        let Some(t) = number(sample, "elapsedSeconds") else { continue; };
        if let Some(v) = signal_number(sample, "speedMps") { speed.push((t, v)); }
        if let Some(v) = signal_number(sample, "heartRateBpm") { hr.push((t, v)); }
        if let Some(v) = signal_number(sample, "powerWatts") { power.push(v); }
    }
    let average = |values: &[f64]| (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64);
    let speeds: Vec<f64> = speed.iter().map(|v| v.1).collect();
    let mean = average(&speeds);
    let cv = mean.filter(|m| *m > 0.0).map(|m| (speeds.iter().map(|v| (v - m).powi(2)).sum::<f64>() / speeds.len() as f64).sqrt() / m);
    let slope = if speed.len() >= 2 {
        let mean_t = speed.iter().map(|p| p.0).sum::<f64>() / speed.len() as f64;
        let mean_v = mean.unwrap_or(0.0);
        let denominator = speed.iter().map(|p| (p.0 - mean_t).powi(2)).sum::<f64>();
        (denominator > 0.0).then(|| speed.iter().map(|p| (p.0 - mean_t) * (p.1 - mean_v)).sum::<f64>() / denominator)
    } else { None };
    // Coverage is supported elapsed duration, not number of rows or a confidence score.
    let supported = |key: &str| samples.windows(2).filter_map(|pair| {
        let dt = number(pair[1], "elapsedSeconds")? - number(pair[0], "elapsedSeconds")?;
        (dt > 0.0 && dt <= MAX_SAMPLE_GAP && signal_number(pair[0], key).is_some() && signal_number(pair[1], key).is_some()).then_some(dt)
    }).sum::<f64>();
    let duration = end - start;
    json!({"speedMeanMps":mean,"speedCv":cv,"speedSlopeMpsPerSecond":slope,
        "heartRateDriftBpm":hr.first().zip(hr.last()).map(|(first,last)|last.1-first.1),
        "powerMeanWatts":average(&power),"sampleCount":samples.len(),
        "coverage":{"speed":if duration>0.0 {supported("speedMps")/duration} else {0.0},
            "heartRate":if duration>0.0 {supported("heartRateBpm")/duration} else {0.0},
            "power":if duration>0.0 {supported("powerWatts")/duration} else {0.0}}})
}

/// Observe workload, never workout intent. All features use original-resolution samples.
pub fn analyze(normalized: &Value) -> Value {
    let empty = Vec::new();
    let samples = normalized["samples"].as_array().unwrap_or(&empty);
    let duration = elapsed_duration(normalized).unwrap_or(0.0);
    let mut gaps = Vec::new();
    let mut duplicate_timestamps = 0;
    let mut out_of_order = 0;
    let mut invalid_times = 0;
    let mut artifacts = Vec::new();
    let mut coverage = [0.0; 3];
    let mut prior: Option<&Value> = None;
    for sample in samples {
        let Some(t) = number(sample, "elapsedSeconds").filter(|t| *t >= 0.0 && *t <= duration) else { invalid_times += 1; continue; };
        for (key, low, high) in [("heartRateBpm",30.0,240.0),("speedMps",0.0,15.0),("powerWatts",0.0,3000.0)] {
            if let Some(v) = number(sample,key) { if v < low || v > high { artifacts.push(json!({"elapsedSeconds":t,"metric":key,"reason":"outsideSignalRange"})); } }
        }
        if let Some(previous) = prior {
            let previous_t = number(previous,"elapsedSeconds").unwrap_or(t);
            let dt = t - previous_t;
            if dt == 0.0 { duplicate_timestamps += 1; }
            else if dt < 0.0 { out_of_order += 1; gaps.push(json!({"startElapsedSeconds":t,"endElapsedSeconds":previous_t,"reason":"outOfOrder"})); }
            else if dt > MAX_SAMPLE_GAP { gaps.push(json!({"startElapsedSeconds":previous_t,"endElapsedSeconds":t,"reason":"sampleGap"})); }
            else {
                for (i,key) in ["heartRateBpm","speedMps","powerWatts"].iter().enumerate() {
                    if signal_number(previous,key).is_some() && signal_number(sample,key).is_some() { coverage[i] += dt; }
                }
            }
        }
        prior = Some(sample);
    }
    let mut reasons = Vec::new();
    if samples.is_empty() { reasons.push("noSamples"); }
    if !gaps.is_empty() { reasons.push("discontinuousSamples"); }
    if duplicate_timestamps > 0 { reasons.push("duplicateSampleTime"); }
    if out_of_order > 0 { reasons.push("outOfOrderSamples"); }
    if invalid_times > 0 { reasons.push("invalidSampleTime"); }
    if duration <= 0.0 { reasons.push("activityBoundsUnavailable"); }
    let timer=timer_timeline(normalized);
    let pause_intervals=timer.as_ref().map(|timeline|timeline.pauses.as_slice());
    if timer.is_err(){reasons.push("timerTimeUncertain");}
    let paused_seconds=pause_intervals.as_ref().map(|pauses|pauses.iter().map(|(start,end)|end-start).sum::<f64>()).unwrap_or(0.0);
    let timer_complete=timer.as_ref().is_ok_and(|timeline|timeline.covers(0.0,duration));
    let recorded_timer=number(&normalized["summary"],"timerTimeSeconds");
    let derived_timer=timer_complete.then_some(duration-paused_seconds);
    let timer_difference=recorded_timer.zip(derived_timer).map(|(recorded,derived)|recorded-derived);
    if timer_difference.is_some_and(|difference|difference.abs()>1.0){reasons.push("timerTotalsDisagree");}
    let mut timeline:Vec<&Value>=samples.iter().filter(|s|number(s,"elapsedSeconds").is_some_and(|t|t>=0.0&&t<=duration)).collect();
    if out_of_order>0 {timeline.sort_by(|a,b|number(a,"elapsedSeconds").unwrap().total_cmp(&number(b,"elapsedSeconds").unwrap()));}
    let mut blocks: Vec<Value> = Vec::new();
    let mut cursor = 0.0;
    let mut carryover = false;
    let mut previous_mean: Option<f64> = None;
    while cursor < duration {
        let mut end = (cursor + WORKLOAD_BLOCK_SECONDS).min(duration);
        let next=timeline.partition_point(|s|number(s,"elapsedSeconds").unwrap_or(0.0)<cursor);
        if timeline.get(next).is_none_or(|s|number(s,"elapsedSeconds").unwrap_or(0.0)>end) {
            end=timeline.get(next).map(|s|(number(s,"elapsedSeconds").unwrap()/WORKLOAD_BLOCK_SECONDS).floor()*WORKLOAD_BLOCK_SECONDS).unwrap_or(duration).max(end).min(duration);
        }
        let selected=in_bounds(&timeline,cursor,end);
        let features = metrics(selected, cursor, end);
        let mean = number(&features,"speedMeanMps");
        let gap = gaps.iter().any(|g| number(g,"startElapsedSeconds").unwrap_or(0.0) < end && number(g,"endElapsedSeconds").unwrap_or(0.0) > cursor);
        let pause=timer.as_ref().is_ok_and(|timeline|timeline.overlaps_pause(cursor,end));
        let coverage_speed = number(&features["coverage"],"speed").unwrap_or(0.0);
        let increase = previous_mean.zip(mean).is_some_and(|(previous,current)|previous>0.0 && current>previous*1.15);
        let decrease = previous_mean.zip(mean).is_some_and(|(previous,current)|previous>0.0 && current<previous*0.85);
        let slope = number(&features,"speedSlopeMpsPerSecond").unwrap_or(0.0);
        let kind = if pause { "pause" } else if gap || coverage_speed < 0.8 { "unknown" }
            else if increase { "repeatedsurge" } else if decrease { "recovery" }
            else if slope >= 0.0003 { "progressive" }
            else if number(&features,"speedCv").is_some_and(|cv| cv <= 0.08) { "steady" }
            else { "unknown" };
        if matches!(kind,"repeatedsurge"|"recovery") { carryover = true; }
        let mut rejected = Vec::new();
        if kind != "progressive" { rejected.push(if kind=="pause" {"timerPause"} else if gap {"sampleGap"} else {"unsupportedProtocol"}); }
        if carryover { rejected.push("fatigueCarryover"); }
        if pause_intervals.is_err(){rejected.push("requiredContextUnprovable");}
        if selected.windows(2).any(|s|number(s[1],"elapsedSeconds")<=number(s[0],"elapsedSeconds")) { rejected.push("sampleOrderUncertain"); }
        let eligible = rejected.is_empty();
        let eligibility = json!({"accepted":eligible,"reasons":rejected});
        blocks.push(json!({"index":blocks.len(),"kind":kind,"startElapsedSeconds":cursor,"endElapsedSeconds":end,
            "durationSeconds":end-cursor,"timeBasis":"elapsed","version":VERSION,"features":features,
            "eligibility":{"lt1":eligibility,"lt2":eligibility}}));
        previous_mean = mean;
        cursor = end;
    }
    let surge_bounds:Vec<(f64,f64)>=blocks.iter().filter(|b|b["kind"]=="repeatedsurge").map(|b|(number(b,"startElapsedSeconds").unwrap(),number(b,"endElapsedSeconds").unwrap())).collect();
    if surge_bounds.len()<2 {for block in &mut blocks {if block["kind"]=="repeatedsurge"{block["kind"]=json!("unknown");}}}
    // Join adjacent same-kind blocks. Numerical RR windows are not downsampled.
    let mut segments: Vec<Value> = Vec::new();
    for block in blocks {
        if let Some(last) = segments.last_mut() {
            if last["kind"] == block["kind"] && last["eligibility"] == block["eligibility"] {
                last["endElapsedSeconds"] = block["endElapsedSeconds"].clone();
                let start = number(last,"startElapsedSeconds").unwrap_or(0.0);
                let end = number(last,"endElapsedSeconds").unwrap_or(start);
                last["durationSeconds"] = json!(end-start);
                continue;
            }
        }
        let mut block = block;
        block["index"] = json!(segments.len());
        segments.push(block);
    }
    for segment in &mut segments {
        let start=number(segment,"startElapsedSeconds").unwrap_or(0.0);
        let end=number(segment,"endElapsedSeconds").unwrap_or(start);
        segment["features"]=metrics(in_bounds(&timeline,start,end),start,end);
        let surge_count=surge_bounds.iter().filter(|(a,b)|*a<end&&*b>start).count();
        segment["features"]["workloadSurgeCount"]=json!(surge_count);
        segment["features"]["repeatPatternObserved"]=json!(surge_bounds.len()>=2&&surge_count>0);
    }
    let targets = thresholds::estimate_activity(normalized, &segments);
    json!({"schemaVersion":"2.0.0","version":VERSION,"quality":{"sampleCount":samples.len(),"elapsedSeconds":duration,
        "coverage":{"heartRate":if duration>0.0 {coverage[0]/duration} else {0.0},"speed":if duration>0.0 {coverage[1]/duration} else {0.0},"power":if duration>0.0 {coverage[2]/duration} else {0.0}},
        "gaps":gaps,"duplicateTimestamps":duplicate_timestamps,"outOfOrder":out_of_order,"invalidTimes":invalid_times,
        "artifacts":artifacts,"pausedSeconds":paused_seconds,"pauseTimeBasis":"observedTimerStateUnion",
        "recordedTimerTimeSeconds":recorded_timer,"derivedTimerTimeSeconds":derived_timer,"timerTotalsDifferenceSeconds":timer_difference,
        "timerCoverage":if timer_complete {"complete"} else if timer.as_ref().is_ok_and(|timeline|!timeline.support.is_empty()) {"partial"} else {"unavailable"},
        "timerPauseIntervals":pause_intervals.unwrap_or_default().iter().map(|(start,end)|json!({"startElapsedSeconds":start,"endElapsedSeconds":end})).collect::<Vec<_>>(),"reasons":reasons},"segments":segments,"thresholds":targets,
        "transformations":[{"type":"derivedWorkloadBlocks","blockSeconds":WORKLOAD_BLOCK_SECONDS,"originalSamplesChanged":false,"derivedTimelineSorted":out_of_order>0,"hrStabilityFilter":false}]})
}

/// Exact source-independent temporal/metric signature; evidence only, never auto-merge.
pub fn duplicate_evidence(normalized: &Value) -> Value {
    let samples = normalized["samples"].as_array().map(|rows|rows.iter().map(|v|json!([
        v["elapsedSeconds"],v["timestamp"],v["speedMps"],v["distanceMeters"],v["heartRateBpm"],v["powerWatts"],v["cadenceStepsPerMinute"],v["altitudeMeters"],v["timerRunning"]])).collect::<Vec<_>>()).unwrap_or_default();
    let rr = normalized["rr"]["intervals"].as_array().map(|rows|rows.iter().map(|v|json!([v["startElapsedSeconds"],v["endElapsedSeconds"],v["rrMs"]])).collect::<Vec<_>>()).unwrap_or_default();
    let laps = normalized["laps"].as_array().map(|rows|rows.iter().map(|v|json!([v["startElapsedSeconds"],v["endElapsedSeconds"],v["summary"]])).collect::<Vec<_>>()).unwrap_or_default();
    let timer_events=normalized["timerEvents"].as_array().map(|rows|rows.iter().map(|v|json!([v["elapsedSeconds"],v["event"],v["eventType"]])).collect::<Vec<_>>()).unwrap_or_default();
    let signature = json!([normalized["startTime"],normalized["endTime"],normalized["summary"],samples,rr,laps,timer_events]);
    let enough = normalized["startTime"].is_string() && normalized["endTime"].is_string() && !samples.is_empty();
    json!({"signatureVersion":"exact-temporal-stream-1.0.0","exactTemporalStreamSignature":if enough {Some(format!("{:x}",Sha256::digest(signature.to_string().as_bytes())))} else {None},
        "groupingEvidence":if enough {"exactTimestampsDurationSummaryLapsAndStreams"} else {"insufficientIdentityEvidence"},"automaticMerge":false})
}
