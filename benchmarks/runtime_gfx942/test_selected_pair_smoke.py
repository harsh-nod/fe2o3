#!/usr/bin/env python3
"""CPU-only controller tests. Fake command reports are not native evidence."""

import importlib.util
import ctypes
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock

SPEC = importlib.util.spec_from_file_location("selected_pair_smoke", Path(__file__).with_name("selected_pair_smoke.py"))
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)
DEVICES = ("6,0000:c5:00.0,0x0000000000000006", "7,0000:c6:00.0,0x0000000000000007")
PAIR = M.endpoints(DEVICES)


def line(fields):
    return "PASS " + " ".join(f"{key}={value}" for key, value in fields.items()) + "\n"


class FakeRecorder(M.Recorder):
    def __init__(self, output, mutate=None):
        super().__init__(output)
        self.mutate = mutate
        self.calls = []

    def run(self, name, argv, seconds, **kwargs):
        self.calls.append((name, argv, seconds, kwargs))
        self.commands.append(name)
        folder = self.output / name
        folder.mkdir()
        staged = "--staged-default" in argv
        pair = PAIR if argv[-2] == PAIR[0][2] else tuple(reversed(PAIR))
        output = line(M.expected(pair, staged))
        record = {"exit": 0, "error": None, "group_absent": True}
        if self.mutate:
            output, record = self.mutate(name, output, record)
        (folder / "stdout").write_text(output)
        (folder / "stderr").write_text("")
        return record


class ControllerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.binary = self.root / M.EXAMPLE
        header = b"\x7fELF\x02\x01" + b"\0" * 12 + b"\x3e\x00"
        self.binary.write_bytes(header + b"synthetic controller test bytes")
        self.binary.chmod(0o700)
        self.fd = os.open(self.binary, os.O_RDONLY)
        self.addCleanup(os.close, self.fd)
        self.identity = M.fingerprint(self.fd)

    def result_dir(self):
        path = self.root / str(len(tuple(self.root.iterdir())))
        path.mkdir()
        return path

    def test_pair_requires_exact_distinct_bound_nonzero_identity_tuple(self):
        self.assertEqual(PAIR[0], (6, "0000:c5:00.0", "0x0000000000000006"))
        for value in ([], [DEVICES[0]], [*DEVICES, DEVICES[0]], [DEVICES[0]] * 2,
                      [DEVICES[0], "7,0000:c6:00.0,0x0000000000000000"],
                      [DEVICES[0], "7,0000:c5:00.0,0x0000000000000007"],
                      [DEVICES[0], "6,0000:c6:00.0,0x0000000000000007"],
                      [DEVICES[0], "64,0000:c6:00.0,0x0000000000000007"],
                      [DEVICES[0], "7,0000:c6:00.0,0x7"]):
            with self.subTest(value=value), self.assertRaises(ValueError):
                M.endpoints(value)

    def test_exact_reports_reject_missing_duplicate_changed_and_unexpected_fields(self):
        for staged in (False, True):
            wanted = M.expected(PAIR, staged)
            self.assertEqual(M.parse_pass(line(wanted), wanted), wanted)
            changes = []
            for key in wanted:
                missing = dict(wanted)
                del missing[key]
                changed = {**wanted, key: "unexpected"}
                changes.extend([line(missing), line(changed)])
            changes.extend(["", "PASS\n", line(wanted) * 2, "diagnostic\n" + line(wanted),
                            line(wanted).rstrip() + " cleanup=logical-and-native-explicit\n",
                            line({**wanted, "extra": "value"})])
            for output in changes:
                with self.subTest(staged=staged, output=output), self.assertRaises(ValueError):
                    M.parse_pass(output, wanted)

    def test_four_cases_use_exact_pair_pinned_fd_and_per_case_cleanup(self):
        recorder = FakeRecorder(self.result_dir())
        with mock.patch.object(M, "observe_pair") as observations:
            result = M.campaign(recorder, None, PAIR, self.binary, self.fd, self.identity, Path("/smi"))
        self.assertTrue(result["accepted"])
        self.assertEqual([row["name"] for row in result["cases"]], [row[0] for row in M.CASES])
        self.assertEqual(observations.call_count, 8)
        for (name, staged, reverse), call in zip(M.CASES, recorder.calls):
            selected = tuple(reversed(PAIR)) if reverse else PAIR
            self.assertEqual(call[1], [str(self.binary), *(["--staged-default"] if staged else []), selected[0][2], selected[1][2]])
            self.assertEqual(call[2], 180)
            self.assertEqual(call[3], {"executable": f"/proc/self/fd/{self.fd}", "pass_fds": (self.fd,)})
            self.assertNotIn("--all", call[1])
        self.assertEqual([call.args[-1] for call in observations.call_args_list],
                         [name + edge for name, _, _ in M.CASES for edge in ("-before", "-after")])
        self.assertTrue(all(row["cleanup_observed"] for row in result["cases"]))
        self.assertEqual(json.loads((recorder.output / "result.json").read_text())["accepted"], True)

    def test_failed_or_missing_pass_stops_matrix_but_still_observes_cleanup(self):
        for mutate in (lambda _n, text, record: (text, {**record, "exit": 1}),
                       lambda _n, _text, record: ("", record),
                       lambda _n, text, record: (text.replace("cleanup=logical-and-native-explicit", "cleanup=missing"), record)):
            recorder = FakeRecorder(self.result_dir(), mutate)
            with mock.patch.object(M, "observe_pair") as observations:
                result = M.campaign(recorder, None, PAIR, self.binary, self.fd, self.identity, Path("/smi"))
            self.assertFalse(result["accepted"])
            self.assertEqual(len(recorder.calls), 1)
            self.assertEqual(observations.call_count, 2)
            self.assertTrue(result["cases"][0]["cleanup_observed"])

    def test_absent_admission_or_failed_post_cleanup_cannot_accept(self):
        for denied_call in (1, 2):
            calls = []

            def observation(*args):
                calls.append(args[-1])
                if len(calls) == denied_call:
                    raise ValueError("selected GPU has foreign attachment or missing evidence")

            recorder = FakeRecorder(self.result_dir())
            with mock.patch.object(M, "observe_pair", side_effect=observation):
                result = M.campaign(recorder, None, PAIR, self.binary, self.fd, self.identity, Path("/smi"))
            self.assertFalse(result["accepted"])
            self.assertEqual(len(recorder.calls), denied_call - 1)
            self.assertFalse(result["cases"][0]["cleanup_observed"])

    def test_binary_mutation_and_path_replacement_are_rejected(self):
        for replace in (False, True):
            original = self.binary.read_bytes()

            def mutate(_name, text, record):
                if replace:
                    self.binary.rename(self.root / "retained-original")
                self.binary.write_bytes(original + b"changed")
                return text, record

            recorder = FakeRecorder(self.result_dir(), mutate)
            with mock.patch.object(M, "observe_pair"):
                result = M.campaign(recorder, None, PAIR, self.binary, self.fd, self.identity, Path("/smi"))
            self.assertFalse(result["accepted"])
            self.assertEqual(len(recorder.calls), 1)
            if not replace:
                self.binary.write_bytes(original)
                self.identity = M.fingerprint(self.fd)

    def test_observer_missing_metrics_malformed_output_and_foreign_occupancy_fail_closed(self):
        helper = M.load_observer()
        # The real reviewed observer/parser runs over a synthetic sysfs tree.
        root = self.root / "sysfs"
        for _index, bdf, uid in PAIR:
            directory = root / bdf
            directory.mkdir(parents=True)
            for metric in helper.METRICS:
                (directory / metric).write_text(uid[2:] if metric == "unique_id" else "0")
        header = "==== ROCm System Management Interface ====\n==== GPUs Indexed by PID ====\n"
        footer = "====\n==== End of ROCm SMI Log ====\n"
        for kind in ("missing", "malformed", "foreign"):
            with self.subTest(kind=kind):
                index, bdf, uid = PAIR[0]
                if kind == "missing":
                    (root / bdf / "gpu_busy_percent").unlink()
                status = json.dumps({f"card{index}": {"Unique ID": uid, "PCI Bus": bdf,
                    "GPU use (%)": "0", "VRAM Total Used Memory (B)": "0"}})
                pids = "malformed" if kind == "malformed" else header + "PID 123 is using 1 DRM device(s):\n" + ("6" if kind == "foreign" else "0") + "\n" + footer
                values = iter([status, pids])
                row = helper.observe(index, bdf, uid, Path("/smi"), root=root,
                    run=lambda _argv: {"exit": 0, "error": None, "stderr": "", "stdout": next(values)})
                self.assertFalse(row["endpoint_admitted"])
                if kind == "missing":
                    (root / bdf / "gpu_busy_percent").write_text("0")

    def test_timeout_only_kills_own_unreaped_child_group_and_records_rejection(self):
        process = mock.Mock(pid=54321)
        process.wait.return_value = -signal.SIGKILL
        recorder = M.Recorder(self.result_dir())
        with mock.patch.object(M.subprocess, "Popen", return_value=process) as popen, \
             mock.patch.object(M, "await_unreaped", side_effect=subprocess.TimeoutExpired(["owned"], 1)), \
             mock.patch.object(M, "group_members", side_effect=[{54321}, set(), set()]), \
             mock.patch.object(M.os, "getpgid", return_value=54321), \
             mock.patch.object(M.os, "killpg") as kill:
            result = recorder.run("timeout", ["/owned"], 1)
        kill.assert_called_once_with(54321, signal.SIGKILL)
        self.assertTrue(popen.call_args.kwargs["start_new_session"])
        self.assertEqual(popen.call_args.kwargs["env"], M.ENV)
        self.assertEqual(result["exit"], -signal.SIGKILL)
        self.assertIn("TimeoutExpired", result["error"])
        self.assertTrue(result["group_absent"])
        self.assertTrue((recorder.output / "timeout" / "receipt.json").is_file())

    def test_normal_exit_never_sends_a_signal(self):
        process = mock.Mock(pid=54321)
        process.wait.return_value = 0
        recorder = M.Recorder(self.result_dir())
        with mock.patch.object(M.subprocess, "Popen", return_value=process), \
             mock.patch.object(M, "await_unreaped", return_value=object()), \
             mock.patch.object(M, "group_members", side_effect=[{54321}, set(), set()]), \
             mock.patch.object(M.os, "killpg") as kill:
            record = recorder.run("done", ["/owned"], 1)
        self.assertEqual(record["exit"], 0)
        self.assertIsNone(record["error"])
        kill.assert_not_called()

    def test_surviving_descendants_reject_and_signal_before_leader_reap(self):
        actions = []
        process = mock.Mock(pid=54321)
        process.wait.side_effect = lambda **_kwargs: actions.append("reap") or 0
        recorder = M.Recorder(self.result_dir())
        with mock.patch.object(M.subprocess, "Popen", return_value=process), \
             mock.patch.object(M, "await_unreaped", return_value=object()), \
             mock.patch.object(M, "group_members", side_effect=[{54321, 54322}, set(), set()]), \
             mock.patch.object(M.os, "getpgid", return_value=54321), \
             mock.patch.object(M.os, "killpg", side_effect=lambda *_: actions.append("signal")):
            record = recorder.run("descendant", ["/owned"], 1)
        self.assertEqual(actions, ["signal", "reap"])
        self.assertIn("descendants", record["error"])
        self.assertEqual(record["members_before_reap"], [54321, 54322])
        self.assertTrue(record["group_absent"])

    def test_group_identity_contradiction_never_signals_another_group(self):
        process = mock.Mock(pid=54321)
        recorder = M.Recorder(self.result_dir())
        with mock.patch.object(M.subprocess, "Popen", return_value=process), \
             mock.patch.object(M, "await_unreaped", side_effect=subprocess.TimeoutExpired(["owned"], 1)), \
             mock.patch.object(M, "group_members", return_value={54321}), \
             mock.patch.object(M.os, "getpgid", return_value=999), \
             mock.patch.object(M.os, "killpg") as kill:
            record = recorder.run("contradiction", ["/owned"], 1)
        kill.assert_not_called()
        self.assertIsNone(record["exit"])
        self.assertIsNotNone(record["error"])

    def test_failed_census_still_closes_owned_unreaped_group(self):
        for timed_out in (False, True):
            with self.subTest(timed_out=timed_out):
                actions = []
                process = mock.Mock(pid=54321)
                process.wait.side_effect = lambda **_kwargs: actions.append("reap") or -signal.SIGKILL
                recorder = M.Recorder(self.result_dir())
                wait = {"side_effect": subprocess.TimeoutExpired(["owned"], 1)} if timed_out else {
                    "return_value": object()}
                with mock.patch.object(M.subprocess, "Popen", return_value=process), \
                     mock.patch.object(M, "await_unreaped", **wait), \
                     mock.patch.object(M, "group_members", side_effect=[PermissionError("census denied"), set(), set()]), \
                     mock.patch.object(M.os, "getpgid", return_value=54321), \
                     mock.patch.object(M.os, "killpg", side_effect=lambda *_: actions.append("signal")) as kill:
                    record = recorder.run("census", ["/owned"], 1)
                self.assertEqual(actions, ["signal", "reap"])
                kill.assert_called_once_with(54321, signal.SIGKILL)
                process.wait.assert_called_once_with(timeout=10)
                self.assertEqual(record["exit"], -signal.SIGKILL)
                self.assertIn("census denied", record["cleanup_error"])
                self.assertIsNotNone(record["error"])
                self.assertTrue(record["group_absent"])
                if timed_out:
                    self.assertIn("TimeoutExpired", record["error"])

    def test_wnowait_retains_leader_and_child_limits_are_scoped(self):
        process = mock.Mock(pid=54321)
        status = object()
        with mock.patch.object(M.os, "waitid", return_value=status) as wait:
            self.assertIs(M.await_unreaped(process, 1), status)
        wait.assert_called_once_with(os.P_PID, 54321, os.WEXITED | os.WNOHANG | os.WNOWAIT)
        process.wait.assert_not_called()
        process.poll.assert_not_called()
        with mock.patch.object(M.resource, "setrlimit") as limit, \
             mock.patch.object(M.signal, "pthread_sigmask") as mask:
            M.child_setup(set())
        self.assertEqual(limit.call_args_list, [
            mock.call(M.resource.RLIMIT_FSIZE, (M.MAX_OUTPUT, M.MAX_OUTPUT)),
            mock.call(M.resource.RLIMIT_CORE, (0, 0))])
        mask.assert_called_once_with(signal.SIG_SETMASK, set())

    def test_real_true_closes_without_signaling(self):
        recorder = M.Recorder(self.result_dir())
        started = time.monotonic()
        with mock.patch.object(M.os, "killpg", wraps=os.killpg) as kill:
            record = recorder.run("true", ["/usr/bin/true"], 2)
        self.assertEqual(M.command_text(recorder, "true", record), "")
        self.assertEqual(record["members_before_reap"], [record["pid"]])
        self.assertFalse(Path(f"/proc/{record['pid']}").exists())
        self.assertLess(time.monotonic() - started, 5)
        kill.assert_not_called()

    def test_real_sleep_timeout_closes_owned_group(self):
        recorder = M.Recorder(self.result_dir())
        started = time.monotonic()
        record = recorder.run("sleep", [sys.executable, "-I", "-B", "-c", "import time; time.sleep(3)"], 0.1)
        self.assertIn("TimeoutExpired", record["error"])
        self.assertEqual(record["exit"], -signal.SIGKILL)
        self.assertTrue(record["group_absent"])
        self.assertFalse(Path(f"/proc/{record['pid']}").exists())
        self.assertLess(time.monotonic() - started, 5)
        with self.assertRaises(ValueError):
            M.command_text(recorder, "sleep", record)

    def test_real_oversized_output_is_bounded_and_rejected(self):
        recorder = M.Recorder(self.result_dir())
        started = time.monotonic()
        code = "import os\nfor _ in range(64): os.write(1, b'x' * 65536)"
        record = recorder.run("output", [sys.executable, "-I", "-B", "-c", code], 2)
        self.assertNotEqual(record["exit"], 0)
        self.assertEqual(record["stdout_bytes"], M.MAX_OUTPUT)
        self.assertLessEqual(record["stderr_bytes"], M.MAX_OUTPUT)
        self.assertTrue(record["group_absent"])
        self.assertLess(time.monotonic() - started, 5)
        with self.assertRaises(ValueError):
            M.command_text(recorder, "output", record)

    def test_real_surviving_fork_is_rejected_and_owned_descendant_reaped(self):
        # Reap only this test's orphan, even under a container PID 1 that does
        # not reap. The runner still retains/signals its own unreaped leader.
        libc = ctypes.CDLL(None, use_errno=True)
        original = ctypes.c_int()
        self.assertEqual(libc.prctl(37, ctypes.byref(original), 0, 0, 0), 0)
        self.assertEqual(libc.prctl(36, 1, 0, 0, 0), 0)
        child_file = self.root / "owned-child"
        reaped = set()

        def reap_owned(_signal=None, _frame=None):
            if not child_file.exists():
                return
            child = int(child_file.read_text())
            try:
                pid, _status = os.waitpid(child, os.WNOHANG)
                if pid:
                    reaped.add(pid)
            except ChildProcessError:
                pass

        previous_handler = signal.signal(signal.SIGCHLD, reap_owned)
        started = time.monotonic()
        try:
            code = ("import os,time\nfrom pathlib import Path\nchild=os.fork()\n"
                    "if child == 0:\n time.sleep(2)\n os._exit(0)\n"
                    f"Path({str(child_file)!r}).write_text(str(child))\nos._exit(0)\n")
            recorder = M.Recorder(self.result_dir())
            record = recorder.run("fork", [sys.executable, "-I", "-B", "-c", code], 2)
            child = int(child_file.read_text())
            self.assertEqual(record["exit"], 0)
            self.assertIn("descendants", record["error"])
            self.assertEqual(record["members_before_reap"], sorted((record["pid"], child)))
            self.assertTrue(record["group_absent"])
            self.assertEqual(reaped, {child})
            self.assertFalse(Path(f"/proc/{child}").exists())
            self.assertLess(time.monotonic() - started, 5)
            with self.assertRaises(ValueError):
                M.command_text(recorder, "fork", record)
        finally:
            # The owned descendant also has a two-second self-exit bound if
            # an assertion or controller failure prevents normal cleanup.
            deadline = time.monotonic() + 3
            while child_file.exists() and not reaped and time.monotonic() < deadline:
                reap_owned()
                time.sleep(0.01)
            signal.signal(signal.SIGCHLD, previous_handler)
            self.assertEqual(libc.prctl(36, original.value, 0, 0, 0), 0)


if __name__ == "__main__":
    unittest.main()
