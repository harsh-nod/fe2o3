#!/usr/bin/env python3
"""Synthetic campaign controls; solver negatives are separate."""
import copy
import importlib.util
import json
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("prefix_check", Path(__file__).with_name("checked_u32_prefix_check.py"))
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)


class Controls(unittest.TestCase):
    def setUp(self):
        self.proof = check.ROOT / check.PROOF
        self.data = {"verus": copy.deepcopy(check.base.VERIFIER), "verification-results": {
            "encountered-error": False, "encountered-vir-error": False, "success": True,
            "errors": 0, "verified": 11, "is-verifying-entire-crate": True}}

    def accepted(self, status=0, rows=(), negative=False, invariant=False):
        return check.classify(status, json.dumps(self.data), "\n".join(map(json.dumps, rows)), self.proof, negative, invariant)

    def negative(self, invariant):
        self.data["verification-results"] = {"encountered-error": True, "encountered-vir-error": False,
            "errors": 1, "verified": 1, "is-verifying-entire-crate": False}
        call, contract, loop, body, definition = check.locations(self.proof)
        expanded = {"file_name": str(body), "expansion": {"macro_decl_name": check.MACRO + "!",
            "span": {"file_name": str(self.proof), "line_start": call},
            "def_site_span": {"file_name": str(body), "line_start": definition}}}
        rows = [{"level": "error", "message": "invariant not satisfied at end of loop body" if invariant else "postcondition not satisfied",
            "spans": [{"is_primary": True, "file_name": str(self.proof), "line_start": loop if invariant else contract}]},
            {"level": "error", "message": "aborting due to 1 previous error", "spans": []}]
        if invariant:
            rows.append({"level": "note", "message": "while loop: not all errors may have been reported; rerun with a higher value for --multiple-errors to find other potential errors in this function", "spans": [expanded]})
        else:
            rows[0]["spans"].append(expanded)
        return rows

    def test_source_contract_and_mutants(self):
        sources = check.snapshot()
        self.assertEqual(len(check.mutants(sources[str(check.BODY)].decode("ascii"))), 8)
        for path in (check.PROOF, check.FOLD, check.HELPER):
            changed = dict(sources)
            changed[str(path)] += b"\n// changed\n"
            with self.assertRaises(ValueError):
                check.validate(changed)

    def test_shared_body_escape_rejects(self):
        for escape in (b'include!("other.rs");\n', b'#[verifier::external_body]\n', b'#[cfg(any())]\n'):
            sources = check.snapshot()
            sources[str(check.BODY)] = escape + sources[str(check.BODY)]
            with self.assertRaises(ValueError):
                check.validate(sources)

    def test_exact_positive(self):
        self.assertTrue(self.accepted())
        for key, value in (("verified", 10), ("errors", False), ("success", False), ("is-verifying-entire-crate", False)):
            old = self.data["verification-results"][key]
            self.data["verification-results"][key] = value
            self.assertFalse(self.accepted())
            self.data["verification-results"][key] = old
        self.assertFalse(self.accepted(1))
        self.assertFalse(self.accepted(rows=[{"level": "warning", "message": "unexpected"}]))
        self.data["verus"]["version"] = "wrong"
        self.assertFalse(self.accepted())

    def test_exact_logical_failures(self):
        for invariant in (False, True):
            rows = self.negative(invariant)
            self.assertTrue(self.accepted(1, rows, True, invariant))
            for status in (0, 2, 124, -9):
                self.assertFalse(self.accepted(status, rows, True, invariant))
            self.assertFalse(self.accepted(1, rows, True, not invariant))
            rows[0]["message"] = "arithmetic underflow/overflow"
            self.assertFalse(self.accepted(1, rows, True, invariant))

    def test_exact_spans(self):
        for invariant in (False, True):
            original = self.negative(invariant)
            for field, value in (("file_name", "/tmp/wrong.rs"), ("line_start", 1), ("is_primary", False)):
                rows = copy.deepcopy(original)
                rows[0]["spans"][0][field] = value
                self.assertFalse(self.accepted(1, rows, True, invariant))
            rows = copy.deepcopy(original)
            span = rows[-1]["spans"][0] if invariant else rows[0]["spans"][1]
            span["expansion"]["macro_decl_name"] = "wrong!"
            self.assertFalse(self.accepted(1, rows, True, invariant))

    def test_malformed_or_additional_diagnostics_reject(self):
        self.assertFalse(check.classify(0, "{", "", self.proof))
        with self.assertRaises(ValueError):
            check.strict_json('{"x":0,"x":1}')
        rows = self.negative(False)
        rows.append({"level": "note", "message": "unexpected"})
        self.assertFalse(self.accepted(1, rows, True))


if __name__ == "__main__":
    unittest.main()
