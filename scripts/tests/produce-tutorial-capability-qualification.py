#!/usr/bin/env python3

from __future__ import annotations

from copy import deepcopy
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import shlex
import stat
import subprocess
import sys
import tempfile
import unittest
import warnings
from unittest.mock import patch
import zipfile


sys.dont_write_bytecode = True
SCRIPTS = Path(__file__).resolve().parents[1]
ROOT = SCRIPTS.parent
sys.path.insert(0, str(SCRIPTS))


def load(name: str, path: Path):
    specification = importlib.util.spec_from_file_location(name, path)
    assert specification is not None and specification.loader is not None
    module = importlib.util.module_from_spec(specification)
    sys.modules[name] = module
    specification.loader.exec_module(module)
    return module


PRODUCER = load(
    "produce_tutorial_capability_qualification_tests",
    SCRIPTS / "produce-tutorial-capability-qualification.py",
)
PROMOTION_FIXTURES = load(
    "tutorial_capability_promotion_fixture_builder",
    Path(__file__).with_name("tutorial_capability_promotion.py"),
)
HARDWARE_RUNNER = load(
    "tutorial_authenticated_hardware_runner_tests",
    SCRIPTS / "run-tutorial-authenticated-hardware.py",
)
PROVISIONER = load(
    "tutorial_hardware_attestor_provisioner_tests",
    SCRIPTS / "provision-tutorial-hardware-attestor.py",
)


def canonical(value: object) -> bytes:
    return (
        json.dumps(
            value,
            allow_nan=False,
            ensure_ascii=True,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("ascii")
        + b"\n"
    )


class TutorialCapabilityQualificationProducerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.temporary = tempfile.TemporaryDirectory()
        cls.root = Path(cls.temporary.name)
        cls.source = cls.root / "source"
        cls.archive = cls.root / "archive"
        cls.source.mkdir()
        cls.archive.mkdir()
        cls.private_key = cls.root / "hardware-attestor-private.pem"
        shutil.copyfile(
            SCRIPTS / "tests" / "fixtures" / "evidence-test-attestor-private.pem",
            cls.private_key,
        )
        cls.private_key.chmod(0o600)
        cls.public_key = (
            SCRIPTS / "tests" / "fixtures" / "evidence-test-attestor-public.pem"
        )
        cls.policy_path = cls.root / "hardware-policy-v1.json"
        cls.policy_path.write_bytes(
            PRODUCER.hardware_receipt_contract.canonical(
                PRODUCER.hardware_receipt_contract.trust_policy_document(
                    [
                        {
                            "attestorIdentity": "tutorial-test-attestor-v1",
                            "lane": "mi300x",
                            "publicKeyPath": str(cls.public_key),
                            "reservationIdentity": "test-mi300x-reservation",
                            "target": "gfx942",
                        },
                        {
                            "attestorIdentity": "tutorial-test-attestor-v1",
                            "lane": "mi350",
                            "publicKeyPath": str(cls.public_key),
                            "reservationIdentity": "test-mi350-reservation",
                            "target": "gfx950",
                        },
                    ]
                )
            )
        )
        cls.policy_path.chmod(0o600)
        cls.hardware_policy = PRODUCER.hardware_receipt_contract.load_trust_policy(
            cls.policy_path
        )
        cls.raw, cls.document = PROMOTION_FIXTURES.manifest_with_v8_suites()
        cls.candidate = {
            "compilerCommit": "1" * 40,
            "compilerTree": "2" * 40,
            "worktreeClean": True,
        }
        batch = PROMOTION_FIXTURES.build_batch(
            cls.source, cls.raw, cls.document, cls.candidate
        )
        references = [batch["semanticQualification"]]
        references.extend(
            reference
            for record in batch["records"]
            for reference in record["evidenceFiles"].values()
        )
        replacements: dict[tuple[int, str], dict] = {}
        for reference in references:
            key = (reference["bytes"], reference["sha256"])
            if key not in replacements:
                payload = (cls.source / reference["path"]).read_bytes()
                replacements[key] = PRODUCER._write_object(cls.archive, payload)
            reference.clear()
            reference.update(replacements[key])
        for record in batch["records"]:
            record["recordBindingSha256"] = (
                PRODUCER.promotion_contract.record_binding_sha256(record)
            )
        batch["batchBindingSha256"] = PRODUCER.promotion_contract.batch_binding_sha256(
            batch
        )
        cls.batch = batch
        cls.fixtures = {
            item["fixtureId"]: item for item in cls.document["compilerFixtures"]
        }
        cls.kernels = {
            item["fixtureId"]: item for item in cls.document["capabilityKernels"]
        }
        cls.manifest_identity = {
            "corpusContractSha256": PRODUCER.promotion_contract.corpus_contract_sha256(
                cls.document
            ),
            "path": PRODUCER.MANIFEST_RELATIVE,
            "rawSha256": PRODUCER._sha256(cls.raw),
        }

    @classmethod
    def tearDownClass(cls) -> None:
        cls.temporary.cleanup()

    def request_and_record(self) -> tuple[dict, dict]:
        record = deepcopy(self.batch["records"][0])
        fixture_id = record["fixtureId"]
        request = PRODUCER.transaction_request(
            self.fixtures[fixture_id],
            self.kernels[fixture_id],
            self.candidate,
            self.manifest_identity,
            record["evidenceFiles"]["simulator"],
            "test-mi300x-reservation",
        )
        return request, self.as_pre_hardware_record(record)

    def as_pre_hardware_record(self, record: dict) -> dict:
        record.pop("recordBindingSha256")
        for kind in PRODUCER.hardware_receipt_contract.PENDING_EVIDENCE_KINDS:
            record["evidenceFiles"].pop(kind, None)
        record["productionEvidence"].pop("hardwareEvidenceSha256")
        record.pop("negativeFixtures")
        record["hardware"] = {
            "commandSha256": record["hardware"]["commandSha256"],
            "lane": record["hardware"]["lane"],
            "status": "pending-authenticated-observation",
            "target": record["hardware"]["target"],
            "timeoutSeconds": record["hardware"]["timeoutSeconds"],
        }
        record["pendingEvidence"] = (
            PRODUCER.hardware_receipt_contract.PENDING_EVIDENCE_KINDS
        )
        record["schema"] = PRODUCER.hardware_receipt_contract.PRE_HARDWARE_RECORD_SCHEMA
        record["preHardwareBindingSha256"] = "0" * 64
        record["preHardwareBindingSha256"] = PRODUCER.pre_hardware_binding_sha256(
            record
        )
        return record

    def platform_observations(
        self, request: dict, record: dict
    ) -> tuple[bytes, bytes]:
        contract = PRODUCER.hardware_receipt_contract
        common = {
            "authority": contract.AUTHORITY,
            "challengeNonce": request["hardwareReceiptChallenge"]["nonce"],
            "preHardwareRecordSha256": contract.pre_hardware_record_identity(record),
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
                "release": "test-kernel",
                "system": "Linux",
            },
            "module": {
                "name": "amdgpu",
                "refCount": 1,
                "sizeBytes": 1024,
                "state": "Live",
            },
            "schema": contract.DRIVER_OBSERVATION_SCHEMA,
        }
        components = {}
        for name, relative in {
            "hipRuntime": "lib/libamdhip64.so",
            "hsaRuntime": "lib/libhsa-runtime64.so.1",
            "rocminfo": "bin/rocminfo",
            "rocmVersion": ".info/version",
        }.items():
            payload = f"test-only-{name}".encode("ascii")
            components[name] = {
                "bytes": len(payload),
                "path": f"/opt/rocm/{relative}",
                "resolvedPath": f"/opt/rocm-7.2/{relative}",
                "sha256": PRODUCER._sha256(payload),
            }
        runtime = {
            **common,
            "components": components,
            "rocmRelease": "7.2.0",
            "rocmRoot": {"path": "/opt/rocm", "resolvedPath": "/opt/rocm-7.2"},
            "schema": contract.RUNTIME_OBSERVATION_SCHEMA,
        }
        return canonical(driver), canonical(runtime)

    def runner_observations(
        self, request: dict, record: dict
    ) -> tuple[bytes, bytes, bytes, bytes, bytes]:
        fixture = request["fixture"]
        files = record["evidenceFiles"]
        driver, runtime = self.platform_observations(request, record)
        artifact = files["artifact"]["sha256"]
        tool = PRODUCER._sha256(b"test-only-inspection-tool")
        isa = {
            "artifactSha256": artifact,
            "disassembly": "test-only exact ISA\n",
            "inspectionToolSha256": tool,
            "kernelSymbols": fixture["compilerInput"]["kernelSymbols"],
            "llvmModuleSha256": files["llvm-module"]["sha256"],
            "schema": PRODUCER.hardware_receipt_contract.ISA_OBSERVATION_SCHEMA,
            "target": fixture["target"],
        }
        resource = {
            "artifactSha256": artifact,
            "inspectionToolSha256": tool,
            "kernelSymbols": fixture["compilerInput"]["kernelSymbols"],
            "ldsBytes": 0,
            "registersPerWorkgroup": 256,
            "schema": PRODUCER.hardware_receipt_contract.RESOURCE_OBSERVATION_SCHEMA,
            "scratchBytes": 0,
            "target": fixture["target"],
            "workgroupSize": [256, 1, 1],
        }
        same = PRODUCER._sha256(b"test-only-same-state")
        output = PRODUCER._sha256(b"test-only-output")
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
            "commandSha256": PRODUCER.promotion_contract.command_sha256(
                fixture["hardwareCommand"]
            ),
            "driverIdentitySha256": PRODUCER._sha256(driver),
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
            "runtimeIdentitySha256": PRODUCER._sha256(runtime),
            "schema": PRODUCER.hardware_receipt_contract.RESULT_OBSERVATION_SCHEMA,
            "target": fixture["target"],
            "transactionSha256": record["productionTransaction"]["transactionSha256"],
        }
        return driver, runtime, canonical(isa), canonical(resource), canonical(result)

    def run_authenticated_adapter(
        self,
        *,
        mode: str = "complete",
        mutate_result=None,
        temporary_inside_repository: bool = False,
    ) -> tuple[bytes, dict, dict]:
        request, record = self.request_and_record()
        with tempfile.TemporaryDirectory(dir=self.root) as temporary:
            base = Path(temporary)
            repository = base / "repository"
            runner = repository / "examples" / "test" / "run-gfx942.sh"
            runner.parent.mkdir(parents=True)
            observation_inputs = base / "observation-inputs"
            observation_inputs.mkdir()
            remote_temporary = (
                repository / "remote-temporary"
                if temporary_inside_repository
                else base / "remote-temporary"
            )
            remote_temporary.mkdir()
            request_path = base / "request-v1.json"
            record_path = base / "record-v1.json"

            request["fixture"]["hardwareCommand"] = {
                "arguments": ["bound-argument"],
                "environment": ["FE2O3_TUTORIAL_HARDWARE_PROTOCOL=authenticated-v1"],
                "executable": "examples/test/run-gfx942.sh",
                "timeoutSeconds": 10,
                "workingDirectory": ".",
            }
            request["requestBindingSha256"] = PRODUCER.request_binding_sha256(request)
            record["hardware"]["commandSha256"] = (
                PRODUCER.promotion_contract.command_sha256(
                    request["fixture"]["hardwareCommand"]
                )
            )
            record["hardware"]["timeoutSeconds"] = request["fixture"][
                "hardwareCommand"
            ]["timeoutSeconds"]
            record["preHardwareBindingSha256"] = PRODUCER.pre_hardware_binding_sha256(
                record
            )
            driver, runtime, isa, resource, result = self.runner_observations(
                request, record
            )
            result_value = json.loads(result)
            if mutate_result is not None:
                mutate_result(result_value)
            source_paths = {
                "isa": observation_inputs / "isa.json",
                "resource": observation_inputs / "resource.json",
                "result": observation_inputs / "result.json",
            }
            source_paths["isa"].write_bytes(isa)
            source_paths["resource"].write_bytes(resource)
            source_paths["result"].write_bytes(canonical(result_value))
            request_path.write_bytes(canonical(request))
            record_path.write_bytes(canonical(record))

            required_inputs = sorted(
                [
                    *HARDWARE_RUNNER.COMPILER_INPUT_ENVIRONMENT.values(),
                    *HARDWARE_RUNNER.PLATFORM_INPUT_ENVIRONMENT.values(),
                    "FE2O3_TUTORIAL_HARDWARE_REQUEST_INPUT",
                    "FE2O3_TUTORIAL_HARDWARE_RECORD_INPUT",
                ]
            )
            lines = [
                "#!/bin/sh",
                "set -eu",
                '[ "$1" = bound-argument ]',
                'if [ "${FE2O3_TUTORIAL_HARDWARE_ATTESTOR_PRIVATE_KEY+set}" = set ]; then exit 70; fi',
            ]
            lines.extend(f'test -f "${{{name}:?}}"' for name in required_inputs)
            if mode != "failure":
                lines.extend(
                    [
                        *(
                            []
                            if mode == "omit-isa"
                            else [
                                f"cp {shlex.quote(str(source_paths['isa']))} "
                                '"$FE2O3_TUTORIAL_HARDWARE_ISA_OBSERVATION_OUTPUT"'
                            ]
                        ),
                        *(
                            []
                            if mode == "omit-resource"
                            else [
                                f"cp {shlex.quote(str(source_paths['resource']))} "
                                '"$FE2O3_TUTORIAL_HARDWARE_RESOURCE_OBSERVATION_OUTPUT"'
                            ]
                        ),
                    ]
                )
                if mode in {"hardlink-result", "symlink-result"}:
                    lines.append(
                        "ln "
                        + ("-s " if mode == "symlink-result" else "")
                        + f"{shlex.quote(str(source_paths['result']))} "
                        '"$FE2O3_TUTORIAL_HARDWARE_RESULT_OBSERVATION_OUTPUT"'
                    )
                elif mode != "omit-result":
                    lines.append(
                        f"cp {shlex.quote(str(source_paths['result']))} "
                        '"$FE2O3_TUTORIAL_HARDWARE_RESULT_OBSERVATION_OUTPUT"'
                    )
            if mode == "replace-output":
                lines.extend(
                    [
                        'rm -f "$FE2O3_TUTORIAL_HARDWARE_SCRATCH/runner-stdout.bin"',
                        ': >"$FE2O3_TUTORIAL_HARDWARE_SCRATCH/runner-stdout.bin"',
                        "head -c 4096 /dev/zero",
                    ]
                )
            if mode == "live-descendant":
                lines.append("sleep 30 &")
            if mode == "mutate-request":
                lines.append(
                    "printf '%s\\n' '{\"substituted\":true}' > "
                    '"$FE2O3_TUTORIAL_HARDWARE_REQUEST_INPUT"'
                )
            lines.append("exit 9" if mode == "failure" else "exit 0")
            runner.write_text("\n".join(lines) + "\n", encoding="ascii")
            runner.chmod(0o755)

            environment = {
                "FE2O3_TUTORIAL_HARDWARE_ATTESTOR_IDENTITY": (
                    "tutorial-test-attestor-v1"
                ),
                "FE2O3_TUTORIAL_HARDWARE_ATTESTOR_PRIVATE_KEY": str(self.private_key),
                "FE2O3_TUTORIAL_HARDWARE_EVIDENCE_ROOT": str(self.archive),
                "FE2O3_TUTORIAL_HARDWARE_PROTOCOL": "authenticated-v1",
                "FE2O3_TUTORIAL_HARDWARE_PRE_RECORD": str(record_path),
                "FE2O3_TUTORIAL_HARDWARE_REQUEST": str(request_path),
                "FE2O3_TUTORIAL_HARDWARE_RESERVATION": ("test-mi300x-reservation"),
                "FE2O3_TUTORIAL_HARDWARE_TEMP_ROOT": str(remote_temporary),
            }
            previous_root = HARDWARE_RUNNER.REPO_ROOT
            try:
                HARDWARE_RUNNER.REPO_ROOT = repository
                with patch.dict(os.environ, environment, clear=False), patch.object(
                    HARDWARE_RUNNER,
                    "measure_platform_observations",
                    return_value={"driver": driver, "runtime": runtime},
                ):
                    archive = HARDWARE_RUNNER.execute(str(runner), ["bound-argument"])
            finally:
                HARDWARE_RUNNER.REPO_ROOT = previous_root
                self.assertEqual([], list(remote_temporary.iterdir()))
            return archive, request, record

    def make_export(
        self,
        *,
        mutate_envelope=None,
        mutate_hardware=None,
        mutate_record=None,
        mutate_transport=None,
    ) -> tuple[tempfile.TemporaryDirectory, Path, Path, dict]:
        request, record = self.request_and_record()
        temporary = tempfile.TemporaryDirectory(dir=self.root)
        export = Path(temporary.name) / "export"
        destination = Path(temporary.name) / "destination"
        export.mkdir()
        destination.mkdir()
        for kind, reference in record["evidenceFiles"].items():
            if kind == "hardware":
                continue
            source = self.archive / reference["path"]
            target = export / reference["path"]
            target.parent.mkdir(parents=True, exist_ok=True)
            if not target.exists():
                shutil.copyfile(source, target)
        scratch = Path(temporary.name) / "runner-scratch"
        spool = Path(temporary.name) / "receipt-spool"
        scratch.mkdir()
        driver, runtime, isa, resource, result = self.runner_observations(
            request, record
        )
        PRODUCER.hardware_receipt_contract.prepare_transport(
            request,
            record,
            export,
            isa,
            resource,
            result,
            scratch,
            spool,
            "tutorial-test-attestor-v1",
            self.private_key,
            driver_observation=driver,
            runtime_observation=runtime,
        )
        shutil.rmtree(scratch)
        transport = PRODUCER.hardware_receipt_contract.finalize_transport(
            spool, scratch, self.private_key, request, record, export
        )
        if mutate_transport is not None:
            transport = mutate_transport(transport)
        (export / PRODUCER.hardware_receipt_contract.TRANSPORT_NAME).write_bytes(
            transport
        )
        self.assertFalse(scratch.exists())
        self.assertFalse(spool.exists())
        if mutate_transport is None:
            index, objects = PRODUCER.hardware_receipt_contract._archive_entries(
                transport
            )
            _, _, cleanup = PRODUCER.hardware_receipt_contract._archive_object(
                objects,
                index["cleanupReceipt"],
                "test cleanup receipt",
                json_object=True,
            )
            assert cleanup is not None
            hardware_reference = cleanup["hardwareEvidence"]
            hardware = json.loads(objects[hardware_reference["sha256"]])
            if mutate_hardware is not None:
                mutate_hardware(hardware)
            hardware_reference = PRODUCER._write_object(export, canonical(hardware))
            final_record = PRODUCER.complete_pre_hardware_record(
                record, hardware_reference, hardware
            )
        else:
            final_record = deepcopy(self.batch["records"][0])
        if mutate_record is not None:
            mutate_record(final_record)
        final_record["recordBindingSha256"] = (
            PRODUCER.promotion_contract.record_binding_sha256(final_record)
        )
        envelope = {
            "candidate": request["candidate"],
            "fixtureId": request["fixture"]["fixtureId"],
            "record": final_record,
            "requestBindingSha256": request["requestBindingSha256"],
            "schema": PRODUCER.EXPORT_SCHEMA,
        }
        if mutate_envelope is not None:
            mutate_envelope(envelope)
        (export / PRODUCER.RESULT_NAME).write_bytes(canonical(envelope))
        return temporary, export, destination, request, record

    def make_pre_export(self) -> tuple[tempfile.TemporaryDirectory, Path, dict, dict]:
        request, record = self.request_and_record()
        temporary = tempfile.TemporaryDirectory(dir=self.root)
        export = Path(temporary.name) / "pre-hardware"
        export.mkdir()
        for kind, reference in record["evidenceFiles"].items():
            if kind == "simulator":
                continue
            source = self.archive / reference["path"]
            destination = export / reference["path"]
            destination.parent.mkdir(parents=True, exist_ok=True)
            if not destination.exists():
                shutil.copyfile(source, destination)
        envelope = {
            "candidate": request["candidate"],
            "fixtureId": request["fixture"]["fixtureId"],
            "record": record,
            "requestBindingSha256": request["requestBindingSha256"],
            "schema": PRODUCER.PRE_HARDWARE_EXPORT_SCHEMA,
        }
        (export / PRODUCER.PRE_HARDWARE_RESULT_NAME).write_bytes(canonical(envelope))
        return temporary, export, request, record

    def assert_export_rejected(self, pattern: str, **mutations) -> None:
        temporary, export, destination, request, pre_record = self.make_export(
            **mutations
        )
        with temporary:
            with self.assertRaisesRegex(PRODUCER.QualificationProducerError, pattern):
                PRODUCER.validate_transaction_export(
                    export,
                    export,
                    pre_record,
                    request,
                    destination,
                    self.hardware_policy,
                    set(),
                )

    def test_complete_47_record_batch_is_accepted_by_promotion_collector(self) -> None:
        fixture_ids = sorted(self.fixtures)
        assembled = PRODUCER.assemble_batch(
            self.batch["records"],
            self.candidate,
            self.manifest_identity,
            self.batch["semanticQualification"],
            fixture_ids,
        )
        validated = PRODUCER.promotion_contract.validate_batch(
            self.document,
            self.raw,
            assembled,
            self.candidate,
            ROOT,
            self.archive,
        )
        self.assertEqual(47, len(validated.records))

    def test_pre_hardware_export_contains_no_predicted_hardware_identity(self) -> None:
        temporary, export, request, record = self.make_pre_export()
        with temporary:
            PRODUCER.hardware_receipt_contract._validate_pre_hardware_record(
                request, record
            )
            envelope = json.loads(
                (export / PRODUCER.PRE_HARDWARE_RESULT_NAME).read_bytes()
            )
            self.assertEqual(record, envelope["record"])
            for kind in PRODUCER.hardware_receipt_contract.PENDING_EVIDENCE_KINDS:
                self.assertNotIn(kind, record["evidenceFiles"])
            self.assertNotIn("hardwareEvidenceSha256", record["productionEvidence"])
            self.assertEqual(
                "pending-authenticated-observation", record["hardware"]["status"]
            )

    def test_exact_compiler_export_is_consumed_without_rebinding(self) -> None:
        temporary, export, destination, request, pre_record = self.make_export()
        with temporary:
            archive_path = export / PRODUCER.hardware_receipt_contract.TRANSPORT_NAME
            archive_payload = archive_path.read_bytes()
            observed = PRODUCER.hardware_receipt_contract.validate_and_ingest(
                archive_path,
                request,
                pre_record,
                self.hardware_policy,
                lambda payload: PRODUCER._write_object(destination, payload),
                set(),
            )
            self.assertEqual(
                PRODUCER.hardware_receipt_contract.pre_hardware_record_identity(
                    pre_record
                ),
                json.loads(
                    (
                        destination
                        / PRODUCER._object_relative(observed.run_receipt_sha256)
                    ).read_bytes()
                )["preHardwareRecordSha256"],
            )
            archive_reference = PRODUCER.hardware_receipt_contract.object_reference(
                archive_payload
            )
            self.assertEqual(
                archive_payload,
                (destination / archive_reference["path"]).read_bytes(),
            )
            hardware = json.loads(
                (
                    destination / observed.hardware_evidence_reference["path"]
                ).read_bytes()
            )
            for key in ("isaInspectionSha256", "resourceUsageSha256"):
                digest = hardware[key]
                path = destination / PRODUCER._object_relative(digest)
                self.assertTrue(path.is_file())
                self.assertEqual(digest, PRODUCER._sha256(path.read_bytes()))

    def test_authenticated_adapter_emits_valid_archive_without_residue(self) -> None:
        archive, request, record = self.run_authenticated_adapter()
        index, objects = PRODUCER.hardware_receipt_contract._archive_entries(archive)
        _, _, cleanup = PRODUCER.hardware_receipt_contract._archive_object(
            objects,
            index["cleanupReceipt"],
            "test cleanup receipt",
            json_object=True,
        )
        assert cleanup is not None
        with tempfile.TemporaryDirectory(dir=self.root) as temporary:
            root = Path(temporary)
            archive_path = root / "hardware-receipt.zip"
            evidence = root / "evidence"
            evidence.mkdir()
            archive_path.write_bytes(archive)
            validated = PRODUCER.hardware_receipt_contract.validate_and_ingest(
                archive_path,
                request,
                record,
                self.hardware_policy,
                lambda payload: PRODUCER._write_object(evidence, payload),
                set(),
            )
            self.assertEqual(
                PRODUCER._sha256(archive), validated.archive_reference["sha256"]
            )
            self.assertEqual(64, len(validated.result_observation_sha256))

        run_reference = index["runReceipt"]
        run = json.loads(objects[run_reference["sha256"]])
        self.assertEqual(
            "authenticated-observation-no-independent-authority", run["authority"]
        )
        cleanup = json.loads(objects[index["cleanupReceipt"]["sha256"]])
        self.assertEqual(
            "authenticated-observation-no-independent-authority",
            cleanup["authority"],
        )
        hardware = json.loads(objects[cleanup["hardwareEvidence"]["sha256"]])
        self.assertEqual(
            "verification-input-no-independent-authority", hardware["authority"]
        )
        self.assertEqual(
            set(PRODUCER.hardware_receipt_contract.COMPILER_OBSERVATION_KINDS)
            | set(PRODUCER.hardware_receipt_contract.PLATFORM_OBSERVATION_KINDS)
            | {"isa", "resource", "result"},
            set(run["observations"]),
        )
        self.assertEqual(
            record["productionEvidence"]["targetIdentitySha256"],
            run["targetIdentitySha256"],
        )
        self.assertEqual("test-mi300x-reservation", run["reservationIdentity"])

    def test_hardware_proof_join_uses_typed_identity_not_raw_content_hash(self) -> None:
        _, record = self.request_and_record()
        reference = record["evidenceFiles"]["proof-evidence"]
        payload = (self.archive / reference["path"]).read_bytes()
        typed = PRODUCER.hardware_receipt_contract._capability_result_set_identity(
            payload
        )
        self.assertEqual(record["productionEvidence"]["proofEvidenceSha256"], typed)
        self.assertNotEqual(reference["sha256"], typed)

        checker_reference = record["evidenceFiles"]["proof-checker"]
        checker_payload = (self.archive / checker_reference["path"]).read_bytes()
        checker_typed = (
            PRODUCER.hardware_receipt_contract._authenticated_checker_evidence_identity(
                checker_payload
            )
        )
        self.assertEqual(
            record["productionEvidence"]["proofCheckerSha256"], checker_typed
        )
        self.assertEqual(record["proof"]["checkerSha256"], checker_typed)
        self.assertNotEqual(checker_reference["sha256"], checker_typed)

    def test_authenticated_adapter_rejects_process_success_without_result(self) -> None:
        with self.assertRaisesRegex(
            HARDWARE_RUNNER.RunnerError, "process success alone is not qualification"
        ):
            self.run_authenticated_adapter(mode="omit-result")

    def test_authenticated_adapter_requires_every_typed_observation_file(self) -> None:
        for name in ("isa", "resource", "result"):
            with (
                self.subTest(name=name),
                self.assertRaisesRegex(
                    HARDWARE_RUNNER.RunnerError,
                    f"omitted typed {name} observation",
                ),
            ):
                self.run_authenticated_adapter(mode=f"omit-{name}")

    def test_authenticated_adapter_rejects_symlinked_observation(self) -> None:
        with self.assertRaisesRegex(
            HARDWARE_RUNNER.RunnerError,
            "omitted typed result observation",
        ):
            self.run_authenticated_adapter(mode="symlink-result")

    def test_authenticated_adapter_rejects_hardlinked_observation(self) -> None:
        with self.assertRaisesRegex(
            HARDWARE_RUNNER.RunnerError,
            "new, owned, non-linked regular file",
        ):
            self.run_authenticated_adapter(mode="hardlink-result")

    def test_authenticated_adapter_requires_external_scratch(self) -> None:
        with self.assertRaisesRegex(
            HARDWARE_RUNNER.RunnerError,
            "temporary root must be outside the repository",
        ):
            self.run_authenticated_adapter(temporary_inside_repository=True)

    def test_authenticated_adapter_rejects_request_path_substitution(self) -> None:
        with self.assertRaisesRegex(
            HARDWARE_RUNNER.RunnerError,
            "changed its transaction request or record",
        ):
            self.run_authenticated_adapter(mode="mutate-request")

    def test_authenticated_adapter_rejects_stale_result(self) -> None:
        def stale(result: dict) -> None:
            result["candidate"]["compilerTree"] = "f" * 40

        with self.assertRaisesRegex(
            HARDWARE_RUNNER.receipt.HardwareReceiptError,
            "complete target-matched semantic observation",
        ):
            self.run_authenticated_adapter(mutate_result=stale)

    def test_authenticated_adapter_rejects_cross_target_result(self) -> None:
        with self.assertRaisesRegex(
            HARDWARE_RUNNER.receipt.HardwareReceiptError,
            "complete target-matched semantic observation",
        ):
            self.run_authenticated_adapter(
                mutate_result=lambda result: result.update(target="gfx950")
            )

    def test_authenticated_adapter_rejects_partial_oracle_and_canary_results(
        self,
    ) -> None:
        mutations = (
            lambda result: result["checks"].update(canariesChecked=False),
            lambda result: result["observedIdentities"].update(
                observedOutputSha256=PRODUCER._sha256(b"partial-output")
            ),
            lambda result: result.update(
                runtimeIdentitySha256=PRODUCER._sha256(b"other-runtime")
            ),
            lambda result: result.update(
                driverIdentitySha256=PRODUCER._sha256(b"other-driver")
            ),
        )
        for mutation in mutations:
            with (
                self.subTest(mutation=mutation),
                self.assertRaisesRegex(
                    HARDWARE_RUNNER.receipt.HardwareReceiptError,
                    "complete target-matched semantic observation",
                ),
            ):
                self.run_authenticated_adapter(mutate_result=mutation)

    def test_authenticated_adapter_cleans_failed_runner(self) -> None:
        with self.assertRaisesRegex(HARDWARE_RUNNER.RunnerError, "status 9"):
            self.run_authenticated_adapter(mode="failure")

    def test_authenticated_adapter_bounds_unlinked_output_and_descendants(self) -> None:
        with (
            patch.object(HARDWARE_RUNNER, "MAX_PROCESS_OUTPUT", 1024),
            self.assertRaisesRegex(HARDWARE_RUNNER.RunnerError, "stdout exceeded"),
        ):
            self.run_authenticated_adapter(mode="replace-output")
        with self.assertRaisesRegex(HARDWARE_RUNNER.RunnerError, "live descendant"):
            self.run_authenticated_adapter(mode="live-descendant")

    def test_dispatch_maps_targets_to_exact_ssh_hosts_and_cleans_attempts(self) -> None:
        for target, host in (("gfx942", "mi300x"), ("gfx950", "mi350")):
            with (
                self.subTest(target=target),
                tempfile.TemporaryDirectory(dir=self.root) as temporary,
            ):
                request, record = self.request_and_record()
                request["fixture"]["target"] = target
                request["fixture"]["hardwareLane"] = host
                session = HARDWARE_RUNNER.RemoteHardwareSession(
                    ROOT,
                    Path(temporary),
                    request["candidate"],
                    {
                        "mi300x": "/operator/mi300x-attestor.pem",
                        "mi350": "/operator/mi350-attestor.pem",
                    },
                )

                def complete(command, *, stdout_path, stderr_path, **_):
                    self.assertIn(host, command)
                    stdout_path.write_bytes(b"signed-hardware-archive")
                    stderr_path.write_bytes(b"")

                with (
                    patch.object(session, "_ensure_host", return_value="/tmp/session"),
                    patch.object(session, "_copy_remote") as copy_remote,
                    patch.object(session, "_remove") as remove,
                    patch.object(session, "_run_remote", return_value=b""),
                    patch.object(
                        HARDWARE_RUNNER, "_bounded_process", side_effect=complete
                    ),
                ):
                    self.assertEqual(
                        b"signed-hardware-archive",
                        session.run(
                            request,
                            record,
                            self.archive,
                            "tutorial-test-attestor-v1",
                        ),
                    )
                copy_remote.assert_called_once()
                self.assertEqual(2, remove.call_count)

    def test_failed_dispatch_still_runs_terminal_cleanup(self) -> None:
        request, record = self.request_and_record()
        with tempfile.TemporaryDirectory(dir=self.root) as temporary:
            session = HARDWARE_RUNNER.RemoteHardwareSession(
                ROOT,
                Path(temporary),
                request["candidate"],
                {"mi300x": "/operator/mi300x-attestor.pem"},
            )
            with (
                patch.object(session, "_ensure_host", return_value="/tmp/session"),
                patch.object(session, "_copy_remote"),
                patch.object(session, "_remove") as remove,
                patch.object(session, "_run_remote", return_value=b""),
                patch.object(
                    HARDWARE_RUNNER,
                    "_bounded_process",
                    side_effect=HARDWARE_RUNNER.RunnerError("ssh failed"),
                ),
                self.assertRaisesRegex(HARDWARE_RUNNER.RunnerError, "ssh failed"),
            ):
                session.run(
                    request,
                    record,
                    self.archive,
                    "tutorial-test-attestor-v1",
                )
            self.assertEqual(2, remove.call_count)

    def test_partial_transfer_and_cleanup_failure_are_fail_closed(self) -> None:
        request, record = self.request_and_record()
        with tempfile.TemporaryDirectory(dir=self.root) as temporary:
            session = HARDWARE_RUNNER.RemoteHardwareSession(
                ROOT,
                Path(temporary),
                request["candidate"],
                {"mi300x": "/operator/mi300x-attestor.pem"},
            )
            with (
                patch.object(session, "_ensure_host", return_value="/tmp/session"),
                patch.object(
                    session,
                    "_copy_remote",
                    side_effect=HARDWARE_RUNNER.RunnerError("partial transfer"),
                ),
                patch.object(session, "_remove") as remove,
                self.assertRaisesRegex(HARDWARE_RUNNER.RunnerError, "partial transfer"),
            ):
                session.run(request, record, self.archive, "tutorial-test-attestor-v1")
            self.assertEqual(2, remove.call_count)

            with (
                patch.object(session, "_ensure_host", return_value="/tmp/session"),
                patch.object(
                    session,
                    "_copy_remote",
                    side_effect=HARDWARE_RUNNER.RunnerError("partial transfer"),
                ),
                patch.object(
                    session,
                    "_remove",
                    side_effect=[HARDWARE_RUNNER.RunnerError("cleanup denied"), None],
                ) as remove,
                self.assertRaisesRegex(
                    HARDWARE_RUNNER.RunnerError, "attempt cleanup failed"
                ),
            ):
                session.run(request, record, self.archive, "tutorial-test-attestor-v1")
            self.assertEqual(2, remove.call_count)

    def test_ssh_and_scp_reject_argument_injection(self) -> None:
        with self.assertRaisesRegex(
            HARDWARE_RUNNER.RunnerError, "configured hardware lane"
        ):
            HARDWARE_RUNNER._ssh_command("-oProxyCommand=touch /tmp/injected", "true")
        request, _ = self.request_and_record()
        with tempfile.TemporaryDirectory(dir=self.root) as temporary:
            source = Path(temporary) / "payload"
            source.write_bytes(b"payload")
            session = HARDWARE_RUNNER.RemoteHardwareSession(
                ROOT,
                Path(temporary),
                request["candidate"],
                {"mi300x": "/operator/mi300x-attestor.pem"},
            )
            with self.assertRaisesRegex(
                HARDWARE_RUNNER.RunnerError, "shell-significant"
            ):
                session._copy_remote(source, "mi300x", "/tmp/a;touch-injected")

    def test_bounded_processes_enforce_timeout_and_output_during_execution(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory(dir=self.root) as temporary:
            root = Path(temporary)
            with self.assertRaisesRegex(HARDWARE_RUNNER.RunnerError, "timeout"):
                HARDWARE_RUNNER._bounded_process(
                    [sys.executable, "-c", "import time; time.sleep(30)"],
                    cwd=ROOT,
                    stdout_path=root / "stdout",
                    stderr_path=root / "stderr",
                    timeout=1,
                    stdout_limit=64,
                )
        with self.assertRaisesRegex(
            PRODUCER.QualificationProducerError, "output bounds"
        ):
            PRODUCER._run_bounded(
                [
                    sys.executable,
                    "-c",
                    "import sys,time; sys.stdout.write('x'*4096); sys.stdout.flush(); time.sleep(30)",
                ],
                cwd=ROOT,
                environment=os.environ.copy(),
                timeout_seconds=5,
                stdout_limit=64,
            )

    def test_dispatch_rejects_cross_target_host_before_transfer(self) -> None:
        request, record = self.request_and_record()
        request["fixture"]["hardwareLane"] = "mi350"
        with tempfile.TemporaryDirectory(dir=self.root) as temporary:
            session = HARDWARE_RUNNER.RemoteHardwareSession(
                ROOT,
                Path(temporary),
                request["candidate"],
                {"mi350": "/operator/mi350-attestor.pem"},
            )
            with self.assertRaisesRegex(
                HARDWARE_RUNNER.RunnerError, "target does not map"
            ):
                session.run(
                    request,
                    record,
                    self.archive,
                    "tutorial-test-attestor-v1",
                )

    def test_typed_observation_schemas_reject_missing_or_unbounded_fields(self) -> None:
        request, record = self.request_and_record()
        driver, runtime, isa, resource, result = self.runner_observations(
            request, record
        )
        cases = []
        missing_isa = json.loads(isa)
        missing_isa.pop("inspectionToolSha256")
        cases.append((canonical(missing_isa), resource, result, "fields differ"))
        missing_resource = json.loads(resource)
        missing_resource.pop("ldsBytes")
        cases.append((isa, canonical(missing_resource), result, "fields differ"))
        oversized_workgroup = json.loads(resource)
        oversized_workgroup["workgroupSize"] = [1024, 1024, 2]
        cases.append(
            (
                isa,
                canonical(oversized_workgroup),
                result,
                "resource observation is incomplete",
            )
        )
        missing_output_identity = json.loads(result)
        missing_output_identity["observedIdentities"].pop("observedOutputSha256")
        cases.append(
            (isa, resource, canonical(missing_output_identity), "fields differ")
        )
        for observed_isa, observed_resource, observed_result, pattern in cases:
            with (
                self.subTest(pattern=pattern),
                tempfile.TemporaryDirectory(dir=self.root) as temporary,
            ):
                scratch = Path(temporary) / "scratch"
                spool = Path(temporary) / "spool"
                scratch.mkdir()
                with self.assertRaisesRegex(
                    PRODUCER.hardware_receipt_contract.HardwareReceiptError, pattern
                ):
                    PRODUCER.hardware_receipt_contract.prepare_transport(
                        request,
                        record,
                        self.archive,
                        observed_isa,
                        observed_resource,
                        observed_result,
                        scratch,
                        spool,
                        "tutorial-test-attestor-v1",
                        self.private_key,
                        driver_observation=driver,
                        runtime_observation=runtime,
                    )
                self.assertFalse(spool.exists())

    def test_attestor_provisioning_is_external_and_not_auto_trusted(self) -> None:
        with tempfile.TemporaryDirectory(dir=self.root) as temporary:
            output = Path(temporary) / "lane-attestor"
            paths = PROVISIONER.provision(
                output,
                "mi350",
                "gfx950",
                "reservation-2026-09-07-a",
                "mi350-attestor-2026-09-07-a",
            )
            self.assertFalse(output.is_relative_to(ROOT))
            self.assertEqual(0o600, stat.S_IMODE(paths["private_key"].stat().st_mode))
            self.assertEqual(0o600, stat.S_IMODE(paths["policy"].stat().st_mode))
            self.assertEqual(
                {
                    "attestor-private.pem",
                    "attestor-public.pem",
                    "proposed-trust-policy-v1.json",
                },
                {path.name for path in output.iterdir()},
            )
            policy = PRODUCER.hardware_receipt_contract.load_trust_policy(
                paths["policy"]
            )
            lane = policy.lanes[("mi350", "gfx950")]
            self.assertEqual("reservation-2026-09-07-a", lane.reservation_identity)

    def test_attestor_provisioning_requires_explicit_nontrust_acknowledgment(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory(dir=self.root) as temporary:
            output = Path(temporary) / "lane-attestor"
            completed = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPTS / "provision-tutorial-hardware-attestor.py"),
                    "--attestor-identity",
                    "mi350-attestor-test",
                    "--lane",
                    "mi350",
                    "--output-directory",
                    str(output),
                    "--reservation",
                    "reservation-test",
                    "--target",
                    "gfx950",
                ],
                check=False,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            self.assertEqual(2, completed.returncode)
            self.assertFalse(output.exists())
            self.assertIn(b"will not be installed or trusted", completed.stderr)

    def test_attestor_provisioning_refuses_repository_output(self) -> None:
        output = ROOT / f".issue272-attestor-test-{os.getpid()}"
        self.assertFalse(output.exists())
        with self.assertRaisesRegex(
            PROVISIONER.ProvisionError, "outside the repository"
        ):
            PROVISIONER.provision(
                output,
                "mi350",
                "gfx950",
                "reservation-test",
                "mi350-attestor-test",
            )
        self.assertFalse(output.exists())

    def test_operator_policy_path_is_exact_external_and_non_symlink(self) -> None:
        self.assertEqual(
            self.policy_path,
            PRODUCER._operator_policy_path(ROOT, self.policy_path),
        )
        link = self.root / "policy-link.json"
        link.symlink_to(self.policy_path)
        with self.assertRaisesRegex(
            PRODUCER.QualificationProducerError, "exact regular non-symlink"
        ):
            PRODUCER._operator_policy_path(ROOT, link)
        with self.assertRaisesRegex(
            PRODUCER.QualificationProducerError, "absolute and lexically normalized"
        ):
            PRODUCER._operator_policy_path(ROOT, Path("policy.json"))
        proposed = self.root / "proposed-trust-policy-v1.json"
        proposed.write_bytes(self.policy_path.read_bytes())
        proposed.chmod(0o600)
        with self.assertRaisesRegex(
            PRODUCER.QualificationProducerError, "independently installed"
        ):
            PRODUCER._operator_policy_path(ROOT, proposed)

    def test_rust_exporter_has_explicit_prepare_and_finalize_phases(self) -> None:
        request = self.root / "request-for-command.json"
        pre = self.root / "pre-for-command"
        hardware = self.root / "hardware-for-command.zip"
        output = self.root / "new-export-directory"
        output.unlink(missing_ok=True)
        with patch.object(PRODUCER, "_run_bounded", return_value=(b"", b"")) as run:
            PRODUCER.invoke_compiler_phase(
                ["future-rust-exporter"],
                ROOT,
                request,
                pre,
                9,
            )
            PRODUCER.invoke_finalization_phase(
                ["future-rust-exporter"],
                ROOT,
                request,
                pre,
                hardware,
                output,
                9,
                self.policy_path,
            )
        prepare = run.call_args_list[0].args[0]
        command = run.call_args_list[1].args[0]
        self.assertIn(
            ["--phase", "prepare"],
            [prepare[index : index + 2] for index in range(len(prepare) - 1)],
        )
        self.assertNotIn("--hardware-trust-policy", prepare)
        self.assertIn(
            ["--phase", "finalize"],
            [command[index : index + 2] for index in range(len(command) - 1)],
        )
        policy_offset = command.index("--hardware-trust-policy")
        self.assertEqual(str(self.policy_path), command[policy_offset + 1])

    def test_legacy_hardware_command_without_protocol_marker_is_rejected(self) -> None:
        request, _ = self.request_and_record()
        request["fixture"]["hardwareCommand"] = {
            "arguments": [],
            "environment": [],
            "executable": "runner.sh",
            "timeoutSeconds": 1,
            "workingDirectory": ".",
        }
        with self.assertRaisesRegex(HARDWARE_RUNNER.RunnerError, "legacy hardware"):
            HARDWARE_RUNNER.command_contract(request, "runner.sh", [])

    def test_duplicate_hardware_environment_names_are_rejected(self) -> None:
        request, _ = self.request_and_record()
        request["fixture"]["hardwareCommand"] = {
            "arguments": [],
            "environment": [
                "FE2O3_TUTORIAL_HARDWARE_PROTOCOL=authenticated-v1",
                "FE2O3_TUTORIAL_HARDWARE_PROTOCOL=authenticated-v2",
            ],
            "executable": "runner.sh",
            "timeoutSeconds": 1,
            "workingDirectory": ".",
        }
        with (
            patch.dict(
                os.environ,
                {"FE2O3_TUTORIAL_HARDWARE_PROTOCOL": "authenticated-v1"},
                clear=False,
            ),
            self.assertRaisesRegex(HARDWARE_RUNNER.RunnerError, "repeats"),
        ):
            HARDWARE_RUNNER.command_contract(request, "runner.sh", [])

    def test_rejects_omitted_evidence(self) -> None:
        self.assert_export_rejected(
            "exact authenticated completion",
            mutate_record=lambda record: record["evidenceFiles"].pop("proof-evidence"),
        )

    def test_rejects_request_substitution(self) -> None:
        self.assert_export_rejected(
            "stale or substituted",
            mutate_envelope=lambda envelope: envelope.update(
                requestBindingSha256=PRODUCER._sha256(b"substituted-request")
            ),
        )

    def test_rejects_stale_candidate(self) -> None:
        self.assert_export_rejected(
            "stale or substituted",
            mutate_envelope=lambda envelope: envelope.update(
                candidate={
                    "compilerCommit": "3" * 40,
                    "compilerTree": "4" * 40,
                    "worktreeClean": True,
                }
            ),
        )

    def test_rejects_cross_target_record(self) -> None:
        self.assert_export_rejected(
            "exact authenticated completion",
            mutate_record=lambda record: record.update(target="gfx950"),
        )

    def test_rejects_cross_launch_hardware(self) -> None:
        self.assert_export_rejected(
            "exact authenticated completion",
            mutate_hardware=lambda evidence: evidence.update(
                commandSha256=PRODUCER._sha256(b"different-launch")
            ),
        )

    def test_rejects_partial_hardware_and_incomplete_cleanup(self) -> None:
        self.assert_export_rejected(
            "exact authenticated completion",
            mutate_hardware=lambda evidence: evidence["checks"].update(
                cleanupComplete=False
            ),
        )

    def test_hardware_challenges_are_fresh_and_bound_to_the_request(self) -> None:
        first, _ = self.request_and_record()
        second, _ = self.request_and_record()
        self.assertNotEqual(
            first["hardwareReceiptChallenge"]["nonce"],
            second["hardwareReceiptChallenge"]["nonce"],
        )
        self.assertNotEqual(
            first["requestBindingSha256"], second["requestBindingSha256"]
        )
        self.assertEqual(
            PRODUCER.promotion_contract.command_sha256(
                first["fixture"]["hardwareCommand"]
            ),
            PRODUCER.hardware_receipt_contract._command_identity(
                first["fixture"]["hardwareCommand"]
            ),
        )

    def test_rejects_a_replayed_signed_hardware_receipt(self) -> None:
        temporary, export, destination, request, pre_record = self.make_export()
        with temporary:
            seen: set[str] = set()
            archive_path = export / PRODUCER.hardware_receipt_contract.TRANSPORT_NAME
            PRODUCER.hardware_receipt_contract.validate_and_ingest(
                archive_path,
                request,
                pre_record,
                self.hardware_policy,
                lambda payload: PRODUCER._write_object(destination, payload),
                seen,
            )
            second_destination = Path(temporary.name) / "destination-two"
            second_destination.mkdir()
            with self.assertRaisesRegex(
                PRODUCER.hardware_receipt_contract.HardwareReceiptError,
                "replay detected",
            ):
                PRODUCER.hardware_receipt_contract.validate_and_ingest(
                    archive_path,
                    request,
                    pre_record,
                    self.hardware_policy,
                    lambda payload: PRODUCER._write_object(
                        second_destination, payload
                    ),
                    seen,
                )

    def test_rejects_archive_after_pre_hardware_record_mutation(self) -> None:
        temporary, export, destination, request, pre_record = self.make_export()
        with temporary:
            pre_record["graph"]["finalGraphEpoch"] += 1
            pre_record["preHardwareBindingSha256"] = (
                PRODUCER.pre_hardware_binding_sha256(pre_record)
            )
            with self.assertRaisesRegex(
                PRODUCER.hardware_receipt_contract.HardwareReceiptError,
                "replayed, cross-target, or cross-transaction",
            ):
                PRODUCER.hardware_receipt_contract.validate_and_ingest(
                    export / PRODUCER.hardware_receipt_contract.TRANSPORT_NAME,
                    request,
                    pre_record,
                    self.hardware_policy,
                    lambda payload: PRODUCER._write_object(destination, payload),
                    set(),
                )

    def test_descriptor_reader_rejects_toctou_entry_swap(self) -> None:
        with tempfile.TemporaryDirectory(dir=self.root) as temporary:
            root = Path(temporary)
            victim = root / "victim"
            replacement = root / "replacement"
            displaced = root / "displaced"
            victim.write_bytes(b"first")
            replacement.write_bytes(b"other")
            real_open = os.open
            swapped = False

            def swap_before_open(path, flags, *args, **kwargs):
                nonlocal swapped
                if (
                    path == "victim"
                    and kwargs.get("dir_fd") is not None
                    and not swapped
                ):
                    swapped = True
                    victim.rename(displaced)
                    replacement.rename(victim)
                return real_open(path, flags, *args, **kwargs)

            with (
                patch.object(
                    PRODUCER.hardware_receipt_contract.os,
                    "open",
                    side_effect=swap_before_open,
                ),
                self.assertRaisesRegex(
                    PRODUCER.hardware_receipt_contract.HardwareReceiptError,
                    "changed before it was opened",
                ),
            ):
                PRODUCER.hardware_receipt_contract._read_regular(victim, "victim", 64)
            self.assertTrue(swapped)

    def test_archive_rejects_traversal_duplicates_and_nonregular_paths(self) -> None:
        temporary, export, destination, request, pre_record = self.make_export()
        with temporary:
            archive_path = export / PRODUCER.hardware_receipt_contract.TRANSPORT_NAME
            valid = archive_path.read_bytes()

            def rewritten(extra_name: str, *, duplicate: bool) -> bytes:
                entries: list[tuple[str, bytes]] = []
                with zipfile.ZipFile(io.BytesIO(valid), "r") as source:
                    entries.extend(
                        (info.filename, source.read(info)) for info in source.infolist()
                    )
                payload = entries[1][1] if duplicate else b"traversal"
                entries.append((entries[1][0] if duplicate else extra_name, payload))
                entries.sort(key=lambda item: item[0])
                output = io.BytesIO()
                with warnings.catch_warnings(), zipfile.ZipFile(output, "w") as archive:
                    warnings.simplefilter("ignore", UserWarning)
                    for name, data in entries:
                        info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
                        info.compress_type = zipfile.ZIP_STORED
                        info.create_system = 3
                        info.external_attr = (stat.S_IFREG | 0o600) << 16
                        archive.writestr(info, data)
                return output.getvalue()

            for payload in (
                rewritten("objects/sha256/aa/../escape", duplicate=False),
                rewritten("", duplicate=True),
            ):
                with (
                    self.subTest(size=len(payload)),
                    self.assertRaises(
                        PRODUCER.hardware_receipt_contract.HardwareReceiptError
                    ),
                ):
                    PRODUCER.hardware_receipt_contract._archive_entries(payload)

            link = export / "archive-link.zip"
            link.symlink_to(archive_path)
            with self.assertRaisesRegex(
                PRODUCER.hardware_receipt_contract.HardwareReceiptError,
                "symbolic links|non-symlink|symlink",
            ):
                PRODUCER.hardware_receipt_contract.validate_and_ingest(
                    link,
                    request,
                    pre_record,
                    self.hardware_policy,
                    lambda payload: PRODUCER._write_object(destination, payload),
                    set(),
                )
            with self.assertRaisesRegex(
                PRODUCER.hardware_receipt_contract.HardwareReceiptError,
                "changed before it was opened|regular",
            ):
                PRODUCER.hardware_receipt_contract.validate_and_ingest(
                    export,
                    request,
                    pre_record,
                    self.hardware_policy,
                    lambda payload: PRODUCER._write_object(destination, payload),
                    set(),
                )

    def test_rejects_cross_lane_and_finalize_key_substitution(self) -> None:
        temporary, export, destination, request, pre_record = self.make_export()
        with temporary:
            request["fixture"]["hardwareLane"] = "mi350"
            with self.assertRaisesRegex(
                PRODUCER.hardware_receipt_contract.HardwareReceiptError,
                "pre-hardware hardware request is stale or cross-target",
            ):
                PRODUCER.hardware_receipt_contract.validate_and_ingest(
                    export / PRODUCER.hardware_receipt_contract.TRANSPORT_NAME,
                    request,
                    pre_record,
                    self.hardware_policy,
                    lambda payload: PRODUCER._write_object(destination, payload),
                    set(),
                )

        temporary, export, _, request, pre_record = self.make_export()
        with temporary:
            scratch = Path(temporary.name) / "key-swap-scratch"
            spool = Path(temporary.name) / "key-swap-spool"
            wrong_key = Path(temporary.name) / "wrong-private.pem"
            wrong_key.write_bytes(
                (
                    SCRIPTS
                    / "tests"
                    / "fixtures"
                    / "evidence-test-reviewer-private.pem"
                ).read_bytes()
            )
            wrong_key.chmod(0o600)
            scratch.mkdir()
            driver, runtime, isa, resource, result = self.runner_observations(
                request, pre_record
            )
            PRODUCER.hardware_receipt_contract.prepare_transport(
                request,
                pre_record,
                export,
                isa,
                resource,
                result,
                scratch,
                spool,
                "tutorial-test-attestor-v1",
                self.private_key,
                driver_observation=driver,
                runtime_observation=runtime,
            )
            shutil.rmtree(scratch)
            with self.assertRaisesRegex(
                PRODUCER.hardware_receipt_contract.HardwareReceiptError,
                "different hardware attestor key",
            ):
                PRODUCER.hardware_receipt_contract.finalize_transport(
                    spool, scratch, wrong_key, request, pre_record, export
                )
            self.assertFalse(spool.exists())

    def test_rejects_tampered_hardware_archive(self) -> None:
        def tamper(payload: bytes) -> bytes:
            changed = bytearray(payload)
            changed[len(changed) // 2] ^= 1
            return bytes(changed)

        self.assert_export_rejected(
            "authenticated hardware archive rejected",
            mutate_transport=tamper,
        )

    def test_rejects_a_valid_signature_from_an_untrusted_lane_key(self) -> None:
        wrong_policy_path = self.root / "wrong-hardware-policy-v1.json"
        wrong_policy_path.write_bytes(
            PRODUCER.hardware_receipt_contract.canonical(
                PRODUCER.hardware_receipt_contract.trust_policy_document(
                    [
                        {
                            "attestorIdentity": "tutorial-test-attestor-v1",
                            "lane": "mi300x",
                            "publicKeyPath": str(
                                SCRIPTS
                                / "tests"
                                / "fixtures"
                                / "evidence-test-reviewer-public.pem"
                            ),
                            "reservationIdentity": "test-mi300x-reservation",
                            "target": "gfx942",
                        }
                    ]
                )
            )
        )
        wrong_policy_path.chmod(0o600)
        wrong_policy = PRODUCER.hardware_receipt_contract.load_trust_policy(
            wrong_policy_path
        )
        temporary, export, destination, request, pre_record = self.make_export()
        with temporary:
            with self.assertRaisesRegex(
                PRODUCER.QualificationProducerError, "unauthenticated"
            ):
                PRODUCER.validate_transaction_export(
                    export,
                    export,
                    pre_record,
                    request,
                    destination,
                    wrong_policy,
                    set(),
                )

    def test_finalize_refuses_to_emit_before_scratch_cleanup(self) -> None:
        temporary, export, _, request, pre_record = self.make_export()
        with temporary:
            record = pre_record
            scratch = Path(temporary.name) / "second-runner-scratch"
            spool = Path(temporary.name) / "second-receipt-spool"
            scratch.mkdir()
            driver, runtime, isa, resource, result = self.runner_observations(
                request, record
            )
            PRODUCER.hardware_receipt_contract.prepare_transport(
                request,
                record,
                export,
                isa,
                resource,
                result,
                scratch,
                spool,
                "tutorial-test-attestor-v1",
                self.private_key,
                driver_observation=driver,
                runtime_observation=runtime,
            )
            with self.assertRaisesRegex(
                PRODUCER.hardware_receipt_contract.HardwareReceiptError,
                "left scratch residue",
            ):
                PRODUCER.hardware_receipt_contract.finalize_transport(
                    spool,
                    scratch,
                    self.private_key,
                    request,
                    record,
                    export,
                )
            self.assertFalse(spool.exists())
            shutil.rmtree(scratch)

    def test_prepare_finalize_cli_emits_a_valid_archive_without_residue(self) -> None:
        temporary, export, destination, request, pre_record = self.make_export()
        with temporary:
            request_path = Path(temporary.name) / "request-v1.json"
            record_path = Path(temporary.name) / "record-v1.json"
            isa_path = Path(temporary.name) / "isa.txt"
            resource_path = Path(temporary.name) / "resource.txt"
            result_path = Path(temporary.name) / "result.json"
            scratch = Path(temporary.name) / "cli-runner-scratch"
            spool = Path(temporary.name) / "cli-receipt-spool"
            request_path.write_bytes(canonical(request))
            record = pre_record
            record_path.write_bytes(canonical(record))
            driver, runtime, isa, resource, result = self.runner_observations(
                request, record
            )
            driver_path = Path(temporary.name) / "driver.json"
            isa_path.write_bytes(isa)
            resource_path.write_bytes(resource)
            result_path.write_bytes(result)
            driver_path.write_bytes(driver)
            runtime_path = Path(temporary.name) / "runtime.json"
            runtime_path.write_bytes(runtime)
            scratch.mkdir()
            command = [
                sys.executable,
                str(SCRIPTS / "tutorial_hardware_receipt.py"),
            ]
            prepared = subprocess.run(
                [
                    *command,
                    "prepare",
                    "--attestor-identity",
                    "tutorial-test-attestor-v1",
                    "--driver-observation",
                    str(driver_path),
                    "--evidence-root",
                    str(export),
                    "--isa-observation",
                    str(isa_path),
                    "--private-key",
                    str(self.private_key),
                    "--record",
                    str(record_path),
                    "--request",
                    str(request_path),
                    "--resource-observation",
                    str(resource_path),
                    "--result-observation",
                    str(result_path),
                    "--runtime-observation",
                    str(runtime_path),
                    "--scratch-path",
                    str(scratch),
                    "--spool",
                    str(spool),
                ],
                check=False,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            self.assertTrue(prepared.stdout.startswith(b"PK"))
            self.assertEqual(b"", prepared.stderr)
            self.assertEqual(0, prepared.returncode)
            shutil.rmtree(scratch)
            finalized = subprocess.run(
                [
                    *command,
                    "finalize",
                    "--evidence-root",
                    str(export),
                    "--private-key",
                    str(self.private_key),
                    "--record",
                    str(record_path),
                    "--request",
                    str(request_path),
                    "--scratch-path",
                    str(scratch),
                    "--spool",
                    str(spool),
                ],
                check=False,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            self.assertEqual(b"", finalized.stderr)
            self.assertEqual(0, finalized.returncode)
            self.assertFalse(scratch.exists())
            self.assertFalse(spool.exists())
            archive_path = export / PRODUCER.hardware_receipt_contract.TRANSPORT_NAME
            archive_path.write_bytes(finalized.stdout)
            final_index, _ = PRODUCER.hardware_receipt_contract._archive_entries(
                finalized.stdout
            )
            self.assertEqual(
                PRODUCER._sha256(prepared.stdout),
                final_index["preCleanupCapsuleSha256"],
            )
            PRODUCER.hardware_receipt_contract.validate_and_ingest(
                archive_path,
                request,
                record,
                self.hardware_policy,
                lambda payload: PRODUCER._write_object(destination, payload),
                set(),
            )

    def test_rejects_missing_authenticated_hardware_archive(self) -> None:
        temporary, export, destination, request, pre_record = self.make_export()
        with temporary:
            (export / PRODUCER.hardware_receipt_contract.TRANSPORT_NAME).unlink()
            with self.assertRaisesRegex(
                PRODUCER.QualificationProducerError,
                "authenticated hardware archive rejected",
            ):
                PRODUCER.validate_transaction_export(
                    export,
                    export,
                    pre_record,
                    request,
                    destination,
                    self.hardware_policy,
                    set(),
                )

    def test_rejects_cross_target_hardware_archive_reuse(self) -> None:
        temporary, export, destination, request, pre_record = self.make_export()
        with temporary:
            request["fixture"]["target"] = "gfx950"
            request["fixture"]["hardwareLane"] = "mi350"
            with self.assertRaisesRegex(
                PRODUCER.hardware_receipt_contract.HardwareReceiptError,
                "pre-hardware record is cross-fixture, cross-kernel, or cross-target",
            ):
                PRODUCER.hardware_receipt_contract.validate_and_ingest(
                    export / PRODUCER.hardware_receipt_contract.TRANSPORT_NAME,
                    request,
                    pre_record,
                    self.hardware_policy,
                    lambda payload: PRODUCER._write_object(destination, payload),
                    set(),
                )

    def test_rejects_record_reordering(self) -> None:
        records = [deepcopy(record) for record in self.batch["records"]]
        records[0], records[1] = records[1], records[0]
        with self.assertRaisesRegex(
            PRODUCER.QualificationProducerError,
            "omitted, duplicated, substituted, or reordered",
        ):
            PRODUCER._require_exact_record_order(records, sorted(self.fixtures))

    def test_failed_workspace_is_removed_and_publishes_nothing(self) -> None:
        parent = self.root / "cleanup"
        batch = parent / "batch.json"
        evidence = parent / "evidence"
        with self.assertRaisesRegex(RuntimeError, "injected failure"):
            with PRODUCER.qualification_workspace(parent) as workspace:
                (workspace / "partial").write_text("not evidence", encoding="ascii")
                raise RuntimeError("injected failure")
        self.assertFalse(batch.exists())
        self.assertFalse(evidence.exists())
        self.assertEqual([], list(parent.iterdir()))

    def test_missing_production_executable_has_a_precise_diagnostic(self) -> None:
        with self.assertRaisesRegex(
            PRODUCER.QualificationProducerError,
            "an explicit cargo-fe2o3 production executable is required",
        ):
            PRODUCER.production_transaction_command(ROOT)


if __name__ == "__main__":
    unittest.main()
