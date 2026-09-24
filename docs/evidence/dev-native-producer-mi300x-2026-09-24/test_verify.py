#!/usr/bin/env python3
"""Hostile-record tests of the retained producer campaign, without device access."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import importlib.util
import json
import os
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("producer_native_replay_tests", HERE / "verify.py")
V = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(V)


class ReplayTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory(prefix="fe2o3-native-producer-replay-")
        cls.root = Path(cls.temporary.name) / "raw"
        shutil.copytree(HERE / "raw", cls.root)
        shutil.copy2(HERE / "artifacts.json", cls.root.parent / "artifacts.json")

    @classmethod
    def tearDownClass(cls):
        cls.temporary.cleanup()

    def setUp(self):
        self.changed = {}
        self.addCleanup(self.restore)
        self.campaign = self.root / "campaign1"
        pins = V.read(self.campaign / "protocol-before.json")
        self.p, self.m, self.n, self.c = [V.module(self.campaign / "protocol" / (name + ".py"), pins[name + ".py"],
                                                 "producer_test_" + name) for name in ("protocol", "prepare", "native", "campaign")]
        self.p.CASE_NAMES = self.n.selected_cases(self.p)
        self.b = self.p.load_module(self.campaign / "remote/artifacts/base.py", self.p.BASE_SHA, "producer_test_inventory")

    def restore(self):
        for path, raw in self.changed.items():
            path.write_bytes(raw)

    def write(self, path, raw):
        self.changed.setdefault(path, path.read_bytes())
        path.write_bytes(raw)

    def mutate(self, path, transform):
        row = V.read(path)
        transform(row)
        self.write(path, (json.dumps(row, sort_keys=True) + "\n").encode())

    def refresh_remote(self):
        path = self.campaign / "remote-inventory.json"
        self.write(path, json.dumps(V.inventory(self.campaign / "remote"), sort_keys=True).encode())

    def native(self):
        V.verify_native(self.campaign, self.p, self.n, self.b, False)

    def local(self):
        V.verify_local(self.campaign, V.read(self.campaign / "binding.json"), V.read(self.campaign / "owner.json"),
                       self.p, self.m, self.n, self.c, False)

    def test_positive_complete_replay(self):
        result = V.verify(self.root)
        self.assertEqual((result["native_commands"], result["harness_passes"], result["strict_admitted"]), (2, 2, 6))
        self.assertFalse(result["formal_refinement"] or result["performance_acceptance"])
        V.verify_retention(self.root)

    def test_native_status_must_be_integer_success(self):
        self.mutate(self.campaign / "remote/01-queued-producer-test/record.json", lambda row: row.update(status=False))
        self.refresh_remote()
        with self.assertRaisesRegex(ValueError, "closed successful native command"):
            self.native()

    def test_rehashed_incomplete_output_rejects(self):
        path = self.campaign / "remote/01-queued-producer-test/stdout.log"
        self.write(path, path.read_bytes().replace(b"checked_bytes=1048576", b"checked_bytes=4"))
        self.mutate(path.parent / "record.json", lambda row: row.update(stdout_sha256=V.sha(path)))
        self.refresh_remote()
        with self.assertRaisesRegex(ValueError, "exact native case marker"):
            self.native()

    def test_retained_elf_is_not_only_metadata(self):
        path = self.root / "build/runtime-tests"
        self.write(path, b"?" + path.read_bytes()[1:])
        with self.assertRaisesRegex(ValueError, "actual CPU-tested build ELF"):
            V.verify_build(self.root / "build", self.campaign, self.p, self.m)

    def test_reordered_binding_rejects(self):
        path = self.campaign / "binding.json"
        self.mutate(path, lambda row: row["order"].reverse())
        self.mutate(self.campaign / "owner.json", lambda row: row.update(binding_sha256=V.sha(path)))
        with self.assertRaisesRegex(ValueError, "fixed complete native matrix"):
            self.native()

    def test_active_group_identity_cannot_change(self):
        self.mutate(self.campaign / "remote/active-003/receipt.json", lambda row: row.update(pid=row["pid"] + 1))
        self.refresh_remote()
        with self.assertRaisesRegex(ValueError, "active-to-terminal command bijection"):
            self.native()

    def test_false_resource_headroom_rejects(self):
        self.mutate(self.campaign / "remote/01-queued-producer-resources.json", lambda row: row.update(disk_free=True))
        self.refresh_remote()
        with self.assertRaisesRegex(ValueError, "disk headroom"):
            self.native()

    def test_missing_delayed_observation_rejects(self):
        self.mutate(self.campaign / "remote/02-published-producer-delayed/record.json", lambda row: row.update(status=1))
        self.refresh_remote()
        with self.assertRaisesRegex(ValueError, "closed successful native command"):
            self.native()

    def test_complete_flag_cannot_replace_missing_case(self):
        self.mutate(self.campaign / "remote/finished.json", lambda row: row["cases"].pop())
        self.refresh_remote()
        with self.assertRaisesRegex(ValueError, "exact native matrix disposition"):
            self.native()

    def test_cleanup_requires_prior_collection(self):
        collected = V.read(self.campaign / "commands/collect/receipt.json")
        self.mutate(self.campaign / "commands/cleanup/receipt.json", lambda row: row.update(started_ns=collected["started_ns"]))
        with self.assertRaisesRegex(ValueError, "collection precedes cleanup"):
            self.local()

    def test_absence_is_independent_and_boolean(self):
        path = self.campaign / "commands/absence/stdout"
        self.write(path, json.dumps({"path_absent": 1, "processes_absent": True}).encode())
        self.mutate(path.parent / "receipt.json", lambda row: row.update(stdout_sha256=V.sha(path)))
        with self.assertRaisesRegex(ValueError, "independent absence"):
            self.local()


class CleanupTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        spec = importlib.util.spec_from_file_location("producer_native_collector_tests", HERE / "collect.py")
        cls.c = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(cls.c)

    def test_loader_authenticates_before_compile(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "verify.py"
            path.write_text("raise AssertionError('not allowed to execute')\n")
            with patch("builtins.compile") as compiler:
                with self.assertRaisesRegex(RuntimeError, "pinned verifier identity"):
                    self.c.load_verifier(path)
                compiler.assert_not_called()
            alias = Path(temporary) / "alias.py"
            alias.symlink_to(HERE / "verify.py")
            with patch("builtins.compile") as compiler:
                with self.assertRaisesRegex(RuntimeError, "ordinary pinned verifier"):
                    self.c.load_verifier(alias)
                compiler.assert_not_called()

    def test_final_recheck_rejects_changed_added_and_aliased_entries(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "build/commands").mkdir(parents=True)
            (root / "campaign1/commands").mkdir(parents=True)
            path = root / "input"
            path.write_bytes(b"original")
            snapshot = self.c.owned_snapshot(root)
            self.c.recheck_before_removal(root, snapshot)
            path.write_bytes(b"changed!")
            with self.assertRaisesRegex(ValueError, "unchanged before deletion"):
                self.c.recheck_before_removal(root, snapshot)
            path.write_bytes(b"original")
            added = root / "added"
            added.write_bytes(b"uncollected")
            with self.assertRaisesRegex(ValueError, "unchanged before deletion"):
                self.c.recheck_before_removal(root, snapshot)
            added.unlink()
            path.unlink()
            path.symlink_to(HERE / "verify.py")
            with self.assertRaisesRegex(ValueError, "ordinary same-device owned entry"):
                self.c.recheck_before_removal(root, snapshot)

    def test_mount_refuses_owned_snapshot(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(Path, "is_mount", return_value=True):
            with self.assertRaisesRegex(ValueError, "ordinary same-device owned entry"):
                self.c.owned_snapshot(Path(temporary))

    def test_missing_file_and_live_recorded_group_prevent_deletion(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "build/commands/fixture").mkdir(parents=True)
            (root / "campaign1/commands").mkdir(parents=True)
            receipt = root / "build/commands/fixture/receipt.json"
            receipt.write_text(json.dumps({"pid": os.getpgrp()}))
            snapshot = self.c.owned_snapshot(root)
            with self.assertRaisesRegex(RuntimeError, "group exists before deletion"):
                self.c.remove_owned(root, snapshot)
            self.assertTrue(receipt.exists())
            receipt.unlink()
            with self.assertRaisesRegex(ValueError, "unchanged before deletion"):
                self.c.remove_owned(root, snapshot)
            self.assertTrue(root.is_dir())

    def test_actual_owned_removal_and_failure_preservation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "owned"
            (root / "build/commands").mkdir(parents=True)
            (root / "campaign1/commands").mkdir(parents=True)
            snapshot = self.c.owned_snapshot(root)
            extra = root / "uncollected"
            extra.write_bytes(b"retain")
            with self.assertRaisesRegex(ValueError, "unchanged before deletion"):
                self.c.remove_owned(root, snapshot)
            self.assertEqual(extra.read_bytes(), b"retain")
            snapshot = self.c.owned_snapshot(root)
            self.c.remove_owned(root, snapshot)
            self.assertFalse(root.exists())


if __name__ == "__main__":
    unittest.main(verbosity=2)
