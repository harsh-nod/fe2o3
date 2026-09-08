#!/usr/bin/env python3
"""Generate the canonical 47-fixture negative-test declaration roster."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import sys
from typing import Any

import tutorial_kernel_manifest as manifest_contract
import tutorial_negative_fixtures as negative_contract


REPO_ROOT = Path(__file__).resolve().parent.parent
MANIFEST = REPO_ROOT / "config" / manifest_contract.MANIFEST_NAME
OUTPUT = REPO_ROOT / negative_contract.DECLARATION_PATH
DIGEST = REPO_ROOT / negative_contract.DECLARATION_DIGEST_PATH
EMPTY_SHA256 = hashlib.sha256(b"").hexdigest()


EXECUTION_SPECS = {
    "abi-collector": {
        "arguments": [
            "abi-collector",
            "crates/rustc-codegen-fe2o3/Cargo.toml",
            "kernel_context_abi_normalization_v1",
            "collector_rejects_each_logical_physical_abi_substitution",
            "ignored",
        ],
        "executable": "scripts/run-tutorial-negative-test.sh",
        "harnessPath": "crates/rustc-codegen-fe2o3/tests/kernel_context_abi_normalization_v1.rs",
        "timeoutSeconds": 1800,
    },
    "capability-type-system": {
        "arguments": [
            "capability-type-system",
            "crates/fe2o3-device/Cargo.toml",
            "device_api_ui",
            "device_api_enforces_witness_boundaries",
        ],
        "executable": "scripts/run-tutorial-negative-test.sh",
        "harnessPath": "crates/fe2o3-device/tests/device_api_ui.rs",
        "timeoutSeconds": 1800,
    },
    "evidence-protocol": {
        "arguments": ["evidence"],
        "executable": "scripts/tutorial-negative-protocol-hostile.py",
        "harnessPath": "scripts/tutorial-negative-protocol-hostile.py",
        "timeoutSeconds": 30,
    },
    "host-invocation": {
        "arguments": [
            "host-invocation",
            "crates/cargo-fe2o3/Cargo.toml",
            "production_build_config",
            "production_runner_rejects_no_envelope_marker",
        ],
        "executable": "scripts/run-tutorial-negative-test.sh",
        "harnessPath": "crates/cargo-fe2o3/tests/production_build_config.rs",
        "timeoutSeconds": 1800,
    },
    "launch-typestate": {
        "arguments": [
            "launch-typestate",
            "crates/fe2o3-host/Cargo.toml",
            "prepared_launch_ui",
            "prepared_launch_types_cannot_be_forged_or_crossed",
        ],
        "executable": "scripts/run-tutorial-negative-test.sh",
        "harnessPath": "crates/fe2o3-host/tests/prepared_launch_ui.rs",
        "timeoutSeconds": 1800,
    },
    "stale-output-protocol": {
        "arguments": ["stale-output"],
        "executable": "scripts/tutorial-negative-protocol-hostile.py",
        "harnessPath": "scripts/tutorial-negative-protocol-hostile.py",
        "timeoutSeconds": 30,
    },
    "target-selection": {
        "arguments": [
            "target-selection",
            "crates/cargo-fe2o3/Cargo.toml",
            "production_build_config",
            "production_driver_rejects_caller_target_selection",
        ],
        "executable": "scripts/run-tutorial-negative-test.sh",
        "harnessPath": "crates/cargo-fe2o3/tests/production_build_config.rs",
        "timeoutSeconds": 1800,
    },
}


CATEGORY_SPECS = {
    "abi": ("compiler-input", "mir-admission", "abi-layout", ["compilerInput", "contractSha256"]),
    "alias": ("optimized-kir-v13", "static-analysis", "alias-overlap", ["semanticContract", "alias"]),
    "bounds": ("optimized-kir-v13", "static-analysis", "index-out-of-bounds", ["semanticContract", "bounds"]),
    "capability-forgery": ("compiler-input", "macro-authentication", "capability-forgery", ["semanticContract", "capability"]),
    "capability-substitution": ("compiler-input", "static-analysis", "cross-kernel-capability", ["kernelSymbol"]),
    "evidence": ("evidence", "sealed-verifier", "evidence-substitution", ["evidence", "semanticFixtureRawSha256"]),
    "host-invocation": ("receipt", "host-preparation", "host-invocation", ["receipt", "hostInvocation"]),
    "initialization": ("optimized-kir-v13", "static-analysis", "uninitialized-read", ["semanticContract", "initialization"]),
    "launch": ("receipt", "host-preparation", "launch-shape", ["launch", "grid", 0]),
    "raw-pointer": ("optimized-kir-v13", "static-analysis", "raw-pointer-effect", ["semanticContract", "operation"]),
    "stale-output": ("evidence", "sealed-verifier", "output-substitution", ["evidence", "expectedOutputSha256"]),
    "synchronization": ("optimized-kir-v13", "static-analysis", "barrier-epoch-order", ["semanticContract", "order"]),
    "target": ("compiler-input", "target-legalization", "cross-target", ["target"]),
    "unsupported-operation": ("optimized-kir-v13", "kir-verification", "unsupported-operation", ["semanticContract", "operation"]),
}

CATEGORY_PROPERTIES = {
    "abi": {"abi-conformance"},
    "alias": {"alias-legality", "output-injectivity", "race-freedom"},
    "bounds": {"address-bounds"},
    "initialization": {"initialized-before-read"},
    "raw-pointer": {"effect-legality"},
    "synchronization": {
        "atomic-legality",
        "barrier-convergence",
        "collective-participation",
        "happens-before",
        "workgroup-memory-epochs",
    },
}

ADVANCED_VARIANTS = {
    "functional-numerical": ("evidence", "evidence", "sealed-verifier", ["evidence", "numericalPolicySha256"]),
    "functional-order": ("launch", "optimized-kir-v13", "static-analysis", ["semanticContract", "order"]),
    "functional-output": ("stale-output", "evidence", "sealed-verifier", ["evidence", "expectedOutputSha256"]),
    "functional-recurrence": ("unsupported-operation", "optimized-kir-v13", "kir-verification", ["semanticContract", "recurrence"]),
}

MUTATION_VALUE_DOMAIN = b"fe2o3-tutorial-negative-mutation-value-v2\0"
CASE_HARNESS = "scripts/tutorial_negative_fixtures.py"


def _sha(path: str) -> str:
    return hashlib.sha256((REPO_ROOT / path).read_bytes()).hexdigest()


def _command(spec: dict[str, Any]) -> dict[str, Any]:
    return {
        "arguments": spec["arguments"],
        "environment": [],
        "executable": spec["executable"],
        "timeoutSeconds": spec["timeoutSeconds"],
        "workingDirectory": ".",
    }


def _required_property(kernel: dict[str, Any], category: str) -> str:
    properties = sorted(kernel["requiredProperties"])
    candidates = sorted(set(properties) & CATEGORY_PROPERTIES.get(category, set()))
    if candidates:
        return candidates[0]
    if "functional-refinement" in properties:
        return "functional-refinement"
    return properties[0]


def _advanced_family(fixture_id: str, symbol: str) -> str | None:
    identity = f"{fixture_id}:{symbol}"
    if "kda" in identity:
        return "kda"
    if "gemm" in identity:
        return "gemm"
    if any(token in identity for token in ("attention", "attn", "flash")):
        return "attention"
    if any(token in identity for token in ("moe", "expert", "gpt-oss")):
        return "moe"
    return None


def _replacement(
    document: dict[str, Any],
    fixture: dict[str, Any],
    mutation_class: str,
    path: list[str | int],
    case_id: str,
) -> Any:
    if path == ["target"]:
        return "gfx950" if fixture["target"] == "gfx942" else "gfx942"
    if path == ["launch", "grid", 0]:
        semantic = json.loads(
            (
                REPO_ROOT
                / negative_contract.SEMANTIC_FIXTURE_ROOT
                / f"{fixture['fixtureId']}.json"
            ).read_bytes()
        )
        return semantic["request"]["grid"][0] + 1
    if path == ["kernelSymbol"]:
        symbols = sorted(
            kernel["kernelSymbol"]
            for kernel in document["capabilityKernels"]
            if kernel["fixtureId"] != fixture["fixtureId"]
        )
        return symbols[0]
    return negative_contract._domain_sha256(
        MUTATION_VALUE_DOMAIN,
        {
            "caseId": case_id,
            "class": mutation_class,
            "compilerInputContractSha256": fixture["compilerInput"]["contractSha256"],
        },
    )


def _case(
    document: dict[str, Any],
    fixture: dict[str, Any],
    kernel: dict[str, Any],
    category: str,
    mutation_class: str,
    subject_kind: str,
    stage: str,
    path: list[str | int],
    suffix: str,
) -> dict[str, Any]:
    fixture_id = fixture["fixtureId"]
    case_id = f"{fixture_id}--{suffix}"
    required_property = _required_property(kernel, category)
    base = negative_contract.base_payload(
        REPO_ROOT, fixture, kernel, required_property
    )
    mutation = {
        "class": mutation_class,
        "path": path,
        "replacement": _replacement(
            document, fixture, mutation_class, path, case_id
        ),
        "subjectKind": subject_kind,
    }
    mutated = negative_contract.apply_mutation(base, mutation)
    return {
        "basePayloadSha256": negative_contract._domain_sha256(
            negative_contract.BASE_PAYLOAD_DOMAIN, base
        ),
        "caseId": case_id,
        "category": category,
        "diagnosticCode": "",
        "expectedResult": dict(negative_contract.UNAVAILABLE_RESULT),
        "failureStage": stage,
        "fixtureBindingSha256": negative_contract.fixture_binding_sha256(
            fixture, kernel
        ),
        "fixtureId": fixture_id,
        "mutatedPayloadSha256": negative_contract._domain_sha256(
            negative_contract.MUTATED_PAYLOAD_DOMAIN, mutated
        ),
        "mutation": mutation,
        "productionBoundary": stage,
        "requiredProperty": required_property,
        "target": fixture["target"],
        "testPath": CASE_HARNESS,
        "testSha256": _sha(CASE_HARNESS),
    }


def generated(document: dict[str, Any]) -> dict[str, Any]:
    prerequisites = []
    for execution_id, spec in sorted(EXECUTION_SPECS.items()):
        stdout = f"negative test passed: {execution_id}\n".encode("ascii")
        if execution_id in {"evidence-protocol", "stale-output-protocol"}:
            case = execution_id.removesuffix("-protocol")
            stdout = f"negative protocol passed: {case}\n".encode("ascii")
        prerequisites.append(
            {
                "command": _command(spec),
                "expectedObservation": {
                    "exitStatus": 0,
                    "stderrBytes": 0,
                    "stderrSha256": EMPTY_SHA256,
                    "stdoutBytes": len(stdout),
                    "stdoutSha256": hashlib.sha256(stdout).hexdigest(),
                },
                "harnessPath": spec["harnessPath"],
                "harnessSha256": _sha(spec["harnessPath"]),
                "prerequisiteId": execution_id,
            }
        )

    kernels = {kernel["fixtureId"]: kernel for kernel in document["capabilityKernels"]}
    fixtures = []
    for fixture in sorted(document["compilerFixtures"], key=lambda item: item["fixtureId"]):
        fixture_id = fixture["fixtureId"]
        kernel = kernels[fixture_id]
        categories = sorted(
            manifest_contract._required_negative_categories(
                set(kernel["requiredProperties"])
            )
        )
        cases = [
            _case(
                document,
                fixture,
                kernel,
                category,
                CATEGORY_SPECS[category][2],
                CATEGORY_SPECS[category][0],
                CATEGORY_SPECS[category][1],
                CATEGORY_SPECS[category][3],
                category,
            )
            for category in categories
        ]
        family = _advanced_family(fixture_id, kernel["kernelSymbol"])
        if family is not None and "functional-refinement" in kernel["requiredProperties"]:
            for variant, (category, subject, stage, path) in ADVANCED_VARIANTS.items():
                cases.append(
                    _case(
                        document,
                        fixture,
                        kernel,
                        category,
                        f"{family}-{variant}",
                        subject,
                        stage,
                        path,
                        f"{category}--{family}-{variant}",
                    )
                )
        cases.sort(key=lambda case: case["caseId"])
        fixtures.append(
            {
                "bindingSha256": negative_contract.fixture_binding_sha256(
                    fixture, kernel
                ),
                "cases": cases,
                "compilerInputContractSha256": fixture["compilerInput"]["contractSha256"],
                "fixtureId": fixture_id,
                "kernelSymbol": kernel["kernelSymbol"],
                "sourceClosureSha256": fixture["compilerInput"]["sourceClosureSha256"],
                "target": fixture["target"],
            }
        )
    offset = 0
    for fixture in fixtures:
        for case in fixture["cases"]:
            offset += 1
            case["diagnosticCode"] = f"FE2O3-NEG-{offset:04d}"
    return {
        "fixtureContractSha256": negative_contract.fixture_contract_sha256(document),
        "fixtures": fixtures,
        "roadmapIssue": negative_contract.ROADMAP_ISSUE,
        "schema": negative_contract.DECLARATION_SCHEMA,
        "sharedPrerequisites": prerequisites,
    }


def _replace(path: Path, payload: bytes) -> None:
    if path.exists() and (path.is_symlink() or not path.is_file()):
        raise ValueError(f"refusing to replace non-regular file {path}")
    temporary = path.with_name(f".{path.name}.tmp-{os.getpid()}")
    descriptor = os.open(
        temporary,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0),
        0o644,
    )
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(payload)
            output.flush()
            os.fsync(output.fileno())
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)


def main(arguments: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true")
    options = parser.parse_args(arguments)
    try:
        _, document = manifest_contract._load_json_unique(MANIFEST)
        payload = (
            json.dumps(generated(document), allow_nan=False, ensure_ascii=True, indent=2) + "\n"
        ).encode("ascii")
        digest = hashlib.sha256(payload).hexdigest()
        digest_payload = f"{digest}  {negative_contract.DECLARATION_PATH.as_posix()}\n".encode(
            "ascii"
        )
        if options.write:
            OUTPUT.parent.mkdir(parents=True, exist_ok=True)
            _replace(OUTPUT, payload)
            _replace(DIGEST, digest_payload)
        elif not OUTPUT.is_file() or OUTPUT.read_bytes() != payload or DIGEST.read_bytes() != digest_payload:
            raise ValueError("negative declarations are stale; rerun with --write")
    except (OSError, ValueError, json.JSONDecodeError, manifest_contract.ManifestError) as error:
        print(f"tutorial negative declaration update: {error}", file=sys.stderr)
        return 1
    print("tutorial negative declarations are current")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
