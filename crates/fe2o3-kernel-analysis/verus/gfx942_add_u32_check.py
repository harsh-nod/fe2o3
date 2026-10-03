#!/usr/bin/env python3
"""Check shared add/carry and projected state-transition proofs and logical mutants."""

import argparse
import hashlib
import importlib.util
import os
from pathlib import Path
import re
import resource
import signal
import sys
import types


ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-kernel-analysis/verus")
SRC = Path("crates/fe2o3-kernel-analysis/src")
BODY = SRC / "gfx942_integer_semantics_v1/add_u32_body.rs"
EXECUTE = SRC / "gfx942_integer_semantics_v1/execute_body.rs"
MOV_RUST = SRC / "gfx942_integer_semantics_v1/mov_b32.rs"
MOV_BODY = SRC / "gfx942_integer_semantics_v1/mov_b32_body.rs"
PREFIX_RUST = SRC / "gfx942_integer_semantics_v1/mov_prefix_add.rs"
PREFIX_BODY = SRC / "gfx942_integer_semantics_v1/mov_prefix_add_body.rs"
RUST = SRC / "gfx942_integer_semantics_v1.rs"
PROOF = V / "gfx942_add_u32_v1.rs"
TEST = V / "gfx942_add_u32_test.py"
WRAPPER = V / "run-gfx942-add-u32.sh"
SUPPORT = Path("crates/fe2o3-runtime-model/verus/compute_xgmi_packet_plan_check.py")
SUPPORT_HASH = "61074f6cdbfde1df24bff3d1e56e04398f2850dd00518c233d847f13a29e0db4"
VERUS_HASH = "d97501a883931d1d173b1bf4b6cf4d973f16d105dbcb468e177b52b2331612d2"
FOCUS = "gfx942_add_u32_v1"
MACRO = "gfx942_add_u32_body_v1"
STATE_FOCUS = "Gfx942SAddU32V1::execute"
STATE_MACRO = "gfx942_s_add_u32_execute_body_v1"
MOV_FOCUS = "Gfx942SMovB32V1::execute"
MOV_MACRO = "gfx942_s_mov_b32_execute_body_v1"
ORIGIN_FOCUS = "prefix_origins"
ORIGIN_MACRO = "gfx942_mov_prefix_origins_body_v1"
PREFIX_FOCUS = "Gfx942MovPrefixAddU32V1::execute"
PREFIX_MACRO = "gfx942_mov_prefix_execute_body_v1"
TARGETS = {FOCUS: (MACRO, BODY), STATE_FOCUS: (STATE_MACRO, EXECUTE),
           MOV_FOCUS: (MOV_MACRO, MOV_BODY), ORIGIN_FOCUS: (ORIGIN_MACRO, PREFIX_BODY),
           PREFIX_FOCUS: (PREFIX_MACRO, PREFIX_BODY)}
# Tokens of the reviewed declarations, validity preconditions, specifications,
# postconditions, and forwarding bodies. This is not a hash-based proof claim.
PROOF_TOKENS_SHA = "a655912622c517e3f23e036f37ca3b0ce1aa09083afe535584a0e41950d4d969"
CONTROL_COUNT = 17
VERIFIED_COUNT = 16  # Includes constants, Clone, getters and loop obligations.
SCOPE = (
    "Shared executable ADD/MOV and bounded MOV-prefix/ADD transition over 102 SGPRs plus SCC, "
    "conditional on valid indices and at most 64 total instructions: actual origin and state folds, "
    "old-input aliasing, modulo value/carry, returned result and untouched-register frame. "
    "No proof of decoder validity, ISA conformance, source/KIR/LLVM entry-state "
    "correspondence, control flow, memory effects, hardware, whole-kernel refinement or authority."
)


def need(value, message):
    if not value:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


support_path = ROOT / SUPPORT
need(support_path.is_file() and not support_path.is_symlink()
     and digest(support_path.read_bytes()) == SUPPORT_HASH, "pinned campaign support")
spec = importlib.util.spec_from_file_location("gfx942_add_proof_support", support_path)
support = importlib.util.module_from_spec(spec)
spec.loader.exec_module(support)
save, strict_json, VERIFIER = support.save, support.strict_json, support.VERIFIER


def tokens(source):
    """Small fail-closed scanner for these sources, not a general Rust parser."""
    result = []
    for match in re.finditer(r'//[^\n]*|/\*[\s\S]*?\*/|"(?:\\.|[^"\\])*"'
                            r'|[A-Za-z_][A-Za-z_0-9]*|[0-9][A-Za-z_0-9]*|[^\s]', source):
        token = match.group()
        if token.startswith("//"):
            continue
        if token.startswith("/*"):
            need("/*" not in token[2:], "no nested comments in checked declarations")
            continue
        result.append(token)
    return result


def positions(source, pattern):
    return [i for i in range(len(source) - len(pattern) + 1)
            if source[i:i + len(pattern)] == pattern]


def one_block(source, prefix):
    starts = positions(source, tokens(prefix) + ["{"])
    need(len(starts) == 1, "one declaration: " + prefix)
    start = starts[0] + len(tokens(prefix)) + 1
    depth = 1
    for end in range(start, len(source)):
        depth += (source[end] == "{") - (source[end] == "}")
        if depth == 0:
            return source[start:end], end + 1
    raise ValueError("unclosed declaration: " + prefix)


def schemas(source):
    for declaration, fields in (
        ("pub struct Gfx942U32AddResultV1", "pub value: u32, pub scc: bool,"),
        ("pub enum Gfx942U32SourceV1", "Sgpr(u8), Constant(u32),"),
        ("pub struct Gfx942ScalarIntegerStateV1",
         "registers: [u32; GFX942_ORDINARY_SGPR_COUNT_V1], scc: bool,"),
        ("pub struct Gfx942SAddU32V1",
         "destination: u8, sources: [Gfx942U32SourceV1; 2], bytes: [u8; 8], byte_len: usize,"),
    ):
        need(one_block(source, declaration)[0] == tokens(fields), "exact schema: " + declaration)
    need(len(positions(source, tokens("const GFX942_ORDINARY_SGPR_COUNT_V1"))) == 1
         and len(positions(source, tokens("pub const GFX942_ORDINARY_SGPR_COUNT_V1: usize = 102;"))) == 1,
         "exact 102 ordinary registers")


def includes(source, expected):
    starts = positions(source, ["include", "!"])
    need(len(starts) == len(expected), "exact shared include count")
    for start, path in zip(starts, expected):
        need(source[start:start + 6] == ["include", "!", "(", '"' + path + '"', ")", ";"],
             "exact shared include path")


def shared_body(source, name):
    body, end = one_block(source, "macro_rules! " + name)
    need(source[:3] == ["macro_rules", "!", name] and end == len(source), "closed shared macro")
    need(source.count("macro_rules") == 1 and not any(token in source for token in
         ("include", "cfg", "cfg_attr", "verus", "external_body", "external_fn_specification", "axiom")),
         "no alternate shared-body compilation or trust escape")
    need(not positions(body, ["assume", "("]) and not positions(body, ["admit", "("]),
         "no shared-body assumptions")


def method(source, owner, signature, body):
    implementation = one_block(source, "impl " + owner)[0]
    name = re.search(r"\bfn\s+(\w+)\(", signature)[1]
    need(len(positions(implementation, ["fn", name])) == 1, "unique method: " + owner + "::" + name)
    need(one_block(implementation, signature)[0] == tokens(body), "exact method: " + owner + "::" + name)


def ordinary_cfg(source):
    need(source.count("cfg") == 1 and "cfg_attr" not in source
         and not positions(source, ["#", "[", "path"])
         and source[-10:] == tokens("#[cfg(test)] mod tests;"), "only ordinary test-module cfg")


def validate_sources(sources):
    for path, expected in {SUPPORT: SUPPORT_HASH, **support.PINS}.items():
        need(digest(sources[str(path)]) == expected, "pinned support: " + str(path))
    rust, body, execute, proof = (tokens(sources[str(path)].decode("ascii"))
                                  for path in (RUST, BODY, EXECUTE, PROOF))
    mov, mov_body, prefix, prefix_body = (tokens(sources[str(path)].decode("ascii"))
                                        for path in (MOV_RUST, MOV_BODY, PREFIX_RUST, PREFIX_BODY))
    schemas(rust)
    schemas(proof)
    includes(rust, ["gfx942_integer_semantics_v1/add_u32_body.rs",
                    "gfx942_integer_semantics_v1/execute_body.rs"])
    includes(proof, ["../src/gfx942_integer_semantics_v1/add_u32_body.rs",
                     "../src/gfx942_integer_semantics_v1/execute_body.rs",
                     "../src/gfx942_integer_semantics_v1/mov_b32_body.rs",
                     "../src/gfx942_integer_semantics_v1/mov_prefix_add_body.rs"])
    includes(mov, ["mov_b32_body.rs"])
    includes(prefix, ["mov_prefix_add_body.rs"])
    shared_body(body, MACRO)
    shared_body(execute, STATE_MACRO)
    shared_body(mov_body, MOV_MACRO)
    remaining = prefix_body
    for name in (ORIGIN_MACRO, PREFIX_MACRO):
        _, end = one_block(remaining, "macro_rules! " + name)
        shared_body(remaining[:end], name)
        remaining = remaining[end:]
    need(not remaining, "closed two-macro fold body")
    need("macro_rules" not in rust + proof + mov, "no shadow shared macros")
    need(prefix.count("macro_rules") == 1 and one_block(prefix, "macro_rules! prefix_rust_expr")[0]
         == tokens("($body:expr) => { $body };"), "exact ordinary expression adapter")
    for module in (rust, mov, prefix):
        ordinary_cfg(module)
    for child in ("mov_b32", "mov_prefix_add"):
        need(len(positions(rust, tokens("mod " + child + ";"))) == 1, "exact child module: " + child)
    for owner, fields, module in (
        ("Gfx942SMovB32V1", "destination: u8, source: Gfx942U32SourceV1, bytes: [u8; 8], byte_len: usize,", mov),
        ("Gfx942MovPrefixAddU32V1", "function: String, block: u32, first_offset: u64, add_offset: u64, end_offset: u64, moves: Vec<Gfx942SMovB32V1>, add: Gfx942SAddU32V1,", prefix),
    ):
        for selected in (module, proof):
            need(one_block(selected, "pub struct " + owner)[0] == tokens(fields), "exact schema: " + owner)
    for selected in (prefix, proof):
        need(one_block(selected, "pub enum Gfx942U32OriginV1")[0]
             == tokens("EntrySgpr(u8), Constant(u32),"), "exact origin schema")
        need(len(positions(selected, tokens("const GFX942_MOV_PREFIX_ADD_MAX_INSTRUCTIONS_V1"))) == 1
             and len(positions(selected, tokens("pub const GFX942_MOV_PREFIX_ADD_MAX_INSTRUCTIONS_V1: usize = 64;"))) == 1,
             "exact64 instruction bound")
    for name in ("Gfx942SMovB32V1", "Gfx942SAddU32V1", "Gfx942U32OriginV1", "Gfx942MovPrefixAddU32V1",
                 "Gfx942ScalarIntegerStateV1", "Gfx942U32SourceV1", "Gfx942U32AddResultV1"):
        combined = rust + mov + prefix
        need(len(positions(combined, ["struct", name])) + len(positions(combined, ["enum", name])) == 1
             and not positions(combined, ["type", name]), "no shadow model type: " + name)
    production = one_block(rust, "pub fn gfx942_add_u32_v1(a: u32, b: u32) -> Gfx942U32AddResultV1")[0]
    need(production == tokens(MACRO + "!(a, b)"), "actual arithmetic forwarding")
    method(rust, "Gfx942SAddU32V1", "pub fn execute(&self, state: &mut Gfx942ScalarIntegerStateV1) -> Gfx942U32AddResultV1", STATE_MACRO + "!(self, state)")
    method(mov, "Gfx942SMovB32V1", "pub fn execute(&self, state: &mut Gfx942ScalarIntegerStateV1) -> u32", MOV_MACRO + "!(self, state)")
    method(rust, "Gfx942SAddU32V1", "pub const fn sources(&self) -> [Gfx942U32SourceV1; 2]", "self.sources")
    method(mov, "Gfx942SMovB32V1", "pub const fn source(&self) -> Gfx942U32SourceV1", "self.source")
    method(mov, "Gfx942SMovB32V1", "pub const fn destination(&self) -> u8", "self.destination")
    method(prefix, "Gfx942MovPrefixAddU32V1", "pub fn execute(&self, state: &mut Gfx942ScalarIntegerStateV1) -> Gfx942U32AddResultV1",
           "let moves = self.moves.as_slice(); let add = &self.add; " + PREFIX_MACRO + "!(prefix_rust_expr, moves, add, state, index, [])")
    method(prefix, "Gfx942MovPrefixAddU32V1", "pub fn terminal_origins(&self) -> [Gfx942U32OriginV1; 2]", "prefix_origins(&self.moves, &self.add)")
    need(one_block(prefix, "fn prefix_origins(moves: &[Gfx942SMovB32V1], add: &Gfx942SAddU32V1) -> [Gfx942U32OriginV1; 2]")[0]
         == tokens(ORIGIN_MACRO + "!(prefix_rust_expr, moves, add, origins, index, [], [])"), "actual origin forwarding")
    for name, macro, arguments in ((FOCUS, MACRO, "a, b"), (STATE_FOCUS, STATE_MACRO, "self, state")):
        short_name = name.split("::")[-1]
        need(len(positions(rust, ["fn", short_name])) == 1 and rust.count(macro) == 1,
             "unique production function and macro use: " + name)
        proof_scope = one_block(proof, "impl " + name.split("::")[0])[0] if "::" in name else proof
        need(len(positions(proof_scope, ["fn", short_name])) == 1 and proof.count(macro) == 1
             and len(positions(proof, tokens("{ " + macro + "!(" + arguments + ") }"))) == 1,
             "actual proof forwarding: " + name)
    for focus, (macro, _) in TARGETS.items():
        scope = one_block(proof, "impl " + focus.split("::")[0])[0] if "::" in focus else proof
        need(len(positions(scope, ["fn", focus.split("::")[-1]])) == 1 and proof.count(macro) == 1,
             "unique exact proof target: " + focus)
    need(digest(support.json.dumps(proof, separators=(",", ":")).encode("ascii")) == PROOF_TOKENS_SHA,
         "reviewed theorem contracts and executable forwarding")


def source_snapshot():
    paths = [BODY, EXECUTE, RUST, MOV_RUST, MOV_BODY, PREFIX_RUST, PREFIX_BODY,
             PROOF, TEST, WRAPPER, Path(__file__).relative_to(ROOT), SUPPORT, *support.PINS]
    sources = {}
    for path in paths:
        selected = ROOT / path
        need(selected.is_file() and not selected.is_symlink(), "ordinary source file: " + str(path))
        sources[str(path)] = selected.read_bytes()
    validate_sources(sources)
    return sources


def mutations(body):
    cases = {}
    for name, before, after in (
        ("discarded-carry", "scc: sum >= 0x1_0000_0000_u64", "scc: false"),
        ("wrong-width", "value: (sum % 0x1_0000_0000_u64) as u32", "value: (sum % 0x1_0000_u64) as u32"),
        ("wrong-result", "value: (sum % 0x1_0000_0000_u64) as u32", "value: $a"),
    ):
        need(body.count(before) == 1, "unique executable mutation: " + name)
        cases[name] = body.replace(before, after)
    need(len(set(cases.values())) == 3, "three distinct logical negatives")
    return cases


def state_mutations(body):
    cases = {}
    for name, before, after in (
        ("early-alias-write", "let right = match", "$state.registers[$instruction.destination as usize] = left;\n        let right = match"),
        ("wrong-destination", "$state.registers[$instruction.destination as usize] = result.value;",
         "$state.registers[($instruction.destination as usize + 1) % 102] = result.value;"),
        ("clobbered-frame", "$state.scc = result.scc;",
         "$state.scc = result.scc;\n        $state.registers[($instruction.destination as usize + 1) % 102] =\n            $state.registers[($instruction.destination as usize + 1) % 102] ^ 1;"),
        ("stale-scc", "$state.scc = result.scc;", ""),
        ("wrong-return", "        result\n", "        Gfx942U32AddResultV1 { value: result.value ^ 1, scc: result.scc }\n"),
    ):
        need(body.count(before) == 1, "unique state mutation: " + name)
        cases[name] = body.replace(before, after)
    need(len(set(cases.values())) == 5, "five distinct state negatives")
    return cases


def mov_mutations(body):
    cases = {}
    for name, before, after in (
        ("mov-wrong-return", "        value\n", "        value ^ 1\n"),
        ("mov-early-alias-write", "let value = match", "$state.registers[$instruction.destination as usize] = 0;\n        let value = match"),
        ("mov-wrong-destination", "$state.registers[$instruction.destination as usize] = value;",
         "$state.registers[($instruction.destination as usize + 1) % 102] = value;"),
        ("mov-clobbered-frame", "        value\n", "        $state.registers[($instruction.destination as usize + 1) % 102] ^= 1;\n        value\n"),
        ("mov-changed-scc", "        value\n", "        $state.scc = !$state.scc;\n        value\n"),
    ):
        need(body.count(before) == 1, "unique MOV mutation: " + name)
        cases[name] = body.replace(before, after)
    need(len(set(cases.values())) == 5, "five distinct MOV negatives")
    return cases


def fold_mutations(body):
    marker = "macro_rules! " + PREFIX_MACRO
    need(body.count(marker) == 1, "one execution fold")
    origins, execute = body.split(marker)
    execute = marker + execute
    cases = {}
    for name, focus, message, before, after in (
        ("origin-initialization", ORIGIN_FOCUS, "invariant not satisfied at end of loop body",
         "Gfx942U32OriginV1::EntrySgpr($index as u8)", "Gfx942U32OriginV1::EntrySgpr((($index + 1) % 102) as u8)"),
        ("origin-transitive-source", ORIGIN_FOCUS, "invariant not satisfied at end of loop body",
         "Gfx942U32SourceV1::Sgpr(register) => $origins[register as usize],",
         "Gfx942U32SourceV1::Sgpr(register) => Gfx942U32OriginV1::EntrySgpr(register),"),
        ("origin-destination", ORIGIN_FOCUS, "invariant not satisfied at end of loop body",
         "$origins[$moves[$index].destination() as usize] = origin;",
         "$origins[($moves[$index].destination() as usize + 1) % 102] = origin;"),
        ("origin-terminal-right", ORIGIN_FOCUS, "postcondition not satisfied", "[left, right]", "{ let _ = right; [left, left] }"),
        ("execute-skipped-move", PREFIX_FOCUS, "invariant not satisfied at end of loop body",
         "$moves[$index].execute($state);", "();"),
        ("execute-reordered-moves", PREFIX_FOCUS, "invariant not satisfied at end of loop body",
         "$moves[$index].execute($state);", "$moves[$moves.len() - 1 - $index].execute($state);"),
        ("execute-skipped-add", PREFIX_FOCUS, "postcondition not satisfied",
         "$add.execute($state)", "let _ = $add; Gfx942U32AddResultV1 { value: 0, scc: false }"),
        ("execute-changed-final-scc", PREFIX_FOCUS, "postcondition not satisfied",
         "$add.execute($state)", "let result = $add.execute($state); $state.scc = !result.scc; result"),
    ):
        selected = origins if focus == ORIGIN_FOCUS else execute
        # Only the first (MOV source) occurrence is changed, not terminal resolution.
        expected = 3 if name == "origin-transitive-source" else 1
        need(selected.count(before) == expected, "exact fold mutation site: " + name)
        changed = selected.replace(before, after, 1)
        mutant = changed + execute if focus == ORIGIN_FOCUS else origins + changed
        cases[name] = (mutant, focus, message)
    need(len({value[0] for value in cases.values()}) == 8, "eight distinct fold negatives")
    return cases


INVARIANT_SITES = {
    "origin-initialization": "forall|register: int| 0 <= register < index",
    "origin-transitive-source": "origins@ == origins_after(moves@, index as nat),",
    "origin-destination": "origins@ == origins_after(moves@, index as nat),",
    "execute-skipped-move": "forall|register: int| 0 <= register < 102 ==> #[trigger] state.registers@[register]",
    "execute-reordered-moves": "forall|register: int| 0 <= register < 102 ==> #[trigger] state.registers@[register]",
}


def target_lines(proof, focus):
    macro, body_path = TARGETS[focus]
    lines = proof.read_text().splitlines()
    calls = [i + 1 for i, line in enumerate(lines) if line.strip().startswith(macro + "!(")]
    need(len(calls) == 1, "one exact target macro call")
    call = calls[0]
    first = max(i + 1 for i, line in enumerate(lines[:call - 1])
                if line.strip().startswith("fn " + focus.split("::")[-1] + "("))
    if "::" in focus:
        owner = focus.split("::")[0]
        previous = [line.strip() for line in lines[:first] if line.strip().startswith("impl ")]
        need(previous[-1] == "impl " + owner + " {", "exact qualified target impl")
    balance = 0
    end = None
    for i in range(call - 1, len(lines)):
        balance += lines[i].count("(") - lines[i].count(")")
        if balance == 0:
            end = i + 1
            break
    need(end is not None, "closed shared macro call")
    body = (proof.parent / "../src/gfx942_integer_semantics_v1" / body_path.name).resolve()
    definition = next(i + 1 for i, line in enumerate(body.read_text().splitlines())
                      if line.startswith("macro_rules! " + macro + " {"))
    return lines, first, call, end, macro, body, definition


def macro_expansion(span, proof, call, macro, body, definition):
    # While desugaring adds a nested expansion; never accept an unrelated note.
    for _ in range(4):
        expansion = span.get("expansion")
        if not isinstance(expansion, dict):
            return False
        if (Path(span.get("file_name", "")).resolve() == body
                and expansion.get("macro_decl_name") == macro + "!"
                and Path(expansion["span"]["file_name"]).resolve() == proof
                and expansion["span"]["line_start"] == call
                and Path(expansion["def_site_span"]["file_name"]).resolve() == body
                and expansion["def_site_span"]["line_start"] == definition):
            return True
        if expansion.get("macro_decl_name") not in {"desugaring of `while` loop", "verus_exec_expr!"}:
            return False
        span = expansion["span"]
    return False


def classify(status, stdout, stderr, proof, negative=False, focus=FOCUS,
             logical_message="postcondition not satisfied", invariant=None):
    try:
        data = strict_json(stdout)
        diagnostics = [strict_json(line) for line in stderr.splitlines() if line]
        need(data.get("verus") == VERIFIER, "exact verifier identity")
        result = data["verification-results"]
        negative_verified = 2 if focus == ORIGIN_FOCUS else 1 if focus == PREFIX_FOCUS else 0
        expected = {"encountered-error": negative, "encountered-vir-error": False,
                    "errors": 1 if negative else 0, "verified": negative_verified if negative else VERIFIED_COUNT,
                    "is-verifying-entire-crate": not negative}
        if not negative:
            expected["success"] = True
        need(result == expected and all(type(result[key]) is type(value)
                                       for key, value in expected.items()), "exact proof summary")
        errors = [row for row in diagnostics if row.get("level") == "error"]
        need(all(row.get("level") in {"error", "note"} for row in diagnostics), "no hidden warnings")
        if not negative:
            return status == 0 and not diagnostics
        need(status == 1, "one selected logical failure")
        need(focus in TARGETS, "one known theorem target")
        need(logical_message == "postcondition not satisfied" if invariant is None else
             (focus in {ORIGIN_FOCUS, PREFIX_FOCUS}
              and logical_message == "invariant not satisfied at end of loop body"
              and invariant in INVARIANT_SITES.values()), "declared logical rejection kind")
        logical = [row for row in errors if row.get("message") == logical_message]
        abort = [row for row in errors if row.get("message") == "aborting due to 1 previous error"
                 and row.get("spans") == []]
        need(len(logical) == len(abort) == 1 and len(errors) == 2,
             "postcondition failure, not parse/timeout/overflow/tool failure")
        notes = {
            "verifying root module (selected functions)",
            "function body check: not all errors may have been reported; rerun with a higher value for --multiple-errors to find other potential errors in this function",
            "while loop: not all errors may have been reported; rerun with a higher value for --multiple-errors to find other potential errors in this function",
        }
        need(all(row.get("message") in notes for row in diagnostics if row.get("level") == "note"),
             "known selected-function diagnostics only")
        lines, first, call, end, macro, body, definition = target_lines(proof, focus)
        spans = logical[0].get("spans", [])
        locations = set(range(first + 1, call)) if invariant is None else {
            i + 1 for i in range(call, end) if lines[i].strip() == invariant}
        contract = any(span.get("is_primary") is True
                       and Path(span.get("file_name", "")).resolve() == proof
                       and type(span.get("line_start")) is int
                       and span["line_start"] in locations for span in spans)
        expansion_rows = logical if invariant is None else [
            row for row in diagnostics if row.get("level") == "note"
            and row.get("message", "").startswith("while loop:")]
        expansion = any(macro_expansion(span, proof, call, macro, body, definition)
                        for row in expansion_rows for span in row.get("spans", []))
        return contract and expansion
    except (ValueError, KeyError, TypeError, AttributeError, StopIteration, IndexError, OSError):
        return False


def main():
    need(sys.flags.isolated and sys.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--verus", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--positive-only", action="store_true", help="development, not acceptance")
    args = parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    out, verus = args.output, args.verus
    need(out.is_absolute() and out.resolve() == out and not out.exists()
         and not out.is_relative_to(ROOT), "fresh output outside repository")
    need(verus.is_absolute() and verus.resolve() == verus and verus.is_file()
         and digest(verus.read_bytes()) == VERUS_HASH, "canonical pinned verifier")
    before = source_snapshot()
    owner = types.ModuleType("gfx942_add_process_owner")
    owner.__file__ = str(ROOT / support.OWNER)
    sys.modules[owner.__name__] = owner
    exec(compile(before[str(support.OWNER)], owner.__file__, "exec"), owner.__dict__)
    out.mkdir(parents=True)
    (out / "tmp").mkdir()
    for path, data in before.items():
        target = out / "inputs" / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    save(out / "source-before.json", {path: digest(data) for path, data in before.items()})
    home = Path.home()
    cargo_home = Path(os.environ.get("CARGO_HOME", str(home / ".cargo"))).resolve()
    rustup_home = Path(os.environ.get("RUSTUP_HOME", str(home / ".rustup"))).resolve()
    env = {"HOME": str(home), "PATH": str(cargo_home / "bin") + ":/usr/bin:/bin",
           "LANG": "C.UTF-8", "LC_ALL": "C.UTF-8", "RUSTUP_HOME": str(rustup_home),
           "CARGO_HOME": str(cargo_home),
           "VERUS_Z3_PATH": str(verus.parent / "z3"), "TMPDIR": str(out / "tmp")}
    save(out / "environment.json", env)
    rows = []
    handlers = {number: signal.getsignal(number) for number in owner.SIGNALS}
    for number in handlers:
        signal.signal(number, owner.interrupted)
    closure_command = ["/bin/sh", str(ROOT / support.CLOSURE), str(verus.parent), str(ROOT / support.MANIFEST)]

    def run(name, command, accept, metadata=None):
        need(source_snapshot() == before, "source continuity before " + name)
        status, stdout, stderr = owner.run_owned(command, 130, out / name, env)
        receipt = strict_json((out / name / "record.json").read_text())
        accepted = bool(receipt.get("group_absent") is True and accept(status, stdout, stderr))
        rows.append({"name": name, "status": status, "accepted": accepted, **(metadata or {})})
        print(name + ": " + ("PASS" if accepted else "FAIL"), flush=True)
        need(source_snapshot() == before, "source continuity after " + name)
        need(accepted, "rejected stage " + name)

    def closure(status, stdout, stderr):
        return status == 0 and stdout == "PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n" and not stderr

    def prove(name, changed=None, focus=FOCUS, logical_message="postcondition not satisfied", invariant=None):
        negative = changed is not None
        staged = out / (name + "-source")
        inputs = {path: before[str(path)] for path in (BODY, EXECUTE, MOV_BODY, PREFIX_BODY, PROOF)}
        if changed is not None:
            path, body = changed
            need(path == TARGETS[focus][1], "mutation belongs to exact selected theorem body")
            inputs[path] = body.encode("ascii")
        for path, data in inputs.items():
            target = staged / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
        source = staged / PROOF
        command = ["/usr/bin/timeout", "--foreground", "--signal=TERM", "--kill-after=5", "120",
                   str(verus), "--crate-type", "lib", "--triggers-mode", "silent", "--no-cheating",
                   "--output-json", "--error-format=json", "--no-report-long-running",
                   "--num-threads", "1", "--multiple-errors", "1",
                   *(["--verify-function", "*" + focus, "--verify-root"] if negative else []), str(source)]
        run(name, command, lambda status, stdout, stderr: classify(
            status, stdout, stderr, source, negative, focus, logical_message, invariant),
            {"target": focus, "logical_rejection": logical_message, "invariant": invariant} if negative
            else {"verified_obligations": VERIFIED_COUNT})
        need(all((staged / path).is_file() and not (staged / path).is_symlink()
                 and (staged / path).read_bytes() == data for path, data in inputs.items()),
             "staged proof continuity after " + name)

    error = None
    unchanged = False
    try:
        run("controls", [sys.executable, "-I", "-B", str(ROOT / TEST)],
            lambda status, stdout, stderr: status == 0 and not stdout
            and re.findall(r"^Ran (\d+) tests in [0-9.]+s$", stderr, re.M) == [str(CONTROL_COUNT)]
            and stderr.endswith("\nOK\n"))
        run("release-before", closure_command, closure)
        prove("positive-before")
        if not args.positive_only:
            for path, generator, focus in ((BODY, mutations, FOCUS), (EXECUTE, state_mutations, STATE_FOCUS),
                                           (MOV_BODY, mov_mutations, MOV_FOCUS)):
                for name, body in generator(before[str(path)].decode("ascii")).items():
                    prove("negative-" + name, (path, body), focus)
            for name, (body, focus, message) in fold_mutations(before[str(PREFIX_BODY)].decode("ascii")).items():
                prove("negative-" + name, (PREFIX_BODY, body), focus, message, INVARIANT_SITES.get(name))
            prove("positive-after")
    except BaseException as failure:
        error = type(failure).__name__ + ": " + str(failure)
    finally:
        try:
            run("release-after", closure_command, closure)
            after = source_snapshot()
            save(out / "source-after.json", {path: digest(data) for path, data in after.items()})
            unchanged = after == before
        except BaseException as failure:
            error = (error or "") + "; closing: " + type(failure).__name__ + ": " + str(failure)
        for number, handler in handlers.items():
            signal.signal(number, handler)
    expected = 4 if args.positive_only else 26
    result = dict(accepted=error is None and unchanged and len(rows) == expected
                  and all(row["accepted"] for row in rows),
                  full_campaign=not args.positive_only, scope=SCOPE, stages=rows, error=error,
                  source_unchanged=unchanged, establishes_hardware_isa_conformance=False,
                  grants_application_authority=False, verified_obligations=VERIFIED_COUNT,
                  arithmetic_mutants=3, state_mutants=5, mov_mutants=5, fold_mutants=8,
                  control_tests=CONTROL_COUNT)
    save(out / "result.json", result)
    print(support.json.dumps(result, indent=2))
    return 0 if result["accepted"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
