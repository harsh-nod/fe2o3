#!/usr/bin/env python3
"""Local collection and owned-cleanup controls; no remote work."""

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("hot_batch_package_test", HERE / "package.py")
P = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(P)


class PackageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.owned = self.root / "owned"
        self.owned.mkdir()
        for name in ("source", "target", "logs"):
            (self.owned / name).mkdir()
            (self.owned / name / "file").write_bytes(name.encode())
        (self.owned / "build-source.tar.gz").write_bytes(b"source archive")
        (self.owned / "source-archive.json").write_text(json.dumps({"sha256": P.sha(self.owned / "build-source.tar.gz")}))
        (self.owned / "binding.json").write_text(json.dumps({"commit": P.COMMIT, "local_source_files": P.inventory(self.owned / "source")}))
        (self.owned / "owner.json").write_text(json.dumps({"commit": P.COMMIT,
            "path": "/home/harsh/fe2o3-hot-batch-20260924.0123456789abcdef", "binding_sha256": P.sha(self.owned / "binding.json")}))

    def test_exact_collection_then_scoped_cleanup_retains_logs(self):
        before = P.inventory(self.owned, exclude=P.TRANSIENT)
        rows = P.cleanup_paths(self.owned)
        self.assertEqual(P.collect(self.owned, self.root / "retained"), before)
        receipt = self.root / "cleanup.json"
        P.remove_owned(rows, receipt)
        self.assertEqual(P.inventory(self.owned), before)
        self.assertEqual(P.inventory(self.root / "retained"), before)
        self.assertTrue(all(row["absent"] for row in json.loads(receipt.read_text())))

    def test_target_alias_refuses_before_any_delete(self):
        path = self.owned / "target"
        (path / "file").unlink()
        path.rmdir()
        path.symlink_to(self.owned / "logs", target_is_directory=True)
        with self.assertRaises(RuntimeError):
            P.cleanup_paths(self.owned)
        self.assertEqual((self.owned / "logs/file").read_bytes(), b"logs")

    def test_changed_source_or_archive_refuses_preflight(self):
        for path in (self.owned / "source/file", self.owned / "build-source.tar.gz"):
            before = path.read_bytes()
            path.write_bytes(b"changed")
            with self.assertRaises(RuntimeError):
                P.cleanup_paths(self.owned)
            path.write_bytes(before)
        self.assertTrue((self.owned / "target/file").is_file())

    def test_source_and_binding_substitution_cannot_authorize_cleanup(self):
        (self.owned / "source/file").write_bytes(b"changed source")
        (self.owned / "binding.json").write_text(json.dumps({"commit": P.COMMIT,
            "local_source_files": P.inventory(self.owned / "source")}))
        with self.assertRaisesRegex(RuntimeError, "authenticated cleanup binding"):
            P.cleanup_paths(self.owned)
        self.assertTrue(all((self.owned / name).exists() for name in P.TRANSIENT))

    def test_wrong_owner_shape_or_commit_rejects(self):
        path = self.owned / "owner.json"
        original = json.loads(path.read_text())
        for update in ({"commit": "0" * 40}, {"path": "/tmp/foreign"}, {"extra": True}):
            path.write_text(json.dumps({**original, **update}))
            with self.subTest(update=update), self.assertRaises(RuntimeError):
                P.cleanup_paths(self.owned)

    def test_retained_symlink_refuses_collection(self):
        (self.owned / "logs/link").symlink_to(self.owned / "source/file")
        with self.assertRaises(RuntimeError):
            P.collect(self.owned, self.root / "retained")
        self.assertFalse((self.root / "retained").exists())

    def test_copy_mismatch_never_deletes_transient_state(self):
        original = P.shutil.copy2
        def corrupt(source, destination, *args, **kwargs):
            result = original(source, destination, *args, **kwargs)
            Path(destination).write_bytes(b"corrupted")
            return result
        with mock.patch.object(P.shutil, "copy2", side_effect=corrupt), self.assertRaises(RuntimeError):
            P.collect(self.owned, self.root / "retained")
        self.assertTrue(all((self.owned / name).exists() for name in P.TRANSIENT))

    def test_failed_delete_is_recorded_without_deleting_later_paths(self):
        rows = P.cleanup_paths(self.owned)
        receipt = self.root / "cleanup.json"
        with mock.patch.object(P.shutil, "rmtree", side_effect=OSError("injected")), self.assertRaises(OSError):
            P.remove_owned(rows, receipt)
        self.assertTrue(all(not row["absent"] for row in json.loads(receipt.read_text())))
        self.assertTrue(all((self.owned / name).exists() for name in P.TRANSIENT))

    def test_second_delete_failure_records_the_completed_first_removal(self):
        rows = P.cleanup_paths(self.owned)
        receipt = self.root / "cleanup.json"
        remove = P.shutil.rmtree
        def fail_second(path):
            if path.name == "target":
                raise OSError("second deletion failed")
            remove(path)
        with mock.patch.object(P.shutil, "rmtree", side_effect=fail_second), self.assertRaises(OSError):
            P.remove_owned(rows, receipt)
        self.assertEqual([row["absent"] for row in json.loads(receipt.read_text())], [True, False, False])
        self.assertFalse((self.owned / "source").exists())
        self.assertTrue((self.owned / "target").exists())
        self.assertTrue((self.owned / "build-source.tar.gz").exists())


if __name__ == "__main__":
    unittest.main()
