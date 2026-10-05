"""CC0 synthetic software comparison, never physiological validation.

Research-only dependencies: numpy==2.2.6 scipy==1.15.3 nolds==0.6.3.
No Rust implementation is imported. SciPy solves the SPD band system; the
installed MIT nolds measures.py computes DFA and NumPy fits HR crossings.
"""
from __future__ import annotations

import hashlib
import importlib.metadata
import importlib.util
import json
from pathlib import Path

import numpy as np
from scipy.linalg import solveh_banded

ROOT = Path(__file__).parent
for package, version in [("numpy", "2.2.6"), ("scipy", "1.15.3"), ("nolds", "0.6.3")]:
    assert importlib.metadata.version(package) == version, (package, version)
SOURCE = Path(importlib.metadata.distribution("nolds").locate_file("nolds/measures.py"))
assert hashlib.sha256(SOURCE.read_bytes()).hexdigest() == "53a59a5c24d737539f584d9f86c09228852768308a0539b6cbd7d3ead3eae52a"
# Avoid nolds.__init__ importing unrelated dataset resources on Python 3.12.
SPEC = importlib.util.spec_from_file_location("pinned_nolds_measures", SOURCE)
NOLDS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(NOLDS)


def dfa(rr):
    rr = np.asarray(rr, dtype=np.float64)
    band = np.zeros((3, len(rr)))
    band[2] = 1.0
    for row in range(len(rr) - 2):
        coefficients = [1.0, -2.0, 1.0]
        for i in range(3):
            for j in range(i, 3):
                band[2 + i - j, row + j] += 500.0**2 * coefficients[i] * coefficients[j]
    detrended = rr - solveh_banded(band, rr, lower=False)
    alpha, (_, log_f, fit) = NOLDS.dfa(
        detrended, nvals=list(range(4, 17)), overlap=False, order=1,
        fit_trend="poly", fit_exp="poly", debug_data=True,
    )
    return float(alpha), np.exp(log_f), float(fit[1])


def quantize(source):
    source = json.loads(json.dumps(source))
    previous = 0
    for beat in source["rr"]["intervals"]:
        tick = int(np.floor(beat["endElapsedSeconds"] * 1024 + 0.5))
        beat.update(rrMs=(tick - previous) * 1000 / 1024,
                    startElapsedSeconds=previous / 1024,
                    endElapsedSeconds=tick / 1024, elapsedSeconds=tick / 1024)
        previous = tick
    for sample in source["samples"]:
        sample["heartRateBpm"] = float(np.floor(sample["heartRateBpm"] + 0.5))
    return source


def pipeline(source):
    beats = source["rr"]["intervals"]
    samples = source["samples"]
    times = np.array([s["elapsedSeconds"] for s in samples])
    hrs = np.array([s["heartRateBpm"] for s in samples])
    windows = []
    for center in np.arange(60, source["summary"]["elapsedTimeSeconds"] - 60 + 0.001, 5):
        left, right = center - 60, center + 60
        first = next((b for b in beats if b["endElapsedSeconds"] > left), None)
        boundary = next((b for b in beats if b["endElapsedSeconds"] > right), None)
        supported = (first is not None and first["startElapsedSeconds"] <= left + 0.005
                     and boundary is not None and boundary["startElapsedSeconds"] <= right + 0.005)
        values = [b["rrMs"] for b in beats if left < b["endElapsedSeconds"] <= right]
        numerical = dfa(values) if supported else None
        integration_times = np.concatenate(([left], times[(times > left) & (times < right)], [right]))
        hr = float(np.trapezoid(np.interp(integration_times, times, hrs), integration_times) / 120)
        windows.append({"alpha":numerical[0] if numerical else None,
                        "fluctuations":numerical[1] if numerical else None,
                        "heartRateBpm":hr, "accepted":supported})
    sections = []
    current = []
    for i, window in enumerate(windows):
        if window["accepted"] and 0.5 <= window["alpha"] <= 1.0:
            current.append(i)
        elif current:
            sections.append(current)
            current = []
    if current:
        sections.append(current)
    candidates = []
    for section in sections:
        if len(section) < 3:
            continue
        indices = list(section)
        if section[0] > 0 and windows[section[0] - 1]["accepted"] and windows[section[0] - 1]["alpha"] > 1:
            indices.insert(0, section[0] - 1)
        if section[-1] + 1 < len(windows) and windows[section[-1] + 1]["accepted"] and windows[section[-1] + 1]["alpha"] < 0.5:
            indices.append(section[-1] + 1)
        x = np.array([windows[i]["heartRateBpm"] for i in indices])
        y = np.array([windows[i]["alpha"] for i in indices])
        slope, intercept = np.polyfit(x, y, 1)
        values = {}
        for slot, target in [("lt1", 0.75), ("lt2", 0.5)]:
            crossing = float((target - intercept) / slope)
            values[slot] = crossing if slope < 0 and min(y) <= target <= max(y) and min(x) <= crossing <= max(x) else None
        candidates.append(values)
    results = {}
    for slot in ["lt1", "lt2"]:
        accepted = [c[slot] for c in candidates if c[slot] is not None]
        results[slot] = accepted[0] if len(accepted) == 1 else None
    return windows, results


def verify():
    fixture = json.loads((ROOT / "runs-engine-dfa-oracle.json").read_text())
    alpha, fluctuations, _ = dfa(fixture["rrMs"])
    assert abs(alpha - fixture["reference"]["alpha"]) < 1e-9
    assert np.max(np.abs(fluctuations - fixture["reference"]["fluctuations"])) < 1e-8
    for suffix in ["", "-positive"]:
        source = json.loads((ROOT / f"runs-engine-progressive{suffix}.json").read_text())
        golden = json.loads((ROOT / f"runs-engine-progressive{suffix}-oracle.json").read_text())
        for name, data in [("raw", source), ("encodedCounterAndIntegerHr", quantize(source))]:
            windows, results = pipeline(data)
            for actual, expected in zip(windows, golden[name]["windows"]):
                if actual["alpha"] is not None:
                    assert abs(actual["alpha"] - expected["alpha"]) < 1e-9
                    assert np.max(np.abs(actual["fluctuations"] - expected["fluctuations"])) < 1e-8
            for slot, value in results.items():
                expected = golden[name]["results"][slot]["value"]
                assert value is None if expected is None else abs(value - expected) < 1e-8
            print(f"progressive{suffix} {name}: {results}")
    print("Pinned independent SciPy/nolds agreement passed; no physiological validation claimed.")


if __name__ == "__main__":
    verify()
