#!/usr/bin/env python3
"""Scalar GEMM fixture/oracle checks, not kernel execution or qualification.

Synthetic result documents below exercise the existing complete-output checker.
Actual source-produced simulation, the broader boundary corpus, and GPU evidence
remain separate production-pipeline requirements.
"""

import copy
import importlib.util
import json
import math
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "scripts/check-simulation-expectation.py"
CORPUS = ROOT / "config/tutorial-simulation-v1"
REQUEST = CORPUS / "gfx942-scalar-gemm.request.json"
EXPECTATION = CORPUS / "gfx942-scalar-gemm.expectation.json"


def raw(hexadecimal):
    return bytes.fromhex(hexadecimal.removeprefix("0x"))


def packed(values):
    return b"".join(struct.pack("<f", value) for value in values)


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def request_payload(request):
    # Normalize only the documented request/result buffer envelopes.
    arguments = []
    for argument in request["arguments"]:
        if argument["kind"] == "buffer":
            arguments.append({
                "kind": "buffer",
                "value": {key: value for key, value in argument.items() if key != "kind"},
            })
        else:
            arguments.append(copy.deepcopy(argument))
    return {
        "schema": "fe2o3-simulation-expectation-v1",
        "arguments": arguments,
        "shared_buffers": [
            {"id": item["id"], "buffer": {
                key: value for key, value in item.items() if key != "id"
            }}
            for item in request["shared_buffers"]
        ],
    }


def synthetic_result(expected):
    return {
        "schema": "fe2o3-simulation-result-v1",
        "status": "ok",
        "authority": "observation_only",
        "simulated": True,
        "hardware_observed": False,
        "hardware_validation": False,
        "performance_prediction": False,
        "arguments": copy.deepcopy(expected["arguments"]),
        "shared_buffers": copy.deepcopy(expected["shared_buffers"]),
        "counts": {
            "arguments": len(expected["arguments"]),
            "shared_buffers": len(expected["shared_buffers"]),
            "invocations_executed": 0,
            "workgroups_visited": 0,
            "scheduled_slots_visited": 0,
            "steps_executed": 0,
            "events_emitted": 0,
        },
    }


class ScalarGemmCorpusTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        spec = importlib.util.spec_from_file_location("simulation_expectation", CHECKER)
        cls.checker = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(cls.checker)
        cls.request = cls.checker.read_document(REQUEST, 64 * 1024, "request")
        cls.expected = cls.checker.read_document(EXPECTATION, 64 * 1024, "expectation")

    def setUp(self):
        scratch = tempfile.TemporaryDirectory(prefix="fe2o3-scalar-gemm-corpus-")
        self.addCleanup(scratch.cleanup)
        self.directory = Path(scratch.name)

    def command(self, *arguments):
        return subprocess.run(
            [sys.executable, "-I", "-B", str(CHECKER), *map(str, arguments)],
            capture_output=True, timeout=10, check=False,
        )

    def compare(self, actual, expected=None):
        expectation = EXPECTATION
        if expected is not None:
            expectation = self.directory / "expectation.json"
            expectation.write_text(json.dumps(expected), encoding="utf-8")
        result = self.directory / "synthetic-result.json"
        result.write_text(json.dumps(actual), encoding="utf-8")
        return self.command("--expectation", expectation, "--result", result)

    def assert_passes(self, completed):
        self.assertEqual(completed.returncode, 0, completed.stderr.decode())
        self.assertEqual(completed.stdout, b"")
        self.assertEqual(completed.stderr, b"")

    def assert_mismatch(self, completed, component):
        self.assertEqual(completed.returncode, 1, completed.stderr.decode())
        self.assertEqual(completed.stdout, b"")
        self.assertEqual(
            completed.stderr,
            f"simulation expectation: result: complete {component} mismatch\n".encode(),
        )

    def changed_backing(self, offset, replacement):
        actual = synthetic_result(self.expected)
        buffer = actual["shared_buffers"][0]["buffer"]
        data = bytearray(raw(buffer["bytes"]))
        data[offset:offset + len(replacement)] = replacement
        buffer["bytes"] = "0x" + data.hex()
        return actual

    def test_exact_request_abi_and_launch(self):
        request = self.request
        self.assertEqual(set(request), {
            "schema", "kernel", "grid", "workgroup", "arguments", "shared_buffers",
        })
        self.assertEqual(request["schema"], "fe2o3-simulation-request-v1")
        self.assertEqual(request["kernel"], "scalar_gemm_v1")
        for field in ("grid", "workgroup"):
            self.assertTrue(self.checker.exactly_equal(request[field], [256, 1, 1]), field)
        self.assertEqual(len(request["arguments"]), 6)
        self.assertEqual(request["arguments"][3:], [
            {"kind": "scalar", "type": "u32", "bits": f"0x{value:08x}"}
            for value in (2, 3, 2)
        ])
        self.assertEqual(request["arguments"][2], {
            "kind": "buffer_view", "backing": 7, "element": "f32",
            "access": "read_write", "alignment": 4, "byte_offset": 4, "elements": 6,
        })
        self.checker.validate_expectation(request_payload(request))
        self.checker.validate_expectation(self.expected)

    def test_independent_nonuniform_matrix_oracle(self):
        inputs = []
        for index, literal in enumerate(((1, 2, 3, 4), (5, 6, 7, 8, 9, 10))):
            argument = self.request["arguments"][index]
            self.assertEqual(argument["kind"], "buffer")
            self.assertEqual(argument["element"], "f32")
            self.assertEqual(argument["access"], "read_only")
            self.assertEqual(argument["alignment"], 4)
            values = tuple(value[0] for value in struct.iter_unpack("<f", raw(argument["bytes"])))
            self.assertTrue(all(math.isfinite(value) and value.is_integer() for value in values))
            self.assertEqual(values, literal)
            inputs.append(tuple(int(value) for value in values))
        # Integer dot products are independent of the kernel's f32 loop recurrence.
        rows = (inputs[0][:2], inputs[0][2:])
        columns = tuple(zip(inputs[1][:3], inputs[1][3:], strict=True))
        output = [sum(a * b for a, b in zip(row, column, strict=True))
                  for row in rows for column in columns]
        self.assertEqual(output, [21, 24, 27, 47, 54, 61])
        backing = self.expected["shared_buffers"][0]["buffer"]
        self.assertEqual(raw(backing["bytes"])[4:28], packed(output))

    def test_complete_state_and_initialization_transition(self):
        before = request_payload(self.request)
        self.assertEqual(before["arguments"], self.expected["arguments"])
        self.assertEqual(len(before["shared_buffers"]), 1)
        self.assertEqual(len(self.expected["shared_buffers"]), 1)
        self.assertEqual(before["shared_buffers"][0]["id"], 7)
        self.assertEqual(self.expected["shared_buffers"][0]["id"], 7)
        old = before["shared_buffers"][0]["buffer"]
        new = self.expected["shared_buffers"][0]["buffer"]
        for key in ("element", "access", "alignment"):
            self.assertEqual(old[key], new[key])
        self.assertEqual((new["element"], new["access"], new["alignment"]), ("f32", "read_write", 4))
        old_bytes, new_bytes = raw(old["bytes"]), raw(new["bytes"])
        self.assertEqual(len(old_bytes), 32)
        self.assertEqual(len(new_bytes), 32)
        self.assertEqual(old_bytes[:4], bytes.fromhex("a5a5a5a5"))
        self.assertEqual(old_bytes[28:], bytes.fromhex("deadbeef"))
        self.assertEqual(new_bytes[:4], old_bytes[:4])
        self.assertEqual(new_bytes[28:], old_bytes[28:])
        self.assertEqual(old_bytes[4:28], packed([-1] * 6))
        self.assertEqual(old["initialized"], "0x0f0000f0")
        self.assertEqual(new["initialized"], "0xffffffff")
        for index in (0, 1):
            buffer = before["arguments"][index]["value"]
            count = len(raw(buffer["bytes"]))
            mask = int.from_bytes(raw(buffer["initialized"]), "little")
            self.assertEqual(mask, (1 << count) - 1)

    def test_existing_checker_accepts_fixture_and_synthetic_control(self):
        self.assert_passes(self.command("--expectation", EXPECTATION, "--validate-only"))
        self.assert_passes(self.compare(synthetic_result(self.expected)))

    def test_each_output_value_and_coordinate_corruption_rejects(self):
        data = raw(self.expected["shared_buffers"][0]["buffer"]["bytes"])
        for index in range(6):
            with self.subTest(output=index):
                offset = 4 + index * 4
                self.assert_mismatch(
                    self.compare(self.changed_backing(offset, bytes([data[offset] ^ 1]))),
                    "shared_buffers",
                )
        for left in range(5):
            with self.subTest(swapped_coordinates=left):
                offset = 4 + left * 4
                swapped = data[offset + 4:offset + 8] + data[offset:offset + 4]
                self.assert_mismatch(self.compare(self.changed_backing(offset, swapped)), "shared_buffers")

    def test_every_readonly_input_byte_is_preserved(self):
        for index in (0, 1):
            source = raw(self.expected["arguments"][index]["value"]["bytes"])
            for offset in range(len(source)):
                with self.subTest(argument=index, byte=offset):
                    actual = synthetic_result(self.expected)
                    changed = bytearray(source)
                    changed[offset] ^= 1
                    actual["arguments"][index]["value"]["bytes"] = "0x" + changed.hex()
                    self.assert_mismatch(self.compare(actual), "arguments")
            actual = synthetic_result(self.expected)
            mask = bytearray(raw(actual["arguments"][index]["value"]["initialized"]))
            mask[0] ^= 1
            actual["arguments"][index]["value"]["initialized"] = "0x" + mask.hex()
            self.assert_mismatch(self.compare(actual), "arguments")

    def test_each_canary_byte_and_output_initialization_bit_is_preserved(self):
        data = raw(self.expected["shared_buffers"][0]["buffer"]["bytes"])
        for offset in (*range(4), *range(28, 32)):
            with self.subTest(canary_byte=offset):
                self.assert_mismatch(
                    self.compare(self.changed_backing(offset, bytes([data[offset] ^ 1]))),
                    "shared_buffers",
                )
        for offset in range(4, 28):
            with self.subTest(uninitialized_byte=offset):
                actual = synthetic_result(self.expected)
                mask = (0xffffffff ^ (1 << offset)).to_bytes(4, "little")
                actual["shared_buffers"][0]["buffer"]["initialized"] = "0x" + mask.hex()
                self.assert_mismatch(self.compare(actual), "shared_buffers")

    def test_skipped_stores_reject_even_when_reported_initialized(self):
        for index in range(6):
            with self.subTest(skipped_output=index):
                self.assert_mismatch(
                    self.compare(self.changed_backing(4 + index * 4, packed([-1]))),
                    "shared_buffers",
                )
        actual = synthetic_result(request_payload(self.request))
        self.assert_mismatch(self.compare(actual), "shared_buffers")

    def test_scalar_view_and_argument_substitution_reject(self):
        for index in (3, 4, 5):
            with self.subTest(scalar=index):
                actual = synthetic_result(self.expected)
                actual["arguments"][index]["bits"] = "0x00000001"
                self.assert_mismatch(self.compare(actual), "arguments")
        for field, replacement in (("byte_offset", 0), ("elements", 5), ("access", "read_only")):
            with self.subTest(view=field):
                actual = synthetic_result(self.expected)
                actual["arguments"][2][field] = replacement
                self.assert_mismatch(self.compare(actual), "arguments")
        actual = synthetic_result(self.expected)
        actual["arguments"][:2] = reversed(actual["arguments"][:2])
        self.assert_mismatch(self.compare(actual), "arguments")
        actual = synthetic_result(self.expected)
        actual["arguments"][2]["backing"] = 8
        actual["shared_buffers"][0]["id"] = 8
        self.assert_mismatch(self.compare(actual), "arguments")

    def test_positive_zero_and_separate_rounding_contract_controls(self):
        epsilon = 2.0 ** -23
        left, right = f32(1.0 + epsilon), f32(1.0 - epsilon)
        self.assertEqual(packed([left, right]).hex(), "0100803ffeff7f3f")
        ordered = f32(f32(-1.0) + f32(left * right))
        contracted = f32(-1.0 + left * right)
        self.assertEqual(packed([ordered]), bytes(4))
        self.assertEqual(contracted, -(2.0 ** -46))
        self.assertNotEqual(packed([ordered]), packed([contracted]))
        # These are comparator controls, not additional source execution cases.
        expected = copy.deepcopy(self.expected)
        buffer = expected["shared_buffers"][0]["buffer"]
        data = bytearray(raw(buffer["bytes"]))
        data[4:8] = packed([ordered])
        buffer["bytes"] = "0x" + data.hex()
        self.assert_passes(self.compare(synthetic_result(expected), expected))
        for wrong in (-0.0, contracted):
            actual = synthetic_result(expected)
            data[4:8] = packed([wrong])
            actual["shared_buffers"][0]["buffer"]["bytes"] = "0x" + data.hex()
            self.assert_mismatch(self.compare(actual, expected), "shared_buffers")


if __name__ == "__main__":
    unittest.main()
