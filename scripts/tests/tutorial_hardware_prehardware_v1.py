#!/usr/bin/env python3

from __future__ import annotations

from copy import deepcopy
import importlib.util
import json
import os
from pathlib import Path
import shutil
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch


sys.dont_write_bytecode = True
SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))

import tutorial_hardware_receipt as RECEIPT  # noqa: E402


def load_runner():
    specification = importlib.util.spec_from_file_location(
        "tutorial_hardware_prehardware_runner",
        SCRIPTS / "run-tutorial-authenticated-hardware.py",
    )
    assert specification is not None and specification.loader is not None
    module = importlib.util.module_from_spec(specification)
    sys.modules[specification.name] = module
    specification.loader.exec_module(module)
    return module


RUNNER = load_runner()


class PreHardwareProtocolTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve()
        self.evidence = self.root / "evidence"
        self.evidence.mkdir()
        self.private_key = self.root / "attestor-private.pem"
        shutil.copyfile(
            SCRIPTS / "tests" / "fixtures" / "evidence-test-attestor-private.pem",
            self.private_key,
        )
        self.private_key.chmod(0o600)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    @staticmethod
    def proof_payload() -> bytes:
        body = RECEIPT.CAPABILITY_RESULT_SET_MAGIC + struct.pack("<HHI", 1, 0, 0)
        body += b"focused-proof"
        declared = len(body) + 32
        body = body[:12] + struct.pack("<I", declared) + body[16:]
        terminal = bytes.fromhex(
            RECEIPT.sha256(
                RECEIPT.CAPABILITY_RESULT_SET_DOMAIN
                + struct.pack("<Q", len(body))
                + body
            )
        )
        return body + terminal

    def add_object(self, payload: bytes) -> dict:
        reference = RECEIPT.object_reference(payload)
        path = self.evidence / reference["path"]
        path.parent.mkdir(parents=True, exist_ok=True)
        if not path.exists():
            path.write_bytes(payload)
        return reference

    def transaction(self, executable: str = "runner.sh") -> tuple[dict, dict]:
        payloads = {
            kind: f"focused-{kind}".encode("ascii")
            for kind in RECEIPT.PRE_HARDWARE_EVIDENCE_KINDS
        }
        payloads["proof-evidence"] = self.proof_payload()
        references = {
            kind: self.add_object(payload) for kind, payload in payloads.items()
        }
        digest = RECEIPT.sha256(b"focused-digest")
        compiler_input = {
            "cargoLockSha256": RECEIPT.sha256(b"cargo-lock"),
            "contractSha256": RECEIPT.sha256(b"compiler-contract"),
            "kernelSymbols": ["focused_kernel"],
            "packageManifestSha256": RECEIPT.sha256(b"package-manifest"),
            "sourceClosureSha256": references["source-closure"]["sha256"],
        }
        command = {
            "arguments": ["bound-argument"],
            "environment": [
                "FE2O3_TUTORIAL_HARDWARE_PROTOCOL=authenticated-v1"
            ],
            "executable": executable,
            "timeoutSeconds": 30,
            "workingDirectory": ".",
        }
        request = {
            "candidate": {
                "compilerCommit": "1" * 40,
                "compilerTree": "2" * 40,
                "worktreeClean": True,
            },
            "capabilityKernel": {
                "kernelSymbol": "focused_kernel",
                "lessonIds": ["focused-lesson"],
            },
            "fixture": {
                "compilerInput": compiler_input,
                "fixtureId": "focused-fixture",
                "hardwareCommand": command,
                "hardwareLane": "mi300x",
                "hardwareReservation": "focused-reservation",
                "target": "gfx942",
            },
            "hardwareReceiptChallenge": {
                "nonce": RECEIPT.sha256(b"focused-nonce"),
                "reservationIdentity": "focused-reservation",
                "schema": RECEIPT.CHALLENGE_SCHEMA,
                "transportSchema": RECEIPT.TRANSPORT_SCHEMA,
            },
            "requestBindingSha256": RECEIPT.sha256(b"focused-request"),
        }
        production = {
            "artifactInspectionSha256": references["artifact-inspection"]["sha256"],
            "artifactSha256": references["artifact"]["sha256"],
            "capabilityAnalysisSha256": references["capability-analysis"]["sha256"],
            "capabilityClosureSha256": digest,
            "compilerPolicySha256": digest,
            "finalOptimizedKirSha256": digest,
            "launchContractSha256": digest,
            "loweringIdentitySha256": digest,
            "machineRefinementSha256": digest,
            "numericalPolicyEvidenceSha256": references["numerical-policy"]["sha256"],
            "proofCheckerSha256": RECEIPT._authenticated_checker_evidence_identity(
                payloads["proof-checker"]
            ),
            "proofEvidenceSha256": RECEIPT._capability_result_set_identity(
                payloads["proof-evidence"]
            ),
            "proofObligationSetSha256": digest,
            "sealedResultSha256": digest,
            "sourceMirIdentitySha256": digest,
            "sourceMirToKirRefinementSha256": digest,
            "targetIdentitySha256": digest,
        }
        record = {
            "capabilityClosure": {},
            "compilerInput": {
                key: compiler_input[key]
                for key in (
                    "cargoLockSha256",
                    "contractSha256",
                    "packageManifestSha256",
                    "sourceClosureSha256",
                )
            },
            "evidenceFiles": references,
            "fixtureId": "focused-fixture",
            "graph": {},
            "hardware": {
                "commandSha256": RECEIPT._command_identity(command),
                "lane": "mi300x",
                "status": "pending-authenticated-observation",
                "target": "gfx942",
                "timeoutSeconds": 30,
            },
            "kernelSymbol": "focused_kernel",
            "lessonIds": ["focused-lesson"],
            "pendingEvidence": RECEIPT.PENDING_EVIDENCE_KINDS,
            "preHardwareBindingSha256": "0" * 64,
            "productionEvidence": production,
            "productionTransaction": {
                "allowsFallback": False,
                "allowsPipelineSelection": False,
                "pipelineEntry": "rustc-codegen-fe2o3::production_pipeline",
                "policyVersion": 4,
                "status": "sealed-production-complete",
                "transactionSha256": RECEIPT.sha256(b"focused-transaction"),
            },
            "proof": {},
            "schema": RECEIPT.PRE_HARDWARE_RECORD_SCHEMA,
            "simulator": {},
            "target": "gfx942",
            "targetDecision": {},
        }
        subject = {
            key: value
            for key, value in record.items()
            if key != "preHardwareBindingSha256"
        }
        record["preHardwareBindingSha256"] = RECEIPT._domain_identity(
            RECEIPT.PRE_HARDWARE_RECORD_DOMAIN, subject
        )
        return request, record

    @staticmethod
    def platform(request: dict, record: dict) -> dict[str, bytes]:
        common = {
            "authority": RECEIPT.AUTHORITY,
            "challengeNonce": request["hardwareReceiptChallenge"]["nonce"],
            "preHardwareRecordSha256": RECEIPT.pre_hardware_record_identity(record),
            "target": request["fixture"]["target"],
        }
        driver = {
            **common,
            "deviceNode": {"major": 235, "minor": 0, "path": "/dev/kfd"},
            "devices": [
                {
                    "deviceId": 29857,
                    "gfxTargetVersion": 90402,
                    "nodeId": 1,
                    "simdCount": 304,
                    "vendorId": 0x1002,
                    "wavefrontSize": 64,
                }
            ],
            "kernel": {
                "machine": "x86_64",
                "release": "focused-kernel",
                "system": "Linux",
            },
            "module": {
                "name": "amdgpu",
                "refCount": 1,
                "sizeBytes": 1024,
                "state": "Live",
            },
            "schema": RECEIPT.DRIVER_OBSERVATION_SCHEMA,
        }
        components = {}
        for name, relative in {
            "hipRuntime": "lib/libamdhip64.so",
            "hsaRuntime": "lib/libhsa-runtime64.so.1",
            "rocminfo": "bin/rocminfo",
            "rocmVersion": ".info/version",
        }.items():
            payload = f"focused-{name}".encode("ascii")
            components[name] = {
                "bytes": len(payload),
                "path": f"/opt/rocm/{relative}",
                "resolvedPath": f"/opt/rocm-7.2/{relative}",
                "sha256": RECEIPT.sha256(payload),
            }
        runtime = {
            **common,
            "components": components,
            "rocmRelease": "7.2.0",
            "rocmRoot": {"path": "/opt/rocm", "resolvedPath": "/opt/rocm-7.2"},
            "schema": RECEIPT.RUNTIME_OBSERVATION_SCHEMA,
        }
        return {"driver": RECEIPT.canonical(driver), "runtime": RECEIPT.canonical(runtime)}

    @staticmethod
    def runner_observations(
        request: dict, record: dict, platform: dict[str, bytes]
    ) -> dict[str, bytes]:
        fixture = request["fixture"]
        artifact = record["evidenceFiles"]["artifact"]["sha256"]
        same = RECEIPT.sha256(b"same")
        output = RECEIPT.sha256(b"output")
        isa = {
            "artifactSha256": artifact,
            "disassembly": "focused exact ISA\n",
            "inspectionToolSha256": RECEIPT.sha256(b"objdump"),
            "kernelSymbols": fixture["compilerInput"]["kernelSymbols"],
            "llvmModuleSha256": record["evidenceFiles"]["llvm-module"]["sha256"],
            "schema": RECEIPT.ISA_OBSERVATION_SCHEMA,
            "target": fixture["target"],
        }
        resource = {
            "artifactSha256": artifact,
            "inspectionToolSha256": RECEIPT.sha256(b"readobj"),
            "kernelSymbols": fixture["compilerInput"]["kernelSymbols"],
            "ldsBytes": 0,
            "registersPerWorkgroup": 256,
            "schema": RECEIPT.RESOURCE_OBSERVATION_SCHEMA,
            "scratchBytes": 0,
            "target": fixture["target"],
            "workgroupSize": [256, 1, 1],
        }
        result = {
            "artifactSha256": artifact,
            "authority": "observation-only",
            "candidate": request["candidate"],
            "checks": {
                "canariesChecked": True,
                "completeOutputChecked": True,
                "inputsUnchangedChecked": True,
                "paddingChecked": True,
                "timedOut": False,
            },
            "commandSha256": RECEIPT._command_identity(fixture["hardwareCommand"]),
            "driverIdentitySha256": RECEIPT.sha256(platform["driver"]),
            "fixtureId": fixture["fixtureId"],
            "kernelSymbols": fixture["compilerInput"]["kernelSymbols"],
            "lane": fixture["hardwareLane"],
            "observedIdentities": {
                "canaryAfterSha256": same,
                "canaryBeforeSha256": same,
                "expectedOutputSha256": output,
                "inputAfterSha256": same,
                "inputBeforeSha256": same,
                "observedOutputSha256": output,
                "paddingAfterSha256": same,
                "paddingBeforeSha256": same,
            },
            "outcome": "passed",
            "reservationIdentity": fixture["hardwareReservation"],
            "runtimeIdentitySha256": RECEIPT.sha256(platform["runtime"]),
            "schema": RECEIPT.RESULT_OBSERVATION_SCHEMA,
            "target": fixture["target"],
            "transactionSha256": record["productionTransaction"]["transactionSha256"],
        }
        return {
            "isa": RECEIPT.canonical(isa),
            "resource": RECEIPT.canonical(resource),
            "result": RECEIPT.canonical(result),
        }

    def archive(self, request: dict, record: dict) -> bytes:
        platform = self.platform(request, record)
        observed = self.runner_observations(request, record, platform)
        scratch = self.root / "scratch"
        spool = self.root / "spool"
        scratch.mkdir()
        RECEIPT.prepare_transport(
            request,
            record,
            self.evidence,
            observed["isa"],
            observed["resource"],
            observed["result"],
            scratch,
            spool,
            "focused-attestor",
            self.private_key,
            driver_observation=platform["driver"],
            runtime_observation=platform["runtime"],
        )
        shutil.rmtree(scratch)
        return RECEIPT.finalize_transport(
            spool, scratch, self.private_key, request, record, self.evidence
        )

    def test_archive_matches_final_rust_object_rosters(self) -> None:
        request, record = self.transaction()
        archive = self.archive(request, record)
        index, objects = RECEIPT._archive_entries(archive)
        run_ref = RECEIPT.validate_reference(index["runReceipt"], "run receipt")
        run = json.loads(objects[run_ref["sha256"]])
        self.assertEqual(
            set(RECEIPT.COMPILER_OBSERVATION_KINDS)
            | set(RECEIPT.PLATFORM_OBSERVATION_KINDS)
            | {"isa", "resource", "result"},
            set(run["observations"]),
        )
        self.assertEqual(
            RECEIPT.pre_hardware_record_identity(record),
            run["preHardwareRecordSha256"],
        )
        for kind in RECEIPT.VERIFIER_ONLY_COMPILER_KINDS:
            reference = record["evidenceFiles"][kind]
            self.assertIn(reference["sha256"], objects)
            self.assertNotIn(kind, run["observations"])
        policy_path = self.root / "policy.json"
        policy_path.write_bytes(
            RECEIPT.canonical(
                RECEIPT.trust_policy_document(
                    [
                        {
                            "attestorIdentity": "focused-attestor",
                            "lane": "mi300x",
                            "publicKeyPath": str(
                                SCRIPTS
                                / "tests"
                                / "fixtures"
                                / "evidence-test-attestor-public.pem"
                            ),
                            "reservationIdentity": "focused-reservation",
                            "target": "gfx942",
                        }
                    ]
                )
            )
        )
        policy_path.chmod(0o600)
        archive_path = self.root / RECEIPT.TRANSPORT_NAME
        archive_path.write_bytes(archive)
        validated = RECEIPT.validate_and_ingest(
            archive_path,
            request,
            record,
            RECEIPT.load_trust_policy(policy_path),
            RECEIPT.object_reference,
            set(),
        )
        self.assertEqual(run_ref["sha256"], validated.run_receipt_sha256)

    def test_stale_platform_and_record_bindings_fail_closed(self) -> None:
        request, record = self.transaction()
        platform = self.platform(request, record)
        driver = json.loads(platform["driver"])
        driver["preHardwareRecordSha256"] = RECEIPT.sha256(b"other-record")
        references = {
            "driver": {
                **RECEIPT.object_reference(RECEIPT.canonical(driver)),
                "_payload": RECEIPT.canonical(driver),
            },
            "runtime": {
                **RECEIPT.object_reference(platform["runtime"]),
                "_payload": platform["runtime"],
            },
        }
        with self.assertRaisesRegex(
            RECEIPT.HardwareReceiptError, "driver observation is stale"
        ):
            RECEIPT._validate_platform_observations(request, record, references)
        mutated = deepcopy(record)
        mutated["target"] = "gfx950"
        with self.assertRaisesRegex(RECEIPT.HardwareReceiptError, "binding is stale"):
            RECEIPT._validate_pre_hardware_record(request, mutated)

    def test_adapter_exposes_fresh_platform_measurements_not_compiler_inputs(self) -> None:
        repository = self.root / "repository"
        repository.mkdir()
        runner_path = repository / "runner.sh"
        runner_path.write_text("#!/bin/sh\nexit 0\n", encoding="ascii")
        runner_path.chmod(0o700)
        request, record = self.transaction("runner.sh")
        platform = self.platform(request, record)
        observed = self.runner_observations(request, record, platform)
        request_path = self.root / "request.json"
        record_path = self.root / "record.json"
        request_path.write_bytes(RECEIPT.canonical(request))
        record_path.write_bytes(RECEIPT.canonical(record))
        remote_temp = self.root / "remote-temp"
        remote_temp.mkdir()

        def run_inner(_runner, _arguments, _timeout, environment, _stdout, _stderr):
            for name, variable in RUNNER.PLATFORM_INPUT_ENVIRONMENT.items():
                path = Path(environment[variable])
                self.assertTrue(path.is_relative_to(remote_temp))
                self.assertEqual(platform[name], path.read_bytes())
            view = json.loads(
                Path(environment["FE2O3_TUTORIAL_HARDWARE_RECORD_INPUT"]).read_bytes()
            )
            self.assertEqual(
                "fe2o3-tutorial-hardware-runner-record-view-v1", view["schema"]
            )
            self.assertNotIn("preHardwareBindingSha256", view)
            self.assertEqual(
                RECEIPT.pre_hardware_record_identity(record),
                view["preHardwareRecordSha256"],
            )
            for name, variable in RUNNER.OBSERVATION_OUTPUTS.items():
                Path(environment[variable]).write_bytes(observed[name])

        environment = {
            "FE2O3_TUTORIAL_HARDWARE_ATTESTOR_IDENTITY": "focused-attestor",
            "FE2O3_TUTORIAL_HARDWARE_ATTESTOR_PRIVATE_KEY": str(self.private_key),
            "FE2O3_TUTORIAL_HARDWARE_EVIDENCE_ROOT": str(self.evidence),
            "FE2O3_TUTORIAL_HARDWARE_PRE_RECORD": str(record_path),
            "FE2O3_TUTORIAL_HARDWARE_PROTOCOL": "authenticated-v1",
            "FE2O3_TUTORIAL_HARDWARE_REQUEST": str(request_path),
            "FE2O3_TUTORIAL_HARDWARE_RESERVATION": "focused-reservation",
            "FE2O3_TUTORIAL_HARDWARE_TEMP_ROOT": str(remote_temp),
        }
        previous_root = RUNNER.REPO_ROOT
        try:
            RUNNER.REPO_ROOT = repository
            with patch.dict(os.environ, environment, clear=False), patch.object(
                RUNNER, "measure_platform_observations", return_value=platform
            ), patch.object(RUNNER, "run_inner", side_effect=run_inner):
                archive = RUNNER.execute(str(runner_path), ["bound-argument"])
        finally:
            RUNNER.REPO_ROOT = previous_root
        self.assertTrue(archive.startswith(b"PK"))
        self.assertEqual([], list(remote_temp.iterdir()))


if __name__ == "__main__":
    unittest.main()
