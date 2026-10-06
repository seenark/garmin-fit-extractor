use serde_json::{Value, json};

pub const METHOD_VERSION: &str = "running-dfa-open-1.0.0";
pub const SEGMENT_VERSION: &str = "workload-observed-1.0.0";
pub const PROJECTION_VERSION: &str = "runs-numerical-input-1.0.0";

fn linear_fit(points: &[(f64, f64)]) -> Option<(f64, f64)> {
    if points.len() < 2 {
        return None;
    }
    let count = points.len() as f64;
    let mx = points.iter().map(|p| p.0).sum::<f64>() / count;
    let my = points.iter().map(|p| p.1).sum::<f64>() / count;
    let variance = points.iter().map(|p| (p.0 - mx).powi(2)).sum::<f64>();
    if !variance.is_finite() || variance <= f64::EPSILON * count {
        return None;
    }
    let slope = points.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum::<f64>() / variance;
    let intercept = my - slope * mx;
    (slope.is_finite() && intercept.is_finite()).then_some((slope, intercept))
}

/// Smoothness-priors residual (I - (I + lambda² D₂ᵀD₂)⁻¹) RR.
/// Pentadiagonal Cholesky uses O(beats) storage and work, never a dense matrix.
fn smoothness_residual(rr: &[f64]) -> Vec<f64> {
    let n = rr.len();
    let lambda_squared = 250_000.0;
    let mut diagonal = vec![1.0_f64; n];
    let mut first = vec![0.0_f64; n];
    let mut second = vec![0.0_f64; n];
    for i in 0..n - 2 {
        diagonal[i] += lambda_squared;
        diagonal[i + 1] += 4.0 * lambda_squared;
        diagonal[i + 2] += lambda_squared;
        first[i + 1] -= 2.0 * lambda_squared;
        first[i + 2] -= 2.0 * lambda_squared;
        second[i + 2] += lambda_squared;
    }
    for i in 0..n {
        if i >= 2 {
            second[i] /= diagonal[i - 2];
        }
        if i >= 1 {
            first[i] = (first[i]
                - if i >= 2 {
                    second[i] * first[i - 1]
                } else {
                    0.0
                })
                / diagonal[i - 1];
        }
        diagonal[i] = (diagonal[i] - first[i].powi(2) - second[i].powi(2)).sqrt();
    }
    // Center before solving to preserve invariance under constant RR offsets.
    let mean = rr.iter().sum::<f64>() / n as f64;
    let mut trend: Vec<f64> = rr.iter().map(|v| v - mean).collect();
    for i in 0..n {
        trend[i] = (trend[i]
            - if i >= 1 { first[i] * trend[i - 1] } else { 0.0 }
            - if i >= 2 {
                second[i] * trend[i - 2]
            } else {
                0.0
            })
            / diagonal[i];
    }
    for i in (0..n).rev() {
        trend[i] = (trend[i]
            - if i + 1 < n {
                first[i + 1] * trend[i + 1]
            } else {
                0.0
            }
            - if i + 2 < n {
                second[i + 2] * trend[i + 2]
            } else {
                0.0
            })
            / diagonal[i];
    }
    rr.iter()
        .zip(trend)
        .map(|(raw, smoothed)| raw - mean - smoothed)
        .collect()
}

/// Public numerical seam. Input is a corrected copy of recorded RR in milliseconds.
/// Returns all F(4..16), not a physiological validation or a window eligibility verdict.
pub fn dfa_window(rr: &[f64]) -> Option<Value> {
    if rr.len() < 32 || rr.iter().any(|v| !v.is_finite() || *v <= 0.0) {
        return None;
    }
    let minimum = rr.iter().copied().fold(f64::INFINITY, f64::min);
    let maximum = rr.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if maximum - minimum <= f64::EPSILON * maximum * 16.0 {
        return None;
    }
    let residual = smoothness_residual(rr);
    let mean = residual.iter().sum::<f64>() / residual.len() as f64;
    let mut cumulative = 0.0;
    let profile: Vec<f64> = residual
        .iter()
        .map(|v| {
            cumulative += v - mean;
            cumulative
        })
        .collect();
    let mut fluctuations = Vec::with_capacity(13);
    let mut logarithms = Vec::with_capacity(13);
    for scale in 4..=16 {
        let x_mean = (scale - 1) as f64 / 2.0;
        let x_variance = (0..scale).map(|i| (i as f64 - x_mean).powi(2)).sum::<f64>();
        let mut squared = 0.0;
        let mut count = 0;
        for chunk in profile.chunks_exact(scale) {
            let y_mean = chunk.iter().sum::<f64>() / scale as f64;
            let slope = chunk
                .iter()
                .enumerate()
                .map(|(i, y)| (i as f64 - x_mean) * (y - y_mean))
                .sum::<f64>()
                / x_variance;
            squared += chunk
                .iter()
                .enumerate()
                .map(|(i, y)| (y - y_mean - slope * (i as f64 - x_mean)).powi(2))
                .sum::<f64>();
            count += scale;
        }
        let fluctuation = (squared / count as f64).sqrt();
        if !fluctuation.is_finite() || fluctuation <= 0.0 {
            return None;
        }
        fluctuations.push(fluctuation);
        logarithms.push(((scale as f64).ln(), fluctuation.ln()));
    }
    let (alpha, intercept) = linear_fit(&logarithms)?;
    Some(
        json!({"alpha":alpha,"fluctuations":fluctuations,"scales":(4..=16).collect::<Vec<_>>(),"logFitIntercept":intercept}),
    )
}

use super::analysis::{elapsed_duration, number};
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub fn configuration() -> Value {
    json!({"version":METHOD_VERSION,"segmentVersion":SEGMENT_VERSION,"lambda":500,
        "samplingConvention":"one sample per recorded beat, no resampling",
        "scales":(4..=16).collect::<Vec<_>>(),"overlap":false,"scaleTail":"discardIncompleteBlock",
        "localTrendOrder":1,"fluctuation":"pooledSquaredResidualsThenRoot","fit":"ordinaryLeastSquares",
        "windowSeconds":120,"stepSeconds":5,"windowClosure":"interval ends in (start,end]; complete anchored boundary support required",
        "windowEnumeration":"continuous source RR chains; one rejected right-tail candidate; no empty elapsed-duration loops",
        "nativeRrTimingMethod":"hr_event_counter_anchor","requiredAnchorFields":[253,0,9],"floatingBoundToleranceSeconds":1e-9,
        "inputProjectionVersion":PROJECTION_VERSION,"workloadBlockSeconds":30,"steadySpeedCvCeiling":0.08,
        "minimumProgressiveSpeedSlopeMpsPerSecond":0.0003,"surgeRelativeChange":0.15,"minimumSpeedCoverage":0.8,"minimumRepeatedSurges":2,
        "artifactCeiling":0.03,"artifactPolicy":"local median 11 beats; outside250..2000ms or deviation>20%; linear time interpolation between clean neighbors",
        "selector":"contiguous inner alpha[0.5,1.0]; at most one directly adjacent crossing-boundary window at each end",
        "minimumInnerWindows":3,"minimumHrCoverage":0.95,"maximumSampleGapSeconds":5,
        "maximumRrContinuityErrorSeconds":0.005,"lookbackDays":7,"comparability":"sensor recording path and observed continuous progressive protocol; never cross-sensor pooling"})
}

fn method(target: f64) -> Value {
    let configuration = configuration();
    json!({"id":if target==0.75 {"running-dfa-a1-075"} else {"running-dfa-a1-050"},
        "version":METHOD_VERSION,"configurationHash":format!("{:x}",Sha256::digest(configuration.to_string().as_bytes()))})
}

fn suggestion(reason: &str, target: f64) -> Option<Value> {
    let (purpose, prerequisites, steps, duration, rationale, expected) = match reason {
        "noExerciseRR" | "rrTimeAlignmentUncertain" | "requiredContextUnprovable" => (
            "Verify exercise RR recording and source-supported timing before collecting more data.",
            vec![
                "A recorder and sensor that store real beat-to-beat exercise RR with demonstrable timing.",
            ],
            vec![
                "Check sensor and recorder documentation for exercise RR and native timestamp support.",
                "Check recording settings and contact; HR-only records cannot recover beat intervals.",
                "Import a future run already in your plan only if recording is supported.",
            ],
            "Recording setup check; no additional workout required.",
            "Sampled HR and unanchored RR cannot support the numerical method.",
            "Real recorded exercise RR with beat order, native timing and sensor provenance.",
        ),
        "excessArtifact" => (
            "Improve recording quality, not exercise intensity.",
            vec!["An exercise-RR capable sensor and recorder."],
            vec![
                "Check belt fit/contact and recording interruptions.",
                "Use your existing planned run for a new recording if safe.",
            ],
            "Setup check; no fixed additional exercise duration.",
            "This method excludes windows or regression portions with more than 3% corrected beats.",
            "A future recording with fewer annotated corrections and continuous timing.",
        ),
        "noTargetBracket"
        | "unsupportedProtocol"
        | "insufficientContinuousWindow"
        | "fatigueCarryover"
        | "contextUnverified" => (
            "Optionally record an existing continuous progression without forcing a threshold crossing.",
            vec![
                "Real time-aligned exercise RR.",
                "A continuous progression already appropriate to your existing plan.",
            ],
            vec![
                "Record the progression already in your plan without adding pauses or recovery repetitions.",
                "Do not raise intensity or continue running only to make this target appear.",
                "For a physiological threshold claim, consider qualified supervised assessment with the exact reference target.",
            ],
            "Existing planned duration only; at least one complete 120-second window is numerically necessary, never sufficient.",
            if target == 0.75 {
                "The first ventilatory proxy requires an observed alpha 0.75 bracket."
            } else {
                "The second ventilatory proxy separately requires an observed alpha 0.50 bracket; LT1 does not provide LT2."
            },
            "Continuous full-resolution workload and exercise RR; an eligible bracket is not guaranteed.",
        ),
        _ => return None,
    };
    Some(
        json!({"optional":true,"purpose":purpose,"prerequisites":prerequisites,"steps":steps,
        "duration":duration,"rationale":rationale,"expectedData":expected,
        "safety":"Do not add exhaustive or maximal effort. Stop according to your normal safety guidance; symptoms or uncertainty warrant qualified advice.",
        "limits":["No guarantee of an estimate.","Synthetic software agreement is not physiological validation.","This is not a training plan or zone prescription."]}),
    )
}

fn input_digest(normalized: &Value) -> Option<String> {
    normalized.get("samples")?;
    let mut hash = Sha256::new();
    serde_json::to_writer(&mut hash, normalized).ok()?;
    Some(format!("{:x}", hash.finalize()))
}

fn historical_input_digest(evidence: &Value) -> Option<String> {
    let normalized = &evidence["normalized"];
    if normalized.get("samples").is_some() {
        return input_digest(normalized);
    }
    // This receipt is constructed only by authenticated core from verified immutable
    // PG manifests. It is not a FIT field or a public ingestion/validation shortcut.
    let receipt = &evidence["evidenceVerification"];
    let revision = normalized["sourceRevision"]
        .as_str()
        .filter(|s| !s.is_empty())?;
    let hash = receipt["inputHash"].as_str().filter(|s| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })?;
    (receipt["kind"] == "immutableNumericalProjection"
        && receipt["revisionId"] == revision
        && receipt["projectionVersion"] == PROJECTION_VERSION
        && normalized["projectionVersion"] == PROJECTION_VERSION)
        .then(|| hash.to_string())
}

fn target_result(
    target: f64,
    normalized: &Value,
    input_hash: &Option<String>,
    windows: &[Value],
    candidates: Vec<Value>,
    reasons: Vec<String>,
) -> Value {
    let accepted: Vec<&Value> = candidates
        .iter()
        .filter(|c| c["accepted"] == true)
        .collect();
    let chosen = (accepted.len() == 1).then(|| accepted[0]);
    let crossing = chosen.and_then(|c| number(c, "crossing"));
    let mut reasons = reasons;
    if accepted.len() > 1 {
        reasons.push("ambiguousCrossing".into());
    }
    let suggestions: Vec<Value> = reasons
        .iter()
        .find_map(|r| suggestion(r, target))
        .into_iter()
        .collect();
    let eligible_count = usize::from(crossing.is_some());
    json!({"status":if crossing.is_some() {"low_confidence"} else {"insufficient_data"},
        "engineStatus":"experimental","researchBlocked":true,"requiredContextUnprovable":reasons.iter().any(|r|matches!(r.as_str(),"requiredContextUnprovable"|"rrTimeAlignmentUncertain")),
        "method":method(target),"targetDefinition":if target==0.75 {"VT1 proxy (HRVT1), not measured blood-lactate LT1"} else {"VT2 proxy (HRVT2), not measured blood-lactate LT2"},
        "value":crossing.map(|heart_rate|json!({"heartRateBpm":heart_rate})),
        "uncertainty":{"interval":null,"reason":"No validated person-level empirical uncertainty interval; overlapping-window OLS is not physiological confidence."},
        "reasons":reasons,"contextUnverified":["rrSensorIdentity","recovery","fatigueState","sleep","caffeine","mealTiming","medication","treadmillCalibration","healthSuitability"],
        "limits":["Experimental open preprocessing and automatic selector, not Kubios-equivalent.","No licensed paired human running RR/gas-exchange or lactate reference corpus validated here.","VT proxy only; not validated lactate threshold or automatic zone prescription.","Seven-day lookback is an operational ceiling, not physiological stability."],
        "evidence":{"independentActivityCount":eligible_count,"startTime":normalized["startTime"],"endTime":normalized["endTime"],"ageDays":0.0},
        "trace":{"parameters":configuration(),"inputRevision":normalized["sourceRevision"],"inputHash":input_hash,"sourceHash":normalized["sourceHash"],
            "inputNormalizedHash":normalized["normalizedDocumentHash"],"inputProjectionVersion":normalized["projectionVersion"],
            "evidenceCutoff":normalized["endTime"],"computedAt":Utc::now().to_rfc3339(),"windows":windows,"candidates":candidates,
            "counts":{"candidate":candidates.len(),"accepted":accepted.len(),"rejected":candidates.len()-accepted.len(),
                "windows":windows.len(),"acceptedWindows":windows.iter().filter(|w|w["accepted"]==true).count(),"rejectedWindows":windows.iter().filter(|w|w["accepted"]!=true).count()},
            "regression":chosen.map(|c|c["regression"].clone()),"crossing":crossing,
            "boundaryWindowIndices":chosen.map(|c|c["boundaryWindowIndices"].clone()),
            "independentActivityCount":eligible_count},
        "suggestions":suggestions})
}

struct Beat {
    start: f64,
    end: f64,
    raw: f64,
    corrected: Option<f64>,
    reasons: Vec<&'static str>,
}

fn recorded_beats(normalized: &Value) -> Result<Vec<Beat>, &'static str> {
    let Some(rows) = normalized["rr"]["intervals"]
        .as_array()
        .filter(|rows| !rows.is_empty())
    else {
        return Err("noExerciseRR");
    };
    if normalized["rr"]["alignmentEligible"] != true {
        return Err("rrTimeAlignmentUncertain");
    }
    let duration = elapsed_duration(normalized).ok_or("requiredContextUnprovable")?;
    let activity_start = normalized["startTime"]
        .as_str()
        .and_then(|t| DateTime::parse_from_rfc3339(t).ok());
    let mut beats: Vec<Beat> = Vec::with_capacity(rows.len());
    for row in rows {
        let provenance = &row["provenance"];
        let timing = provenance["timingMethod"].as_str().unwrap_or("");
        let anchor = provenance["anchorTimestamp"]
            .as_str()
            .and_then(|v| DateTime::parse_from_rfc3339(v).ok());
        let native = timing == "hr_event_counter_anchor"
            && provenance["messageNumber"] == 132
            && provenance["fieldNumber"] == 9
            && [
                ("timestamp", 253),
                ("fractionalTimestamp", 0),
                ("eventCounter", 9),
            ]
            .iter()
            .all(|(key, field)| {
                provenance["anchorSourceReferences"][key]["globalMessageNumber"] == 132
                    && provenance["anchorSourceReferences"][key]["fieldNumber"] == *field
            });
        let synthetic =
            timing == "synthetic_explicit_beat_bounds" && normalized["softwareFixture"] == true;
        if row["timingEligible"] != true
            || provenance["kind"] != "recorded_rr"
            || anchor.is_none()
            || !(native || synthetic)
        {
            return Err("requiredContextUnprovable");
        }
        let raw = number(row, "rrMs").ok_or("invalidRrInterval")?;
        let end = number(row, "endElapsedSeconds")
            .or_else(|| number(row, "elapsedSeconds"))
            .ok_or("rrTimeAlignmentUncertain")?;
        if native {
            let absolute_end = row["timestamp"]
                .as_str()
                .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
                .ok_or("requiredContextUnprovable")?;
            let origin = activity_start.ok_or("requiredContextUnprovable")?;
            let elapsed = (absolute_end - origin)
                .num_microseconds()
                .ok_or("rrTimeAlignmentUncertain")? as f64
                / 1_000_000.0;
            if (elapsed - end).abs() > 0.005 {
                return Err("rrTimeAlignmentUncertain");
            }
        }
        let start = number(row, "startElapsedSeconds").unwrap_or(end - raw / 1000.0);
        if start < -1e-9
            || end > duration + 1e-9
            || end <= start
            || (end - start - raw / 1000.0).abs() > 0.005
        {
            return Err("rrTimeAlignmentUncertain");
        }
        let start = start.max(0.0);
        let end = end.min(duration);
        if beats
            .last()
            .is_some_and(|previous| end <= previous.end || start < previous.end - 0.005)
        {
            return Err("reorderedOrDuplicateRr");
        }
        beats.push(Beat {
            start,
            end,
            raw,
            corrected: Some(raw),
            reasons: Vec::new(),
        });
    }
    let mut marked = vec![false; beats.len()];
    for i in 0..beats.len() {
        let mut neighbors = [0.0_f64; 11];
        let mut count = 0;
        for beat in &beats[i.saturating_sub(5)..(i + 6).min(beats.len())] {
            if (250.0..=2000.0).contains(&beat.raw) {
                neighbors[count] = beat.raw;
                count += 1;
            }
        }
        neighbors[..count].sort_unstable_by(f64::total_cmp);
        let median = (count > 0).then(|| neighbors[count / 2]);
        let outlier = median.is_some_and(|m| (beats[i].raw - m).abs() / m > 0.20);
        if beats[i].raw < 250.0 || beats[i].raw > 2000.0 || outlier {
            marked[i] = true;
            beats[i].reasons.push(if outlier {
                "localMedianDeviation"
            } else {
                "outsideRrRange"
            });
        }
    }
    let mut cursor = 0;
    while cursor < beats.len() {
        if !marked[cursor] {
            cursor += 1;
            continue;
        }
        let start = cursor;
        while cursor < beats.len() && marked[cursor] {
            cursor += 1;
        }
        let endpoints = start
            .checked_sub(1)
            .zip((cursor < beats.len()).then_some(cursor))
            .and_then(|(previous, next)| {
                // Inspect continuity once per artifact run; never bridge a source gap.
                if beats[previous..=next]
                    .windows(2)
                    .any(|b| (b[1].start - b[0].end).abs() > 0.005)
                {
                    return None;
                }
                Some((
                    beats[previous].end,
                    beats[next].end,
                    beats[previous].raw,
                    beats[next].raw,
                ))
            });
        for beat in &mut beats[start..cursor] {
            beat.corrected =
                endpoints.map(|(a, b, ra, rb)| ra + (rb - ra) * (beat.end - a) / (b - a));
            if beat.corrected.is_some() {
                beat.reasons.push("linearTimeInterpolation");
            }
        }
    }
    Ok(beats)
}

struct HrInterval {
    start: f64,
    end: f64,
    hr: f64,
    slope: f64,
    area_before: f64,
    support_before: f64,
}

struct HrTimeline {
    intervals: Vec<HrInterval>,
    total_area: f64,
    total_support: f64,
}

impl HrTimeline {
    fn new(normalized: &Value) -> Self {
        let mut timeline = Self {
            intervals: Vec::new(),
            total_area: 0.0,
            total_support: 0.0,
        };
        if let Some(samples) = normalized["samples"].as_array() {
            for pair in samples.windows(2) {
                let (Some(start), Some(end)) = (
                    number(&pair[0], "elapsedSeconds"),
                    number(&pair[1], "elapsedSeconds"),
                ) else {
                    continue;
                };
                if end <= start
                    || end - start > 5.0
                    || pair[0]["timerRunning"] == false
                    || timeline
                        .intervals
                        .last()
                        .is_some_and(|previous| start < previous.end)
                {
                    continue;
                }
                let (Some(a), Some(b)) = (
                    number(&pair[0], "heartRateBpm"),
                    number(&pair[1], "heartRateBpm"),
                ) else {
                    continue;
                };
                if !(30.0..=240.0).contains(&a) || !(30.0..=240.0).contains(&b) {
                    continue;
                }
                timeline.intervals.push(HrInterval {
                    start,
                    end,
                    hr: a,
                    slope: (b - a) / (end - start),
                    area_before: timeline.total_area,
                    support_before: timeline.total_support,
                });
                timeline.total_area += (a + b) / 2.0 * (end - start);
                timeline.total_support += end - start;
            }
        }
        timeline
    }

    fn cumulative(&self, time: f64) -> (f64, f64) {
        let i = self
            .intervals
            .partition_point(|interval| interval.end <= time);
        let Some(interval) = self.intervals.get(i) else {
            return (self.total_area, self.total_support);
        };
        let duration = (time - interval.start)
            .max(0.0)
            .min(interval.end - interval.start);
        (
            interval.area_before + duration * interval.hr + 0.5 * interval.slope * duration.powi(2),
            interval.support_before + duration,
        )
    }

    fn mean(&self, start: f64, end: f64) -> Option<(f64, f64)> {
        let a = self.cumulative(start);
        let b = self.cumulative(end);
        let support = b.1 - a.1;
        (support > 0.0).then_some(((b.0 - a.0) / support, support / (end - start)))
    }
}

fn compute_windows(normalized: &Value, segments: &[Value], beats: &[Beat]) -> Vec<Value> {
    let duration = elapsed_duration(normalized).unwrap_or(0.0);
    let mut windows = Vec::new();
    let hr_timeline = HrTimeline::new(normalized);
    let timer = super::analysis::timer_timeline(normalized);
    for chain in beats.chunk_by(|a, b| (b.start - a.end).abs() <= 0.005) {
        if chain.len() < 32 {
            continue;
        }
        let mut center = ((chain[0].start + 60.0) / 5.0).ceil() * 5.0;
        let last_center = ((chain.last().unwrap().end - 60.0) / 5.0).ceil() * 5.0;
        while center <= last_center && center + 60.0 <= duration {
            let start = center - 60.0;
            let end = center + 60.0;
            let first = beats.partition_point(|b| b.end <= start);
            let last = beats.partition_point(|b| b.end <= end);
            let selected = &beats[first..last];
            let mut reasons: Vec<&str> = Vec::new();
            if !timer
                .as_ref()
                .is_ok_and(|timeline| timeline.covers(start, end))
            {
                reasons.push("requiredContextUnprovable");
            }
            if timer
                .as_ref()
                .is_ok_and(|timeline| timeline.overlaps_pause(start, end))
            {
                reasons.push("timerPause");
            }
            let segment = segments.iter().find(|s| {
                number(s, "startElapsedSeconds").is_some_and(|t| t <= start)
                    && number(s, "endElapsedSeconds").is_some_and(|t| t >= end)
            });
            if !segment.is_some_and(|s| s["eligibility"]["lt1"]["accepted"] == true) {
                let carryover = segments.iter().any(|s| {
                    number(s, "startElapsedSeconds").is_some_and(|t| t < end)
                        && number(s, "endElapsedSeconds").is_some_and(|t| t > start)
                        && s["eligibility"]["lt1"]["reasons"]
                            .as_array()
                            .is_some_and(|r| r.iter().any(|r| r == "fatigueCarryover"))
                });
                reasons.push(if carryover {
                    "fatigueCarryover"
                } else {
                    "unsupportedProtocol"
                });
            }
            let boundary_support = beats.get(first).is_some_and(|b| b.start <= start + 0.005)
                && beats
                    .get(last)
                    .is_some_and(|b| b.start <= end + 0.005 && b.end >= end)
                || beats
                    .get(last.saturating_sub(1))
                    .is_some_and(|b| (b.end - end).abs() <= 0.005)
                    && beats.get(first).is_some_and(|b| b.start <= start + 0.005);
            let continuity = selected
                .windows(2)
                .all(|pair| (pair[1].start - pair[0].end).abs() <= 0.005);
            if !boundary_support || !continuity || selected.len() < 32 {
                reasons.push("insufficientContinuousWindow");
            }
            let corrected_count = selected.iter().filter(|b| !b.reasons.is_empty()).count();
            let fraction = if selected.is_empty() {
                0.0
            } else {
                corrected_count as f64 / selected.len() as f64
            };
            if fraction > 0.03 + 1e-12 {
                reasons.push("excessArtifact");
            }
            if selected.iter().any(|b| b.corrected.is_none()) {
                reasons.push("uncorrectableArtifact");
            }
            let hr = hr_timeline.mean(start, end);
            if !hr.is_some_and(|(_, coverage)| coverage >= 0.95 - 1e-12) {
                reasons.push("insufficientHrCoverage");
            }
            let corrected: Option<Vec<f64>> = selected.iter().map(|b| b.corrected).collect();
            let numerical = if boundary_support && continuity {
                corrected.as_deref().and_then(dfa_window)
            } else {
                None
            };
            if numerical.is_none() && reasons.is_empty() {
                reasons.push("undefinedDfa");
            }
            let annotations:Vec<Value>=selected.iter().enumerate().filter(|(_,b)|!b.reasons.is_empty()).map(|(i,b)|json!({"beatIndex":first+i,"rawRrMs":b.raw,"correctedRrMs":b.corrected,"reasons":b.reasons})).collect();
            windows.push(json!({"index":windows.len(),"centerElapsedSeconds":center,"startElapsedSeconds":start,"endElapsedSeconds":end,
            "segmentIndex":segment.map(|s|s["index"].clone()),"firstBeatIndex":first,"lastBeatIndexExclusive":last,
            "alpha":numerical.as_ref().map(|v|v["alpha"].clone()),"heartRateBpm":hr.map(|h|h.0),"heartRateCoverage":hr.map(|h|h.1),
            "fluctuations":numerical.as_ref().map(|v|v["fluctuations"].clone()),"scales":(4..=16).collect::<Vec<_>>(),
            "logFitIntercept":numerical.as_ref().map(|v|v["logFitIntercept"].clone()),
            "rawBeatCount":selected.len(),"correctedBeatCount":corrected_count,"invalidBeatCount":selected.iter().filter(|b|b.raw<250.0||b.raw>2000.0).count(),
            "missingBeatCount":if continuity {Some(0)} else {None},"artifactFraction":fraction,"artifactAnnotations":annotations,
            "accepted":reasons.is_empty(),"reasons":reasons}));
            center += 5.0;
        }
    }
    windows
}

fn adjacent(a: &Value, b: &Value) -> bool {
    a["segmentIndex"] == b["segmentIndex"]
        && number(a, "centerElapsedSeconds")
            .zip(number(b, "centerElapsedSeconds"))
            .is_some_and(|(a, b)| (b - a - 5.0).abs() < 1e-9)
}

fn select_candidates(windows: &[Value], target: f64, beats: Option<&[Beat]>) -> Vec<Value> {
    let mut candidates = Vec::new();
    let mut i = 0;
    while i < windows.len() {
        let is_inner = |w: &Value| {
            w["accepted"] == true && number(w, "alpha").is_some_and(|a| (0.5..=1.0).contains(&a))
        };
        if !is_inner(&windows[i]) {
            i += 1;
            continue;
        }
        let start = i;
        while i + 1 < windows.len()
            && is_inner(&windows[i + 1])
            && adjacent(&windows[i], &windows[i + 1])
        {
            i += 1;
        }
        let inner_end = i;
        let inner_count = inner_end - start + 1;
        i += 1;
        if inner_count < 3 {
            continue;
        }
        let mut first = start;
        let mut last = inner_end;
        let mut boundaries = Vec::new();
        if start > 0
            && windows[start - 1]["accepted"] == true
            && adjacent(&windows[start - 1], &windows[start])
            && number(&windows[start - 1], "alpha").is_some_and(|a| a > 1.0)
        {
            first = start - 1;
            boundaries.push(first);
        }
        if inner_end + 1 < windows.len()
            && windows[inner_end + 1]["accepted"] == true
            && adjacent(&windows[inner_end], &windows[inner_end + 1])
            && number(&windows[inner_end + 1], "alpha").is_some_and(|a| a < 0.5)
        {
            last = inner_end + 1;
            boundaries.push(last);
        }
        let selected = &windows[first..=last];
        let points: Vec<(f64, f64)> = selected
            .iter()
            .filter_map(|w| Some((number(w, "heartRateBpm")?, number(w, "alpha")?)))
            .collect();
        let alpha_min = points.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
        let alpha_max = points.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
        let hr_min = points.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
        let hr_max = points.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
        let regression = linear_fit(&points);
        let mut reasons = Vec::new();
        if alpha_min > target || alpha_max < target {
            reasons.push("noTargetBracket");
        }
        if regression.is_none() {
            reasons.push("noHrVariance");
        }
        if regression.is_some_and(|(s, _)| s >= 0.0) {
            reasons.push("nonDecliningRelation");
        }
        let crossing = regression
            .and_then(|(s, b)| (s < 0.0).then_some((target - b) / s))
            .filter(|v| v.is_finite());
        if crossing.is_some_and(|v| v < hr_min || v > hr_max) {
            reasons.push("noExtrapolation");
        }
        let first_beat = selected
            .first()
            .and_then(|w| w["firstBeatIndex"].as_u64())
            .unwrap_or(0) as usize;
        let last_beat = selected
            .last()
            .and_then(|w| w["lastBeatIndexExclusive"].as_u64())
            .unwrap_or(0) as usize;
        let unique = beats.and_then(|b| b.get(first_beat..last_beat));
        let corrected = unique.map(|b| b.iter().filter(|b| !b.reasons.is_empty()).count());
        let invalid = unique.map(|b| {
            b.iter()
                .filter(|b| !(250.0..=2000.0).contains(&b.raw))
                .count()
        });
        let missing = unique.and_then(|b| {
            b.windows(2)
                .all(|p| (p[1].start - p[0].end).abs() <= 0.005)
                .then_some(0)
        });
        let fraction = unique.zip(corrected).map(|(b, count)| {
            if b.is_empty() {
                0.0
            } else {
                count as f64 / b.len() as f64
            }
        });
        if fraction.is_some_and(|fraction| fraction > 0.03 + 1e-12) {
            reasons.push("excessArtifact");
        }
        candidates.push(json!({"segmentIndex":windows[start]["segmentIndex"],"firstWindowIndex":first,"lastWindowIndex":last,
            "windowCount":selected.len(),"innerWindowCount":inner_count,"boundaryWindowIndices":boundaries,
            "accepted":reasons.is_empty(),"reasons":reasons,"crossing":crossing,
            "regression":regression.map(|(s,b)|json!({"slope":s,"intercept":b,"hrMin":hr_min,"hrMax":hr_max,"alphaMin":alpha_min,"alphaMax":alpha_max})),
            "rawBeatCount":unique.map(|b|b.len()),"correctedBeatCount":corrected,"invalidBeatCount":invalid,"missingBeatCount":missing,"artifactFraction":fraction}));
    }
    candidates
}

/// Public numerical selector seam for already computed windows. No sensor/physiology
/// verdict: use analyze for full RR eligibility and correction denominators.
pub fn select_decline(windows: &[Value], target: f64) -> Value {
    if target != 0.75 && target != 0.5 {
        return json!({"value":null,"reasons":["unsupportedTarget"],"candidates":[]});
    }
    let candidates = select_candidates(windows, target, None);
    let accepted: Vec<&Value> = candidates
        .iter()
        .filter(|c| c["accepted"] == true)
        .collect();
    let chosen = (accepted.len() == 1).then(|| accepted[0]);
    let reasons = if accepted.len() > 1 {
        vec!["ambiguousCrossing"]
    } else if chosen.is_none() {
        vec!["noEligibleDecline"]
    } else {
        vec![]
    };
    json!({"value":chosen.map(|c|c["crossing"].clone()),"reasons":reasons,"candidates":candidates,"policyVersion":METHOD_VERSION})
}

fn mark_order_conflict(result: &mut Value) {
    if let (Some(a), Some(b)) = (
        number(&result["lt1"]["value"], "heartRateBpm"),
        number(&result["lt2"]["value"], "heartRateBpm"),
    ) && a >= b
    {
        for slot in ["lt1", "lt2"] {
            result[slot]["reasons"]
                .as_array_mut()
                .unwrap()
                .push(json!("conflictingTargetOrder"));
        }
    }
}

/// Activity-local experimental estimates. Segments are observed workload, not labels.
pub fn estimate_activity(normalized: &Value, segments: &[Value]) -> Value {
    let beats = recorded_beats(normalized);
    let windows = beats
        .as_ref()
        .map(|b| compute_windows(normalized, segments, b))
        .unwrap_or_default();
    let input_hash = input_digest(normalized);
    let produce = |target| {
        let candidates = beats
            .as_ref()
            .map(|b| select_candidates(&windows, target, Some(b)))
            .unwrap_or_default();
        let mut reasons = Vec::new();
        if let Err(reason) = beats.as_ref() {
            reasons.push((*reason).to_string());
        }
        if beats.is_ok() && candidates.iter().all(|c| c["accepted"] != true) {
            let mut distinct = BTreeSet::new();
            for candidate in &candidates {
                if let Some(rejected) = candidate["reasons"].as_array() {
                    for reason in rejected {
                        if let Some(reason) = reason.as_str() {
                            distinct.insert(reason.to_string());
                        }
                    }
                }
            }
            if distinct.is_empty() {
                for window in &windows {
                    if let Some(rejected) = window["reasons"].as_array() {
                        for reason in rejected {
                            if let Some(reason) = reason.as_str() {
                                distinct.insert(reason.to_string());
                            }
                        }
                    }
                }
            }
            if windows.is_empty() {
                distinct.insert("insufficientContinuousWindow".into());
            }
            if distinct.is_empty() {
                distinct.insert("noTargetBracket".into());
            }
            reasons.extend(distinct);
        }
        target_result(
            target,
            normalized,
            &input_hash,
            &windows,
            candidates,
            reasons,
        )
    };
    let mut result = json!({"lt1":produce(0.75),"lt2":produce(0.5)});
    mark_order_conflict(&mut result);
    result
}

fn comparability(normalized: &Value) -> Option<String> {
    let sensors = normalized["sensors"].as_array()?;
    if sensors.is_empty() {
        return None;
    }
    let mut identities: Vec<String> = sensors
        .iter()
        .filter(|sensor| normalized["softwareFixture"] == true || sensor["rrSourceProven"] == true)
        .map(|sensor| {
            let mut identity = serde_json::Map::new();
            for key in [
                "manufacturer",
                "product",
                "model",
                "type",
                "sensorType",
                "recordingPath",
                "sensorId",
                "serialNumber",
            ] {
                if let Some(v) = sensor.get(key).filter(|v| !v.is_null()) {
                    identity.insert(key.into(), v.clone());
                }
            }
            Value::Object(identity).to_string()
        })
        .filter(|v| v != "{}")
        .collect();
    if identities.is_empty() {
        return None;
    }
    identities.sort();
    Some(format!(
        "{:x}",
        Sha256::digest(
            json!([
                identities,
                "observed-continuous-progressive",
                METHOD_VERSION
            ])
            .to_string()
            .as_bytes()
        )
    ))
}

fn event_end(evidence: &Value) -> Option<DateTime<chrono::FixedOffset>> {
    let outer = DateTime::parse_from_rfc3339(evidence["endTime"].as_str()?).ok()?;
    let inner = DateTime::parse_from_rfc3339(evidence["normalized"]["endTime"].as_str()?).ok()?;
    if outer != inner || elapsed_duration(&evidence["normalized"]).is_none() {
        return None;
    }
    if evidence.get("startTime").is_some()
        && evidence["startTime"] != evidence["normalized"]["startTime"]
    {
        return None;
    }
    Some(outer)
}

struct HistoricalEvidence<'a> {
    source: &'a Value,
    age_days: f64,
    recording_scope: Option<String>,
    analysis: std::borrow::Cow<'a, Value>,
    input_hash: Option<String>,
}

/// Owner-scoped evidence is supplied by core; the pure engine independently enforces
/// event cutoff, source bounds, seven-day freshness, duplicate independence and sensor scope.
/// It never uses computation/upload time or learned calibration from another activity.
pub fn estimate_history(cutoff: &str, evidence: &[Value]) -> Value {
    let cutoff_time = DateTime::parse_from_rfc3339(cutoff).ok();
    let mut rows: Vec<(&Value, DateTime<chrono::FixedOffset>)> = evidence
        .iter()
        .filter_map(|row| {
            let end = event_end(row)?;
            if cutoff_time.is_none_or(|cutoff| end > cutoff)
                || row["sourceUnavailable"] == true
                || row["deleted"] == true
            {
                return None;
            }
            Some((row, end))
        })
        .collect();
    rows.sort_by(|(a, ta), (b, tb)| {
        tb.cmp(ta).then_with(|| {
            a["activityId"]
                .as_str()
                .unwrap_or("")
                .cmp(b["activityId"].as_str().unwrap_or(""))
        })
    });
    let mut prepared = Vec::new();
    let configuration_hash = method(0.75)["configurationHash"].clone();
    for (row, end) in rows {
        let analysis = if row["analysis"]["thresholds"].is_object() {
            std::borrow::Cow::Borrowed(&row["analysis"])
        } else {
            std::borrow::Cow::Owned(super::analysis::analyze(&row["normalized"]))
        };
        let age = cutoff_time
            .map(|cutoff| (cutoff - end).num_milliseconds() as f64 / 86_400_000.0)
            .unwrap_or(f64::INFINITY);
        let input_hash = historical_input_digest(row);
        let recording_scope = comparability(&row["normalized"]);
        prepared.push(HistoricalEvidence {
            source: row,
            age_days: age,
            recording_scope,
            analysis,
            input_hash,
        });
    }
    // Each target selects one source, so duplicate members never add weight.
    // Validate first; a stale representative must not hide valid cache.
    let produce = |slot: &str, target: f64| {
        let expected_id = if target == 0.75 {
            "running-dfa-a1-075"
        } else {
            "running-dfa-a1-050"
        };
        let verified = |entry: &HistoricalEvidence| {
            let result = &entry.analysis["thresholds"][slot];
            entry.age_days <= 7.0
                && entry.input_hash.is_some()
                && matches!(
                    result["status"].as_str(),
                    Some("estimated" | "low_confidence" | "insufficient_data")
                )
                && result["method"]["id"] == expected_id
                && result["method"]["version"] == METHOD_VERSION
                && result["method"]["configurationHash"] == configuration_hash
                && result["trace"]["inputRevision"] == entry.source["normalized"]["sourceRevision"]
                && result["trace"]["inputHash"] == json!(entry.input_hash)
                && result["trace"]["evidenceCutoff"] == entry.source["normalized"]["endTime"]
        };
        // Stop at the first proven sensor boundary or verified timed-RR context.
        // Unknown HR-only/stale data cannot define RR context or promote a group.
        let reference = prepared
            .iter()
            .find(|entry| {
                entry.recording_scope.is_some()
                    || verified(entry)
                        && entry.analysis["thresholds"][slot]["trace"]["windows"]
                            .as_array()
                            .is_some_and(|windows| {
                                windows.iter().any(|window| {
                                    window["rawBeatCount"]
                                        .as_u64()
                                        .is_some_and(|count| count > 0)
                                })
                            })
            })
            .or_else(|| prepared.iter().find(|entry| verified(entry)));
        let scope = reference.and_then(|entry| entry.recording_scope.as_ref());
        let evaluated = |entry: &HistoricalEvidence| {
            verified(entry)
                && (scope.is_some_and(|scope| entry.recording_scope.as_ref() == Some(scope))
                    || reference.is_some_and(|reference| {
                        reference.source["activityId"] == entry.source["activityId"]
                    }))
        };
        let current = prepared.iter().find(|entry| {
            evaluated(entry)
                && matches!(
                    entry.analysis["thresholds"][slot]["status"].as_str(),
                    Some("estimated" | "low_confidence")
                )
                && number(&entry.analysis["thresholds"][slot]["value"], "heartRateBpm").is_some()
        });
        let attempted = prepared.iter().find(|entry| evaluated(entry));
        if let Some(entry) = current.or(attempted) {
            let row = entry.source;
            let age = entry.age_days;
            let mut result = entry.analysis["thresholds"][slot].clone();
            result["evidence"] = json!({"independentActivityCount":usize::from(result["value"].is_object()),"activityId":row["activityId"],"observationGroupId":row["observationGroupId"],
                "startTime":row["startTime"],"endTime":row["endTime"],"ageDays":age});
            result["trace"]["evidenceCutoff"] = json!(cutoff);
            result["trace"]["inputManifestId"] = row["manifestId"].clone();
            result["freshness"] = json!({"stale":false,"ageDays":age});
            result
        } else {
            let empty = json!({});
            let normalized = reference
                .or_else(|| prepared.first())
                .map(|entry| &entry.source["normalized"])
                .unwrap_or(&empty);
            let reason = if cutoff_time.is_none() {
                "invalidEvidenceCutoff"
            } else {
                "noRecentComparableEvidence"
            };
            let input_hash = input_digest(normalized);
            let mut result = target_result(
                target,
                normalized,
                &input_hash,
                &[],
                vec![],
                vec![reason.into()],
            );
            result["trace"]["evidenceCutoff"] = json!(cutoff);
            result["freshness"] = json!({"stale":false,"ageDays":null});
            result
        }
    };
    let mut result = json!({"evidenceCutoff":cutoff,"computedAt":Utc::now().to_rfc3339(),"lt1":produce("lt1",0.75),"lt2":produce("lt2",0.5),
        "policyVersion":METHOD_VERSION,"engineStatus":"experimental","researchBlocked":true});
    mark_order_conflict(&mut result);
    result
}

fn leaf_fields(value: &Value, keys: &[&str]) -> Value {
    if value.is_null() {
        return Value::Null;
    }
    let mut projected = serde_json::Map::new();
    for key in keys {
        if let Some(value) = value.get(*key).filter(|v| {
            !v.is_object()
                && v.as_array()
                    .is_none_or(|items| items.iter().all(|v| !v.is_object() && !v.is_array()))
        }) {
            projected.insert((*key).into(), value.clone());
        }
    }
    Value::Object(projected)
}

/// Closed selected-only aggregate projection. No source identities, unselected
/// windows/samples/laps, history arrays, last-good payloads or device identities.
pub fn export_projection(result: &Value) -> Value {
    if result.is_null() {
        return Value::Null;
    }
    if result.get("lt1").is_some() || result.get("lt2").is_some() {
        let mut projection = json!({"lt1":export_projection(&result["lt1"]),"lt2":export_projection(&result["lt2"])});
        for key in [
            "evidenceCutoff",
            "computedAt",
            "policyVersion",
            "engineStatus",
            "researchBlocked",
        ] {
            if let Some(value) = result.get(key) {
                projection[key] = value.clone();
            }
        }
        return projection;
    }
    let mut projection = leaf_fields(
        result,
        &[
            "status",
            "engineStatus",
            "researchBlocked",
            "requiredContextUnprovable",
            "targetDefinition",
            "reasons",
            "contextUnverified",
            "limits",
        ],
    );
    for (key, fields) in [
        ("method", &["id", "version", "configurationHash"][..]),
        ("value", &["heartRateBpm"][..]),
        ("uncertainty", &["interval", "reason"][..]),
        ("freshness", &["stale", "ageDays"][..]),
        (
            "evidence",
            &[
                "independentActivityCount",
                "startTime",
                "endTime",
                "ageDays",
            ][..],
        ),
    ] {
        if let Some(value) = result.get(key) {
            projection[key] = leaf_fields(value, fields);
        }
    }
    projection["suggestions"] = json!(
        result["suggestions"]
            .as_array()
            .map(|suggestions| suggestions
                .iter()
                .map(|v| leaf_fields(
                    v,
                    &[
                        "optional",
                        "purpose",
                        "prerequisites",
                        "steps",
                        "duration",
                        "rationale",
                        "expectedData",
                        "safety",
                        "limits"
                    ]
                ))
                .collect::<Vec<_>>())
            .unwrap_or_default()
    );
    let source_trace = &result["trace"];
    let mut trace = leaf_fields(
        source_trace,
        &[
            "crossing",
            "independentActivityCount",
            "evidenceCutoff",
            "computedAt",
        ],
    );
    trace["parameters"] = leaf_fields(
        &source_trace["parameters"],
        &[
            "version",
            "segmentVersion",
            "lambda",
            "samplingConvention",
            "scales",
            "overlap",
            "scaleTail",
            "localTrendOrder",
            "fluctuation",
            "fit",
            "windowSeconds",
            "stepSeconds",
            "windowClosure",
            "windowEnumeration",
            "nativeRrTimingMethod",
            "requiredAnchorFields",
            "floatingBoundToleranceSeconds",
            "inputProjectionVersion",
            "workloadBlockSeconds",
            "steadySpeedCvCeiling",
            "minimumProgressiveSpeedSlopeMpsPerSecond",
            "surgeRelativeChange",
            "minimumSpeedCoverage",
            "minimumRepeatedSurges",
            "artifactCeiling",
            "artifactPolicy",
            "selector",
            "minimumInnerWindows",
            "minimumHrCoverage",
            "maximumSampleGapSeconds",
            "maximumRrContinuityErrorSeconds",
            "lookbackDays",
            "comparability",
        ],
    );
    trace["counts"] = leaf_fields(
        &source_trace["counts"],
        &[
            "candidate",
            "accepted",
            "rejected",
            "windows",
            "acceptedWindows",
            "rejectedWindows",
        ],
    );
    trace["regression"] = leaf_fields(
        &source_trace["regression"],
        &[
            "slope",
            "intercept",
            "hrMin",
            "hrMax",
            "alphaMin",
            "alphaMax",
        ],
    );
    trace["lineage"] = json!(
        "Full source/window lineage is available only in the owner-scoped web evidence view."
    );
    projection["trace"] = trace;
    projection
}
