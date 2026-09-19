#!/usr/bin/env python3
"""Calibrate inherited packet checks and admission-profile rejection rules."""

import sys

if not sys.flags.isolated:
    raise RuntimeError("run calibration with python3 -I")

import hashlib
import importlib.util
from pathlib import Path
import unittest

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
PRIOR = "docs/evidence/dev-xgmi-peer-batch-cpu-2026-09-18/"
PRIOR_TOOLS = {
    PRIOR + "verify.py": "c4f19d2bc23396dc585ebc29e391bfcc96cbabf2e54705e5bb034cf1fcf20442",
    PRIOR + "test_verify.py": "d635c53753462bd2528f6a8040a9aa0a90abe4d59a95ca7723ff9eaaec22dcf8",
}
for name, expected in PRIOR_TOOLS.items():
    path = ROOT / name
    if (
        not path.is_file()
        or path.is_symlink()
        or hashlib.sha256(path.read_bytes()).hexdigest() != expected
    ):
        raise RuntimeError("unauthenticated frozen helper: " + name)

spec = importlib.util.spec_from_file_location("admission_cpu_verify", HERE / "verify.py")
V = importlib.util.module_from_spec(spec)
spec.loader.exec_module(V)
spec = importlib.util.spec_from_file_location(
    "admission_prior_cpu_calibration", ROOT / (PRIOR + "test_verify.py")
)
T = importlib.util.module_from_spec(spec)
spec.loader.exec_module(T)
T.HERE = HERE
T.V = V


class ProfileParser(unittest.TestCase):
    def accepted_output(self):
        rows = [
            f"schema={V.PROFILE_SCHEMA} active={active} requested={requested} "
            f"opposite_ready={opposite} blocked=0 "
            f"reference_ns={'not-run' if active == 65536 else 0} "
            "candidate_ns=0 accepted=true\n"
            for active, requested, opposite in V.PROFILE_CASES
        ]
        return (
            f"\nrunning 1 test\ntest {V.PROFILE_TEST} ... "
            + "".join(rows)
            + "ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; "
            "0 measured; 0 filtered out; finished in 0.00s\n\n"
        )

    def test_exact_rows_and_zero_timings_have_no_performance_threshold(self):
        rows = V.profile_rows(self.accepted_output())
        self.assertEqual(len(rows), 6)
        self.assertEqual(rows[-1]["reference_ns"], None)
        self.assertTrue(all(row["candidate_ns"] == 0 for row in rows))

    def test_row_membership_schema_and_harness_corruption_are_rejected(self):
        good = self.accepted_output()
        start = good.index("schema=")
        first = good[start : good.index("\n", start) + 1]
        mutations = {
            "missing": good.replace(first, "", 1),
            "duplicate": good.replace(first, first + first, 1),
            "different-case": good.replace("active=64 ", "active=65 ", 1),
            "wrong-order": good.replace("active=64 ", "active=256 ", 1),
            "missing-reference": good.replace(
                "reference_ns=0", "reference_ns=not-run", 1
            ),
            "invented-capacity-reference": good.replace(
                "reference_ns=not-run", "reference_ns=1", 1
            ),
            "not-accepted": good.replace("accepted=true", "accepted=false", 1),
            "negative-time": good.replace("candidate_ns=0", "candidate_ns=-1", 1),
            "noncanonical-time": good.replace("candidate_ns=0", "candidate_ns=00", 1),
            "unknown-field": good.replace("accepted=true", "extra=true accepted=true", 1),
            "different-schema": good.replace(V.PROFILE_SCHEMA, "wrong-schema", 1),
            "wrong-test": good.replace(V.PROFILE_TEST, "different_test", 1),
            "failed-harness": good.replace("1 passed; 0 failed", "0 passed; 1 failed", 1),
        }
        for name, output in mutations.items():
            with self.subTest(name=name), self.assertRaises(RuntimeError):
                V.profile_rows(output)


class Calibration(T.Calibration):
    __unittest_skip__ = not (
        (HERE / "binding.json").is_file() and (HERE / "raw").is_dir()
    )
    __unittest_skip_why__ = "recorded admission CPU packet is not prepared yet"

    def test_profile_roster_corruption_is_rejected_after_reseal(self):
        self.change_stdout(
            "gnu-admission-profile",
            lambda output: output.replace("active=64 ", "active=65 ", 1),
        )
        self.reseal()
        self.rejected()

    def test_valid_profile_timing_change_is_rejected_by_frozen_binding(self):
        self.change_stdout(
            "musl-admission-profile",
            lambda output: output.replace("candidate_ns=", "candidate_ns=1", 1),
        )
        self.reseal()
        self.rejected()

    def test_extra_binding_profile_field_is_rejected_after_reseal(self):
        binding = V.read(self.archive / "binding.json")
        binding[V.PROFILE_FIELD]["gnu"]["extra"] = True
        self.write_json(self.archive / "binding.json", binding)
        self.reseal()
        self.rejected()

    def test_boolean_profile_count_is_not_an_integer_after_reseal(self):
        binding = V.read(self.archive / "binding.json")
        binding[V.PROFILE_FIELD]["gnu"]["rows"][0]["requested"] = True
        self.write_json(self.archive / "binding.json", binding)
        self.reseal()
        self.rejected()


if __name__ == "__main__":
    unittest.main()
