#!/usr/bin/env python3
"""CPU-only forwarding controller controls; synthetic reports are not native evidence."""

import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock

SPEC = importlib.util.spec_from_file_location("forward_window_smoke", Path(__file__).with_name("forward_window_smoke.py"))
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)
DEVICES = ("1,0000:26:00.0,0x0000000000000001", "6,0000:c6:00.0,0x0000000000000006",
           "7,0000:e5:00.0,0x0000000000000007")
SELECTED = M.endpoints(DEVICES)


def line(fields):
    return "PASS " + " ".join(f"{key}={value}" for key, value in fields.items()) + "\n"


class FakeRecorder(M.S.Recorder):
    def __init__(self, output, mutate=None):
        super().__init__(output)
        self.calls = []
        self.mutate = mutate

    def run(self, name, argv, seconds, **kwargs):
        self.calls.append((name, argv, seconds, kwargs))
        self.commands.append(name)
        folder = self.output / name
        folder.mkdir()
        devices = SELECTED if argv[2] == SELECTED[0][2] else tuple(reversed(SELECTED))
        output = line(M.expected(devices, argv[-1], argv[1].startswith("--late-"), argv[1].endswith("-segments")))
        receipt = dict(exit=0, error=None, group_absent=True)
        if self.mutate:
            output, receipt = self.mutate(output, receipt)
        (folder / "stdout").write_text(output)
        (folder / "stderr").write_text("")
        return receipt


class ControllerTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.binary = self.root / M.EXAMPLE
        self.binary.write_bytes(b"\x7fELF\x02\x01" + b"\0" * 12 + b"\x3e\x00" + b"synthetic test")
        self.binary.chmod(0o700)
        self.fd = os.open(self.binary, os.O_RDONLY)
        self.addCleanup(os.close, self.fd)
        self.identity = M.S.fingerprint(self.fd)

    def recorder(self, mutate=None):
        path = self.root / str(len(tuple(self.root.iterdir())))
        path.mkdir()
        return FakeRecorder(path, mutate)

    def run_campaign(self, recorder):
        return M.campaign(recorder, None, SELECTED, self.binary, self.fd, self.identity, Path("/smi"))

    def test_three_distinct_exact_devices_required(self):
        self.assertEqual(len(SELECTED), 3)
        for values in (DEVICES[:2], (*DEVICES, DEVICES[0]), (DEVICES[0], DEVICES[1], DEVICES[1]),
                       (*DEVICES[:2], "7,0000:c6:00.0,0x0000000000000007"),
                       (*DEVICES[:2], "7,0000:e5:00.0,0x0000000000000001")):
            with self.subTest(values=values), self.assertRaises(ValueError):
                M.endpoints(values)

    def test_oracle_extents_packets_and_changed_rounds(self):
        hashes = {
            "overlap": ("fe1d671a0476fe3ff746d208cc805e73a8b5b2e2e35581c5ea75afd1b3ca5ab4",
                        "07e594f11eac7fb2d948a2df3cc71e2e853c3f3761c7875d877d829dd88a79e9"),
            "packets": ("d6f76388c758f683af972bc9f1118370776d1a6b10dd6b906e9c027a64ffda31",
                        "48c64cbdb5bbac355eda3fbbd0032e787e644fd7c0f13f7389a18413513818b9"),
        }
        for shape in ("overlap", "packets"):
            lengths, envelopes, copied, packets, digests = M.oracle(shape)
            self.assertEqual(lengths[2] - lengths[1], 173)
            self.assertEqual(lengths[1] - lengths[0], 128)
            self.assertGreater(lengths[1] - 7, envelopes[1] + 53)
            self.assertNotEqual(digests[0], digests[1])
            self.assertEqual(digests, hashes[shape])
            self.assertEqual(packets, (2, 2, 1) if shape == "overlap" else (3, 3, 3))
            if shape == "overlap":
                self.assertEqual(lengths, (65729, 65857, 66030))
                self.assertEqual(copied, (12290, 12294))
            fields = M.expected(SELECTED, shape, False)
            self.assertEqual(int(fields["host_bytes"]), lengths[2] + 42)
            self.assertEqual(fields["readback_bytes"], fields["target_bytes"])
        with self.assertRaises(ValueError):
            M.oracle("unknown")

    def test_exact_reports_reject_every_missing_or_changed_field(self):
        wanted = M.expected(SELECTED, "overlap", True)
        self.assertEqual(M.S.parse_pass(line(wanted), wanted), wanted)
        for key in wanted:
            missing = dict(wanted)
            del missing[key]
            for output in (line(missing), line({**wanted, key: "wrong"})):
                with self.subTest(key=key), self.assertRaises(ValueError):
                    M.S.parse_pass(output, wanted)

    def test_eight_cases_exact_routes_fd_and_cleanup(self):
        recorder = self.recorder()
        with mock.patch.object(M.S, "observe_pair") as observe:
            report = self.run_campaign(recorder)
        self.assertTrue(report["accepted"])
        self.assertEqual(len(recorder.calls), 8)
        self.assertEqual(observe.call_count, 16)
        for case, call in zip(M.CASES, recorder.calls):
            name, shape, late, reverse = case
            selected = tuple(reversed(SELECTED)) if reverse else SELECTED
            self.assertEqual(call, (name, [str(self.binary), "--late-forward-window" if late else "--forward-window",
                                          *(row[2] for row in selected), shape], 180,
                                   dict(executable=f"/proc/self/fd/{self.fd}", pass_fds=(self.fd,))))
        self.assertTrue(all(row["accepted"] and row["cleanup_observed"] for row in report["cases"]))
        self.assertTrue(json.loads((recorder.output / "result.json").read_text())["accepted"])

    def test_failure_stops_roster_and_observes_cleanup(self):
        for mutation in (lambda out, row: (out, {**row, "exit": 1}),
                         lambda out, row: (out.replace("native_counter=0,3,6", "native_counter=0,2,4"), row),
                         lambda out, row: (out, {**row, "group_absent": False}),
                         lambda _out, row: ("", row)):
            recorder = self.recorder(mutation)
            with mock.patch.object(M.S, "observe_pair") as observe:
                report = self.run_campaign(recorder)
            self.assertFalse(report["accepted"])
            self.assertEqual(len(recorder.calls), 1)
            self.assertEqual(observe.call_count, 2)

    def test_admission_and_post_observation_fail_closed(self):
        for side_effect, commands in (([ValueError("busy")], 0), ([None, ValueError("uncertain cleanup")], 1)):
            recorder = self.recorder()
            with mock.patch.object(M.S, "observe_pair", side_effect=side_effect):
                report = self.run_campaign(recorder)
            self.assertFalse(report["accepted"])
            self.assertEqual(len(recorder.calls), commands)
            self.assertFalse(report["cases"][0]["cleanup_observed"])

    def test_changed_executable_rejects_before_gpu_observation(self):
        self.binary.write_bytes(self.binary.read_bytes() + b"changed")
        with mock.patch.object(M.S, "observe_pair") as observe:
            report = self.run_campaign(self.recorder())
        self.assertFalse(report["accepted"])
        observe.assert_not_called()

    def test_segment_profile_routes_counts_oracle_and_old_schema_rejection(self):
        recorder = self.recorder()
        with mock.patch.object(M.S, "observe_pair") as observe:
            report = M.campaign(recorder, None, SELECTED, self.binary, self.fd, self.identity, Path("/smi"), True)
        self.assertTrue(report["accepted"])
        self.assertEqual(report["schema"], "fe2o3.forward-segments-smoke.v1")
        self.assertEqual(len(recorder.calls), 8)
        self.assertEqual(observe.call_count, 16)
        for (_, shape, late, reverse), call in zip(M.CASES, recorder.calls):
            self.assertEqual(call[1][1], "--late-forward-segments" if late else "--forward-segments")
            selected = tuple(reversed(SELECTED)) if reverse else SELECTED
            self.assertEqual(call[1][2:5], [row[2] for row in selected])
            wanted = M.expected(selected, shape, late, True)
            self.assertEqual(wanted["lists"], "6")
            self.assertEqual(wanted["scalar_peers"], "0")
            self.assertEqual(wanted["peer_descriptors"], "4")
            self.assertEqual(wanted["packets_per_round"], "3,3,6" if shape == "packets" else "2,2,4")
            self.assertEqual(int(wanted["peer_copied_bytes"]), int(wanted["peer_bytes"]) + 24)
            self.assertGreater(int(wanted["peer_target_envelope"]), int(wanted["peer_bytes"]))
            old = M.expected(selected, shape, late)
            self.assertNotEqual(wanted["round_sha256"], old["round_sha256"])
            with self.assertRaises(ValueError):
                M.S.parse_pass(line(old), wanted)
            for key in wanted:
                bad = dict(wanted)
                del bad[key]
                with self.subTest(key=key), self.assertRaises(ValueError):
                    M.S.parse_pass(line(bad), wanted)


if __name__ == "__main__":
    unittest.main()
