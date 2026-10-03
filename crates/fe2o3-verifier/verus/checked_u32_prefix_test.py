#!/usr/bin/env python3
"""Synthetic campaign controls; solver negatives are separate."""
import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("prefix_check", Path(__file__).with_name("checked_u32_prefix_check.py"))
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)


class Controls(unittest.TestCase):
    def setUp(self):
        self.proof = check.ROOT / check.PROOF
        self.data = {"verus": copy.deepcopy(check.base.VERIFIER), "verification-results": {
            "encountered-error": False, "encountered-vir-error": False, "success": True,
            "errors": 0, "verified": check.VERIFIED_COUNT, "is-verifying-entire-crate": True}}

    def accepted(self, status=0, rows=(), target="fold", failure=None):
        return check.classify(status, json.dumps(self.data), "\n".join(map(json.dumps, rows)), self.proof, target, failure)

    def negative(self, target, failure):
        selected = check.TARGETS[target]
        invariant = failure != "post"
        self.data["verification-results"] = {"encountered-error": True, "encountered-vir-error": False,
            "errors": 1, "verified": selected["verified"], "is-verifying-entire-crate": False}
        call, contract, loop, body, definition = check.locations(self.proof, target, failure)
        proof = check.target_proof(self.proof, target)
        expanded = {"file_name": str(body), "expansion": {"macro_decl_name": selected["macro"] + "!",
            "span": {"file_name": str(proof), "line_start": call},
            "def_site_span": {"file_name": str(body), "line_start": definition}}}
        rows = [{"level": "error", "message": "invariant not satisfied at end of loop body" if invariant else "postcondition not satisfied",
            "spans": [{"is_primary": True, "file_name": str(proof), "line_start": loop if invariant else contract}]},
            {"level": "error", "message": "aborting due to 1 previous error", "spans": []}]
        if invariant:
            rows.append({"level": "note", "message": "while loop: not all errors may have been reported; rerun with a higher value for --multiple-errors to find other potential errors in this function", "spans": [expanded]})
        else:
            rows[0]["spans"].append(expanded)
        return rows

    def test_source_contract_and_mutants(self):
        sources = check.snapshot()
        self.assertEqual(len(check.mutants(sources[str(check.BODY)].decode("ascii"))), 8)
        for path in (check.PROOF, check.FOLD, check.HELPER, check.BASIS, check.ADAPTER, check.ASSEMBLE):
            changed = dict(sources)
            changed[str(path)] += b"\n// changed\n"
            with self.assertRaises(ValueError):
                check.validate(changed)

    def test_shared_body_escape_rejects(self):
        for escape in (b'include!("other.rs");\n', b'#[verifier::external_body]\n', b'#[cfg(any())]\n'):
            for body in (check.BODY, check.BASIS_BODY, check.ASSEMBLE_BODY):
                sources = check.snapshot()
                sources[str(body)] = escape + sources[str(body)]
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
        for target, failure in (("fold", "post"), ("fold", "loop"), ("basis", "post"), ("basis", "loop"), ("basis", "clear")):
            rows = self.negative(target, failure)
            self.assertTrue(self.accepted(1, rows, target, failure))
            for status in (0, 2, 124, -9):
                self.assertFalse(self.accepted(status, rows, target, failure))
            self.assertFalse(self.accepted(1, rows, target, "loop" if failure == "post" else "post"))
            for message in ("arithmetic underflow/overflow", "precondition not satisfied", "assertion failed", "mismatched types"):
                rows[0]["message"] = message
                self.assertFalse(self.accepted(1, rows, target, failure))

    def test_exact_spans(self):
        for target, failure in (("fold", "post"), ("fold", "loop"), ("basis", "post"), ("basis", "loop"), ("basis", "clear")):
            original = self.negative(target, failure)
            for field, value in (("file_name", "/tmp/wrong.rs"), ("line_start", 1), ("is_primary", False)):
                rows = copy.deepcopy(original)
                rows[0]["spans"][0][field] = value
                self.assertFalse(self.accepted(1, rows, target, failure))
            rows = copy.deepcopy(original)
            span = rows[-1]["spans"][0] if failure != "post" else rows[0]["spans"][1]
            span["expansion"]["macro_decl_name"] = "wrong!"
            self.assertFalse(self.accepted(1, rows, target, failure))

    def test_malformed_or_additional_diagnostics_reject(self):
        self.assertFalse(check.classify(0, "{", "", self.proof))
        with self.assertRaises(ValueError):
            check.strict_json('{"x":0,"x":1}')
        rows = self.negative("fold", "post")
        rows.append({"level": "note", "message": "unexpected"})
        self.assertFalse(self.accepted(1, rows, "fold", "post"))

    def test_basis_mutants_and_actual_forwarding(self):
        sources = check.snapshot()
        self.assertEqual(len(check.basis_mutants(sources[str(check.BASIS_BODY)].decode("ascii"))), 8)
        for path, before, after in (
            (check.ADAPTER, b"semantic_local: u32,", b"semantic_local: u64,"),
            (check.PROOF, b"semantic_local: u32,", b"semantic_local: u64,"),
            (check.KIR_SCHEMA, b"pub struct ValueId(pub u32);", b"pub struct ValueId(pub u64);"),
            (check.ADAPTER, b"basis::initialize_argument_basis(&arguments, &mut source_state, &mut kernel_state)", b"true"),
            (check.ADAPTER, b"kernel::kernel_prefix(&arguments, operations, capture)", b"kernel::kernel_prefix(&[], operations, capture)"),
            (check.BASIS, b"ordinary_exec,\n        arguments,\n        source,\n        kernel,",
             b"ordinary_exec,\n        arguments,\n        kernel,\n        source,"),
        ):
            self.assertEqual(sources[str(path)].count(before), 1, str(path))
            changed = dict(sources)
            changed[str(path)] = changed[str(path)].replace(before, after)
            with self.assertRaises(ValueError):
                check.validate(changed)

    def test_cross_target_diagnostics_reject(self):
        for target, other in (("fold", "basis"), ("basis", "fold")):
            for failure in ("post", "loop"):
                rows = self.negative(target, failure)
                self.data["verification-results"]["verified"] = check.TARGETS[other]["verified"]
                self.assertFalse(self.accepted(1, rows, other, failure))
                self.data["verification-results"]["verified"] = check.TARGETS[target]["verified"]
                other_rows = self.negative(other, failure)
                self.data["verification-results"]["verified"] = check.TARGETS[target]["verified"]
                if failure == "post":
                    rows[0]["spans"][1] = other_rows[0]["spans"][1]
                else:
                    rows[-1]["spans"][0] = other_rows[-1]["spans"][0]
                self.assertFalse(self.accepted(1, rows, target, failure))

    def test_normalization_mutants_and_closed_body(self):
        sources = check.snapshot()
        self.assertEqual(len(check.normalization_mutants(sources[str(check.NORMALIZE_BODY)].decode("ascii"))), 17)
        for escape in (b'include!("other.rs");\n', b'#[cfg(any())]\n', b'#[verifier::external_body]\n'):
            changed = dict(sources)
            changed[str(check.NORMALIZE_BODY)] = escape + sources[str(check.NORMALIZE_BODY)]
            with self.assertRaises(ValueError):
                check.validate(changed)

    def test_kernel_schema_forwarding_and_closed_body(self):
        sources = check.snapshot()
        self.assertEqual(len(check.kernel_mutants(sources[str(check.KERNEL_BODY)].decode("ascii"))), 20)
        for path, before, after in (
            (check.KIR_SCHEMA, b"pub enum OperationKind {", b"pub enum OperationKind { Extra,"),
            (check.KIR_SCHEMA, b"pub results: Vec<ValueDef>", b"pub results: Proxy<ValueDef>"),
            (check.KIR_SCHEMA, b"U32(u32)", b"U32(u64)"),
            (check.KIR_SCHEMA, b"pub struct Operation {", b"#[cfg(any())] pub struct Operation {"),
            (check.kernel_schema.TYPES, b"pub enum ScalarType {", b"pub enum ScalarType { Extra,"),
            (check.kernel_schema.TYPES, b"Scalar(ScalarType)", b"Scalar(Proxy)"),
            (check.schema.CAPTURE, b"self.capture.operand", b"self.capture.value"),
            (check.KERNEL, b"let operand = capture.operand().0;", b"let operand = capture.value().0;"),
        ):
            self.assertEqual(sources[str(path)].count(before), 1, str(path))
            changed = dict(sources)
            changed[str(path)] = changed[str(path)].replace(before, after)
            with self.assertRaises(ValueError):
                check.validate(changed)
        for escape in (b'include!("other.rs");\n', b'#[cfg(any())]\n', b'#[verifier::external_body]\n'):
            changed = dict(sources)
            changed[str(check.KERNEL_BODY)] = escape + changed[str(check.KERNEL_BODY)]
            with self.assertRaises(ValueError):
                check.validate(changed)

    def test_kernel_diagnostics(self):
        for target, failure in (("constant_binding", "post"), ("terminal_origin", "post"),
                                ("assemble_kernel", "post"), ("assemble_kernel", "argument"), ("assemble_kernel", "loop")):
            rows = self.negative(target, failure)
            self.assertTrue(self.accepted(1, rows, target, failure))
            rows[0]["spans"][0]["file_name"] = str(self.proof)
            self.assertFalse(self.accepted(1, rows, target, failure))

    def test_normalization_ast_schema_getters_and_forwarding(self):
        sources = check.snapshot()
        for path, before, after in (
            (check.schema.MODEL, b"Integer { signed: bool, bits: u16 }", b"Integer { signed: bool, bits: u32 }"),
            (check.schema.MODEL, b"pub enum SemanticOperandV1 {", b"pub enum SemanticOperandV1 { Extra,"),
            (check.schema.MODEL, b"projections: Box<[SemanticProjectionV1]>", b"projections: Vec<SemanticProjectionV1>"),
            (check.schema.MODEL, b"pub const fn index(self) -> u32 {\n                self.0", b"pub const fn index(self) -> u32 {\n                0"),
            (check.schema.MODEL, b"pub const fn bits(self) -> u128 {\n        self.bits", b"pub const fn bits(self) -> u128 {\n        0"),
            (check.schema.MODEL, b"pub const fn shape(&self)", b"#[transform]\n    pub const fn shape(&self)"),
            (check.schema.MODEL, b"pub fn projections(&self) -> &[SemanticProjectionV1] {\n        &self.projections", b"pub fn projections(&self) -> &[SemanticProjectionV1] {\n        &[]"),
            (check.schema.CORRESPONDENCE, b"pub const fn operation_count(self) -> u32 {\n        self.operation_count", b"pub const fn operation_count(self) -> u32 {\n        0"),
            (check.schema.CORRESPONDENCE, b"pub const fn operation_count(self) -> u32", b"#[transform]\n    pub const fn operation_count(self) -> u32"),
            (check.ASSEMBLE, b"let operation = capture.operation();", b"let operation = 0;"),
            (check.NORMALIZE, b"ordinary_exec, types, locals, statement, operations", b"ordinary_exec, types, locals, statement, 0"),
        ):
            expected_sites = 3 if path == check.schema.CORRESPONDENCE else 1
            self.assertEqual(sources[str(path)].count(before), expected_sites, str(path))
            changed = dict(sources)
            changed[str(path)] = changed[str(path)].replace(before, after, 1)
            with self.assertRaises(ValueError):
                check.validate(changed)

        for declaration in (b"pub struct SemanticKirStatementOperationSpanV1 {", b"impl SemanticKirStatementOperationSpanV1 {"):
            original = sources[str(check.schema.CORRESPONDENCE)]
            for prefix in (b"#[cfg(any())]\n", b"mod replacement {\n"):
                changed = dict(sources)
                changed[str(check.schema.CORRESPONDENCE)] = original.replace(declaration, prefix + declaration, 1)
                with self.assertRaises(ValueError):
                    check.validate(changed)
        original = sources[str(check.schema.MODEL)]
        declaration = b"""#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticOperandV1 {
    Copy(SemanticPlaceV1),
    Move(SemanticPlaceV1),
    Constant(SemanticConstantV1),
}"""
        self.assertEqual(original.count(declaration), 1)
        changed = dict(sources)
        changed[str(check.schema.MODEL)] = original.replace(declaration, b"mod hidden {\n" + declaration + b"\n}")
        with self.assertRaises(ValueError):
            check.validate(changed)
        for opening, closing in ((b"discard!(;\n", b"\n);"), (b"discard![;\n", b"\n];")):
            changed = dict(sources)
            changed[str(check.schema.MODEL)] = original.replace(declaration, opening + declaration + closing)
            with self.assertRaises(ValueError):
                check.validate(changed)

    def test_exact_normalization_failures_and_wrong_targets(self):
        for target in ("is_u32", "scalar_local", "scalar_constant", "source_step"):
            rows = self.negative(target, "post")
            self.assertTrue(self.accepted(1, rows, target, "post"))
            for other in ("is_u32", "scalar_local", "scalar_constant", "source_step"):
                if other != target:
                    self.assertFalse(self.accepted(1, rows, other, "post"))
            for status in (0, 2, 124, -9):
                self.assertFalse(self.accepted(status, rows, target, "post"))
            for message in ("precondition not satisfied", "assertion failed", "mismatched types"):
                changed = copy.deepcopy(rows)
                changed[0]["message"] = message
                self.assertFalse(self.accepted(1, changed, target, "post"))

        rows = self.negative("source_step", "post")
        call = check.locations(self.proof, "source_step")[0]
        span = {"is_primary": False, "label": "at the end of the function body",
                "expansion": None, "file_name": str(self.proof),
                "line_start": call - 1, "line_end": call + 1}
        rows[0]["spans"][1] = span
        self.assertTrue(self.accepted(1, rows, "source_step", "post"))
        missing = copy.deepcopy(rows)
        del missing[0]["spans"][1]["expansion"]
        self.assertFalse(self.accepted(1, missing, "source_step", "post"))
        for field, value in (("is_primary", True), ("label", "elsewhere"),
                             ("expansion", {}), ("file_name", "/tmp/wrong.rs"),
                             ("line_start", call), ("line_end", call)):
            changed = copy.deepcopy(rows)
            changed[0]["spans"][1][field] = value
            self.assertFalse(self.accepted(1, changed, "source_step", "post"))
        for other in ("is_u32", "scalar_local", "scalar_constant", "fold", "basis"):
            changed = self.negative(other, "post")
            changed[0]["spans"][1] = span
            self.assertFalse(self.accepted(1, changed, other, "post"))
        with tempfile.TemporaryDirectory(prefix="fe2o3-prefix-control-") as temporary:
            proof = Path(temporary) / "proof.rs"
            changed_span = dict(span, file_name=str(proof))
            proof.write_bytes(self.proof.read_bytes().replace(
                b"!(verus_exec_expr, types, locals, statement, operations)",
                b"!(verus_exec_expr, types, locals, statement, 0)"))
            self.assertFalse(check.source_step_exit(changed_span, proof, call, "source_step", "post"))

    def test_assembly_bindings_mutants_and_diagnostics(self):
        sources = check.snapshot()
        self.assertEqual(len(check.assembly_mutants(sources[str(check.ASSEMBLE_BODY)].decode("ascii"))), 15)
        for failure in ("post", "select", "loop"):
            rows = self.negative("assemble", failure)
            self.assertTrue(self.accepted(1, rows, "assemble", failure))
            for status in (0, 2, 124, -9):
                self.assertFalse(self.accepted(status, rows, "assemble", failure))
            for other in ("post", "select", "loop"):
                if other != failure:
                    self.assertFalse(self.accepted(1, rows, "assemble", other))
            changed = copy.deepcopy(rows)
            changed[0]["spans"][0]["line_start"] = 1
            self.assertFalse(self.accepted(1, changed, "assemble", failure))
        for path, before, after in (
            (check.schema.CORRESPONDENCE, b"pub const fn statement_ordinal(self) -> u32 {\n        self.statement_ordinal", b"pub const fn statement_ordinal(self) -> u32 {\n        0"),
            (check.schema.CORRESPONDENCE, b"&self.statement_operation_spans", b"&[]"),
            (check.schema.CAPTURE, b"pub const fn root(self)", b"#[transform]\n    pub const fn root(self)"),
            (check.schema.CAPTURE, b"self.capture.operation", b"0"),
            (check.schema.MODEL, b"statements: Box<[SemanticStatementV1]>", b"statements: Proxy"),
            (check.schema.CORRESPONDENCE, b"statement_operation_spans: Box<[SemanticKirStatementOperationSpanV1]>", b"statement_operation_spans: Proxy"),
            (check.schema.CAPTURE, b"source: Source,", b"source: Proxy,"),
            (check.KIR_SCHEMA, b"pub struct BlockId(pub u32);", b"pub struct BlockId(pub u64);"),
            (check.ASSEMBLE, b"let function_index = capture.request().function().index();", b"let function_index = capture.request().root().index();"),
        ):
            self.assertGreater(sources[str(path)].count(before), 0, str(path))
            changed = dict(sources)
            changed[str(path)] = changed[str(path)].replace(before, after, 1)
            with self.assertRaises(ValueError):
                check.validate(changed)


if __name__ == "__main__":
    unittest.main()
