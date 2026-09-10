#!/usr/bin/env python3

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import sys
import unittest


sys.dont_write_bytecode = True
SCRIPT = Path(__file__).resolve().parents[1] / "run-tutorial-semantic-qualification.py"
sys.path.insert(0, str(SCRIPT.parent))
SPEC = importlib.util.spec_from_file_location("tutorial_semantic_qualification", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
RUNNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNNER)
ROOT = SCRIPT.parent.parent


def manifest() -> tuple[bytes, dict]:
    raw = (ROOT / "config" / "tutorial-kernel-manifest-v1.json").read_bytes()
    return raw, json.loads(raw)


class TutorialSemanticQualificationTests(unittest.TestCase):
    def test_corpus_identity_matches_shared_baseline(self) -> None:
        _, document = manifest()
        self.assertEqual(
            "b0d99f0c4b7c0431dda5e7441062dc94fea3aecb5c51a89ba1180471caa6e11c",
            RUNNER.corpus_contract_sha256(document),
        )

    def test_evidence_is_explicitly_authority_free(self) -> None:
        raw, document = manifest()
        candidate = {"commit": "1" * 40, "tree": "2" * 40, "worktreeClean": True}
        record = RUNNER.build_evidence(document, raw, candidate, [])
        self.assertEqual(RUNNER.EVIDENCE_SCHEMA, record["schema"])
        self.assertEqual(candidate, record["candidate"])
        self.assertFalse(any(record["authority"].values()))
        self.assertEqual(RUNNER._sha256(raw), record["manifest"]["rawSha256"])

    def test_declared_suite_roster_is_closed_and_resolving(self) -> None:
        _, document = manifest()
        suites = RUNNER.declared_suites(document)
        self.assertEqual(61, len(suites))
        self.assertEqual(
            14, sum(suite["gate"] == "cpu-reference" for suite in suites)
        )
        self.assertEqual(
            47, sum(suite["gate"] == "semantic-simulation" for suite in suites)
        )
        self.assertEqual(
            sorted(suite["suiteId"] for suite in suites),
            [suite["suiteId"] for suite in suites],
        )

    def test_command_identity_is_order_independent_for_object_keys(self) -> None:
        left = {"executable": "scripts/a", "arguments": [], "environment": []}
        right = {"environment": [], "arguments": [], "executable": "scripts/a"}
        self.assertEqual(RUNNER.command_sha256(left), RUNNER.command_sha256(right))


if __name__ == "__main__":
    unittest.main()
