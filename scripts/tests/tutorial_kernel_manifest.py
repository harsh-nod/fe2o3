#!/usr/bin/env python3

from __future__ import annotations

from copy import deepcopy
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch


sys.dont_write_bytecode = True
SCRIPT = Path(__file__).resolve().parents[1] / "tutorial_kernel_manifest.py"
SPEC = importlib.util.spec_from_file_location("tutorial_kernel_manifest", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)
ROOT = SCRIPT.parent.parent
EXPECTED_HARDWARE_RUNNERS = {
    "examples/fill/run-gfx942.sh",
    "examples/flash_attention_general_v1/run-gfx942.sh",
    "examples/gemm_autoresearch_v1/run-gfx942.sh",
    "examples/gfx950_advanced_attention/run-attnres-aggregate-gfx950.sh",
    "examples/gfx950_advanced_attention/run-compressed-hybrid-attention-gfx950.sh",
    "examples/gfx950_advanced_attention/run-content-sparse-attention-gfx950.sh",
    "examples/gfx950_advanced_attention/run-deepseek-sparse-attention-gfx950.sh",
    "examples/gfx950_advanced_attention/run-four-branch-residual-gfx950.sh",
    "examples/gfx950_advanced_attention/run-gfx950.sh",
    "examples/gfx950_advanced_attention/run-kda-chunkwise-prefill-gfx950.sh",
    "examples/gfx950_advanced_attention/run-kda-decode-gfx950.sh",
    "examples/gfx950_advanced_attention/run-mhc-sinkhorn-mix-gfx950.sh",
    "examples/gfx950_advanced_systems/run-combine-expert-ranks-gfx950.sh",
    "examples/gfx950_advanced_systems/run-moe-expert-rank-gfx950.sh",
    "examples/gfx950_advanced_systems/run-moe-route-gfx950.sh",
    "examples/gfx950_advanced_systems/run-muon-update-gfx950.sh",
    "examples/gfx950_advanced_systems/run-qwen-ngram-gather-gfx950.sh",
    "examples/gfx950_advanced_systems/run-speculative-transaction-gfx950.sh",
    "examples/gfx950_advanced_systems/run-stage-gradient-shard-gfx950.sh",
    "examples/gfx950_gpt_oss_decode/run-ablation-gfx950.sh",
    "examples/gfx950_gpt_oss_decode/run-gfx950.sh",
    "examples/gfx950_low_precision/run-fp4-attention-gfx950.sh",
    "examples/gfx950_low_precision/run-fp4-gemm-gfx950.sh",
    "examples/gfx950_low_precision/run-fp8-attention-gfx950.sh",
    "examples/gfx950_low_precision/run-fp8-gemm-gfx950.sh",
    "examples/moe_grouped_expert_general_v1/run-gfx942.sh",
    "examples/moe_top2_v1/run-gfx942.sh",
    "examples/row_softmax_general_v1/run-gfx942.sh",
    "examples/tiled_gemm_general_v1/run-gfx942.sh",
    "examples/vecadd/run-gfx942.sh",
    "examples/wave64_collectives_v1/run-gfx942.sh",
    "examples/workgroup_sync_v1/run-gfx942.sh",
}


def manifest() -> dict:
    return json.loads((ROOT / "config" / CHECKER.MANIFEST_NAME).read_text())


def identity(label: str) -> str:
    return hashlib.sha256(label.encode("ascii")).hexdigest()


def promote_fill(document: dict) -> dict:
    fixture_id = "gfx942-fill-simulation"
    fixture = next(
        item for item in document["compilerFixtures"] if item["fixtureId"] == fixture_id
    )
    kernel = next(
        item for item in document["capabilityKernels"] if item["fixtureId"] == fixture_id
    )
    for entry in document["entries"]:
        if fixture_id in entry["compilerFixtureIds"]:
            entry["classification"] = "compiler-produced"

    def digest(name: str) -> str:
        return identity(f"{fixture_id}:{name}")

    closure = digest("closure")
    artifact = digest("artifact")
    final_kir = digest("final-kir")
    target_decision = digest("target-decision")
    target_identity = digest("target-identity")
    proof_properties = sorted(CHECKER.PRODUCTION_PROOF_PROPERTIES)
    proof_obligations = digest("proof-obligation-set")
    proof_checker = digest("proof-checker")
    proof_evidence = digest("proof-evidence")
    negative_cases = [
        {
            "category": category,
            "diagnosticCode": f"FE2O3-CAP-{offset:03d}",
            "failureStage": "static-analysis",
            "fixtureId": f"fill-negative-{offset}",
            "testPath": f"tests/fill-negative-{offset}.rs",
        }
        for offset, category in enumerate(
            sorted(CHECKER._required_negative_categories(set(kernel["requiredProperties"]))),
            start=1,
        )
    ]
    negative_identity = CHECKER._canonical_json_sha256(
        CHECKER.NEGATIVE_FIXTURE_DIGEST_DOMAIN,
        {"cases": negative_cases, "fixtureId": fixture_id},
    )
    simulator_evidence = digest("simulator-evidence")
    hardware_evidence = digest("hardware-evidence")
    simulator_commands = CHECKER._expected_simulator_commands(
        document["qualification"], set(kernel["lessonIds"]), fixture_id
    )
    assert len(simulator_commands) == 1

    kernel["capabilityClosure"].update(status="complete", sha256=closure)
    kernel["proofRequirements"] = {
        "checkerSha256": proof_checker,
        "evidenceSha256": proof_evidence,
        "obligationSetSha256": proof_obligations,
        "properties": proof_properties,
        "status": "complete",
    }
    kernel["targetMatrix"][0]["status"] = "requirements-derived"
    kernel["targetMatrix"][1].update(
        capabilityDecisionSha256=target_decision,
        status="capability-complete",
        targetIdentitySha256=target_identity,
    )
    kernel["simulatorCommand"].update(
        command=simulator_commands[0],
        evidenceSha256=simulator_evidence,
        reasonCode=None,
        status="capability-path-qualified",
        subjectSha256=final_kir,
    )
    kernel["hardwareCommand"].update(
        evidenceSha256=hardware_evidence,
        reasonCode=None,
        status="capability-path-qualified",
        subjectSha256=artifact,
    )
    kernel["negativeFixtureCoverage"] = {
        "cases": negative_cases,
        "status": "complete",
    }
    kernel["productionCapabilityPath"] = {
        "evidence": {
            "artifactSha256": artifact,
            "artifactInspectionSha256": digest("artifact-inspection"),
            "capabilityAnalysisSha256": digest("capability-analysis"),
            "capabilityClosureSha256": closure,
            "compilerCommit": document["baseline"]["compilerCommit"],
            "compilerPolicySha256": digest("compiler-policy"),
            "compilerTree": document["baseline"]["compilerTree"],
            "finalOptimizedKirSha256": final_kir,
            "hardwareEvidenceSha256": hardware_evidence,
            "hostAdmissionSha256": digest("host-admission"),
            "launchContractSha256": digest("launch-contract"),
            "loweringIdentitySha256": digest("lowering"),
            "machineRefinementSha256": digest("machine-refinement"),
            "negativeFixtureSetSha256": negative_identity,
            "numericalPolicySha256": digest("numerical-policy"),
            "proofCheckerSha256": proof_checker,
            "proofEvidenceSha256": proof_evidence,
            "proofObligationSetSha256": proof_obligations,
            "simulatorEvidenceSha256": simulator_evidence,
            "sourceMirIdentitySha256": digest("source-mir"),
            "sourceMirToKirRefinementSha256": digest("source-refinement"),
            "targetCapabilityDecisionSha256": target_decision,
            "targetIdentitySha256": target_identity,
        },
        "path": "canonical-capability",
        "status": "complete",
    }
    self_check = CHECKER._expected_hardware_command(fixture)
    assert kernel["hardwareCommand"]["command"] == self_check
    return kernel


class TutorialKernelManifestTests(unittest.TestCase):
    def test_checked_in_manifest_is_valid(self) -> None:
        stats = CHECKER.validate_repository(ROOT)
        self.assertEqual(
            {"entries": 25, "fixtures": 47, "production_entries": 0}, stats
        )

    def test_release_gate_rejects_migration_manifest(self) -> None:
        with self.assertRaisesRegex(
            CHECKER.ManifestError, "release requires a fully qualified capability manifest"
        ):
            CHECKER.validate_repository(ROOT, require_qualified=True)

    def test_all_manifest_hardware_runners_use_authenticated_transport(self) -> None:
        document = manifest()
        fixtures = {
            fixture["fixtureId"]: fixture for fixture in document["compilerFixtures"]
        }
        runners = {
            fixture["matrix"]["runnerPath"] for fixture in fixtures.values()
        }
        self.assertEqual(EXPECTED_HARDWARE_RUNNERS, runners)
        for runner in sorted(runners):
            path = ROOT / runner
            self.assertTrue(path.stat().st_mode & 0o111, runner)
            self.assertTrue(
                path.read_bytes().startswith(CHECKER.HARDWARE_RUNNER_PROLOGUE), runner
            )
        for kernel in document["capabilityKernels"]:
            fixture = fixtures[kernel["fixtureId"]]
            environment = fixture["matrix"]["environment"]
            hardware = kernel["hardwareCommand"]
            self.assertEqual(1, environment.count(CHECKER.HARDWARE_PROTOCOL_ENVIRONMENT))
            self.assertEqual(environment, hardware["command"]["environment"])
            self.assertEqual("available-authenticated-unobserved", hardware["status"])
            self.assertEqual(CHECKER.UNOBSERVED_HARDWARE_REASON, hardware["reasonCode"])
            self.assertIsNone(hardware["subjectSha256"])
            self.assertIsNone(hardware["evidenceSha256"])

    def test_rejects_authenticated_protocol_omission(self) -> None:
        document = manifest()
        fixture = document["compilerFixtures"][0]
        kernel = document["capabilityKernels"][0]
        fixture["matrix"]["environment"].remove(
            CHECKER.HARDWARE_PROTOCOL_ENVIRONMENT
        )
        kernel["hardwareCommand"]["command"]["environment"].remove(
            CHECKER.HARDWARE_PROTOCOL_ENVIRONMENT
        )
        fixture["compilerInput"]["contractSha256"] = (
            CHECKER._fixture_input_contract_sha256(fixture)
        )
        with self.assertRaisesRegex(
            CHECKER.ManifestError, "authenticated hardware protocol"
        ):
            CHECKER.validate_document(document)

    def test_repository_validation_rejects_runner_hook_bypass(self) -> None:
        original = CHECKER._repository_file

        def omit_hook(root, relative, label, **options):
            path, payload = original(root, relative, label, **options)
            if label.endswith(".matrix.runnerPath"):
                payload = payload.removeprefix(CHECKER.HARDWARE_RUNNER_PROLOGUE)
            return path, payload

        with patch.object(CHECKER, "_repository_file", side_effect=omit_hook):
            with self.assertRaisesRegex(CHECKER.ManifestError, "shared authenticated"):
                CHECKER.validate_repository(ROOT)

    def test_rejects_duplicate_fixture_identity(self) -> None:
        document = manifest()
        document["compilerFixtures"].append(deepcopy(document["compilerFixtures"][0]))
        with self.assertRaisesRegex(CHECKER.ManifestError, "duplicate compilerFixtures"):
            CHECKER.validate_document(document)

    def test_rejects_stale_compiler_input_contract(self) -> None:
        document = manifest()
        document["compilerFixtures"][0]["compilerInput"]["contractSha256"] = "0" * 64
        with self.assertRaisesRegex(CHECKER.ManifestError, "contractSha256 is stale"):
            CHECKER.validate_document(document)

    def test_rejects_label_only_production_promotion(self) -> None:
        document = manifest()
        lessons = set(document["capabilityKernels"][0]["lessonIds"])
        for entry in document["entries"]:
            if entry["lessonId"] in lessons:
                entry["classification"] = "compiler-produced"
        with self.assertRaisesRegex(CHECKER.ManifestError, "complete capability closure"):
            CHECKER.validate_document(document)

    def test_rejects_complete_authority_on_nonproduction_entry(self) -> None:
        document = manifest()
        document["capabilityKernels"][0]["productionCapabilityPath"] = {
            "status": "complete",
            "path": "canonical-capability",
            "evidence": {},
        }
        with self.assertRaisesRegex(CHECKER.ManifestError, "carries production authority"):
            CHECKER.validate_document(document)

    def test_complete_kernel_requires_exact_cross_record_joins(self) -> None:
        document = manifest()
        kernel = promote_fill(document)
        CHECKER.validate_document(document)

        mutations = (
            (
                "baseline",
                lambda selected: selected["productionCapabilityPath"]["evidence"].update(
                    compilerCommit="f" * 40
                ),
            ),
            (
                "proof identities",
                lambda selected: selected["proofRequirements"].update(
                    evidenceSha256="f" * 64
                ),
            ),
            (
                "target decision",
                lambda selected: selected["targetMatrix"][1].update(
                    targetIdentitySha256="f" * 64
                ),
            ),
            (
                "command evidence",
                lambda selected: selected["simulatorCommand"].update(
                    subjectSha256="f" * 64
                ),
            ),
            (
                "negative-fixture identity",
                lambda selected: selected["negativeFixtureCoverage"]["cases"][0].update(
                    diagnosticCode="FE2O3-CAP-999"
                ),
            ),
        )
        for expected, mutate in mutations:
            candidate = deepcopy(document)
            kernel = next(
                item
                for item in candidate["capabilityKernels"]
                if item["fixtureId"] == "gfx942-fill-simulation"
            )
            mutate(kernel)
            with self.assertRaisesRegex(CHECKER.ManifestError, expected):
                CHECKER.validate_document(candidate)

    def test_rejects_content_digest_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / "record.json"
            digest = root / "record.sha256"
            path.write_text("{}\n")
            digest.write_text(f"{'0' * 64}  config/record.json\n")
            with self.assertRaisesRegex(CHECKER.ManifestError, "does not exactly address"):
                CHECKER._read_content_addressed(path, digest)

    def test_release_file_check_rejects_symlinks_and_nonexecutables(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            regular = root / "runner.sh"
            regular.write_text("#!/bin/sh\nexit 0\n")
            link = root / "link.sh"
            link.symlink_to(regular.name)
            with self.assertRaisesRegex(CHECKER.ManifestError, "traverses a symlink"):
                CHECKER._repository_file(root, "link.sh", "runner", executable=True)
            with self.assertRaisesRegex(CHECKER.ManifestError, "not executable"):
                CHECKER._repository_file(root, "runner.sh", "runner", executable=True)

    def test_rejects_site_parity_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            site = Path(temporary)
            (site / "config").mkdir()
            for name in (
                CHECKER.MANIFEST_NAME,
                CHECKER.MANIFEST_NAME.replace(".json", ".sha256"),
                CHECKER.SCHEMA_NAME,
                CHECKER.SCHEMA_NAME.replace(".json", ".sha256"),
                CHECKER.EXPECTATION_SCHEMA_NAME,
                CHECKER.EXPECTATION_SCHEMA_NAME.replace(".json", ".sha256"),
                CHECKER.EVIDENCE_SCHEMA_NAME,
                CHECKER.EVIDENCE_SCHEMA_NAME.replace(".json", ".sha256"),
                CHECKER.CAPABILITY_QUALIFICATION_SCHEMA_NAME,
                CHECKER.CAPABILITY_QUALIFICATION_SCHEMA_NAME.replace(
                    ".json", ".sha256"
                ),
            ):
                (site / "config" / name).write_bytes((ROOT / "config" / name).read_bytes())
            (site / "config" / CHECKER.MANIFEST_NAME).write_text("{}\n")
            with self.assertRaisesRegex(CHECKER.ManifestError, "differs byte-for-byte"):
                CHECKER.validate_repository(ROOT, site)


if __name__ == "__main__":
    unittest.main()
