#!/usr/bin/env python3
"""CPU receipt mutations and compiled invalid-CLI checks; never opens a GPU."""

import importlib.util
import os
from pathlib import Path
import subprocess
import unittest

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("budget_results", HERE / "xgmi_backing_budget_results.py")
R = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(R)
IDS = [0xb7baafd0fb173d8e, 0x10a254ce4987e716]
# Deliberately independent of the parser's constants.
RECEIPT = (
    b"PASS schema=fe2o3.runtime.xgmi-backing-budget.v1 uid0=b7baafd0fb173d8e uid1=10a254ce4987e716 "
    b"device_limits=8192:3,24576:2 coherent_limits=4096:1,8192:2 "
    b"pressure_dimensions=inferred-byte-record capacity_rejections=2 retries=2 "
    b"fresh_identity=context directed_copies=2 statuses=2-succeeded published_snapshots=2 checked_bytes=36875 "
    b"guards=complete n2_mapping_delta=0 n1=0/0-4096/0-4096/4096-0/0 "
    b"request_credits=restored charges=zero cleanup=complete native_execution=true "
    b"exclusive_reservation=false performance_claim=false formal_refinement=false aggregate_bound=false\n"
)


class BackingBudgetResults(unittest.TestCase):
    def test_exact_receipt_and_no_input_alias(self):
        identities = list(IDS)
        parsed = R.parse_receipt(RECEIPT, identities)
        self.assertEqual(parsed, dict(schema="fe2o3.runtime.xgmi-backing-budget.v1", unique_ids=IDS,
            directed_copies=2, capacity_rejections=2, retries=2, checked_bytes=36875,
            pressure_dimensions="inferred-byte-record", fresh_identity="context",
            runtime_cleanup=True, native_execution=True, exclusive_reservation=False,
            performance_acceptance=False, formal_refinement=False, aggregate_bound=False))
        identities[0] = 1
        self.assertEqual(parsed["unique_ids"], IDS)

    def test_each_field_missing_changed_duplicated_or_reordered_rejects(self):
        fields = RECEIPT.rstrip(b"\n").split(b" ")
        for index, field in enumerate(fields):
            variants = [fields[:index] + fields[index + 1:],
                        fields[:index] + [field + b"x"] + fields[index + 1:],
                        fields[:index] + [field, field] + fields[index + 1:]]
            swapped = list(fields)
            other = (index + 1) % len(fields)
            swapped[index], swapped[other] = swapped[other], swapped[index]
            variants.append(swapped)
            for variant in variants:
                with self.subTest(field=field, variant=variant), self.assertRaises(ValueError):
                    R.parse_receipt(b" ".join(variant) + b"\n", IDS)

    def test_framing_historical_receipt_and_overclaims_reject(self):
        for raw in [b"", RECEIPT[:-1], RECEIPT + b"\n", RECEIPT.replace(b"\n", b"\r\n"),
                    b"log\n" + RECEIPT, RECEIPT + b"log\n", bytearray(RECEIPT), RECEIPT.decode(),
                    RECEIPT.replace(b"backing-budget", b"directed-owner"),
                    RECEIPT.replace(b"cleanup=complete", b"cleanup=partial"),
                    RECEIPT.replace(b"formal_refinement=false", b"formal_refinement=true"),
                    RECEIPT.replace(b"performance_claim=false", b"performance_claim=true"),
                    RECEIPT.replace(b"exclusive_reservation=false", b"exclusive_reservation=true"),
                    RECEIPT.replace(b"aggregate_bound=false", b"aggregate_bound=true"),
                    RECEIPT.replace(b"capacity_rejections=2", b"capacity_rejections=02")]:
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                R.parse_receipt(raw, IDS)

    def test_identity_validation_is_exact_and_ordered(self):
        for values in [[], IDS[:1], IDS + [1], [IDS[0]] * 2, [0, IDS[1]], [-1, IDS[1]],
                       [2**64, IDS[1]], [True, IDS[1]], [float(IDS[0]), IDS[1]],
                       [str(IDS[0]), IDS[1]], IDS[::-1], tuple(IDS), None, [[1], IDS[1]]]:
            with self.subTest(values=values), self.assertRaises(ValueError):
                R.parse_receipt(RECEIPT, values)

    @unittest.skipUnless(os.environ.get("FE2O3_BACKING_BUDGET_RUST_BINARY"), "compiled CLI not supplied")
    def test_invalid_cli_rejects_before_native_initialization(self):
        binary = os.environ["FE2O3_BACKING_BUDGET_RUST_BINARY"]
        for args in [[], ["1"], ["1", "2", "3"], ["0", "2"], ["1", "1"],
                     ["0x", "2"], ["18446744073709551616", "2"]]:
            with self.subTest(args=args):
                result = subprocess.run([binary, *args], capture_output=True, timeout=10, check=False)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(result.stdout, b"")
                self.assertIn(b"Error:", result.stderr)
                self.assertNotIn(b"/dev/kfd", result.stderr)


if __name__ == "__main__":
    unittest.main()
