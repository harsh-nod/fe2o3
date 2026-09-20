#!/usr/bin/env python3
"""CPU-only adversarial receipt validation; fixtures are not hardware evidence."""

import importlib.util
from pathlib import Path
import unittest

SPEC = importlib.util.spec_from_file_location("segments_results", Path(__file__).with_name("xgmi_peer_segments_results.py"))
R = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(R)

CONTROLS = dict(backend="kfd", unique_ids=[1, 2], useful_bytes=65536,
                descriptor_count=65, warmups=2, samples=2)


def fixture(backend="kfd", warmups=2, samples=2):
    rows = []
    for band in range(1 + warmups + samples):
        for direction in range(2):
            population = "prime" if band == 0 else "warmup" if band <= warmups else "sample"
            elapsed = 999_999 if band <= warmups else 100 * band + direction
            rows.append(f"schema={R.SCHEMA} record=list backend={backend} band={band} direction={direction} population={population} elapsed_ns={elapsed}")
    rows.append(f"schema={R.SCHEMA} record=complete backend={backend} unique_ids=0000000000000001,0000000000000002 useful_bytes=65536 descriptor_count=65 logical_depth=1 warmups={warmups} samples={samples} prime_lists=1 band_bytes=73728 source_bytes=73728 destination_bytes={73728 * (1 + warmups + samples)} layout=reversed-ragged-slots-v1 progress={R.PROGRESS[backend]} deadline_ns=60000000000 timing=list-admission-through-observed-completion mapping_lifetime=retained-pair-no-allocation-host-readwrite-between-lists completion_cleanup=outside-timing correctness=passed teardown=explicit")
    return ("\n".join(rows) + "\n").encode("ascii")


class ReceiptTests(unittest.TestCase):
    def test_quantile_and_deadline_boundaries(self):
        for count, median, p95 in ((1, 100, 100), (10, 550, 1000), (64, 3250, 6100)):
            result = R.parse_receipt(fixture(warmups=0, samples=count), **{**CONTROLS, "warmups": 0, "samples": count})
            row = result["directions"][0]
            self.assertEqual(row["p50_ns_median"], median)
            self.assertEqual(row["p95_ns_nearest_rank"], p95)
            self.assertEqual(row["effective_useful_GBps_at_p50"], 65536 / median)
        R.parse_receipt(fixture().replace(b"elapsed_ns=999999", b"elapsed_ns=59999999999", 1), **CONTROLS)

    def test_exact_roster_and_sample_only_statistics(self):
        for backend in R.PROGRESS:
            result = R.parse_receipt(fixture(backend), **{**CONTROLS, "backend": backend})
            for direction, row in enumerate(result["directions"]):
                self.assertEqual(row["sample_ns"], [300 + direction, 400 + direction])
                self.assertEqual(row["p50_ns_median"], 350 + direction)
                self.assertEqual(row["p95_ns_nearest_rank"], 400 + direction)
                self.assertEqual(row["samples"], 2)

    def test_rejects_modified_or_partial_receipts(self):
        valid = fixture()
        rows = valid.splitlines(keepends=True)
        bad = [b"", valid[:-1], valid + b"\n", valid + rows[0], b"".join(rows[1:]),
               b"".join(rows[:-1]), b"".join([rows[1], rows[0], *rows[2:]]),
               valid.replace(b"\n", b"\r\n"), valid.replace(b"record=list", b"record=li\xffst", 1)]
        for a, b in [
            (b"backend=kfd", b"backend=hip"), (b"band=0", b"band=1"),
            (b"direction=0", b"direction=1"), (b"population=prime", b"population=sample"),
            (b"elapsed_ns=999999", b"elapsed_ns=0"), (b"elapsed_ns=999999", b"elapsed_ns=NaN"),
            (b"elapsed_ns=999999", b"elapsed_ns=60000000000"),
            (b"elapsed_ns=999999", b"elapsed_ns=01"), (b"elapsed_ns=999999", b"elapsed_ns=-1"),
            (b"elapsed_ns=999999", b"elapsed_ns=1e3"), (b"elapsed_ns=999999", b"elapsed_ns=+1"),
            (b"elapsed_ns=999999", b"elapsed_ns=999999 elapsed_ns=1"),
            (b"elapsed_ns=999999", b"elapsed_ns=999999 extra=field"),
            (b"unique_ids=0000000000000001,0000000000000002", b"unique_ids=0000000000000002,0000000000000001"),
            (b"logical_depth=1", b"logical_depth=65"), (b"useful_bytes=65536", b"useful_bytes=65537"),
            (b"descriptor_count=65", b"descriptor_count=64"), (b"warmups=2", b"warmups=1"),
            (b"samples=2", b"samples=3"), (b"prime_lists=1", b"prime_lists=0"),
            (b"band_bytes=73728", b"band_bytes=73729"), (b"source_bytes=73728", b"source_bytes=1"),
            (b"destination_bytes=368640", b"destination_bytes=1"),
            (b"progress=single-ticket-sequence-full-currentness", b"progress=unchecked"),
            (b"correctness=passed", b"correctness=failed"), (b"teardown=explicit", b"teardown=implicit"),
            (b"schema=fe2o3.xgmi-ordered-segments.v1", b"schema=fe2o3.xgmi-ordered-segments.v2"),
        ]:
            self.assertIn(a, valid)
            bad.append(valid.replace(a, b, 1))
        for index, raw in enumerate(bad):
            with self.subTest(mutation=index), self.assertRaises(ValueError):
                R.parse_receipt(raw, **CONTROLS)

    def test_invalid_admitted_controls(self):
        for key, value in [("backend", "unknown"), ("unique_ids", [1, 1]),
                           ("unique_ids", [0, 2]), ("unique_ids", [1, 2**64]),
                           ("useful_bytes", 0), ("descriptor_count", 4097),
                           ("warmups", -1), ("samples", 0), ("samples", 65),
                           ("useful_bytes", True), ("samples", 2.0)]:
            with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                R.parse_receipt(fixture(), **{**CONTROLS, key: value})


if __name__ == "__main__":
    unittest.main()
