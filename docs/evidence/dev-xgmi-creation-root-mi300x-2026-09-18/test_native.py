#!/usr/bin/env python3
"""CPU-only calibration of archive, receipt and process ownership boundaries."""

import json
import copy
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest import mock

sys.dont_write_bytecode = True
import native as N  # noqa: E402
import controller as C  # noqa: E402
import verify as V  # noqa: E402


class NativeTests(unittest.TestCase):
    def test_endpoint_replays_raw_evidence_instead_of_trusting_admission(self):
        path = V.PRIOR / "results/logical-mux-2-preflight/stdout.log"
        baseline = json.loads(path.read_text().splitlines()[0])
        V.endpoint(baseline, *N.DEVICES[0])
        mutations = []
        for key, value in (
            ("gpu_busy_percent", "1"),
            ("mem_info_vram_used", str(512 * 1024 * 1024)),
            ("unique_id", "0000000000000001"),
        ):
            changed = copy.deepcopy(baseline)
            changed["sysfs"][0]["values"][key] = value
            mutations.append(changed)
        changed = copy.deepcopy(baseline)
        changed["status"]["exit"] = 1
        mutations.append(changed)
        changed = copy.deepcopy(baseline)
        changed["pids"]["stdout"] = (
            "==== ROCm System Management Interface ====\n==== GPUs Indexed by PID ====\n"
            "PID 42 is using 1 DRM device(s):\n1\n====\n==== End of ROCm SMI Log ====\n"
        )
        mutations.append(changed)
        changed = copy.deepcopy(baseline)
        changed["sysfs"][0]["unexpected"] = "extra"
        mutations.append(changed)
        changed = copy.deepcopy(baseline)
        changed["status"]["stdout"] = "{}"
        mutations.append(changed)
        for changed in mutations:
            self.assertTrue(changed["endpoint_admitted"])
            with self.assertRaises((RuntimeError, ValueError, KeyError)):
                V.endpoint(changed, *N.DEVICES[0])

    def test_receipt_verifier_rejects_status_time_digest_and_command_mutations(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            rec = N.Recorder(root / "results", root)
            command = [sys.executable, "-c", "print('ok')"]
            folder = rec.run("good", command, 5)
            good = V.receipt(folder, command)
            for change in (
                {"exit": 1},
                {"exit": False},
                {"error": "timeout"},
                {"group_absent": False},
                {"started_ns": good["finished_ns"] + 1},
                {"stdout_sha256": "0" * 64},
                {"command": ["wrong"]},
                {"stdin_sha256": "0" * 64},
            ):
                with mock.patch.object(V, "read", return_value={**good, **change}):
                    with self.assertRaises(RuntimeError):
                        V.receipt(folder, command)

    def test_seal_detects_mutation_and_refuses_overwrite(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            N.write_json(root / "data.json", {"test": True})
            with mock.patch.object(V, "HERE", root):
                V.seal(True)
                V.seal(False)
                with self.assertRaises(FileExistsError):
                    V.seal(True)
                N.write_json(root / "extra.json", {"test": False})
                with self.assertRaises(RuntimeError):
                    V.seal(False)

    def test_controller_cannot_cleanup_before_verified_collection(self):
        for failure in ("empty", "mismatch", "collect", "cleanup", None):
            with (
                self.subTest(failure=failure),
                tempfile.TemporaryDirectory() as directory,
            ):
                root = Path(directory)
                result = root / "inventory"
                result.mkdir()
                N.write_json(
                    result / "stdout", {} if failure == "empty" else {"data": "digest"}
                )
                events, state = [], {}

                def remote(name, mode):
                    events.append(name)
                    if name == "cleanup" and failure == "cleanup":
                        raise RuntimeError("cleanup failed")
                    return result

                def run(name, command, seconds):
                    events.append(name)
                    if failure == "collect":
                        raise RuntimeError("collection failed")

                rec = mock.Mock()
                rec.run.side_effect = run
                with (
                    mock.patch.object(C, "HERE", root),
                    mock.patch.object(
                        N,
                        "inventory",
                        return_value={
                            "data": "wrong" if failure == "mismatch" else "digest"
                        },
                    ),
                ):
                    if failure:
                        with self.assertRaises(RuntimeError):
                            C.collect_and_clean(rec, remote, {"path": "/owned"}, state)
                    else:
                        C.collect_and_clean(rec, remote, {"path": "/owned"}, state)
                expected = {
                    "empty": ["remote-inventory"],
                    "mismatch": ["remote-inventory", "collect"],
                    "collect": ["remote-inventory", "collect"],
                    "cleanup": ["remote-inventory", "collect", "cleanup"],
                    None: ["remote-inventory", "collect", "cleanup", "absence"],
                }
                self.assertEqual(events, expected[failure])
                self.assertEqual(state.get("absence", False), failure is None)

    def test_results_bind_exact_roster_controls_and_numeric_values(self):
        def transcript(backend, depth):
            measurements = (
                ["remap-per-round", "persistent-hot"] if backend == "kfd" else [None]
            )
            rows = []
            for measurement in measurements:
                row = N.expected_fields(backend, depth, measurement)
                if backend != "kfd":
                    row["targets"] = "gfx942:xnack-,gfx942:xnack-"
                for direction in ("forward", "reverse"):
                    row.update(
                        {
                            direction + "_p50_ns": "1048576",
                            direction + "_p95_ns": "2097152",
                            direction + "_p50_GBps": f"{depth:.3f}",
                        }
                    )
                rows.append(" ".join(key + "=" + value for key, value in row.items()))
            return ("\n".join(rows) + "\n").encode()

        for backend in ("kfd", "hsa", "hip"):
            for depth in (1, 16):
                good = transcript(backend, depth)
                self.assertEqual(
                    len(N.parse_results(good, backend, depth)),
                    2 if backend == "kfd" else 1,
                )
                mutations = [
                    b"",
                    good[:-1],
                    good + good,
                    good.replace(b"\n", b"\r\n"),
                    b"extra=x " + good,
                    b"backend=" + backend.encode() + b" " + good,
                    good.replace(b"samples=30", b"samples=31"),
                    good.replace(b"ab83d2ffef0d3cdf", b"ab83d2ffef0d3cde"),
                    good.replace(b"p50_ns=1048576", b"p50_ns=0"),
                    good.replace(b"p95_ns=2097152", b"p95_ns=1"),
                    good.replace(b"p50_GBps=", b"p50_GBps=nan"),
                    good.replace(b"p50_GBps=", b"p50_GBps=2"),
                ]
                if backend == "kfd":
                    mutations += [
                        good.replace(b"canaries=pass", b"canaries=fail"),
                        good.replace(b"prime_batches=1", b"prime_batches=0"),
                        b"\n".join(reversed(good.splitlines())) + b"\n",
                    ]
                for mutation in mutations:
                    with self.assertRaises(RuntimeError):
                        N.parse_results(mutation, backend, depth)

    def test_archive_rejects_duplicates_traversal_links_and_wrong_roster(self):
        def item(name, kind=tarfile.REGTYPE):
            value = tarfile.TarInfo(name)
            value.type = kind
            return value

        N.validate_members([item("a/b")], {"a/b": "digest"})
        for rows in (
            [item("../b")],
            [item("/a/b")],
            [item("a/./b")],
            [item("a/b", tarfile.SYMTYPE)],
            [item("a/b"), item("a/b")],
            [item("extra")],
        ):
            with self.assertRaises(RuntimeError):
                N.validate_members(rows, {"a/b": "digest"})

    def test_marker_rejects_other_paths_and_malformed_identities(self):
        marker = {
            "path": N.PREFIX + "a" * 16,
            "commit": "b" * 40,
            "binding_sha256": "c" * 64,
        }
        self.assertEqual(str(N.owned_path(marker, exists=False)), marker["path"])
        for key, value in (
            ("path", "/tmp/unowned"),
            ("path", N.PREFIX + "../elsewhere"),
            ("commit", "short"),
            ("binding_sha256", "D" * 64),
        ):
            with self.assertRaises(RuntimeError):
                N.owned_path({**marker, key: value}, exists=False)

    def test_inventory_rejects_symlinks(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "link").symlink_to("/dev/null")
            with self.assertRaises(RuntimeError):
                N.inventory(root)

    def test_recorder_preserves_exit_output_and_refuses_overwrite(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            rec = N.Recorder(root / "results", root)
            folder = rec.run("good", [sys.executable, "-c", "print('ok')"], 5)
            row = json.loads((folder / "receipt.json").read_text())
            self.assertEqual(row["exit"], 0)
            self.assertTrue(row["group_absent"])
            self.assertEqual((folder / "stdout").read_text(), "ok\n")
            with self.assertRaises(FileExistsError):
                rec.run("good", [sys.executable, "-c", "pass"], 5)
            with self.assertRaises(RuntimeError):
                rec.run("bad", [sys.executable, "-c", "raise SystemExit(7)"], 5)
            self.assertEqual(
                json.loads((root / "results/bad/receipt.json").read_text())["exit"], 7
            )

    def test_timeout_kills_only_owned_group(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            unrelated = subprocess.Popen(
                [sys.executable, "-c", "import time; time.sleep(60)"],
                start_new_session=True,
            )
            try:
                rec = N.Recorder(root / "results", root)
                with self.assertRaises(RuntimeError):
                    rec.run(
                        "timeout",
                        [sys.executable, "-c", "import time; time.sleep(60)"],
                        0.05,
                    )
                row = json.loads((root / "results/timeout/receipt.json").read_text())
                self.assertTrue(row["group_absent"])
                self.assertIn("TimeoutExpired", row["error"])
                self.assertIsNone(unrelated.poll())
            finally:
                unrelated.terminate()
                unrelated.wait(timeout=5)


if __name__ == "__main__":
    unittest.main()
