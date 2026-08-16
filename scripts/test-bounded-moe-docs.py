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
        ROOT / "examples/moe_expert_v1/README.md",
        ROOT / "docs/implementation-roadmap-v2.md",
        ROOT / "docs/testing.md",
        DOC,
    ]
    for markdown in owned_markdown:
        for link in local_links(markdown):
            require(link.exists(), f"broken local link in {markdown.relative_to(ROOT)}: {link}")

    print("bounded MoE documentation is consistent with the fixed source profile")


if __name__ == "__main__":
    main()
