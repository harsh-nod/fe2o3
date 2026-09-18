#!/usr/bin/env python3
"""Calibrate strict roster and outcome parsing, including missing or extra rows."""

import unittest
from unittest.mock import patch
from pathlib import Path
import tempfile
import verify

ROSTER = (verify.ARCHIVE / "raw/gnu-roster.log").read_text()
EXPECTED = verify.parse_roster(ROSTER)


def harness():
    return ("running 227 tests\n" + "".join(f"test {name} ... ok\n" for name in sorted(EXPECTED))
            + "\ntest result: ok. 227 passed; 0 failed; 0 ignored; 0 measured; 1216 filtered out; finished in 1.00s\n\n")


def parse(text):
    return verify.helper.parse_harness(text, EXPECTED, 1216)


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
            text.replace("1216 filtered", "1215 filtered"),
            text.replace("227 passed", "226 passed"),
            text[:text.index("test result:")] + "\0" * 256,
        ]:
            with self.subTest(mutated=mutated):
                self.rejects(parse, mutated)

    def test_complete_roster(self):
        self.assertEqual(verify.parse_roster(ROSTER), EXPECTED)

    def test_malformed_rosters(self):
        first, second = sorted(EXPECTED)[:2]
        for mutated in [ROSTER.replace(first, second), ROSTER + "extra\n",
                        ROSTER.replace("227 tests", "226 tests"), ROSTER.replace(first, "other::test")]:
            self.rejects(verify.parse_roster, mutated)

    def test_existing_seal_cannot_be_bypassed_or_overwritten(self):
        with tempfile.TemporaryDirectory(prefix="fe2o3-creation-escrow-seal-") as directory:
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



    def test_archive_membership_and_symlinks_fail_closed(self):
        with tempfile.TemporaryDirectory(prefix="fe2o3-escrow-membership-") as directory:
            root = Path(directory)
            names = {".gitattributes", "README.md", "record.sh", "qualify.sh", "verify.py", "test_verify.py"}
            names.update(f"raw/{name}.{suffix}" for name in verify.commands()
                         for suffix in ("command", "exit", "started", "finished", "log"))
            names.update(f"interrupted/{name}.{suffix}" for name in verify.INTERRUPTED
                         for suffix in ("command", "exit", "started", "finished", "log"))
            for name in names:
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("fixture\n")
            with patch.object(verify, "ARCHIVE", root):
                original = verify.manifest()
                self.assertEqual(len(original.splitlines()), 121)
                (root / "SHA256SUMS").write_text("root seal excluded\n")
                self.assertEqual(verify.manifest(), original)
                for name in ("unexpected", "raw/SHA256SUMS"):
                    extra = root / name
                    extra.write_text("not part of archive\n")
                    with self.assertRaises(ValueError):
                        verify.manifest()
                    extra.unlink()
                missing = root / "raw/source-after.exit"
                missing.unlink()
                with self.assertRaises(ValueError):
                    verify.manifest()
                missing.write_text("fixture\n")
                self.assertEqual(verify.manifest(), original)
                missing.unlink()
                missing.symlink_to("source-before.exit")
                with self.assertRaises(ValueError):
                    verify.manifest()

    def test_receipt_commands_statuses_and_timestamps_fail_closed(self):
        with tempfile.TemporaryDirectory(prefix="fe2o3-escrow-receipt-") as directory:
            root = Path(directory)
            (root / "raw").mkdir()
            values = {
                "command": "cargo test --frozen\n",
                "exit": "0\n",
                "started": "2026-09-18T20:00:00.000000000Z\n",
                "finished": "2026-09-18T20:00:01.000000000Z\n",
            }
            for suffix, value in values.items():
                (root / f"raw/sample.{suffix}").write_text(value)
            with patch.object(verify.helper, "ARCHIVE", root):
                verify.helper.receipt("sample", ["cargo", "test", "--frozen"])
                for suffix, value in [
                    ("command", "cargo build --frozen\n"),
                    ("exit", "1\n"),
                    ("started", "not a timestamp\n"),
                    ("finished", "2026-09-18T19:59:59.000000000Z\n"),
                ]:
                    path = root / f"raw/sample.{suffix}"
                    path.write_text(value)
                    with self.assertRaises(ValueError):
                        verify.helper.receipt("sample", ["cargo", "test", "--frozen"])
                    path.write_text(values[suffix])


if __name__ == "__main__":
    unittest.main()
