#!/usr/bin/env python3
"""Independent rows and cross-profile refusal for directional-series results."""

import hashlib
from pathlib import Path
import unittest

import xgmi_peer_hot_results as hot
import xgmi_peer_series_results as series
from test_xgmi_peer_hot_results import IDS, encode, record as hot_record


def record(backend, depth=16):
    fields = hot_record(backend, depth)
    fields.update(surface="native-api", measurement="persistent-series",
                  direction="forward-series-then-reverse-series",
                  lifetime_setup="outside-samples", lifetime_finish="outside-samples",
                  validation="final-readback", forward_samples="30", reverse_samples="30")
    if backend == "kfd":
        for key in ("aggregate_roster", "background_progress"):
            del fields[key]
        fields.update(
            schema="fe2o3.xgmi-peer-retained-pair-series-benchmark.v1",
            qualification_profile="fe2o3.gfx942-xgmi-retained-pair-ordinary-lifetime.v1",
            qualification_policy_sha256="18cfe1c56d270d9cab1cdc2f67a2b26b35e7cf1540a2962e4dc2f5cb42155b61",
            environment_assumption="reviewed-mi300x-amdgpu61613-ordinary-lifetime",
            gpu_ids="101,202", forward_engine="2", reverse_engine="3", scopes="2",
            mapping_lifetime="directional-retained-pair", operational_fences="inside-samples",
            progress="explicit-exact-roster-native-wait",
            timing="native-enqueue-through-paired-operational-completion",
            forward_scope_entry_ns="1000000", reverse_scope_entry_ns="2000000",
            forward_scope_finish_ns="10000", reverse_scope_finish_ns="20000",
        )
    else:
        fields["schema"] = "fe2o3.xgmi-peer-persistent-series-benchmark.v1"
    return fields


def controls(backend="kfd", depth=16):
    result = dict(backend=backend, unique_ids=IDS, copy_bytes=1048576,
                  depth=depth, warmups=10, samples=30)
    if backend == "kfd":
        result.update(kfd_gpu_ids=[101, 202], kfd_engines=[2, 3])
    return result


class SeriesResultsTests(unittest.TestCase):
    def test_all_backend_depth_cells(self):
        for backend in ("kfd", "hip", "hsa"):
            for depth in (1, 16, 32):
                fields = record(backend, depth)
                self.assertEqual(series.parse_result(encode(fields), **controls(backend, depth)), fields)

    def test_old_and_new_profiles_are_refused_both_ways(self):
        for backend in ("kfd", "hip", "hsa"):
            args = controls(backend)
            with self.assertRaises(ValueError):
                series.parse_result(encode(hot_record(backend, 16)), **args)
            args.pop("kfd_gpu_ids", None)
            args.pop("kfd_engines", None)
            with self.assertRaises(ValueError):
                hot.parse_result(encode(record(backend)), **args)

    def test_every_field_is_required_and_checked(self):
        for backend in ("kfd", "hip", "hsa"):
            fields = record(backend)
            for key in fields:
                wrong = {**fields, key: "incorrect"}
                with self.subTest(backend=backend, key=key), self.assertRaises(ValueError):
                    series.parse_result(encode(wrong), **controls(backend))
                missing = dict(fields)
                del missing[key]
                with self.assertRaises(ValueError):
                    series.parse_result(encode(missing), **controls(backend))
            with self.assertRaises(ValueError):
                series.parse_result(encode({**fields, "extra": "field"}), **controls(backend))

    def test_exact_external_physical_pair_engine_and_trial_controls(self):
        for key, values in {
            "kfd_gpu_ids": (None, [202, 101], [101, 101], [True, 202], [101, -1]),
            "kfd_engines": (None, [3, 2], [2], [2, True], [2, 1 << 32]),
            "unique_ids": (list(reversed(IDS)), [IDS[0], IDS[0]], ["0x0", IDS[1]]),
            "samples": (29, 31, 0, True), "warmups": (9, 11, -1),
            "copy_bytes": (0, 4096, 0x003F_FFE1), "depth": (0, 2, 32),
        }.items():
            for value in values:
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    series.parse_result(encode(record("kfd")), **{**controls(), key: value})
        with self.assertRaises(ValueError):
            series.parse_result(encode(record("hip")), **controls("hip"), kfd_gpu_ids=[101, 202])

    def test_strict_row_framing(self):
        row = encode(record("kfd"))
        for bad in (row[:-1], row + row, b" " + row, row.replace(b" ", b"  ", 1),
                    row[:-1] + b" backend=kfd\n", row + b"\xff", b"x" * 8193):
            with self.assertRaises(ValueError):
                series.parse_result(bad, **controls())

    def test_scope_timing_is_separate_positive_population(self):
        fields = record("kfd")
        for direction in ("forward", "reverse"):
            for phase in ("entry", "finish"):
                key = f"{direction}_scope_{phase}_ns"
                for value in ("0", "-1", "01", "1.0", str(1 << 64)):
                    with self.assertRaises(ValueError):
                        series.parse_result(encode({**fields, key: value}), **controls())
        for key, value in (("operational_fences", "outside-samples"),
                           ("validation", "every-sample"), ("scopes", "1"),
                           ("forward_p95_ns", "1"), ("forward_p50_GBps", "0.000")):
            with self.assertRaises(ValueError):
                series.parse_result(encode({**fields, key: value}), **controls())

    def test_numeric_controls_are_not_coerced_and_do_not_overflow(self):
        for key in ("copy_bytes", "depth", "warmups", "samples"):
            for value in (None, "16", 16.0, True, -1, 1 << 64):
                with self.assertRaises(ValueError):
                    series.parse_result(encode(record("kfd")), **{**controls(), key: value})
        with self.assertRaises(ValueError):
            series.parse_result(encode(record("kfd")), **{**controls(), "warmups": (1 << 64) - 20})

    def test_independent_policy_digest_matches_embedded_profile(self):
        repo = Path(__file__).resolve().parents[2]
        policy = (repo / "crates/fe2o3-kfd/src/sdma/retained_pair_policy_v1.txt").read_bytes()
        self.assertEqual(hashlib.sha256(policy).hexdigest(), record("kfd")["qualification_policy_sha256"])
        self.assertEqual(series.POLICY_SHA256, record("kfd")["qualification_policy_sha256"])


if __name__ == "__main__":
    unittest.main()
