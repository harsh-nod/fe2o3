#!/usr/bin/env python3
"""Prelaunch CPU calibration of the two-source campaign; no device access."""

import sys

if not sys.flags.isolated:
    raise RuntimeError("run with python3 -I")

import copy
import gzip
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
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


C, V = module("campaign"), module("verify")
P, B = C.P, C.B
OLD = P.load_pinned(
    P.ROOT / (P.PRIOR + "test_campaign.py"), C.PARSER_TEST_SHA, "prior_fixtures"
)
SELECTED = P.devices([1, 2])
OWNED = Path(P.PREFIX + "1" * 16)


def state():
    return {
        **{
            k: False
            for k in (
                "create_attempted",
                "created",
                "native_attempted",
                "native_success",
                "collected",
                "cleaned",
                "absence",
                "local_payload_absent",
            )
        },
        "secondary_failures": [],
        "failure": None,
    }


class Calibration(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.cohorts = {}
        for name, spec in P.COHORTS.items():
            value, data = P.materialize(P.ROOT, spec["commit"], P.cpu_source(name))
            assert len(data) == spec["tar_bytes"]
            assert value["tar_sha256"] == spec["tar_sha256"]
            cls.cohorts[name] = {"cohort": name, "prerequisite": spec, **value}
        cls.binding = {
            "schema": "fe2o3.xgmi-pair-source-comparison.v1",
            "commit": "1" * 40,
            "tools": {n: "2" * 64 for n in P.STATIC_INPUTS},
            "helpers": {"attribution.py": P.ATTRIBUTION_SHA, "base.py": P.BASE_SHA},
            "devices": SELECTED,
            "plan": P.PLAN,
            "cohorts": cls.cohorts,
            "source_difference": P.source_difference(cls.cohorts),
            "payload": {n: "2" * 64 for n in P.PAYLOAD},
        }
        cls.binding["payload"].update(cls.binding["helpers"])
        cls.binding["payload"].update(
            {"source-" + n + ".tar.gz": c["tar_sha256"] for n, c in cls.cohorts.items()}
        )

    def test_real_signed_cohorts_and_binding(self):
        P.validate_binding(self.binding)
        for name, spec in P.COHORTS.items():
            V.verify_signature(spec["commit"])
            self.assertEqual(self.cohorts[name]["tree"], spec["tree"])
            self.assertEqual(
                P.json_digest(self.cohorts[name]["source_modes"]), spec["modes_sha256"]
            )

    def test_binding_substitution(self):
        for path, value in (
            (("schema",), "other"),
            (("plan", "samples"), True),
            (("cohorts", "candidate", "cohort"), "baseline"),
            (("cohorts", "candidate", "tree"), "0" * 40),
            (("cohorts", "candidate", "tar_sha256"), "0" * 64),
            (("payload", "campaign.py"), "0" * 64),
            (("devices",), P.devices([2, 3])),
        ):
            value_copy = copy.deepcopy(self.binding)
            parent = value_copy
            for key in path[:-1]:
                parent = parent[key]
            parent[path[-1]] = value
            if path == ("devices",):
                P.validate_binding(
                    value_copy
                )  # Selection is free but exact identities are bound.
            else:
                with self.subTest(path=path), self.assertRaises(RuntimeError):
                    P.validate_binding(value_copy)

    def test_binding_file_mode_and_plan_rosters(self):
        for kind in ("files", "modes", "plan", "tools", "swap"):
            value = copy.deepcopy(self.binding)
            if kind in ("files", "modes"):
                values = value["cohorts"]["candidate"]["source_" + kind]
                values.pop(next(iter(values)))
            elif kind == "swap":
                value["cohorts"]["candidate"], value["cohorts"]["baseline"] = (
                    value["cohorts"]["baseline"],
                    value["cohorts"]["candidate"],
                )
            elif kind == "plan":
                value["plan"]["order"].reverse()
            else:
                value["tools"]["extra"] = "0" * 64
            with self.subTest(kind=kind), self.assertRaises(RuntimeError):
                P.validate_binding(value)

    def test_exact_source_difference(self):
        self.assertEqual(P.source_difference(self.cohorts)["added"], sorted(P.ADDED))
        for kind in ("addition", "changed", "mode"):
            values = copy.deepcopy(self.cohorts)
            if kind == "addition":
                values["candidate"]["source_files"]["unexpected"] = "0" * 64
            elif kind == "changed":
                name = next(iter(P.CHANGED))
                values["candidate"]["source_files"][name] = values["baseline"][
                    "source_files"
                ][name]
            else:
                values["candidate"]["source_modes"][next(iter(P.ADDED))] = "100755"
            with self.assertRaises(RuntimeError):
                P.source_difference(values)

    def test_canonical_paths(self):
        for name in ("", "/x", "a//b", "../x", "a/./b", "a\\b", "a\nx", "x" * 4097):
            with self.subTest(name=name), self.assertRaises(RuntimeError):
                P.canonical_path(name)
        P.canonical_path("crates/ordinary.rs")

    def test_tree_framing_types_and_roster(self):
        valid = b"100644 blob " + b"1" * 40 + b"\ta\0"
        self.assertEqual(P.tree_entries(valid, {"a": "2" * 64})["a"]["mode"], "100644")
        for data in (
            valid[:-1],
            valid + valid,
            valid.replace(b"100644", b"120000"),
            valid.replace(b"blob", b"tree"),
            valid.replace(b"\ta", b"\t../a"),
            valid.replace(b"\ta", b"\tb"),
        ):
            with self.assertRaises((RuntimeError, ValueError)):
                P.tree_entries(data, {"a": "2" * 64})
        with mock.patch.object(P, "MAX_TREE_BYTES", 1), self.assertRaises(RuntimeError):
            P.tree_entries(valid, {"a": "2" * 64})

    def test_blob_size_framing_identity_and_digest(self):
        entries, files = (
            {"a": {"oid": "1" * 40}},
            {"a": hashlib.sha256(b"body").hexdigest()},
        )
        valid = b"1" * 40 + b" blob 4\nbody\n"
        self.assertEqual(P.blob_contents(valid, entries, files), {"a": b"body"})
        for data in (
            valid[:-1],
            valid + b"extra",
            valid.replace(b"blob", b"tree"),
            valid.replace(b"body", b"bad!"),
            valid.replace(b" 4", b" 04"),
            valid.replace(b" 4", b" 5"),
            valid.replace(b"1", b"2", 1),
        ):
            with self.assertRaises(RuntimeError):
                P.blob_contents(data, entries, files)
        with (
            mock.patch.object(P, "MAX_BATCH_BYTES", 1),
            self.assertRaises(RuntimeError),
        ):
            P.blob_contents(valid, entries, files)

    def test_materialization_reads_commit_not_dirty_tree(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            P.git(root, ["-c", "init.defaultBranch=calibration", "init", "-q"])
            (root / "Cargo.toml").write_bytes(b"signed\n")
            (root / ".gitattributes").write_text("Cargo.toml export-ignore\n")
            P.git(root, ["add", "."])
            P.git(
                root,
                [
                    "-c",
                    "user.name=Calibration",
                    "-c",
                    "user.email=calibration@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                    "commit",
                    "-qm",
                    "fixture",
                ],
            )
            commit = P.git(root, ["rev-parse", "HEAD"]).decode().strip()
            (root / "Cargo.toml").write_bytes(b"dirty\n")
            (root / "Cargo.toml").chmod(0o755)
            expected = {"Cargo.toml": hashlib.sha256(b"signed\n").hexdigest()}
            with mock.patch.object(P, "SOURCE_PATHS", ["Cargo.toml", ".cargo"]):
                value, data = P.materialize(root, commit, expected)
            self.assertEqual(value["source_modes"], {"Cargo.toml": "100644"})
            P.validate_transport(data, expected, value["source_modes"])

    def test_transport_determinism_metadata_and_bounds(self):
        content, modes = (
            {"a": b"data", "b/" + "c" * 110: b"long"},
            {"a": "100755", "b/" + "c" * 110: "100644"},
        )
        files = {n: hashlib.sha256(v).hexdigest() for n, v in content.items()}
        data = P.transport(content, modes)
        self.assertEqual(
            data, P.transport(dict(reversed(list(content.items()))), modes)
        )
        P.validate_transport(data, files, modes)
        for field in ("MAX_TRANSPORT_BYTES", "MAX_ARCHIVE_BYTES"):
            with mock.patch.object(P, field, 1), self.assertRaises(RuntimeError):
                P.validate_transport(data, files, modes)
        with self.assertRaises(RuntimeError):
            P.validate_transport(data, files, {**modes, "a": "100644"})
        raw = bytearray(gzip.decompress(data))
        raw[512] ^= 1
        with self.assertRaises(RuntimeError):
            P.validate_transport(gzip.compress(raw), files, modes)

    def test_transport_rejects_unsafe_members(self):
        for kind in ("symlink", "duplicate", "path", "metadata", "override"):
            output = io.BytesIO()
            with P.tarfile.open(
                fileobj=output, mode="w", format=P.tarfile.PAX_FORMAT
            ) as archive:
                item = P.tarfile.TarInfo("../a" if kind == "path" else "a")
                item.size, item.mode = 1, 0o644
                if kind == "symlink":
                    item.type, item.linkname = P.tarfile.SYMTYPE, "b"
                if kind == "metadata":
                    item.uid = 1
                if kind == "override":
                    item.pax_headers = {"mtime": "1"}
                archive.addfile(item, io.BytesIO(b"x"))
                if kind == "duplicate":
                    archive.addfile(item, io.BytesIO(b"x"))
            with self.subTest(kind=kind), self.assertRaises(RuntimeError):
                P.validate_transport(
                    gzip.compress(output.getvalue()),
                    {"a": hashlib.sha256(b"x").hexdigest()},
                    {"a": "100644"},
                )

    def test_command_roster_and_cohort_separation(self):
        specs = P.remote_specs(OWNED, SELECTED)
        self.assertEqual(len(specs), 62)
        self.assertEqual(len({r[0] for r in specs}), 62)
        self.assertGreater(P.native_timeout(), sum(r[2] for r in specs) + 8 * 22)
        for phase in P.PHASES:
            label, cohort, enabled = phase
            rows = P.phase_specs(OWNED, SELECTED, phase)
            self.assertEqual(len(rows), 7)
            self.assertEqual(rows[2][0], label)
            self.assertEqual(rows[2][1][0], str(OWNED / P.binary(cohort)))
            self.assertEqual("--diagnose-xgmi" in rows[2][1], enabled)
            for _, _, _, cwd, env in rows:
                self.assertEqual(cwd, OWNED / ("source-" + cohort))
                self.assertEqual(
                    env["CARGO_TARGET_DIR"], str(OWNED / ("target-" + cohort))
                )
        builds = P.build_specs(OWNED)[-2:]
        self.assertEqual(builds[0][1:3], builds[1][1:3])
        self.assertIn("hardware-diagnostic", builds[0][1])
        self.assertNotEqual(builds[0][3:], builds[1][3:])

    def test_duplicate_command_names_rejected(self):
        with (
            mock.patch.object(P, "PHASES", [P.PHASES[0], P.PHASES[0]]),
            self.assertRaises(RuntimeError),
        ):
            P.remote_specs(OWNED, SELECTED)

    def test_local_recorded_path_relocation(self):
        marker = {"path": str(OWNED), "commit": "2" * 40, "binding_sha256": "3" * 64}
        ordinary = C.local_specs(Path("/payload"), marker)
        relocated = C.local_specs(
            Path("/payload"), marker, execution_root=Path("/relocated")
        )
        self.assertEqual(len(ordinary), 23)
        for name, (command, seconds, stdin) in ordinary.items():
            expected = [
                "/relocated" + arg[len(str(P.ROOT)) :]
                if arg.startswith(str(P.ROOT) + "/")
                else arg
                for arg in command
            ]
            self.assertEqual(relocated[name], (expected, seconds, stdin))

    def test_authenticated_helpers_never_execute_replacements(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "helper.py"
            path.write_text("raise AssertionError('executed')\n")
            with (
                mock.patch.object(
                    P.importlib.util, "spec_from_file_location"
                ) as loader,
                self.assertRaises(RuntimeError),
            ):
                P.load_pinned(path, "0" * 64, "wrong")
            loader.assert_not_called()

    def test_each_phase_failure_still_observes_both_endpoints(self):
        phase = P.PHASES[0]
        rows = P.phase_specs(OWNED, SELECTED, phase)
        specs = {n: (cmd, timeout, cwd, env) for n, cmd, timeout, cwd, env in rows}
        order = [r[0] for r in rows]
        with tempfile.TemporaryDirectory() as temp:
            folder = Path(temp)
            (folder / "stdout").write_bytes(OLD.transcript())
            (folder / "stderr").write_bytes(b"")
            for fail_at in (None, *order):
                calls, sleeps, results = [], [], {}
                original = RuntimeError(str(fail_at))

                def run(name, *_args, **_kwargs):
                    calls.append(name)
                    if name == fail_at:
                        raise original
                    return folder

                real = B.settled_postflight
                rec = mock.Mock(run=run)
                with mock.patch.object(
                    B,
                    "settled_postflight",
                    side_effect=lambda observe, name, error: real(
                        observe, name, error, sleep=sleeps.append
                    ),
                ):
                    if fail_at is None:
                        C.execute_phase(rec, specs, SELECTED, phase, results)
                        self.assertIn(phase[0], results)
                    else:
                        with self.assertRaises(RuntimeError) as caught:
                            C.execute_phase(rec, specs, SELECTED, phase, results)
                        self.assertIs(caught.exception, original)
                self.assertEqual(
                    calls,
                    [n for n in order if not (fail_at in order[:2] and n == phase[0])],
                )
                self.assertEqual(sleeps, [2, 20])

    def test_phase_parser_error_preserves_postflight(self):
        phase = P.PHASES[0]
        rows = P.phase_specs(OWNED, SELECTED, phase)
        calls, original = [], RuntimeError("parse")
        real = B.settled_postflight
        with tempfile.TemporaryDirectory() as temp:
            folder = Path(temp)
            (folder / "stdout").write_bytes(b"invalid\n")
            (folder / "stderr").write_bytes(b"")

            def run(name, *_args, **_kwargs):
                calls.append(name)
                if "settled" in name:
                    raise OSError("secondary")
                return folder

            with (
                mock.patch.object(P, "parse_transcript", side_effect=original),
                mock.patch.object(
                    B,
                    "settled_postflight",
                    side_effect=lambda observe, name, error: real(
                        observe, name, error, sleep=lambda _: None
                    ),
                ),
                self.assertRaises(RuntimeError) as caught,
            ):
                C.execute_phase(
                    mock.Mock(run=run),
                    {n: (c, s, w, e) for n, c, s, w, e in rows},
                    SELECTED,
                    phase,
                    {},
                )
            self.assertIs(caught.exception, original)
            self.assertEqual(calls, [r[0] for r in rows])

    def test_collection_digest_gates_cleanup(self):
        for match in (False, True):
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                (root / "inventory").mkdir()
                (root / "remote").mkdir()
                (root / "remote/file").write_bytes(b"result")
                (root / "inventory/stdout").write_text(
                    json.dumps(
                        B.inventory(root / "remote") if match else {"file": "0" * 64}
                    )
                )
                calls, value = [], state()

                def step(name):
                    calls.append(name)
                    return root / "inventory"

                with mock.patch.object(C, "HERE", root):
                    if match:
                        C.collect_and_clean(step, value)
                    else:
                        with self.assertRaises(RuntimeError):
                            C.collect_and_clean(step, value)
                self.assertEqual(
                    calls,
                    ["remote-inventory", "collect"]
                    + (["cleanup", "absence"] if match else []),
                )
                self.assertEqual(value["absence"], match)

    def test_native_failure_wins_settlement(self):
        original, value = RuntimeError("native"), state()
        with (
            mock.patch.object(C, "collect_and_clean", side_effect=OSError("collect")),
            self.assertRaises(RuntimeError) as caught,
        ):
            C.run_and_settle(mock.Mock(side_effect=original), value)
        self.assertIs(caught.exception, original)
        self.assertEqual(value["secondary_failures"][0]["stage"], "remote-settlement")

    def test_create_or_upload_failure_attempts_cleanup_and_absence(self):
        for failure_at in ("create", "upload"):
            original, value, calls = RuntimeError(failure_at), state(), []

            def step(name):
                calls.append(name)
                if name == failure_at:
                    raise original

            with self.assertRaises(RuntimeError) as caught:
                C.create_upload_and_run(step, value)
            self.assertIs(caught.exception, original)
            self.assertEqual(
                calls,
                ["create"]
                + (["upload"] if failure_at == "upload" else [])
                + ["cleanup", "absence"],
            )
            self.assertTrue(value["absence"])
            self.assertFalse(value["native_attempted"])

    def test_pre_native_cleanup_error_is_secondary(self):
        original, value, calls = RuntimeError("ambiguous create"), state(), []

        def step(name):
            calls.append(name)
            if name == "create":
                raise original
            if name == "cleanup":
                raise OSError("absent or mismatched owner")

        with self.assertRaises(RuntimeError) as caught:
            C.create_upload_and_run(step, value)
        self.assertIs(caught.exception, original)
        self.assertEqual(calls, ["create", "cleanup", "absence"])
        self.assertTrue(value["absence"])
        self.assertFalse(value["cleaned"])
        self.assertEqual(value["secondary_failures"][0]["stage"], "pre-native-cleanup")

    def test_native_attempt_does_not_enter_pre_native_cleanup(self):
        calls, value, original = [], state(), RuntimeError("uncollected native failure")
        with (
            mock.patch.object(C, "run_and_settle", side_effect=original),
            self.assertRaises(RuntimeError) as caught,
        ):
            C.create_upload_and_run(calls.append, value)
        self.assertIs(caught.exception, original)
        self.assertEqual(calls, ["create", "upload"])
        self.assertTrue(value["native_attempted"])
        self.assertFalse(value["absence"])

    def test_local_payload_retained_until_absence(self):
        for created, absent, remove in (
            (False, False, True),
            (True, False, False),
            (True, True, True),
        ):
            value = state()
            value.update(create_attempted=created, absence=absent)
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                payload = root / "payload"
                payload.mkdir()
                with mock.patch.object(C, "HERE", root):
                    self.assertIsNone(C.finalize_local(payload, value, None))
                self.assertEqual(payload.exists(), not remove)
                self.assertEqual(value["local_payload_absent"], remove)

    def test_finalization_preserves_primary_cause(self):
        value, original = state(), RuntimeError("original")
        with (
            mock.patch.object(C.shutil, "rmtree", side_effect=OSError("cleanup")),
            mock.patch.object(B, "write_json", side_effect=OSError("write")),
            mock.patch.object(C.sys, "stderr", io.StringIO()),
        ):
            self.assertIs(C.finalize_local(Path("/unused"), value, original), original)
        self.assertEqual(
            [r["stage"] for r in value["secondary_failures"]],
            ["local-payload-cleanup", "state-finalization"],
        )

    def test_source_observation_detects_bytes_and_modes(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            path = root / "source.rs"
            path.write_bytes(b"source")
            path.chmod(0o644)
            before = C.source_observation(root)
            path.chmod(0o755)
            after = C.source_observation(root)
            self.assertEqual(before["source_files"], after["source_files"])
            self.assertNotEqual(before["source_modes"], after["source_modes"])
            path.chmod(0o600)
            with self.assertRaises(RuntimeError):
                C.source_observation(root)

    def test_facade_keeps_processes_modes_and_directions_separate(self):
        parsed = {
            name: P.parse_transcript(OLD.transcript(enabled), SELECTED, enabled)
            for name, _, enabled in P.PHASES
        }
        parsed["candidate-on2"]["aggregates"][0]["forward_p50_ns"] = "500000"
        report = V.facade_comparison(parsed)
        self.assertEqual(len(report), 16)
        self.assertEqual(
            report["on/remap-per-round/forward/p50"][
                "candidate_over_baseline_by_replicate_index"
            ],
            [1.0, 0.5],
        )
        self.assertEqual(
            report["off/remap-per-round/forward/p50"][
                "candidate_over_baseline_by_replicate_index"
            ],
            [1.0, 1.0],
        )

    def test_initial_closure_rejects_extra_config_and_targets(self):
        for extra in ("target-baseline", "target-candidate", ".cargo"):
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                for name in {"owner.json", "binding.json", *P.PAYLOAD}:
                    (root / name).write_bytes(b"fixture")
                (root / "results").mkdir()
                C.initial_closure(root)
                (root / extra).mkdir()
                (root / extra / "config.toml").write_bytes(b"extra")
                with self.assertRaises(RuntimeError):
                    C.initial_closure(root)

    def test_initial_closure_rejects_links_and_nonordinary_nodes(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for name in {"owner.json", "binding.json", *P.PAYLOAD}:
                (root / name).write_bytes(b"fixture")
            (root / "results").mkdir()
            path = root / "campaign.py"
            path.unlink()
            path.symlink_to("protocol.py")
            with self.assertRaises(RuntimeError):
                C.initial_closure(root)
            path.unlink()
            C.os.mkfifo(path)
            with self.assertRaises(RuntimeError):
                C.initial_closure(root)


if __name__ == "__main__":
    unittest.main()
