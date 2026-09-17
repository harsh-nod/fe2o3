#!/usr/bin/env python3
"""Tool-only complete-output oracle tests; no compiler or execution authority."""

import copy
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "scripts/check-simulation-expectation.py"


def buffer_value(data="0x00002a42", initialized="0x0f"):
    return {
        "element": "f32", "access": "read_write", "alignment": 4,
        "bytes": data, "initialized": initialized,
    }


def expectation():
    return {
        "schema": "fe2o3-simulation-expectation-v1",
        "arguments": [
            {"kind": "scalar", "type": "u32", "bits": "0x0000002a"},
            {"kind": "buffer", "value": buffer_value()},
            {
                "kind": "buffer_view", "backing": 7, "element": "f32",
                "access": "read_only", "alignment": 4,
                "byte_offset": 4, "elements": 1,
            },
        ],
        "shared_buffers": [{
            "id": 7,
            "buffer": buffer_value("0xa5a5a5a500002a42deadbeef", "0xff0f"),
        }],
    }


def result_for(expected):
    return {
        "schema": "fe2o3-simulation-result-v1", "status": "ok",
        "authority": "observation_only", "simulated": True,
        "hardware_observed": False, "hardware_validation": False,
        "performance_prediction": False,
        "arguments": copy.deepcopy(expected["arguments"]),
        "shared_buffers": copy.deepcopy(expected["shared_buffers"]),
        "counts": {
            "arguments": len(expected["arguments"]),
            "shared_buffers": len(expected["shared_buffers"]),
            "invocations_executed": 1, "workgroups_visited": 1,
            "scheduled_slots_visited": 64, "steps_executed": 7,
            "events_emitted": 0,
        },
    }


def replaced(document, path, value):
    changed = copy.deepcopy(document)
    parent = changed
    for key in path[:-1]:
        parent = parent[key]
    parent[path[-1]] = value
    return changed


class SimulationExpectationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        spec = importlib.util.spec_from_file_location("simulation_expectation", CHECKER)
        cls.checker = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(cls.checker)

    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory(prefix="fe2o3-oracle-test-")
        self.addCleanup(self.scratch.cleanup)
        self.directory = Path(self.scratch.name)
        self.expected = expectation()
        self.actual = result_for(self.expected)

    def write_document(self, name, document):
        path = self.directory / name
        data = document if isinstance(document, bytes) else json.dumps(document).encode("utf-8")
        path.write_bytes(data)
        return path

    def command(self, *arguments):
        return subprocess.run(
            [sys.executable, "-I", "-B", str(CHECKER), *map(str, arguments)],
            capture_output=True, timeout=10, check=False,
        )

    def check(self, expected=None, actual=None, validate_only=False):
        path = self.write_document("expectation.json", self.expected if expected is None else expected)
        if validate_only:
            return self.command("--expectation", path, "--validate-only")
        output = self.write_document("result.json", self.actual if actual is None else actual)
        return self.command("--expectation", path, "--result", output)

    def assert_success(self, completed):
        self.assertEqual(completed.returncode, 0, completed.stderr.decode(errors="replace"))
        self.assertEqual(completed.stdout, b"")
        self.assertEqual(completed.stderr, b"")

    def assert_rejected(self, completed, code=1):
        self.assertEqual(completed.returncode, code, completed.stderr.decode(errors="replace"))
        self.assertEqual(completed.stdout, b"")
        self.assertTrue(completed.stderr)
        self.assertNotIn(b"Traceback", completed.stderr)
        if code == 1:
            self.assertTrue(completed.stderr.startswith(b"simulation expectation:"))
            self.assertLessEqual(len(completed.stderr), 550)

    def test_complete_scalar_buffer_and_shared_view_match(self):
        self.assert_success(self.check())
        self.assert_success(self.check(validate_only=True))
        self.actual["diagnostic_metadata"] = {"not_an_oracle": [1, 2, 3]}
        self.assert_success(self.check())

    def test_empty_rosters_buffers_and_zero_extent_views(self):
        for expected in [
            {"schema": self.expected["schema"], "arguments": [], "shared_buffers": []},
            {
                "schema": self.expected["schema"],
                "arguments": [{"kind": "buffer", "value": buffer_value("0x", "0x")}],
                "shared_buffers": [],
            },
            replaced(self.expected, ("arguments", 2, "elements"), 0),
        ]:
            with self.subTest(expected=expected):
                self.assert_success(self.check(expected, result_for(expected)))

    def test_scalar_bit_patterns_are_exact_including_floating_nan_payloads(self):
        scalars = [("bool", "0x1"), ("i8", "0xff"), ("u128", "0x" + "f" * 32),
                   ("index", "0xffffffffffffffff"), ("f32", "0x7fc01234")]
        for scalar_type, bits in scalars:
            with self.subTest(scalar_type=scalar_type):
                expected = replaced(self.expected, ("arguments", 0), {
                    "kind": "scalar", "type": scalar_type, "bits": bits,
                })
                self.assert_success(self.check(expected, result_for(expected)))

    def test_every_observed_payload_component_is_compared(self):
        mutations = [
            (("arguments", 0, "bits"), "0x0000002b"),
            (("arguments", 0, "type"), "i32"),
            (("arguments", 1, "value", "bytes"), "0x01002a42"),
            (("arguments", 1, "value", "initialized"), "0x07"),
            (("arguments", 1, "value", "alignment"), 8),
            (("arguments", 1, "value", "access"), "read_only"),
            (("arguments", 1, "value", "element"), "u32"),
            (("arguments", 2, "byte_offset"), 0),
            (("arguments", 2, "elements"), 0),
            (("arguments", 2, "access"), "read_write"),
            (("shared_buffers", 0, "buffer", "bytes"), "0x00a5a5a500002a42deadbeef"),
            (("shared_buffers", 0, "buffer", "initialized"), "0xfe0f"),
        ]
        for path, value in mutations:
            with self.subTest(path=path):
                self.assert_rejected(self.check(actual=replaced(self.actual, path, value)))
        swapped = copy.deepcopy(self.actual)
        swapped["arguments"][0:2] = reversed(swapped["arguments"][0:2])
        self.assert_rejected(self.check(actual=swapped))
        substituted = copy.deepcopy(self.actual)
        substituted["arguments"][2]["backing"] = 8
        substituted["shared_buffers"][0]["id"] = 8
        self.assert_rejected(self.check(actual=substituted))

    def test_partial_extra_missing_null_and_wrong_typed_payloads_reject(self):
        mutations = [
            (("arguments",), None), (("shared_buffers",), None),
            (("arguments", 0), None), (("arguments", 0, "bits"), 42),
            (("arguments", 0, "bits"), "0x2a"),
            (("arguments", 0, "bits"), "0x0000002A"),
            (("arguments", 1, "value", "alignment"), True),
            (("arguments", 1, "value", "alignment"), 4.0),
            (("arguments", 1, "value", "alignment"), 3),
            (("arguments", 1, "value", "bytes"), "0x00"),
            (("arguments", 1, "value", "initialized"), "0xff"),
            (("arguments", 1, "value", "initialized"), "0x"),
            (("arguments", 2, "elements"), True),
            (("arguments", 2, "elements"), -1),
            (("arguments", 2, "elements"), 1 << 64),
            (("arguments", 2, "elements"), 3),
            (("arguments", 2, "byte_offset"), 2),
            (("arguments", 2, "backing"), 99),
            (("shared_buffers", 0, "id"), True),
        ]
        for path, value in mutations:
            with self.subTest(path=path, value=value):
                self.assert_rejected(self.check(expected=replaced(self.expected, path, value), validate_only=True))
                self.assert_rejected(self.check(actual=replaced(self.actual, path, value)))
        for key in ["schema", "arguments", "shared_buffers"]:
            malformed = copy.deepcopy(self.expected)
            del malformed[key]
            self.assert_rejected(self.check(expected=malformed, validate_only=True))
        for path in [("arguments", 0), ("arguments", 1, "value"), ("shared_buffers", 0)]:
            malformed = copy.deepcopy(self.expected)
            selected = malformed
            for key in path:
                selected = selected[key]
            selected["extra"] = 0
            self.assert_rejected(self.check(expected=malformed, validate_only=True))
        malformed = copy.deepcopy(self.expected)
        del malformed["arguments"][1]["value"]["initialized"]
        self.assert_rejected(self.check(expected=malformed, validate_only=True))
        malformed = copy.deepcopy(self.expected)
        malformed["arguments"].pop()
        self.assert_rejected(self.check(expected=malformed))
        malformed = copy.deepcopy(self.expected)
        malformed["shared_buffers"] *= 2
        self.assert_rejected(self.check(expected=malformed, validate_only=True))
        malformed = copy.deepcopy(self.expected)
        malformed["partial"] = True
        self.assert_rejected(self.check(expected=malformed, validate_only=True))

    def test_required_observation_metadata_and_all_counts_are_strict(self):
        for key, wrong in [
            ("schema", "fe2o3-simulation-result-v2"), ("status", "failed"),
            ("authority", "compiler"), ("simulated", 1),
            ("hardware_observed", 0), ("hardware_validation", True),
            ("performance_prediction", None),
        ]:
            with self.subTest(key=key):
                self.assert_rejected(self.check(actual=replaced(self.actual, (key,), wrong)))
                missing = copy.deepcopy(self.actual)
                del missing[key]
                self.assert_rejected(self.check(actual=missing))
        for key in self.actual["counts"]:
            for invalid in [True, 1.0, -1, 1 << 64]:
                with self.subTest(key=key, invalid=invalid):
                    self.assert_rejected(self.check(actual=replaced(self.actual, ("counts", key), invalid)))
            missing = copy.deepcopy(self.actual)
            del missing["counts"][key]
            self.assert_rejected(self.check(actual=missing))
        for key in ["arguments", "shared_buffers"]:
            self.assert_rejected(self.check(actual=replaced(self.actual, ("counts", key), 99)))
        extra = copy.deepcopy(self.actual)
        extra["counts"]["future"] = 0
        self.assert_rejected(self.check(actual=extra))
        self.assert_rejected(self.check(actual=replaced(self.actual, ("counts",), None)))

    def test_json_duplicate_nonfinite_utf8_and_document_shape_fail_closed(self):
        invalid = [
            b"", b"{", b"null", b"[]", b"\xff",
            b'{"schema":"a","schema":"b"}',
            b'{"extra":NaN}', b'{"extra":Infinity}', b'{"extra":-Infinity}',
            b'{"extra":1e9999}', b'{} trailing',
        ]
        for raw in invalid:
            with self.subTest(raw=raw):
                self.assert_rejected(self.check(expected=raw, validate_only=True))
                self.assert_rejected(self.check(actual=raw))
        duplicate = json.dumps(self.actual).replace('"simulated": true', '"simulated": true, "simulated": true').encode()
        self.assert_rejected(self.check(actual=duplicate))

    def test_cli_requires_exactly_one_mode(self):
        expected = self.write_document("valid.json", self.expected)
        actual = self.write_document("valid-result.json", self.actual)
        for arguments in [
            [], ["--expectation", expected], ["--result", actual],
            ["--expectation", expected, "--result", actual, "--validate-only"],
        ]:
            with self.subTest(arguments=arguments):
                self.assert_rejected(self.command(*arguments), code=2)

    def test_regular_file_and_byte_limits_are_enforced_for_both_inputs(self):
        expected = self.write_document("valid.json", self.expected)
        actual = self.write_document("valid-result.json", self.actual)
        directory = self.directory / "directory"
        directory.mkdir()
        symlink = self.directory / "symlink"
        symlink.symlink_to(expected)
        fifo = self.directory / "fifo"
        os.mkfifo(fifo)
        for bad in [self.directory / "missing", directory, symlink, fifo]:
            with self.subTest(path=bad.name):
                self.assert_rejected(self.command("--expectation", bad, "--validate-only"))
                self.assert_rejected(self.command("--expectation", expected, "--result", bad))
        for label, maximum in [("expectation", 16 * 1024 * 1024), ("result", 64 * 1024 * 1024)]:
            huge = self.directory / (label + "-too-large")
            with huge.open("wb") as stream:
                stream.truncate(maximum + 1)
            arguments = ["--expectation", huge, "--validate-only"] if label == "expectation" else ["--expectation", expected, "--result", huge]
            self.assert_rejected(self.command(*arguments))
        length = actual.stat().st_size
        self.assertEqual(self.checker.read_document(actual, length, "result"), self.actual)
        with self.assertRaisesRegex(self.checker.CheckError, "byte bounds"):
            self.checker.read_document(actual, length - 1, "result")

    def test_read_errors_and_changed_file_metadata_are_not_success(self):
        path = self.write_document("valid.json", self.expected)
        with mock.patch.object(self.checker.os, "open", side_effect=PermissionError):
            with self.assertRaisesRegex(self.checker.CheckError, "cannot securely read"):
                self.checker.read_document(path, 4096, "expectation")
        actual_fstat = os.fstat
        calls = 0

        def changed_fstat(descriptor):
            nonlocal calls
            calls += 1
            value = actual_fstat(descriptor)
            names = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
            fields = {name: getattr(value, name) for name in names}
            if calls == 2:
                fields["st_mtime_ns"] += 1
            return SimpleNamespace(**fields)

        with mock.patch.object(self.checker.os, "fstat", side_effect=changed_fstat):
            with self.assertRaisesRegex(self.checker.CheckError, "changed while being read"):
                self.checker.read_document(path, 4096, "expectation")

    def test_resource_limits_and_strict_equality(self):
        with mock.patch.object(self.checker, "MAX_ITEMS", 3):
            self.checker.validate_expectation(self.expected)
            too_many = copy.deepcopy(self.expected)
            too_many["arguments"].append(too_many["arguments"][0])
            with self.assertRaisesRegex(self.checker.CheckError, "invalid arguments"):
                self.checker.validate_expectation(too_many)
        with mock.patch.object(self.checker, "MAX_JSON_NODES", 2):
            self.checker.check_json_bounds([0])
            with self.assertRaisesRegex(self.checker.CheckError, "node limit"):
                self.checker.check_json_bounds([0, 1])
        with mock.patch.object(self.checker, "MAX_JSON_DEPTH", 1):
            self.checker.check_json_bounds([0])
            with self.assertRaisesRegex(self.checker.CheckError, "depth limit"):
                self.checker.check_json_bounds([[0]])
        self.assertFalse(self.checker.exactly_equal([True], [1]))
        self.assertFalse(self.checker.exactly_equal({"x": 1}, {"x": 1.0}))
        self.assertFalse(self.checker.exactly_equal([], None))


if __name__ == "__main__":
    unittest.main()
