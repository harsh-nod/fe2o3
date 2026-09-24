#!/usr/bin/env python3
"""CPU controls for the fixed hot-batch campaign; no SSH or GPU operations."""

import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tarfile
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("hot_batch_campaign_test", HERE / "campaign.py")
C = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(C)
N = C.N
DEVICES = [[5, "0000:a6:00.0", "0xb7baafd0fb173d8e"], [6, "0000:c6:00.0", "0x10a254ce4987e716"]]
EXPECTED = [("kfd", 1), ("hsa", 1), ("hip", 1), ("kfd", 16), ("hsa", 16), ("hip", 16),
            ("kfd", 32), ("hsa", 32), ("hip", 32), ("hip", 32), ("hsa", 32), ("kfd", 32),
            ("hip", 16), ("hsa", 16), ("kfd", 16), ("hip", 1), ("hsa", 1), ("kfd", 1)]


class Controls(unittest.TestCase):
    def test_exact_order_names_and_controls(self):
        rows = list(N.trials(Path("/owned"), DEVICES))
        self.assertEqual([(row[1], row[2]) for row in rows], EXPECTED)
        self.assertEqual(len({row[0] for row in rows}), 18)
        for _, backend, depth, command, env in rows:
            controls = ["1048576", str(depth), "10", "30"]
            if backend == "kfd":
                self.assertEqual(command, ["/owned/kfd", *[d[2] for d in DEVICES], *controls, "--aggregate-peer-batch-hot-only"])
                self.assertNotIn("HIP_VISIBLE_DEVICES", env)
                self.assertNotIn("ROCR_VISIBLE_DEVICES", env)
            else:
                self.assertEqual(command, ["/owned/peer-" + backend, "0", "1", *controls, *[d[2] for d in DEVICES], "--persistent-hot"])
                self.assertEqual(env["HSA_XNACK"], "0")
                own = "HIP_VISIBLE_DEVICES" if backend == "hip" else "ROCR_VISIBLE_DEVICES"
                other = "ROCR_VISIBLE_DEVICES" if backend == "hip" else "HIP_VISIBLE_DEVICES"
                self.assertEqual(env[own], "5,6")
                self.assertNotIn(other, env)

    def test_reversed_endpoint_order(self):
        rows = list(N.trials(Path("/owned"), DEVICES[::-1]))
        self.assertEqual(rows[0][3][1:3], [DEVICES[1][2], DEVICES[0][2]])
        self.assertEqual(rows[1][4]["ROCR_VISIBLE_DEVICES"], "6,5")
        self.assertEqual(rows[2][4]["HIP_VISIBLE_DEVICES"], "6,5")

    def test_outer_deadline_covers_all_inner_bounds_and_stop_margins(self):
        self.assertEqual(N.OBSERVE_SECONDS, 100)
        self.assertEqual(N.TRIAL_SECONDS, 300)
        self.assertEqual(N.BUILD_SECONDS, 180)
        self.assertEqual(N.REMOTE_SECONDS, 19446)
        self.assertEqual(N.REMOTE_SECONDS, 6 * (30 + 15) + 2 * (180 + 15)
                         + 18 * (6 * (100 + 15) + 300 + 15 + 22) + 300)

    def test_cold_target_refuses_existing_directory_and_symlink(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            env = C.build_environment(root)
            self.assertEqual(env["CARGO_TARGET_DIR"], str(root / "target"))
            self.assertEqual(list((root / "target").iterdir()), [])
            with self.assertRaises(FileExistsError):
                C.build_environment(root)
            (root / "target").rmdir()
            (root / "target").symlink_to(root, target_is_directory=True)
            with self.assertRaises(FileExistsError):
                C.build_environment(root)

    def test_canonical_distinct_devices(self):
        good = [",".join(map(str, device)) for device in DEVICES]
        self.assertEqual(C.K.devices_from_args(good), DEVICES)
        for values in ([good[0], good[0]], [good[0]], [good[0].replace("5,", "05,"), good[1]],
                       [good[0].replace("b7baafd0fb173d8e", "0000000000000000"), good[1]]):
            with self.subTest(values=values), self.assertRaises((RuntimeError, ValueError)):
                C.K.devices_from_args(values)


class Source(unittest.TestCase):
    def fixture(self, mutation=None):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            tree = root / "tree"
            tree.mkdir()
            files = {"Cargo.toml": b"[workspace]\n", "crates/a/lib.rs": b"pub fn a() {}\n"}
            rows = []
            for name, raw in files.items():
                oid = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
                rows.append(f"100644 blob {oid}\t{name}".encode())
            (tree / "stdout").write_bytes(b"\0".join(rows) + b"\0")
            with tarfile.open(root / "build-source.tar.gz", "w:gz") as archive:
                for name in ("crates/", "crates/a/"):
                    info = tarfile.TarInfo(name)
                    info.type = tarfile.DIRTYPE
                    archive.addfile(info)
                if mutation == "directory":
                    info = tarfile.TarInfo("../escape/")
                    info.type = tarfile.DIRTYPE
                    archive.addfile(info)
                for name, raw in files.items():
                    if mutation == "missing" and name == "Cargo.toml":
                        continue
                    if mutation == "substitution" and name == "Cargo.toml":
                        raw = b"changed\n"
                    info = tarfile.TarInfo(name)
                    info.size, info.mode = len(raw), 0o644
                    if mutation == "symlink":
                        info.type, info.linkname, info.size = tarfile.SYMTYPE, "/foreign", 0
                    archive.addfile(info, io.BytesIO(raw))
            rec = mock.Mock()
            rec.run.return_value = tree
            result = C.source(rec, root)[1]
            command = rec.run.call_args_list[1].args[1]
            self.assertIn("--output=" + str(root / "build-source.tar.gz"), command)
            return result

    def test_exact_git_files_and_parents_accept(self):
        self.assertEqual(set(self.fixture()), {"Cargo.toml", "crates/a/lib.rs"})

    def test_missing_substituted_foreign_and_symlink_members_reject(self):
        for mutation in ("missing", "substitution", "directory", "symlink"):
            with self.subTest(mutation=mutation), self.assertRaises(RuntimeError):
                self.fixture(mutation)

    def test_cpu_binding_requires_exact_source_not_subset(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "stdout").write_text(json.dumps({"source": {"Cargo.toml": "a" * 64}}))
            rec = mock.Mock()
            rec.run.return_value = root
            with mock.patch.object(C, "authenticate_cpu_packets", return_value={"fixture": "a" * 64}):
                C.cpu_binding(rec, {"Cargo.toml": "a" * 64})
            for files in ({}, {"Cargo.toml": "b" * 64}, {"Cargo.toml": "a" * 64, "crates/new.rs": "c" * 64}):
                rec.reset_mock()
                with self.subTest(files=files), mock.patch.object(C, "authenticate_cpu_packets", return_value={}), \
                        self.assertRaisesRegex(RuntimeError, "exact signed CPU"):
                    C.cpu_binding(rec, files)
                self.assertEqual(rec.run.call_count, 1)

    def test_all_cpu_packet_bytes_and_roster_are_authenticated(self):
        for mutation in (None, "bytes", "extra", "missing", "symlink", "mode"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                rows = []
                for packet in (C.CPU, C.PRIOR_CPU):
                    path = root / packet / "receipt.json"
                    path.parent.mkdir(parents=True)
                    raw = b'{"status": 0}\n'
                    path.write_bytes(raw)
                    oid = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
                    rows.append(f"100644 blob {oid}\t{packet}/receipt.json".encode())
                folder = root / "records"
                folder.mkdir()
                (folder / "stdout").write_bytes(b"\0".join(rows) + b"\0")
                path = root / C.CPU / "receipt.json"
                if mutation == "bytes":
                    path.write_bytes(b'{"status": 1}\n')
                elif mutation == "extra":
                    (path.parent / "extra").write_bytes(b"extra")
                elif mutation in ("missing", "symlink"):
                    path.unlink()
                    if mutation == "symlink":
                        path.symlink_to(root / C.PRIOR_CPU / "receipt.json")
                elif mutation == "mode":
                    path.chmod(0o755)
                rec = mock.Mock()
                rec.run.return_value = folder
                with mock.patch.object(C, "REPO", root):
                    if mutation:
                        with self.assertRaises(RuntimeError):
                            C.authenticate_cpu_packets(rec)
                    else:
                        self.assertEqual(len(C.authenticate_cpu_packets(rec)), 2)

    def test_helper_substitution_rejected_before_execution(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "helper.py"
            path.write_text("raise AssertionError('executed untrusted helper')\n")
            for load in (C.load, N.load):
                with self.assertRaisesRegex(RuntimeError, "authenticated helper"):
                    load(path, "0" * 64, "untrusted")


class TrialLifecycle(unittest.TestCase):
    def exercise(self, failure=None):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "stdout").write_bytes(b"result\n")
            (root / "stderr").write_bytes(b"bad\n" if failure == "stderr" else b"")
            rec = mock.Mock()
            rec.run.return_value = root
            if failure == "command":
                rec.run.side_effect = RuntimeError("command failure")
            parser = SimpleNamespace(parse_result=mock.Mock(return_value={"fixture": True}))
            if failure == "parser":
                parser.parse_result.side_effect = ValueError("parser failure")
            identities = mock.Mock()
            labels, sleeps, results = [], [], []
            def observe(label):
                labels.append(label)
                if failure == "admission" and label.endswith("-before"):
                    raise RuntimeError("admission failure")
                if failure == "postflight" and label.endswith("-settled"):
                    raise RuntimeError("postflight failure")
            settle = N.B.settled_postflight
            def postflight(callback, name, error):
                return settle(callback, name, error, sleep=sleeps.append)
            with mock.patch.object(N.B, "settled_postflight", side_effect=postflight):
                if failure:
                    with self.assertRaises((RuntimeError, ValueError)):
                        N.run_trials(rec, root, DEVICES, parser, identities, observe, results)
                else:
                    N.run_trials(rec, root, DEVICES, parser, identities, observe, results)
            return rec, parser, identities, labels, sleeps, results

    def test_every_trial_is_bracketed_and_parser_gets_independent_controls(self):
        rec, parser, identities, labels, sleeps, results = self.exercise()
        self.assertEqual(len(results), 18)
        self.assertEqual(rec.run.call_count, 18)
        self.assertEqual(identities.call_count, 18)
        self.assertEqual(sleeps, [2, 20] * 18)
        expected_labels = []
        for ordinal, (backend, depth) in enumerate(EXPECTED, 1):
            name = f"{ordinal:02}-{backend}-depth{depth}"
            expected_labels.extend(name + suffix for suffix in ("-before", "-settled", "-delayed"))
            self.assertEqual(parser.parse_result.call_args_list[ordinal - 1].kwargs,
                             dict(backend=backend, unique_ids=[d[2] for d in DEVICES], copy_bytes=1048576,
                                  depth=depth, warmups=10, samples=30))
            self.assertEqual(rec.run.call_args_list[ordinal - 1].args[2], 300)
        self.assertEqual(labels, expected_labels)

    def test_admission_failure_never_runs_workload_and_still_postflights(self):
        rec, _, _, labels, sleeps, _ = self.exercise("admission")
        rec.run.assert_not_called()
        self.assertEqual(labels, ["01-kfd-depth1" + suffix for suffix in ("-before", "-settled", "-delayed")])
        self.assertEqual(sleeps, [2, 20])

    def test_command_parse_stderr_and_postflight_failures_stop_after_postflights(self):
        for failure in ("command", "parser", "stderr", "postflight"):
            with self.subTest(failure=failure):
                rec, _, _, labels, sleeps, _ = self.exercise(failure)
                self.assertEqual(rec.run.call_count, 1)
                self.assertEqual(labels[-2:], ["01-kfd-depth1-settled", "01-kfd-depth1-delayed"])
                self.assertEqual(sleeps, [2, 20])

    def test_endpoint_failure_still_observes_the_other_endpoint(self):
        rec = mock.Mock()
        rec.run.side_effect = RuntimeError("observer failed")
        with self.assertRaisesRegex(RuntimeError, "observer failed"):
            N.observe_endpoints(rec, {}, DEVICES, "test")
        self.assertEqual([call.args[0] for call in rec.run.call_args_list], ["test-gpu5", "test-gpu6"])
        self.assertTrue(all(call.args[2] == 100 for call in rec.run.call_args_list))

    def test_endpoint_parse_failure_still_observes_the_other_endpoint(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "stdout").write_bytes(b"invalid\n")
            (root / "stderr").write_bytes(b"")
            rec = mock.Mock()
            rec.run.return_value = root
            with self.assertRaises(RuntimeError):
                N.observe_endpoints(rec, {}, DEVICES, "test")
            self.assertEqual(rec.run.call_count, 2)


if __name__ == "__main__":
    unittest.main()
