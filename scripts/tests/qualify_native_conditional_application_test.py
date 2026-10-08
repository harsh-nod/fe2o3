#!/usr/bin/env python3
"""Native qualification observer controls, not protected execution evidence."""
import json
import contextlib
import copy
import io
import os
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import qualify_native_conditional_application as q

DEVICE = "0x0000000000000001"


def result():
    value = {key: {"sha256": "a" * 64, "bytes": 64} for key in q.BLOBS}
    value.update({key: "b" * 64 for key in q.IDENTITIES})
    value.update(schema=q.SCHEMA, device=DEVICE, target="gfx942:xnack-", outputs=[64, 37],
                 settled_native_launches=2, shutdown="released", result_credits="refunded",
                 physical_overlap_measured=False, all_host_devices_qualified=False)
    return value


def encoded(value):
    return json.dumps(value).encode() + b"\n"


def roster_result(selected, transport):
    value = result()
    del value["device"]
    pairs = [(source, destination) for source in selected for destination in selected
             if source != destination]
    value.update(schema=q.ROSTER_SCHEMA,
                 devices=[{"uid": uid, "render_minor": 128 + index}
                          for index, uid in enumerate(selected)],
                 outputs=list(range(32, 32 + len(selected))), transport=transport,
                 settled_native_launches=len(selected), settled_directed_copies=len(pairs),
                 native_peer_completions=len(pairs) if transport == "native-xgmi" else 0,
                 selected_roster_complete=True, copy_compute_order="compute-then-serial-copy",
                 directed_pairs=[{"source": source, "destination": destination,
                                  "payload_bytes": 24 + 4 * (32 + selected.index(source)),
                                  "native_peer_completions": ordinal if transport == "native-xgmi" else 0}
                                 for ordinal, (source, destination) in enumerate(pairs, 1)])
    return value


def successful_outcome(raw):
    return {"status": "cargo-completed-unqualified", "exitCode": 0,
            "logComplete": True, "directChildReaped": True,
            "logBytes": len(raw), "logSha256": q.hashlib.sha256(raw).hexdigest()}


class NativeQualificationTests(unittest.TestCase):
    def test_conflicting_startup_observation_prevents_any_cargo_attempt(self):
        startup = q.startup_helper()
        values = {name: {"Id": name, "LoadState": "loaded", "ActiveState": "inactive",
                         "SubState": "dead", "MainPID": "0", "InvocationID": "", "Job": ""}
                  for name in startup.UNITS}
        values[startup.MANAGER].update(ActiveState="active", SubState="running", MainPID="123",
                                       InvocationID="ab" * 16)
        values[startup.STANDALONE[0]].update(ActiveState="active", SubState="running", MainPID="456")
        raw = ("\n\n".join("\n".join(f"{key}={item}" for key, item in value.items())
                             for value in values.values()) + "\n").encode()
        with tempfile.TemporaryDirectory(prefix="fe2o3-native-startup-refusal-") as directory:
            root = Path(directory)
            args = SimpleNamespace(cargo_fe2o3=root / "cargo", repo_root=root,
                                   target_dir=root / "target", output=root / "output",
                                   producer_source="./src/lib.rs", device=DEVICE, devices=None,
                                   transport=None, cargo_fe2o3_sha256="a" * 64, timeout_seconds=10)
            calls = []
            def run(arguments, cwd, environment, log, timeout, maximum):
                calls.append(arguments)
                log.write_bytes(raw)
                return successful_outcome(raw)
            support = SimpleNamespace(prerequisites=lambda *_: [], write_report=lambda *_: None,
                                      run_command=run)
            with mock.patch.object(q, "startup_helper", return_value=startup), \
                 mock.patch.object(q, "source_snapshot") as source:
                report = q.qualify(args, support)
            self.assertEqual(report["status"], "refused")
            self.assertIn("conflicting standalone compiler", report["error"])
            self.assertFalse(report["nativeApplicationQualificationPassed"])
            self.assertFalse(report["rootServiceCleanupQualified"])
            self.assertEqual(len(calls), 1)
            self.assertEqual(calls[0][0], "/usr/bin/systemctl")
            source.assert_not_called()

    def test_installed_inventory_contains_all_three_native_policy_records(self):
        native_records = {
            Path("/etc/fe2o3/proof-custodian/application-native-deployment-v1"),
            Path("/etc/fe2o3/proof-custodian/native-manager-deployment-v1"),
            Path("/etc/fe2o3/proof-custodian/native-conditional-root-policy-v1"),
        }
        self.assertTrue(native_records.issubset(q.FIXED_FILES))
        self.assertEqual(len(q.FIXED_FILES), len(set(q.FIXED_FILES)))

    def test_explicit_native_command_preserves_source_spelling(self):
        command = q.command_for(Path("/tools/cargo-fe2o3"), Path("/repo"), Path("/target"),
                                "./src/lib.rs", DEVICE)
        self.assertEqual(command[:6], ["/tools/cargo-fe2o3", "authority", "release", "--native", "run",
                                      "--native-application-proof-custodian"])
        self.assertEqual(command.count("--native"), 1)
        self.assertNotIn("--application-proof-custodian", command)
        self.assertEqual(command[command.index("--bin") + 1], "native-conditional-fill")
        self.assertEqual(command[command.index("--") + 1:],
                         ["--native-v5", "--producer-source", "./src/lib.rs", "--device", DEVICE])

    def test_exact_native_result_amid_compiler_output(self):
        raw = b"Compiling application\n{\"reason\":\"compiler-artifact\"}\n" + encoded(result())
        self.assertEqual(q.result_from_log(raw, DEVICE), result())

    def test_absent_duplicate_and_legacy_results_refuse(self):
        for raw in (b"Cargo finished successfully\n", encoded(result()) * 2,
                    encoded({"schema": "fe2o3.genuine-two-gpu.v1"}),
                    encoded(result()) + encoded({"schema": "fe2o3.genuine-receipt-coexistence.v1"})):
            with self.subTest(raw=raw[:100]), self.assertRaises(ValueError):
                q.result_from_log(raw, DEVICE)

    def test_duplicate_keys_and_malformed_native_json_refuse(self):
        raw = encoded(result())
        for malformed in (raw.replace(b'"schema":', b'"schema":"old","schema":', 1),
                          raw.replace(b'"bytes": 64', b'"bytes": 64,"bytes": 64', 1),
                          raw[:-3]):
            with self.subTest(raw=malformed[:100]), self.assertRaises(ValueError):
                q.result_from_log(malformed, DEVICE)

    def test_closed_fields_and_native_cleanup_are_required(self):
        changes = {"schema": "fe2o3.native-conditional-fill.v2", "device": "0x0000000000000002",
                   "target": "gfx950:xnack-", "outputs": [37, 64], "settled_native_launches": 1,
                   "shutdown": "pending", "result_credits": "retained",
                   "physical_overlap_measured": True, "all_host_devices_qualified": True}
        for key, wrong in changes.items():
            value = result()
            value[key] = wrong
            with self.subTest(key=key), self.assertRaises(ValueError):
                q.result_from_log(encoded(value), DEVICE)
        for extra in (True, False):
            value = result()
            if extra:
                value["legacy"] = False
            else:
                del value["source"]
            with self.assertRaises(ValueError):
                q.result_from_log(encoded(value), DEVICE)

    def test_all_content_and_identity_axes_are_exact(self):
        for key in q.BLOBS:
            for wrong in ({"sha256": "0" * 64, "bytes": 1}, {"sha256": "A" * 64, "bytes": 1},
                          {"sha256": "a" * 64, "bytes": 0}, {"sha256": "a" * 64, "bytes": True},
                          {"sha256": "a" * 64, "bytes": 2**64}, {"sha256": "a" * 64}):
                value = result()
                value[key] = wrong
                with self.subTest(key=key, wrong=wrong), self.assertRaises(ValueError):
                    q.validate_result(value, DEVICE)
        for key in q.IDENTITIES:
            value = result()
            value[key] = "0" * 64
            with self.subTest(key=key), self.assertRaises(ValueError):
                q.validate_result(value, DEVICE)

    def test_boolean_and_numeric_types_cannot_substitute(self):
        for key, wrong in (("physical_overlap_measured", 0), ("all_host_devices_qualified", 0),
                           ("settled_native_launches", 2.0), ("outputs", [64.0, 37])):
            value = result()
            value[key] = wrong
            with self.subTest(key=key), self.assertRaises(ValueError):
                q.validate_result(value, DEVICE)

    def test_noncanonical_device_and_source_refuse(self):
        for value in ("1", "0x1", "0X0000000000000001", "0x000000000000000A", "0x0000000000000000"):
            with self.subTest(value=value), self.assertRaises(ValueError):
                q.device(value)
        for source in ("", "src/lib.rs\n", "src/\0lib.rs"):
            with self.subTest(source=source), self.assertRaises(ValueError):
                q.command_for(Path("/cargo"), Path("/repo"), Path("/target"), source, DEVICE)

    def test_success_requires_real_exit_complete_capture_and_cleanup(self):
        raw = encoded(result())
        outcome = {"status": "cargo-completed-unqualified", "exitCode": 0,
                   "logComplete": True, "directChildReaped": True,
                   "logBytes": len(raw), "logSha256": q.hashlib.sha256(raw).hexdigest()}
        self.assertEqual(q.reconcile_execution(outcome, raw, DEVICE), result())
        for key, wrong in (("status", "cargo-failed"), ("status", "timeout"),
                           ("exitCode", 1), ("exitCode", False), ("logComplete", False), ("directChildReaped", False),
                           ("logBytes", len(raw) - 1), ("logSha256", "f" * 64)):
            with self.subTest(key=key, wrong=wrong), self.assertRaises(ValueError):
                q.reconcile_execution({**outcome, key: wrong}, raw, DEVICE)

    def test_regular_input_hash_and_bounds(self):
        with tempfile.TemporaryDirectory(prefix="fe2o3-native-observer-") as directory:
            path = Path(directory) / "input"
            path.write_bytes(b"exact")
            self.assertEqual(q.measure(path), {"sha256": q.hashlib.sha256(b"exact").hexdigest(), "bytes": 5})
            with self.assertRaises(ValueError):
                q.measure(path, 4)
            alias = Path(directory) / "alias"
            alias.symlink_to(path)
            with self.assertRaises(OSError):
                q.measure(alias)
            fifo = Path(directory) / "fifo"
            os.mkfifo(fifo)
            with self.assertRaises(ValueError):
                q.measure(fifo)

    def test_roster_command_and_oracle_cover_every_bounded_directed_pair(self):
        for count in range(2, 9):
            selected = [f"0x{number:016x}" for number in range(count, 0, -1)]
            for transport in q.TRANSPORTS:
                with self.subTest(count=count, transport=transport):
                    command = q.command_for(Path("/cargo"), Path("/repo"), Path("/target"),
                                            "./src/lib.rs", selected, transport)
                    self.assertEqual(command[:6], ["/cargo", "authority", "release", "--native", "run",
                                                  "--native-application-proof-custodian"])
                    self.assertEqual(command.count("--native"), 1)
                    self.assertEqual(command[command.index("--bin") + 1], "native-conditional-fill-roster")
                    self.assertEqual(command[command.index("--") + 1:],
                                     ["--native-v5-roster", "--producer-source", "./src/lib.rs",
                                      "--transport", transport, "--devices", *selected])
                    value = roster_result(selected, transport)
                    self.assertEqual(q.result_from_log(encoded(value), selected, transport), value)
                    self.assertEqual(len(value["directed_pairs"]), count * (count - 1))
                    self.assertEqual(q.reconcile_execution(successful_outcome(encoded(value)),
                                                           encoded(value), selected, transport), value)

    def test_roster_requires_unique_explicit_canonical_selection_and_transport(self):
        for selected in ([], [DEVICE], [DEVICE, DEVICE], [DEVICE, "0x2"],
                         [DEVICE, 2], [DEVICE, "0x0000000000000000"],
                         [f"0x{index:016x}" for index in range(1, 10)]):
            with self.subTest(selected=selected), self.assertRaises(ValueError):
                q.selection(selected, "native-xgmi")
        for transport in (None, "automatic", "hip", "native-peer", "host-staging"):
            with self.subTest(transport=transport), self.assertRaises(ValueError):
                q.selection([DEVICE, "0x0000000000000002"], transport)
        for transport in q.TRANSPORTS:
            with self.assertRaises(ValueError):
                q.selection(DEVICE, transport)
        for source in ("a" * 4097, "\u00e9" * 2049):
            with self.assertRaises(ValueError):
                q.command_for(Path("/cargo"), Path("/repo"), Path("/target"), source,
                              [DEVICE, "0x0000000000000002"], "native-xgmi")

    def test_cli_preserves_single_device_and_excludes_mixed_selectors(self):
        common = ["--cargo-fe2o3", "/cargo", "--cargo-fe2o3-sha256", "a" * 64,
                  "--target-dir", "/target", "--output", "/output", "--producer-source", "src/lib.rs"]
        single = q.argument_parser().parse_args([*common, "--device", DEVICE])
        self.assertEqual(single.device, DEVICE)
        self.assertIsNone(single.devices)
        self.assertIsNone(single.transport)
        selected = [DEVICE, "0x0000000000000002"]
        roster = q.argument_parser().parse_args([*common, "--transport", "host-staged", "--devices", *selected])
        self.assertEqual(roster.devices, selected)
        self.assertIsNone(roster.device)
        for extra in ([], ["--device", DEVICE, "--devices", *selected]):
            with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                q.argument_parser().parse_args([*common, *extra])

    def test_roster_fields_cleanup_render_identity_and_scope_are_closed(self):
        selected = [DEVICE, "0x0000000000000002", "0x0000000000000003"]
        changes = {"transport": "host-staged", "outputs": [32, 33],
                   "settled_native_launches": 2, "settled_directed_copies": 5,
                   "native_peer_completions": 0, "selected_roster_complete": False,
                   "copy_compute_order": "concurrent", "physical_overlap_measured": True,
                   "all_host_devices_qualified": True, "shutdown": "pending",
                   "result_credits": "retained", "target": "gfx950:xnack-"}
        for key, wrong in changes.items():
            value = roster_result(selected, "native-xgmi")
            value[key] = wrong
            with self.subTest(key=key), self.assertRaises(ValueError):
                q.validate_result(value, selected, "native-xgmi")
        for field, wrong in (("uid", selected[1]), ("render_minor", 129),
                             ("render_minor", 127), ("render_minor", 256),
                             ("render_minor", True), ("render_minor", 128.0)):
            value = roster_result(selected, "native-xgmi")
            value["devices"][0][field] = wrong
            with self.subTest(field=field, wrong=wrong), self.assertRaises(ValueError):
                q.validate_result(value, selected, "native-xgmi")
        for key, wrong in (("outputs", [32.0, 33, 34]), ("settled_native_launches", 3.0),
                           ("selected_roster_complete", 1), ("physical_overlap_measured", 0)):
            value = roster_result(selected, "native-xgmi")
            value[key] = wrong
            with self.subTest(key=key), self.assertRaises(ValueError):
                q.validate_result(value, selected, "native-xgmi")
        value = roster_result(selected, "native-xgmi")
        value["device"] = DEVICE
        with self.assertRaises(ValueError):
            q.validate_result(value, selected, "native-xgmi")

    def test_roster_pair_oracle_rejects_substitutions_duplicates_and_partial_campaigns(self):
        selected = [DEVICE, "0x0000000000000002", "0x0000000000000003"]
        for transport in q.TRANSPORTS:
            original = roster_result(selected, transport)
            replacements = [original["directed_pairs"][:-1], original["directed_pairs"] * 2,
                            list(reversed(original["directed_pairs"])),
                            [original["directed_pairs"][0]] * 6]
            for pairs in replacements:
                value = {**original, "directed_pairs": pairs}
                with self.subTest(transport=transport, pairs=pairs), self.assertRaises(ValueError):
                    q.validate_result(value, selected, transport)
            for index in range(6):
                for key, wrong in (("source", "0x0000000000000009"),
                                   ("destination", original["directed_pairs"][index]["source"]),
                                   ("payload_bytes", 1), ("payload_bytes", 152.0),
                                   ("native_peer_completions", True),
                                   ("native_peer_completions", index + 2)):
                    value = copy.deepcopy(original)
                    value["directed_pairs"][index][key] = wrong
                    with self.subTest(transport=transport, index=index, key=key), self.assertRaises(ValueError):
                        q.validate_result(value, selected, transport)

    def test_roster_never_accepts_single_legacy_duplicate_or_unreaped_results(self):
        selected = [DEVICE, "0x0000000000000002"]
        value = roster_result(selected, "native-xgmi")
        raw = encoded(value)
        for observation in (encoded(result()), raw * 2,
                            raw + encoded({"schema": "fe2o3.genuine-two-gpu.v1"}),
                            raw.replace(b'"payload_bytes": 152', b'"payload_bytes":152,"payload_bytes":152', 1)):
            with self.assertRaises(ValueError):
                q.result_from_log(observation, selected, "native-xgmi")
        with self.assertRaises(ValueError):
            q.result_from_log(raw, DEVICE)
        for key, wrong in (("status", "timeout"), ("exitCode", 1), ("logComplete", False),
                           ("directChildReaped", False), ("logBytes", len(raw) + 1)):
            with self.assertRaises(ValueError):
                q.reconcile_execution({**successful_outcome(raw), key: wrong}, raw, selected, "native-xgmi")
        value["analysis_receipt"]["sha256"] = "0" * 64
        with self.assertRaises(ValueError):
            q.validate_result(value, selected, "native-xgmi")

    def test_source_inventory_rejects_missing_symlinked_and_unreadable_trees(self):
        with tempfile.TemporaryDirectory(prefix="fe2o3-native-roster-inventory-") as directory:
            root = Path(directory)
            with self.assertRaises(OSError):
                q.source_snapshot(root)
            trees = [Path("crates") / name / "src" for name in q.SOURCE_CRATES] + [q.FIXTURE / "src"]
            for tree in trees:
                (root / tree).mkdir(parents=True)
            tree = root / trees[0]
            tree.rmdir()
            tree.symlink_to(root / trees[1])
            with self.assertRaises(ValueError):
                q.source_snapshot(root)
            tree.unlink()
            tree.mkdir()
            def inaccessible(*args, **kwargs):
                kwargs["onerror"](PermissionError("unreadable inventory child"))
            with mock.patch.object(q.os, "walk", side_effect=inaccessible), self.assertRaises(PermissionError):
                q.source_snapshot(root)

    def test_roster_orchestration_uses_same_preflight_and_never_claims_all_host(self):
        # Every process/deployment observation is a stub here, not native evidence.
        selected = [DEVICE, "0x0000000000000002"]
        for transport in q.TRANSPORTS:
            for refused in (False, True):
                with tempfile.TemporaryDirectory(prefix="fe2o3-native-roster-driver-") as directory:
                    root = Path(directory)
                    args = SimpleNamespace(cargo_fe2o3=root / "cargo", repo_root=root,
                                           target_dir=root / "target", output=root / "output",
                                           producer_source="./src/lib.rs", device=None, devices=selected,
                                           transport=transport, cargo_fe2o3_sha256="a" * 64,
                                           timeout_seconds=10)
                    calls = []
                    reports = []
                    raw = encoded(roster_result(selected, transport))
                    def run(arguments, cwd, environment, log, timeout, maximum):
                        calls.append((arguments, cwd, environment, timeout, maximum))
                        log.write_bytes(raw)
                        return successful_outcome(raw)
                    support = SimpleNamespace(
                        prerequisites=lambda *_: ["root deployment unavailable"] if refused else [],
                        write_report=lambda _, value: reports.append(copy.deepcopy(value)),
                        run_command=run, RUNTIME=root / "proof-runtime", REQUIRED_ENV=(), CONFIG_ENV=(),
                        MAX_LOG_BYTES=1024 * 1024,
                    )
                    startup = SimpleNamespace(observe_installed_systemd=lambda *_: {
                        "scope": "test-only-stub-not-admission"})
                    with mock.patch.object(q, "startup_helper", return_value=startup), \
                         mock.patch.object(q, "measure", return_value={"sha256": "a" * 64, "bytes": 1}) as measured, \
                         mock.patch.object(q, "source_snapshot", return_value={"observed.rs": "exact"}):
                        report = q.qualify(args, support)
                    self.assertTrue(report["complete"])
                    self.assertEqual(report["nativeApplicationQualificationPassed"], not refused)
                    self.assertFalse(report["allHostDevicesQualified"])
                    self.assertFalse(report["milestoneClosure"])
                    self.assertFalse(report["grantsArtifactOrLaunchAuthority"])
                    self.assertFalse(report["rootServiceCleanupQualified"])
                    self.assertEqual(report["devices"], selected)
                    self.assertEqual(report["transport"], transport)
                    self.assertEqual(report["requiredApplicationSchema"], q.ROSTER_SCHEMA)
                    self.assertEqual(report["qualificationScope"], "explicit-selected-device-roster")
                    self.assertEqual(len(calls), 0 if refused else 1)
                    self.assertEqual(report["status"], "refused" if refused else "native-closed-fill-roster-pass")
                    if not refused:
                        self.assertEqual(set(report["installedObservations"]), set(map(str, q.FIXED_FILES)))
                        for name in ("application-native-deployment-v1", "native-manager-deployment-v1",
                                     "native-conditional-root-policy-v1"):
                            path = Path("/etc/fe2o3/proof-custodian") / name
                            self.assertEqual(sum(call.args == (path,) for call in measured.call_args_list), 2,
                                             f"{name} requires both before and after measurements")
                        self.assertEqual(calls[0][0], report["arguments"])
                        self.assertEqual(calls[0][2]["FE2O3_TARGET"], "gfx942")
                    self.assertEqual(reports[-1], report)

    def test_source_inventory_walks_named_roots_and_refuses_child_symlinks(self):
        with tempfile.TemporaryDirectory(prefix="fe2o3-native-inventory-roots-") as directory:
            root = Path(directory)
            trees = [Path("crates") / name / "src" for name in q.SOURCE_CRATES] + [q.FIXTURE / "src"]
            for tree in trees:
                (root / tree).mkdir(parents=True)
            with mock.patch.object(q, "measure", return_value={"sha256": "a" * 64, "bytes": 1}):
                records = q.source_snapshot(root)
            self.assertIn(str(q.FIXTURE / "Cargo.toml"), records)
            self.assertIn(str(q.FIXTURE / "Cargo.lock"), records)
            (root / trees[0] / "aliased").symlink_to(root / trees[1])
            with self.assertRaises(ValueError):
                q.source_snapshot(root)


if __name__ == "__main__":
    unittest.main()
