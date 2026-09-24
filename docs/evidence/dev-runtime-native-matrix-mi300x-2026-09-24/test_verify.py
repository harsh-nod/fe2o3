#!/usr/bin/env python3
"""Hostile-record controls for native replay; never access a GPU or remote host."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import copy
import importlib.util
import json
import os
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("matrix_verify_tests", HERE / "verify.py")
V = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(V)


class ReplayTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory(prefix="fe2o3-native-replay-")
        cls.root = Path(cls.temporary.name) / "raw"
        shutil.copytree(HERE / "raw", cls.root)
        shutil.copy2(HERE / "artifacts.json", cls.root.parent / "artifacts.json")

    @classmethod
    def tearDownClass(cls):
        cls.temporary.cleanup()

    def setUp(self):
        self.changed = {}
        self.addCleanup(self.restore)
        self.campaign = self.root / "campaign3"
        self.rejected = False
        self.load()

    def restore(self):
        for path, raw in self.changed.items():
            if raw is None:
                path.unlink()
            else:
                path.write_bytes(raw)

    def write(self, path, raw):
        self.changed.setdefault(path, path.read_bytes() if path.exists() else None)
        path.write_bytes(raw)

    def json(self, path, value):
        self.write(path, (json.dumps(value, sort_keys=True) + "\n").encode())

    def mutate(self, path, change):
        row = V.read(path)
        change(row)
        self.json(path, row)

    def load(self):
        before = V.read(self.campaign / "protocol-before.json")
        self.p, self.m, self.n, self.c = [V.module(self.campaign / "protocol" / (name + ".py"), before[name + ".py"],
                                                 "test_matrix_" + name) for name in ("protocol", "prepare", "native", "campaign")]
        if not self.rejected:
            self.p.CASE_NAMES = self.n.selected_cases(self.p)
        self.b = self.p.load_module(self.campaign / "remote/artifacts/base.py", self.p.BASE_SHA, "test_matrix_base")

    def refresh_remote(self):
        value = V.inventory(self.campaign / "remote")
        self.json(self.campaign / "remote-inventory.json", value)
        path = self.campaign / "commands/inventory/stdout"
        self.json(path, value)
        self.mutate(path.parent / "receipt.json", lambda row: row.update(stdout_sha256=V.sha(path)))

    def native(self):
        return V.verify_native(self.campaign, self.p, self.n, self.b, self.rejected)

    def local(self):
        return V.verify_local(self.campaign, V.read(self.campaign / "binding.json"), V.read(self.campaign / "owner.json"),
                              self.p, self.m, self.n, self.c, self.rejected)

    def test_original_complete_replay_and_cleanup(self):
        result = V.verify(self.root)
        self.assertFalse(result["original_matrix_accepted"])
        self.assertTrue(result["separate_suffix_accepted"])
        self.assertEqual(result["strict_admitted"], 65)
        self.assertEqual(result["refused_observations"], 1)
        V.verify_retention(self.root)

    def test_cohort_join_rejects_different_device_or_roster(self):
        prefix = V.verify_campaign(self.root, "campaign2", True)
        for key in ("identity", "roster", "protocol", "payload", "build"):
            with self.subTest(key=key):
                suffix = copy.deepcopy(prefix)
                suffix[key] = "substituted"
                with patch.object(V, "verify_campaign", side_effect=[prefix, suffix]), self.assertRaisesRegex(ValueError, "across cohorts"):
                    V.verify(self.root)

    def test_retained_build_elf_is_not_just_metadata(self):
        path = self.root / "build/runtime-tests"
        raw = path.read_bytes()
        self.write(path, b"?" + raw[1:])
        with self.assertRaisesRegex(ValueError, "actual CPU-tested build ELF"):
            V.verify_build(self.root / "build", self.campaign, self.p, self.m)

    def test_coordinated_roster_and_command_map_mismatch(self):
        identity = V.verify_campaign(self.root, "campaign2", True)
        identity["tests"]["two-stream"] = identity["tests"]["typed"]
        with patch.object(V, "verify_campaign", return_value=identity), self.assertRaisesRegex(ValueError, "executed test-name map"):
            V.verify(self.root)

    def test_public_signer_is_pinned_before_git(self):
        self.write(self.root / "build/allowed-signers", b"substituted public key\n")
        with self.assertRaisesRegex(ValueError, "pinned expected source signer"):
            V.signed_source(self.root / "build", self.p, self.m)

    def test_extracted_source_rejects_special_nodes(self):
        source = self.root / "build/source"
        source.mkdir()
        self.addCleanup(shutil.rmtree, source)
        os.mkfifo(source / "not-source")
        with self.assertRaisesRegex(ValueError, "ordinary extracted source nodes"):
            V.signed_source(self.root / "build", self.p, self.m)

    def test_rehashed_native_helper_substitution(self):
        for folder in ("payload", "remote/artifacts"):
            path = self.campaign / folder / "recorder.py"
            self.write(path, path.read_bytes() + b"\n# substituted\n")
        binding = self.campaign / "binding.json"
        self.mutate(binding, lambda row: row["payload"].update({"recorder.py": V.sha(self.campaign / "payload/recorder.py")}))
        self.mutate(self.campaign / "owner.json", lambda row: row.update(binding_sha256=V.sha(binding)))
        self.refresh_remote()
        with self.assertRaisesRegex(ValueError, "frozen executable payload pins"):
            self.native()

    def test_rehashed_reordered_case_roster(self):
        path = self.campaign / "binding.json"
        self.mutate(path, lambda row: row["order"].reverse())
        self.mutate(self.campaign / "owner.json", lambda row: row.update(binding_sha256=V.sha(path)))
        with self.assertRaisesRegex(ValueError, "fixed complete native matrix"):
            self.native()

    def test_rehashed_native_status_bool(self):
        path = self.campaign / "remote/01-generated-cold-abort-test/record.json"
        self.mutate(path, lambda row: row.update(status=False))
        self.refresh_remote()
        with self.assertRaisesRegex(ValueError, "closed successful native command"):
            self.native()

    def test_rehashed_test_output(self):
        folder = self.campaign / "remote/01-generated-cold-abort-test"
        path = folder / "stdout.log"
        self.write(path, path.read_bytes().replace(b"1 passed", b"0 passed"))
        self.mutate(folder / "record.json", lambda row: row.update(stdout_sha256=V.sha(path)))
        self.refresh_remote()
        with self.assertRaises(ValueError):
            self.native()

    def test_rehashed_active_custody_pid(self):
        self.mutate(self.campaign / "remote/active-003/receipt.json", lambda row: row.update(pid=row["pid"] + 1))
        self.refresh_remote()
        with self.assertRaisesRegex(ValueError, "active-to-terminal command bijection"):
            self.native()

    def test_rehashed_active_custody_timestamp(self):
        self.mutate(self.campaign / "remote/active-003/receipt.json", lambda row: row["started"].update(monotonic_ns=0))
        self.refresh_remote()
        with self.assertRaisesRegex(ValueError, "immediate active custody"):
            self.native()

    def test_rehashed_resource_headroom(self):
        self.mutate(self.campaign / "remote/01-generated-cold-abort-resources.json", lambda row: row.update(disk_free=True))
        self.refresh_remote()
        with self.assertRaisesRegex(ValueError, "disk headroom"):
            self.native()

    def test_rehashed_false_suffix_completion(self):
        self.mutate(self.campaign / "remote/finished.json", lambda row: row.update(complete_matrix=1))
        self.refresh_remote()
        with self.assertRaisesRegex(ValueError, "exact native matrix disposition"):
            self.native()

    def test_refusal_cannot_be_promoted_by_delayed_success(self):
        self.campaign, self.rejected = self.root / "campaign2", True
        self.load()
        self.mutate(self.campaign / "remote/finished.json", lambda row: row.update(complete_matrix=True, failures=[]))
        self.refresh_remote()
        with self.assertRaisesRegex(ValueError, "exact native matrix disposition"):
            self.native()

    def test_refused_observer_exit_must_remain_one(self):
        self.campaign, self.rejected = self.root / "campaign2", True
        self.load()
        self.mutate(self.campaign / "remote/06-primary-panic-immediate/record.json", lambda row: row.update(status=0))
        self.refresh_remote()
        with self.assertRaisesRegex(ValueError, "closed successful native command"):
            self.native()

    def test_rehashed_busy_sample_cannot_be_changed_to_idle(self):
        self.campaign, self.rejected = self.root / "campaign2", True
        self.load()
        path = self.campaign / "remote/06-primary-panic-immediate/stdout.log"
        lines = [json.loads(line) for line in path.read_bytes().splitlines()]
        lines[0]["sysfs"][0]["values"]["gpu_busy_percent"] = "0"
        self.write(path, b"".join((json.dumps(row) + "\n").encode() for row in lines))
        self.mutate(path.parent / "record.json", lambda row: row.update(stdout_sha256=V.sha(path)))
        self.refresh_remote()
        with self.assertRaisesRegex(ValueError, "independently replayed original refusal"):
            self.native()

    def test_rehashed_local_absence_boolean(self):
        path = self.campaign / "commands/absence/stdout"
        self.json(path, {"path_absent": 1, "processes_absent": True})
        self.mutate(path.parent / "receipt.json", lambda row: row.update(stdout_sha256=V.sha(path)))
        with self.assertRaisesRegex(ValueError, "independent absence"):
            self.local()

    def test_cleanup_cannot_precede_collection(self):
        collect = V.read(self.campaign / "commands/collect/receipt.json")
        self.mutate(self.campaign / "commands/cleanup/receipt.json", lambda row: row.update(started_ns=collect["started_ns"]))
        with self.assertRaisesRegex(ValueError, "collection precedes cleanup"):
            self.local()

    def test_local_unreaped_group_rejected(self):
        self.mutate(self.campaign / "commands/native/receipt.json", lambda row: row.update(group_absent=False))
        with self.assertRaisesRegex(ValueError, "successful closed local command"):
            self.local()

    def test_early_attempt_cannot_acquire_owner(self):
        self.json(self.root / "campaign/owner.json", {"path": "/tmp/not-owned"})
        with self.assertRaisesRegex(ValueError, "prelaunch rejection preceded"):
            V.verify_prelaunch(self.root)

    def test_manifest_detects_extra_records(self):
        self.write(self.root / "extra", b"not in the retained campaign\n")
        with self.assertRaisesRegex(ValueError, "complete retained artifact inventory"):
            V.verify_retention(self.root)

    def test_rehashed_local_cleanup_claim(self):
        self.mutate(self.root / "retention.json", lambda row: row.update(absent=1))
        self.json(self.root.parent / "artifacts.json", V.inventory(self.root))
        with self.assertRaisesRegex(ValueError, "owned local cleanup receipt"):
            V.verify_retention(self.root)

    def test_duplicate_and_nonfinite_json(self):
        path = self.root / "invalid-json"
        for raw in (b'{"a":1,"a":2}', b'{"a":NaN}'):
            self.write(path, raw)
            with self.assertRaises(ValueError):
                V.read(path)


if __name__ == "__main__":
    unittest.main(verbosity=2)
