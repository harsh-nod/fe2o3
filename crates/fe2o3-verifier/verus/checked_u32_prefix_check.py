#!/usr/bin/env python3
"""Qualify shared typed source normalization and basis/folds, not application authority."""

import argparse
import hashlib
import importlib.util
import os
from pathlib import Path
import resource
import signal
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
HELPER = Path("crates/fe2o3-kernel-analysis/verus/gfx942_add_u32_check.py")
HELPER_SHA = "a0f1ebfcbb1467c323658450e75811de861efb1d317a716f7b43014fb9207950"
PROOF = Path("crates/fe2o3-verifier/verus/checked_u32_prefix_v1.rs")
FOLD = Path("crates/fe2o3-verifier/src/gfx942_local_checked_u32_add_v1/source_prefix/fold.rs")
BODY = FOLD.with_name("fold_body.rs")
BASIS = FOLD.with_name("basis.rs")
BASIS_BODY = FOLD.with_name("basis_body.rs")
NORMALIZE = FOLD.with_name("normalize.rs")
NORMALIZE_BODY = FOLD.with_name("normalize_body.rs")
ASSEMBLE = FOLD.with_name("assemble.rs")
ASSEMBLE_BODY = FOLD.with_name("assemble_body.rs")
KERNEL = FOLD.with_name("kernel.rs")
KERNEL_BODY = FOLD.with_name("kernel_body.rs")
KERNEL_PROOF = PROOF.with_name("checked_u32_kernel_v1.rs")
KERNEL_SCHEMA = PROOF.with_name("checked_u32_kernel_schema.py")
SCHEMA = PROOF.with_name("checked_u32_normalization_schema.py")
ADAPTER = FOLD.parent.with_suffix(".rs")
KIR_SCHEMA = Path("crates/fe2o3-kernel-ir/src/ir.rs")
TEST = PROOF.with_name("checked_u32_prefix_test.py")
FOLD_SHA = "fc5167c5ea5eb019e872ed332b94b7aec697b32cf613cffae865d0037f537f5d"
PROOF_SHA = "0908f129f8c615fde3bdf5e561a2702dd2dfb398a79efd4314e8ef2e11b0d427"
BASIS_SHA = "48fa84b1d850c92157ff217ca56eb1ce90c5d5225bd5e7a45143f56c5a82af7d"
ADAPTER_SHA = "073caacd893e2d4b235c5232ab93f8f36ea5b618d612044249a42e2731af78a8"
KERNEL_SHA = "7a7c6a14a65360d92d58107baaced591f4451107b4ad0d95f2da29aeca703a09"
KERNEL_PROOF_SHA = "9bdacb7a6c2fe6f8f5c2e28e0f1cbb2300fec6ce09c37fb5bf2b1025a134d875"
NORMALIZE_SHA = "d57442f493ef03158925e784dfb3332e8bccfc57132a7f0a058cf3b043eaa99a"
ASSEMBLE_SHA = "c9faa74b7401cfe751fe1f1ed806ee014443186b7ba70d55df4b17a0f8164610"
MACRO = "checked_u32_prefix_fold_body_v1"
INVARIANT = "symbolic_after(before, steps@, index as nat) == Some(state@),"
TARGETS = {
    "fold": dict(body=BODY, macro=MACRO, function="fold", verified=1,
                 contract="ensures accepted ==> symbolic_after(old(state)@, steps@, steps@.len()) == Some(final(state)@),",
                 invariants={"loop": INVARIANT}),
    "basis": dict(body=BASIS_BODY, macro="checked_u32_prefix_basis_body_v1",
                  function="initialize_argument_basis", verified=2,
                  contract="accepted == (old(kernel)@.len() == arguments@.len()",
                  invariants={
                      "loop": "paired_basis_prefix(arguments@, source@, kernel@, index as nat),",
                      "clear": "forall|slot: int| 0 <= slot < clear ==> #[trigger] source@[slot] == Origin::Uninitialized,",
                  }),
}
for name, macro, contract in (
    ("is_u32", "type", "ensures accepted == u32_type(types@, ty),"),
    ("scalar_local", "local", "ensures result == if valid_place(types@, locals@, *place) { Ok(place.local.0 as usize) }"),
    ("scalar_constant", "constant", "ensures result == constant_value(types@, *operand),"),
    ("source_step", "source_step", "ensures result == typed_step(types@, locals@, *statement, operations),"),
):
    TARGETS[name] = dict(body=NORMALIZE_BODY, macro="checked_u32_prefix_" + macro + "_body_v1",
                         function=name, verified=0, contract=contract, invariants={}, module="normalization")
TARGETS["assemble"] = dict(body=ASSEMBLE_BODY, macro="checked_u32_prefix_assemble_body_v1",
    function="assemble_source", verified=2, module="normalization",
    contract="ensures assembly_result(result) == source_assembly(types@, locals@, prefix@, spans@, root, function, block, kernel_block, operation),",
    invariants={
        "select": "select_spans(spans@, root, function, block, prefix@.len(), index as nat) == Some(selected@),",
        "loop": "walk_spans(types@, locals@, prefix@, spans@, selected@, kernel_block, operation, ordinal as nat)",
    })
for name, macro, contract, verified, invariants in (
    ("constant_binding", "constant", "ensures result == constant(*operation),", 0, {}),
    ("terminal_origin", "terminal", "ensures origin_result(result) == terminal_result(*terminal, *previous, origins@, operand, value, overflow, literal),", 0, {}),
    ("assemble_kernel", "assemble", "ensures origin_result(result) == assembly(arguments@, operations@, operand, value, overflow, literal),", 2, {
        "argument": "arguments_prefix(arguments@, argument as nat) == Some(origins@),",
        "loop": "constants_prefix(operations@, arguments_prefix(arguments@, arguments@.len()).unwrap(), index as nat) == Some(origins@),",
    }),
):
    TARGETS[name] = dict(body=KERNEL_BODY, macro="checked_u32_prefix_kernel_" + macro + "_body_v1", function=name,
        verified=verified, contract=contract, invariants=invariants, module="kernel_assembly", proof=KERNEL_PROOF)
VERIFIED_COUNT = 86
CONTROL_COUNT = 14
SCOPE = ("Shared row initialization accepts exactly positional, bounded, source-injective rows "
         "with matching KIR scratch length; it establishes exact paired origins and uninitialized "
         "unmapped source cells. Its denotation composes with uninitialized KIR padding and the fold. "
         "Successful shared origin fold refines an independent concrete u32 fold for every "
         "valid common argument vector; equal initialized terminal origins imply equal values. "
         "Shared typed source-AST normalization accepts exactly Nop/count0, typed Copy/count0 and u32 Constant/count1; "
         "it preserves direct statement denotation and composes with the origin step. AST field/variant/getter "
         "correspondence is source-checked with explicit irrelevant-payload erasure, not a parser/layout theorem. "
         "Actual retained source-span selection and prefix assembly have exact acceptance and pre-ADD AST denotation; "
         "the last span is checked but terminal AST evaluation is separate. "
         "Actual borrowed KIR assembly accepts exactly the bounded constant/checked-add profile, preserves sparse IDs "
         "and agrees with independent typed KIR evaluation and source-fold terminal values. "
         "Shared checked add proves modulo-2^32 value and overflow. No ABI discovery, terminal source-AST, "
         "rustc extraction, machine-entry, continuation, memory or launch-authority proof.")


def digest(data):
    return hashlib.sha256(data).hexdigest()


def need(condition, message):
    if not condition:
        raise ValueError(message)


need(digest((ROOT / HELPER).read_bytes()) == HELPER_SHA, "pinned campaign helper")
spec = importlib.util.spec_from_file_location("prefix_add_support", ROOT / HELPER)
base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)
support = base.support
save, strict_json = base.save, base.strict_json
schema_spec = importlib.util.spec_from_file_location("prefix_normalization_schema", ROOT / SCHEMA)
schema = importlib.util.module_from_spec(schema_spec)
schema_spec.loader.exec_module(schema)
kernel_schema_spec = importlib.util.spec_from_file_location("prefix_kernel_schema", ROOT / KERNEL_SCHEMA)
kernel_schema = importlib.util.module_from_spec(kernel_schema_spec)
kernel_schema_spec.loader.exec_module(kernel_schema)


def validate(sources):
    base.validate_sources(sources)
    for path, expected in ((HELPER, HELPER_SHA), (FOLD, FOLD_SHA), (PROOF, PROOF_SHA),
                           (BASIS, BASIS_SHA), (ADAPTER, ADAPTER_SHA), (NORMALIZE, NORMALIZE_SHA), (ASSEMBLE, ASSEMBLE_SHA),
                           (KERNEL, KERNEL_SHA), (KERNEL_PROOF, KERNEL_PROOF_SHA)):
        need(digest(sources[str(path)]) == expected, "reviewed forwarding/contract: " + str(path))
    base.shared_body(base.tokens(sources[str(BODY)].decode("ascii")), MACRO)
    base.shared_body(base.tokens(sources[str(BASIS_BODY)].decode("ascii")), TARGETS["basis"]["macro"])
    base.shared_body(base.tokens(sources[str(ASSEMBLE_BODY)].decode("ascii")), TARGETS["assemble"]["macro"])
    remaining = base.tokens(sources[str(NORMALIZE_BODY)].decode("ascii"))
    for name in ("is_u32", "scalar_local", "scalar_constant", "source_step"):
        macro = TARGETS[name]["macro"]
        _, end = base.one_block(remaining, "macro_rules! " + macro)
        base.shared_body(remaining[:end], macro)
        remaining = remaining[end:]
    need(not remaining, "closed four-macro normalizer")
    remaining = base.tokens(sources[str(KERNEL_BODY)].decode("ascii"))
    for name in ("constant_binding", "terminal_origin", "assemble_kernel"):
        macro = TARGETS[name]["macro"]
        _, end = base.one_block(remaining, "macro_rules! " + macro)
        base.shared_body(remaining[:end], macro)
        remaining = remaining[end:]
    need(not remaining, "closed three-macro KIR assembler")
    schema.validate(base, sources, ADAPTER, NORMALIZE, PROOF, ASSEMBLE)
    kernel_schema.validate(base, schema, sources, KERNEL_PROOF, KERNEL, ADAPTER)
    fields = base.tokens("argument: usize, semantic_local: u32, kernel_ir_value: ValueId,")
    for path, declaration in ((ADAPTER, "pub struct CheckedU32PrefixArgumentV1"),
                              (PROOF, "struct CheckedU32PrefixArgumentV1")):
        need(base.one_block(base.tokens(sources[str(path)].decode("ascii")), declaration)[0] == fields,
             "exact existing argument row schema")
    need(len(base.positions(base.tokens(sources[str(KIR_SCHEMA)].decode("ascii")),
                            base.tokens("pub struct ValueId(pub u32);"))) == 1,
         "exact opaque KIR value identity shape")
    kir = base.tokens(sources[str(KIR_SCHEMA)].decode("ascii"))
    schema.top_level(base, kir, "pub struct BlockId(pub u32);",
                     "#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]")


def snapshot():
    sources = base.source_snapshot()
    for path in (HELPER, FOLD, BODY, BASIS, BASIS_BODY, NORMALIZE, NORMALIZE_BODY, ASSEMBLE, ASSEMBLE_BODY, SCHEMA,
                 schema.MODEL, schema.CORRESPONDENCE, schema.CAPTURE, ADAPTER, KIR_SCHEMA, PROOF, TEST,
                 KERNEL, KERNEL_BODY, KERNEL_PROOF, KERNEL_SCHEMA, kernel_schema.TYPES,
                 PROOF.with_name("run-checked-u32-prefix.sh"), Path(__file__).relative_to(ROOT)):
        selected = ROOT / path
        need(selected.is_file() and not selected.is_symlink(), "ordinary source: " + str(path))
        sources[str(path)] = selected.read_bytes()
    validate(sources)
    return sources


def mutants(body):
    cases = {}
    for name, before, after, invariant in (
        ("destination-oob", "if step.destination >= $state.len() {\n                    return false;",
         "if step.destination >= $state.len() {\n                    return true;", False),
        ("source-oob", "if source >= $state.len() {\n                            return false;",
         "if source >= $state.len() {\n                            return true;", False),
        ("uninitialized", "if matches!(origin, Origin::Uninitialized) {\n                    return false;",
         "if matches!(origin, Origin::Uninitialized) {\n                    return true;", False),
        ("wrong-constant", "Origin::Constant(value)", "Origin::Constant(value ^ 1)", True),
        ("wrong-copy", "$state[source]", "$state[step.destination]", True),
        ("wrong-destination", "$state[step.destination] = origin;", "$state[0] = origin;", True),
        ("write-before-read", "let origin = match step.input {",
         "$state[step.destination] = Origin::Constant(0);\n                let origin = match step.input {", True),
        ("reverse-order", "let step = $steps[$index];", "let step = $steps[$steps.len() - 1 - $index];", True),
    ):
        need(body.count(before) == 1, "one mutation site: " + name)
        cases[name] = (body.replace(before, after), invariant)
    need(len({value[0] for value in cases.values()}) == 8, "distinct logical mutants")
    return cases


def basis_mutants(body):
    cases = {}
    for name, before, after, failure in (
        ("length-success", "if $kernel.len() != $arguments.len() {\n                return false;",
         "if $kernel.len() != $arguments.len() {\n                return true;", "post"),
        ("invalid-row-success", "if binding.argument != $index || local >= $source.len() {\n                    return false;",
         "if binding.argument != $index || local >= $source.len() {\n                    return true;", "post"),
        ("duplicate-success", "if !matches!($source[local], Origin::Uninitialized) {\n                    return false;",
         "if !matches!($source[local], Origin::Uninitialized) {\n                    return true;", "post"),
        ("wrong-clear", "$source[$clear] = Origin::Uninitialized;", "$source[$clear] = Origin::Constant(0);", "clear"),
        ("wrong-source-origin", "$source[local] = Origin::Argument($index);", "$source[local] = Origin::Constant(0);", "loop"),
        ("wrong-kernel-origin", "$kernel[$index] = Origin::Argument($index);", "$kernel[$index] = Origin::Constant(0);", "loop"),
        ("wrong-local", "$source[local] = Origin::Argument($index);", "$source[0] = Origin::Argument($index);", "loop"),
        ("always-reject", "            true\n", "            false\n", "post"),
    ):
        need(body.count(before) == 1, "one basis mutation site: " + name)
        cases[name] = (body.replace(before, after), failure)
    need(len({value[0] for value in cases.values()}) == 8, "distinct basis mutants")
    return cases


def normalization_mutants(body):
    cases = {}
    for name, target, before, after in (
        ("type-oob-success", "is_u32", "return false;", "return true;"),
        ("signed-type", "is_u32", "signed: false,", "signed: true,"),
        ("wide-type", "is_u32", "bits: 32", "bits: 64"),
        ("projected-local", "scalar_local", "!$place.projections().is_empty()", "false"),
        ("wrong-local-type", "scalar_local", "$locals[index].ty().index() != $place.ty().index()", "false"),
        ("wrong-local-index", "scalar_local", "Ok(index)", "Ok(0usize)"),
        ("constant-size", "scalar_constant", "value.size_bytes() != 4", "value.size_bytes() != 8"),
        ("constant-truncation", "scalar_constant", "value.bits() > u32::MAX as u128", "false"),
        ("wrong-constant", "scalar_constant", "Some(value.bits() as u32)", "Some((value.bits() as u32) ^ 1)"),
        ("copy-count", "source_step", "SemanticOperandV1::Copy(place) if $operations == 0", "SemanticOperandV1::Copy(place) if $operations == 1"),
        ("constant-count", "source_step", "_ if $operations == 1", "_ if $operations == 0"),
        ("result-type", "source_step", "assign.value().result_type().index() != assign.destination().ty().index()", "false"),
        ("wrong-destination", "source_step", "PrefixStep { destination, input }", "PrefixStep { destination: destination ^ 1, input }"),
        ("wrong-copy-source", "source_step", "PrefixInput::Cell(scalar_local($types, $locals, place)?)", "PrefixInput::Cell(scalar_local($types, $locals, assign.destination())?)"),
        ("nop-count", "source_step", "SemanticStatementKindV1::Nop if $operations == 0", "SemanticStatementKindV1::Nop"),
        ("move-as-copy", "source_step", "SemanticOperandV1::Copy(place) if",
         "SemanticOperandV1::Move(place) if $operations == 0 => { PrefixInput::Cell(scalar_local($types, $locals, place)?) }, SemanticOperandV1::Copy(place) if"),
    ):
        need(body.count(before) == 1, "one normalization mutation site: " + name)
        changed = body.replace(before, after)
        if name == "constant-truncation":
            changed = changed.replace("Some(value.bits() as u32)", "Some((value.bits() % 0x1_0000_0000u128) as u32)")
        cases[name] = (changed, target)
    nop, step = "=> Ok(None)", "Ok(Some(PrefixStep { destination, input }))"
    need(body.count(nop) == body.count(step) == 1, "two success sites for always-reject")
    changed = body.replace(nop, "=> Err(CheckedU32PrefixErrorV1::Source)").replace(
        step, "{ let _ = (destination, input); Err(CheckedU32PrefixErrorV1::Source) }")
    cases["always-reject"] = (changed, "source_step")
    need(len(cases) == len({value[0] for value in cases.values()}) == 17, "distinct normalization mutants")
    return cases


def assembly_mutants(body):
    cases = {}
    for name, before, after, failure in (
        ("wrong-root", "span.correspondence_owner().index() == $root", "span.correspondence_owner().index() == ($root ^ 1)", "post"),
        ("wrong-function", "span.semantic_function().index() == $function", "span.semantic_function().index() == ($function ^ 1)", "post"),
        ("wrong-block", "span.semantic_block().index() == $block", "span.semantic_block().index() == ($block ^ 1)", "post"),
        ("duplicate", "if $selected[ordinal].is_some()", "if false && $selected[ordinal].is_some()", "select"),
        ("wrong-row", "$selected[ordinal] = Some($index);", "$selected[ordinal] = Some(0usize);", "select"),
        ("missing-success", "let Some(index) = $selected[$ordinal] else {\n                    return Err(CheckedU32PrefixErrorV1::Span);",
         "let Some(index) = $selected[$ordinal] else {\n                    return Ok(($steps, $next));", "post"),
        ("kernel-block", "span.kernel_ir_block().0 != $kernel_block", "false", "loop"),
        ("first-operation", "span.first_operation_ordinal() != $next", "false", "loop"),
        ("terminal-count", "span.operation_count() != 2", "span.operation_count() > 2", "loop"),
        ("terminal-operation", "span.first_operation_ordinal().checked_add(1) != Some($operation)", "false", "loop"),
        ("wrong-end", "$next = end;", "$next = end ^ 1;", "loop"),
        ("discard-step", "$steps.push(step);", "let _ = step;", "loop"),
        ("wrong-destination", "$steps.push(step);", "$steps.push(PrefixStep { destination: step.destination ^ 1, input: step.input });", "loop"),
        ("reject-boundary", "$prefix.len() > 256", "$prefix.len() > 255", "post"),
        ("always-reject", "$prefix.len() > 256", "$prefix.len() > 0", "post"),
    ):
        need(body.count(before) == 1, "one source assembly mutation: " + name)
        cases[name] = (body.replace(before, after), failure)
    need(len(cases) == len({v[0] for v in cases.values()}) == 15, "distinct assembly mutants")
    return cases


def kernel_mutants(body):
    cases = {}
    for name, target, before, after, failure in (
        ("constant-type", "constant_binding", "Type::Scalar(ScalarType::U32)", "Type::Scalar(ScalarType::I32)", "post"),
        ("constant-arity", "constant_binding", "$operation.results.len() != 1", "$operation.results.len() < 1", "post"),
        ("constant-value", "constant_binding", "Some((result.id.0, value))", "Some((result.id.0, value ^ 1))", "post"),
        ("constant-id", "constant_binding", "Some((result.id.0, value))", "Some((result.id.0 ^ 1, value))", "post"),
        ("checked-operator", "terminal_origin", "CheckedBinaryOperator::Add", "CheckedBinaryOperator::Add | CheckedBinaryOperator::Subtract", "post"),
        ("output-order", "terminal_origin", "let value = &$terminal.results[0];", "let value = &$terminal.results[1];", "post"),
        ("capture-operand", "terminal_origin", "lhs.0 != $operand", "false", "post"),
        ("capture-output", "terminal_origin", "value.id.0 != $value", "false", "post"),
        ("output-alias", "terminal_origin", "value.id.0 == overflow.id.0", "false", "post"),
        ("output-freshness", "terminal_origin", "$origins.contains_key(&value.id.0)", "false", "post"),
        ("overflow-type", "terminal_origin", "Type::Scalar(ScalarType::Bool)", "Type::Scalar(ScalarType::U32)", "post"),
        ("previous-literal", "terminal_origin", "constant_binding($previous) != Some((rhs.0, $literal))", "false", "post"),
        ("rhs-origin", "terminal_origin", "if *value == $literal", "if true", "post"),
        ("always-reject", "terminal_origin", "Some(origin) => Ok(*origin)", "Some(_origin) => Err(CheckedU32PrefixErrorV1::Kernel)", "post"),
        ("capacity", "assemble_kernel", "$operations.len() > 257", "$operations.len() > 256", "post"),
        ("argument-index", "assemble_kernel", "binding.argument != $argument", "false", "argument"),
        ("argument-origin", "assemble_kernel", "Origin::Argument($argument)", "Origin::Argument(0)", "argument"),
        ("duplicate-argument", "assemble_kernel", "Origin::Argument($argument)).is_some()", "Origin::Argument($argument)).is_some() && false", "argument"),
        ("constant-origin", "assemble_kernel", "Origin::Constant(value)).is_some()", "Origin::Constant(value ^ 1)).is_some()", "loop"),
        ("duplicate-constant", "assemble_kernel", "Origin::Constant(value)).is_some()", "Origin::Constant(value)).is_some() && false", "loop"),
    ):
        # A type tag can also occur in a different macro; mutate only the selected one.
        start = body.index("macro_rules! " + TARGETS[target]["macro"])
        end = body.find("macro_rules!", start + 1)
        end = len(body) if end < 0 else end
        part = body[start:end]
        need(part.count(before) == 1, "one KIR mutation site: " + name)
        cases[name] = (body[:start] + part.replace(before, after) + body[end:], target, failure)
    need(len(cases) == len({v[0] for v in cases.values()}) == 20, "distinct KIR mutants")
    return cases


def target_proof(proof, target):
    return proof.with_name(TARGETS[target]["proof"].name) if "proof" in TARGETS[target] else proof


def locations(proof, target="fold", failure="post"):
    proof = target_proof(proof, target)
    selected = TARGETS[target]
    lines = proof.read_text().splitlines()
    call = next(i + 1 for i, line in enumerate(lines) if line.strip().startswith(selected["macro"] + "!("))
    contract = next(i + 1 for i, line in enumerate(lines) if line.strip() == selected["contract"])
    invariant = (next(i + 1 for i, line in enumerate(lines)
                     if line.strip() == selected["invariants"]["loop" if failure == "post" else failure])
                 if selected["invariants"] else contract)
    body = (proof.parent / "../src/gfx942_local_checked_u32_add_v1/source_prefix" / selected["body"].name).resolve()
    definition = next(i + 1 for i, line in enumerate(body.read_text().splitlines())
                      if line.startswith("macro_rules! " + selected["macro"] + " {"))
    return call, contract, invariant, body, definition


def source_step_exit(span, proof, call, target, failure):
    # Verus may locate a tail-expression failure at the one-macro wrapper exit.
    if target != "source_step" or failure != "post":
        return False
    content = proof.read_bytes()
    lines = content.decode("ascii").splitlines()
    return (digest(content) == PROOF_SHA
            and lines[call - 2:call + 1] == ["{",
                "    checked_u32_prefix_source_step_body_v1!(verus_exec_expr, types, locals, statement, operations)", "}"]
            and span.get("is_primary") is False
            and span.get("label") == "at the end of the function body"
            and "expansion" in span and span["expansion"] is None
            and Path(span.get("file_name", "")).resolve() == proof
            and type(span.get("line_start")) is int and span["line_start"] == call - 1
            and type(span.get("line_end")) is int and span["line_end"] == call + 1)


def classify(status, stdout, stderr, proof, target="fold", failure=None):
    try:
        selected = TARGETS[target]
        negative = failure is not None
        need(failure is None or failure == "post" or failure in selected["invariants"], "known failure target")
        invariant = negative and failure != "post"
        data = strict_json(stdout)
        rows = [strict_json(line) for line in stderr.splitlines() if line]
        expected = {"encountered-error": negative, "encountered-vir-error": False,
                    "errors": 1 if negative else 0, "verified": selected["verified"] if negative else VERIFIED_COUNT,
                    "is-verifying-entire-crate": not negative}
        if not negative:
            expected["success"] = True
        result = data["verification-results"]
        need(data["verus"] == base.VERIFIER and result == expected
             and all(type(result[k]) is type(v) for k, v in expected.items()), "exact proof result")
        if not negative:
            return status == 0 and not rows
        need(status == 1 and all(row.get("level") in {"error", "note"} for row in rows), "logical failure")
        message = "invariant not satisfied at end of loop body" if invariant else "postcondition not satisfied"
        errors = [row for row in rows if row["level"] == "error"]
        logical = [row for row in errors if row.get("message") == message]
        abort = [row for row in errors if row.get("message") == "aborting due to 1 previous error"
                 and row.get("spans") == []]
        need(len(errors) == 2 and len(logical) == len(abort) == 1, "one exact logical rejection")
        notes = {"verifying root module (selected functions)", "verifying module normalization (selected functions)",
                 "verifying module kernel_assembly (selected functions)",
                 "function body check: not all errors may have been reported; rerun with a higher value for --multiple-errors to find other potential errors in this function",
                 "while loop: not all errors may have been reported; rerun with a higher value for --multiple-errors to find other potential errors in this function"}
        need(all(row.get("message") in notes for row in rows if row["level"] == "note"), "known notes only")
        call, contract, loop, body, definition = locations(proof, target, failure)
        proof = target_proof(proof, target)
        need(any(span.get("is_primary") is True and Path(span.get("file_name", "")).resolve() == proof
                 and type(span.get("line_start")) is int and span["line_start"] == (loop if invariant else contract)
                 for span in logical[0].get("spans", [])), "exact target contract span")
        expansions = [row for row in rows if row["level"] == "note" and row.get("message", "").startswith("while loop:")] if invariant else logical
        return any(base.macro_expansion(span, proof, call, selected["macro"], body, definition)
                   or source_step_exit(span, proof, call, target, failure)
                   for row in expansions for span in row.get("spans", []))
    except (ValueError, KeyError, TypeError, AttributeError, IndexError, StopIteration, OSError):
        return False


def main():
    need(sys.flags.isolated and sys.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--verus", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    out, verus = args.output, args.verus
    need(out.is_absolute() and out.resolve() == out and not out.exists()
         and not out.is_relative_to(ROOT), "fresh external output")
    need(verus.is_absolute() and verus.resolve() == verus and digest(verus.read_bytes()) == base.VERUS_HASH,
         "canonical pinned verifier")
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    before = snapshot()
    owner = types.ModuleType("prefix_process_owner")
    owner.__file__ = str(ROOT / support.OWNER)
    sys.modules[owner.__name__] = owner
    exec(compile(before[str(support.OWNER)], owner.__file__, "exec"), owner.__dict__)
    out.mkdir(parents=True)
    (out / "tmp").mkdir()
    for path, data in before.items():
        target = out / "inputs" / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    save(out / "source-before.json", {p: digest(data) for p, data in before.items()})
    home = Path.home()
    env = {"HOME": str(home), "PATH": str(home / ".cargo/bin") + ":/usr/bin:/bin",
           "LANG": "C.UTF-8", "LC_ALL": "C.UTF-8", "RUSTUP_HOME": str(home / ".rustup"),
           "CARGO_HOME": str(home / ".cargo"), "VERUS_Z3_PATH": str(verus.parent / "z3"), "TMPDIR": str(out / "tmp")}
    save(out / "environment.json", env)
    rows = []
    handlers = {number: signal.getsignal(number) for number in owner.SIGNALS}
    for number in handlers:
        signal.signal(number, owner.interrupted)

    def run(name, command, accept):
        need(snapshot() == before, "source continuity before " + name)
        status, stdout, stderr = owner.run_owned(command, 130, out / name, env)
        receipt = strict_json((out / name / "record.json").read_text())
        accepted = receipt.get("group_absent") is True and accept(status, stdout, stderr)
        rows.append(dict(name=name, status=status, accepted=accepted))
        print(name + ": " + ("PASS" if accepted else "FAIL"), flush=True)
        need(snapshot() == before and accepted, "stage rejected: " + name)

    def closure(name):
        run(name, ["/bin/sh", str(ROOT / support.CLOSURE), str(verus.parent), str(ROOT / support.MANIFEST)],
            lambda status, stdout, stderr: status == 0 and not stderr and stdout ==
            "PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n")

    def prove(name, changed=None, target="fold", failure=None):
        staged = out / (name + "-source")
        inputs = {path: before[str(path)] for path in (PROOF, BODY, BASIS_BODY, NORMALIZE_BODY, ASSEMBLE_BODY, KERNEL_PROOF, KERNEL_BODY, base.BODY)}
        if changed is not None:
            inputs[TARGETS[target]["body"]] = changed.encode("ascii")
        for path, data in inputs.items():
            destination = staged / path
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(data)
        proof = staged / PROOF
        run(name, ["/usr/bin/timeout", "--foreground", "--signal=TERM", "--kill-after=5", "120", str(verus),
                   "--crate-type", "lib", "--triggers-mode", "silent", "--no-cheating", "--output-json",
                   "--error-format=json", "--no-report-long-running", "--num-threads", "1", "--multiple-errors", "1",
                   *(["--verify-function", "*" + TARGETS[target]["function"],
                      *(["--verify-only-module", TARGETS[target]["module"]] if "module" in TARGETS[target] else ["--verify-root"])]
                     if changed is not None else []), str(proof)],
            lambda status, stdout, stderr: classify(status, stdout, stderr, proof, target, failure))
        need(all((staged / path).is_file() and not (staged / path).is_symlink()
                 and (staged / path).read_bytes() == data for path, data in inputs.items()), "staged continuity")

    error = None
    unchanged = False
    try:
        run("controls", [sys.executable, "-I", "-B", str(ROOT / TEST)],
            lambda status, stdout, stderr: status == 0 and not stdout and f"\nRan {CONTROL_COUNT} tests in " in stderr and stderr.endswith("\nOK\n"))
        closure("release-before")
        prove("positive-before")
        for name, (changed, invariant) in mutants(before[str(BODY)].decode("ascii")).items():
            prove("negative-" + name, changed, "fold", "loop" if invariant else "post")
        for name, (changed, failure) in basis_mutants(before[str(BASIS_BODY)].decode("ascii")).items():
            prove("negative-basis-" + name, changed, "basis", failure)
        for name, (changed, target) in normalization_mutants(before[str(NORMALIZE_BODY)].decode("ascii")).items():
            prove("negative-normalization-" + name, changed, target, "post")
        for name, (changed, failure) in assembly_mutants(before[str(ASSEMBLE_BODY)].decode("ascii")).items():
            prove("negative-assembly-" + name, changed, "assemble", failure)
        for name, (changed, target, failure) in kernel_mutants(before[str(KERNEL_BODY)].decode("ascii")).items():
            prove("negative-kernel-" + name, changed, target, failure)
        prove("positive-after")
    except BaseException as failure:
        error = type(failure).__name__ + ": " + str(failure)
    finally:
        try:
            closure("release-after")
            after = snapshot()
            save(out / "source-after.json", {p: digest(data) for p, data in after.items()})
            unchanged = after == before
        except BaseException as failure:
            error = (error or "") + "; closing: " + type(failure).__name__ + ": " + str(failure)
        for number, handler in handlers.items():
            signal.signal(number, handler)
    accepted = error is None and unchanged and len(rows) == 73 and all(row["accepted"] for row in rows)
    save(out / "result.json", dict(accepted=accepted, source_unchanged=unchanged, scope=SCOPE,
         verified_obligations=VERIFIED_COUNT, logical_mutants=68, controls=CONTROL_COUNT, stages=rows, error=error,
         grants_application_authority=False, proves_normalization_adapters=False,
         proves_argument_basis_initialization=True, proves_typed_source_statement_normalization=True,
         proves_actual_source_span_assembly=True, proves_terminal_source_statement=False,
         proves_actual_kir_prefix_assembly=True,
         structural_ast_correspondence="source-checked explicit irrelevant-payload erasure"))
    return 0 if accepted else 1


if __name__ == "__main__":
    raise SystemExit(main())
