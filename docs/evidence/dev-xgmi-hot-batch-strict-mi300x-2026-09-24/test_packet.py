#!/usr/bin/env python3
"""Run the existing hostile-record controls against strict successful evidence."""

from contextlib import redirect_stdout
import importlib.util
import io
import json
from pathlib import Path
import shutil
import tempfile
from types import ModuleType
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("strict_packet", HERE / "packet.py")
P = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(P)
BASE_PATH, BASE_RAW = P.authenticated("test_verify")
BASE = ModuleType("prior_hot_replay_tests")
BASE.__file__ = str(BASE_PATH)
exec(compile(BASE_RAW, str(BASE_PATH), "exec"), BASE.__dict__)


class StrictReplayTests(BASE.ReplayTests):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="fe2o3-hot-strict-replay-")
        self.addCleanup(self.temporary.cleanup)
        self.check = P.configured("verify")
        self.root = Path(self.temporary.name)
        shutil.copytree(HERE / "raw", self.root / "raw")
        shutil.copytree(HERE / "protocol3", self.root / "protocol3")
        for name in (*self.check.C.PROTOCOL, "artifacts.json"):
            shutil.copy2(HERE / name, self.root / name)
        self.check.ROOT = self.root
        self.run_root = self.root / "raw/native1"
        self.remote = self.run_root / "remote"

    def refresh_remote(self):
        values = self.check.B.inventory(self.remote)
        (self.run_root / "remote-inventory.json").write_text(json.dumps(values))
        path = self.run_root / "local/inventory/stdout"
        path.write_text(json.dumps(values))
        self.rewrite(path.parent / "receipt.json", lambda row: row.update(stdout_sha256=self.check.sha(path)))

    def replay(self):
        with redirect_stdout(io.StringIO()):
            P.verify(self.check)

    def test_original_strict_campaign_is_still_rejected(self):
        original = P.load("verify")
        with self.assertRaisesRegex(ValueError, "strict campaign did not qualify"):
            original.main()

    def test_numeric_collection_success(self):
        self.rewrite(self.run_root / "collection.json", lambda row: row.update(owned_cleanup=1))
        self.reject()

    def test_false_remote_absence(self):
        path = self.run_root / "local/absence/stdout"
        path.write_text('{"path_absent": false, "processes_absent": true}\n')
        self.rewrite(path.parent / "receipt.json", lambda row: row.update(stdout_sha256=self.check.sha(path)))
        self.reject()

    def test_failed_native_transport_is_not_strict_success(self):
        self.rewrite(self.run_root / "local/native/receipt.json", lambda row: row.update(exit=255))
        self.reject()

    def test_incomplete_native_transcript(self):
        path = self.run_root / "local/native/stdout"
        path.write_bytes(path.read_bytes().splitlines(keepends=True)[0])
        self.rewrite(path.parent / "receipt.json", lambda row: row.update(stdout_sha256=self.check.sha(path)))
        self.reject()

    def test_unreaped_local_group(self):
        self.rewrite(self.run_root / "local/cleanup/receipt.json", lambda row: row.update(group_absent=False))
        self.reject()

    def test_extra_local_command(self):
        shutil.copytree(self.run_root / "local/cleanup", self.run_root / "local/extra")
        self.reject()

    def test_failed_remote_removal(self):
        self.rewrite(self.run_root / "collection.json", lambda row: row.update(owned_cleanup=False))
        self.reject()

    def test_local_command_overrun(self):
        self.rewrite(self.run_root / "local/absence/receipt.json", lambda row: row.update(
            finished_ns=row["started_ns"] + (row["timeout_seconds"] + 16) * 10**9))
        self.refresh()
        with self.assertRaisesRegex(ValueError, "bounded strict receipt elapsed time"):
            self.replay()

    def test_remote_command_overrun(self):
        self.rewrite(self.remote / "after-rocm/receipt.json", lambda row: row.update(
            finished_ns=row["started_ns"] + (row["timeout_seconds"] + 16) * 10**9))
        self.refresh_remote()
        self.refresh()
        with self.assertRaisesRegex(ValueError, "bounded strict receipt elapsed time"):
            self.replay()


class BindingTests(unittest.TestCase):
    def test_only_data_roots_change(self):
        verifier = P.configured("verify")
        original = P.load("verify")
        self.assertEqual(verifier.ROOT, HERE)
        self.assertEqual(verifier.LOCAL, P.LOCAL)
        for name in ("PACKET_PATH", "PROTOCOL_COMMIT", "CAMPAIGN_SHA"):
            self.assertEqual(getattr(verifier, name), getattr(original, name))
        package = P.configured("package")
        self.assertEqual(package.HERE, HERE)
        self.assertEqual(package.OWNED, P.LOCAL)

    def test_unknown_helper_role_is_rejected(self):
        with self.assertRaises(KeyError):
            P.load("recovery")

    def test_changed_helper_rejects_before_execution(self):
        with tempfile.TemporaryDirectory(prefix="fe2o3-hot-helper-") as directory:
            root = Path(directory)
            (root / "verify.py").write_text("raise AssertionError('must not execute')\n")
            with patch.object(P, "PRIOR", root), self.assertRaisesRegex(RuntimeError, "signed helper digest"):
                P.load("verify")

    def test_symlinked_helper_rejects_before_execution(self):
        with tempfile.TemporaryDirectory(prefix="fe2o3-hot-helper-") as directory:
            root = Path(directory)
            (root / "verify.py").symlink_to(P.PRIOR / "verify.py")
            with patch.object(P, "PRIOR", root), self.assertRaisesRegex(RuntimeError, "ordinary signed helper"):
                P.load("verify")

    def test_wrong_signed_source_association_rejects(self):
        with patch.object(P, "git", side_effect=[b"", b"different signed source"]), self.assertRaisesRegex(
            RuntimeError, "signed helper source association"
        ):
            P.load("verify")

    def test_failed_signature_rejects_before_helper_execution(self):
        with patch.object(P, "git", side_effect=RuntimeError("signature rejected")), self.assertRaisesRegex(
            RuntimeError, "signature rejected"
        ):
            P.load("verify")


if __name__ == "__main__":
    unittest.main()
