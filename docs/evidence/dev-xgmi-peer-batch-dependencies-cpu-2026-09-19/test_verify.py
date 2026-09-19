#!/usr/bin/env python3
"""Calibrate inherited packet checks and dependency-profile rejection rules."""

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

spec = importlib.util.spec_from_file_location(
    "dependencies_cpu_verify", HERE / "verify.py"
)
V = importlib.util.module_from_spec(spec)
spec.loader.exec_module(V)
spec = importlib.util.spec_from_file_location(
    "dependencies_prior_cpu_calibration", ROOT / (PRIOR + "test_verify.py")
)
T = importlib.util.module_from_spec(spec)
spec.loader.exec_module(T)
T.HERE = HERE
T.V = V


class ScalingRosters(unittest.TestCase):
    def test_exact_frozen_families_allow_unrelated_runtime_tests(self):
        self.assertEqual(len(V.DEPENDENCY_TESTS), 10)
        self.assertEqual(len(V.ADMISSION_TESTS), 8)
        self.assertIn(V.PROFILE_TEST, V.DEPENDENCY_TESTS)
        V.scaling_rosters(
            sorted(V.DEPENDENCY_TESTS | V.ADMISSION_TESTS | {"context::other_test"})
        )

    def test_same_family_omission_substitution_and_addition_are_rejected(self):
        names = V.DEPENDENCY_TESTS | V.ADMISSION_TESTS
        for prefix, expected, label in (
            (V.DEPENDENCY_PREFIX, V.DEPENDENCY_TESTS, "dependency"),
            (V.ADMISSION_PREFIX, V.ADMISSION_TESTS, "admission"),
        ):
            original = min(
                name for name in expected if not name.endswith("_profile_rows")
            )
            replacement = prefix + "unreviewed_same_family_test"
            for mutation, changed in (
                ("omission", names - {original}),
                ("substitution", (names - {original}) | {replacement}),
                ("addition", names | {replacement}),
            ):
                with self.subTest(family=label, mutation=mutation):
                    with self.assertRaisesRegex(
                        RuntimeError, "exact " + label + "-scaling test roster"
                    ):
                        V.scaling_rosters(sorted(changed))


class ProfileParser(unittest.TestCase):
    def accepted_output(self):
        rows = [
            f"schema={V.PROFILE_SCHEMA} case={case} active={active} "
            f"requested={requested} requested_dependency_edges={edges} "
            f"blocked_waiters={waiters} "
            f"reference_ns={'not-run' if active == 65536 else 0} "
            "candidate_ns=0 result=valid\n"
            for case, active, requested, edges, waiters in V.PROFILE_CASES
        ]
        return (
            f"\nrunning 1 test\ntest {V.PROFILE_TEST} ... "
            + "".join(rows)
            + "ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; "
            "0 measured; 0 filtered out; finished in 0.00s\n\n"
        )

    def test_exact_rows_and_zero_timings_have_no_performance_threshold(self):
        rows = V.profile_rows(self.accepted_output())
        self.assertEqual(len(rows), 5)
        self.assertIsNone(rows[-1]["reference_ns"])
        self.assertTrue(all(row["candidate_ns"] == 0 for row in rows))
        slow = self.accepted_output().replace(
            "candidate_ns=0", "candidate_ns=999999999"
        )
        self.assertTrue(
            all(row["candidate_ns"] == 999999999 for row in V.profile_rows(slow))
        )

    def test_row_membership_schema_and_harness_corruption_are_rejected(self):
        good = self.accepted_output()
        start = good.index("schema=")
        first = good[start : good.index("\n", start) + 1]
        mutations = {
            "missing": good.replace(first, "", 1),
            "duplicate": good.replace(first, first + first, 1),
            "different-case": good.replace("case=one-empty-64", "case=unknown", 1),
            "wrong-order": good.replace("case=one-empty-64", "case=one-four-4096", 1),
            "different-active": good.replace("active=64 ", "active=65 ", 1),
            "different-edges": good.replace(
                "requested_dependency_edges=0", "requested_dependency_edges=1", 1
            ),
            "different-waiters": good.replace(
                "blocked_waiters=63", "blocked_waiters=62", 1
            ),
            "missing-reference": good.replace(
                "reference_ns=0", "reference_ns=not-run", 1
            ),
            "invented-capacity-reference": good.replace(
                "reference_ns=not-run", "reference_ns=1", 1
            ),
            "invalid-result": good.replace("result=valid", "result=invalid", 1),
            "negative-time": good.replace("candidate_ns=0", "candidate_ns=-1", 1),
            "noncanonical-time": good.replace("candidate_ns=0", "candidate_ns=00", 1),
            "unknown-field": good.replace("result=valid", "extra=true result=valid", 1),
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
    __unittest_skip_why__ = "recorded dependency CPU packet is not prepared yet"

    def test_profile_roster_corruption_is_rejected_after_reseal(self):
        self.change_stdout(
            "gnu-dependency-profile",
            lambda output: output.replace("active=64 ", "active=65 ", 1),
        )
        self.reseal()
        self.rejected()

    def test_valid_profile_timing_change_is_rejected_by_frozen_binding(self):
        self.change_stdout(
            "musl-dependency-profile",
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

    def test_rebased_cross_target_rosters_still_require_dependency_allowlist(self):
        original = (
            V.DEPENDENCY_PREFIX
            + "dependency_use_count_overflow_is_reported_without_wrapping"
        )
        replacement = V.DEPENDENCY_PREFIX + "unreviewed_same_family_test"
        for mutation in ("omission", "substitution"):
            with self.subTest(mutation=mutation):
                self.reset_archive()
                names = V.V.roster(
                    (self.archive / "raw/gnu-runtime-roster/stdout").read_text()
                )
                self.assertIn(original, names)
                changed = sorted(
                    (set(names) - {original})
                    | ({replacement} if mutation == "substitution" else set())
                )

                def change_roster(output):
                    replacement_row = (
                        replacement + ": test\n" if mutation == "substitution" else ""
                    )
                    return (
                        output.replace(original + ": test\n", replacement_row, 1)
                        .replace(
                            f"{len(names)} tests, 0 benchmarks\n",
                            f"{len(changed)} tests, 0 benchmarks\n",
                            1,
                        )
                    )

                def change_passing(output):
                    replacement_row = (
                        "test " + replacement + " ... ok\n"
                        if mutation == "substitution"
                        else ""
                    )
                    return (
                        output.replace(
                            "test " + original + " ... ok\n", replacement_row, 1
                        )
                        .replace(
                            f"running {len(names)} tests\n",
                            f"running {len(changed)} tests\n",
                            1,
                        )
                        .replace(
                            f"test result: ok. {len(names)} passed;",
                            f"test result: ok. {len(changed)} passed;",
                            1,
                        )
                    )

                for target in ("gnu", "musl"):
                    self.change_stdout(target + "-runtime-roster", change_roster)
                    self.change_stdout(target + "-runtime", change_passing)
                binding = V.read(self.archive / "binding.json")
                binding["rosters"]["runtime"] = V.V.roster_binding(changed)
                self.write_json(self.archive / "binding.json", binding)
                self.reseal()
                with self.assertRaisesRegex(
                    RuntimeError, "exact dependency-scaling test roster"
                ):
                    V.verify_bundle(self.archive)


if __name__ == "__main__":
    unittest.main()
