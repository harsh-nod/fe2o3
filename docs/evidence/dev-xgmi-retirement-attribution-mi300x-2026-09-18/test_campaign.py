#!/usr/bin/env python3
"""CPU protocol calibration with synthetic records; no device access."""

import sys

if not sys.flags.isolated:
    raise RuntimeError("run with python3 -I")

from collections import Counter
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent


def module(name):
    spec = importlib.util.spec_from_file_location(
        "calibrated_" + name, HERE / (name + ".py")
    )
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


C, V = module("campaign"), module("verify")
SELECTED = C.devices([1, 2])


def line(row):
    return " ".join(key + "=" + str(value) for key, value in row.items())


def aggregates(enabled):
    result = []
    for measurement in ("remap-per-round", "persistent-hot"):
        row = C.B.expected_fields("kfd", 1, measurement)
        if enabled:
            row["diagnostic"] = "xgmi-host-stages-v1"
        for direction in ("forward", "reverse"):
            row.update(
                {
                    direction + "_p50_ns": "1000000",
                    direction + "_p95_ns": "2000000",
                    direction + "_p50_GBps": "1.049",
                }
            )
        result.append(line(row))
    return result


def transcript(enabled=True, pending=()):
    result = []
    if enabled:
        for identity in range(7, 169):
            direction = (identity - 7) % 2
            for call in [
                "submit",
                *(["pending"] * pending.count(identity)),
                "completed",
            ]:
                result.append(
                    line(
                        {
                            "schema": "fe2o3.xgmi-host-attribution.v1",
                            "backend": "kfd",
                            "ordinal": len(result),
                            "backend_submission": identity,
                            "source_uid": SELECTED[direction][2][2:],
                            "destination_uid": SELECTED[1 - direction][2][2:],
                            "call": call,
                            "opening_currentness_ns": 10,
                            "preparation_ns": 2
                            if call == "submit"
                            else "not-applicable",
                            "native_call_ns": 3,
                            "closing_currentness_ns": 11,
                            "total_ns": 30,
                            "authority": "none",
                            "teardown": "explicit",
                        }
                    )
                )
    return ("\n".join(result + aggregates(enabled)) + "\n").encode()


class Calibration(unittest.TestCase):
    def reject(self, value, enabled=True):
        with self.assertRaises((RuntimeError, ValueError, KeyError, UnicodeError)):
            C.parse_transcript(value, SELECTED, enabled)

    def test_exact_off_and_on_rosters(self):
        self.assertEqual(
            len(C.parse_transcript(transcript(False), SELECTED, False)["observations"]),
            0,
        )
        result = C.parse_transcript(transcript(), SELECTED, True)
        self.assertEqual(len(result["observations"]), 324)
        self.assertEqual(
            Counter(r["call"] for r in result["observations"]),
            {"submit": 162, "completed": 162},
        )

    def test_pending_groups_and_population_summaries(self):
        parsed = C.parse_transcript(transcript(pending=(7, 109, 109)), SELECTED, True)
        self.assertEqual(len(parsed["observations"]), 327)
        summary = C.summarize(parsed)
        self.assertEqual(len(summary), 10)
        self.assertEqual(
            summary["prime/forward"]["submissions"]["all_calls_total_ns"],
            {"count": 1, "value": 60},
        )
        measured = summary["hot-sample/forward"]
        self.assertEqual(measured["submissions"]["pending_count"]["count"], 30)
        self.assertEqual(measured["submissions"]["pending_count"]["sum"], 2)
        self.assertEqual(
            measured["calls"]["pending"]["total_ns"],
            {"count": 2, "p50": 30, "p95": 30, "sum": 60},
        )
        self.assertEqual(
            C.statistics(list(range(30, 0, -1))),
            {"count": 30, "p50": 15, "p95": 29, "sum": 465},
        )

    def test_exact_population_boundaries(self):
        expected = {
            "remap-warmup": 20,
            "remap-sample": 60,
            "prime": 2,
            "hot-warmup": 20,
            "hot-sample": 60,
        }
        self.assertEqual(Counter(C.population(i)[0] for i in range(7, 169)), expected)
        for identity in (7, 26, 27, 86, 87, 88, 89, 108, 109, 168):
            self.assertEqual(
                C.population(identity)[1], "forward" if identity % 2 else "reverse"
            )
        for invalid in (6, 169, True):
            with self.assertRaises(RuntimeError):
                C.population(invalid)

    def test_identity_ordinal_and_direction_substitution(self):
        original = transcript()
        for old, new in (
            (b"ordinal=0 ", b"ordinal=1 "),
            (b"backend_submission=7 ", b"backend_submission=8 "),
            (b"source_uid=ab83d2ffef0d3cdf", b"source_uid=d2e26fef80cf5c33"),
        ):
            self.reject(original.replace(old, new, 1))

    def test_missing_reordered_and_extra_calls(self):
        original = transcript()
        self.reject(original.replace(b"call=submit", b"call=pending", 1))
        self.reject(original.replace(b"call=completed", b"call=submit", 1))
        self.reject(b"\n".join(original.splitlines()[1:]) + b"\n")
        self.reject(original.replace(b"call=completed", b"call=pending", 1))

    def test_duplicate_unknown_and_missing_fields(self):
        original = transcript()
        self.reject(original.replace(b"backend=kfd ", b"backend=kfd backend=kfd ", 1))
        self.reject(original.replace(b"backend=kfd ", b"backend=kfd extra=value ", 1))
        self.reject(original.replace(b"authority=none ", b"", 1))

    def test_duration_bounds_stage_sum_and_preparation(self):
        original = transcript()
        for old, new in (
            (b"total_ns=30", b"total_ns=1"),
            (b"total_ns=30", b"total_ns=18446744073709551616"),
            (b"opening_currentness_ns=10", b"opening_currentness_ns=01"),
            (b"native_call_ns=3", b"native_call_ns=unavailable"),
            (b"preparation_ns=2", b"preparation_ns=not-applicable"),
            (b"preparation_ns=not-applicable", b"preparation_ns=0"),
        ):
            self.reject(original.replace(old, new, 1))

    def test_complete_ascii_transcript_and_mode(self):
        original = transcript()
        for value in (
            original[:-1],
            original + b"\n",
            original.replace(b"\n", b"\r\n", 1),
            original + b"\0",
            b"\xff" + original,
        ):
            self.reject(value)
        self.reject(original, False)
        self.reject(transcript(False), True)
        self.reject(original.replace(b" diagnostic=xgmi-host-stages-v1", b"", 1))

    def test_aggregate_controls_and_bandwidth(self):
        original = transcript(False)
        for old, new in (
            (b"samples=30", b"samples=31"),
            (b"forward_p95_ns=2000000", b"forward_p95_ns=1"),
            (b"forward_p50_GBps=1.049", b"forward_p50_GBps=NaN"),
            (b"forward_p50_GBps=1.049", b"forward_p50_GBps=2.049"),
            (b"measurement=remap-per-round", b"measurement=persistent-hot"),
        ):
            self.reject(original.replace(old, new, 1), False)

    def test_device_admission_and_command_roster(self):
        for indices in ([1, 1], [1], [True, 2], [-1, 2], [1, 8]):
            with self.assertRaises(RuntimeError):
                C.devices(indices)
        owned = Path(C.PREFIX + "1" * 16)
        specs = C.remote_commands(owned, SELECTED)
        self.assertEqual(len(specs), 33)
        self.assertEqual(len({name for name, _, _ in specs}), 33)
        commands = {name: command for name, command, _ in specs}
        self.assertEqual(commands["off1"], commands["off2"])
        self.assertEqual(commands["on1"], commands["off1"] + ["--diagnose-xgmi"])
        self.assertEqual(commands["on1"], commands["on2"])
        self.assertIn("hardware-diagnostic", commands["build"])

    def test_original_postflight_failure_is_preserved(self):
        original = RuntimeError("workload")
        calls, sleeps = [], []

        def observe(name):
            calls.append(name)
            raise RuntimeError(name)

        self.assertIs(
            C.B.settled_postflight(observe, "run", original, sleep=sleeps.append),
            original,
        )
        self.assertEqual(sleeps, [2, 20])
        self.assertEqual(calls, ["run-settled", "run-delayed"])

    def test_recorded_paths_are_independent_of_verifier_checkout(self):
        marker = {
            "path": C.PREFIX + "1" * 16,
            "commit": "2" * 40,
            "binding_sha256": "3" * 64,
        }
        payload = Path("/payload")
        execution = Path("/recorded-execution")
        original = C.local_commands(payload, marker)
        relocated = C.local_commands(payload, marker, execution_root=execution)
        changed = {"calibration", "cpu-verify", "source", "collect"}
        self.assertEqual(original.keys(), relocated.keys())
        for name in original:
            command, seconds, stdin = original[name]
            expected = [
                str(execution) + arg[len(str(C.ROOT)) :]
                if arg.startswith(str(C.ROOT) + "/")
                else arg
                for arg in command
            ]
            self.assertEqual(relocated[name], (expected, seconds, stdin))
            self.assertEqual(original[name] != relocated[name], name in changed)
        self.assertTrue(C.CPU.is_relative_to(C.ROOT))
        self.assertTrue(C.SELECTOR.is_relative_to(C.ROOT))

    def test_cleanup_requires_complete_collection(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "inventory").mkdir()
            (root / "inventory/stdout").write_text(json.dumps({"file": "0" * 64}))
            (root / "remote").mkdir()
            (root / "remote/file").write_text("mismatch")
            calls, state = [], {}

            def step(name):
                calls.append(name)
                return root / "inventory"

            with mock.patch.object(C, "HERE", root), self.assertRaises(RuntimeError):
                C.collect_and_clean(step, state)
            self.assertEqual(calls, ["remote-inventory", "collect"])
            self.assertEqual(state, {})

    def test_cleanup_order_and_absence_are_distinct(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "inventory").mkdir()
            (root / "remote").mkdir()
            (root / "remote/file").write_text("collected")
            (root / "inventory/stdout").write_text(
                json.dumps(C.B.inventory(root / "remote"))
            )
            calls, state = [], {}

            def step(name):
                calls.append(name)
                return root / "inventory"

            with mock.patch.object(C, "HERE", root):
                C.collect_and_clean(step, state)
            self.assertEqual(
                calls, ["remote-inventory", "collect", "cleanup", "absence"]
            )
            self.assertEqual(
                state, {"collected": True, "cleaned": True, "absence": True}
            )

    def test_receipt_types_chronology_and_digest(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for stream in ("stdout", "stderr"):
                (root / stream).write_bytes(b"")
            row = {
                "command": ["test"],
                "cwd": "/work",
                "started_ns": 20,
                "finished_ns": 30,
                "timeout_seconds": 5,
                "pid": 100,
                "exit": 0,
                "error": None,
                "group_absent": True,
                "environment": None,
                "stdin_sha256": None,
                "stdout_sha256": hashlib.sha256(b"").hexdigest(),
                "stderr_sha256": hashlib.sha256(b"").hexdigest(),
            }
            (root / "receipt.json").write_text(json.dumps(row))
            self.assertEqual(V.receipt(root, ["test"], 5, Path("/work"), 10), row)
            for key, value in (
                ("exit", False),
                ("started_ns", 9),
                ("finished_ns", 20),
                ("stdout_sha256", "0" * 64),
                ("group_absent", 1),
                ("timeout_seconds", True),
            ):
                (root / "receipt.json").write_text(json.dumps({**row, key: value}))
                with self.assertRaises(RuntimeError):
                    V.receipt(root, ["test"], 5, Path("/work"), 10)

    def test_tree_rejects_extra_directories_and_symlinks(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "file").write_text("content")
            V.exact_tree(root, {"file"})
            (root / "empty").mkdir()
            with self.assertRaises(RuntimeError):
                V.exact_tree(root, {"file"})
            (root / "empty").rmdir()
            (root / "file").unlink()
            (root / "file").symlink_to("/dev/null")
            with self.assertRaises(RuntimeError):
                V.exact_tree(root, {"file"})

    def test_settled_and_delayed_lower_bounds(self):
        rows = {
            "on1": {"finished_ns": 10**9},
            "on1-settled-gpu1": {"started_ns": 3 * 10**9},
            "on1-settled-gpu2": {"finished_ns": 5 * 10**9},
            "on1-delayed-gpu1": {"started_ns": 25 * 10**9},
        }
        V.postflight_timing(rows, "on1", SELECTED)
        for key in ("on1-settled-gpu1", "on1-delayed-gpu1"):
            rows[key]["started_ns"] -= 1
            with self.assertRaises(RuntimeError):
                V.postflight_timing(rows, "on1", SELECTED)
            rows[key]["started_ns"] += 1

    def test_helper_authenticated_before_loading(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "helper.py"
            path.write_text("raise AssertionError('untrusted')\n")
            with (
                mock.patch.object(
                    C.importlib.util, "spec_from_file_location"
                ) as loader,
                self.assertRaises(RuntimeError),
            ):
                C.load_pinned(path, "0" * 64, "bad")
            loader.assert_not_called()
            with (
                mock.patch.object(
                    V.importlib.util, "spec_from_file_location"
                ) as loader,
                self.assertRaises(RuntimeError),
            ):
                V.authenticated_campaign(path)
            loader.assert_not_called()

    def test_native_failure_wins_settlement_and_finalization(self):
        original = RuntimeError("native failure")
        for settlement in (None, OSError("collect failed")):
            state = {"secondary_failures": []}

            def step(_name):
                raise original

            with (
                mock.patch.object(C, "collect_and_clean", side_effect=settlement),
                self.assertRaises(RuntimeError) as caught,
            ):
                C.run_and_settle(step, state)
            self.assertIs(caught.exception, original)
            self.assertEqual(
                len(state["secondary_failures"]), int(settlement is not None)
            )
        state = {"absence": True, "secondary_failures": [], "failure": str(original)}
        with (
            mock.patch.object(
                C.shutil, "rmtree", side_effect=OSError("payload cleanup")
            ),
            mock.patch.object(C.B, "write_json", side_effect=OSError("state write")),
            mock.patch.object(C.sys, "stderr", io.StringIO()),
        ):
            self.assertIs(C.finalize_local(Path("/unused"), state, original), original)
        self.assertEqual(
            [row["stage"] for row in state["secondary_failures"]],
            ["local-payload-cleanup", "state-finalization"],
        )

    def test_static_inputs_must_match_commit_tree(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            packet = root / "packet"
            packet.mkdir()
            for name in C.STATIC_INPUTS:
                (packet / name).write_bytes(b"committed")
            with (
                mock.patch.object(C, "ROOT", root),
                mock.patch.object(C, "HERE", packet),
                mock.patch.object(
                    C.subprocess, "check_output", return_value=b"committed"
                ),
            ):
                self.assertEqual(set(C.committed_tools("1" * 40)), set(C.STATIC_INPUTS))
                (packet / "campaign.py").write_bytes(b"dirty")
                with self.assertRaises(RuntimeError):
                    C.committed_tools("1" * 40)

    def test_json_duplicates_and_nonfinite_are_rejected(self):
        for value in ('{"key": 1, "key": 2}', '{"key": NaN}', '{"key": Infinity}'):
            with self.assertRaises(RuntimeError):
                C.parse_json(value)
        self.assertFalse(C.same_json({"count": True}, {"count": 1}))
        self.assertFalse(C.same_json({"count": 1.0}, {"count": 1}))


if __name__ == "__main__":
    unittest.main()
