#!/usr/bin/env python3

from __future__ import annotations

from copy import deepcopy
import hashlib
import importlib.util
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest


sys.dont_write_bytecode = True
SCRIPT = Path(__file__).resolve().parents[1] / "promote-tutorial-capabilities.py"
sys.path.insert(0, str(SCRIPT.parent))
SPEC = importlib.util.spec_from_file_location("tutorial_capability_promotion", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
PROMOTER = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = PROMOTER
SPEC.loader.exec_module(PROMOTER)
ROOT = SCRIPT.parent.parent
MANIFEST = ROOT / PROMOTER.MANIFEST_PATH
NEGATIVE_TEST_PATH = "scripts/tests/tutorial_capability_promotion.py"


def identity(label: str) -> str:
    return hashlib.sha256(label.encode("ascii")).hexdigest()


def manifest_with_v8_suites() -> tuple[bytes, dict]:
    document = json.loads(MANIFEST.read_text())
    fixtures = {item["fixtureId"]: item for item in document["compilerFixtures"]}
    kernels = {item["fixtureId"]: item for item in document["capabilityKernels"]}
    suites = []
    for fixture_id in sorted(fixtures):
        suites.append(
            {
                "availability": "available",
                "command": {
                    "arguments": [
                        "--fixture",
                        fixture_id,
                        "--bundle-version",
                        "8",
                    ],
                    "environment": [],
                    "executable": "scripts/run-tutorial-semantic-simulation.py",
                    "timeoutSeconds": 3600,
                    "workingDirectory": ".",
                },
                "coverage": [
                    {"fixtureIds": [fixture_id], "lessonId": lesson_id}
                    for lesson_id in sorted(kernels[fixture_id]["lessonIds"])
                ],
                "gate": "semantic-simulation",
                "suiteId": f"capability-{fixture_id}",
                "unavailableReason": None,
            }
        )
    document["qualification"]["suites"] = suites
    raw = (json.dumps(document, ensure_ascii=True, indent=2) + "\n").encode("ascii")
    return raw, document


def evidence_reference(root: Path, path: str, payload: bytes) -> dict:
    destination = root / path
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(payload)
    return {
        "bytes": len(payload),
        "path": path,
        "sha256": hashlib.sha256(payload).hexdigest(),
    }


def build_capability_sets(seed: str) -> tuple[bytes, str, bytes, str]:
    digest = lambda label: hashlib.sha256(f"{seed}:{label}".encode("ascii")).digest()
    subject = (
        digest("kernel")
        + digest("root")
        + digest("kir")
        + struct.pack("<Q", 1)
        + digest("target")
        + digest("launch")
    )
    property_id = digest("property-namespace") + struct.pack("<HHI", 1, 0, 1)
    statement = digest("statement")
    obligation_identity = hashlib.sha256(
        b"FE2O3/INERT-CAPABILITY-OBLIGATION/V1\0"
        + subject
        + property_id
        + statement
    ).digest()
    obligation_total = 16 + len(subject) + 4 + 104 + 32
    obligation_body = (
        b"FE2OCAPO"
        + struct.pack("<HHI", 1, 0, obligation_total)
        + subject
        + struct.pack("<I", 1)
        + property_id
        + statement
        + obligation_identity
    )
    obligation_set_identity = bytes.fromhex(
        PROMOTER._length_delimited_identity(
            PROMOTER.CAPABILITY_OBLIGATION_SET_DOMAIN, obligation_body
        )
    )
    obligation_set = obligation_body + obligation_set_identity

    outcome = (
        struct.pack("<BBH", 1, 0, 0)
        + digest("evidence")
        + digest("tool-executable")
        + digest("tool-configuration")
        + digest("artifact-bytes")
        + digest("artifact-format")
    )
    result_preimage = subject + obligation_set_identity + obligation_identity + outcome
    result_identity = bytes.fromhex(
        PROMOTER._length_delimited_identity(
            b"FE2O3/INERT-CAPABILITY-RESULT/V1\0", result_preimage
        )
    )
    result_total = 16 + len(subject) + 32 + 4 + 64 + len(outcome) + 32
    result_body = (
        b"FE2OCAPR"
        + struct.pack("<HHI", 1, 0, result_total)
        + subject
        + obligation_set_identity
        + struct.pack("<I", 1)
        + obligation_identity
        + result_identity
        + outcome
    )
    result_set_identity = bytes.fromhex(
        PROMOTER._length_delimited_identity(
            PROMOTER.CAPABILITY_RESULT_SET_DOMAIN, result_body
        )
    )
    return (
        obligation_set,
        obligation_set_identity.hex(),
        result_body + result_set_identity,
        result_set_identity.hex(),
    )


def build_v8(target: str, seed: str) -> tuple[bytes, dict, bytes, bytes]:
    kir = f"canonical-kir-v13:{seed}".encode("ascii")
    source = f"source-map:{seed}".encode("ascii")
    semantic = f"semantic-mir:{seed}".encode("ascii")
    storage = f"storage-map:{seed}".encode("ascii")
    aggregate = f"aggregate-map:{seed}".encode("ascii")
    target_bytes = target.encode("ascii")
    kir_identity = bytes.fromhex(identity(f"kir-identity:{seed}"))
    inventory = bytes.fromhex(identity(f"inventory:{seed}"))
    preflight = bytes.fromhex(identity(f"preflight:{seed}"))
    abi = bytes.fromhex(identity(f"abi:{seed}"))
    subject = bytes.fromhex(identity(f"subject:{seed}"))
    epoch = 1 + int(identity(f"epoch:{seed}")[:8], 16)
    header = bytearray()
    header.extend(PROMOTER.V8_MAGIC)
    header.extend(struct.pack("<HHHH", 8, 0, 13, 13))
    header.extend(struct.pack("<I", 1))
    header.extend(struct.pack("<H", len(target_bytes)))
    header.extend(bytes(6))
    header.extend(struct.pack("<Q", len(kir)))
    header.extend(struct.pack("<I", len(source)))
    header.extend(struct.pack("<Q", len(semantic)))
    header.extend(struct.pack("<I", len(storage)))
    header.extend(struct.pack("<I", len(aggregate)))
    header.extend(struct.pack("<Q", epoch))
    header.extend(inventory)
    header.extend(struct.pack("<Q", 64))
    header.extend(preflight)
    header.extend(struct.pack("<Q", 64))
    header.extend(kir_identity)
    header.extend(struct.pack("<Q", len(kir)))
    header.extend(kir_identity)
    header.extend(struct.pack("<Q", len(kir)))
    header.extend(abi)
    header.extend(
        bytes.fromhex(PROMOTER._domain_sha256(PROMOTER.V8_SOURCE_MAP_DOMAIN, source))
    )
    semantic_identity = PROMOTER._domain_sha256(
        PROMOTER.V8_SEMANTIC_MIR_DOMAIN, semantic
    )
    header.extend(bytes.fromhex(semantic_identity))
    header.extend(
        bytes.fromhex(PROMOTER._domain_sha256(PROMOTER.V8_STORAGE_MAP_DOMAIN, storage))
    )
    header.extend(
        bytes.fromhex(PROMOTER._domain_sha256(PROMOTER.V8_AGGREGATE_MAP_DOMAIN, aggregate))
    )
    header.extend(subject)
    assert len(header) == PROMOTER.V8_HEADER_BYTES
    bundle = bytes(header) + target_bytes + kir + source + semantic + storage + aggregate
    graph = {
        "bundleContentIdentitySha256": PROMOTER._domain_sha256(
            PROMOTER.V8_CONTENT_DOMAIN, bundle
        ),
        "bundleSubjectIdentitySha256": subject.hex(),
        "canonicalKirBytes": len(kir),
        "canonicalKirVersion": 13,
        "finalGraphEpoch": epoch,
        "kernelAbiIdentitySha256": abi.hex(),
        "kernelCount": 1,
        "productionKirIdentitySha256": kir_identity.hex(),
        "semanticMirIdentitySha256": semantic_identity,
        "sourceInventoryReceiptSha256": inventory.hex(),
        "sourcePreflightReceiptSha256": preflight.hex(),
    }
    return bundle, graph, kir, semantic


def rebind_record(batch: dict, index: int) -> None:
    record = batch["records"][index]
    record["recordBindingSha256"] = PROMOTER.record_binding_sha256(record)
    batch["batchBindingSha256"] = PROMOTER.batch_binding_sha256(batch)


def rebind_batch(batch: dict) -> None:
    batch["batchBindingSha256"] = PROMOTER.batch_binding_sha256(batch)


def build_batch(root: Path, raw: bytes, document: dict, candidate: dict) -> dict:
    fixtures = {item["fixtureId"]: item for item in document["compilerFixtures"]}
    kernels = {item["fixtureId"]: item for item in document["capabilityKernels"]}
    test_payload = (ROOT / NEGATIVE_TEST_PATH).read_bytes()
    test_sha = hashlib.sha256(test_payload).hexdigest()
    records = []
    for fixture_id in sorted(fixtures):
        fixture = fixtures[fixture_id]
        kernel = kernels[fixture_id]
        bundle, graph, kir, semantic_mir = build_v8(fixture["target"], fixture_id)
        stage = evidence_reference(
            root,
            f"{fixture_id}/sealed-stage-evidence.json",
            f'{{"fixtureId":"{fixture_id}","status":"test-only"}}\n'.encode("ascii"),
        )
        artifact = evidence_reference(
            root, f"{fixture_id}/artifact.hsaco", f"artifact:{fixture_id}\n".encode("ascii")
        )
        artifact_inspection = evidence_reference(
            root,
            f"{fixture_id}/artifact-inspection.json",
            f"artifact-inspection:{fixture_id}\n".encode("ascii"),
        )
        driver = evidence_reference(
            root,
            f"{fixture_id}/driver-identity.json",
            f"driver:{fixture_id}\n".encode("ascii"),
        )
        runtime = evidence_reference(
            root,
            f"{fixture_id}/runtime-identity.json",
            f"runtime:{fixture_id}\n".encode("ascii"),
        )
        v8 = evidence_reference(root, f"{fixture_id}/bundle-v8.bin", bundle)
        kir_ref = evidence_reference(root, f"{fixture_id}/optimized-kir-v13.bin", kir)
        mir_ref = evidence_reference(root, f"{fixture_id}/semantic-mir.bin", semantic_mir)
        files = {kind: deepcopy(stage) for kind in PROMOTER.ARCHIVE_KINDS}
        files["artifact"] = artifact
        files["artifact-inspection"] = artifact_inspection
        files["driver-identity"] = driver
        files["runtime-identity"] = runtime
        files["simulation-bundle-v8"] = v8
        files["optimized-kir-v13"] = kir_ref
        files["semantic-mir"] = mir_ref

        required_categories = sorted(
            PROMOTER.manifest_contract._required_negative_categories(
                set(kernel["requiredProperties"])
            )
        )
        negative_cases = [
            {
                "category": category,
                "diagnosticCode": f"FE2O3-CAP-{offset:03d}",
                "evidenceSha256": identity(
                    f"{fixture_id}:negative-evidence:{category}"
                ),
                "failureStage": "static-analysis",
                "fixtureId": f"{fixture_id}-negative-{offset:03d}",
                "testPath": NEGATIVE_TEST_PATH,
                "testSha256": test_sha,
            }
            for offset, category in enumerate(required_categories, start=1)
        ]
        manifest_cases = [
            {
                key: case[key]
                for key in (
                    "category",
                    "diagnosticCode",
                    "failureStage",
                    "fixtureId",
                    "testPath",
                )
            }
            for case in negative_cases
        ]
        negative_set = PROMOTER.manifest_contract._canonical_json_sha256(
            PROMOTER.manifest_contract.NEGATIVE_FIXTURE_DIGEST_DOMAIN,
            {"cases": manifest_cases, "fixtureId": fixture_id},
        )
        negative_preimage = (
            PROMOTER.manifest_contract.NEGATIVE_FIXTURE_DIGEST_DOMAIN
            + json.dumps(
                {"cases": manifest_cases, "fixtureId": fixture_id},
                allow_nan=False,
                ensure_ascii=True,
                separators=(",", ":"),
                sort_keys=True,
            ).encode("ascii")
        )
        files["negative-fixture-set"] = evidence_reference(
            root,
            f"{fixture_id}/negative-fixture-set.bin",
            negative_preimage,
        )
        properties = sorted(kernel["proofRequirements"]["properties"])
        obligation_bytes, obligations, result_bytes, results = build_capability_sets(
            fixture_id
        )
        lowering_bytes = f"lowering:{fixture_id}\n".encode("ascii")
        source_refinement_bytes = f"source-refinement:{fixture_id}\n".encode("ascii")
        machine_refinement_bytes = f"machine-refinement:{fixture_id}\n".encode("ascii")
        files["lowering"] = evidence_reference(
            root, f"{fixture_id}/lowering.bin", lowering_bytes
        )
        files["source-mir-to-kir-refinement"] = evidence_reference(
            root,
            f"{fixture_id}/source-mir-to-kir-refinement.bin",
            source_refinement_bytes,
        )
        files["machine-refinement"] = evidence_reference(
            root, f"{fixture_id}/machine-refinement.bin", machine_refinement_bytes
        )
        files["proof-obligation-set"] = evidence_reference(
            root, f"{fixture_id}/proof-obligation-set.bin", obligation_bytes
        )
        files["proof-evidence"] = evidence_reference(
            root, f"{fixture_id}/proof-result-set.bin", result_bytes
        )
        evidence = {
            key: stage["sha256"]
            for key in PROMOTER.manifest_contract.PRODUCTION_EVIDENCE_KEYS
            if key not in {"compilerCommit", "compilerTree"}
        }
        evidence.update(
            artifactSha256=artifact["sha256"],
            artifactInspectionSha256=artifact_inspection["sha256"],
            capabilityClosureSha256=stage["sha256"],
            compilerCommit=candidate["compilerCommit"],
            compilerTree=candidate["compilerTree"],
            finalOptimizedKirSha256=graph["productionKirIdentitySha256"],
            hardwareEvidenceSha256=stage["sha256"],
            negativeFixtureSetSha256=negative_set,
            loweringIdentitySha256=PROMOTER._length_delimited_identity(
                PROMOTER.LOWERING_RECEIPT_DOMAIN, lowering_bytes
            ),
            sourceMirToKirRefinementSha256=PROMOTER._length_delimited_identity(
                PROMOTER.SOURCE_REFINEMENT_RECEIPT_DOMAIN, source_refinement_bytes
            ),
            machineRefinementSha256=PROMOTER._length_delimited_identity(
                PROMOTER.MACHINE_REFINEMENT_RECEIPT_DOMAIN, machine_refinement_bytes
            ),
            proofCheckerSha256=PROMOTER._length_delimited_identity(
                PROMOTER.AUTHENTICATED_CHECKER_EVIDENCE_DOMAIN,
                (root / files["proof-checker"]["path"]).read_bytes(),
            ),
            proofEvidenceSha256=results,
            proofObligationSetSha256=obligations,
            simulatorEvidenceSha256=stage["sha256"],
            sourceMirIdentitySha256=graph["semanticMirIdentitySha256"],
        )
        proof_checker = evidence["proofCheckerSha256"]
        target_decision = evidence["targetCapabilityDecisionSha256"]
        target_identity = evidence["targetIdentitySha256"]
        simulator_command = PROMOTER.manifest_contract._expected_simulator_commands(
            document["qualification"], set(kernel["lessonIds"]), fixture_id
        )[0]
        hardware_command = PROMOTER.manifest_contract._expected_hardware_command(fixture)
        record = {
            "capabilityClosure": {
                "requirements": sorted(kernel["capabilityClosure"]["requirements"]),
                "sha256": evidence["capabilityClosureSha256"],
                "status": "complete",
            },
            "compilerInput": {
                key: fixture["compilerInput"][key]
                for key in (
                    "cargoLockSha256",
                    "contractSha256",
                    "packageManifestSha256",
                    "sourceClosureSha256",
                )
            },
            "evidenceFiles": files,
            "fixtureId": fixture_id,
            "graph": graph,
            "hardware": {
                "artifactInspectionSha256": evidence["artifactInspectionSha256"],
                "artifactSha256": evidence["artifactSha256"],
                "canariesChecked": True,
                "commandSha256": PROMOTER.command_sha256(hardware_command),
                "driverIdentitySha256": driver["sha256"],
                "evidenceSha256": evidence["hardwareEvidenceSha256"],
                "fullOutputChecked": True,
                "inputsUnchangedChecked": True,
                "lane": PROMOTER.HARDWARE_LANES[fixture["target"]],
                "launchContractSha256": evidence["launchContractSha256"],
                "paddingChecked": True,
                "runtimeIdentitySha256": runtime["sha256"],
                "status": "passed",
                "subjectSha256": evidence["artifactSha256"],
                "target": fixture["target"],
                "targetIdentitySha256": evidence["targetIdentitySha256"],
                "timeoutSeconds": hardware_command["timeoutSeconds"],
            },
            "kernelSymbol": kernel["kernelSymbol"],
            "lessonIds": sorted(kernel["lessonIds"]),
            "negativeFixtures": {
                "cases": negative_cases,
                "setSha256": negative_set,
                "status": "passed",
            },
            "productionEvidence": evidence,
            "productionTransaction": {
                "allowsFallback": False,
                "allowsPipelineSelection": False,
                "pipelineEntry": PROMOTER.PIPELINE_ENTRY,
                "policyVersion": PROMOTER.POLICY_VERSION,
                "status": "sealed-production-complete",
                "transactionSha256": identity(f"{fixture_id}:transaction"),
            },
            "proof": {
                "checkerSha256": proof_checker,
                "evidenceSha256": evidence["proofEvidenceSha256"],
                "obligationSetSha256": obligations,
                "properties": properties,
                "status": "complete",
            },
            "recordBindingSha256": "0" * 64,
            "simulator": {
                "commandSha256": PROMOTER.command_sha256(simulator_command),
                "evidenceSha256": evidence["simulatorEvidenceSha256"],
                "status": "passed",
                "subjectSha256": evidence["finalOptimizedKirSha256"],
            },
            "target": fixture["target"],
            "targetDecision": {
                "capabilityDecisionSha256": target_decision,
                "status": "capability-complete",
                "targetIdentitySha256": target_identity,
            },
        }
        record["recordBindingSha256"] = PROMOTER.record_binding_sha256(record)
        records.append(record)
    records_by_fixture = {record["fixtureId"]: record for record in records}
    suite_results = []
    for suite in document["qualification"]["suites"]:
        fixture_ids = {
            fixture_id
            for coverage in suite["coverage"]
            for fixture_id in coverage["fixtureIds"]
        }
        assert len(fixture_ids) == 1
        fixture_id = fixture_ids.pop()
        simulator_ref = records_by_fixture[fixture_id]["evidenceFiles"]["simulator"]
        suite_results.append(
            {
                "commandSha256": PROMOTER.command_sha256(suite["command"]),
                "coverage": suite["coverage"],
                "exitStatus": 0,
                "gate": suite["gate"],
                "stderrBytes": 0,
                "stderrSha256": hashlib.sha256(b"").hexdigest(),
                "stdoutBytes": simulator_ref["bytes"],
                "stdoutSha256": simulator_ref["sha256"],
                "status": "passed",
                "suiteId": suite["suiteId"],
            }
        )
    semantic_document = {
        "authority": {
            "compilerAuthority": False,
            "hardwareAuthority": False,
            "launchAuthority": False,
            "loadAuthority": False,
            "publicationAuthority": False,
        },
        "candidate": {
            "commit": candidate["compilerCommit"],
            "tree": candidate["compilerTree"],
            "worktreeClean": True,
        },
        "manifest": {
            "corpusContractSha256": PROMOTER.corpus_contract_sha256(document),
            "path": PROMOTER.MANIFEST_PATH,
            "rawSha256": hashlib.sha256(raw).hexdigest(),
        },
        "roadmapIssue": PROMOTER.ROADMAP_ISSUE,
        "schema": PROMOTER.SEMANTIC_QUALIFICATION_SCHEMA,
        "suites": suite_results,
    }
    semantic_ref = evidence_reference(
        root,
        "semantic-qualification-evidence-v1.json",
        PROMOTER._canonical(semantic_document) + b"\n",
    )
    batch = {
        "batchBindingSha256": "0" * 64,
        "candidate": candidate,
        "manifest": {
            "corpusContractSha256": PROMOTER.corpus_contract_sha256(document),
            "path": PROMOTER.MANIFEST_PATH,
            "rawSha256": hashlib.sha256(raw).hexdigest(),
        },
        "records": records,
        "roadmapIssue": PROMOTER.ROADMAP_ISSUE,
        "schema": PROMOTER.BATCH_SCHEMA,
        "semanticQualification": semantic_ref,
    }
    batch["batchBindingSha256"] = PROMOTER.batch_binding_sha256(batch)
    return batch


class TutorialCapabilityPromotionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.temporary = tempfile.TemporaryDirectory()
        cls.evidence_root = Path(cls.temporary.name)
        cls.raw, cls.document = manifest_with_v8_suites()
        cls.candidate = {
            "compilerCommit": "1" * 40,
            "compilerTree": "2" * 40,
            "worktreeClean": True,
        }
        cls.batch = build_batch(
            cls.evidence_root, cls.raw, cls.document, cls.candidate
        )

    @classmethod
    def tearDownClass(cls) -> None:
        cls.temporary.cleanup()

    def validate(self, batch: dict | None = None):
        return PROMOTER.validate_batch(
            self.document,
            self.raw,
            self.batch if batch is None else batch,
            self.candidate,
            ROOT,
            self.evidence_root,
        )

    def test_complete_batch_promotes_all_47_fixtures_and_25_lessons(self) -> None:
        validated = self.validate()
        self.assertEqual(47, len(validated.records))
        promoted = PROMOTER.build_promoted_manifest(
            self.document, self.candidate, validated.records
        )
        self.assertEqual(
            {"entries": 25, "fixtures": 47, "production_entries": 25},
            PROMOTER.manifest_contract.validate_document(
                promoted, require_qualified=True
            ),
        )
        self.assertEqual(
            {"compiler-produced"},
            {entry["classification"] for entry in promoted["entries"]},
        )

    def test_typed_receipt_and_set_identities_are_not_raw_archive_hashes(self) -> None:
        record = self.batch["records"][0]
        claims = record["productionEvidence"]
        files = record["evidenceFiles"]
        typed_archives = {
            "loweringIdentitySha256": "lowering",
            "sourceMirToKirRefinementSha256": "source-mir-to-kir-refinement",
            "machineRefinementSha256": "machine-refinement",
            "proofCheckerSha256": "proof-checker",
            "proofObligationSetSha256": "proof-obligation-set",
            "proofEvidenceSha256": "proof-evidence",
        }
        for claim, kind in typed_archives.items():
            self.assertNotEqual(claims[claim], files[kind]["sha256"])

            candidate = deepcopy(self.batch)
            candidate_record = candidate["records"][0]
            candidate_record["productionEvidence"][claim] = candidate_record[
                "evidenceFiles"
            ][kind]["sha256"]
            if claim == "proofObligationSetSha256":
                candidate_record["proof"]["obligationSetSha256"] = candidate_record[
                    "productionEvidence"
                ][claim]
            elif claim == "proofEvidenceSha256":
                candidate_record["proof"]["evidenceSha256"] = candidate_record[
                    "productionEvidence"
                ][claim]
            elif claim == "proofCheckerSha256":
                candidate_record["proof"]["checkerSha256"] = candidate_record[
                    "productionEvidence"
                ][claim]
            rebind_record(candidate, 0)
            with self.subTest(claim=claim):
                with self.assertRaisesRegex(
                    PROMOTER.PromotionError, "typed production identity"
                ):
                    self.validate(candidate)

    def test_sealed_producer_report_accepts_only_the_exact_batch(self) -> None:
        report = PROMOTER.expected_verification_report(self.batch, identity("verifier"))
        PROMOTER.validate_verification_report(report, self.batch)
        mutations = (
            lambda value: value.update(batchBindingSha256=identity("wrong-batch")),
            lambda value: value["candidate"].update(compilerTree="3" * 40),
            lambda value: value["records"].reverse(),
            lambda value: value["records"].pop(),
            lambda value: value["records"][0].update(
                productionTransactionSha256=identity("wrong-transaction")
            ),
            lambda value: value["records"][0].update(
                simulationBundleV8Sha256=identity("wrong-v8")
            ),
        )
        for mutate in mutations:
            candidate = deepcopy(report)
            mutate(candidate)
            with self.subTest(candidate=candidate):
                with self.assertRaisesRegex(PROMOTER.PromotionError, "report"):
                    PROMOTER.validate_verification_report(candidate, self.batch)

    def test_policy_mutation_requires_new_sealed_producer_authentication(self) -> None:
        report = PROMOTER.expected_verification_report(self.batch, identity("verifier"))
        candidate = deepcopy(self.batch)
        candidate["records"][0]["productionEvidence"]["compilerPolicySha256"] = identity(
            "different-compiler-policy"
        )
        rebind_record(candidate, 0)
        self.validate(candidate)
        with self.assertRaisesRegex(PROMOTER.PromotionError, "report"):
            PROMOTER.validate_verification_report(report, candidate)

    def test_rejects_missing_extra_and_reordered_fixture_records(self) -> None:
        missing = deepcopy(self.batch)
        missing["records"].pop()
        rebind_batch(missing)
        extra = deepcopy(self.batch)
        duplicate = deepcopy(extra["records"][0])
        extra["records"].append(duplicate)
        rebind_batch(extra)
        reordered = deepcopy(self.batch)
        reordered["records"][0], reordered["records"][1] = (
            reordered["records"][1],
            reordered["records"][0],
        )
        rebind_batch(reordered)
        for candidate in (missing, extra, reordered):
            with self.subTest(records=len(candidate["records"])):
                with self.assertRaisesRegex(PROMOTER.PromotionError, "every manifest fixture"):
                    self.validate(candidate)

    def test_rejects_cross_fixture_target_and_source_input_substitution(self) -> None:
        mutations = (
            ("every manifest fixture", lambda record: record.update(fixtureId="wrong-fixture")),
            ("kernel symbol", lambda record: record.update(kernelSymbol="wrong_kernel")),
            ("target", lambda record: record.update(target="gfx950")),
            (
                "sourceClosureSha256",
                lambda record: record["compilerInput"].update(
                    sourceClosureSha256=identity("wrong-source")
                ),
            ),
        )
        for expected, mutate in mutations:
            candidate = deepcopy(self.batch)
            mutate(candidate["records"][0])
            rebind_record(candidate, 0)
            with self.subTest(expected=expected):
                with self.assertRaisesRegex(PROMOTER.PromotionError, expected):
                    self.validate(candidate)

    def test_rejects_graph_epoch_kir_mir_and_v8_substitution(self) -> None:
        mutations = (
            ("graph coordinates", lambda record: record["graph"].update(finalGraphEpoch=99)),
            (
                "source/MIR or final graph",
                lambda record: record["productionEvidence"].update(
                    finalOptimizedKirSha256=identity("wrong-kir")
                ),
            ),
            (
                "source/MIR or final graph",
                lambda record: record["productionEvidence"].update(
                    sourceMirIdentitySha256=identity("wrong-mir")
                ),
            ),
        )
        for expected, mutate in mutations:
            candidate = deepcopy(self.batch)
            mutate(candidate["records"][0])
            rebind_record(candidate, 0)
            with self.subTest(expected=expected):
                with self.assertRaisesRegex(PROMOTER.PromotionError, expected):
                    self.validate(candidate)

        candidate = deepcopy(self.batch)
        v8_ref = candidate["records"][0]["evidenceFiles"]["simulation-bundle-v8"]
        v8_ref["sha256"] = identity("substituted-v8")
        rebind_record(candidate, 0)
        with self.assertRaisesRegex(PROMOTER.PromotionError, "content digest"):
            self.validate(candidate)

    def test_rejects_stale_artifact_simulator_hardware_and_proof_evidence(self) -> None:
        keys = (
            ("artifactSha256", "artifact digest"),
            ("simulatorEvidenceSha256", "simulator evidence digest"),
            ("hardwareEvidenceSha256", "hardware evidence digest"),
            ("proofEvidenceSha256", "proof identity"),
        )
        for key, expected in keys:
            candidate = deepcopy(self.batch)
            candidate["records"][0]["productionEvidence"][key] = identity(
                f"substituted:{key}"
            )
            rebind_record(candidate, 0)
            with self.subTest(key=key):
                with self.assertRaisesRegex(PROMOTER.PromotionError, expected):
                    self.validate(candidate)

    def test_hardware_is_mandatory_target_matched_and_complete(self) -> None:
        mutations = (
            ("hardware evidence", lambda hardware: hardware.update(status="skipped")),
            ("hardware evidence", lambda hardware: hardware.update(target="gfx950")),
            ("hardware evidence", lambda hardware: hardware.update(lane="mi350")),
            ("hardware evidence", lambda hardware: hardware.update(canariesChecked=False)),
            ("hardware evidence", lambda hardware: hardware.update(fullOutputChecked=False)),
            ("hardware evidence", lambda hardware: hardware.update(inputsUnchangedChecked=False)),
            ("hardware evidence", lambda hardware: hardware.update(paddingChecked=False)),
            ("hardware evidence", lambda hardware: hardware.update(timeoutSeconds=1)),
            (
                "hardware launchContractSha256",
                lambda hardware: hardware.update(
                    launchContractSha256=identity("wrong-hardware-launch")
                ),
            ),
            (
                "hardware artifactInspectionSha256",
                lambda hardware: hardware.update(
                    artifactInspectionSha256=identity("wrong-hardware-inspection")
                ),
            ),
        )
        for expected, mutate in mutations:
            candidate = deepcopy(self.batch)
            mutate(candidate["records"][0]["hardware"])
            rebind_record(candidate, 0)
            with self.subTest(expected=expected):
                with self.assertRaisesRegex(PROMOTER.PromotionError, expected):
                    self.validate(candidate)

    def test_promotion_makes_hardware_a_required_gate_for_every_lesson(self) -> None:
        validated = self.validate()
        promoted = PROMOTER.build_promoted_manifest(
            self.document, self.candidate, validated.records
        )
        self.assertTrue(
            all("hardware" in entry["requiredGates"] for entry in promoted["entries"])
        )
        self.assertEqual(
            [
                {"lane": "mi300x", "status": "required-qualified", "target": "gfx942"},
                {"lane": "mi350", "status": "required-qualified", "target": "gfx950"},
            ],
            promoted["qualification"]["hardwareTargets"],
        )

    def test_rejects_proof_target_command_and_negative_fixture_drift(self) -> None:
        mutations = (
            (
                "proofObligationSetSha256",
                lambda record: (
                    record["proof"].update(obligationSetSha256=identity("wrong-obligation")),
                    record["productionEvidence"].update(
                        proofObligationSetSha256=identity("wrong-obligation")
                    ),
                ),
            ),
            (
                "target capability decision",
                lambda record: record["targetDecision"].update(
                    targetIdentitySha256=identity("wrong-target")
                ),
            ),
            (
                "simulator evidence",
                lambda record: record["simulator"].update(
                    commandSha256=identity("wrong-command")
                ),
            ),
            (
                "negative fixture set",
                lambda record: record["negativeFixtures"]["cases"][0].update(
                    diagnosticCode="FE2O3-CAP-999"
                ),
            ),
        )
        for expected, mutate in mutations:
            candidate = deepcopy(self.batch)
            mutate(candidate["records"][0])
            rebind_record(candidate, 0)
            with self.subTest(expected=expected):
                with self.assertRaisesRegex(PROMOTER.PromotionError, expected):
                    self.validate(candidate)

        invalid_case_fields = (
            ("category", "invented-category"),
            ("diagnosticCode", "not-a-diagnostic"),
            ("failureStage", "invented-stage"),
        )
        for key, value in invalid_case_fields:
            candidate = deepcopy(self.batch)
            candidate["records"][0]["negativeFixtures"]["cases"][0][key] = value
            rebind_record(candidate, 0)
            with self.subTest(key=key):
                with self.assertRaisesRegex(PROMOTER.PromotionError, f"{key} is invalid"):
                    self.validate(candidate)

    def test_rejects_non_v8_simulator_command(self) -> None:
        raw, document = manifest_with_v8_suites()
        document["qualification"]["suites"][0]["command"]["arguments"][3] = "7"
        raw = (json.dumps(document, ensure_ascii=True, indent=2) + "\n").encode("ascii")
        with tempfile.TemporaryDirectory() as temporary:
            batch = build_batch(Path(temporary), raw, document, self.candidate)
            with self.assertRaisesRegex(PROMOTER.PromotionError, "Bundle V8"):
                PROMOTER.validate_batch(
                    document,
                    raw,
                    batch,
                    self.candidate,
                    ROOT,
                    Path(temporary),
                )

    def test_rejects_reused_transaction_and_stale_batch_binding(self) -> None:
        reused = deepcopy(self.batch)
        reused["records"][1]["productionTransaction"]["transactionSha256"] = reused[
            "records"
        ][0]["productionTransaction"]["transactionSha256"]
        rebind_record(reused, 1)
        with self.assertRaisesRegex(PROMOTER.PromotionError, "reused"):
            self.validate(reused)

        stale = deepcopy(self.batch)
        stale["manifest"]["corpusContractSha256"] = identity("stale-corpus")
        with self.assertRaisesRegex(PROMOTER.PromotionError, "input manifest"):
            self.validate(stale)

    def test_rejects_stale_or_authority_bearing_semantic_qualification(self) -> None:
        reference = self.batch["semanticQualification"]
        original = self.evidence_root / reference["path"]
        document = json.loads(original.read_text())
        mutations = (
            (
                "authority-free",
                lambda value: value["authority"].update(compilerAuthority=True),
            ),
            (
                "compiler candidate",
                lambda value: value["candidate"].update(tree="3" * 40),
            ),
            (
                "stale, reordered, or did not pass",
                lambda value: value["suites"][0].update(status="failed"),
            ),
            (
                "stale, reordered, or did not pass",
                lambda value: value["suites"].reverse(),
            ),
        )
        for expected, mutate in mutations:
            with tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                candidate = deepcopy(self.batch)
                mutated = deepcopy(document)
                mutate(mutated)
                candidate["semanticQualification"] = evidence_reference(
                    root,
                    "semantic.json",
                    PROMOTER._canonical(mutated) + b"\n",
                )
                for record in candidate["records"]:
                    for evidence in record["evidenceFiles"].values():
                        source = self.evidence_root / evidence["path"]
                        destination = root / evidence["path"]
                        destination.parent.mkdir(parents=True, exist_ok=True)
                        if not destination.exists():
                            destination.write_bytes(source.read_bytes())
                rebind_batch(candidate)
                with self.subTest(expected=expected):
                    with self.assertRaisesRegex(PROMOTER.PromotionError, expected):
                        PROMOTER.validate_batch(
                            self.document,
                            self.raw,
                            candidate,
                            self.candidate,
                            ROOT,
                            root,
                        )

    def test_rejects_symlinked_and_changed_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            payload = b"evidence\n"
            real = root / "real"
            real.write_bytes(payload)
            link = root / "link"
            link.symlink_to(real.name)
            reference = {
                "bytes": len(payload),
                "path": "link",
                "sha256": hashlib.sha256(payload).hexdigest(),
            }
            with self.assertRaisesRegex(PROMOTER.PromotionError, "symlink"):
                PROMOTER._snapshot_file(root, reference, "evidence")
            reference["path"] = "real"
            reference["sha256"] = identity("wrong")
            with self.assertRaisesRegex(PROMOTER.PromotionError, "content digest"):
                PROMOTER._snapshot_file(root, reference, "evidence")

            reference["sha256"] = hashlib.sha256(payload).hexdigest()
            snapshot = PROMOTER._snapshot_file(root, reference, "evidence")
            real.unlink()
            real.symlink_to("link")
            with self.assertRaisesRegex(PROMOTER.PromotionError, "cannot read"):
                PROMOTER._read_snapshot(snapshot, "evidence", len(payload))

    def test_clean_candidate_rejects_tracked_and_untracked_dirt(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repository = Path(temporary)
            subprocess.run(["git", "init", "-q", repository], check=True)
            subprocess.run(
                ["git", "-C", repository, "config", "user.name", "Promotion Test"],
                check=True,
            )
            subprocess.run(
                [
                    "git",
                    "-C",
                    repository,
                    "config",
                    "user.email",
                    "promotion@example.invalid",
                ],
                check=True,
            )
            tracked = repository / "tracked"
            tracked.write_text("clean\n")
            subprocess.run(["git", "-C", repository, "add", "tracked"], check=True)
            subprocess.run(
                ["git", "-C", repository, "commit", "-q", "-m", "initial"],
                check=True,
            )
            self.assertTrue(PROMOTER.clean_candidate(repository)["worktreeClean"])
            (repository / "untracked").write_text("dirty\n")
            with self.assertRaisesRegex(PROMOTER.PromotionError, "clean compiler worktree"):
                PROMOTER.clean_candidate(repository)
            (repository / "untracked").unlink()
            tracked.write_text("dirty\n")
            with self.assertRaisesRegex(PROMOTER.PromotionError, "clean compiler worktree"):
                PROMOTER.clean_candidate(repository)

    def test_atomic_generation_refuses_replacement_and_contains_receipt(self) -> None:
        verification = PROMOTER.expected_verification_report(
            self.batch, identity("verifier")
        )
        payload = b'{"qualified":true}\n'
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "generation"
            PROMOTER.publish_generation(output, payload, self.batch, verification)
            manifest_path = output / "config" / PROMOTER.manifest_contract.MANIFEST_NAME
            self.assertEqual(payload, manifest_path.read_bytes())
            self.assertTrue((output / "promotion-receipt-v1.json").is_file())
            with self.assertRaisesRegex(PROMOTER.PromotionError, "existing"):
                PROMOTER.publish_generation(output, payload, self.batch, verification)

    def test_schema_is_closed_and_content_addressed(self) -> None:
        name = PROMOTER.manifest_contract.CAPABILITY_QUALIFICATION_SCHEMA_NAME
        path = ROOT / "config" / name
        raw = path.read_bytes()
        schema = json.loads(raw)
        self.assertFalse(schema["additionalProperties"])
        self.assertEqual(
            PROMOTER.BATCH_SCHEMA,
            schema["properties"]["schema"]["const"],
        )
        self.assertEqual(
            PROMOTER.ARCHIVE_KINDS,
            set(schema["$defs"]["evidenceFiles"]["required"]),
        )
        self.assertEqual(
            f"{hashlib.sha256(raw).hexdigest()}  config/{name}\n",
            path.with_suffix(".sha256").read_text(),
        )


if __name__ == "__main__":
    unittest.main()
