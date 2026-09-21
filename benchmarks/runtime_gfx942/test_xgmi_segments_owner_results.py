#!/usr/bin/env python3
"""CPU-only receipt mutation and pre-open CLI refusal tests."""

import importlib.util
import os
from pathlib import Path
import subprocess
import unittest

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("owner_results", HERE / "xgmi_segments_owner_results.py")
R = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(R)


class OwnerResults(unittest.TestCase):
    def receipt(self):
        return (f"PASS schema={R.SCHEMA} uid0=0000000000000071 uid1=0000000000000093 " + R.FIELDS + "\n").encode("ascii")

    def test_exact_receipt(self):
        result = R.parse_receipt(self.receipt(), [0x71, 0x93])
        self.assertFalse(result["performance_acceptance"])
        self.assertFalse(result["formal_refinement"])
        self.assertEqual(result["pending_dataflow"], "refused_before_submission")

    def test_malformed_or_overclaiming_receipts_reject(self):
        raw = self.receipt()
        for mutated in [b"", raw + b"\n", raw + raw, raw[:-1], raw.replace(b"\n", b"\r\n"),
                        raw.replace(b"0071", b"0072"), raw.replace(b"0071", b"71"),
                        raw.replace(b"pending_dataflow=refused_before_submission", b"pending_dataflow=supported"),
                        raw.replace(b"journal=enabled", b"journal=disabled"),
                        raw.replace(b"performance_claim=false", b"performance_claim=true")]:
            with self.subTest(raw=mutated), self.assertRaises(ValueError):
                R.parse_receipt(mutated, [0x71, 0x93])
        for field in R.FIELDS.split():
            with self.subTest(field=field), self.assertRaises(ValueError):
                R.parse_receipt(raw.replace(field.encode(), b""), [0x71, 0x93])
        for ids in [[], [1], [1, 1], [0, 2], [True, 2], [2**64, 2], [0x93, 0x71]]:
            with self.subTest(ids=ids), self.assertRaises(ValueError):
                R.parse_receipt(raw, ids)

    @unittest.skipUnless(os.environ.get("FE2O3_OWNER_RUST_BINARY"), "compiled CLI not supplied")
    def test_invalid_cli_rejects_before_native_open(self):
        binary = os.environ["FE2O3_OWNER_RUST_BINARY"]
        for args in [[], ["1"], ["1", "2", "3"], ["0", "2"], ["1", "1"], ["0x", "2"], ["18446744073709551616", "2"]]:
            result = subprocess.run([binary, *args], capture_output=True, timeout=10, check=False)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(result.stdout, b"")
            self.assertIn(b"Error:", result.stderr)
            self.assertFalse(any(word in result.stderr for word in [b"/dev/kfd", b"owner initialization"]))


if __name__ == "__main__":
    unittest.main()
