#!/usr/bin/env python3
"""Rehashed hostile controls for the completed native campaign replay."""

from contextlib import redirect_stdout
from datetime import datetime, timedelta
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
        self.temporary = tempfile.TemporaryDirectory(prefix="fe2o3-hot-native-replay-")
        self.addCleanup(self.temporary.cleanup)
        spec = importlib.util.spec_from_file_location("hot_native_replay", HERE / "verify.py")
        self.check = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.check)
        self.root = Path(self.temporary.name)
        shutil.copytree(HERE / "raw", self.root / "raw")
        shutil.copytree(HERE / "protocol3", self.root / "protocol3")
        for name in (*self.check.C.PROTOCOL, "artifacts.json"):
            shutil.copy2(HERE / name, self.root / name)
        self.check.ROOT = self.root
        self.run_root = self.root / "raw/native1"
        self.remote = self.run_root / "recovery1/remote"

    def rewrite(self, path, change):
        value = json.loads(path.read_text())
        change(value)
        path.write_text(json.dumps(value, indent=2) + "\n")

    def refresh(self):
        values = self.check.B.inventory(self.root / "raw")
        (self.root / "artifacts.json").write_text(json.dumps(values))

    def refresh_remote(self):
        values = self.check.B.inventory(self.remote)
        (self.run_root / "recovery1/remote-inventory.json").write_text(json.dumps(values))
        path = self.run_root / "recovery1/commands/inventory/stdout"
        path.write_text(json.dumps(values))
        self.rewrite(path.parent / "receipt.json", lambda row: row.update(stdout_sha256=self.check.sha(path)))

    def replay(self):
        with redirect_stdout(io.StringIO()):
            self.check.main(recovered=True)

    def reject(self, *, remote=False):
        if remote:
            self.refresh_remote()
        self.refresh()
        with self.assertRaises((ValueError, RuntimeError)):
            self.replay()

    def test_valid_packet(self):
        self.replay()

    def test_original_strict_campaign_is_still_rejected(self):
        with self.assertRaisesRegex(ValueError, "strict campaign did not qualify"):
            self.check.main()

    def test_unreaped_remote_group(self):
        self.rewrite(self.remote / "01-kfd-depth1/receipt.json", lambda row: row.update(group_absent=False))
        self.reject(remote=True)

    def test_wrong_remote_command(self):
        self.rewrite(self.remote / "01-kfd-depth1/receipt.json", lambda row: row.update(command=["true"]))
        self.reject(remote=True)

    def test_wrong_remote_deadline(self):
        self.rewrite(self.remote / "01-kfd-depth1/receipt.json", lambda row: row.update(timeout_seconds=999))
        self.reject(remote=True)

    def test_wrong_gpu_mask(self):
        self.rewrite(self.remote / "02-hsa-depth1/receipt.json", lambda row: row["environment"].update(ROCR_VISIBLE_DEVICES="0,1"))
        self.reject(remote=True)

    def test_missing_endpoint(self):
        shutil.rmtree(self.remote / "01-kfd-depth1-before-gpu6")
        self.refresh_remote()
        self.refresh()
        with self.assertRaises((ValueError, RuntimeError, FileNotFoundError)):
            self.replay()

    def test_extra_remote_command(self):
        shutil.copytree(self.remote / "01-kfd-depth1", self.remote / "extra")
        self.reject(remote=True)

    def test_postflight_delay_removed(self):
        finished = json.loads((self.remote / "01-kfd-depth1/receipt.json").read_text())["finished_ns"]
        self.rewrite(self.remote / "01-kfd-depth1-settled-gpu5/receipt.json", lambda row: row.update(started_ns=finished + 1))
        self.reject(remote=True)

    def test_wrong_observer_identity(self):
        path = self.remote / "01-kfd-depth1-before-gpu5/stdout"
        lines = path.read_text().splitlines()
        value = json.loads(lines[0])
        value["unique_id"] = "0x0000000000000001"
        path.write_text(json.dumps(value) + "\n" + lines[1] + "\n")
        self.rewrite(path.parent / "receipt.json", lambda row: row.update(stdout_sha256=self.check.sha(path)))
        self.reject(remote=True)

    def test_stale_internally_ordered_observation(self):
        path = self.remote / "01-kfd-depth1-before-gpu5/stdout"
        lines = path.read_text().splitlines()
        value = json.loads(lines[0])
        def stale(node):
            if isinstance(node, dict):
                for key, item in node.items():
                    if key == "utc":
                        prefix = datetime.strptime(item[:19], "%Y-%m-%dT%H:%M:%S") - timedelta(days=1)
                        node[key] = prefix.strftime("%Y-%m-%dT%H:%M:%S") + item[19:]
                    else:
                        stale(item)
            elif isinstance(node, list):
                for item in node:
                    stale(item)
        stale(value)
        path.write_text(json.dumps(value) + "\n" + lines[1] + "\n")
        self.rewrite(path.parent / "receipt.json", lambda row: row.update(stdout_sha256=self.check.sha(path)))
        self.reject(remote=True)

    def test_float_trial_controls_with_rehashed_binding(self):
        path = self.run_root / "binding.json"
        self.rewrite(path, lambda row: row["controls"].update(bytes=1048576.0, warmups=10.0, samples=30.0))
        self.rewrite(self.run_root / "owner.json", lambda row: row.update(binding_sha256=self.check.sha(path)))
        self.refresh()
        with self.assertRaisesRegex(ValueError, "fixed trial plan"):
            self.replay()

    def test_boolean_phase_exit(self):
        path = self.run_root / "local/native/stdout"
        lines = path.read_text().splitlines()
        row = json.loads(lines[0])
        row["exit"] = False
        lines[0] = json.dumps(row)
        path.write_text("\n".join(lines) + "\n")
        self.rewrite(path.parent / "receipt.json", lambda row: row.update(stdout_sha256=self.check.sha(path)))
        self.reject()

    def test_float_result_depth(self):
        self.rewrite(self.remote / "validated-results.json", lambda rows: rows[0].update(depth=1.0))
        self.reject(remote=True)

    def test_numeric_native_success(self):
        self.rewrite(self.remote / "finished.json", lambda row: row.update(native_execution=1))
        self.reject(remote=True)

    def test_numeric_collection_success(self):
        self.rewrite(self.run_root / "recovery1/result.json", lambda row: row.update(owned_cleanup=1))
        self.reject()

    def test_invented_parsed_results(self):
        self.rewrite(self.remote / "validated-results.json", lambda rows: rows[0].update(depth=32))
        self.reject(remote=True)

    def test_wrong_timed_byte_numerator(self):
        path = self.remote / "01-kfd-depth1/stdout"
        value = path.read_text()
        self.assertIn("bytes=1048576", value)
        path.write_text(value.replace("bytes=1048576", "bytes=2097152"))
        self.rewrite(path.parent / "receipt.json", lambda row: row.update(stdout_sha256=self.check.sha(path)))
        self.reject(remote=True)

    def test_corrupt_retained_elf(self):
        path = self.remote / "artifacts/peer-hip"
        path.write_bytes(path.read_bytes() + b"changed")
        self.reject(remote=True)

    def test_empty_elf_roster(self):
        for name in ("binaries.json", "binaries-after.json"):
            (self.remote / name).write_text("{}\n")
        self.reject(remote=True)

    def test_wrong_local_build_command(self):
        self.rewrite(self.run_root / "local/build-kfd/receipt.json", lambda row: row.update(command=["true"]))
        self.reject()

    def test_missing_cold_target_environment(self):
        self.rewrite(self.run_root / "local/build-kfd/receipt.json", lambda row: row["environment"].pop("CARGO_TARGET_DIR"))
        self.reject()

    def test_source_map_substitution(self):
        self.rewrite(self.run_root / "source.json", lambda row: row.update({"Cargo.toml": "0" * 64}))
        self.reject()

    def test_protocol_substitution(self):
        path = self.run_root / "protocol/native.py"
        path.write_text(path.read_text() + "\n# substituted\n")
        self.reject()

    def test_failed_protocol_qualification(self):
        self.rewrite(self.root / "protocol3/commands/test_campaign/receipt.json", lambda row: row.update(exit=1))
        self.reject()

    def test_unknown_publication_not_complete(self):
        self.rewrite(self.remote / "finished.json", lambda row: row.update(native_execution=False))
        self.reject(remote=True)

    def test_cleanup_path_substitution(self):
        self.rewrite(self.root / "raw/local-cleanup.json", lambda rows: rows[0].update(path="/tmp/foreign"))
        self.reject()

    def test_empty_cleanup_roster(self):
        (self.root / "raw/local-cleanup.json").write_text("[]\n")
        self.reject()

    def test_failed_cleanup(self):
        self.rewrite(self.root / "raw/local-cleanup.json", lambda rows: rows[1].update(absent=False))
        self.reject()

    def test_false_remote_absence(self):
        path = self.run_root / "recovery1/commands/absence/stdout"
        path.write_text('{"path_absent": false, "processes_absent": true}\n')
        self.rewrite(path.parent / "receipt.json", lambda row: row.update(stdout_sha256=self.check.sha(path)))
        self.reject()

    def test_failed_collection(self):
        self.rewrite(self.run_root / "collection.json", lambda row: row.update(failures=["collection failed"]))
        self.reject()

    def test_unsigned_artifact_substitution(self):
        path = self.run_root / "local/rustc/stdout"
        path.write_text(path.read_text() + "changed\n")
        with self.assertRaises(ValueError):
            self.replay()


if __name__ == "__main__":
    unittest.main()
