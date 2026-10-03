#!/usr/bin/env python3
"""Fail-closed controls for the fill-dispatch proof campaign."""
import copy
import importlib.util
from pathlib import Path
import unittest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("dispatch_check", HERE / "gfx942_fill_dispatch_check.py")
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)


class Controls(unittest.TestCase):
    def setUp(self):
        self.inputs = {str(path): (check.ROOT / path).read_bytes() for path in check.PINS}

    def test_current_sources(self):
        check.validate_sources(self.inputs)

    def test_vacuous_premises_and_alternate_forwarder(self):
        for path, before, after in [
            (check.PROOF, b"requires dispatch_valid(*input),", b"requires false,"),
            (check.RUST, b"gfx942_fill_group_body_v1!", b"other_body!"),
        ]:
            inputs = dict(self.inputs)
            self.assertIn(before, inputs[str(path)])
            inputs[str(path)] = inputs[str(path)].replace(before, after)
            with self.assertRaises(ValueError):
                check.validate_sources(inputs)

    def test_escape_and_alternate_macro_arm(self):
        for addition in [b"assume(false);", b"admit();", b"#[verifier::external_body]", b"#[cfg(verus)]",
                         b"macro_rules! gfx942_fill_group_body_v1 { (exec_expr, $($rest:tt)*) => { panic!() }; }"]:
            inputs = dict(self.inputs)
            inputs[str(check.BODY)] += addition
            with self.assertRaises(ValueError):
                check.validate_sources(inputs)

    def test_mutants_distinct_and_complete(self):
        cases = check.mutations(self.inputs[str(check.BODY)].decode("ascii"))
        self.assertEqual(len(cases), 13)
        self.assertEqual(len({body for _, body, _ in cases}), 13)
        self.assertEqual({name for name, _, _ in cases}, set(check.FAILURES))
        for name, _, target in cases:
            self.assertEqual(target, check.FAILURES[name][0])

    def test_descriptor_storage_cannot_be_replaced_by_explicit_prefix(self):
        inputs = dict(self.inputs)
        before = b"kernarg_bytes: self.kernarg_storage_bytes()"
        self.assertIn(before, inputs[str(check.RUST)])
        inputs[str(check.RUST)] = inputs[str(check.RUST)].replace(before, b"kernarg_bytes: 16")
        with self.assertRaises(ValueError):
            check.validate_sources(inputs)

    def test_positive_exact_count_and_identity(self):
        result = {"verus": check.support.VERIFIER, "verification-results": {
            "encountered-error": False, "encountered-vir-error": False, "errors": 0,
            "verified": 44, "is-verifying-entire-crate": True, "success": True}}
        encode = check.support.json.dumps
        self.assertTrue(check.classify(0, encode(result), "", check.ROOT / check.PROOF))
        for count in [0, 30, 43, 45, True]:
            result["verification-results"]["verified"] = count
            self.assertFalse(check.classify(0, encode(result), "", check.ROOT / check.PROOF))
        result["verification-results"]["verified"] = 44
        result["verus"] = {}
        self.assertFalse(check.classify(0, encode(result), "", check.ROOT / check.PROOF))

    def test_timeout_malformed_and_duplicate_fields_reject(self):
        for status, stdout, stderr in [(124, "", ""), (0, "{}", ""), (1, "{}", "parse error"),
                                       (1, '{"verus":1,"verus":2}', "")]:
            self.assertFalse(check.classify(status, stdout, stderr, check.ROOT / check.PROOF))

    def negative(self):
        proof = check.ROOT / check.PROOF
        _, _, call, macro, body, definition = check.target_info(proof, "valid_dispatch")
        result = {"verus": check.support.VERIFIER, "verification-results": {
            "encountered-error": True, "encountered-vir-error": False, "errors": 1,
            "verified": 0, "is-verifying-entire-crate": False}}
        primary = dict(is_primary=True, file_name=str(proof), line_start=35,
                       column_start=13, line_end=35, column_end=45)
        expansion = dict(is_primary=False, file_name=str(body), expansion={
            "macro_decl_name": macro + "!", "span": {"file_name": str(proof), "line_start": call},
            "def_site_span": {"file_name": str(body), "line_start": definition}})
        rows = [{"level": "error", "message": "postcondition not satisfied", "spans": [primary, expansion]},
                {"level": "error", "message": "aborting due to 1 previous error", "spans": []}]
        return result, rows

    def classified(self, result, rows, target="valid_dispatch", mutant="zero-grid"):
        encode = check.support.json.dumps
        return check.classify(1, encode(result), "\n".join(map(encode, rows)),
                              check.ROOT / check.PROOF, target, mutant)

    def test_exact_negative_and_cross_target_rejection(self):
        result, rows = self.negative()
        self.assertTrue(self.classified(result, rows))
        self.assertFalse(self.classified(result, rows, "initialize_entry", "group-register"))
        self.assertFalse(self.classified(result, rows, "valid_dispatch", "unknown"))

    def test_wrong_kind_site_and_parser_error(self):
        result, rows = self.negative()
        for message in ["assertion failed", "arithmetic overflow", "mismatched types", "expected identifier",
                        "assertion might fail due to timeout"]:
            changed = copy.deepcopy(rows)
            changed[0]["message"] = message
            self.assertFalse(self.classified(result, changed))
        changed = copy.deepcopy(rows)
        changed[0]["spans"][0]["line_start"] += 1
        self.assertFalse(self.classified(result, changed))

    def test_missing_or_wrong_expansion(self):
        result, rows = self.negative()
        changed = copy.deepcopy(rows)
        changed[0]["spans"].pop()
        self.assertFalse(self.classified(result, changed))
        changed = copy.deepcopy(rows)
        changed[0]["spans"][1]["expansion"]["span"]["line_start"] += 1
        self.assertFalse(self.classified(result, changed))

    def test_exact_group_precondition_without_macro_expansion(self):
        result, _ = self.negative()
        proof = check.ROOT / check.PROOF
        start, _, _, _, _, _ = check.target_info(proof, "dispatch_byte_after")
        rows = [
            {"level": "note", "message": "function body check: not all errors may have been reported; rerun with a higher value for --multiple-errors to find other potential errors in this function",
             "spans": [{"file_name": str(proof), "line_start": start}]},
            {"level": "error", "message": "precondition not satisfied", "spans": [
                dict(is_primary=True, file_name=str(proof), line_start=214, column_start=13, line_end=214, column_end=49),
                dict(is_primary=False, file_name=str(proof), line_start=88, column_start=37, line_end=88, column_end=63,
                     label="failed precondition")]},
            {"level": "error", "message": "aborting due to 1 previous error", "spans": []},
        ]
        self.assertTrue(self.classified(result, rows, "dispatch_byte_after", "byte-group"))
        changed = copy.deepcopy(rows)
        changed[1]["spans"][1]["line_start"] += 1
        self.assertFalse(self.classified(result, changed, "dispatch_byte_after", "byte-group"))
        self.assertFalse(self.classified(result, rows[1:], "dispatch_byte_after", "byte-group"))

    def test_contradictory_summary_or_warning(self):
        result, rows = self.negative()
        for key, value in [("success", True), ("verified", 1), ("errors", True), ("encountered-vir-error", True)]:
            changed = copy.deepcopy(result)
            changed["verification-results"][key] = value
            self.assertFalse(self.classified(changed, rows))
        self.assertFalse(self.classified(result, rows + [{"level": "warning", "message": "unexpected"}]))


if __name__ == "__main__":
    unittest.main()
