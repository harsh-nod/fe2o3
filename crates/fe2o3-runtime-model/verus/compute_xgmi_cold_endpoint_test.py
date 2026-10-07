#!/usr/bin/env python3
"""Synthetic source/diagnostic controls only; no Verus execution."""

import copy
import importlib.util
import json
from pathlib import Path
import unittest


PATH = Path(__file__).with_name("compute_xgmi_cold_endpoint_check.py")
SPEC = importlib.util.spec_from_file_location("cold_endpoint_check", PATH)
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


class ColdEndpointControls(unittest.TestCase):
    def setUp(self):
        self.proof = CHECK.ROOT / CHECK.PROOF
        self.summary = {"verus": copy.deepcopy(CHECK.VERIFIER), "verification-results": {
            "encountered-error": False, "encountered-vir-error": False, "success": True,
            "errors": 0, "verified": 2, "is-verifying-entire-crate": True,
        }}

    def positive(self, status=0, stderr=""):
        return CHECK.classify(status, json.dumps(self.summary), stderr, self.proof)

    def negative(self):
        lines = self.proof.read_text().splitlines()
        contract = next(i + 1 for i, line in enumerate(lines) if "ensures result == initial_facts(facts)," in line)
        call = next(i + 1 for i, line in enumerate(lines) if line.strip().startswith(CHECK.MACRO + "!("))
        body = CHECK.ROOT / CHECK.BODY
        definition = next(i + 1 for i, line in enumerate(body.read_text().splitlines())
                          if line.startswith("macro_rules! " + CHECK.MACRO + " {"))
        diagnostic = {"level": "error", "message": "postcondition not satisfied", "spans": [
            {"file_name": str(self.proof), "line_start": contract, "is_primary": True},
            {"file_name": str(body), "expansion": {
                "macro_decl_name": CHECK.MACRO + "!",
                "span": {"file_name": str(self.proof), "line_start": call},
                "def_site_span": {"file_name": str(body), "line_start": definition},
            }},
        ]}
        self.summary["verification-results"] = {
            "encountered-error": True, "encountered-vir-error": False, "errors": 1,
            "verified": 0, "is-verifying-entire-crate": False,
        }
        return [diagnostic, {"level": "error", "message": "aborting due to 1 previous error", "spans": []}]

    def test_actual_sources_and_nine_distinct_conjunct_mutations(self):
        sources = CHECK.source_snapshot()
        mutations = CHECK.mutations(sources[str(CHECK.BODY)].decode("ascii"))
        self.assertEqual(len(mutations), 9)
        self.assertEqual(len(set(mutations.values())), 9)

    def test_source_binding_rejects_schema_forwarding_and_observer_changes(self):
        original = CHECK.source_snapshot()
        for path, before, after in [
            (CHECK.PROOF, "detached_data_count: usize", "detached_data_count: u64"),
            (CHECK.RUST, "compute_xgmi_cold_endpoint_body_v1!(facts)", "true"),
            (CHECK.RUST, "established || cold", "cold"),
            (CHECK.RUST, "self.key == self.compute_lane_session", "true"),
            (CHECK.RUST, "dispatch_attached: self.dispatch.is_some()", "dispatch_attached: false"),
            (CHECK.OBSERVER, "self.ring.write() == 0", "self.ring.write() <= 1"),
            (CHECK.PROOF, "ensures result", "#[verifier::external_body]\n    ensures result"),
        ]:
            with self.subTest(path=path, before=before):
                sources = original.copy()
                self.assertIn(before.encode(), sources[str(path)])
                sources[str(path)] = sources[str(path)].replace(before.encode(), after.encode(), 1)
                with self.assertRaises(ValueError):
                    CHECK.validate_sources(sources)

    def test_positive_requires_exact_tool_counts_exit_and_full_crate(self):
        self.assertTrue(self.positive())
        self.assertFalse(self.positive(status=1))
        for key, value in [("verified", 1), ("errors", 1), ("success", False),
                           ("is-verifying-entire-crate", False), ("encountered-vir-error", True)]:
            original = self.summary["verification-results"][key]
            self.summary["verification-results"][key] = value
            self.assertFalse(self.positive())
            self.summary["verification-results"][key] = original
        self.summary["verus"]["version"] = "other"
        self.assertFalse(self.positive())

    def test_positive_rejects_warnings_and_malformed_json(self):
        self.assertFalse(self.positive(stderr=json.dumps({"level": "warning", "message": "unproved"})))
        self.assertFalse(CHECK.classify(0, "{", "", self.proof))
        with self.assertRaises(ValueError):
            CHECK.strict_json('{"x":1,"x":2}')
        with self.assertRaises(ValueError):
            CHECK.strict_json('{"x":NaN}')

    def test_negative_requires_exact_contract_and_macro_expansion(self):
        diagnostics = self.negative()
        encode = lambda: "\n".join(json.dumps(row) for row in diagnostics)
        self.assertTrue(CHECK.classify(1, json.dumps(self.summary), encode(), self.proof, True))
        original = copy.deepcopy(diagnostics)
        diagnostics[0]["spans"][0]["line_start"] = 1
        self.assertFalse(CHECK.classify(1, json.dumps(self.summary), encode(), self.proof, True))
        diagnostics[:] = copy.deepcopy(original)
        diagnostics[0]["spans"][1]["expansion"]["macro_decl_name"] = "other!"
        self.assertFalse(CHECK.classify(1, json.dumps(self.summary), encode(), self.proof, True))
        diagnostics[:] = copy.deepcopy(original)
        diagnostics[0]["spans"][1].pop("expansion")
        self.assertFalse(CHECK.classify(1, json.dumps(self.summary), encode(), self.proof, True))

    def test_negative_rejects_timeout_compiler_and_unrelated_diagnostics(self):
        diagnostics = self.negative()
        encode = lambda: "\n".join(json.dumps(row) for row in diagnostics)
        self.assertFalse(CHECK.classify(124, json.dumps(self.summary), encode(), self.proof, True))
        diagnostics[0]["message"] = "mismatched types"
        self.assertFalse(CHECK.classify(1, json.dumps(self.summary), encode(), self.proof, True))
        diagnostics[0]["message"] = "postcondition not satisfied"
        diagnostics.append({"level": "note", "message": "unknown unverified item"})
        self.assertFalse(CHECK.classify(1, json.dumps(self.summary), encode(), self.proof, True))


if __name__ == "__main__":
    unittest.main()
