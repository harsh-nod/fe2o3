#!/usr/bin/env python3
"""Local diagnostic orchestration tests; no compiler, proof runtime, or hardware."""

from __future__ import annotations

import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "tutorial_source_export_generator", ROOT / "scripts/generate-tutorial-semantic-fixtures.py"
)
assert SPEC is not None and SPEC.loader is not None
GENERATOR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GENERATOR)
PRODUCER = GENERATOR._load_producer_contract()
PYTHON = Path(sys.executable).resolve()
MANIFEST = json.loads(GENERATOR.MANIFEST.read_bytes())
FIXTURES = {item["fixtureId"]: item for item in MANIFEST["compilerFixtures"]}
IDS = sorted(FIXTURES)
PREFIX = b"fe2o3 tutorial production transaction: "
BLOCKER = PREFIX + b"FE2O3-TUTORIAL-TXN-007: protected build stopped\n"


def digest(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


class SweepTestCase(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory(prefix="fe2o3-sweep-test-")
        self.addCleanup(temporary.cleanup)
        self.parent = Path(temporary.name).resolve()
        self.output = self.parent / "report"
        # Candidate discovery is mocked; no temporary commits or authority are produced.
        self.candidate = {"compilerCommit": "1" * 40, "compilerTree": "2" * 40, "worktreeClean": True}
        self.patch(GENERATOR, "_load_producer_contract", return_value=PRODUCER)
        self.candidate_check = self.patch(PRODUCER, "clean_candidate", return_value=self.candidate)
        self.forbidden = [
            self.patch(GENERATOR, name, side_effect=AssertionError(f"unexpected {name}"))
            for name in ("_git_snapshot", "_build_producer", "probe_production_exports", "records")
        ]
        self.forbidden += [
            self.patch(PRODUCER, name, side_effect=AssertionError(f"unexpected {name}"))
            for name in ("invoke_finalization_phase", "complete_pre_hardware_record")
        ]
        self.requests = []
        self.units = []
        self.exit_status = 7
        self.stderr = BLOCKER
        self.stdout = b""
        self.termination = None
        self.boundary = self.patch(PRODUCER, "_run_bounded", side_effect=self.export_boundary)

    def patch(self, target, name, **kwargs):
        patcher = mock.patch.object(target, name, **kwargs)
        self.addCleanup(patcher.stop)
        return patcher.start()

    def export_boundary(self, command, *, cwd, environment, timeout_seconds, observe):
        self.assertEqual(str(PYTHON), command[0])
        self.assertEqual(["--cargo-fe2o3", str(PYTHON), "--phase", "prepare", "--request"], command[1:6])
        self.assertEqual("--output-directory", command[7])
        self.assertEqual(9, len(command))
        self.assertEqual(ROOT, cwd)
        self.assertTrue(PRODUCER.SCRUBBED_ENVIRONMENT.isdisjoint(environment))
        request_path, export_root = Path(command[6]), Path(command[8])
        request = json.loads(request_path.read_bytes())
        self.assertEqual(PRODUCER.request_binding_sha256(request), request["requestBindingSha256"])
        self.assertTrue(all(not path.exists() for path in self.units))
        self.units.append(request_path.parent)
        self.requests.append(request)
        logs = {
            name: {"payload": payload, "observedBytes": len(payload), "observedSha256": digest(payload), "complete": self.termination is None}
            for name, payload in (("stdout", self.stdout), ("stderr", self.stderr))
        }
        observe({
            "command": command, "workingDirectory": str(cwd), "timeoutSeconds": timeout_seconds,
            "exitStatus": self.exit_status, "termination": self.termination, "logs": logs,
        })
        if self.exit_status != 0 or self.termination is not None:
            # Leave partial output to exercise per-fixture cleanup after failure.
            export_root.mkdir()
            (export_root / "partial").write_bytes(b"not production evidence")
            raise PRODUCER.QualificationProducerError("local test exporter failure")
        return self.stdout, self.stderr

    def sweep(self, fixture_ids=None, **kwargs):
        return GENERATOR.export_diagnostic_sweep(
            ROOT, self.output, PYTHON, mode="prepare", cargo_fe2o3=PYTHON,
            fixture_ids=fixture_ids, timeout_seconds=2, **kwargs
        )

    def object_bytes(self, reference):
        payload = (self.output / reference["path"]).read_bytes()
        self.assertEqual(len(payload), reference["bytes"])
        self.assertEqual(digest(payload), reference["sha256"])
        return payload

    def assert_cleaned(self):
        self.assertTrue(all(not path.exists() for path in self.units))
        self.assertEqual([self.output] if self.output.exists() else [], list(self.parent.iterdir()))


class PreparationSweepTests(SweepTestCase):
    def test_all_47_exact_manifest_routes_once_without_builds_or_mutations(self):
        protected = [GENERATOR.MANIFEST, *GENERATOR.FIXTURE_ROOT.glob("*.json"), * (ROOT / "config").glob("tutorial-negative-fixtures-v1.*")]
        before = {path: path.read_bytes() for path in protected}
        report = self.sweep()
        self.assertEqual(IDS, [request["fixture"]["fixtureId"] for request in self.requests])
        self.assertEqual(47, self.boundary.call_count)
        self.assertEqual(47, report["selection"]["manifestFixtureCount"])
        self.assertEqual({"gfx942": 10, "gfx950": 37}, report["summary"]["byTarget"])
        self.assertEqual(14, len(report["summary"]["byPackage"]))
        self.assertEqual({"exporter-failed": 47}, report["summary"]["byStatus"])
        self.assertEqual("diagnostic-only-no-qualification-authority", report["authority"])
        self.assertEqual("diagnostic-marker-not-simulation-evidence", report["simulatorInput"]["kind"])
        self.assertEqual(GENERATOR.PROBE_SIMULATOR_MARKER, self.object_bytes(report["simulatorInput"]["reference"]))
        self.assertEqual(PRODUCER._manifest_identity(MANIFEST, GENERATOR.MANIFEST.read_bytes()), report["manifest"])
        for row, request in zip(report["observations"], self.requests):
            fixture = FIXTURES[row["fixtureId"]]
            self.assertEqual(fixture["compilerInput"], row["compilerInput"])
            self.assertEqual(fixture["compilerInput"], request["fixture"]["compilerInput"])
            command = PRODUCER.manifest_contract._expected_hardware_command(fixture)
            self.assertEqual(command, row["hardwareCommandNotExecuted"])
            self.assertEqual(command, request["fixture"]["hardwareCommand"])
            self.assertEqual([
                suite for suite in MANIFEST["qualification"]["suites"]
                if any(row["fixtureId"] in coverage["fixtureIds"] for coverage in suite["coverage"])
            ], row["qualificationAdaptersNotExecuted"])
            self.assertFalse(request["productionTransaction"]["allowsFallback"])
            self.assertFalse(request["productionTransaction"]["allowsPipelineSelection"])
            self.assertEqual(request, json.loads(self.object_bytes(row["request"])))
            self.assertEqual(BLOCKER, self.object_bytes(row["process"]["logs"]["stderr"]["captured"]))
            empty = row["process"]["logs"]["stdout"]
            self.assertIsNone(empty["captured"])
            self.assertEqual((0, digest(b""), True), (empty["observedBytes"], empty["observedSha256"], empty["complete"]))
            self.assertEqual("protected-completion", row["earliestObservedBlockingStage"])
            self.assertEqual(7, row["process"]["exitStatus"])
            self.assertTrue(row["scratchCleanupComplete"])
            subject = {key: value for key, value in row.items() if key != "observationSha256"}
            self.assertEqual(digest(GENERATOR.SOURCE_OBSERVATION_DOMAIN + PRODUCER._canonical(subject)), row["observationSha256"])
        self.assertEqual(report, json.loads((self.output / "report.json").read_bytes()))
        self.assertEqual(before, {path: path.read_bytes() for path in protected})
        for forbidden in self.forbidden:
            forbidden.assert_not_called()
        self.candidate_check.assert_has_calls([mock.call(ROOT), mock.call(ROOT)])
        self.assert_cleaned()

    def test_subset_is_exact_sorted_and_never_expanded(self):
        report = self.sweep([IDS[-1], IDS[0]])
        self.assertEqual([IDS[0], IDS[-1]], report["selection"]["fixtureIds"])
        self.assertEqual(2, self.boundary.call_count)
        self.assertEqual(2, report["summary"]["observedFixtures"])
        self.assert_cleaned()

    def test_invalid_selection_cannot_start_an_export(self):
        for selection in ([], [IDS[0], IDS[0]], ["gfx950-*"], [IDS[0], "missing"]):
            with self.subTest(selection=selection), self.assertRaises(ValueError):
                self.sweep(selection)
        self.boundary.assert_not_called()
        self.assert_cleaned()

    def test_stale_candidate_rejected_without_snapshot_fallback(self):
        self.candidate_check.side_effect = PRODUCER.QualificationProducerError("dirty compiler worktree")
        with self.assertRaisesRegex(ValueError, "dirty compiler"):
            self.sweep([IDS[0]])
        self.boundary.assert_not_called()
        self.assert_cleaned()

    def test_candidate_changed_during_sweep_is_not_published(self):
        self.candidate_check.side_effect = [self.candidate, {**self.candidate, "compilerCommit": "3" * 40}]
        with self.assertRaisesRegex(ValueError, "candidate changed"):
            self.sweep([IDS[0]])
        self.assert_cleaned()

    def test_exporter_changed_during_sweep_is_not_published(self):
        real_read = PRODUCER._read_regular
        reads = []

        def read(path, label, *args):
            payload = real_read(path, label, *args)
            if label == "exporter":
                reads.append(path)
                if len(reads) == 2:
                    return payload + b"changed"
            return payload

        self.patch(PRODUCER, "_read_regular", side_effect=read)
        with self.assertRaisesRegex(ValueError, "exporter changed"):
            self.sweep([IDS[0]])
        self.assert_cleaned()

    def test_outputs_refuse_overwrite_repository_and_symlink(self):
        self.output.mkdir()
        with self.assertRaisesRegex(ValueError, "new directory outside"):
            self.sweep([IDS[0]])
        self.output.rmdir()
        with self.assertRaisesRegex(ValueError, "new directory outside"):
            GENERATOR.export_diagnostic_sweep(ROOT, ROOT / "config/new-report", PYTHON, mode="prepare", cargo_fe2o3=PYTHON)
        self.output.symlink_to(self.parent / "missing")
        with self.assertRaisesRegex(ValueError, "new directory outside"):
            self.sweep([IDS[0]])
        self.output.unlink()
        self.boundary.assert_not_called()
        self.assert_cleaned()

    def test_timeout_bound_checked_before_export(self):
        for timeout in (0, -1, True, 1.5, GENERATOR.PRODUCER_TIMEOUT_SECONDS + 1):
            with self.subTest(timeout=timeout), self.assertRaisesRegex(ValueError, "timeout"):
                GENERATOR.export_diagnostic_sweep(ROOT, self.output, PYTHON, mode="prepare", cargo_fe2o3=PYTHON, timeout_seconds=timeout)
        self.boundary.assert_not_called()
        self.assert_cleaned()

    def test_exit_zero_without_real_preparation_is_not_success(self):
        self.exit_status, self.stderr = 0, b""
        report = self.sweep([IDS[0]])
        row = report["observations"][0]
        self.assertEqual("invalid-preparation", row["status"])
        self.assertEqual("preparation-transport-validation", row["earliestObservedBlockingStage"])
        self.assertIsNone(row["preparedExport"])
        self.assertTrue(self.object_bytes(row["orchestrationDiagnostic"]))
        self.assert_cleaned()

    def test_exit_zero_stdout_claim_cannot_be_preparation_authority(self):
        self.exit_status, self.stderr, self.stdout = 0, b"", b'{"qualified":true}\n'
        report = self.sweep([IDS[0]])
        row = report["observations"][0]
        self.assertEqual("invalid-preparation", row["status"])
        self.assertEqual(self.stdout, self.object_bytes(row["process"]["logs"]["stdout"]["captured"]))
        self.assertIn(b"not stdout authority", self.object_bytes(row["orchestrationDiagnostic"]))
        self.assert_cleaned()

    def test_transport_only_success_never_finalizes_or_qualifies(self):
        self.exit_status, self.stderr = 0, b""

        def transport_only(export_root, request):
            # This mock tests routing only, not sealed proof or native acceptance.
            export_root.mkdir()
            (export_root / PRODUCER.PRE_HARDWARE_RESULT_NAME).write_bytes(b"mock transport only\n")
            return {"evidenceFiles": {
                kind: PRODUCER._reference(f"mock {kind}".encode())
                for kind in ("sealed-production-receipt", "simulation-bundle-v8")
            }}

        self.patch(PRODUCER, "validate_pre_hardware_export", side_effect=transport_only)
        report = self.sweep([IDS[0]])
        row = report["observations"][0]
        self.assertEqual("prepared-not-qualified", row["status"])
        self.assertIsNone(row["earliestObservedBlockingStage"])
        self.assertEqual([], report["summary"]["blockerGroups"])
        self.assertEqual(b"mock transport only\n", self.object_bytes(row["preparedExport"]["envelope"]))
        self.assertFalse(row["preparedExport"]["exportArtifactsRetained"])
        for forbidden in self.forbidden:
            forbidden.assert_not_called()
        self.assert_cleaned()

    def test_spawn_error_is_not_started_with_null_exit(self):
        self.exit_status, self.stderr, self.termination = None, b"", "spawn-error"
        row = self.sweep([IDS[0]])["observations"][0]
        self.assertEqual("not-started", row["status"])
        self.assertEqual("exporter-invocation", row["earliestObservedBlockingStage"])
        self.assertIsNone(row["process"]["exitStatus"])
        self.assert_cleaned()

    def test_signal_exit_is_preserved_not_replaced_by_wrapper_status(self):
        self.exit_status, self.stderr, self.termination = -signal.SIGKILL, b"partial\xff", "timeout"
        row = self.sweep([IDS[0]])["observations"][0]
        self.assertEqual(-signal.SIGKILL, row["process"]["exitStatus"])
        self.assertEqual("timeout", row["process"]["termination"])
        self.assertEqual("unclassified-exporter-failure", row["earliestObservedBlockingStage"])
        self.assertEqual(self.stderr, self.object_bytes(row["process"]["logs"]["stderr"]["captured"]))
        self.assertFalse(row["process"]["logs"]["stderr"]["complete"])
        self.assert_cleaned()

    def test_interrupt_cleans_scratch_without_publishing_partial_batch(self):
        self.boundary.side_effect = KeyboardInterrupt
        with self.assertRaises(KeyboardInterrupt):
            self.sweep([IDS[0]])
        self.assert_cleaned()

    def test_native_phase_preserves_runtime_pins_but_not_cache_override(self):
        with mock.patch.dict(os.environ, {
            "FE2O3_PRODUCTION_BUILD_CONFIG_V2": "/runtime/production-v2.json",
            "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1": "4" * 64,
            "CARGO_TARGET_DIR": "/must/not/override/native/preparation",
            "RUSTC_WRAPPER": "/must/not/run",
        }):
            self.sweep([IDS[0]])
        environment = self.boundary.call_args.kwargs["environment"]
        self.assertEqual("/runtime/production-v2.json", environment["FE2O3_PRODUCTION_BUILD_CONFIG_V2"])
        self.assertEqual("4" * 64, environment["FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1"])
        self.assertNotIn("CARGO_TARGET_DIR", environment)
        self.assertNotIn("RUSTC_WRAPPER", environment)
        self.assert_cleaned()


class RawSourceExportSweepTests(SweepTestCase):
    def setUp(self):
        super().setUp()
        self.stderr = b"fe2o3 rustc extraction: local proof hierarchy coverage failed\nfe2o3-export-sim: Cargo extraction failed with exit status: 1\n"
        self.bundle_bytes = b"mock bytes, not a sealed Bundle V8"
        self.shared_paths = []
        self.commands = []
        self.patch(PRODUCER, "transaction_request", side_effect=AssertionError("raw extraction is not preparation"))
        self.patch(PRODUCER, "invoke_compiler_phase", side_effect=AssertionError("raw extraction is not preparation"))
        self.patch(PRODUCER, "validate_pre_hardware_export", side_effect=AssertionError("raw extraction is not preparation"))

    def export_boundary(self, command, *, cwd, environment, timeout_seconds, observe):
        self.assertEqual(str(PYTHON), command[0])
        self.assertEqual(ROOT, cwd)
        self.assertEqual("--crate", command[1])
        self.assertEqual("--output", command[3])
        self.assertEqual(["--bundle-version", "8", "--target"], command[5:8])
        self.assertEqual("--target-dir", command[9])
        self.assertEqual("--", command[11])
        self.assertEqual("1", environment["FE2O3_HIP_SYS_DISABLE"])
        self.assertEqual("1", environment["FE2O3_HSA_RUNTIME_DISABLE"])
        bundle = Path(command[4])
        self.assertFalse(bundle.exists())
        self.assertTrue(all(not path.exists() for path in self.units))
        self.units.append(bundle.parent)
        self.commands.append(command)
        self.shared_paths.append(Path(command[10]))
        Path(command[10]).mkdir(exist_ok=True)
        logs = {
            name: {"payload": payload, "observedBytes": len(payload), "observedSha256": digest(payload), "complete": True}
            for name, payload in (("stdout", b""), ("stderr", self.stderr))
        }
        observe({"command": command, "workingDirectory": str(cwd), "timeoutSeconds": timeout_seconds, "exitStatus": self.exit_status, "termination": None, "logs": logs})
        if self.exit_status:
            bundle.write_bytes(b"incomplete output")
            raise PRODUCER.QualificationProducerError("local test extraction failure")
        if self.bundle_bytes is not None:
            bundle.write_bytes(self.bundle_bytes)
        return b"", self.stderr

    def raw_sweep(self, fixture_ids=None, **kwargs):
        return GENERATOR.export_diagnostic_sweep(ROOT, self.output, PYTHON, mode="source", fixture_ids=fixture_ids, timeout_seconds=2, **kwargs)

    def test_all47_raw_extraction_exact_cargo_contract_and_shared_scratch(self):
        report = self.raw_sweep()
        self.assertEqual(47, len(self.commands))
        self.assertEqual("source", report["mode"])
        self.assertEqual(GENERATOR.SOURCE_SWEEP_SCHEMA, report["schema"])
        self.assertEqual({"gfx942": 10, "gfx950": 37}, report["summary"]["byTarget"])
        self.assertEqual(14, len(report["summary"]["byPackage"]))
        self.assertEqual({"exporter-failed": 47}, report["summary"]["byStatus"])
        self.assertIsNone(report["simulatorInput"])
        self.assertEqual(1, len(set(self.shared_paths)))
        self.assertEqual("sweep-temporary", report["buildCache"]["ownership"])
        for identity, row, command in zip(IDS, report["observations"], self.commands):
            fixture = FIXTURES[identity]
            compiler_input = fixture["compilerInput"]
            cargo = ["--manifest-path", compiler_input["packageManifest"], "--lib"]
            if not compiler_input["defaultFeatures"]:
                cargo.append("--no-default-features")
            if compiler_input["features"]:
                cargo += ["--features", ",".join(compiler_input["features"])]
            self.assertEqual(cargo, command[12:])
            self.assertEqual(compiler_input["cargoTarget"]["name"], command[2])
            self.assertEqual(fixture["target"], command[8])
            self.assertEqual(command, row["process"]["command"])
            self.assertEqual(identity, row["fixtureId"])
            self.assertEqual("rustc-source-extraction", row["earliestObservedBlockingStage"])
            self.assertIsNone(row["request"])
            self.assertIsNone(row["sourceExport"])
            self.assertIsNone(row["preparedExport"])
            self.assertEqual(self.stderr, self.object_bytes(row["process"]["logs"]["stderr"]["captured"]))
        for path in self.shared_paths:
            self.assertFalse(path.exists())
        self.assert_cleaned()

    def test_operator_cache_reused_and_never_removed(self):
        cache = self.parent / "shared-target"
        cache.mkdir()
        sentinel = cache / "existing-user-cache"
        sentinel.write_bytes(b"keep")
        report = self.raw_sweep([IDS[-1], IDS[0]], target_dir=cache)
        self.assertEqual([cache, cache], self.shared_paths)
        self.assertEqual("operator-retained", report["buildCache"]["ownership"])
        self.assertEqual(b"keep", sentinel.read_bytes())
        self.assertTrue(all(not path.exists() for path in self.units))
        self.assertEqual({cache, self.output}, set(self.parent.iterdir()))

    def test_success_bytes_are_unverified_and_do_not_promote(self):
        self.exit_status, self.stderr = 0, b""
        report = self.raw_sweep([IDS[0]])
        row = report["observations"][0]
        self.assertEqual("exported-unverified", row["status"])
        self.assertEqual("diagnostic-only-no-qualification-authority", report["authority"])
        self.assertEqual(digest(self.bundle_bytes), row["sourceExport"]["sha256"])
        self.assertEqual("bytes-only-not-sealed-authority", row["sourceExport"]["validation"])
        self.assertFalse(row["sourceExport"]["retained"])
        self.assertIsNone(row["preparedExport"])
        self.assert_cleaned()

    def test_cache_hit_without_fresh_output_cannot_reuse_old_bundle(self):
        self.exit_status, self.stderr, self.bundle_bytes = 0, b"", None
        row = self.raw_sweep([IDS[0]])["observations"][0]
        self.assertEqual("invalid-source-output", row["status"])
        self.assertEqual("source-output-validation", row["earliestObservedBlockingStage"])
        self.assertIsNone(row["sourceExport"])
        self.assert_cleaned()

    def test_raw_subset_never_expands(self):
        report = self.raw_sweep([IDS[-1], IDS[0]])
        self.assertEqual([IDS[0], IDS[-1]], report["selection"]["fixtureIds"])
        self.assertEqual(2, len(self.commands))
        self.assert_cleaned()

    def test_cargo_diagnostic_routes_to_report_without_changing_exit_or_raw_log(self):
        line = b"error[E0425]: cannot find value `value_base` in this scope\n"
        self.stderr = b"warning: unrelated FE2O3-TUTORIAL-TXN-011\n" + line + b"fe2o3-export-sim: Cargo extraction failed with exit status: 101\n"
        self.exit_status = 1
        report = self.raw_sweep([IDS[0]])
        row = report["observations"][0]
        self.assertEqual("rustc-compilation", row["earliestObservedBlockingStage"])
        self.assertEqual("E0425", row["diagnostic"]["code"])
        self.assertEqual(digest(line), row["diagnostic"]["sha256"])
        self.assertEqual(1, row["process"]["exitStatus"])
        self.assertEqual(self.stderr, self.object_bytes(row["process"]["logs"]["stderr"]["captured"]))
        self.assertEqual([{
            "stage": "rustc-compilation", "diagnosticCode": "E0425", "fixtureIds": [IDS[0]],
        }], report["summary"]["blockerGroups"])
        self.assertEqual("exporter-failed", row["status"])
        self.assertEqual("diagnostic-only-no-qualification-authority", report["authority"])
        self.assert_cleaned()


class SourceExportDiagnosticTests(unittest.TestCase):
    def test_archived_cargo_and_rustc_forms_precede_export_exit(self):
        examples = (
            (b'error: failed to select a version for the requirement `cc = "^1"` (locked to 1.4.5)', "cargo-dependency-resolution", None),
            (b"error: cannot update the lock file /candidate/examples/gemm/Cargo.lock because --locked was passed to prevent this", "cargo-lockfile", None),
            (b"error[E0425]: cannot find value `value_base` in this scope", "rustc-compilation", "E0425"),
            (b"error: bounded for lowering requires literal range endpoints", "cargo-rustc", None),
        )
        noise = b"   Compiling example v0.1.0\nwarning: unrelated \xff\n"
        suffix = b"error: could not compile `example` (lib) due to previous errors\nfe2o3-export-sim: Cargo extraction failed with exit status: 101\n"
        for error, stage, code in examples:
            with self.subTest(stage=stage):
                line = error + b"\r\n"
                result = GENERATOR.source_export_diagnostic(noise + line + suffix, "source")
                self.assertEqual(stage, result["stage"])
                self.assertEqual(code, result["code"])
                self.assertEqual(len(noise), result["byteOffset"])
                self.assertEqual(len(line), result["bytes"])
                self.assertEqual(digest(line), result["sha256"])
                self.assertEqual(error.split(b": ", 1)[1].decode(), result["text"])

    def test_cargo_lock_and_missing_package_variants(self):
        for line, stage in (
            (b"error: the lock file /candidate/Cargo.lock needs to be updated but --locked was passed to prevent this\n", "cargo-lockfile"),
            (b"error: no matching package named `cc` found\n", "cargo-dependency-resolution"),
            (b"error: could not compile `example` (lib)\n", "cargo-rustc"),
            (b"error: failed to run custom build command for `example`\n", "cargo-rustc"),
        ):
            with self.subTest(line=line):
                result = GENERATOR.source_export_diagnostic(line, "source")
                self.assertEqual(stage, result["stage"])
                self.assertIsNone(result["code"])

    def test_source_stage_is_first_diagnostic_not_a_later_or_ranked_stage(self):
        extraction = b"fe2o3 rustc extraction: production collection failed\n"
        compilation = b"error[E0425]: unresolved value\n"
        for first, second, expected in (
            (extraction, compilation, "rustc-source-extraction"),
            (compilation, extraction, "rustc-compilation"),
        ):
            with self.subTest(expected=expected):
                result = GENERATOR.source_export_diagnostic(first + second, "source")
                self.assertEqual(expected, result["stage"])
                self.assertEqual(0, result["byteOffset"])
                self.assertEqual(digest(first), result["sha256"])

    def test_source_ignores_warning_notes_source_excerpts_and_embedded_codes(self):
        noise = (
            b"warning: error[E0425]: unresolved value\n"
            b"note: error: cannot update the lock file /tmp/Cargo.lock\n"
            b"  --> examples/lib.rs:4:5\n"
            b'4 | let text = "error[E0425]: cannot find value";\n'
            b"  error[E0425]: indented source excerpt\n"
            b"proof note FE2O3-TUTORIAL-TXN-011: unrelated\n"
        )
        self.assertIsNone(GENERATOR.source_export_diagnostic(noise, "source"))
        wrapper = b"fe2o3-export-sim: Cargo extraction failed with exit status: 101\n"
        result = GENERATOR.source_export_diagnostic(noise + wrapper, "source")
        self.assertEqual("source-export", result["stage"])
        self.assertEqual(len(noise), result["byteOffset"])
        self.assertIsNone(result["code"])
        for line in (
            b"error: message quotes FE2O3-TUTORIAL-TXN-011: failure\n",
            b"error: message quotes error[E0425]: failure\n",
            b"error: message quotes cannot update the lock file /tmp/Cargo.lock\n",
            b"error: message quotes failed to select a version for a dependency\n",
        ):
            result = GENERATOR.source_export_diagnostic(line, "source")
            self.assertEqual("cargo-rustc", result["stage"])
            self.assertIsNone(result["code"])
        extraction = b"fe2o3 rustc extraction: production admission failed: error[FE2O3-OWN-002]: hierarchy incomplete\n"
        result = GENERATOR.source_export_diagnostic(extraction, "source")
        self.assertEqual("rustc-source-extraction", result["stage"])
        self.assertIsNone(result["code"])

    def test_display_color_does_not_change_raw_diagnostic_identity(self):
        noise = b"warning: unrelated\n"
        line = b"\x1b[1m\x1b[31merror[E0425]\x1b[0m: unresolved \xff\r\n"
        result = GENERATOR.source_export_diagnostic(noise + line, "source")
        self.assertEqual("rustc-compilation", result["stage"])
        self.assertEqual("E0425", result["code"])
        self.assertEqual(len(noise), result["byteOffset"])
        self.assertEqual(len(line), result["bytes"])
        self.assertEqual(digest(line), result["sha256"])
        self.assertEqual("unresolved \ufffd", result["text"])

    def test_preparation_still_requires_native_transaction_diagnostic(self):
        noise = b"error[E0425]: unresolved\nerror: cannot update the lock file /tmp/Cargo.lock\n"
        self.assertIsNone(GENERATOR.source_export_diagnostic(noise, "prepare"))
        result = GENERATOR.source_export_diagnostic(noise + BLOCKER, "prepare")
        self.assertEqual("protected-completion", result["stage"])
        self.assertEqual("FE2O3-TUTORIAL-TXN-007", result["code"])
        self.assertEqual(len(noise), result["byteOffset"])

    def test_first_structured_boundary_keeps_exact_raw_line_identity(self):
        noise = b"inner error proof, mfma, FE2O3-TUTORIAL-TXN-005: not exporter boundary\xff\n"
        line = PREFIX + b"FE2O3-TUTORIAL-TXN-007: compiler failed \xff\r\n"
        result = GENERATOR.source_export_diagnostic(noise + line + BLOCKER)
        self.assertEqual(len(noise), result["byteOffset"])
        self.assertEqual(len(line), result["bytes"])
        self.assertEqual(digest(line), result["sha256"])
        self.assertEqual("protected-completion", result["stage"])
        self.assertEqual("FE2O3-TUTORIAL-TXN-007", result["code"])
        self.assertFalse(result["textTruncated"])

    def test_unknown_codes_and_plain_text_never_infer_proof_progress(self):
        result = GENERATOR.source_export_diagnostic(PREFIX + b"FE2O3-TUTORIAL-TXN-999: source proof issue\n")
        self.assertEqual("unclassified-exporter-failure", result["stage"])
        self.assertEqual("FE2O3-TUTORIAL-TXN-999", result["code"])
        self.assertIsNone(GENERATOR.source_export_diagnostic(b"proof failed: FE2O3-TUTORIAL-TXN-007\n"))
        result = GENERATOR.source_export_diagnostic(PREFIX + b"source proof failed\n")
        self.assertIsNone(result["code"])
        self.assertEqual("unclassified-exporter-failure", result["stage"])

    def test_display_truncation_does_not_change_line_hash(self):
        line = PREFIX + b"FE2O3-TUTORIAL-TXN-007: " + b"x" * 4096 + b"\n"
        result = GENERATOR.source_export_diagnostic(line)
        self.assertTrue(result["textTruncated"])
        self.assertEqual(2048, len(result["text"]))
        self.assertEqual(digest(line), result["sha256"])


class BoundedObservationTests(unittest.TestCase):
    def run_python(self, source, observations, **kwargs):
        return PRODUCER._run_bounded(
            [str(PYTHON), "-c", source], cwd=ROOT, environment=os.environ.copy(),
            timeout_seconds=kwargs.pop("timeout_seconds", 5), observe=observations.append, **kwargs
        )

    def test_failed_process_preserves_exact_bytes_and_still_raises(self):
        observations = []
        source = "import os; os.write(1,b'out\\x00\\xff'); os.write(2,b'err\\xfe\\n'); raise SystemExit(7)"
        with self.assertRaisesRegex(PRODUCER.QualificationProducerError, "status 7"):
            self.run_python(source, observations)
        self.assertEqual(1, len(observations))
        record = observations[0]
        self.assertEqual(7, record["exitStatus"])
        self.assertIsNone(record["termination"])
        self.assertEqual([str(PYTHON), "-c", source], record["command"])
        for name, payload in (("stdout", b"out\x00\xff"), ("stderr", b"err\xfe\n")):
            self.assertEqual({"payload": payload, "observedBytes": len(payload), "observedSha256": digest(payload), "complete": True}, record["logs"][name])

    def test_successful_process_is_observed_without_changing_result(self):
        observations = []
        self.assertEqual((b"ok\n", b""), self.run_python("print('ok')", observations))
        self.assertEqual(0, observations[0]["exitStatus"])
        self.assertEqual(digest(b""), observations[0]["logs"]["stderr"]["observedSha256"])

    def test_output_limit_retains_bounded_prefix_and_full_observed_digest(self):
        observations = []
        with self.assertRaisesRegex(ValueError, "output bounds"):
            self.run_python("import os,time; os.write(1,b'x'*4096); time.sleep(30)", observations, stdout_limit=64)
        record = observations[0]
        self.assertEqual("output-bound", record["termination"])
        self.assertEqual(-signal.SIGKILL, record["exitStatus"])
        self.assertEqual({"payload": b"x" * 64, "observedBytes": 4096, "observedSha256": digest(b"x" * 4096), "complete": False}, record["logs"]["stdout"])

    def test_timeout_preserves_actual_signal_and_partial_log(self):
        observations = []
        with self.assertRaisesRegex(ValueError, "timeout"):
            self.run_python("import os,time; os.write(2,b'started'); time.sleep(30)", observations, timeout_seconds=1)
        self.assertEqual("timeout", observations[0]["termination"])
        self.assertEqual(-signal.SIGKILL, observations[0]["exitStatus"])
        self.assertEqual(b"started", observations[0]["logs"]["stderr"]["payload"])
        self.assertFalse(observations[0]["logs"]["stderr"]["complete"])

    def test_missing_executable_has_no_counterfeit_exit_status(self):
        observations = []
        with tempfile.TemporaryDirectory() as directory, self.assertRaisesRegex(ValueError, "cannot start command"):
            PRODUCER._run_bounded(
                [str(Path(directory) / "missing")], cwd=ROOT, environment={},
                timeout_seconds=1, observe=observations.append,
            )
        self.assertEqual("spawn-error", observations[0]["termination"])
        self.assertIsNone(observations[0]["exitStatus"])

    def test_keyboard_interrupt_reaps_process_group(self):
        processes = []
        popen = subprocess.Popen

        def start(*args, **kwargs):
            process = popen(*args, **kwargs)
            processes.append(process)
            return process

        with mock.patch.object(PRODUCER.subprocess, "Popen", side_effect=start), mock.patch.object(PRODUCER.time, "sleep", side_effect=KeyboardInterrupt):
            with self.assertRaises(KeyboardInterrupt):
                self.run_python("import time; time.sleep(30)", [])
        self.assertEqual(1, len(processes))
        self.assertEqual(-signal.SIGKILL, processes[0].returncode)
        with self.assertRaises(ProcessLookupError):
            os.killpg(processes[0].pid, 0)


class SweepCliTests(unittest.TestCase):
    def test_invalid_mode_arguments_never_start_old_live_generator(self):
        with mock.patch.object(GENERATOR, "records", side_effect=AssertionError("live generator")), mock.patch.object(GENERATOR, "export_diagnostic_sweep", side_effect=AssertionError("sweep")), mock.patch("sys.stderr", new_callable=io.StringIO):
            for arguments in (
                ["--exporter", str(PYTHON)], ["--source-export-report", "/tmp/report"],
                ["--source-export-report", "/tmp/report", "--write"], ["--fixture", IDS[0]],
                ["--source-export-report", "/tmp/report", "--exporter", str(PYTHON), "--cargo-fe2o3", str(PYTHON)],
                ["--preparation-report", "/tmp/report", "--exporter", str(PYTHON)],
                ["--source-export-report", "/tmp/report", "--preparation-report", "/tmp/other"],
            ):
                with self.subTest(arguments=arguments), self.assertRaises(SystemExit) as raised:
                    GENERATOR.main(arguments)
                self.assertEqual(2, raised.exception.code)

    def test_diagnostic_exit_codes_do_not_mean_qualification(self):
        for status, expected in (("prepared-not-qualified", 0), ("exporter-failed", 1), ("invalid-preparation", 1)):
            report = {"authority": "diagnostic-only-no-qualification-authority", "summary": {"byStatus": {status: 1}}, "observations": [{"status": status}]}
            with self.subTest(status=status), mock.patch.object(GENERATOR, "export_diagnostic_sweep", return_value=report) as sweep, mock.patch("sys.stdout", new_callable=io.StringIO) as output:
                result = GENERATOR.main([
                    "--preparation-report", "/tmp/report", "--repository", str(ROOT),
                    "--exporter", str(PYTHON), "--cargo-fe2o3", str(PYTHON), "--fixture", IDS[0], "--timeout-seconds", "2",
                ])
                self.assertEqual(expected, result)
                self.assertEqual(report["authority"], json.loads(output.getvalue())["authority"])
                self.assertEqual([IDS[0]], sweep.call_args.kwargs["fixture_ids"])


if __name__ == "__main__":
    unittest.main()
