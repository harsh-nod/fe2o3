#!/usr/bin/env python3
"""CPU-only mutation tests for the peer-batch native controller."""

import sys

if not sys.flags.isolated:
    raise RuntimeError("run with python3 -I")

import copy
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest import mock

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent


def load(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


C = load(HERE / "campaign.py", "tested_peer_batch_campaign")
P = C.P


def transcript(phase):
    rows = []
    depth = phase[1]
    latency = 1_048_576 * depth
    for measurement in ("remap-per-round", "persistent-hot"):
        row = P.fixed_fields(P.devices(), phase, measurement)
        for direction in ("forward", "reverse"):
            row[direction + "_p50_ns"] = str(latency)
            row[direction + "_p95_ns"] = str(latency + 1)
            row[direction + "_p50_GBps"] = "1.000"
        rows.append(" ".join(key + "=" + value for key, value in row.items()))
    return ("\n".join(rows) + "\n").encode()


def endpoint_transcript(device, *, mem_busy="0"):
    metrics = {
        "gpu_busy_percent": "0",
        "mem_busy_percent": mem_busy,
        "mem_info_gtt_used": "1",
        "mem_info_vis_vram_used": "2",
        "mem_info_vram_used": "3",
        "unique_id": device[2][2:],
    }
    snapshot = {
        "started": {},
        "finished": {},
        "path": "/sys/bus/pci/devices/" + device[1],
        "values": metrics,
        "errors": {},
    }
    observation = {
        "schema": "fe2o3.copy-host-observation.v1",
        "record": "observation",
        "index": 0,
        "gpu_index": device[0],
        "pci_bdf": device[1],
        "unique_id": device[2],
        "started": {},
        "finished": {},
        "status": {},
        "pids": {},
        "sysfs": [copy.deepcopy(snapshot) for _ in range(3)],
        "endpoint_admitted": True,
        "reasons": [],
        "selected_pids": [],
        "scope": "sequential-endpoint-observations-not-continuous-monitoring-or-reservation",
        "visibility_filters": "removed-for-cli",
        "vram_limit_exclusive": 512 * 1024 * 1024,
    }
    complete = {
        "schema": "fe2o3.copy-host-observation.v1",
        "record": "complete",
        "observations": 1,
        "refused": 0,
        "all_endpoints_admitted": True,
        "performance_accepted": False,
    }
    return (json.dumps(observation) + "\n" + json.dumps(complete) + "\n").encode()


class ProtocolTests(unittest.TestCase):
    def test_unconfigured_source_contract_fails_closed(self):
        original = (P.SOURCE_COMMIT, P.CPU_SEAL_SHA256, P.TOOLING_BRANCH)
        try:
            P.SOURCE_COMMIT = P.CPU_SEAL_SHA256 = P.TOOLING_BRANCH = ""
            with self.assertRaisesRegex(RuntimeError, "configure source commit"):
                P.configured()
        finally:
            P.SOURCE_COMMIT, P.CPU_SEAL_SHA256, P.TOOLING_BRANCH = original
        if all(original):
            P.configured()

    def test_phase_roster_is_two_abba_blocks_plus_depth63(self):
        self.assertEqual(len(P.PHASES), 9)
        for offset, depth in ((0, 1), (4, 2)):
            block = P.PHASES[offset : offset + 4]
            self.assertEqual([phase[1] for phase in block], [depth] * 4)
            self.assertEqual(
                [phase[2] for phase in block],
                ["ordinary", "aggregate", "aggregate", "ordinary"],
            )
            self.assertTrue(all(phase[3:5] == [10, 30] for phase in block))
        self.assertEqual(
            P.PHASES[-1], ["d63-aggregate-correctness", 63, "aggregate", 0, 1, 900]
        )

    def test_remote_roster_has_one_build_nine_workloads_and_54_guards(self):
        specs = P.remote_specs(Path(P.PREFIX + "0" * 16), P.devices())
        names = [spec[0] for spec in specs]
        self.assertEqual(len(specs), 68)
        self.assertEqual(sum("-gpu" in name for name in names), 54)
        self.assertEqual(
            sum(name in {phase[0] for phase in P.PHASES} for name in names), 9
        )
        self.assertEqual(names.count("build"), 1)

    def test_build_is_frozen_default_feature_and_two_jobs(self):
        owned = Path(P.PREFIX + "0" * 16)
        build = {spec[0]: spec for spec in P.build_specs(owned)}["build"]
        self.assertEqual(
            build[1],
            [
                "cargo",
                "build",
                "--frozen",
                "--release",
                "-p",
                "fe2o3-runtime",
                "--example",
                "gfx942-runtime-xgmi-peer-benchmark",
            ],
        )
        self.assertEqual(build[4]["CARGO_BUILD_JOBS"], "2")
        self.assertNotIn("--features", build[1])

    def test_phase_commands_differ_only_by_aggregate_flag(self):
        owned, selected = Path(P.PREFIX + "0" * 16), P.devices()
        ordinary = P.phase_specs(owned, selected, P.PHASES[0])[2][1]
        aggregate = P.phase_specs(owned, selected, P.PHASES[1])[2][1]
        self.assertEqual(ordinary[-4:], ["1048576", "1", "10", "30"])
        self.assertEqual(aggregate[:-1], ordinary)
        self.assertEqual(aggregate[-1], "--aggregate-peer-batch")

    def test_ordinary_transcript_is_exact(self):
        phase = P.PHASES[0]
        rows = P.parse_transcript(transcript(phase), P.devices(), phase)
        self.assertEqual(len(rows), 2)
        self.assertTrue(
            all(row["schema"] == "fe2o3.xgmi-peer-benchmark.v1" for row in rows)
        )

    def test_aggregate_transcript_is_exact(self):
        phase = P.PHASES[1]
        rows = P.parse_transcript(transcript(phase), P.devices(), phase)
        self.assertTrue(
            all(
                row["schema"] == "fe2o3.xgmi-peer-aggregate-benchmark.v1"
                and row["aggregate_roster"] == "exact-round-submissions"
                for row in rows
            )
        )

    def test_mode_schema_substitution_is_rejected(self):
        phase = P.PHASES[1]
        data = transcript(phase).replace(
            b"fe2o3.xgmi-peer-aggregate-benchmark.v1",
            b"fe2o3.xgmi-peer-benchmark.v1",
        )
        with self.assertRaises(RuntimeError):
            P.parse_transcript(data, P.devices(), phase)

    def test_bandwidth_mismatch_is_rejected(self):
        phase = P.PHASES[0]
        data = transcript(phase).replace(
            b"forward_p50_GBps=1.000", b"forward_p50_GBps=2.000"
        )
        with self.assertRaises(RuntimeError):
            P.parse_transcript(data, P.devices(), phase)

    def test_incomplete_or_extra_transcript_is_rejected(self):
        phase, data = P.PHASES[0], transcript(P.PHASES[0])
        for malformed in (data.splitlines(keepends=True)[0], data + b"extra=x\n"):
            with self.assertRaises(RuntimeError):
                P.parse_transcript(malformed, P.devices(), phase)

    def test_selected_device_identity_is_bound(self):
        phase = P.PHASES[0]
        wrong = copy.deepcopy(P.devices())
        wrong.reverse()
        with self.assertRaises(RuntimeError):
            P.parse_transcript(transcript(phase), wrong, phase)

    def test_pre_native_cleanup_keeps_original_and_records_secondary(self):
        lifecycle = C.lifecycle_module()
        state = {
            "create_attempted": False,
            "created": False,
            "native_attempted": False,
            "native_success": False,
            "collected": False,
            "cleaned": False,
            "absence": False,
            "failure": None,
            "native_failure": None,
            "secondary_failures": [],
        }
        calls = []

        def step(name):
            calls.append(name)
            if name == "upload":
                raise ValueError("original")
            if name == "cleanup":
                raise KeyError("secondary")

        with self.assertRaisesRegex(ValueError, "original"):
            C.controlled_create_upload_run(step, state, lifecycle)
        self.assertEqual(calls, ["create", "upload", "cleanup", "absence"])
        self.assertTrue(state["absence"])
        self.assertIsNone(state["failure"])
        self.assertEqual(len(state["secondary_failures"]), 1)
        self.assertEqual(state["secondary_failures"][0]["stage"], "pre-native-cleanup")

    def test_flat_remote_payload_imports_only_authenticated_local_helpers(self):
        with tempfile.TemporaryDirectory(prefix=".remote-layout-", dir=HERE) as folder:
            remote = Path(folder)
            shutil.copyfile(HERE / "protocol.py", remote / "protocol.py")
            for name in P.HELPERS:
                shutil.copyfile(P.helper_source(name), remote / name)
            module = load(remote / "protocol.py", "tested_flat_peer_batch_protocol")
            helpers = module.helper_identities()
            self.assertEqual(helpers, P.helper_identities())
            module.SOURCE_COMMIT = "a" * 40
            module.CPU_SEAL_SHA256 = "b" * 64
            module.TOOLING_BRANCH = "refs/heads/test"
            tools = {
                name: chr(99 + index) * 64
                for index, name in enumerate(module.STATIC_INPUTS)
            }
            payload = {name: helpers.get(name, "e" * 64) for name in module.PAYLOAD}
            payload["protocol.py"] = tools["protocol.py"]
            payload["campaign.py"] = tools["campaign.py"]
            payload["source.tar.gz"] = "f" * 64
            binding = {
                "schema": "fe2o3.xgmi-peer-batch-native.v1",
                "tooling_commit": "1" * 40,
                "source_commit": module.SOURCE_COMMIT,
                "cpu": {
                    "relative": module.CPU_RELATIVE,
                    "seal_sha256": module.CPU_SEAL_SHA256,
                    "binding_sha256": "2" * 64,
                    "source_snapshot_sha256": "3" * 64,
                    "qualified_source_base": "4" * 40,
                    "toolchain": {
                        "rustc_stdout_sha256": "7" * 64,
                        "cargo_stdout_sha256": "8" * 64,
                    },
                },
                "tools": tools,
                "helpers": helpers,
                "devices": module.devices(),
                "plan": module.PLAN,
                "source": {
                    "tree": "5" * 40,
                    "source_files": {"Cargo.toml": "6" * 64},
                    "source_modes": {"Cargo.toml": "100644"},
                    "tar_sha256": payload["source.tar.gz"],
                    "tar_bytes": 1,
                },
                "payload": payload,
            }
            module.validate_binding(binding)

    def test_cpu_binding_requires_typed_count_and_toolchain(self):
        original = (
            P.ROOT,
            P.CPU_RELATIVE,
            P.SOURCE_COMMIT,
            P.CPU_SEAL_SHA256,
            P.TOOLING_BRANCH,
        )
        with tempfile.TemporaryDirectory(prefix=".cpu-binding-", dir=HERE) as folder:
            root, archive = Path(folder), Path(folder) / "cpu"
            archive.mkdir()
            binding = {
                "schema": "fe2o3.xgmi-peer-batch-cpu.v1",
                "execution_root": str(root),
                "source_base": "a" * 40,
                "source_snapshot_sha256": "b" * 64,
                "source_files": 1,
                "rosters": {},
                "tools": {},
                "document": {},
                "toolchain": {
                    "rustc_stdout_sha256": "c" * 64,
                    "cargo_stdout_sha256": "d" * 64,
                },
                "commands": 1,
            }

            def install(value):
                (archive / "binding.json").write_text(
                    json.dumps(value, sort_keys=True) + "\n"
                )
                (archive / "SHA256SUMS").write_text(
                    P.sha(archive / "binding.json") + "  binding.json\n"
                )
                P.CPU_SEAL_SHA256 = P.sha(archive / "SHA256SUMS")

            try:
                P.ROOT, P.CPU_RELATIVE = root, "cpu"
                P.SOURCE_COMMIT, P.TOOLING_BRANCH = "e" * 40, "refs/heads/test"
                install(binding)
                self.assertEqual(P.cpu_binding(), binding)
                for malformed in ({"Cargo.toml": "f" * 64}, True):
                    invalid = copy.deepcopy(binding)
                    invalid["source_files"] = malformed
                    install(invalid)
                    with self.assertRaisesRegex(RuntimeError, "exact CPU binding"):
                        P.cpu_binding()
                invalid = copy.deepcopy(binding)
                invalid["toolchain"]["extra"] = "0" * 64
                install(invalid)
                with self.assertRaisesRegex(RuntimeError, "toolchain"):
                    P.cpu_binding()
            finally:
                (
                    P.ROOT,
                    P.CPU_RELATIVE,
                    P.SOURCE_COMMIT,
                    P.CPU_SEAL_SHA256,
                    P.TOOLING_BRANCH,
                ) = original

    def test_native_toolchain_receipts_match_cpu_and_have_empty_stderr(self):
        with tempfile.TemporaryDirectory(prefix=".toolchain-", dir=HERE) as folder:
            root = Path(folder)
            for name, output in (("rustc", b"rustc\n"), ("cargo", b"cargo\n")):
                receipt = root / name
                receipt.mkdir()
                (receipt / "stdout").write_bytes(output)
                (receipt / "stderr").write_bytes(b"")
            expected = {
                name + "_stdout_sha256": P.sha(root / name / "stdout")
                for name in ("rustc", "cargo")
            }
            P.validate_toolchain_receipts(root, expected)
            (root / "cargo" / "stderr").write_bytes(b"warning\n")
            with self.assertRaisesRegex(RuntimeError, "stderr"):
                P.validate_toolchain_receipts(root, expected)
            (root / "cargo" / "stderr").write_bytes(b"")
            (root / "rustc" / "stdout").write_bytes(b"different\n")
            with self.assertRaisesRegex(RuntimeError, "toolchain equality"):
                P.validate_toolchain_receipts(root, expected)

    def test_cpu_replay_authenticates_seal_before_subprocess(self):
        verifier = load(HERE / "verify.py", "tested_peer_batch_verify")
        with (
            mock.patch.object(
                verifier.P,
                "cpu_binding",
                side_effect=RuntimeError("pinned CPU seal"),
            ),
            mock.patch.object(verifier.subprocess, "run") as run,
        ):
            with self.assertRaisesRegex(RuntimeError, "pinned CPU seal"):
                verifier.cpu_replay()
            run.assert_not_called()

    def test_local_absence_rejects_dangling_symlink_and_archive_requires_readme(self):
        verifier = load(HERE / "verify.py", "tested_peer_batch_verify_roster")
        self.assertIn("README.md", verifier.ARCHIVE_STATIC)
        with tempfile.TemporaryDirectory(prefix=".absence-", dir=HERE) as folder:
            candidate = Path(folder) / "payload"
            candidate.symlink_to(Path(folder) / "missing", target_is_directory=True)
            self.assertFalse(C.path_absent(candidate))
            candidate.unlink()
            self.assertTrue(C.path_absent(candidate))

    def test_live_mem_busy_observation_blocks_workload(self):
        selected, phase = P.devices(), P.PHASES[0]
        P.parse_endpoint(endpoint_transcript(selected[0]), selected[0])
        with self.assertRaisesRegex(RuntimeError, "raw sysfs idle"):
            P.parse_endpoint(
                endpoint_transcript(selected[0], mem_busy="1"), selected[0]
            )
        with tempfile.TemporaryDirectory(prefix=".endpoint-", dir=HERE) as folder:
            root, calls = Path(folder), []

            class Recorder:
                cwd = None

                def run(self, name, _command, _seconds, *, env):
                    calls.append(name)
                    receipt = root / name
                    receipt.mkdir()
                    device = selected[0] if name.endswith("gpu1") else selected[1]
                    busy = "1" if name.endswith("gpu1") else "0"
                    (receipt / "stdout").write_bytes(
                        endpoint_transcript(device, mem_busy=busy)
                    )
                    (receipt / "stderr").write_bytes(b"")
                    return receipt

            specs = {
                name: (command, seconds, cwd, env)
                for name, command, seconds, cwd, env in P.phase_specs(
                    Path(P.PREFIX + "0" * 16), selected, phase
                )
            }
            with (
                mock.patch.object(
                    C.B,
                    "settled_postflight",
                    side_effect=lambda _observe, _name, failure: failure,
                ),
                self.assertRaisesRegex(RuntimeError, "raw sysfs idle"),
            ):
                C.execute_phase(Recorder(), specs, selected, phase, {})
            self.assertNotIn(phase[0], calls)


if __name__ == "__main__":
    unittest.main()
