#!/usr/bin/env python3
"""Rehashed hostile-record controls for this CPU packet's replay checker."""

from contextlib import redirect_stdout
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import shutil
import tempfile
import unittest


HERE = Path(__file__).resolve().parent


class ReplayTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="fe2o3-hot-replay-")
        self.addCleanup(self.temporary.cleanup)
        spec = importlib.util.spec_from_file_location("hot_packet_replay", HERE / "verify.py")
        self.check = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.check)
        self.root = Path(self.temporary.name) / "packet"
        self.root.mkdir()
        shutil.copytree(HERE / "raw", self.root / "raw")
        for name in ("runner.py", "artifacts.json"):
            shutil.copyfile(HERE / name, self.root / name)
        prior = Path(self.temporary.name) / "prior"
        shutil.copytree(self.check.PRIOR, prior)
        self.check.ROOT = self.root
        self.check.PRIOR = prior

    def rewrite(self, path, change):
        value = json.loads(path.read_text())
        change(value)
        path.write_text(json.dumps(value, indent=2) + "\n")

    def refresh(self):
        values = {str(path.relative_to(self.root)): hashlib.sha256(path.read_bytes()).hexdigest()
                  for path in (self.root / "raw").rglob("*") if path.is_file()}
        (self.root / "artifacts.json").write_text(json.dumps(values))

    def replay(self):
        with redirect_stdout(io.StringIO()):
            self.check.main()

    def reject(self):
        self.refresh()
        with self.assertRaises(ValueError):
            self.replay()

    def test_valid_packet(self):
        self.replay()

    def test_nonzero_status(self):
        self.rewrite(self.root / "raw/cpu2/build-hip/record.json", lambda row: row.update(status=1))
        self.reject()

    def test_unreaped_group(self):
        self.rewrite(self.root / "raw/cpu2/build-hip/record.json", lambda row: row.update(group_absent=False))
        self.reject()

    def test_wrong_command(self):
        self.rewrite(self.root / "raw/cpu2/build-hip/record.json", lambda row: row.update(command=["true"]))
        self.reject()

    def test_wrong_prior_command(self):
        self.rewrite(self.check.PRIOR / "gnu-runtime/record.json", lambda row: row.update(command=["true"]))
        self.reject()

    def test_source_bracket_change(self):
        self.rewrite(self.root / "raw/cpu2/inputs-after.json", lambda row: row.update(runner="0" * 64))
        self.reject()

    def test_rehashed_source_substitution(self):
        for name in ("inputs-before.json", "inputs-after.json"):
            self.rewrite(self.root / "raw/cpu2" / name,
                         lambda row: row["source"].update({"Cargo.toml": "0" * 64}))
        self.reject()

    def test_balanced_source_roster_omissions(self):
        for name in ("inputs-before.json", "inputs-after.json"):
            path = self.root / "raw/cpu2" / name
            value = json.loads(path.read_text())
            for omitted in ("xgmi_peer_hot_results.py", "xgmi_peer_segments_test.cpp"):
                del value["source"]["benchmarks/runtime_gfx942/" + omitted]
            path.write_text(json.dumps(value))
        old = json.loads((self.check.PRIOR / "inputs-before.json").read_text())["source"]
        new = value["source"]
        changed = sorted(path for path in set(old) | set(new) if old.get(path) != new.get(path))
        self.assertEqual(len(changed), 10)
        self.rewrite(self.root / "raw/cpu2/prior-qualification-delta.json",
                     lambda row: row.update(changed=changed, allowed=changed))
        self.reject()

    def test_prior_bracket_change(self):
        self.rewrite(self.check.PRIOR / "inputs-after.json", lambda row: row.update(runner="0" * 64))
        self.reject()

    def test_prior_failed_command(self):
        self.rewrite(self.check.PRIOR / "gnu-runtime/record.json", lambda row: row.update(status=1))
        self.reject()

    def test_missing_command(self):
        shutil.rmtree(self.root / "raw/cpu2/clippy")
        self.reject()

    def test_extra_command(self):
        shutil.copytree(self.root / "raw/cpu2/clippy", self.root / "raw/cpu2/extra")
        self.reject()

    def test_wrong_test_count(self):
        path = self.root / "raw/cpu2/gnu-default/stdout.log"
        text = path.read_text()
        self.assertIn("13 passed", text)
        path.write_text(text.replace("13 passed", "12 passed"))
        self.reject()

    def test_wrong_runner(self):
        path = self.root / "runner.py"
        path.write_text(path.read_text() + "\n# changed\n")
        self.reject()

    def test_wrong_binary(self):
        path = self.root / "raw/cpu2/peer-hip"
        path.write_bytes(path.read_bytes() + b"changed")
        self.reject()

    def test_empty_binary_roster(self):
        (self.root / "raw/cpu2/binaries.json").write_text("{}\n")
        for name in ("peer-hip", "peer-hsa"):
            (self.root / "raw/cpu2" / name).unlink()
        self.reject()

    def test_empty_cleanup_roster(self):
        (self.root / "raw/cleanup.json").write_text("[]\n")
        self.reject()

    def test_failed_cleanup(self):
        self.rewrite(self.root / "raw/cleanup.json", lambda rows: rows[0].update(absent=False))
        self.reject()

    def test_wrong_cleanup_path(self):
        self.rewrite(self.root / "raw/cleanup.json", lambda rows: rows[0].update(path="/tmp/foreign"))
        self.reject()

    def test_unmanifested_artifact_change(self):
        path = self.root / "raw/cpu2/gnu-default/stdout.log"
        path.write_text(path.read_text() + "changed\n")
        with self.assertRaises(ValueError):
            self.replay()


if __name__ == "__main__":
    unittest.main()
