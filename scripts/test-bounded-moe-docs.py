#!/usr/bin/env python3
"""Check bounded MoE documentation against its source-side fixed profile."""

from __future__ import annotations

import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs/bounded-moe-v1.md"
CANONICAL_SOURCE = (
    ROOT
    / "crates/rustc-codegen-fe2o3/src/moe_top2_source_kir_correspondence.rs"
)
PROOF_TEST = ROOT / "crates/fe2o3-verifier/tests/moe_expert_compact_plan_v1.rs"
NEGATIVE_MANIFEST = (
    ROOT
    / "crates/fe2o3-verifier/verus/moe_expert_compact_plan_v1/NEGATIVE_SHA256"
)
README = ROOT / "README.md"
TESTING = ROOT / "docs/testing.md"
ROADMAP = ROOT / "docs/implementation-roadmap-v2.md"
EVIDENCE_RECORD = ROOT / "docs/evidence-record-v1.md"
EXAMPLE = ROOT / "examples/moe_expert_v1/README.md"
ROW_EXAMPLE = ROOT / "examples/row_softmax_v1/README.md"
LLVM_LINK_WORKER_README = ROOT / "tools/fe2o3-llvm-link-worker/README.md"
V2_BRIDGE = ROOT / "crates/fe2o3-host/src/moe_routing_expert_bridge_v2.rs"
V2_ADAPTER = ROOT / "crates/fe2o3-host/src/generated_moe_expert_v2.rs"
V2_UI_COMMON = ROOT / "crates/fe2o3-host/tests/ui/generated_moe_expert_v2"
V2_UI_HARDWARE_HOOKS = (
    ROOT / "crates/fe2o3-host/tests/ui/generated_moe_expert_v2_hardware_hooks"
)
CI_LOCAL = ROOT / "scripts/ci-local.sh"

V2_CHECKPOINT = "10e5f90ece1937aaee77492e8e4e4742863d013b"
V2_UNIT_UI_COMMANDS = [
    "cargo test --locked -p fe2o3-host --lib moe_routing_expert_bridge_v2::tests",
    "cargo test --locked -p fe2o3-host --lib generated_moe_expert_v2::tests",
]
V2_UI_COMMAND_LINES = [
    "cargo test --locked -p fe2o3-host --features hardware-test-hooks \\",
    "--test generated_moe_expert_v2_ui",
]
V2_UI_COMMON_FIXTURES = {
    "batch_identity_cannot_clone.rs",
    "batch_identity_fields_are_private.rs",
    "checked_inputs_cannot_clone.rs",
    "checked_inputs_cannot_construct.rs",
    "checked_inputs_fields_are_private.rs",
    "checked_readback_cannot_clone.rs",
    "checked_readback_cannot_construct.rs",
    "checked_readback_fields_are_private.rs",
    "completed_bridge_cannot_clone.rs",
    "completed_bridge_has_no_authority.rs",
    "completed_v1_api_is_absent.rs",
    "provenance_cannot_clone.rs",
    "provenance_fields_are_private.rs",
    "provenance_use_after_move.rs",
    "raw_weight_view_cannot_enter_v2.rs",
    "synthetic_cannot_convert_to_provenance.rs",
    "synthetic_cannot_enter_check.rs",
    "test_issuer_is_not_public.rs",
    "v1_bridge_cannot_enter_v2.rs",
    "v1_test_issuer_is_not_public.rs",
    "weight_binding_fields_are_private.rs",
}
V2_UI_HARDWARE_HOOK_FIXTURES = {
    "hardware_namespace_test_issuer_is_not_public.rs",
}


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


def main() -> None:
    doc = DOC.read_text(encoding="utf-8")
    source = CANONICAL_SOURCE.read_text(encoding="utf-8")
    proof_test = PROOF_TEST.read_text(encoding="utf-8")
    readme = README.read_text(encoding="utf-8")
    testing = TESTING.read_text(encoding="utf-8")
    roadmap = ROADMAP.read_text(encoding="utf-8")
    evidence_record = EVIDENCE_RECORD.read_text(encoding="utf-8")
    example = EXAMPLE.read_text(encoding="utf-8")
    row_example = ROW_EXAMPLE.read_text(encoding="utf-8")
    llvm_link_worker_readme = LLVM_LINK_WORKER_README.read_text(encoding="utf-8")
    v2_bridge = V2_BRIDGE.read_text(encoding="utf-8")
    v2_adapter = V2_ADAPTER.read_text(encoding="utf-8")
    ci_local = CI_LOCAL.read_text(encoding="utf-8")

    generic_match = re.search(r"run_generic\(\) \{\n(?P<body>.*?)\n\}", ci_local, re.DOTALL)
    require(generic_match is not None, "generic CI function is absent")
    generic_docs_command = (
        "  run_step bounded-moe-docs \\\n"
        "    python3 scripts/test-bounded-moe-docs.py"
    )
    require(
        generic_match.group("body").count(generic_docs_command) == 1,
        "generic CI must run the exact bounded MoE documentation command once",
    )

    source_entries = re.findall(r'canonical\.push\(\s*"([^"]+)"', source)
    doc_entries = re.findall(r"^\|\s*\d+\s*\|\s*`([^`]+)`\s*\|", doc, re.MULTILINE)
    require(len(source_entries) == 31, "source canonical table is no longer 31 entries")
    require(doc_entries == source_entries, "documented canonical table drifted from source")

    require(
        'mutations=7 obligations=19' in proof_test,
        "proof transcript no longer pins 19 obligations and seven mutations",
    )
    require(
        len(NEGATIVE_MANIFEST.read_text(encoding="utf-8").splitlines()) == 7,
        "negative manifest no longer contains seven mutations",
    )
    require("assert_eq!(checked_vectors, 625);" in proof_test, "625-vector test drifted")
    for marker in [
        "`19` verified Verus obligations",
        "seven named negative mutations",
        "`5^4 = 625` valid expert-count vectors",
        "not MIR-to-KIR semantic refinement",
        "no authenticated router-completion provenance",
        "not compiler-derived",
        "deny_moe_expert_execution_v1",
    ]:
        require(marker in doc, f"missing required boundary text: {marker}")

    for marker in [
        "pub struct MoeRoutingExpertBatchIdentityV2",
        "pub struct MoeRoutingCompletionReadbackProvenanceV2",
        "pub struct CheckedMoeCompletedRoutingReadbackV2",
        "pub struct CheckedMoeCompletedRoutingExpertInputsV2",
        "pub struct MoeExpertWeightArtifactBindingV2",
        "pub struct MoeCompletedRoutingExpertBridgeV2",
        "completion_readback_order_identity",
        "This function is publicly callable but constructively unreachable",
        "There is no public constructor, no feature-gated issuer",
    ]:
        require(marker in v2_bridge, f"V2 bridge contract drifted: {marker}")
    for marker in [
        "pub struct GeneratedMoeExpertV2HostAdapterV2",
        "pub const fn has_production_issuer",
        "pub const fn grants_artifact_authority",
        "pub const fn grants_copy_authority",
        "pub const fn grants_load_authority",
        "pub const fn grants_dispatch_authority",
        "MoeExpertV2BufferRoleV2::ALL",
    ]:
        require(marker in v2_adapter, f"V2 adapter contract drifted: {marker}")
    for method in [
        "has_production_issuer",
        "grants_artifact_authority",
        "grants_copy_authority",
        "grants_load_authority",
        "grants_dispatch_authority",
    ]:
        pattern = rf"pub const fn {method}\([^)]*\)[^{{]*\{{\s*false\s*\}}"
        require(
            re.search(pattern, v2_adapter, re.DOTALL) is not None,
            f"V2 adapter no-authority result drifted: {method}",
        )

    actual_ui_fixtures = {path.name for path in V2_UI_COMMON.glob("*.rs")}
    require(
        actual_ui_fixtures == V2_UI_COMMON_FIXTURES,
        "common V2 compile-fail fixture inventory drifted",
    )
    actual_hardware_hook_fixtures = {
        path.name for path in V2_UI_HARDWARE_HOOKS.glob("*.rs")
    }
    require(
        actual_hardware_hook_fixtures == V2_UI_HARDWARE_HOOK_FIXTURES,
        "hardware-hook V2 compile-fail fixture inventory drifted",
    )

    for command in V2_UNIT_UI_COMMANDS:
        for name, text in [
            ("bounded MoE evidence", doc),
            ("testing guide", testing),
            ("MoE example", example),
        ]:
            require(command in text, f"missing V2 command in {name}: {command}")
    for command_line in V2_UI_COMMAND_LINES:
        for name, text in [
            ("bounded MoE evidence", doc),
            ("testing guide", testing),
            ("MoE example", example),
        ]:
            require(
                command_line in text,
                f"missing feature-complete V2 UI command in {name}: {command_line}",
            )

    for name, text, marker in [
        ("README", readme, "V1 evidence only"),
        ("bounded MoE evidence", doc, "there is no V2 GPU observation"),
        ("testing guide", testing, "V1-only upload/readback observation"),
        ("implementation roadmap", roadmap, "V1 evidence only"),
        ("MoE example", example, "not V2 evidence"),
    ]:
        require(
            re.search(r"constructively\s+unreachable", text) is not None,
            f"{name} omits V2 reachability boundary",
        )
        require(marker in text, f"{name} merges V1 hardware evidence into V2")
        require(
            re.search(
                r"no\s+artifact,\s+copy,\s+load,\s+or\s+dispatch\s+authority",
                text,
            )
            is not None,
            f"{name} omits V2 no-authority boundary",
        )

    require(V2_CHECKPOINT in doc, "bounded MoE evidence has stale V2 checkpoint")
    require(V2_CHECKPOINT in roadmap, "implementation roadmap has stale V2 checkpoint")
    require(
        "not the current integration checkpoint" in roadmap,
        "roadmap conflates the bounded MoE V2 and current integration checkpoints",
    )

    for name, text in [
        ("README", readme),
        ("testing guide", testing),
        ("evidence record", evidence_record),
        ("implementation roadmap", roadmap),
        ("row-softmax example", row_example),
        ("LLVM link worker README", llvm_link_worker_readme),
    ]:
        require(
            "subsequent manifest-only Commit B" in text,
            f"{name} does not preserve the Commit A/B release boundary",
        )

    require(
        "`0 Complete / 82 Partial / 0 Missing / 12 N/A` normative rows" in readme,
        "README normative parity totals changed",
    )
    require(
        "`0 Complete / 15 Partial / 0 Missing` supplemental rows" in readme,
        "README supplemental parity totals changed",
    )

    owned_markdown = [
        README,
        EXAMPLE,
        ROADMAP,
        TESTING,
        DOC,
    ]
    for markdown in owned_markdown:
        for link in local_links(markdown):
            require(link.exists(), f"broken local link in {markdown.relative_to(ROOT)}: {link}")

    print("bounded MoE documentation is consistent with the fixed source profile")


if __name__ == "__main__":
    main()
