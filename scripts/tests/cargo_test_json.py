#!/usr/bin/env python3
"""Controls for exact Cargo/libtest evidence, with optional sibling filtering."""
import copy
import json
from pathlib import Path
import runpy
import subprocess
import sys
import tempfile
import tomllib
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / "verify-cargo-test-json.py"
VERIFY = runpy.run_path(str(SCRIPT))["verify"]
TARGET = "fixture-target"
NAME = "fixture::selected"


def valid_events(filtered=0):
    return [
        {"reason": "compiler-artifact", "target": {"name": TARGET, "kind": ["test"]},
         "executable": "/fixture/test"},
        {"reason": "build-finished", "success": True},
        {"type": "suite", "event": "started", "test_count": 1},
        {"type": "test", "event": "started", "name": NAME},
        {"type": "test", "event": "ok", "name": NAME},
        {"type": "suite", "event": "ok", "passed": 1, "failed": 0,
         "ignored": 0, "measured": 0, "filtered_out": filtered},
    ]


def library_events(kinds=None):
    kinds = ["rlib", "dylib"] if kinds is None else kinds
    events = valid_events(91)
    events[0]["target"].update(kind=kinds, crate_types=kinds)
    events[0]["profile"] = {"test": True}
    return events


class CargoTestJsonTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="cargo-test-json-controls-")
        self.addCleanup(self.directory.cleanup)
        self.path = Path(self.directory.name) / "events.jsonl"

    def write(self, events):
        self.path.write_text("".join(json.dumps(row) + "\n" for row in events), encoding="utf-8")

    def verify(self, events, *, filtered=False, name=NAME, library_kinds=None):
        self.write(events)
        VERIFY(self.path, TARGET, name, allow_filtered=filtered, library_kinds=library_kinds)

    def reject(self, events, *, filtered=True, name=NAME, library_kinds=None):
        with self.assertRaises(ValueError):
            self.verify(events, filtered=filtered, name=name, library_kinds=library_kinds)

    def test_default_unfiltered_and_explicit_mode_accept_one_test(self):
        self.verify(valid_events())
        self.verify(valid_events(), filtered=True)

    def test_filtering_requires_explicit_mode(self):
        self.reject(valid_events(91), filtered=False)
        self.verify(valid_events(91), filtered=True)

    def test_one_ordered_long_running_notification_is_informational(self):
        notice = {"type": "test", "event": "timeout", "name": NAME}
        events = valid_events()
        events.insert(4, notice)
        self.verify(events)
        self.verify(events, filtered=True)
        events[-1]["filtered_out"] = 91
        self.verify(events, filtered=True)
        self.reject(events, filtered=False)
        for position in (0, 1, 2, 3, 5, 6):
            events = valid_events()
            events.insert(position, notice)
            self.reject(events)
        events = valid_events()
        events[4:4] = [notice, notice.copy()]
        self.reject(events)
        events = valid_events()
        events.insert(4, {**notice, "name": "fixture::other"})
        self.reject(events)
        for outcome in ("failed", "ignored"):
            events = valid_events()
            events[4]["event"] = outcome
            events.insert(4, notice)
            self.reject(events)

    def test_zero_selected_and_renamed_parent_refuse(self):
        events = valid_events(92)
        events[2]["test_count"] = 0
        events[-1]["passed"] = 0
        del events[3:5]
        self.reject(events)
        self.reject(valid_events(), name="fixture::renamed")

    def test_every_required_event_is_single_and_present(self):
        for index in range(6):
            with self.subTest(index=index):
                events = valid_events()
                del events[index]
                self.reject(events)
                events = valid_events()
                events.insert(index, copy.deepcopy(events[index]))
                self.reject(events)

    def test_extra_unknown_failed_and_ignored_events_refuse(self):
        for extra in [
            {"type": "test", "event": "started", "name": "fixture::other"},
            {"type": "test", "event": "timeout", "name": NAME},
            {"type": "suite", "event": "started", "test_count": 0},
            {"type": "test", "event": "failed", "name": NAME},
            {"type": "test", "event": "ignored", "name": NAME},
            {"type": "unknown", "event": "ok"},
            {"reason": "unknown"},
            {"reason": "compiler-message", "type": "test", "event": "ok", "name": NAME},
        ]:
            with self.subTest(extra=extra):
                self.reject([*valid_events(), extra])

    def test_failed_build_or_parent_cannot_be_promoted(self):
        events = valid_events()
        events[1]["success"] = False
        self.reject(events)
        for event in ("failed", "ignored"):
            events = valid_events()
            events[4]["event"] = event
            self.reject(events)

    def test_suite_counters_are_nonnegative_integers_not_bools(self):
        for index, key in [
            (2, "test_count"), (5, "passed"), (5, "failed"), (5, "ignored"),
            (5, "measured"), (5, "filtered_out"),
        ]:
            for value in (True, False, -1, 0.0, 1.0, "0", None):
                with self.subTest(key=key, value=value):
                    events = valid_events()
                    events[index][key] = value
                    self.reject(events)
            events = valid_events()
            del events[index][key]
            self.reject(events)

    def test_nonzero_failure_ignore_or_measurement_refuses(self):
        for key in ("failed", "ignored", "measured"):
            events = valid_events()
            events[5][key] = 1
            self.reject(events)

    def test_event_order_is_authenticated(self):
        for index in range(5):
            events = valid_events()
            events[index], events[index + 1] = events[index + 1], events[index]
            self.reject(events)

    def test_wrong_or_extra_test_artifact_refuses(self):
        for target in (None, [], {"name": "other", "kind": ["test"]},
                       {"name": TARGET, "kind": ["lib"]}):
            events = valid_events()
            events[0]["target"] = target
            self.reject(events)
        extra = copy.deepcopy(valid_events()[0])
        extra["target"]["name"] = "other"
        self.reject([extra, *valid_events()])
        for executable in (None, "", False):
            extra = copy.deepcopy(valid_events()[0])
            extra["executable"] = executable
            self.reject([extra, *valid_events()])
        extra = {"reason": "compiler-artifact", "target": None}
        self.reject([extra, *valid_events()])

    def test_invalid_json_and_nonobjects_refuse(self):
        for text in ("", "not json\n", "[]\n", "null\n", '{"type":\n'):
            with self.subTest(text=text), self.assertRaises(ValueError):
                self.path.write_text(text, encoding="utf-8")
                VERIFY(self.path, TARGET, NAME)

    def test_cli_filter_optin_preserves_default_refusal(self):
        self.write(valid_events(91))
        argv = [sys.executable, "-I", "-B", str(SCRIPT), str(self.path),
                "--test-target", TARGET, "--test-name", NAME]
        default = subprocess.run(argv, capture_output=True, text=True, timeout=10)
        self.assertEqual(default.returncode, 1)
        explicit = subprocess.run([*argv, "--allow-filtered"],
                                  capture_output=True, text=True, timeout=10)
        self.assertEqual(explicit.returncode, 0, explicit.stderr)
        self.assertIn("verified Cargo/libtest JSON", explicit.stdout)

    def test_explicit_library_roster_preserves_integration_default(self):
        events = library_events()
        self.verify(events, filtered=True, library_kinds=["rlib", "dylib"])
        self.reject(events)
        self.reject(valid_events(), library_kinds=["rlib", "dylib"])
        self.reject(events, filtered=False, library_kinds=["rlib", "dylib"])
        for kinds in (["lib"], ["rlib"], ["proc-macro"]):
            self.verify(library_events(kinds), filtered=True, library_kinds=kinds)
        dependency = copy.deepcopy(events[0])
        dependency["profile"]["test"] = False
        dependency["executable"] = None
        self.verify([dependency, *events], filtered=True, library_kinds=["rlib", "dylib"])

    def test_library_artifact_requires_exact_both_rosters_and_test_profile(self):
        for field in ("kind", "crate_types"):
            for value in (None, [], ["lib"], ["dylib", "rlib"],
                          ["rlib", "dylib", "lib"], ["rlib", "rlib"], "rlib,dylib"):
                with self.subTest(field=field, value=value):
                    events = library_events()
                    events[0]["target"][field] = value
                    self.reject(events, library_kinds=["rlib", "dylib"])
        for profile in (None, {}, {"test": False}, {"test": 1}, {"test": "true"}):
            events = library_events()
            events[0]["profile"] = profile
            self.reject(events, library_kinds=["rlib", "dylib"])
        for kinds in ([], ["bin"], ["test"], ["rlib", "rlib"], ["rlib", ""]):
            self.reject(library_events(), library_kinds=kinds)

    def test_library_selection_rejects_other_or_extra_harnesses(self):
        for extra in (valid_events()[0], library_events(["lib"])[0], library_events()[0]):
            events = library_events()
            extra = copy.deepcopy(extra)
            extra["target"]["name"] = "other"
            self.reject([extra, *events], library_kinds=["rlib", "dylib"])
        for name, executable in (("other", "/fixture/test"), (TARGET, None), (TARGET, "")):
            events = library_events()
            events[0]["target"]["name"] = name
            events[0]["executable"] = executable
            self.reject(events, library_kinds=["rlib", "dylib"])
        for index in range(6):
            events = library_events()
            del events[index]
            self.reject(events, library_kinds=["rlib", "dylib"])
        for outcome in ("failed", "ignored"):
            events = library_events()
            events[4]["event"] = outcome
            self.reject(events, library_kinds=["rlib", "dylib"])

    def test_library_cli_requires_explicit_exact_kind_roster(self):
        self.write(library_events())
        argv = [sys.executable, "-I", "-B", str(SCRIPT), str(self.path),
                "--test-target", TARGET, "--test-name", NAME, "--allow-filtered"]
        for extra, expected in (([], 1), (["--lib", "rlib,dylib"], 0),
                                (["--lib", "dylib,rlib"], 1), (["--lib", "rlib,rlib"], 1)):
            result = subprocess.run([*argv, *extra], capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, expected, result.stderr)

    def test_ci_library_roster_matches_declared_current_backend(self):
        root = SCRIPT.parent.parent
        manifest = tomllib.loads((root / "crates/rustc-codegen-fe2o3/Cargo.toml").read_text())
        kinds = manifest["lib"]["crate-type"]
        self.assertEqual(kinds, ["rlib", "dylib"])
        self.assertEqual(manifest["lib"]["name"], "rustc_codegen_fe2o3")
        pipeline = (root / "scripts/ci-generic-core.sh").read_text()
        parents = (
            "production_rustc_driver_v1::checked_output_source_v1_tests::formal_memory_diagnostic::ordinary_lds_source_retains_owner_bound_execution_discharge",
            "production_semantic_body_v1::slice_constant_index_source_v1_tests::genuine_slice_constant_indices_preserve_retained_mir",
        )
        self.assertEqual(pipeline.count(
            "bash scripts/ci-cargo-test-json.sh --lib " + ",".join(kinds)), len(parents))
        for parent in parents:
            self.assertEqual(pipeline.count(parent), 1)


if __name__ == "__main__":
    unittest.main()
