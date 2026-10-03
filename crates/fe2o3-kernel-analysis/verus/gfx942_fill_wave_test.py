#!/usr/bin/env python3
"""Fail-closed controls for the fill-wave proof campaign."""
import importlib.util
import copy
from pathlib import Path
import unittest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("fill_check", HERE / "gfx942_fill_wave_check.py")
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)


class Controls(unittest.TestCase):
    def setUp(self):
        self.inputs = {str(path): (check.ROOT / path).read_bytes()
                       for path in [check.PROOF, check.BODY, check.RUST]}

    def test_current_sources(self):
        check.validate_sources(self.inputs)

    def test_vacuous_precondition(self):
        key = str(check.PROOF)
        self.inputs[key] = self.inputs[key].replace(b"requires valid(state, kernarg@),", b"requires false,")
        with self.assertRaises(ValueError):
            check.validate_sources(self.inputs)

    def test_alternate_forwarder(self):
        key = str(check.RUST)
        self.inputs[key] = self.inputs[key].replace(b"gfx942_fill_wave_body_v1!", b"other_body!")
        with self.assertRaises(ValueError):
            check.validate_sources(self.inputs)

    def test_trust_escapes(self):
        for escape in [b"assume(false);", b"admit();", b"#[verifier::external_body]", b"#[cfg(verus)]"]:
            with self.subTest(escape=escape):
                inputs = dict(self.inputs)
                inputs[str(check.BODY)] += escape
                with self.assertRaises(ValueError):
                    check.validate_sources(inputs)

    def test_mutants_are_distinct(self):
        cases = check.mutations(self.inputs[str(check.BODY)].decode("ascii"))
        self.assertEqual(len(cases), 16)
        self.assertEqual(len({body for _, body, _ in cases}), 16)

    def test_malformed_and_timeout_rejected(self):
        for status, stdout, stderr in [(124, "", ""), (1, "{}", "parse error"), (0, "{}", ""),
                                       (1, '{"verus":1,"verus":2}', "")]:
            self.assertFalse(check.classify(status, stdout, stderr, check.ROOT / check.PROOF, "execute_wave"))

    def test_positive_count_and_identity(self):
        result = {"verus": check.support.VERIFIER, "verification-results": {
            "encountered-error": False, "encountered-vir-error": False, "errors": 0,
            "verified": 30, "is-verifying-entire-crate": True, "success": True}}
        encode = check.support.json.dumps
        self.assertTrue(check.classify(0, encode(result), "", check.ROOT / check.PROOF))
        for count in [0, 29, 31]:
            result["verification-results"]["verified"] = count
            self.assertFalse(check.classify(0, encode(result), "", check.ROOT / check.PROOF))
        result["verification-results"]["verified"] = 30
        result["verus"] = {}
        self.assertFalse(check.classify(0, encode(result), "", check.ROOT / check.PROOF))

    def test_wrong_theorem_location(self):
        result = {"verus": check.support.VERIFIER, "verification-results": {
            "encountered-error": True, "encountered-vir-error": False, "errors": 1,
            "verified": 0, "is-verifying-entire-crate": False}}
        encode = check.support.json.dumps
        errors = [{"level": "error", "message": "postcondition not satisfied", "spans": [
            {"is_primary": True, "file_name": str(check.ROOT / check.PROOF), "line_start": 1}]},
            {"level": "error", "message": "aborting due to 1 previous error", "spans": []}]
        self.assertFalse(check.classify(1, encode(result), "\n".join(map(encode, errors)),
                                        check.ROOT / check.PROOF, "execute_wave", "load-base-clobber"))

    def negative(self):
        proof = check.ROOT / check.PROOF
        _, _, call, macro, body, definition = check.target_info(proof, "word")
        result = {"verus": check.support.VERIFIER, "verification-results": {
            "encountered-error": True, "encountered-vir-error": False, "errors": 1,
            "verified": 0, "is-verifying-entire-crate": False}}
        primary = dict(is_primary=True, file_name=str(proof), line_start=38,
                       column_start=13, line_end=38, column_end=44)
        expansion = dict(is_primary=False, file_name=str(body), expansion={
            "macro_decl_name": macro + "!", "span": {"file_name": str(proof), "line_start": call},
            "def_site_span": {"file_name": str(body), "line_start": definition}})
        rows = [{"level": "error", "message": "postcondition not satisfied", "spans": [primary, expansion]},
                {"level": "error", "message": "aborting due to 1 previous error", "spans": []}]
        return result, rows

    def classified(self, result, rows, target="word", mutant="kernarg-endian"):
        encode = check.support.json.dumps
        return check.classify(1, encode(result), "\n".join(map(encode, rows)),
                              check.ROOT / check.PROOF, target, mutant)

    def test_accepted_negative(self):
        self.assertTrue(self.classified(*self.negative()))

    def test_wrong_kind_site_and_cross_mutant(self):
        result, rows = self.negative()
        for message in ["assertion failed", "invariant not satisfied before loop", "arithmetic overflow"]:
            changed = copy.deepcopy(rows)
            changed[0]["message"] = message
            self.assertFalse(self.classified(result, changed))
        changed = copy.deepcopy(rows)
        changed[0]["spans"][0]["line_start"] += 1
        self.assertFalse(self.classified(result, changed))
        self.assertFalse(self.classified(result, rows, "decode_kernarg", "kernarg-field"))

    def test_missing_or_wrong_expansion(self):
        result, rows = self.negative()
        changed = copy.deepcopy(rows)
        changed[0]["spans"].pop()
        self.assertFalse(self.classified(result, changed))
        changed = copy.deepcopy(rows)
        changed[0]["spans"][1]["expansion"]["span"]["line_start"] += 1
        self.assertFalse(self.classified(result, changed))

    def test_contradictory_summary(self):
        result, rows = self.negative()
        for key, value in [("success", True), ("verified", 3), ("errors", True), ("encountered-vir-error", True)]:
            changed = copy.deepcopy(result)
            changed["verification-results"][key] = value
            self.assertFalse(self.classified(changed, rows))

    def test_production_only_macro_arm(self):
        key = str(check.BODY)
        self.inputs[key] = self.inputs[key].replace(b"macro_rules! gfx942_fill_wave_body_v1 {",
            b"macro_rules! gfx942_fill_wave_body_v1 {\n (exec_expr, $($rest:tt)*) => { panic!() }; ")
        with self.assertRaises(ValueError):
            check.validate_sources(self.inputs)

    def test_parser_failure_not_logical(self):
        result, rows = self.negative()
        for message in ["expected identifier", "mismatched types", "assertion might fail due to timeout"]:
            changed = copy.deepcopy(rows)
            changed[0]["message"] = message
            self.assertFalse(self.classified(result, changed))


if __name__ == "__main__":
    unittest.main()
