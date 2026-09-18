#!/usr/bin/env python3
"""Calibrate strict roster and outcome parsing, including missing or extra rows."""

import unittest
from pathlib import Path
import tempfile
import verify

ROSTER = (verify.ARCHIVE / "raw/gnu-roster.log").read_text()
EXPECTED = verify.parse_roster(ROSTER)


def harness():
    return ("running 136 tests\n" + "".join(f"test {name} ... ok\n" for name in sorted(EXPECTED))
            + "\ntest result: ok. 136 passed; 0 failed; 0 ignored; 0 measured; 1302 filtered out; finished in 1.00s\n\n")


def parse(text):
    return verify.helper.parse_harness(text, EXPECTED, 1302)


class Calibration(unittest.TestCase):
    def rejects(self, function, text):
        with self.assertRaises((AssertionError, ValueError, IndexError)):
            function(text)

    def test_complete_harness(self):
        self.assertEqual(parse(harness()), dict.fromkeys(EXPECTED, "ok"))

    def test_malformed_harnesses(self):
        text = harness()
        first, second = sorted(EXPECTED)[:2]
        for mutated in [
            text[:text.index("test result:")], text + "trailing\n",
            text.replace(" ... ok", " ... FAILED", 1),
            text.replace(" ... ok", " ... ignored", 1),
            text.replace(first, "wrong::test", 1), text.replace(first, second, 1),
            text.replace("1302 filtered", "1301 filtered"),
            text.replace("136 passed", "135 passed"),
            text[:text.index("test result:")] + "\0" * 256,
        ]:
            with self.subTest(mutated=mutated):
                self.rejects(parse, mutated)

    def test_complete_roster(self):
        self.assertEqual(verify.parse_roster(ROSTER), EXPECTED)

    def test_malformed_rosters(self):
        first, second = sorted(EXPECTED)[:2]
        for mutated in [ROSTER.replace(first, second), ROSTER + "extra\n",
                        ROSTER.replace("136 tests", "135 tests"), ROSTER.replace(first, "other::test")]:
            self.rejects(verify.parse_roster, mutated)

    def test_existing_seal_cannot_be_bypassed_or_overwritten(self):
        with tempfile.TemporaryDirectory(prefix="fe2o3-logical-mux-seal-") as directory:
            seal = Path(directory) / "SHA256SUMS"
            with self.assertRaises(ValueError):
                verify.verify_seal(seal, "expected\n")
            verify.verify_seal(seal, "expected\n", allow_absent=True)
            self.assertFalse(seal.exists())
            verify.verify_seal(seal, "expected\n", create=True)
            for allow_absent in [False, True]:
                verify.verify_seal(seal, "expected\n", allow_absent=allow_absent)
                with self.assertRaises(ValueError):
                    verify.verify_seal(seal, "tampered\n", allow_absent=allow_absent)
            with self.assertRaises(FileExistsError):
                verify.verify_seal(seal, "tampered\n", create=True)
            self.assertEqual(seal.read_text(), "expected\n")


if __name__ == "__main__":
    unittest.main()
