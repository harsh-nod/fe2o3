#!/usr/bin/env python3
"""CPU receipt mutations and compiled invalid-CLI checks; never opens a GPU."""

import importlib.util
import os
from pathlib import Path
import subprocess
import unittest

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("directed_results", HERE / "xgmi_directed_owner_results.py")
R = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(R)
IDS = [0xb7baafd0fb173d8e, 0x10a254ce4987e716]
# Deliberately independent of the parser's constants.
RECEIPT = (
    b"PASS schema=fe2o3.runtime.xgmi-directed-owner.v1 uid0=b7baafd0fb173d8e uid1=10a254ce4987e716 "
    b"owner_threads=1 allocations=5 streams=4 directed_copies=4 dependency_edges=4 max_depth=3 "
    b"routes=forward-reverse-reverse-forward journal=enabled pending_dataflow=supported "
    b"adapter=join-tracked events=released-before-progress statuses=4-succeeded rejected=0 "
    b"checked_bytes=327680 payload_bytes=131072 guard_bytes=131072 source_bytes=65536 "
    b"canaries=complete source=unchanged cleanup=complete native_execution=true "
    b"exclusive_reservation=false performance_claim=false formal_refinement=false\n"
)


class DirectedOwnerResults(unittest.TestCase):
    def test_exact_receipt_and_no_input_alias(self):
        identities = list(IDS)
        parsed = R.parse_receipt(RECEIPT, identities)
        self.assertEqual(parsed, dict(schema="fe2o3.runtime.xgmi-directed-owner.v1", unique_ids=IDS,
            directed_copies=4, dependency_edges=4, checked_bytes=327680, owned_cleanup=True,
            version_journal=True, pending_dataflow="supported", native_execution=True,
            events="released-before-progress", exclusive_reservation=False,
            performance_acceptance=False, formal_refinement=False))
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
                    RECEIPT.replace(b"directed-owner", b"segments-owner"),
                    RECEIPT.replace(b"cleanup=complete", b"cleanup=partial"),
                    RECEIPT.replace(b"formal_refinement=false", b"formal_refinement=true"),
                    RECEIPT.replace(b"performance_claim=false", b"performance_claim=true"),
                    RECEIPT.replace(b"exclusive_reservation=false", b"exclusive_reservation=true"),
                    RECEIPT.replace(b"rejected=0", b"rejected=00")]:
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                R.parse_receipt(raw, IDS)

    def test_identity_validation_is_exact_and_ordered(self):
        for values in [[], IDS[:1], IDS + [1], [IDS[0]] * 2, [0, IDS[1]], [-1, IDS[1]],
                       [2**64, IDS[1]], [True, IDS[1]], [float(IDS[0]), IDS[1]],
                       [str(IDS[0]), IDS[1]], IDS[::-1], tuple(IDS), None, [[1], IDS[1]]]:
            with self.subTest(values=values), self.assertRaises(ValueError):
                R.parse_receipt(RECEIPT, values)

    @unittest.skipUnless(os.environ.get("FE2O3_DIRECTED_OWNER_RUST_BINARY"), "compiled CLI not supplied")
    def test_invalid_cli_rejects_before_native_initialization(self):
        binary = os.environ["FE2O3_DIRECTED_OWNER_RUST_BINARY"]
        for args in [[], ["1"], ["1", "2", "3"], ["0", "2"], ["1", "1"],
                     ["0x", "2"], ["18446744073709551616", "2"]]:
            with self.subTest(args=args):
                result = subprocess.run([binary, *args], capture_output=True, timeout=10, check=False)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(result.stdout, b"")
                self.assertIn(b"Error:", result.stderr)
                self.assertNotIn(b"owner initialization", result.stderr)
                self.assertNotIn(b"/dev/kfd", result.stderr)


if __name__ == "__main__":
    unittest.main()
