#!/usr/bin/env python3
"""Synthetic CPU fixtures only; these tests are not native evidence."""

import importlib.util
from pathlib import Path
import unittest


def load(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


D = load("xgmi_peer_segments_diagnostic_results")
BASE = load("test_xgmi_peer_segments_results")
CONTROLS = {**BASE.CONTROLS, "diagnostic": True}


def fixture():
    rows = []
    for ordinal in range(10):
        direction = ordinal % 2
        row = dict(schema=D.SCHEMA, backend="kfd", ordinal=str(ordinal),
                   backend_submission=str(ordinal + 7), source_uid=f"{[1, 2][direction]:016x}",
                   destination_uid=f"{[2, 1][direction]:016x}", descriptor_count="65",
                   useful_bytes="65536", submission_calls="65", wait_calls="65")
        row.update({field: "1" for field in D.TIMINGS})
        row.update(opening_currentness_ns="10", closing_currentness_ns="10", total_ns="25")
        for side in ("opening", "closing"):
            row.update({f"{side}_topology_discovery_ns": "4", f"{side}_topology_total_ns": "4",
                        f"{side}_pair_total_ns": "9"})
        row.update(authority="none", teardown="explicit", timing="backend-ordered-segments-currentness-host-only")
        rows.append(" ".join(f"{key}={value}" for key, value in row.items()))
    return ("\n".join(rows) + "\n").encode("ascii") + BASE.fixture()


class DiagnosticReceiptTests(unittest.TestCase):
    def test_complete_join_and_mode_binding(self):
        raw = fixture()
        result = D.parse_receipt(raw, **CONTROLS)
        self.assertEqual(result["receipt"], BASE.R.parse_receipt(BASE.fixture(), **BASE.CONTROLS))
        self.assertEqual(len(result["observations"]), 10)
        self.assertEqual([row["population"] for row in result["observations"]],
                         ["prime"] * 2 + ["warmup"] * 4 + ["sample"] * 4)
        self.assertEqual(result["observations"][-1]["backend_submission"], 16)
        self.assertEqual(D.parse_receipt(BASE.fixture(), **{**CONTROLS, "diagnostic": False})["observations"], [])
        for raw, overrides in [(raw, {"diagnostic": False}), (BASE.fixture(), {}),
                               (raw, {"backend": "hip"}), (raw, {"diagnostic": 1}),
                               (raw, {"diagnostic": "true"})]:
            with self.assertRaises(ValueError):
                D.parse_receipt(raw, **{**CONTROLS, **overrides})

    def test_every_field_is_exact_bounded_and_required(self):
        raw = fixture()
        first = raw.split(b"\n", 1)[0]
        fields = dict(item.split(b"=", 1) for item in first.split(b" "))
        for key, value in fields.items():
            for replacement in (b"", b"-1", b"01", b"18446744073709551616", b"unavailable"):
                with self.subTest(key=key, replacement=replacement), self.assertRaises(ValueError):
                    D.parse_receipt(raw.replace(key + b"=" + value, key + b"=" + replacement, 1), **CONTROLS)
            with self.subTest(missing=key), self.assertRaises(ValueError):
                changed = b" ".join(part for part in first.split(b" ") if not part.startswith(key + b"="))
                D.parse_receipt(changed + b"\n" + raw.split(b"\n", 1)[1], **CONTROLS)

    def test_nested_bounds_roster_and_legacy_suffix_are_fail_closed(self):
        raw = fixture()
        lines = raw.splitlines(keepends=True)
        bad = [b"", raw[:-1], raw + b"\n", raw + lines[0], b"".join(lines[1:]),
               b"".join([lines[1], lines[0], *lines[2:]]), b"".join([*lines[:9], *lines[10:]]),
               raw.replace(b"\n", b"\r\n"), raw.replace(b"schema=", b"schema=\xff", 1),
               lines[0][:-1] + b" extra=1\n" + b"".join(lines[1:]),
               lines[0][:-1] + b" total_ns=25\n" + b"".join(lines[1:])]
        for before, after in [
            (b"total_ns=25", b"total_ns=24"), (b"total_ns=25", b"total_ns=999999999"),
            (b"submission_ns=1", b"submission_ns=18446744073709551615"),
            (b"opening_pair_total_ns=9", b"opening_pair_total_ns=11"),
            (b"closing_pair_total_ns=9", b"closing_pair_total_ns=8"),
            (b"opening_topology_total_ns=4", b"opening_topology_total_ns=3"),
            (b"closing_topology_total_ns=4", b"closing_topology_total_ns=5"),
            (b"opening_source_before_ns=1", b"opening_source_before_ns=2"),
            (b"backend_submission=7", b"backend_submission=8"),
            (b"source_uid=0000000000000001", b"source_uid=0000000000000002"),
            (b"submission_calls=65", b"submission_calls=64"),
            (b"wait_calls=65", b"wait_calls=66"),
            (b"descriptor_count=65", b"descriptor_count=64"),
            (b"correctness=passed", b"correctness=failed"),
            (b"record=list backend=kfd band=0", b"record=list backend=kfd band=1"),
            (b"teardown=explicit", b"teardown=implicit"),
        ]:
            self.assertIn(before, raw)
            bad.append(raw.replace(before, after, 1))
        for index, receipt in enumerate(bad):
            with self.subTest(mutation=index), self.assertRaises(ValueError):
                D.parse_receipt(receipt, **CONTROLS)


if __name__ == "__main__":
    unittest.main()
