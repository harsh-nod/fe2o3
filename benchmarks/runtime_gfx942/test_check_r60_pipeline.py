#!/usr/bin/env python3
"""CPU-only rejection tests for R60 matched pipeline evidence."""

import copy
import importlib.util
import json
import pathlib
import unittest

CHECKER_PATH = pathlib.Path(__file__).with_name("check-r60-pipeline.py")
SPEC = importlib.util.spec_from_file_location("check_r60_pipeline", CHECKER_PATH)
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)


def records(backend="kfd", multiplier=1):
    config = copy.deepcopy(CHECKER.FIXED_CONFIG)
    config.update(
        backend=backend,
        source_commit="1" * 40,
        run_id="2" * 64,
        unique_id="ab83d2ffef0d3cdf",
        issue_api_calls_per_batch={"kfd": 128, "hip": 64, "hsa": 192}.get(backend, 1)
        if isinstance(backend, str) else 1,
    )
    result = [config]
    for ordinal in range(40):
        issue = (100 + ordinal) * multiplier
        tail = (200 + ordinal) * multiplier
        result.append({
            "record": "batch",
            "phase": "warmup" if ordinal < 10 else "sample",
            "index": ordinal if ordinal < 10 else ordinal - 10,
            "issued_launches": 64,
            "completed_launches": 64,
            "issue_batch_ns": issue,
            "tail_wait_batch_ns": tail,
            "total_batch_ns": issue + tail,
            "output_sha256": CHECKER.OUTPUT_SHA256,
        })
    result.append({"record": "complete", "validated_batches": 40})
    return result


def encoded(rows):
    return "".join(json.dumps(row) + "\n" for row in rows)


class EvidenceTests(unittest.TestCase):
    def reject(self, rows):
        with self.assertRaises(CHECKER.CheckError):
            CHECKER.parse_log(encoded(rows))

    def test_accepts_matched_batch_metrics_and_nearest_rank_percentiles(self):
        logs = [CHECKER.parse_log(encoded(records(backend, factor)))
                for backend, factor in (("kfd", 1), ("hip", 2), ("hsa", 3))]
        result = CHECKER.compare(logs)
        self.assertEqual(result["timings_ns"]["kfd"]["issue_batch_ns"],
                         {"p50": 124, "p95": 138})
        self.assertEqual(result["timings_ns"]["kfd"]["tail_wait_batch_ns"],
                         {"p50": 224, "p95": 238})
        self.assertEqual(result["timings_ns"]["kfd"]["total_batch_ns"],
                         {"p50": 348, "p95": 376})
        for metric in CHECKER.METRICS:
            self.assertEqual(result["baseline_over_kfd"]["hip"][metric],
                             {"p50": 2.0, "p95": 2.0})
            self.assertEqual(result["baseline_over_kfd"]["hsa"][metric],
                             {"p50": 3.0, "p95": 3.0})

    def test_warmups_are_validated_but_not_in_percentiles(self):
        rows = records()
        for batch in rows[1:11]:
            batch.update(issue_batch_ns=1, tail_wait_batch_ns=1, total_batch_ns=2)
        log = CHECKER.parse_log(encoded(rows))
        self.assertEqual(len(log["samples"]), 30)
        self.assertEqual(log["samples"][0]["issue_batch_ns"], 110)
        rows[1]["output_sha256"] = "0" * 64
        self.reject(rows)

    def test_rejects_every_changed_fixed_config_field(self):
        for field, value in CHECKER.FIXED_CONFIG.items():
            with self.subTest(field=field):
                rows = records()
                rows[0][field] = not value if type(value) is bool else None
                self.reject(rows)

    def test_rejects_numeric_type_substitution_in_config_and_geometry(self):
        for field, value in CHECKER.FIXED_CONFIG.items():
            if type(value) is int:
                with self.subTest(field=field):
                    rows = records()
                    rows[0][field] = float(value)
                    self.reject(rows)
        for field in ("grid", "workgroup"):
            rows = records()
            rows[0][field][1] = True
            self.reject(rows)
        for field in ("reset_timed", "explicit_allocation_api_timed"):
            rows = records()
            rows[0][field] = 0
            self.reject(rows)

    def test_rejects_invalid_identifiers(self):
        for field, length in (("source_commit", 40), ("run_id", 64), ("unique_id", 16)):
            for value in ("0" * length, "A" * length, "g" * length,
                          "1" * (length - 1), "1" * (length + 1), None, 1):
                with self.subTest(field=field, value=value):
                    rows = records()
                    rows[0][field] = value
                    self.reject(rows)

    def test_requires_exact_backend_specific_issue_api_count(self):
        for backend in CHECKER.BACKENDS:
            for value in (128 if backend != "kfd" else 64, 0, 64.0, 128.0, 192.0, True):
                with self.subTest(backend=backend, value=value):
                    rows = records(backend)
                    rows[0]["issue_api_calls_per_batch"] = value
                    self.reject(rows)

    def test_rejects_missing_extra_reordered_or_foreign_records(self):
        for ordinal in (0, 1, 10, 11, 40, 41):
            with self.subTest(missing=ordinal):
                rows = records()
                del rows[ordinal]
                self.reject(rows)
        rows = records()
        rows.insert(3, copy.deepcopy(rows[2]))
        self.reject(rows)
        rows = records()
        rows[11], rows[12] = rows[12], rows[11]
        self.reject(rows)
        rows = records()
        rows[0], rows[-1] = rows[-1], rows[0]
        self.reject(rows)
        rows = records()
        rows[11]["phase"] = "warmup"
        self.reject(rows)
        rows = records()
        rows[10]["phase"] = "sample"
        self.reject(rows)
        rows = records()
        rows[12]["index"] = 0
        self.reject(rows)

    def test_rejects_unknown_or_missing_fields_at_each_record_type(self):
        for ordinal in (0, 1, 41):
            rows = records()
            rows[ordinal]["foreign"] = 1
            self.reject(rows)
            for key in records()[ordinal]:
                with self.subTest(ordinal=ordinal, key=key):
                    rows = records()
                    del rows[ordinal][key]
                    self.reject(rows)

    def test_rejects_noninteger_nonfinite_out_of_bounds_timings(self):
        for field in CHECKER.METRICS:
            for value in (True, False, "100", 100.0, None, -1, 0,
                          CHECKER.TIMEOUT_NS + 1, float("nan"), float("inf"),
                          float("-inf")):
                with self.subTest(field=field, value=value):
                    rows = records()
                    rows[11][field] = value
                    self.reject(rows)

    def test_rejects_inconsistent_total_and_completion_counts(self):
        rows = records()
        rows[11]["total_batch_ns"] += 1
        self.reject(rows)
        for field in ("issued_launches", "completed_launches", "index"):
            for value in (True, 64.0, 63 if field != "index" else 64):
                rows = records()
                rows[11][field] = value
                self.reject(rows)
        for value in (39, 41, 40.0, True):
            rows = records()
            rows[-1]["validated_batches"] = value
            self.reject(rows)

    def test_rejects_unequal_or_duplicate_backends_and_context(self):
        for backend in ("kfd", "hip"):
            logs = [CHECKER.parse_log(encoded(records(name)))
                    for name in ("kfd", "hip", backend)]
            with self.assertRaises(CHECKER.CheckError):
                CHECKER.compare(logs)
        for field, value in (("source_commit", "3" * 40), ("run_id", "3" * 64),
                             ("unique_id", "3" * 16)):
            rows = records("hsa")
            rows[0][field] = value
            logs = [CHECKER.parse_log(encoded(records(name)))
                    for name in ("kfd", "hip")]
            logs.append(CHECKER.parse_log(encoded(rows)))
            with self.assertRaises(CHECKER.CheckError):
                CHECKER.compare(logs)
        for backend in ("other", "HIP", None, []):
            rows = records()
            rows[0]["backend"] = backend
            self.reject(rows)

    def test_rejects_duplicate_keys_bad_json_incomplete_and_oversize_logs(self):
        text = encoded(records())
        for altered in (
            text.replace('"index": 0', '"index": 0, "index": 0', 1),
            text.replace('"backend": "kfd"',
                         '"backend": "kfd", "backend": "kfd"', 1),
            text[:-1], text + "\n", text + "{}\n", "[]\n", "garbage\n",
            " " * (CHECKER.MAX_LOG_BYTES + 1) + "\n",
        ):
            with self.subTest(prefix=altered[:80]):
                with self.assertRaises(CHECKER.CheckError):
                    CHECKER.parse_log(altered)


if __name__ == "__main__":
    unittest.main()
