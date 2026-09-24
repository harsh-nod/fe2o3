#!/usr/bin/env python3
"""CPU-only runner orchestration with real bound files, parser and postflight."""

import hashlib
import importlib.util
import io
from pathlib import Path
import shutil
import tarfile
import tempfile
import unittest
from unittest import mock

HERE = Path(__file__).resolve().parent


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


N = module("directed_native_test", HERE / "xgmi_backing_budget_native.py")
R = module("directed_receipt_test", HERE / "test_xgmi_backing_budget_results.py")
B, H = N.B, N.H
DEVICES = [[5, "0000:a6:00.0", "0xb7baafd0fb173d8e"],
           [6, "0000:c6:00.0", "0x10a254ce4987e716"]]
TRIAL = "backing-budget"
ENDPOINTS = [f"{TRIAL}-{phase}-gpu{device[0]}"
             for phase in ("before", "settled", "delayed") for device in DEVICES]
ORDER = ["rocm", "kernel", *ENDPOINTS[:2], TRIAL, *ENDPOINTS[2:], "after-rocm", "after-kernel"]
OBSERVER = "benchmarks/runtime_gfx942/copy-host-observe.py"


class DirectedOwnerNative(unittest.TestCase):
    def exercise(self, *, fail_at=None, secondary=None, malformed=False,
                 change=None, controls=None):
        with tempfile.TemporaryDirectory(prefix="fe2o3-backing-budget-test-") as temp:
            owned = Path(temp)
            for name, source in {"native.py": Path(N.__file__),
                                 "results.py": HERE / "xgmi_backing_budget_results.py",
                                 "hot.py": Path(H.__file__), "base.py": Path(B.__file__)}.items():
                shutil.copyfile(source, owned / name)
            (owned / "kfd-owner").write_bytes(b"inert test ELF; never execute\n")
            observer = b"inert observer; recorder supplies test observations\n"
            with tarfile.open(owned / "source.tar.gz", "w:gz") as archive:
                member = tarfile.TarInfo(OBSERVER)
                member.size = len(observer)
                archive.addfile(member, io.BytesIO(observer))
            binding = {"commit": "1" * 40, "payload": {name: H.sha(owned / name) for name in N.PAYLOAD},
                       "source_files": {OBSERVER: hashlib.sha256(observer).hexdigest()},
                       "devices": DEVICES, "controls": N.CONTROLS if controls is None else controls,
                       "order": list(N.ORDER), "features": "default"}
            B.write_json(owned / "binding.json", binding)
            marker = {"path": str(owned), "commit": binding["commit"],
                      "binding_sha256": H.sha(owned / "binding.json")}
            B.write_json(owned / "owner.json", marker)
            calls, sleeps, observations = [], [], []
            original = RuntimeError("injected " + str(fail_at))

            class Recorder:
                def __init__(self, output, cwd):
                    self.output, self.cwd = output, cwd
                    output.mkdir()

                def run(self, name, command, seconds, *, env):
                    calls.append(name)
                    folder = self.output / name
                    folder.mkdir()
                    raw = b"identity\n"
                    if name == TRIAL:
                        raw = b"invalid\n" if malformed else R.RECEIPT
                    if name == "after-rocm" and change == "host":
                        raw = b"changed identity\n"
                    (folder / "stdout").write_bytes(raw)
                    (folder / "stderr").write_bytes(b"")
                    if name == "after-kernel":
                        if change == "source":
                            (owned / "source" / OBSERVER).write_bytes(b"changed source\n")
                        if change == "elf":
                            (owned / "kfd-owner").write_bytes(b"changed ELF\n")
                    if name == fail_at:
                        raise original
                    if name == secondary:
                        raise RuntimeError("secondary failure")
                    return folder

            def endpoint(raw, index, bdf, uid):
                self.assertEqual(raw, b"identity\n")
                self.assertIn([index, bdf, uid], DEVICES)
                observations.append(index)

            real_postflight = B.settled_postflight
            with (
                mock.patch.object(N, "HERE", owned),
                mock.patch.object(B, "owned_path", return_value=owned),
                mock.patch.object(B, "Recorder", Recorder),
                mock.patch.object(N.resource, "setrlimit") as limit,
                mock.patch.object(B.subprocess, "Popen", side_effect=AssertionError("CPU test spawned process")) as spawn,
                mock.patch.object(H, "parse_endpoint", side_effect=endpoint),
                mock.patch.object(B, "settled_postflight", side_effect=lambda observe, name, error:
                                  real_postflight(observe, name, error, sleep=sleeps.append)),
            ):
                if fail_at or malformed or change or controls is not None:
                    with self.assertRaises(RuntimeError):
                        N.run_native(marker)
                else:
                    N.run_native(marker)
                spawn.assert_not_called()
                limit.assert_called_once()
            finished = H.parse_json((owned / "results/finished.json").read_bytes())
            results = H.parse_json((owned / "results/validated-results.json").read_bytes())
            return calls, sleeps, observations, finished, results

    def test_success_requires_six_observations_one_workload_and_fixed_delays(self):
        calls, sleeps, observations, finished, results = self.exercise()
        self.assertEqual(calls, ORDER)
        self.assertEqual(sleeps, [2, 20])
        self.assertEqual(observations, [5, 6, 5, 6, 5, 6])
        self.assertEqual(finished, dict(commit="1" * 40, failures=[], native_execution=True,
            exclusive_reservation=False, performance_acceptance=False, formal_refinement=False))
        self.assertEqual(results, [{"trial": TRIAL, "result":
            R.R.parse_receipt(R.RECEIPT, list(R.IDS))}])

    def test_every_trial_failure_keeps_both_postflight_passes_and_no_retry(self):
        for failed in [*ENDPOINTS, TRIAL]:
            with self.subTest(failed=failed):
                calls, sleeps, _, finished, results = self.exercise(fail_at=failed)
                expected = [name for name in ORDER if not (failed in ENDPOINTS[:2] and name == TRIAL)]
                self.assertEqual(calls, expected)
                self.assertEqual(sleeps, [2, 20])
                self.assertFalse(finished["native_execution"])
                self.assertEqual(finished["failures"], [repr(RuntimeError("injected " + failed))])
                self.assertEqual(len(results), int(failed in ENDPOINTS[2:]))

    def test_parser_failure_remains_primary_over_postflight_failure(self):
        calls, sleeps, _, finished, results = self.exercise(malformed=True, secondary=ENDPOINTS[2])
        self.assertEqual(calls, ORDER)
        self.assertEqual(sleeps, [2, 20])
        self.assertEqual(results, [])
        self.assertFalse(finished["native_execution"])
        self.assertEqual(len(finished["failures"]), 1)
        self.assertIn("receipt does not match", finished["failures"][0])
        self.assertNotIn("secondary", finished["failures"][0])

    def test_post_run_source_elf_and_host_changes_refuse_acceptance(self):
        for changed in ("source", "elf", "host"):
            with self.subTest(changed=changed):
                calls, sleeps, _, finished, results = self.exercise(change=changed)
                self.assertEqual(calls, ORDER)
                self.assertEqual(sleeps, [2, 20])
                self.assertFalse(finished["native_execution"])
                self.assertTrue(finished["failures"])
                self.assertEqual(len(results), 1)

    def test_rehashed_boolean_control_substitution_rejects_before_workload(self):
        calls, sleeps, observations, finished, results = self.exercise(
            controls={**N.CONTROLS, "pressure_bytes": True})
        self.assertNotIn(TRIAL, calls)
        self.assertFalse(set(calls).intersection(ENDPOINTS))
        self.assertEqual(sleeps, [])
        self.assertEqual(observations, [])
        self.assertEqual(results, [])
        self.assertFalse(finished["native_execution"])
        self.assertIn("fixed backing-budget correctness controls", finished["failures"][0])

    def test_rehashed_budget_changes_reject_before_native_workload(self):
        changes = [
            {**N.CONTROLS, "device_limits": N.CONTROLS["device_limits"][::-1]},
            {**N.CONTROLS, "coherent_limits": [[4096, True], [8192, 2]]},
            {**N.CONTROLS, "capacity_rejections": 1},
            {**N.CONTROLS, "retries": 1},
            {**N.CONTROLS, "checked_bytes": 36874},
        ]
        for changed in changes:
            with self.subTest(controls=changed):
                calls, sleeps, observations, finished, results = self.exercise(controls=changed)
                self.assertNotIn(TRIAL, calls)
                self.assertFalse(set(calls).intersection(ENDPOINTS))
                self.assertEqual((sleeps, observations, results), ([], [], []))
                self.assertFalse(finished["native_execution"])
                self.assertIn("fixed backing-budget correctness controls", finished["failures"][0])


if __name__ == "__main__":
    unittest.main()
