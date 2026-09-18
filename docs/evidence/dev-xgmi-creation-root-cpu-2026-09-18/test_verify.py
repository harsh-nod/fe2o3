#!/usr/bin/env python3
"""Calibrate exact roster, harness, receipt, and seal rejection."""

from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import verify


class Calibration(unittest.TestCase):
    def test_rosters_reject_extra_missing_duplicate_and_wrong_names(self):
        for kind, suffix in [("kfd", "roster"), ("runtime", "runtime-roster")]:
            text = (verify.ARCHIVE / f"raw/gnu-{suffix}.log").read_text()
            names = verify.parse_roster(text, kind)
            first, second = sorted(names)[:2]
            for bad in [text + "extra\n", text.replace(first, second), text.replace(first, "other::test"), text.replace(f"{first}: test\n", "")]:
                with self.assertRaises(ValueError):
                    verify.parse_roster(bad, kind)

    def test_harnesses_reject_partial_failed_ignored_and_duplicate_results(self):
        for kind, suffix in [("kfd", "roster"), ("runtime", "runtime-roster")]:
            names = verify.parse_roster((verify.ARCHIVE / f"raw/gnu-{suffix}.log").read_text(), kind)
            text = (verify.ARCHIVE / f"raw/gnu-{kind}.log").read_text()
            filtered = verify.ROSTERS[kind][1]
            verify.helper.parse_harness(text, names, filtered)
            first, second = sorted(names)[:2]
            for bad in [text[:text.index("test result:")], text + "trailing\n", text.replace(" ... ok", " ... FAILED", 1), text.replace(" ... ok", " ... ignored", 1), text.replace(first, second)]:
                with self.assertRaises((ValueError, AssertionError, IndexError)):
                    verify.helper.parse_harness(bad, names, filtered)

    def test_seals_cannot_be_bypassed_or_overwritten(self):
        with tempfile.TemporaryDirectory(prefix="fe2o3-xgmi-seal-") as directory:
            seal = Path(directory) / "SHA256SUMS"
            with self.assertRaises(ValueError):
                verify.verify_seal(seal, "expected\n")
            verify.verify_seal(seal, "expected\n", allow_absent=True)
            verify.verify_seal(seal, "expected\n", create=True)
            verify.verify_seal(seal, "expected\n")
            with self.assertRaises(ValueError):
                verify.verify_seal(seal, "tampered\n", allow_absent=True)
            with self.assertRaises(FileExistsError):
                verify.verify_seal(seal, "tampered\n", create=True)

    def test_receipt_commands_status_and_chronology(self):
        with tempfile.TemporaryDirectory(prefix="fe2o3-xgmi-receipt-") as directory:
            root = Path(directory)
            (root / "raw").mkdir()
            values = {"command": "cargo test --frozen\n", "exit": "0\n",
                      "started": "2026-09-18T20:00:00.000000000Z\n", "finished": "2026-09-18T20:00:01.000000000Z\n"}
            for suffix, value in values.items():
                (root / f"raw/sample.{suffix}").write_text(value)
            with patch.object(verify.helper, "ARCHIVE", root):
                verify.helper.receipt("sample", ["cargo", "test", "--frozen"])
                for suffix, value in [("command", "cargo build --frozen\n"), ("exit", "1\n"), ("finished", "2026-09-18T19:59:59.000000000Z\n")]:
                    path = root / f"raw/sample.{suffix}"
                    path.write_text(value)
                    with self.assertRaises(ValueError):
                        verify.helper.receipt("sample", ["cargo", "test", "--frozen"])
                    path.write_text(values[suffix])
        with patch.object(verify, "commands", return_value={"first": [], "second": []}):
            with patch.object(verify.helper, "receipt", side_effect=[("1", "2"), ("2", "3")]):
                verify.check_receipts()
            with patch.object(verify.helper, "receipt", side_effect=[("1", "3"), ("2", "4")]):
                with self.assertRaises(ValueError):
                    verify.check_receipts()

    def membership_fixture(self, root):
        names = {".gitattributes", "README.md", "record.sh", "qualify.sh", "verify.py", "test_verify.py"}
        names.update(f"raw/{name}.{suffix}" for name in verify.commands()
                     for suffix in ["command", "exit", "started", "finished", "log"])
        for name in names:
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("fixture\n")

    def test_membership_rejects_missing_and_extra_files(self):
        with tempfile.TemporaryDirectory(prefix="fe2o3-xgmi-members-") as directory:
            root = Path(directory)
            self.membership_fixture(root)
            with patch.object(verify, "ARCHIVE", root):
                original = verify.manifest()
                (root / "SHA256SUMS").write_text("excluded root seal\n")
                self.assertEqual(verify.manifest(), original)
                (root / "raw/SHA256SUMS").write_text("unexpected nested seal\n")
                with self.assertRaises(ValueError):
                    verify.manifest()
                (root / "raw/SHA256SUMS").unlink()
                (root / "empty").mkdir()
                with self.assertRaises(ValueError):
                    verify.manifest()
                (root / "empty").rmdir()
                (root / "extra").write_text("extra\n")
                with self.assertRaises(ValueError):
                    verify.manifest()
                (root / "extra").unlink()
                self.assertEqual(verify.manifest(), original)
                (root / "raw/source-after.exit").unlink()
                with self.assertRaises(ValueError):
                    verify.manifest()

    def test_membership_rejects_symlinks(self):
        with tempfile.TemporaryDirectory(prefix="fe2o3-xgmi-symlink-") as directory:
            root = Path(directory)
            self.membership_fixture(root)
            with patch.object(verify, "ARCHIVE", root):
                verify.manifest()
                path = root / "raw/source-after.exit"
                path.unlink()
                path.symlink_to("source-before.exit")
                with self.assertRaises(ValueError):
                    verify.manifest()


if __name__ == "__main__":
    unittest.main()
