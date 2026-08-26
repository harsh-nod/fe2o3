#!/usr/bin/env python3
"""Check retained bounded MoE evidence and the Worker V3-only runtime boundary."""

from __future__ import annotations

import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs/bounded-moe-v1.md"
CANONICAL_SOURCE = (
    ROOT / "crates/rustc-codegen-fe2o3/src/moe_top2_source_kir_correspondence.rs"
)
PROOF_TEST = ROOT / "crates/fe2o3-verifier/tests/moe_expert_compact_plan_v1.rs"
NEGATIVE_MANIFEST = (
    ROOT / "crates/fe2o3-verifier/verus/moe_expert_compact_plan_v1/NEGATIVE_SHA256"
)
README = ROOT / "README.md"
TESTING = ROOT / "docs/testing.md"
ROADMAP = ROOT / "docs/implementation-roadmap-v2.md"
EXAMPLE = ROOT / "examples/moe_expert_v1/README.md"
CI_LOCAL = ROOT / "scripts/ci-local.sh"
PRODUCTION_ABSENCE = (
    ROOT
    / "crates/fe2o3-host/tests/ui/production_application_handoff"
    / "exact_moe_route_is_unavailable.rs"
)

RETIRED_PATHS = [
    ROOT / "crates/fe2o3-host/MOE_ROUTING_EXPERT_BRIDGE_V1.md",
    ROOT / "crates/fe2o3-host/src/generated_moe_expert_v1.rs",
    ROOT / "crates/fe2o3-host/src/generated_moe_expert_v2.rs",
    ROOT / "crates/fe2o3-host/src/generated_moe_top2_v1.rs",
    ROOT / "crates/fe2o3-host/src/moe_routing_expert_bridge_v1.rs",
    ROOT / "crates/fe2o3-host/src/moe_routing_expert_bridge_v2.rs",
    ROOT / "crates/fe2o3-host/src/moe_top2_v1_lifecycle.rs",
    ROOT / "crates/fe2o3-hsa-runtime/src/moe_top2_resource_observation.rs",
    ROOT / "crates/fe2o3-hsa-runtime/tests/moe_top2_v1_hardware.rs",
]

RETAINED_COMMANDS = [
    "python3 scripts/test-bounded-moe-docs.py",
    "cargo test --locked -p fe2o3-verifier --test moe_expert_compact_plan_v1",
]


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(f"bounded MoE documentation check failed: {message}")


def local_links(markdown: Path) -> list[Path]:
    text = markdown.read_text(encoding="utf-8")
    links = []
    for target in re.findall(r"\[[^]]+\]\(([^)]+)\)", text):
        if "://" in target or target.startswith("#"):
            continue
        path = target.split("#", 1)[0]
        if path:
            links.append((markdown.parent / path).resolve())
    return links


def function_body(source: str, name: str, next_name: str) -> str:
    start = f"{name}() {{\n"
    require(source.count(start) == 1, f"{name} CI function is absent or duplicated")
    remainder = source.split(start, 1)[1]
    boundary = re.search(r"\n}\n\n([A-Za-z_][A-Za-z0-9_]*)\(\) \{", remainder)
    require(boundary is not None, f"{name} CI function boundary is absent")
    require(
        boundary.group(1) == next_name,
        f"{name} CI function must be immediately followed by {next_name}",
    )
    return remainder[: boundary.start()]


def validate_ci_dispatch(ci_local: str) -> None:
    docs_command = (
        "  run_step bounded-moe-docs \\\n"
        "    python3 scripts/test-bounded-moe-docs.py"
    )
    core = function_body(ci_local, "run_generic_core", "run_generic")
    generic = function_body(ci_local, "run_generic", "run_rocm_compile")
    require(ci_local.count(docs_command) == 1, "CI must own one MoE docs check")
    require(docs_command in core, "generic-core CI must run the MoE docs check")
    require(docs_command not in generic, "generic CI must delegate through generic-core")


def main() -> None:
    doc = DOC.read_text(encoding="utf-8")
    source = CANONICAL_SOURCE.read_text(encoding="utf-8")
    proof_test = PROOF_TEST.read_text(encoding="utf-8")
    owned_docs = {
        "README": README.read_text(encoding="utf-8"),
        "bounded MoE evidence": doc,
        "testing guide": TESTING.read_text(encoding="utf-8"),
        "implementation roadmap": ROADMAP.read_text(encoding="utf-8"),
        "MoE example": EXAMPLE.read_text(encoding="utf-8"),
    }

    validate_ci_dispatch(CI_LOCAL.read_text(encoding="utf-8"))

    source_entries = re.findall(r'canonical\.push\(\s*"([^"]+)"', source)
    doc_entries = re.findall(r"^\|\s*\d+\s*\|\s*`([^`]+)`\s*\|", doc, re.MULTILINE)
    require(len(source_entries) == 31, "source canonical table is no longer 31 entries")
    require(doc_entries == source_entries, "documented canonical table drifted from source")

    require(
        "mutations=7 obligations=19" in proof_test,
        "proof transcript no longer pins 19 obligations and seven mutations",
    )
    require(
        len(NEGATIVE_MANIFEST.read_text(encoding="utf-8").splitlines()) == 7,
        "negative manifest no longer contains seven mutations",
    )
    require("assert_eq!(checked_vectors, 625);" in proof_test, "625-vector test drifted")

    for marker in [
        "## Retired host routes",
        "generic Worker V3",
        "not a second runtime pipeline",
        "No MoE hardware execution",
    ]:
        require(marker in doc, f"missing current MoE boundary text: {marker}")

    for command in RETAINED_COMMANDS:
        for name in ["bounded MoE evidence", "testing guide", "MoE example"]:
            require(command in owned_docs[name], f"missing retained command in {name}: {command}")

    for retired in RETIRED_PATHS:
        require(not retired.exists(), f"retired MoE route returned: {retired.relative_to(ROOT)}")

    stale_commands = [
        "generated_moe_expert_v1_ui",
        "generated_moe_expert_v2_ui",
        "moe_expert_v1_upload_hardware",
        "moe_routing_expert_bridge_v1::tests",
        "moe_routing_expert_bridge_v2::tests",
    ]
    for name, text in owned_docs.items():
        for stale in stale_commands:
            require(stale not in text, f"{name} still advertises retired command {stale}")

    require(PRODUCTION_ABSENCE.exists(), "MoE production-route absence fixture is missing")
    absence = PRODUCTION_ABSENCE.read_text(encoding="utf-8")
    require("join_moe_top2_v1" in absence, "MoE route absence fixture drifted")

    for markdown in [README, DOC, TESTING, ROADMAP, EXAMPLE]:
        for link in local_links(markdown):
            require(link.exists(), f"broken local link in {markdown.relative_to(ROOT)}: {link}")

    print("bounded MoE evidence is consistent with the Worker V3-only runtime")


if __name__ == "__main__":
    main()
