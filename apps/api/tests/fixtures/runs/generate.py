#!/usr/bin/env python3
# SPDX-License-Identifier: CC0-1.0
# Original synthetic FIT encoder. Standard library only; no SDK or activity input.
"""Regenerate FIT fixtures and arithmetic oracles beside this script.

Run: python3 apps/api/tests/fixtures/runs/generate.py
All inputs are literal synthetic values. No randomness, clock, network, or SDK.
"""
import hashlib
import io
import json
import struct
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent
VERSION = 1
SEED = "runs-synthetic-v1"
T = 1_000_000_000
U8, U16, U32, BYTE = 2, 0x84, 0x86, 0x0D
FILES = {}


def crc(content):
    value = 0
    for byte in content:
        value ^= byte
        for _ in range(8):
            value = (value >> 1) ^ (0xA001 if value & 1 else 0)
    return value


def definition(local, global_number, fields, developers=(), architecture=0):
    result = bytes([0x40 | local | (0x20 if developers else 0), 0, architecture])
    result += struct.pack(">H" if architecture else "<H", global_number) + bytes([len(fields)])
    result += b"".join(bytes(field) for field in fields)
    if developers:
        result += bytes([len(developers)]) + b"".join(bytes(field) for field in developers)
    return result


def data(local, payload):
    return bytes([local]) + payload


def message(local, global_number, fields, payload, developers=()):
    return definition(local, global_number, fields, developers) + data(local, payload)


def wrap(body, header_size=14, manufacturer=1, full_identity=False):
    fields = [(0, 1, 0), (1, 2, U16)]
    payload = struct.pack("<BH", 4, manufacturer)
    if full_identity:
        fields += [(2, 2, U16), (3, 4, 0x8C), (4, 4, U32)]
        payload += struct.pack("<HII", 65534, 123456, T)
    body = message(15, 0, fields, payload) + body
    header = struct.pack("<BBHI4s", header_size, 0x20, 21217, len(body), b".FIT")
    if header_size == 14:
        header += struct.pack("<H", crc(header))
    return header + body + struct.pack("<H", crc(header + body))


def pack12(*values):
    assert all(0 <= value < 4096 for value in values)
    return sum(value << (12 * index) for index, value in enumerate(values)).to_bytes(
        (12 * len(values) + 7) // 8, "little"
    )


def add(name, content, features, expected):
    FILES[name] = (content, features, expected)


def conformance_cases():
    body = message(0, 20, [(253, 4, U32), (3, 1, U8), (6, 2, U16),
                           (240, 2, U16), (241, 1, U8)],
                   struct.pack("<IBHHB", T, 0, 0, 54321, 255))
    add("native_unknown.fit", wrap(body), ["unknown_fields", "zero", "invalid_sentinel"],
        {"record": {"timestamp_fit": T, "heart_rate": 0, "speed_mps": 0,
                    "unknown_240": 54321, "unknown_241": None}, "cadence_present": False})
    fields = [(0, 6, U16), (1, 1, U8), (2, 8, 0x8F), (3, 8, 0x8E),
              (4, 1, 0x0A), (5, 4, U16)]
    body = message(0, 65280, fields,
                   struct.pack("<3HBQqB2H", 0, 65535, 42, 255,
                               9007199254740993, -9007199254740993, 0, 65535, 65535))
    add("unknown_arrays_integers.fit", wrap(body), ["unknown_message", "arrays", "unsafe_integers", "invalid_zero"],
        {"global_number": 65280, "fields": {"0": [0, None, 42], "1": None,
         "2": "9007199254740993", "3": "-9007199254740993", "4": None, "5": [None, None]},
         "invalid_rules": {"uint8": 255, "uint16": 65535, "uint8z": 0}})
    fields = [(0, 12, 0x88), (1, 8, 0x89), (2, 8, 7), (3, 3, BYTE), (4, 1, BYTE), (5, 8, 7)]
    # Canonical literal IEEE-754 bytes: float32 1.25, quiet NaN, +Inf; float64 +Inf.
    body = message(0, 65281, fields, bytes.fromhex("0000a03f0000c07f0000807f000000000000f07f")
                   + b"abc\0\0\0\0\0" + bytes([0, 255, 17, 255]) + b"\0" * 8)
    add("unknown_nonfinite_bytes.fit", wrap(body),
        ["unknown_message", "nonfinite", "string", "invalid_byte", "invalid_string"],
        {"global_number": 65281, "fields": {"0": [1.25, None, None], "1": None,
                                           "2": "abc", "3": [0, None, 17], "4": None, "5": None},
         "raw_values": {"5": ""}, "field_validity": {"5": "invalid"},
         "raw_float32_hex": "0000a03f0000c07f0000807f", "raw_float64_hex": "000000000000f07f",
         "invalid_rules": "Nonfinite numeric values serialize as null; ordinary FIT byte255 is invalid. String leading0x00 is invalid with rawValue empty string. Packed known composites retain original packed bytes."})
    body = message(0, 20, [(253, 4, U32), (3, 1, U8)], struct.pack("<IB", 1000000031, 120))
    body += definition(1, 20, [(3, 1, U8)]) + bytes([0xA0, 121, 0xA2, 122])
    add("compressed_timestamp.fit", wrap(body), ["compressed_timestamp", "five_bit_rollover"],
        {"timestamps_fit": [1000000031, 1000000032, 1000000034], "heart_rate_bpm": [120, 121, 122],
         "arithmetic": "Previous low five bits 31; offsets 0 then 2 reconstruct next 32-second window."})
    body = developer_definitions()
    body += message(2, 20, [(253, 4, U32), (3, 1, U8)],
                    struct.pack("<IBHH", T, 120, 248, 480), [(3, 2, 0), (3, 2, 1)])
    add("developer_collisions.fit", wrap(body), ["developer_identity", "developer_name_collision", "scale_offset"],
        {"native_heart_rate_bpm": 120, "developers": [
            {"developer_data_index": 0, "field_definition_number": 3, "field_name": "heart_rate",
             "raw": 248, "scale": 2, "offset": 3, "value": 121,
             "developer_id_hex": bytes(range(16)).hex(), "application_id_hex": bytes(range(16, 32)).hex()},
            {"developer_data_index": 1, "field_definition_number": 3, "field_name": "heart_rate",
             "raw": 480, "scale": 4, "offset": -2, "value": 122,
             "developer_id_hex": bytes(range(32, 48)).hex(), "application_id_hex": bytes(range(48, 64)).hex()}],
         "arithmetic": "Physical developer value = raw / scale - offset; neither overwrites native field3."})
    body = message(0, 20, [(253, 4, U32), (6, 2, U16), (8, 3, BYTE)],
                   struct.pack("<IH", T, 3000) + pack12(300, 160))
    add("components.fit", wrap(body), ["native_components", "stored_speed_conflict"],
        {"stored_speed_mps": 3, "component_speed_mps": 3, "component_distance_m": 10,
         "packed_hex": pack12(300, 160).hex(),
         "arithmetic": "record8 low12 speed300/100=3m/s; next12 distance160/16=10m."})
    body = definition(0, 20, [(253, 4, U32), (3, 1, U8)])
    body += definition(1, 132, [(253, 4, U32), (6, 2, U8), (9, 4, U32), (10, 3, BYTE)])
    body += definition(2, 78, [(0, 6, U16)])
    body += data(0, struct.pack("<IB", T, 100))
    body += data(1, struct.pack("<IBBI", T, 120, 121, 102400) + pack12(1024, 2048))
    body += data(2, struct.pack("<3H", 800, 65535, 810))
    body += data(0, struct.pack("<IB", T + 1, 101))
    body += data(1, struct.pack("<IBBI", T + 1, 122, 123, 104448) + pack12(3072, 0))
    add("sparse_hr_stress.fit", wrap(body), ["hr_and_hrv", "missing_fractional_anchor", "packed_rollover"],
        {"hrv_intervals_ms": [800, 810], "record_hr_bpm": [100, 101],
         "packed_event_ticks": [1024, 2048, 3072, 0], "packed_timestamp_anchor_complete": False})
    body = message(0, 20, [(253, 4, U32), (3, 1, U8)], struct.pack("<IB", T, 100))
    body += message(1, 132, [(253, 4, U32), (0, 2, U16), (6, 1, U8), (9, 4, U32)],
                    struct.pack("<IHBI", T, 0, 120, 102400))
    body += message(2, 78, [(0, 6, U16)], struct.pack("<3H", 800, 65535, 810))
    body += data(0, struct.pack("<IB", T + 1, 101))
    body += message(1, 132, [(6, 2, U8), (10, 3, BYTE)], bytes([121, 122]) + pack12(1024, 2048))
    add("hr_rr_order.fit", wrap(body), ["hr_rr_order", "hrv_invalid_element", "packed_hr"],
        {"heart_rate_source_order_bpm": [100, 120, 101, 121, 122],
         "rr_source_order_ms": [800, 810, 1000, 1000],
         "packed_rr_timestamps_fit": [T + 1, T + 2],
         "arithmetic": "HRV78 field0 /1000 gives seconds. Invalid65535 omitted, not zero. HR132 field9 /1024."})
    add("header12.fit", wrap(message(0, 20, [(3, 1, U8)], bytes([120])), 12),
        ["twelve_byte_header"], {"valid_crc": True, "record_hr_bpm": [120]})
    valid = FILES["native_unknown.fit"][0]
    add("bad_file_crc.fit", valid[:-1] + bytes([valid[-1] ^ 0xFF]), ["bad_file_crc"], {"accepted": False})
    bad = bytearray(valid)
    bad[12] ^= 0xFF
    bad[-2:] = struct.pack("<H", crc(bad[:-2]))
    add("bad_header_crc.fit", bytes(bad), ["bad_header_crc", "independent_valid_file_crc"], {"accepted": False})
    add("truncated.fit", valid[:-3], ["truncation"], {"accepted": False})
    add("bad_signature.fit", valid[:8] + b"NOPE" + valid[12:], ["bad_signature"], {"accepted": False})


def developer_definitions():
    body = definition(0, 207, [(0, 16, BYTE), (1, 16, BYTE), (2, 2, U16), (3, 1, U8), (4, 4, U32)])
    for index in (0, 1):
        body += data(0, bytes(range(index * 32, index * 32 + 16))
                     + bytes(range(index * 32 + 16, index * 32 + 32))
                     + struct.pack("<HBI", 255, index, 1))
    body += definition(1, 206, [(0, 1, U8), (1, 1, U8), (2, 1, U8), (3, 16, 7),
                               (6, 1, U8), (7, 1, 1), (8, 8, 7), (14, 2, U16), (15, 1, U8)])
    for index, scale, offset in [(0, 2, 3), (1, 4, -2)]:
        body += data(1, bytes([index, 3, U16]) + b"heart_rate\0".ljust(16, b"\0")
                     + struct.pack("<Bb", scale, offset) + b"bpm\0".ljust(8, b"\0")
                     + struct.pack("<HB", 20, 3))
    return body


def run_body(sport=1, subtype=0, sessions=1, avg_speed=None):
    body = definition(0, 23, [(253, 4, U32), (0, 1, U8), (1, 1, U8), (2, 2, U16),
                             (3, 4, 0x8C), (4, 2, U16), (5, 2, U16), (25, 1, 0)])
    body += data(0, struct.pack("<IBBHIHHB", T, 0, 0, 1, 123456, 65534, 1234, 5))
    body += data(0, struct.pack("<IBBHIHHB", T, 1, 120, 32, 654321, 42, 100, 1))
    body += message(1, 2, [(0, 1, U8), (1, 4, U32), (2, 4, U32), (5, 1, 1)],
                    struct.pack("<BIIb", 0, 0, 28800, 32))
    body += message(1, 7, [(1, 1, U8), (2, 1, U8), (5, 1, 0)], bytes([190, 170, 1]))
    body += definition(1, 8, [(254, 2, U16), (1, 1, U8), (2, 8, 7)])
    for index, limit in enumerate([120, 140, 160, 180, 190]):
        body += data(1, struct.pack("<HB", index, limit) + f"Zone {index + 1}".encode().ljust(8, b"\0"))
    body += definition(2, 21, [(253, 4, U32), (0, 1, 0), (1, 1, 0)])
    body += data(2, struct.pack("<IBB", T, 0, 0))
    body += definition(3, 20, [(253, 4, U32), (3, 1, U8), (4, 1, U8), (5, 4, U32),
                             (6, 2, U16), (73, 4, U32)])
    # Boundary outsiders must not become in-session points. Zero speed is valid.
    for offset, hr, cadence, distance, base, enhanced in [
        (-1, 199, 99, 0, 9000, 9000), (0, 120, 85, 0, 9000, 2000),
        (10, 130, 85, 2000, 1000, 4000), (30, 140, 85, 10000, 5000, 0),
        (360, 120, 85, 100000, 5000, 0), (361, 199, 99, 100000, 9000, 9000)]:
        body += data(3, struct.pack("<IBBIHI", T + offset, hr, cadence, distance, base, enhanced))
    for offset, event_type in [(120, 1), (180, 0), (360, 4)]:
        body += data(2, struct.pack("<IBB", T + offset, 0, event_type))
    fields = [(254, 2, U16), (253, 4, U32), (2, 4, U32), (7, 4, U32),
              (8, 4, U32), (9, 4, U32), (25, 1, 0), (39, 1, 0), (0, 1, 0), (1, 1, 0)]
    body += definition(4, 19, fields)
    for index, start, end, timer, distance in [(0, 0, 180, 120, 400), (1, 180, 360, 180, 600)]:
        body += data(4, struct.pack("<HIIIIIBBBB", index, T + end, T + start, 180000,
                                   timer * 1000, distance * 100, sport, subtype, 9, 1))
    fields = [(254, 2, U16), (253, 4, U32), (2, 4, U32), (5, 1, 0), (6, 1, 0),
              (7, 4, U32), (8, 4, U32), (9, 4, U32), (25, 2, U16), (26, 2, U16),
              (18, 1, U8), (0, 1, 0), (1, 1, 0)]
    if avg_speed is not None:
        fields += [(14, 2, U16)]
    body += definition(5, 18, fields)
    for index in range(sessions):
        shift = 360 * index
        payload = struct.pack("<HIIBBIIIHHBBB", index, T + shift + 360, T + shift,
                              sport, subtype, 360000, 300000, 100000, 0, 2, 85, 8, 1)
        if avg_speed is not None:
            payload += struct.pack("<H", avg_speed)
        body += data(5, payload)
    body += message(6, 34, [(253, 4, U32), (0, 4, U32), (1, 2, U16), (2, 1, 0),
                           (3, 1, 0), (4, 1, 0)],
                    struct.pack("<IIHBBB", T + 360 * sessions, 300000 * sessions, sessions, 0, 26, 1))
    return body


def run_oracle(sport=1, subtype=0, manufacturer=1, sessions=1):
    return {"runs_supported": sport == 1 and manufacturer == 1 and sessions == 1,
            "file_manufacturer": manufacturer, "sport": sport, "sub_sport": subtype,
            "session_count": sessions, "start_timestamp_fit": T, "end_timestamp_fit": T + 360,
            "session_timer_seconds": 300, "session_elapsed_seconds": 360, "session_distance_m": 1000,
            "session_avg_speed_mps_from_distance_timer": 10 / 3,
            "session_avg_pace_seconds_per_km": 300, "cadence_cycles_per_min": 85,
            "running_cadence_steps_per_min": 170,
            "in_session_record_offsets_seconds": [0, 10, 30, 360],
            "out_of_session_record_offsets_seconds": [-1, 361],
            "effective_record_speed_mps": [2, 4, 0, 0], "base_record_speed_mps": [9, 1, 5, 5],
            "moving_record_speed_duration_seconds": [10, 20],
            "moving_record_distance_integral_m": 100, "moving_record_mean_speed_mps": 10 / 3,
            "elapsed_record_mean_speed_mps_including_zero": 5 / 18,
            "moving_record_mean_pace_seconds_per_km": 300,
            "laps": [{"start_offset": 0, "end_offset": 180, "timer_seconds": 120, "distance_m": 400},
                     {"start_offset": 180, "end_offset": 360, "timer_seconds": 180, "distance_m": 600}],
            "timer_event_offsets_seconds": [0, 120, 180, 360], "timer_event_types": [0, 1, 0, 4],
            "sensor_manufacturers": [1, 32], "foreign_accessory_rejects_garmin_owner": False,
            "zone_high_bpm": [120, 140, 160, 180, 190], "max_heart_rate_bpm": 190,
            "threshold_heart_rate_bpm": 170, "device_time_zone_offset_hours": 8,
            "arithmetic": "Enhanced73 /1000 wins base6 /1000, including0. Cadence85 cycles means170 steps. Moving weighted speed=(2*10+4*20)/30; arithmetic sample mean3 is wrong. Session totals are authoritative, not sparse record integral."}


def rr_body(anchor_ticks=102400, packed=(1024, 2048), short_bytes=None, anchored=True, fractional_raw=16384):
    fields = [(253, 4, U32), (0, 2, U16), (6, 1, U8), (9, 4, U32)]
    body = message(7, 132, fields, struct.pack("<IHBI", T, fractional_raw, 120, anchor_ticks)) if anchored else b""
    packed_bytes = pack12(*packed) if short_bytes is None else short_bytes
    body += message(7, 132, [(6, 2, U8), (10, len(packed_bytes), BYTE)], bytes([121, 122]) + packed_bytes)
    return body


def run_cases():
    features = ["garmin_owner", "running_session", "session_boundaries", "irregular_samples",
                "enhanced_speed_precedence", "valid_zero_speed", "cycle_cadence", "laps", "timer_events",
                "foreign_accessory", "device_settings", "device_hr_zones"]
    for name, sport, subtype, manufacturer, sessions in [
        ("garmin_run.fit", 1, 0, 1, 1), ("garmin_treadmill.fit", 1, 1, 1, 1),
        ("garmin_trail.fit", 1, 3, 1, 1), ("cycling.fit", 2, 0, 1, 1),
        ("non_garmin_run.fit", 1, 0, 32, 1), ("multiple_sessions.fit", 1, 0, 1, 2)]:
        add(name, wrap(run_body(sport, subtype, sessions), manufacturer=manufacturer, full_identity=True),
            features + (["unsupported_runs_scope"] if sport != 1 or manufacturer != 1 or sessions != 1 else []),
            run_oracle(sport, subtype, manufacturer, sessions))
    zero_speed_expected = run_oracle()
    zero_speed_expected.update({"session_avg_speed_raw": 0, "primary_summary_speed_mps": 0,
                                "primary_summary_pace_seconds_per_km": None,
                                "session_avg_pace_seconds_per_km": None,
                                "derived_pace_seconds_per_km_from_distance_timer": 300,
                                "arithmetic": "Explicit session18 avg_speed14 raw0/1000 is recorded0 and wins positive record-derived speed; pace at0 speed is undefined, not300. Distance1000m/timer300s derived pace300 stays separately identified."})
    add("garmin_summary_zero_speed.fit", wrap(run_body(avg_speed=0), full_identity=True),
        features + ["session_avg_speed_precedence", "valid_zero_summary_speed"], zero_speed_expected)
    valid = FILES["garmin_run.fit"][0]
    add("garmin_header12.fit", wrap(run_body(), header_size=12, full_identity=True),
        features + ["twelve_byte_header"], {**run_oracle(), "accepted": True, "valid_crc": True})
    zero_header = bytearray(valid)
    zero_header[12:14] = b"\0\0"
    zero_header[-2:] = struct.pack("<H", crc(zero_header[:-2]))
    add("garmin_zero_header_crc.fit", bytes(zero_header), features + ["zero_header_crc"],
        {**run_oracle(), "accepted": True, "valid_crc": True,
         "header_crc": "Zero means header CRC absent; complete file CRC remains verified."})
    add("garmin_trailing_byte.fit", valid + b"\0", ["trailing_bytes"],
        {"accepted": False, "error": "InvalidFit", "reason": "Runs integrity policy rejects trailing bytes."})
    add("garmin_chained.fit", valid + valid, ["chained_fit"],
        {"accepted": False, "error": "InvalidFit", "reason": "Runs integrity policy rejects concatenated FIT containers."})
    extended = bytes([16]) + valid[1:12] + b"\0\0"
    extended += struct.pack("<H", crc(extended))
    extended += valid[14:-2]
    extended += struct.pack("<H", crc(extended))
    add("garmin_extended_header16.fit", extended, features + ["extended_header", "zero_header_crc", "unknown_header_extensions"],
        {**run_oracle(), "accepted": True, "valid_crc": True,
         "header_crc": "Zero at bytes12-13 means absent; bytes14-15 are opaque extensions covered by the full file CRC."})
    for header_size, extension in [(16, b"\xA5\x5A"), (15, b"\xA5"), (13, b"\xA5")]:
        header = bytes([header_size]) + valid[1:12]
        if header_size >= 14:
            header_crc = crc(header)
            assert header_crc != 0
            header += struct.pack("<H", header_crc)
        header += extension
        extended_valid = header + valid[14:-2]
        extended_valid += struct.pack("<H", crc(extended_valid))
        name = "garmin_extended_header16_crc.fit" if header_size == 16 else f"garmin_extended_header{header_size}.fit"
        add(name, extended_valid, features + ["extended_header", "unknown_header_extensions"],
            {**run_oracle(), "accepted": True, "valid_crc": True, "header_size": header_size,
             "header_crc": "Nonzero bytes12-13 cover only bytes0-11." if header_size >= 14 else
                           "A13-byte header has no complete optional header CRC; byte12 is opaque.",
             "header_extension_hex": extension.hex(),
             "file_crc": "Full file CRC covers every declared header byte and the unchanged Garmin run body."})
        if header_size == 16:
            bad_header = bytearray(extended_valid)
            bad_header[12] ^= 1
            bad_header[-2:] = struct.pack("<H", crc(bad_header[:-2]))
            add("garmin_extended_header16_bad_header_crc.fit", bytes(bad_header),
                ["extended_header", "bad_header_crc", "unknown_header_extensions"],
                {"accepted": False, "error": "InvalidFit", "valid_crc": True,
                 "reason": "Header CRC is corrupt while the recomputed full file CRC is valid."})
            bad_extension = bytearray(extended_valid)
            bad_extension[14] ^= 1
            add("garmin_extended_header16_bad_extension_crc.fit", bytes(bad_extension),
                ["extended_header", "bad_file_crc", "unknown_header_extensions"],
                {"accepted": False, "error": "InvalidFit", "valid_crc": False,
                 "reason": "An opaque extension byte is corrupt; header CRC stays valid and the unchanged full file CRC fails."})
            add("garmin_extended_header16_truncated.fit", extended_valid[:14],
                ["extended_header", "truncation"],
                {"accepted": False, "error": "InvalidFit",
                 "reason": "Header declares16 bytes but only the first14 exist; no data or footer follows."})
    big = definition(0, 20, [(253, 4, U32), (3, 1, U8), (6, 2, U16),
                            (240, 2, U16), (241, 1, U8)], architecture=1)
    big += data(0, struct.pack(">IBHHB", T, 0, 0, 54321, 255))
    big += definition(0, 65280, [(0, 6, U16), (1, 1, U8), (2, 8, 0x8F),
                               (3, 8, 0x8E), (4, 1, 0x0A), (5, 4, U16)], architecture=1)
    big += data(0, struct.pack(">3HBQqB2H", 0, 65535, 42, 255,
                              9007199254740993, -9007199254740993, 0, 65535, 65535))
    add("garmin_big_endian.fit", wrap(run_body() + big, full_identity=True),
        features + ["big_endian", "unknown_fields", "unsafe_integers", "arrays", "invalid_zero"],
        {"runs_supported": True, "record_oracle": FILES["native_unknown.fit"][2],
         "unknown_message_oracle": FILES["unknown_arrays_integers.fit"][2],
         "arithmetic": "Architecture1 encodes global numbers and numeric payloads big-endian; decoded values match architecture0 oracles."})
    component_record = message(0, 20, [(253, 4, U32), (8, 3, BYTE)],
                               struct.pack("<I", T + 35) + pack12(300, 160))
    add("garmin_components_only.fit", wrap(run_body() + component_record, full_identity=True),
        features + ["native_components", "component_only_speed_distance"],
        {"runs_supported": True, "component_record_timestamp_fit": T + 35,
         "stored_speed_field6_present": False, "stored_distance_field5_present": False,
         "expanded_speed_mps": 3, "expanded_distance_m": 1034,
         "component_low_distance_m": 10, "native_anchor_distance_m": 1000,
         "packed_hex": pack12(300, 160).hex(),
         "arithmetic": "record8 low12 speed300/100=3m/s. Distance accumulation follows wire order: previous native distance1000m*16=16000 ticks (low12=3712); packed160 rolls to(16000 &~4095)+4096+160=16544 ticks; /16=1034m. TimestampT+35 does not erase the preceding wire-order anchor. No native6/5 exists on this record, so normalized fields use recorded component expansion."})
    hrv_zero = message(0, 78, [(0, 6, U16)], struct.pack("<3H", 0, 1000, 65535))
    add("garmin_hrv_zero.fit", wrap(run_body() + hrv_zero, full_identity=True),
        features + ["native_hrv", "valid_recorded_zero", "invalid_sentinel", "unaligned_rr"],
        {"runs_supported": True, "native_hrv_time_ms": [0, 1000, None],
         "rr_intervals_ms": [0, 1000], "rr_timestamps_fit": [None, None],
         "rr_timing_eligible": False,
         "arithmetic": "HRV78 field0 uint16 /1000 seconds. Zero is a recorded numeric0, not uint16 invalid65535. No UTC anchor exists; zero is not asserted physiologically valid."})
    for name, anchor, packed, intervals, offsets, events in [
        ("garmin_packed_hr.fit", 102400, (1024, 2048), [1000, 1000], [1.5, 2.5], [103424, 104448]),
        ("garmin_packed_hr_rollover.fit", 105984, (0, 1024), [500, 1000], [1, 2], [106496, 107520]),
        ("garmin_packed_hr_u32_rollover.fit", 4294967040, (256, 1280), [500, 1000], [1, 2], [4294967552, 4294968576])]:
        expected = run_oracle()
        expected.update({"rr_intervals_ms": intervals, "rr_timestamp_offsets_seconds": offsets,
                         "rr_timestamps_fit": [T + offset for offset in offsets],
                         "hr_anchor_fractional_raw": 16384, "hr_anchor_event_ticks": anchor,
                         "packed_low12_ticks": list(packed), "unwrapped_event_ticks": events,
                         "arithmetic_rr": "Each RR=(nextEventTicks-previousEventTicks)*1000/1024. Wall timestamp=T+16384/32768+(eventTicks-anchorTicks)/1024. Packed counters roll modulo4096, full counters modulo2^32. No reciprocal BPM."})
        add(name, wrap(run_body() + rr_body(anchor, packed), full_identity=True),
            features + ["native_packed_rr", "fractional_anchor", "counter_carry"], expected)
    fractional_expected = run_oracle()
    fractional_expected.update({
        "hr_anchor_fractional_raw": 9, "hr_anchor_fractional_seconds": 9 / 32768,
        "hr_anchor_event_ticks": 102400, "packed_low12_ticks": [1024, 2048],
        "unwrapped_event_ticks": [103424, 104448], "rr_intervals_ms": [1000, 1000],
        "rr_start_elapsed_seconds": [9 / 32768, 1 + 9 / 32768],
        "rr_end_elapsed_seconds": [1 + 9 / 32768, 2 + 9 / 32768],
        "rr_timestamps_fit": [T + 1 + 9 / 32768, T + 2 + 9 / 32768],
        "rr_timestamps_utc": ["2021-09-08T01:46:41.000274658Z", "2021-09-08T01:46:42.000274658Z"],
        "arithmetic_rr": "FIT epoch1989-12-31UTC + T1000000000s; fractional anchor9/32768s=0.000274658203125s. Counter100s to101/102s adds1/2s. Nanoseconds floor(9*1e9/32768)=274658. Preserve exact binary fraction through intermediate JSON spool."
    })
    add("garmin_fractional_anchor.fit", wrap(run_body() + rr_body(fractional_raw=9), full_identity=True),
        features + ["native_packed_rr", "fractional_anchor", "json_float_roundtrip"], fractional_expected)
    for size, raw, events in [(1, b"\x00", []), (2, pack12(1024), [103424]),
                              (4, pack12(1024, 2048) + b"\xFF", [103424, 104448])]:
        expected = run_oracle()
        expected.update({"packed_byte_count": size, "complete_packed_event_count": size * 8 // 12,
                         "unwrapped_event_ticks": events, "rr_intervals_ms": [1000] * len(events),
                         "trailing_bits_ignored": size * 8 % 12,
                         "arithmetic_rr": "Only floor(byteCount*8/12) complete events exist. Never pad missing bits into synthetic events."})
        add(f"garmin_packed_hr_short_{size}.fit", wrap(run_body() + rr_body(short_bytes=raw), full_identity=True),
            features + ["short_packed_bits", "fractional_anchor"], expected)
    expected = run_oracle()
    expected.update({"rr_intervals_ms": [1000], "packed_timestamp_anchor_complete": False,
                     "rr_timestamps_fit": [None], "rr_timing_eligible": False,
                     "reason": "Two native event counters1024,2048 yield one genuine RR=(2048-1024)*1000/1024=1000ms. No prior event forms the first interval. No HR132 wall-clock anchor means unknown absolute alignment; record BPM cannot supply timing or synthesize RR."})
    add("garmin_packed_hr_unanchored.fit", wrap(run_body() + rr_body(anchored=False), full_identity=True),
        features + ["unanchored_packed_hr", "no_hr_reciprocal"], expected)

    archive_names = ["native_unknown.fit", "unknown_arrays_integers.fit",
                     "developer_collisions.fit", "components.fit", "compressed_timestamp.fit",
                     "hr_rr_order.fit", "unknown_nonfinite_bytes.fit"]
    minimal_id = message(15, 0, [(0, 1, 0), (1, 2, U16)], struct.pack("<BH", 4, 1))
    body = run_body()
    archive_expected = {}
    for name in archive_names:
        content, _, oracle = FILES[name]
        body += content[content[0] + len(minimal_id):-2]
        archive_expected[name] = oracle
    add("garmin_archive.fit", wrap(body, full_identity=True),
        ["garmin_owner", "running_session", "lossless_archive", "unknown_message",
         "unknown_fields", "arrays", "unsafe_integers", "invalid_zero", "developer_identity",
         "developer_name_collision", "scale_offset", "native_components", "compressed_timestamp",
         "hr_rr_order", "nonfinite", "invalid_byte"], {"runs_supported": True, "session_count": 1,
                          "start_timestamp_fit": T, "end_timestamp_fit": T + 360,
                          "archive_oracles": archive_expected})


def progressive_case(
    source_name="progressive_rr_input.json",
    expected_source_hash="a57bea09b526d64689980b7eceba266d7d62a790bb21889d3da94a8027432fc1",
    oracle_name="progressive_rr_oracle.json",
    expected_oracle_hash="d5d5c78a54cefd386f375a7cbebc3e7c368bcb312178136a08a8eee9d36e19ab",
    output_name="garmin_progressive_rr.fit",
    amplitude=3,
):
    source_path = ROOT / source_name
    source = json.loads(source_path.read_bytes())
    source_hash = hashlib.sha256(source_path.read_bytes()).hexdigest()
    assert source_hash == expected_source_hash
    oracle_path = ROOT / oracle_name
    oracle = json.loads(oracle_path.read_bytes())
    oracle_hash = hashlib.sha256(oracle_path.read_bytes()).hexdigest()
    assert oracle_hash == expected_oracle_hash
    beats = source["rr"]["intervals"]
    encoded_reference = oracle["encodedCounterAndIntegerHr"]
    thresholds = {key: result["value"] for key, result in encoded_reference["results"].items()}
    ticks = [int(beat["endElapsedSeconds"] * 1024 + 0.5) for beat in beats]
    # Quantize cumulative event times, not each interval: no cumulative drift.
    body = message(0, 23, [(253, 4, U32), (0, 1, U8), (2, 2, U16), (25, 1, 0)],
                   struct.pack("<IBHB", T, 0, 1, 5))
    body += definition(1, 20, [(253, 4, U32), (3, 1, U8), (4, 1, U8), (73, 4, U32)])
    for second in range(481):
        speed = int((2.2 + second / 480) * 1000 + 0.5)
        hr = int(100 + second / 6 + 0.5)
        body += data(1, struct.pack("<IBBI", T + second, hr, 85, speed))
    body += definition(2, 21, [(253, 4, U32), (0, 1, 0), (1, 1, 0)])
    body += data(2, struct.pack("<IBB", T, 0, 0))
    body += data(2, struct.pack("<IBB", T + 480, 0, 4))
    body += message(3, 132, [(253, 4, U32), (0, 2, U16), (6, 1, U8), (9, 4, U32)],
                    struct.pack("<IHBI", T, 0, 100, 0))
    for start in range(0, len(ticks), 10):
        chunk = ticks[start:start + 10]
        packed = pack12(*(tick & 4095 for tick in chunk))
        bpms = bytes(int(100 + beat["endElapsedSeconds"] / 6 + 0.5)
                     for beat in beats[start:start + len(chunk)])
        body += message(3, 132, [(6, len(chunk), U8), (10, len(packed), BYTE)], bpms + packed)
    body += message(4, 19, [(254, 2, U16), (253, 4, U32), (2, 4, U32),
                           (7, 4, U32), (8, 4, U32), (9, 4, U32), (25, 1, 0)],
                    struct.pack("<HIIIIIB", 0, T + 480, T, 480000, 480000, 129600, 1))
    body += message(5, 18, [(254, 2, U16), (253, 4, U32), (2, 4, U32),
                           (5, 1, 0), (6, 1, 0), (7, 4, U32), (8, 4, U32), (9, 4, U32),
                           (25, 2, U16), (26, 2, U16)],
                    struct.pack("<HIIBBIIIHH", 0, T + 480, T, 1, 0, 480000, 480000, 129600, 0, 1))
    body += message(6, 34, [(253, 4, U32), (0, 4, U32), (1, 2, U16), (2, 1, 0)],
                    struct.pack("<IIHB", T + 480, 480000, 1, 0))
    intervals = [tick - previous for previous, tick in zip([0] + ticks, ticks)]
    add(output_name, wrap(body, full_identity=True),
        ["garmin_owner", "running_session", "synthetic_progressive_rr", "native_packed_rr",
         "fractional_anchor", "counter_carry", "dfa_crossing_input",
         "dfa_lt1_and_lt2_positive" if thresholds["lt1"] is not None and thresholds["lt2"] is not None
         else "dfa_lt2_no_extrapolation_abstention"],
        {"runs_supported": True, "session_count": 1, "start_timestamp_fit": T,
         "end_timestamp_fit": T + 480, "session_timer_seconds": 480,
         "session_elapsed_seconds": 480, "session_distance_m": 1296,
         "record_count": 481, "record_speed_mps_formula": "2.2 + elapsedSeconds/480",
         "record_hr_bpm_formula": "100 + elapsedSeconds/6", "record_speed_quantization_mps": 0.001,
         "record_hr_quantization_bpm": 1, "rr_interval_count": len(beats),
         "hr_anchor_event_ticks": 0, "hr_anchor_fractional_raw": 0, "packed_message_count": (len(beats) + 9) // 10,
         "event_ticks_head": ticks[:5], "event_ticks_tail": ticks[-5:],
         "rr_ticks_head": intervals[:5], "rr_ticks_tail": intervals[-5:],
         "rr_duration_ticks": ticks[-1], "rr_duration_seconds": ticks[-1] / 1024,
         "event_ticks_sha256": hashlib.sha256(struct.pack("<" + "I" * len(ticks), *ticks)).hexdigest(),
         "max_rr_error_ms": 1000 / 1024, "max_event_timestamp_error_ms": 500 / 1024,
         "numerical_reference": {"name": oracle_path.name, "sha256": oracle_hash,
                                 "license": "CC0-1.0", "software_only": True,
                                 "comparator": oracle["comparator"], "tolerance": oracle["tolerance"],
                                 "encoded_candidates": encoded_reference["candidates"],
                                 "threshold_results": encoded_reference["results"],
                                 "no_extrapolation_thresholds_bpm": thresholds,
                                 "heart_rate_basis": "Same FIT uint8 rounded HR, trapezoidal sample integration, packed1/1024-second event quantization; exact encoded-input software comparison."},
         "input": {"name": source_path.name, "sha256": source_hash, "license": "CC0-1.0",
                   "origin": "Original synthetic progressive DFA input from Runs analysis corpus; copied verbatim, no activity data.",
                   "source_generator": {"numpy_version": "2.2.6", "random_generator": "PCG64",
                                        "seed": 7429,
                                        "rho": "0.97 - 1.57*t/480",
                                        "state": "rho*state + standard_normal()",
                                        "interval_ms": f"600 - 270*t/480 + {amplitude}*state",
                                        "advance_t_seconds": "interval_ms/1000"},
                   "regeneration": "Committed immutable JSON is the input; NumPy is not needed to generate FIT."},
         "arithmetic_rr": "eventTick=floor(sourceEndElapsedSeconds*1024+0.5); packed=eventTick modulo4096; RR=(eventTick-previousEventTick)*1000/1024. Genuine HR132 anchor timestampT fractional0 counter0. No HR reciprocal."})


def archive_cases():
    valid = FILES["garmin_run.fit"][0]
    corrupt = valid[:-1] + bytes([valid[-1] ^ 0xFF])
    add("corrupt.fit", corrupt, ["bad_file_crc", "running_session"],
        {"accepted": False, "import_status": "failed"})
    for name, members, counts in [
        ("garmin_mixed.zip", ["garmin_run.fit", "cycling.fit", "non_garmin_run.fit", "corrupt.fit"],
         {"imported": 1, "unsupported": 2, "failed": 1}),
        ("garmin_run.zip", ["garmin_run.fit"], {"imported": 1, "unsupported": 0, "failed": 0}),
    ]:
        buffer = io.BytesIO()
        member_oracles = []
        with zipfile.ZipFile(buffer, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
            for member in members:
                content = FILES[member][0]
                info = zipfile.ZipInfo(member, date_time=(1980, 1, 1, 0, 0, 0))
                info.compress_type = zipfile.ZIP_DEFLATED
                info.create_system = 3
                info.external_attr = 0o100644 << 16
                archive.writestr(info, content, compresslevel=9)
                status = "imported" if member == "garmin_run.fit" else "failed" if member == "corrupt.fit" else "unsupported"
                member_oracles.append({"name": member, "bytes": len(content),
                                       "sha256": hashlib.sha256(content).hexdigest(),
                                       "license": "CC0-1.0", "import_status": status})
        content = buffer.getvalue()
        with zipfile.ZipFile(io.BytesIO(content)) as archive:
            assert archive.namelist() == members
            for member in members:
                assert archive.read(member) == FILES[member][0]
        add(name, content, ["deterministic_zip", "zip_batch_import"],
            {"counts": counts, "members": member_oracles,
             "zip_timestamp": "1980-01-01T00:00:00", "compression": "DEFLATE level9"})


FIELD_DICTIONARY = {
    "FIT_header": {"0": "header_size uint8: declared length at least12", "1": "protocol_version uint8",
                   "2-3": "profile_version uint16 little-endian", "4-7": "data_size uint32 little-endian",
                   "8-11": "data_type literal .FIT", "12-13": "optional header CRC uint16 little-endian when header_size >=14; nonzero covers bytes0-11, zero means absent",
                   "unknown_extensions": "Opaque declared header bytes at offsets14 onward, or byte12 for a13-byte header without a complete optional CRC; every header byte participates in the full file CRC"},
    "0:file_id": {"0": "type enum: activity4", "1": "manufacturer uint16: Garmin1, Wahoo32", "2": "product uint16", "3": "serial_number uint32z", "4": "time_created FITseconds"},
    "2:device_settings": {"0": "active_time_zone uint8", "1": "utc_offset uint32 seconds", "2": "time_offset uint32 seconds", "5": "time_zone_offset sint8 /4 hours"},
    "7:zones_target": {"1": "max_heart_rate uint8 bpm", "2": "threshold_heart_rate uint8 bpm", "5": "hr_calc_type enum: percent_max_hr1"},
    "8:hr_zone": {"254": "message_index uint16", "1": "high_bpm uint8", "2": "name string"},
    "18:session": {"254": "message_index uint16", "253": "timestamp FITseconds", "2": "start_time FITseconds", "5": "sport enum: running1 cycling2", "6": "sub_sport enum: generic0 treadmill1 trail3", "7": "total_elapsed_time uint32 /1000 seconds", "8": "total_timer_time uint32 /1000 seconds", "9": "total_distance uint32 /100 meters", "14": "avg_speed uint16 /1000 m/s", "25": "first_lap_index uint16", "26": "num_laps uint16", "18": "avg_cadence uint8 cycles/min", "0": "event enum session8", "1": "event_type enum stop1"},
    "19:lap": {"254": "message_index uint16", "253": "timestamp FITseconds", "2": "start_time FITseconds", "7": "total_elapsed_time uint32 /1000 seconds", "8": "total_timer_time uint32 /1000 seconds", "9": "total_distance uint32 /100 meters", "25": "sport enum", "39": "sub_sport enum", "0": "event enum lap9", "1": "event_type enum stop1"},
    "20:record": {"253": "timestamp FITseconds", "3": "heart_rate uint8 bpm", "4": "cadence uint8 cycles/min", "5": "distance uint32 /100 meters", "6": "speed uint16 /1000 m/s", "73": "enhanced_speed uint32 /1000 m/s", "8": "compressed_speed_distance byte: speed12bits /100 m/s, distance12bits /16 meters accumulated"},
    "21:event": {"253": "timestamp FITseconds", "0": "event enum: timer0", "1": "event_type enum: start0 stop1 stop_all4"},
    "23:device_info": {"253": "timestamp FITseconds", "0": "device_index uint8: creator0 accessory1", "1": "device_type uint8: ANT+HR120", "2": "manufacturer uint16", "3": "serial_number uint32z", "4": "product uint16", "5": "software_version uint16 /100", "25": "source_type enum: ANT+1 local5"},
    "34:activity": {"253": "timestamp FITseconds", "0": "total_timer_time uint32 /1000 seconds", "1": "num_sessions uint16", "2": "type enum manual0", "3": "event enum activity26", "4": "event_type enum stop1"},
    "78:hrv": {"0": "time uint16 array /1000 seconds; invalid65535"},
    "132:hr": {"253": "timestamp FITseconds", "0": "fractional_timestamp uint16 /32768 seconds", "1": "time256 uint8 /256 seconds (not used here)", "6": "filtered_bpm uint8 array", "9": "event_timestamp uint32 /1024 seconds", "10": "event_timestamp_12 byte array: little-endian contiguous12bit event counters /1024 seconds, modulo4096"},
    "206:field_description": {"0": "developer_data_index uint8", "1": "field_definition_number uint8", "2": "fit_base_type_id uint8: uint16=132", "3": "field_name string", "6": "scale uint8", "7": "offset sint8", "8": "units string", "14": "native_mesg_num uint16", "15": "native_field_num uint8"},
    "207:developer_data_id": {"0": "developer_id byte[16] synthetic", "1": "application_id byte[16] synthetic", "2": "manufacturer_id uint16 development255", "3": "developer_data_index uint8", "4": "application_version uint32"},
}


def main():
    conformance_cases()
    run_cases()
    progressive_case()
    progressive_case(
        source_name="progressive_rr_positive_input.json",
        expected_source_hash="4ba341dc2b62db6ccf0fe282ea6610ad85818823d9f5cc023af27eb969248b7f",
        oracle_name="progressive_rr_positive_oracle.json",
        expected_oracle_hash="d65bbc3da0c94f3de8478024f77084bbd05fb9a46006aee1dd27cd11f733e1da",
        output_name="progressive_rr_positive.fit",
        amplitude=6,
    )
    archive_cases()
    # CRC known check from standard CRC-16/ARC catalogue; independent literal.
    assert crc(b"123456789") == 0xBB3D
    assert pack12(1024, 2048) == bytes.fromhex("000480")
    entries = []
    for name, (content, features, expected) in sorted(FILES.items()):
        if name.endswith(".fit") and not any(feature in features for feature in
                ["bad_file_crc", "bad_header_crc", "truncation", "bad_signature", "trailing_bytes", "chained_fit"]):
            assert content[8:12] == b".FIT" and crc(content) == 0
            assert len(content) == content[0] + int.from_bytes(content[4:8], "little") + 2
        (ROOT / name).write_bytes(content)
        entries.append({"name": name, "bytes": len(content), "sha256": hashlib.sha256(content).hexdigest(),
                        "license": "CC0-1.0", "origin": "Original literal synthetic bytes generated by generate.py; no imported activity.",
                        "features": features, "expected": expected})
    manifest = {"schema_version": 1, "generator": "generate.py", "generator_version": VERSION,
                "generator_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                "seed": SEED, "seed_role": "Fixed corpus identity; no random input", "license": "CC0-1.0",
                "fit_protocol_version": "2.0", "fit_profile_version": "212.17",
                "provenance": "Entire corpus is synthetic, including device and developer IDs. No personal FIT, SDK, network or secrets are used.",
                "inputs": [{"name": "progressive_rr_input.json", "license": "CC0-1.0",
                            "sha256": hashlib.sha256((ROOT / "progressive_rr_input.json").read_bytes()).hexdigest(),
                            "bytes": (ROOT / "progressive_rr_input.json").stat().st_size,
                            "provenance": "Original software-generated synthetic AR-process intervals and linear-ramp records, explicitly authorized CC0 by the Runs analysis author; no human activity."},
                           {"name": "progressive_rr_oracle.json", "license": "CC0-1.0",
                            "sha256": hashlib.sha256((ROOT / "progressive_rr_oracle.json").read_bytes()).hexdigest(),
                            "bytes": (ROOT / "progressive_rr_oracle.json").stat().st_size,
                            "provenance": "Independent nolds/SciPy software reference supplied by the Runs analysis author, explicitly CC0; includes exact encoded-input RR and rounded HR comparison."},
                           {"name": "progressive_rr_positive_input.json", "license": "CC0-1.0",
                            "sha256": hashlib.sha256((ROOT / "progressive_rr_positive_input.json").read_bytes()).hexdigest(),
                            "bytes": (ROOT / "progressive_rr_positive_input.json").stat().st_size,
                            "provenance": "Distinct amplitude6ms synthetic positive progression, PCG64 seed7429, explicitly CC0; no human activity."},
                           {"name": "progressive_rr_positive_oracle.json", "license": "CC0-1.0",
                            "sha256": hashlib.sha256((ROOT / "progressive_rr_positive_oracle.json").read_bytes()).hexdigest(),
                            "bytes": (ROOT / "progressive_rr_positive_oracle.json").stat().st_size,
                            "provenance": "Independent frozen encoded-input nolds/SciPy oracle with one admissible candidate per LT1 and LT2, no extrapolation."}],
                "source_references": [
                    {"source": "fitparser 0.11.0 src/profile/decode.rs and field_types.rs", "license": "MIT", "url": "https://docs.rs/crate/fitparser/0.11.0/source/src/profile/", "use": "Field numbers, scalar scales and enum values inspected only; no source or recorded fixture copied."},
                    {"source": "FIT protocol public documentation", "url": "https://developer.garmin.com/fit/protocol/", "use": "Binary definition headers, little-endian payloads, timestamps, CRC and component packing."},
                    {"source": "CRC catalogue CRC-16/ARC", "url": "https://reveng.sourceforge.io/crc-catalogue/16.htm#crc.cat.crc-16-arc", "use": "Independent check123456789=0xBB3D."}],
                "oracle_basis": "Literal expected values worked from the field dictionary and FIT arithmetic, not decoder output.",
                "field_dictionary": FIELD_DICTIONARY, "files": entries}
    encoded = (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode()
    (ROOT / "manifest.json").write_bytes(encoded)
    print(json.dumps({"generated_files": len(entries), "bytes": sum(entry["bytes"] for entry in entries),
                      "manifest_sha256": hashlib.sha256(encoded).hexdigest()}, sort_keys=True))


if __name__ == "__main__":
    main()
