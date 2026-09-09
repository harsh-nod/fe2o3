#!/usr/bin/env python3
"""CPU-only runner custody, qualification, and cleanup regression tests."""

import argparse
import contextlib
import copy
import hashlib
import importlib.util
import io
import json
import os
import pathlib
import signal
import subprocess
import tarfile
import tempfile
import unittest
from unittest import mock


HERE = pathlib.Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("r60_runner", HERE / "run-r60-pipeline-mi300x.py")
RUNNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNNER)
CHECKER = RUNNER.load_module(HERE / "check-r60-pipeline.py", "r60_runner_test_checker")
RETAINED = RUNNER.load_module(HERE / "check-parity.py", "r60_runner_test_retained")


def profile():
    device_digest = hashlib.sha256(b"fe2o3.kfd-runtime-profile.device.v1\0")
    for value in (int(RUNNER.UNIQUE_ID, 16).to_bytes(8, "little"), b"gfx942:xnack-", b"\x40\x00"):
        device_digest.update(len(value).to_bytes(8, "little"))
        device_digest.update(value)
    events = []
    for kind in ("dispatch_published", "dispatch_completed"):
        for index in range(64):
            event = {"kind": kind, "dispatch": f"{index + 1:064x}"}
            if kind == "dispatch_completed":
                event["host_timing"] = {"native_binding_ns": 100 if index == 0 else 0}
            events.append({"sequence": len(events), "origin": "observed", "event": event})
    return {
        "schema": "fe2o3-kfd-runtime-profile-v1", "schema_version": 1,
        "device": {"identity": device_digest.hexdigest(), "target_profile": "gfx942:xnack-", "wave_width": 64},
        "coverage": {"origin": "observed", "complete_runtime_operation_history": True,
                     "dropped_events": 0, "observed_events": len(events)},
        "events": events,
    }


def seal(prefix, fields, names):
    text = prefix + " " + " ".join(f"{name}={fields[name]}" for name in names)
    digest = hashlib.sha256((text + "\n").encode()).hexdigest()
    return f"{text} {prefix}_sha256={digest}\n"


class RunnerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.temp.name)
        self.output = self.root / "output"
        self.output.mkdir()
        self.stage = self.root / "stage"
        self.stage.mkdir(mode=0o700)
        args = argparse.Namespace(output_dir=self.output, repo=self.root / "repo")
        self.runner = RUNNER.Runner(args, self.stage)

    def tearDown(self):
        RUNNER.cleanup_tree(self.root)
        self.temp.cleanup()

    def test_profile_accepts_exact_prefix_and_rejects_semantic_gaps(self):
        RUNNER.validate_profile(json.dumps(profile()).encode(), CHECKER)
        for mutation in (
            lambda p: p["coverage"].update(dropped_events=1),
            lambda p: p["coverage"].update(dropped_events=False),
            lambda p: p["coverage"].update(complete_runtime_operation_history=1),
            lambda p: p["coverage"].update(observed_events=127),
            lambda p: p["device"].update(identity="a" * 64),
            lambda p: p["device"].update(target_profile="gfx942:xnack+"),
            lambda p: p["device"].update(wave_width=64.0),
            lambda p: p["events"][1]["event"].update(dispatch=p["events"][0]["event"]["dispatch"]),
            lambda p: p["events"][64]["event"].update(dispatch="f" * 64),
            lambda p: p["events"][65]["event"]["host_timing"].update(native_binding_ns=1),
            lambda p: p["events"][65]["event"]["host_timing"].update(native_binding_ns=False),
            lambda p: p["events"][1].update(sequence=0),
            lambda p: p["events"][1].update(origin="unavailable"),
            lambda p: p["events"].pop(),
        ):
            data = profile()
            mutation(data)
            with self.subTest(data=str(data)[:100]):
                with self.assertRaises(RUNNER.RunError):
                    RUNNER.validate_profile(json.dumps(data).encode(), CHECKER)
        data = profile()
        data["events"][63]["event"], data["events"][64]["event"] = data["events"][64]["event"], data["events"][63]["event"]
        with self.assertRaises(RUNNER.RunError):
            RUNNER.validate_profile(json.dumps(data).encode(), CHECKER)

    def test_profile_rejects_duplicate_json_keys(self):
        data = json.dumps(profile()).replace('"dropped_events": 0', '"dropped_events": 0, "dropped_events": 0')
        with self.assertRaises(CHECKER.CheckError):
            RUNNER.validate_profile(data.encode(), CHECKER)

    def test_monitor_requires_seal_clean_census_and_exact_target_bytes(self):
        output = self.stage / "target.jsonl"
        output.write_text("validated-target\n")
        topology = {"kfd_gpu_id": "12345", "observer_cpu": "4"}
        values = dict(zip(RETAINED.R26_MONITOR_SEALED_FIELDS, (
            "fe2o3.r26-kfd-queue-monitor.v2", "clean",
            "selected-kfd-gpu-process-tree-census-v2", "absolute-monotonic-raw-deadline-v1",
            "12345", "100", "100", "4", "2000", "10000", "3000", "10", "5",
            "0", "0", "0", "1", "1", str(output.stat().st_size), RUNNER.sha256_file(output),
        )))
        text = seal("monitor", values, RETAINED.R26_MONITOR_SEALED_FIELDS)
        RUNNER.validate_monitor(text, output, topology, RETAINED)
        for field, value in (("foreign_selected_queues", "1"), ("terminal_selected_queues", "1"),
                             ("target_exit_code", "2"), ("process_group_absent", "0"),
                             ("target_reaped", "0"), ("target_selected_queue_observations", "0"),
                             ("observations", "2"), ("observed_maximum_gap_us", "10001"),
                             ("process_group", "200"), ("observer_cpu", "5"),
                             ("target_output_sha256", "f" * 64), ("target_output_bytes", "1")):
            altered = dict(values, **{field: value})
            with self.subTest(field=field):
                with self.assertRaises(RUNNER.RunError):
                    RUNNER.validate_monitor(seal("monitor", altered, RETAINED.R26_MONITOR_SEALED_FIELDS),
                                            output, topology, RETAINED)
        output.write_text("different-target\n")
        with self.assertRaises(RUNNER.RunError):
            RUNNER.validate_monitor(text, output, topology, RETAINED)
        with self.assertRaises(RUNNER.RunError):
            RUNNER.validate_monitor(text.replace("status=clean", "status=dirty"), output, topology, RETAINED)

    def test_archive_rejects_traversal_links_duplicates_and_oversize(self):
        def checked(members):
            archive = mock.Mock()
            archive.getmembers.return_value = members
            return RUNNER.archive_members(archive)
        regular = tarfile.TarInfo("source/file.rs")
        regular.size = 3
        self.assertEqual(checked([regular]), [regular])
        for name, kind, size in (("../outside", tarfile.REGTYPE, 1),
                                  ("/outside", tarfile.REGTYPE, 1),
                                  ("link", tarfile.SYMTYPE, 0),
                                  ("hardlink", tarfile.LNKTYPE, 0),
                                  ("fifo", tarfile.FIFOTYPE, 0),
                                  ("oversize", tarfile.REGTYPE, RUNNER.MAX_ARCHIVE_BYTES + 1)):
            member = tarfile.TarInfo(name)
            member.type = kind
            member.size = size
            with self.subTest(name=name):
                with self.assertRaises(RUNNER.RunError):
                    checked([member])
        with self.assertRaises(RUNNER.RunError):
            checked([regular, copy.copy(regular)])

    def test_phase_commands_pin_identity_placement_visibility_and_timeout(self):
        self.runner.topology = {"measurement_cpu_list": "0-3", "numa_node": "0"}
        self.runner.binaries = {backend: self.stage / backend for backend in CHECKER.BACKENDS}
        for backend in CHECKER.BACKENDS:
            command = [str(value) for value in self.runner.phase_command(backend, [RUNNER.UNIQUE_ID])]
            self.assertEqual(command[:3], ["/usr/bin/env", "-i", "LANG=C"])
            self.assertIn("--membind=0", command)
            self.assertIn("--physcpubind=0-3", command)
            self.assertIn("--kill-after=5s", command)
            self.assertIn("180s", command)
            self.assertEqual(command[-1], RUNNER.UNIQUE_ID)
            visibility = [word for word in command if "VISIBLE_DEVICES=" in word]
            self.assertEqual(visibility, [] if backend == "kfd" else
                             ["ROCR_VISIBLE_DEVICES=1" if backend == "hsa" else "HIP_VISIBLE_DEVICES=1"])

    def test_timeout_reaps_owned_subprocess_and_leaves_other_process_untouched(self):
        other = subprocess.Popen(["/usr/bin/sleep", "30"], start_new_session=True)
        try:
            with self.assertRaises(RUNNER.RunError):
                self.runner.run(["/usr/bin/sleep", "30"], label="bounded-sleep", timeout=0)
            self.assertIsNone(other.poll())
        finally:
            other.terminate()
            other.wait(timeout=5)

    def test_early_leader_exit_reaps_retained_child(self):
        code = "import subprocess; p = subprocess.Popen(['/usr/bin/sleep', '30']); print(p.pid, flush=True)"
        with self.assertRaisesRegex(RUNNER.RunError, "left a process group"):
            self.runner.run(["/usr/bin/python3", "-c", code], label="retained-child")
        pid = int((self.runner.evidence / "000-retained-child.stdout").read_text())
        with self.assertRaises(ProcessLookupError):
            os.kill(pid, 0)
        self.assertFalse(RUNNER.group_exists(self.runner.commands[-1]["process_group"]))

    def test_interruption_reaps_command_group_and_retained_child(self):
        code = "import subprocess,time; p = subprocess.Popen(['/usr/bin/sleep', '30']); print(p.pid, flush=True); time.sleep(30)"
        original_sleep = RUNNER.time.sleep
        interrupted = False

        def interrupt_once(duration):
            nonlocal interrupted
            if not interrupted:
                interrupted = True
                original_sleep(0.2)
                raise RUNNER.RunError("simulated interruption")
            original_sleep(duration)

        with mock.patch.object(RUNNER.time, "sleep", side_effect=interrupt_once):
            with self.assertRaisesRegex(RUNNER.RunError, "simulated interruption"):
                self.runner.run(["/usr/bin/python3", "-c", code], label="interrupted-child")
        pid = int((self.runner.evidence / "000-interrupted-child.stdout").read_text())
        with self.assertRaises(ProcessLookupError):
            os.kill(pid, 0)
        self.assertFalse(RUNNER.group_exists(self.runner.commands[-1]["process_group"]))

    def test_main_failure_removes_private_stage_and_preserves_caller_files(self):
        repo = self.root / "repo"
        repo.mkdir()
        signers = self.root / "signers"
        signers.write_text("trusted-key-placeholder\n")
        marker = self.output / "caller-owned"
        marker.write_text("preserve")
        arguments = ["--repo", str(repo), "--allowed-signers", str(signers),
                     "--output-dir", str(self.output), "--rocm-path", str(self.root),
                     "--staging-parent", str(self.root)]
        previous_umask = os.umask(0o077)
        try:
            with mock.patch.object(RUNNER.Runner, "snapshot", side_effect=RUNNER.RunError("rejected source")), \
                    mock.patch.object(signal, "signal"), contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(RUNNER.main(arguments), 2)
        finally:
            os.umask(previous_umask)
        self.assertEqual(marker.read_text(), "preserve")
        self.assertEqual(list(self.root.glob("fe2o3-r60.*")), [])
        self.assertEqual(list(self.output.iterdir()), [marker])

    def test_nonzero_and_excess_output_are_rejected(self):
        with self.assertRaises(RUNNER.RunError):
            self.runner.run(["/usr/bin/false"], label="failure")
        with self.assertRaises(RUNNER.RunError):
            self.runner.run(["/usr/bin/printf", "too-much-output"], label="large-output", limit=4)
        self.assertEqual(self.runner.run(["/usr/bin/printf", "ok"], label="success"), "ok")

    def prepare_publication(self):
        self.runner.commit = "1" * 40
        self.runner.set_id = "2" * 64
        self.runner.slots = []
        self.runner.topology = {"kfd_gpu_id": "12345"}
        self.runner.identity_start = {"pci_bdf": "0000:26:00.0"}
        self.runner.binaries = {}
        self.runner.binary_hashes = {}
        for backend in CHECKER.BACKENDS:
            path = self.stage / backend
            path.write_bytes(backend.encode())
            self.runner.binaries[backend] = path
            self.runner.binary_hashes[backend] = RUNNER.sha256_file(path)
        for name in ("source.tar", "source-files.json", "allowed-signers"):
            (self.runner.evidence / name).write_bytes(b"fixture\n")
        self.runner.snapshot_input_hashes = {}
        self.runner.tool_hashes = {}

    def test_publication_is_private_verified_and_does_not_overwrite_existing_output(self):
        self.prepare_publication()
        destination = self.runner.publish()
        self.assertEqual(destination.stat().st_mode & 0o077, 0)
        manifest = json.loads((destination / "sha256.json").read_text())
        actual = RUNNER.tree_hashes(destination)
        del actual["sha256.json"]
        self.assertEqual(actual, manifest)
        self.assertFalse(list(self.output.glob(".r60-publish.*")))
        with self.assertRaises((RUNNER.RunError, FileExistsError)):
            self.runner.publish()
        self.assertTrue((destination / "sha256.json").is_file())

    def test_failed_copy_verification_leaves_no_published_or_temporary_output(self):
        self.prepare_publication()
        real_copytree = RUNNER.shutil.copytree

        def corrupt(source, destination, *args, **kwargs):
            result = real_copytree(source, destination, *args, **kwargs)
            if pathlib.Path(destination).parent == self.output:
                (pathlib.Path(destination) / "allowed-signers").write_text("changed")
            return result

        with mock.patch.object(RUNNER.shutil, "copytree", side_effect=corrupt):
            with self.assertRaises(RUNNER.RunError):
                self.runner.publish()
        self.assertEqual(list(self.output.iterdir()), [])

    def test_readonly_source_is_cleanable_and_tampering_is_detected(self):
        source = self.stage / "source"
        source.mkdir()
        nested = source / "nested"
        nested.mkdir()
        file = nested / "file"
        file.write_bytes(b"original")
        hashes = RUNNER.tree_hashes(source)
        RUNNER.readonly_tree(source)
        self.assertEqual(source.stat().st_mode & 0o222, 0)
        self.assertEqual(file.stat().st_mode & 0o222, 0)
        file.chmod(0o600)
        file.write_bytes(b"changed")
        self.assertNotEqual(hashes, RUNNER.tree_hashes(source))
        RUNNER.cleanup_tree(source)
        self.assertFalse(source.exists())

    def test_latin_square_covers_each_backend_once_at_each_position(self):
        self.assertEqual(len(RUNNER.ORDERS), 3)
        for order in RUNNER.ORDERS:
            self.assertEqual(set(order), set(CHECKER.BACKENDS))
        for position in range(3):
            self.assertEqual({order[position] for order in RUNNER.ORDERS}, set(CHECKER.BACKENDS))

    def test_hsa_preserves_invalid_header_and_times_signal_rearm(self):
        source = (HERE / "r60_pipeline_hsa.cpp").read_text()
        self.assertNotIn("std::memcpy(packet,", source)
        self.assertIn("reinterpret_cast<std::byte *>(packet) + header_bytes", source)
        self.assertIn("sizeof(*packet) - header_bytes", source)
        start = source.index("const auto start = Clock::now();")
        rearm = source.index("hsa_signal_store_screlease(signals[i], 1);")
        publication = source.index("__atomic_store_n(", start)
        issued = source.index("const auto issued = Clock::now();", start)
        self.assertLess(start, rearm)
        self.assertLess(rearm, publication)
        self.assertLess(publication, issued)
        done = source.index("const auto done = Clock::now();", issued)
        confirmations = source.index("for (auto signal : signals)", done)
        self.assertLess(done, confirmations)


if __name__ == "__main__":
    unittest.main()
