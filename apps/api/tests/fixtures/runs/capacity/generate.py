#!/usr/bin/env python3
# SPDX-License-Identifier: CC0-1.0
"""Generate deterministic CC0 synthetic Garmin-profile FIT activities."""
from __future__ import annotations

import argparse
import hashlib
import json

import struct
from datetime import datetime, timedelta, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent
FIT_EPOCH = datetime(1989, 12, 31, tzinfo=timezone.utc)
PROFILE_VERSION = 21202  # FIT profile 21.202, matching MIT fitparser 0.11.0.
CRC_TABLE = (
    0x0000, 0xCC01, 0xD801, 0x1400, 0xF001, 0x3C00, 0x2800, 0xE401,
    0xA001, 0x6C00, 0x7800, 0xB401, 0x5000, 0x9C01, 0x8801, 0x4400,
)

# FIT base types: enum, uint8, sint32, uint16, uint32, uint32z.
ENUM, UINT8, SINT32, UINT16, UINT32, UINT32Z = 0x00, 0x02, 0x85, 0x84, 0x86, 0x8C


def crc16(data: bytes) -> int:
    crc = 0
    for byte in data:
        tmp = CRC_TABLE[crc & 0xF]
        crc = ((crc >> 4) & 0x0FFF) ^ tmp ^ CRC_TABLE[byte & 0xF]
        tmp = CRC_TABLE[crc & 0xF]
        crc = ((crc >> 4) & 0x0FFF) ^ tmp ^ CRC_TABLE[(byte >> 4) & 0xF]
    return crc


def definition(local: int, global_number: int, fields: list[tuple[int, int, int]]) -> bytes:
    return (
        bytes((0x40 | local, 0, 0))
        + struct.pack("<H", global_number)
        + bytes((len(fields),))
        + b"".join(bytes(field) for field in fields)
    )


def message(local: int, values: list[bytes]) -> bytes:
    return bytes((local,)) + b"".join(values)


def pack_field(code: str, value: int) -> bytes:
    return struct.pack("<" + code, value)


def timestamp(dt: datetime) -> int:
    return int((dt - FIT_EPOCH).total_seconds())


def semicircles(degrees: float) -> int:
    return round(degrees * (2**31) / 180.0)


def file_id(start: datetime, serial: int) -> bytes:
    fields = [(0, 1, ENUM), (1, 2, UINT16), (2, 2, UINT16), (3, 4, UINT32Z), (4, 4, UINT32), (5, 2, UINT16)]
    body = definition(0, 0, fields)
    body += message(0, [
        pack_field("B", 4),                 # file type: activity
        pack_field("H", 1),                 # manufacturer: Garmin profile ID
        pack_field("H", 0),                 # unknown product; no real device claim
        pack_field("I", serial),            # synthetic privacy canary only
        pack_field("I", timestamp(start)),
        pack_field("H", 0),
    ])
    return body


def timer_event(local: int, when: datetime, event_type: int) -> bytes:
    # Global message 21 (event): timestamp 253, event 0 (timer), event_type 1.
    fields = [(253, 4, UINT32), (0, 1, ENUM), (1, 1, ENUM)]
    return definition(local, 21, fields) + message(local, [
        pack_field("I", timestamp(when)), pack_field("B", 0), pack_field("B", event_type),
    ])


def record_definition(include_sensors: bool) -> tuple[bytes, list[tuple[int, int, int]]]:
    fields = [(0, 4, SINT32), (1, 4, SINT32)]
    if include_sensors:
        fields += [(3, 1, UINT8)]
    fields += [(4, 1, UINT8), (5, 4, UINT32), (6, 2, UINT16)]

    if include_sensors:
        fields += [(7, 2, UINT16)]
    fields += [(253, 4, UINT32)]
    return definition(2, 20, fields), fields


def session(start: datetime, end: datetime, duration: int, distance_cm: int, include_sensors: bool) -> bytes:
    fields = [(2, 4, UINT32), (5, 1, ENUM), (6, 1, ENUM), (7, 4, UINT32), (8, 4, UINT32), (9, 4, UINT32)]
    if include_sensors:
        fields += [(13, 2, UINT16), (14, 2, UINT16), (15, 1, UINT8), (16, 1, UINT8), (17, 1, UINT8), (18, 1, UINT8), (19, 2, UINT16), (20, 2, UINT16)]
    fields += [(253, 4, UINT32)]
    body = definition(3, 18, fields)
    values = [
        pack_field("I", timestamp(start)), pack_field("B", 1), pack_field("B", 0),
        pack_field("I", duration * 1000), pack_field("I", duration * 1000),
        pack_field("I", distance_cm),
    ]
    if include_sensors:
        values += [pack_field("H", 3000), pack_field("H", 3000), pack_field("B", 147), pack_field("B", 165),
                   pack_field("B", 180), pack_field("B", 190), pack_field("H", 220), pack_field("H", 260)]
    values.append(pack_field("I", timestamp(end)))
    return body + message(3, values)


def activity(end: datetime, duration: int) -> bytes:
    fields = [(0, 4, UINT32), (1, 4, UINT32), (2, 2, UINT16), (3, 1, ENUM), (4, 1, ENUM), (5, 1, ENUM)]
    return definition(4, 34, fields) + message(4, [
        pack_field("I", timestamp(end)), pack_field("I", duration * 1000), pack_field("H", 1),
        pack_field("B", 0), pack_field("B", 26), pack_field("B", 1),
    ])


def generate(name: str, count: int, start_day: str, duration_seconds: int,
             distance_m: float, include_sensors: bool, serial: int) -> dict[str, object]:
    start = datetime.fromisoformat(start_day).replace(tzinfo=timezone.utc)
    end = start + timedelta(seconds=duration_seconds)
    total_cm = round(distance_m * 100)
    data = bytearray(file_id(start, serial))
    data += timer_event(1, start, 0)  # timer start
    record_def, fields = record_definition(include_sensors)
    data += record_def
    lat = semicircles(12.345678)
    lon = semicircles(-98.765432)
    for i in range(count):
        sec = round(i * (duration_seconds - 1) / (count - 1)) if count > 1 else 0
        rec_time = timestamp(start) + sec
        distance = round(i * total_cm / (count - 1)) if count > 1 else 0
        vals = [pack_field("i", lat), pack_field("i", lon)]
        if include_sensors:
            vals += [pack_field("B", 130 + (i * 17) % 36)]
        # Record field4 is cycles/min, not steps/min: actual170/177/184 ->340/354/368 steps/min.
        vals += [pack_field("B", 170 + (i * 7) % 21), pack_field("I", distance), pack_field("H", 3000)]
        if include_sensors:
            vals += [pack_field("H", 180 + (i * 13) % 81)]
        vals += [pack_field("I", rec_time)]
        assert len(vals) == len(fields)
        data += message(2, vals)
    data += timer_event(1, end, 9)  # timer stop_all
    data += session(start, end, duration_seconds, total_cm, include_sensors)
    data += activity(end, duration_seconds)

    data_size = len(data)
    header_without_crc = struct.pack("<BBHI4s", 14, 0x20, PROFILE_VERSION, data_size, b".FIT")
    header = header_without_crc + struct.pack("<H", crc16(header_without_crc))
    raw = header + data + struct.pack("<H", crc16(data))
    path = ROOT / name
    path.write_bytes(raw)
    digest = hashlib.sha256(raw).hexdigest()
    assert len(raw) < 20 * 1024 * 1024
    assert raw[8:12] == b".FIT"
    assert int.from_bytes(raw[12:14], "little") == crc16(raw[:12])
    assert int.from_bytes(raw[-2:], "little") == crc16(raw[14:-2])
    assert count > 0 and total_cm > 0
    return {
        "file": name,
        "bytes": len(raw),
        "sha256": digest,
        "records": count,
        "first_record": {"timestamp_utc": start.isoformat(), "distance_m": 0.0},
        "last_record": {
            "timestamp_utc": (start + timedelta(seconds=duration_seconds - 1)).isoformat(),
            "distance_m": total_cm / 100.0,
        },
        "session": {
            "start_utc": start.isoformat(), "end_utc": end.isoformat(),
            "duration_s": duration_seconds, "elapsed_time_ms": duration_seconds * 1000,
            "timer_time_ms": duration_seconds * 1000, "distance_m": total_cm / 100.0,
            "sport": "running", "sessions": 1,
        },
        "sensors": {"heart_rate": include_sensors, "power": include_sensors, "native_cadence": True,
                    "speed": True, "rr_intervals": False},
        "synthetic_privacy_canaries": {
            "coordinates_degrees": [12.345678, -98.765432],
            "device_serial_u32": serial,
            "only_synthetic": True,
        },
    }


def main() -> None:
    global ROOT
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT,
                        help="Generated FIT/manifest directory; use a temporary directory to avoid committing large FITs.")
    ROOT = parser.parse_args().output.resolve()
    ROOT.mkdir(parents=True, exist_ok=True)
    generated = [
        generate("chart_1h_3600.fit", 3600, "2026-10-01T06:00:00", 3600, 10800.0, True, 0x5A17C0DE),
        generate("full_export_100k.fit", 100000, "2026-10-03T06:00:00", 3600, 10800.0, True, 0x5A17C0DF),
        generate("no_hr_power_600.fit", 600, "2026-10-04T06:00:00", 600, 1800.0, False, 0x5A17C0E0),
    ]
    manifest = {
        "license": "CC0-1.0",
        "provenance": "Entirely synthetic bytes and values; no user/device FIT data. FIT profile IDs and CRC algorithm follow the FIT protocol/profile and MIT fitparser 0.11.0 source.",
        "generator": "generate.py",
        "determinism": "Fixed parameters, no runtime randomness; rerunning replaces only these generated fixture names.",
        "profile_version": "21.202",
        "seed": "Fixed deterministic activity parameters; serial canaries 0x5A17C0DE..E0.",
        "units": {"distance": "m", "speed": "m/s", "elapsed_time": "ms in FIT fields; seconds in summary", "coordinates": "degrees in manifest; FIT semicircles on wire", "heart_rate": "bpm", "power": "W", "cadence": "cycles/min synthetic native record cadence; canonical running steps/min = cycles/min * 2"},
        "cadence": {"fit_message_number": 20, "fit_field_number": 4,
                    "encoded_cycle_per_min_envelope": [170, 190],
                    "canonical_step_per_min_envelope": [340, 380],
                    "encoded_cycle_per_min_values": [170, 177, 184],
                    "canonical_step_per_min_values": [340, 354, 368],
                    "arithmetic": "170 + (recordIndex * 7) % 21 cycles/min; only three discrete values occur. Running steps/min multiply native cycles/min by2."},
        "fixtures": generated,
    }
    (ROOT / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps({"directory": str(ROOT), "fixtures": [{"file": x["file"], "bytes": x["bytes"], "sha256": x["sha256"], "records": x["records"]} for x in generated]}, indent=2))


if __name__ == "__main__":
    main()
