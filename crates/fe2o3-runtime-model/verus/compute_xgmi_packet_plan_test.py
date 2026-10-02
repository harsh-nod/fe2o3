#!/usr/bin/env python3
"""Synthetic classifier and source controls; this test does not invoke Verus."""

import copy
import importlib.util
import json
from pathlib import Path
import unittest


PATH = Path(__file__).with_name("compute_xgmi_packet_plan_check.py")
SPEC = importlib.util.spec_from_file_location("packet_plan_check", PATH)
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


class PacketPlanControls(unittest.TestCase):
    def setUp(self):
        self.proof = CHECK.ROOT / CHECK.PROOF
        self.summary = {"verus": copy.deepcopy(CHECK.VERIFIER), "verification-results": {
            "encountered-error": False, "encountered-vir-error": False, "success": True,
            "errors": 0, "verified": 5, "is-verifying-entire-crate": True,
        }}

    def positive(self, status=0, stderr=""):
        return CHECK.classify(status, json.dumps(self.summary), stderr, self.proof)

    def negative(self):
        focus = "compute_xgmi_packet_count_v1"
        macro = "compute_xgmi_packet_count_body_v1"
        lines = self.proof.read_text().splitlines()
        contract = next(index + 1 for index, line in enumerate(lines) if "None => !admitted(total)," in line)
        call = next(index + 1 for index, line in enumerate(lines) if line.strip().startswith(macro + "!("))
        body = CHECK.ROOT / CHECK.BODY
        definition = next(index + 1 for index, line in enumerate(body.read_text().splitlines())
                          if line.startswith("macro_rules! " + macro + " {"))
        diagnostic = {"level": "error", "message": "postcondition not satisfied", "spans": [
            {"file_name": str(self.proof), "line_start": contract, "is_primary": True},
            {"file_name": str(body), "expansion": {
                "macro_decl_name": macro + "!",
                "span": {"file_name": str(self.proof), "line_start": call},
                "def_site_span": {"file_name": str(body), "line_start": definition},
            }},
        ]}
        self.summary["verification-results"] = {
            "encountered-error": True, "encountered-vir-error": False, "errors": 1,
            "verified": 0, "is-verifying-entire-crate": False,
        }
        abort = {"level": "error", "message": "aborting due to 1 previous error", "spans": []}
        return focus, [diagnostic, abort]

    def test_actual_source_and_eight_mutations_are_closed(self):
        sources = CHECK.source_snapshot()
        mutations = CHECK.mutations(sources[str(CHECK.BODY)].decode("ascii"))
        self.assertEqual(len(mutations), 8)
        self.assertEqual(len(set(mutations.values())), 8)

    def test_exact_positive_rejects_exit_error_count_and_tool_drift(self):
        self.assertTrue(self.positive())
        self.assertFalse(self.positive(status=1))
        for key, value in [("verified", 4), ("errors", 1), ("success", False),
                           ("is-verifying-entire-crate", False), ("encountered-vir-error", True)]:
            original = self.summary["verification-results"][key]
            self.summary["verification-results"][key] = value
            self.assertFalse(self.positive())
            self.summary["verification-results"][key] = original
        self.summary["verus"]["version"] = "other"
        self.assertFalse(self.positive())

    def test_positive_rejects_warning_and_malformed_json(self):
        self.assertFalse(self.positive(stderr=json.dumps({"level": "warning", "message": "unproved"})))
        self.assertFalse(CHECK.classify(0, "{", "", self.proof))
        with self.assertRaises(ValueError):
            CHECK.strict_json('{"x":1,"x":2}')
        with self.assertRaises(ValueError):
            CHECK.strict_json('{"x":NaN}')

    def test_negative_requires_selected_contract_and_actual_macro_expansion(self):
        focus, diagnostics = self.negative()
        encode = lambda: "\n".join(json.dumps(row) for row in diagnostics)
        self.assertTrue(CHECK.classify(1, json.dumps(self.summary), encode(), self.proof, focus))
        original = copy.deepcopy(diagnostics)
        diagnostics[0]["spans"][0]["line_start"] = 1
        self.assertFalse(CHECK.classify(1, json.dumps(self.summary), encode(), self.proof, focus))
        diagnostics[:] = copy.deepcopy(original)
        diagnostics[0]["spans"][1]["expansion"]["macro_decl_name"] = "other!"
        self.assertFalse(CHECK.classify(1, json.dumps(self.summary), encode(), self.proof, focus))
        diagnostics[:] = copy.deepcopy(original)
        diagnostics[0]["spans"][1].pop("expansion")
        self.assertFalse(CHECK.classify(1, json.dumps(self.summary), encode(), self.proof, focus))

    def test_negative_rejects_compiler_timeout_and_unrelated_diagnostics(self):
        focus, diagnostics = self.negative()
        encode = lambda: "\n".join(json.dumps(row) for row in diagnostics)
        self.assertFalse(CHECK.classify(124, json.dumps(self.summary), encode(), self.proof, focus))
        diagnostics[0]["message"] = "arithmetic underflow/overflow"
        self.assertFalse(CHECK.classify(1, json.dumps(self.summary), encode(), self.proof, focus))
        diagnostics[0]["message"] = "postcondition not satisfied"
        diagnostics.append({"level": "note", "message": "unknown unverified item"})
        self.assertFalse(CHECK.classify(1, json.dumps(self.summary), encode(), self.proof, focus))


if __name__ == "__main__":
    unittest.main()
