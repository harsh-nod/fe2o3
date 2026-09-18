#!/usr/bin/env python3
"""CPU-only tests of complete copy-host telemetry and fail-closed decisions."""

import contextlib
import copy
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock

SPEC = importlib.util.spec_from_file_location(
    "copy_host_observe", Path(__file__).with_name("copy-host-observe.py")
)
OBSERVER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(OBSERVER)
BDF = "0000:85:00.0"
UID = "0x54f88318ca05093d"
HEADER = "==== ROCm System Management Interface ====\n==== GPUs Indexed by PID ====\n"
FOOTER = "====\n==== End of ROCm SMI Log ====\n"


def pid_text(
    rows="PID 42 is using 1 DRM device(s):\n0\nPID 43 is using 0 DRM device(s)\n",
):
    return HEADER + rows + FOOTER


def status_text(**changes):
    card = {
        "Unique ID": UID,
        "PCI Bus": BDF,
        "GPU use (%)": "0",
        "VRAM Total Used Memory (B)": "298647552",
    }
    card.update(changes)
    return json.dumps({"card4": card})


def command_record(stdout, **changes):
    result = {"stdout": stdout, "stderr": "", "exit": 0, "error": None}
    result.update(changes)
    return result


class ObserverTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        device = self.root / BDF
        device.mkdir()
        for name in OBSERVER.METRICS:
            self.metric(name, UID[2:] if name == "unique_id" else "0")
        self.metric("mem_info_vram_used", "298647552")

    def metric(self, name, value):
        (self.root / BDF / name).write_text(value + "\n", encoding="ascii")

    def observe(self, status=None, pids=None, hook=None):
        self.commands = []
        responses = [
            status or command_record(status_text()),
            pids or command_record(pid_text()),
        ]

        def run(command):
            self.commands.append(command)
            if hook:
                hook(len(self.commands))
            return responses[len(self.commands) - 1]

        return OBSERVER.observe(
            4, BDF, UID, Path("/opt/rocm/bin/rocm-smi"), root=self.root, run=run
        )

    def test_admits_complete_quiet_endpoint_with_other_gpu_attached(self):
        result = self.observe()
        self.assertTrue(result["endpoint_admitted"])
        self.assertEqual(result["reasons"], [])
        self.assertEqual(result["selected_pids"], [])
        self.assertEqual(len(result["sysfs"]), 3)
        self.assertEqual(self.commands[1][-1], "--showpidgpus")
        self.assertNotIn("--json", self.commands[1])

    def test_vram_refusal_still_collects_pid_attribution(self):
        result = self.observe(
            status=command_record(
                status_text(**{"VRAM Total Used Memory (B)": "648634368"})
            ),
            pids=command_record(pid_text("PID 55 is using 2 DRM device(s):\n4 7\n")),
        )
        self.assertEqual(len(self.commands), 2)
        self.assertEqual(result["selected_pids"], [55])
        self.assertEqual(result["reasons"], ["smi-vram", "selected-gpu-attachments"])

    def test_status_failure_never_suppresses_pid_capture(self):
        for changes in ({"exit": 1}, {"error": "timeout"}, {"stderr": "warning"}):
            with self.subTest(changes=changes):
                result = self.observe(status=command_record("unusable", **changes))
                self.assertEqual(len(self.commands), 2)
                self.assertIn("status-capture-failed", result["reasons"])
                self.assertEqual(result["selected_pids"], [])

    def test_missing_sysfs_does_not_suppress_either_cli(self):
        (self.root / BDF / "unique_id").unlink()
        result = self.observe()
        self.assertEqual(len(self.commands), 2)
        self.assertIn("unique_id", result["sysfs"][0]["errors"])
        self.assertIn("sysfs-before-invalid", result["reasons"])

    def test_all_three_sysfs_snapshots_must_pass(self):
        self.metric("mem_info_vram_used", str(OBSERVER.VRAM_LIMIT))

        def hook(_ordinal):
            self.metric("mem_info_vram_used", "0")

        result = self.observe(hook=hook)
        self.assertEqual(result["reasons"], ["sysfs-before-vram"])
        self.assertFalse(result["endpoint_admitted"])

    def test_identity_change_during_capture_is_not_rehabilitated(self):
        def hook(ordinal):
            self.metric("unique_id", "1234567890abcdef" if ordinal == 1 else UID[2:])

        result = self.observe(hook=hook)
        self.assertEqual(result["reasons"], ["sysfs-between-invalid"])

    def test_threshold_is_exclusive_and_busy_is_zero(self):
        for value, admitted in (
            (OBSERVER.VRAM_LIMIT - 1, True),
            (OBSERVER.VRAM_LIMIT, False),
        ):
            with self.subTest(value=value):
                result = self.observe(
                    status=command_record(
                        status_text(**{"VRAM Total Used Memory (B)": str(value)})
                    )
                )
                self.assertEqual(result["endpoint_admitted"], admitted)
        result = self.observe(
            status=command_record(status_text(**{"GPU use (%)": "1"}))
        )
        self.assertIn("smi-busy", result["reasons"])

    def test_invalid_status_types_numbers_and_identities_fail_closed(self):
        cases = ["{}", "null", "[]", '{"card4": {}, "card4": {}}']
        for key, value in (
            ("Unique ID", None),
            ("Unique ID", "0x1"),
            ("PCI Bus", "0000:05:00.0"),
            ("GPU use (%)", "101"),
            ("GPU use (%)", "-1"),
            ("VRAM Total Used Memory (B)", True),
            ("GPU use (%)", "00"),
        ):
            cases.append(status_text(**{key: value}))
        for text in cases:
            with self.subTest(text=text):
                result = self.observe(status=command_record(text))
                self.assertIn("status-invalid", result["reasons"])
                self.assertEqual(len(self.commands), 2)

    def test_pid_json_false_success_is_unknown_not_empty(self):
        result = self.observe(pids=command_record("WARNING: No JSON data to report\n"))
        self.assertEqual(result["reasons"], ["pids-invalid"])
        self.assertIsNone(result["selected_pids"])

    def test_invalid_pid_rosters_fail_closed(self):
        cases = [
            "",
            HEADER,
            pid_text(""),
            pid_text("PID 1 is using 1 DRM device(s):\n"),
            pid_text("PID 1 is using 2 DRM device(s):\n4\n"),
            pid_text("PID 1 is using 2 DRM device(s):\n4 4\n"),
            pid_text("PID 1 is using 1 DRM device(s):\n64\n"),
            pid_text("PID 1 is using 0 DRM device(s):\n"),
            pid_text("PID 1 is using 1 DRM device(s)\n4\n"),
            pid_text(
                "PID 1 is using 0 DRM device(s)\nPID 1 is using 0 DRM device(s)\n"
            ),
            pid_text() + "WARNING\n",
            pid_text("PID 1 is using 01 DRM device(s):\n4\n"),
        ]
        for text in cases:
            with self.subTest(text=text):
                result = self.observe(pids=command_record(text))
                self.assertFalse(result["endpoint_admitted"])
                self.assertIn("pids-invalid", result["reasons"])

    def test_unsorted_gpu_indices_and_zero_attachment_records_parse(self):
        self.assertEqual(
            OBSERVER.parse_pids(
                pid_text("PID 77 is using 8 DRM device(s):\n0 2 4 6 1 3 5 7\n")
            ),
            {77: [0, 2, 4, 6, 1, 3, 5, 7]},
        )
        self.assertEqual(
            OBSERVER.parse_pids(pid_text("PID 7 is using 0 DRM device(s)\n")), {7: []}
        )

    def test_pid_capture_failure_is_unknown(self):
        result = self.observe(pids=command_record(pid_text(), exit=1))
        self.assertIsNone(result["selected_pids"])
        self.assertEqual(result["reasons"], ["pids-capture-failed"])

    def test_oversized_and_invalid_sysfs_values_reject(self):
        for value in ("x" * 129, "-1", "NaN", "18446744073709551616"):
            with self.subTest(value=value):
                self.metric("mem_info_vram_used", value)
                result = self.observe()
                self.assertIn("sysfs-before-invalid", result["reasons"])

    def test_later_good_observation_cannot_override_first_refusal(self):
        bad, good = self.observe(pids=command_record("bad")), self.observe()
        output = io.StringIO()
        with (
            mock.patch.object(OBSERVER, "observe", side_effect=[bad, good]),
            mock.patch.object(OBSERVER.time, "sleep") as sleep,
            contextlib.redirect_stdout(output),
        ):
            status = OBSERVER.main(
                [
                    "--gpu-index",
                    "4",
                    "--pci-bdf",
                    BDF,
                    "--unique-id",
                    UID,
                    "--samples",
                    "2",
                ]
            )
        records = [json.loads(line) for line in output.getvalue().splitlines()]
        self.assertEqual(status, 1)
        self.assertEqual([record["index"] for record in records[:-1]], [0, 1])
        self.assertEqual(records[-1]["refused"], 1)
        self.assertFalse(records[-1]["all_endpoints_admitted"])
        self.assertFalse(records[-1]["performance_accepted"])
        sleep.assert_called_once_with(0.25)

    def test_cli_validates_bounds_before_sampling(self):
        for extra in (
            ["--samples", "0"],
            ["--samples", "31"],
            ["--interval-ms", "99"],
            ["--pci-bdf", "../bad"],
            ["--unique-id", "0x0000000000000000"],
            ["--gpu-index", "64"],
            ["--rocm-smi", "relative"],
        ):
            with (
                self.subTest(extra=extra),
                mock.patch.object(OBSERVER, "observe") as observe,
                contextlib.redirect_stderr(io.StringIO()),
                self.assertRaises(SystemExit) as error,
            ):
                OBSERVER.main(
                    ["--gpu-index", "4", "--pci-bdf", BDF, "--unique-id", UID] + extra
                )
            self.assertEqual(error.exception.code, 2)
            observe.assert_not_called()


class CaptureTests(unittest.TestCase):
    def test_timeout_retains_partial_output(self):
        with mock.patch.object(
            OBSERVER.subprocess,
            "run",
            side_effect=subprocess.TimeoutExpired(["x"], 30, b"partial", b"error"),
        ):
            result = OBSERVER.capture_command(["x"])
        self.assertEqual(result["stdout"], "partial")
        self.assertEqual(result["stderr"], "error")
        self.assertEqual(result["error"], "capture-timeout")
        self.assertIsNone(result["exit"])

    def test_visibility_removed_without_mutating_parent_environment(self):
        completed = subprocess.CompletedProcess(["x"], 0, b"ok", b"")
        with mock.patch.dict(os.environ, {name: "4" for name in OBSERVER.VISIBILITY}):
            before = copy.deepcopy(dict(os.environ))
            with mock.patch.object(
                OBSERVER.subprocess, "run", return_value=completed
            ) as run:
                OBSERVER.capture_command(["x"])
            self.assertEqual(before, dict(os.environ))
            self.assertFalse(
                set(OBSERVER.VISIBILITY) & run.call_args.kwargs["env"].keys()
            )
            self.assertEqual(run.call_args.kwargs["timeout"], 30)

    def test_launch_failure_is_recorded(self):
        with mock.patch.object(
            OBSERVER.subprocess, "run", side_effect=FileNotFoundError("missing")
        ):
            result = OBSERVER.capture_command(["x"])
        self.assertIsNone(result["exit"])
        self.assertEqual(result["error"], "missing")

    def test_truncation_and_non_ascii_cannot_admit(self):
        for output, reason in (
            (b"x" * (OBSERVER.MAX_OUTPUT + 1), "oversized-output-truncated"),
            (b"\xff", "non-ASCII-output"),
        ):
            with (
                self.subTest(reason=reason),
                mock.patch.object(
                    OBSERVER.subprocess,
                    "run",
                    return_value=subprocess.CompletedProcess(["x"], 0, output, b""),
                ),
            ):
                result = OBSERVER.capture_command(["x"])
            self.assertEqual(result["error"], reason)
            self.assertLessEqual(len(result["stdout"]), OBSERVER.MAX_OUTPUT)


if __name__ == "__main__":
    unittest.main()
