#!/usr/bin/env python3
"""Harness tests only; mock Cargo results never become compiler evidence."""

import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import types
import unittest
from unittest.mock import patch


SCRIPT_ROOT = Path(__file__).resolve().parents[1]
ROOT = Path(os.environ.get("FE2O3_TUTORIAL_TEST_REPO_ROOT", SCRIPT_ROOT.parent)).resolve()
spec = importlib.util.spec_from_file_location("default_cargo_harness", SCRIPT_ROOT / "qualify-tutorial-default-cargo.py")
harness = importlib.util.module_from_spec(spec)
spec.loader.exec_module(harness)


class DefaultCargoHarnessTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.loaded = harness.load_inputs(ROOT, ROOT / "config/tutorial-kernel-manifest-v1.json")

    def temporary_run(self, runner, *, failures=(), loaded=None, targets=("gfx942", "gfx950")):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        output = Path(temporary.name) / "output"
        with patch.object(harness, "load_inputs", return_value=loaded or self.stub_inputs()), \
                patch.object(harness, "prerequisites", return_value=list(failures)):
            report = harness.run_census(ROOT, ROOT / "config/tutorial-kernel-manifest-v1.json",
                                        Path("/unit-only/cargo-fe2o3"), Path(temporary.name) / "cache",
                                        output, targets, 2, 2048, runner=runner, environment={"KEEP": "unchanged"})
        self.assertEqual(json.loads((output / "report.json").read_text()), report)
        self.assertFalse((output / "report.json.tmp").exists())
        return report, output

    def stub_inputs(self):
        validator, _, manifest, digest, registered = self.loaded
        rows = {}
        for number, (fixture, refs, negative) in enumerate(list(registered.values())[:3]):
            fixture = copy.deepcopy(fixture)
            fixture["fixtureId"] = f"unit-case-{number}"
            fixture["target"] = "gfx942" if number < 2 else "gfx950"
            rows[fixture["fixtureId"]] = (fixture, refs, number == 1)
        fake_census = types.SimpleNamespace(input_snapshot=lambda *args: {"snapshot": args[-1]})
        return validator, fake_census, manifest, digest, rows

    @staticmethod
    def mock_result(status="cargo-failed", exit_code=1):
        return {"status": status, "exitCode": exit_code, "logSha256": hashlib.sha256(b"").hexdigest(),
                "logBytes": 0, "logComplete": True, "directChildReaped": True}

    def test_real_manifest_roster_and_exact_cargo_feature_selection(self):
        _, _, manifest, _, registered = self.loaded
        self.assertGreaterEqual(len(registered), len(manifest["compilerFixtures"]))
        self.assertTrue(any(row[2] for row in registered.values()))
        self.assertEqual({row[0]["target"] for row in registered.values()}, {"gfx942", "gfx950"})
        for fixture, _, _ in registered.values():
            args = harness.command_for(ROOT, Path("/tool/cargo-fe2o3"), Path("/cache"), fixture)
            inputs = fixture["compilerInput"]
            self.assertEqual(args[:4], ["/tool/cargo-fe2o3", "authority", "release", "build"])
            self.assertEqual(args[args.index("--manifest-path") + 1], str(ROOT / inputs["packageManifest"]))
            self.assertEqual(args[args.index("--target-dir") + 1], "/cache/" + fixture["target"])
            self.assertEqual("--no-default-features" in args, not inputs["defaultFeatures"])
            self.assertEqual("--features" in args, bool(inputs["features"]))
            if inputs["features"]:
                self.assertEqual(args[args.index("--features") + 1].split(","), inputs["features"])
            for forbidden in ("--target", "--config", "engineering", "probe", "--all-features"):
                self.assertNotIn(forbidden, args)
            self.assertIn("--lib", args)
            self.assertIn("--locked", args)
            self.assertIn("--offline", args)
            self.assertTrue(inputs["kernelSymbols"])

    def test_generic_ci_dispatches_exact_harness_once_without_running_compiler_census(self):
        dispatch = r'''
set -Eeuo pipefail
source "$1"
run_step() {
  printf '%s' "$1"
  shift
  printf '\t%s' "$@"
  printf '\n'
}
run_workspace_dependency_policy() { :; }
run_standalone_lockfiles() { :; }
run_runtime_pure_rust_policy() { :; }
run_shard_policy() { :; }
run_parity_matrix_checks() { :; }
run_format() { :; }
run_check() { :; }
run_backend_build() { :; }
run_cpu_tests() { :; }
run_rustc_codegen_lib_tests() { :; }
run_auxiliary_tests() { :; }
run_all_rustc_codegen_shards() { :; }
"$2"
'''
        expected = ["tutorial-default-cargo-harness-tests", "python3", "-I", "-B",
                    "scripts/tests/tutorial_default_cargo.py"]
        with tempfile.TemporaryDirectory() as temporary:
            environment = {"PATH": os.defpath, "HOME": temporary,
                           "CARGO_TARGET_DIR": str(Path(temporary) / "cache"),
                           "CI_LOG_DIR": str(Path(temporary) / "logs")}
            for function in ("run_generic_core", "run_generic"):
                with self.subTest(function=function):
                    result = subprocess.run(
                        ["bash", "--noprofile", "--norc", "-c", dispatch, "bash",
                         str(SCRIPT_ROOT / "ci-local.sh"), function],
                        cwd=temporary, env=environment, text=True, capture_output=True,
                        timeout=10, check=True,
                    )
                    steps = [line.split("\t") for line in result.stdout.splitlines()]
                    self.assertEqual([step for step in steps
                                      if step[0] == expected[0] or expected[-1] in step], [expected])
                    self.assertFalse(any("scripts/qualify-tutorial-default-cargo.py" in step
                                         for step in steps))
            self.assertFalse((Path(temporary) / "cache").exists())
            self.assertFalse((Path(temporary) / "logs").exists())

    def test_real_physical_snapshot_uses_existing_source_closure(self):
        validator, census, manifest, digest, registered = self.loaded
        for source_case in (False, True):
            fixture = next(row[0] for row in registered.values()
                           if row[0]["fixtureId"].startswith("source-driver-") == source_case)
            snapshot = census.input_snapshot(validator, ROOT, manifest, digest, fixture)
            self.assertEqual(snapshot["manifestSha256"], digest)
            self.assertEqual(snapshot["fixture"], fixture)
            self.assertTrue(snapshot["sources"])
            for source in snapshot["sources"]:
                raw = (ROOT / source["path"]).read_bytes()
                self.assertEqual(hashlib.sha256(raw).hexdigest(), source["sha256"])
                self.assertEqual(len(raw), source["bytes"])

    def test_missing_runtime_marks_complete_roster_without_invoking_compiler(self):
        def forbidden(*args):
            self.fail("missing runtime must not launch even the first Cargo command")
        failure = {"kind": "missing-required-verus-runtime", "path": str(harness.RUNTIME)}
        report, _ = self.temporary_run(forbidden, failures=[failure])
        self.assertEqual(report["counts"], {"blocked-prerequisite": 3})
        self.assertTrue(report["complete"])
        self.assertTrue(report["coversAllRegisteredInvocations"])
        self.assertEqual(report["prerequisiteFailures"], [failure])
        self.assertTrue(all(case["expectedNegativeMatched"] is None for case in report["cases"]))

    def test_runtime_path_is_exact_production_requirement_not_cli_override(self):
        wrapper = (ROOT / "crates/rustc-codegen-fe2o3/src/production_pipeline_source_predicated_publish_v90.rs").read_text()
        self.assertIn('include!("production_pipeline_source_mixed_publish_family.rs")', wrapper)
        self.assertIn("publish_predicated_worker_handoff_v90", wrapper)
        source = (ROOT / "crates/rustc-codegen-fe2o3/src/production_pipeline_source_mixed_publish_family.rs").read_text()
        self.assertIn('"' + str(harness.RUNTIME) + '"', source)
        with patch.object(Path, "is_dir", return_value=False):
            failures = harness.prerequisites(Path("/unavailable/cli"), {})
        self.assertEqual(failures[0]["kind"], "missing-required-verus-runtime")
        self.assertTrue(any(row["kind"] == "missing-protected-environment" for row in failures))
        self.assertTrue(any(row["kind"] == "requires-exactly-one-production-build-config" for row in failures))

    def test_all_failures_and_zero_exit_retained_without_negative_or_success_claim(self):
        calls = []
        outcomes = [self.mock_result(), self.mock_result("cargo-completed-unqualified", 0),
                    self.mock_result("timeout", -9)]
        def runner(args, cwd, env, log, timeout, limit):
            self.assertEqual(cwd, ROOT)
            self.assertEqual(env["KEEP"], "unchanged")
            self.assertIn(env["FE2O3_TARGET"], ("gfx942", "gfx950"))
            self.assertEqual((timeout, limit), (2, 2048))
            calls.append(args)
            log.write_bytes(b"")
            return outcomes[len(calls) - 1]
        report, _ = self.temporary_run(runner)
        self.assertEqual(len(calls), 3)
        self.assertEqual(report["counts"], {"cargo-failed": 1, "cargo-completed-unqualified": 1, "timeout": 1})
        for key in ("qualified", "authenticatesCompilerExecution", "grantsArtifactOrLaunchAuthority",
                    "defaultPipelineQualificationPassed"):
            self.assertIs(report[key], False)
        self.assertTrue(all(not case["qualified"] and case["expectedNegativeMatched"] is None
                            for case in report["cases"]))

    def test_subset_is_explicit_not_all_tutorial_claim(self):
        report, _ = self.temporary_run(lambda *args: self.mock_result(), targets=("gfx950",))
        self.assertEqual(report["selectedInvocations"], 1)
        self.assertFalse(report["coversAllRegisteredInvocations"])

    def test_preexisting_output_is_not_overwritten(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(harness, "load_inputs", return_value=self.stub_inputs()):
            output = Path(temporary)
            sentinel = output / "report.json"
            sentinel.write_bytes(b"previous observation")
            with self.assertRaises(FileExistsError):
                harness.run_census(ROOT, ROOT / "config/tutorial-kernel-manifest-v1.json", Path("/tool"),
                                    output / "cache", output, ("gfx942",), 1, 1)
            self.assertEqual(sentinel.read_bytes(), b"previous observation")

    def test_changed_source_cannot_be_hidden_by_zero_exit(self):
        loaded = list(self.stub_inputs())
        calls = []
        def snapshot(*args):
            calls.append(args[-1]["fixtureId"])
            return {"revision": len(calls)}
        loaded[1] = types.SimpleNamespace(input_snapshot=snapshot)
        report, _ = self.temporary_run(lambda *args: self.mock_result("cargo-completed-unqualified", 0), loaded=loaded)
        self.assertEqual(report["counts"], {"source-changed": 3})
        self.assertTrue(all(row["execution"]["exitCode"] == 0 for row in report["cases"]))

    def test_validator_system_exit_is_retained_per_case_and_continues(self):
        loaded = list(self.stub_inputs())
        def snapshot(*args):
            raise SystemExit("tutorial kernel manifest: physical source hash changed")
        loaded[1] = types.SimpleNamespace(input_snapshot=snapshot)
        def forbidden(*args):
            self.fail("invalid source input cannot launch")
        report, _ = self.temporary_run(forbidden, loaded=loaded)
        self.assertEqual(report["counts"], {"invalid-source-input": 3})
        self.assertTrue(all("hash changed" in row["sourceError"] for row in report["cases"]))

    def test_real_subprocess_failure_log_is_exact_and_reaped(self):
        with tempfile.TemporaryDirectory() as temporary:
            log = Path(temporary) / "log"
            result = harness.run_command([sys.executable, "-c", "import sys; print('actual refusal'); sys.exit(7)"],
                                         ROOT, dict(os.environ), log, 3, 1024)
            self.assertEqual(result["status"], "cargo-failed")
            self.assertEqual(result["exitCode"], 7)
            self.assertEqual(log.read_bytes(), b"actual refusal\n")
            self.assertEqual(result["logSha256"], hashlib.sha256(log.read_bytes()).hexdigest())
            self.assertTrue(result["directChildReaped"])
            self.assertTrue(result["logComplete"])

    def test_timeout_and_exact_output_limit_are_failures(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            exact = harness.run_command([sys.executable, "-c", "import os; os.write(1,b'x'*32)"],
                                        ROOT, dict(os.environ), root / "exact", 3, 32)
            self.assertEqual(exact["status"], "cargo-completed-unqualified")
            short = harness.run_command([sys.executable, "-c", "import os; os.write(1,b'x'*32)"],
                                        ROOT, dict(os.environ), root / "short", 3, 31)
            self.assertEqual(short["status"], "log-limit")
            self.assertEqual(short["logBytes"], 31)
            self.assertFalse(short["logComplete"])
            timed = harness.run_command([sys.executable, "-c", "import time; time.sleep(30)"],
                                        ROOT, dict(os.environ), root / "timed", 0.05, 32)
            self.assertEqual(timed["status"], "timeout")
            self.assertTrue(timed["directChildReaped"])

    def test_launch_error_is_recorded_with_empty_exact_log(self):
        with tempfile.TemporaryDirectory() as temporary:
            result = harness.run_command(["/nonexistent/unit-only/command"], ROOT, {},
                                         Path(temporary) / "log", 1, 32)
            self.assertEqual(result["status"], "launch-error")
            self.assertIsNone(result["exitCode"])
            self.assertEqual(result["logBytes"], 0)


    def signal_case(self, number, *, during_spawn=False):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            process = subprocess.Popen(
                [sys.executable, str(Path(__file__).resolve()), "--signal-fixture", temporary,
                 "spawn" if during_spawn else "running"],
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True,
            )
            owned = None
            try:
                if not during_spawn:
                    deadline = time.monotonic() + 10
                    while not (directory / "ready").exists():
                        if process.poll() is not None or time.monotonic() >= deadline:
                            self.fail("real subprocess did not reach the interruption point")
                        time.sleep(0.01)
                    os.kill(process.pid, number)
                stdout, stderr = process.communicate(timeout=10)
                owned = int((directory / "owned.pid").read_text())
                self.assertEqual(process.returncode, 130, (stdout, stderr))
                self.assertIn(b"census interrupted", stderr)
                cleanup = json.loads((directory / "cleanup.json").read_text())
                self.assertEqual(cleanup, {"exitCode": 130, "childReaped": True,
                                           "handlersRestored": True, "childExitCode": -signal.SIGKILL})
                with self.assertRaises(ProcessLookupError):
                    os.kill(owned, 0)
                with self.assertRaises(ProcessLookupError):
                    os.killpg(owned, 0)
                report = json.loads((directory / "output/report.json").read_text())
                self.assertFalse(report["complete"])
                self.assertFalse(report["qualified"])
                self.assertEqual(report["cases"][0]["status"], "not-attempted")
            finally:
                if (directory / "owned.pid").exists():
                    owned = int((directory / "owned.pid").read_text())
                if owned is not None:
                    try:
                        os.killpg(owned, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                if process.poll() is None:
                    os.killpg(process.pid, signal.SIGKILL)
                process.communicate(timeout=5)

    def test_cli_signals_reap_owned_child_and_restore_handlers(self):
        for number in (signal.SIGTERM, signal.SIGHUP, signal.SIGINT):
            with self.subTest(signal=number):
                self.signal_case(number)

    def test_cli_signal_during_spawn_preserves_cleanup_ownership(self):
        self.signal_case(signal.SIGTERM, during_spawn=True)


class ProcessLifecycleTests(unittest.TestCase):
    def test_real_terminal_leader_is_waitable_until_group_cleanup(self):
        original_stop = harness.stop_group
        observations = []

        def checked_stop(process):
            self.assertIsNone(process.returncode)
            terminal = os.waitid(os.P_PID, process.pid,
                                 os.WEXITED | os.WNOHANG | os.WNOWAIT)
            self.assertIsNotNone(terminal)
            observations.append((terminal.si_pid, terminal.si_status))
            original_stop(process)
            with self.assertRaises(ChildProcessError):
                os.waitpid(process.pid, os.WNOHANG)

        for code in (0, 7):
            with self.subTest(exit_code=code), tempfile.TemporaryDirectory() as temporary, \
                    patch.object(harness, "stop_group", side_effect=checked_stop):
                result = harness.run_command([sys.executable, "-c", f"raise SystemExit({code})"],
                                             ROOT, dict(os.environ), Path(temporary) / "log", 3, 32)
                self.assertEqual(result["exitCode"], code)
                self.assertTrue(result["directChildReaped"])
        self.assertEqual([code for _, code in observations], [0, 7])

    def test_already_reaped_process_never_authorizes_group_signal(self):
        with subprocess.Popen([sys.executable, "-c", "pass"], start_new_session=True) as process:
            process.wait(timeout=3)
            with patch.object(harness.os, "killpg") as signal_group:
                harness.stop_group(process)
            signal_group.assert_not_called()

    def test_closed_output_does_not_remove_process_deadline(self):
        with tempfile.TemporaryDirectory() as temporary:
            result = harness.run_command(
                [sys.executable, "-c", "import os,time; os.close(1); os.close(2); time.sleep(30)"],
                ROOT, dict(os.environ), Path(temporary) / "log", 0.05, 32)
            self.assertEqual(result["status"], "timeout")
            self.assertEqual(result["exitCode"], -signal.SIGKILL)
            self.assertTrue(result["directChildReaped"])
            self.assertFalse(result["logComplete"])


def signal_fixture(directory, during_spawn):
    """A real sleeping subprocess under the CLI, never a compiler qualification."""
    directory = Path(directory)
    original_popen = subprocess.Popen
    original_stop = harness.stop_group
    previous = {number: signal.getsignal(number) for number in harness.INTERRUPT_SIGNALS}
    cleanup = {"childReaped": False}
    fixture = {"target": "gfx942"}
    loaded = (None, types.SimpleNamespace(input_snapshot=lambda *args: {}), {}, "0" * 64,
              {"signal-only": (fixture, [], False)})
    command = [sys.executable, "-c",
               "from pathlib import Path; import time; Path(" + repr(str(directory / "ready"))
               + ").write_text('ready'); time.sleep(60)"]

    def tracked_popen(*args, **kwargs):
        process = original_popen(*args, **kwargs)
        (directory / "owned.pid").write_text(str(process.pid))
        if during_spawn:
            os.kill(os.getpid(), signal.SIGTERM)
        return process

    def checked_stop(process):
        original_stop(process)
        cleanup["childExitCode"] = process.returncode
        try:
            os.waitpid(process.pid, os.WNOHANG)
        except ChildProcessError:
            cleanup["childReaped"] = True

    with patch.object(harness, "load_inputs", return_value=loaded), \
            patch.object(harness, "prerequisites", return_value=[]), \
            patch.object(harness, "command_for", return_value=command), \
            patch.object(harness.subprocess, "Popen", side_effect=tracked_popen), \
            patch.object(harness, "stop_group", side_effect=checked_stop):
        status = harness.main(["--repo-root", str(ROOT), "--cargo-fe2o3", sys.executable,
                               "--legacy-compile-census",
                               "--target-dir", str(directory / "cache"),
                               "--output", str(directory / "output")])
    cleanup["exitCode"] = status
    cleanup["handlersRestored"] = all(signal.getsignal(number) == handler
                                      for number, handler in previous.items())
    (directory / "cleanup.json").write_text(json.dumps(cleanup))
    return status


if __name__ == "__main__":
    if len(sys.argv) == 4 and sys.argv[1] == "--signal-fixture":
        raise SystemExit(signal_fixture(sys.argv[2], sys.argv[3] == "spawn"))
    unittest.main()
