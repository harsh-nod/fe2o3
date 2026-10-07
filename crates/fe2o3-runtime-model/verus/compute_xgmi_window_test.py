#!/usr/bin/env python3
"""Synthetic source/diagnostic controls only; no Verus execution."""

import copy
import importlib.util
import json
from pathlib import Path
import unittest


PATH = Path(__file__).with_name("compute_xgmi_window_check.py")
SPEC = importlib.util.spec_from_file_location("window_check", PATH)
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


class WindowControls(unittest.TestCase):
    def setUp(self):
        self.proof = CHECK.ROOT / CHECK.PROOF
        self.summary = {"verus": copy.deepcopy(CHECK.VERIFIER), "verification-results": {
            "encountered-error": False, "encountered-vir-error": False, "success": True,
            "errors": 0, "verified": 3, "is-verifying-entire-crate": True,
        }}

    def positive(self, status=0, stderr=""):
        return CHECK.classify(status, json.dumps(self.summary), stderr, self.proof)

    def negative(self, family="bounds"):
        focus, macro = CHECK.FOCI[family]
        lines = self.proof.read_text().splitlines()
        first = next(i + 1 for i, line in enumerate(lines) if line.startswith("fn " + focus + "("))
        contract = next(i + 1 for i, line in enumerate(lines) if i + 1 > first and "ensures" in line)
        call = next(i + 1 for i, line in enumerate(lines) if line.strip().startswith(macro + "!("))
        body = CHECK.ROOT / CHECK.BODY
        definition = next(i + 1 for i, line in enumerate(body.read_text().splitlines())
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
        return [diagnostic, {"level": "error", "message": "aborting due to 1 previous error", "spans": []}]

    def test_actual_sources_and_eleven_distinct_logical_mutations(self):
        sources = CHECK.source_snapshot()
        mutations = CHECK.mutations(sources[str(CHECK.BODY)].decode("ascii"))
        self.assertEqual(len(mutations), 11)
        self.assertEqual(len(set(mutations.values())), 11)
        self.assertEqual(sum(family == "bounds" for _, family in mutations.values()), 5)
        self.assertEqual(sum(family == "packet" for _, family in mutations.values()), 6)

    def test_source_binding_rejects_schema_forwarding_and_trust_changes(self):
        original = CHECK.source_snapshot()
        for path, before, after in [
            (CHECK.RUST, "source_logical_bytes: u64,", "source_logical_bytes: u32,"),
            (CHECK.RUST, "Gfx942ComputeXgmiPacketPlanV1::new(bytes)?", "Gfx942ComputeXgmiPacketPlanV1::new(1)?"),
            (CHECK.RUST, "let packet = self.plan.packet(index)?;", "let packet = self.plan.packet(0)?;"),
            (CHECK.RUST, "self.plan.total_bytes()", "1"),
            (CHECK.RUST, "self.source_offset,", "self.destination_offset,"),
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
        for key, value in [("verified", 2), ("errors", 1), ("success", False),
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
        for family in CHECK.FOCI:
            diagnostics = self.negative(family)
            encode = lambda: "\n".join(json.dumps(row) for row in diagnostics)
            self.assertTrue(CHECK.classify(1, json.dumps(self.summary), encode(), self.proof, family))
            original = copy.deepcopy(diagnostics)
            diagnostics[0]["spans"][0]["line_start"] = 1
            self.assertFalse(CHECK.classify(1, json.dumps(self.summary), encode(), self.proof, family))
            diagnostics[:] = copy.deepcopy(original)
            diagnostics[0]["spans"][1]["expansion"]["macro_decl_name"] = "other!"
            self.assertFalse(CHECK.classify(1, json.dumps(self.summary), encode(), self.proof, family))
            diagnostics[:] = copy.deepcopy(original)
            diagnostics[0]["spans"][1].pop("expansion")
            self.assertFalse(CHECK.classify(1, json.dumps(self.summary), encode(), self.proof, family))

    def test_negative_rejects_timeout_compiler_and_unrelated_diagnostics(self):
        diagnostics = self.negative()
        encode = lambda: "\n".join(json.dumps(row) for row in diagnostics)
        self.assertFalse(CHECK.classify(124, json.dumps(self.summary), encode(), self.proof, "bounds"))
        diagnostics[0]["message"] = "mismatched types"
        self.assertFalse(CHECK.classify(1, json.dumps(self.summary), encode(), self.proof, "bounds"))
        diagnostics[0]["message"] = "postcondition not satisfied"
        diagnostics.append({"level": "note", "message": "unknown unverified item"})
        self.assertFalse(CHECK.classify(1, json.dumps(self.summary), encode(), self.proof, "bounds"))

    def test_negative_cannot_substitute_a_different_contract(self):
        for family, other in [("bounds", "packet"), ("packet", "bounds")]:
            diagnostics = self.negative(family)
            encoded = "\n".join(json.dumps(row) for row in diagnostics)
            self.assertFalse(CHECK.classify(1, json.dumps(self.summary), encoded, self.proof, other))
        original = (CHECK.ROOT / CHECK.BODY).read_text()
        with self.assertRaises(ValueError):
            CHECK.mutations(original + "\n// bytes > 0\n")


if __name__ == "__main__":
    unittest.main()
