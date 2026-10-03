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
RUST = SRC / "gfx942_integer_semantics_v1.rs"
PROOF = V / "gfx942_add_u32_v1.rs"
TEST = V / "gfx942_add_u32_test.py"
WRAPPER = V / "run-gfx942-add-u32.sh"
SUPPORT = Path("crates/fe2o3-runtime-model/verus/compute_xgmi_packet_plan_check.py")
SUPPORT_HASH = "61074f6cdbfde1df24bff3d1e56e04398f2850dd00518c233d847f13a29e0db4"
VERUS_HASH = "d97501a883931d1d173b1bf4b6cf4d973f16d105dbcb468e177b52b2331612d2"
FOCUS = "gfx942_add_u32_v1"
MACRO = "gfx942_add_u32_body_v1"
STATE_FOCUS = "execute"
STATE_MACRO = "gfx942_s_add_u32_execute_body_v1"
# Tokens of the reviewed declarations, validity preconditions, specifications,
# postconditions, and forwarding bodies. This is not a hash-based proof claim.
PROOF_TOKENS_SHA = "26b196a8b8963286c8d8c8f5b9961b90cfe5c9c88022a102e6bf03433a07987a"
CONTROL_COUNT = 12
VERIFIED_COUNT = 4  # Constant initializer, derived Clone, arithmetic, execute.
SCOPE = (
    "Shared executable u32 add/carry and 102-SGPR plus SCC transition, conditional on valid "
    "instruction indices: old-input aliasing, modulo value/carry, returned result and register frame. "
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


def validate_sources(sources):
    for path, expected in {SUPPORT: SUPPORT_HASH, **support.PINS}.items():
        need(digest(sources[str(path)]) == expected, "pinned support: " + str(path))
    rust, body, execute, proof = (tokens(sources[str(path)].decode("ascii"))
                                  for path in (RUST, BODY, EXECUTE, PROOF))
    schemas(rust)
    schemas(proof)
    includes(rust, ["gfx942_integer_semantics_v1/add_u32_body.rs",
                    "gfx942_integer_semantics_v1/execute_body.rs"])
    includes(proof, ["../src/gfx942_integer_semantics_v1/add_u32_body.rs",
                     "../src/gfx942_integer_semantics_v1/execute_body.rs"])
    shared_body(body, MACRO)
    shared_body(execute, STATE_MACRO)
    need("macro_rules" not in rust and "macro_rules" not in proof, "no shadow shared macros")
    need(rust.count("cfg") == 1 and "cfg_attr" not in rust
         and rust[-10:] == tokens("#[cfg(test)] mod tests;"), "only ordinary test-module cfg")
    production = one_block(rust, "pub fn gfx942_add_u32_v1(a: u32, b: u32) -> Gfx942U32AddResultV1")[0]
    need(production == tokens(MACRO + "!(a, b)"), "actual arithmetic forwarding")
    implementation = one_block(rust, "impl Gfx942SAddU32V1")[0]
    production = one_block(implementation, "pub fn execute(&self, state: &mut Gfx942ScalarIntegerStateV1) -> Gfx942U32AddResultV1")[0]
    need(production == tokens(STATE_MACRO + "!(self, state)"), "actual state forwarding")
    for name, macro, arguments in ((FOCUS, MACRO, "a, b"), (STATE_FOCUS, STATE_MACRO, "self, state")):
        need(len(positions(rust, ["fn", name])) == 1 and rust.count(macro) == 1,
             "unique production function and macro use: " + name)
        need(len(positions(proof, ["fn", name])) == 1 and proof.count(macro) == 1
             and len(positions(proof, tokens("{ " + macro + "!(" + arguments + ") }"))) == 1,
             "actual proof forwarding: " + name)
    need(digest(support.json.dumps(proof, separators=(",", ":")).encode("ascii")) == PROOF_TOKENS_SHA,
         "reviewed theorem contracts and executable forwarding")


def source_snapshot():
    paths = [BODY, EXECUTE, RUST, PROOF, TEST, WRAPPER, Path(__file__).relative_to(ROOT), SUPPORT, *support.PINS]
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


def classify(status, stdout, stderr, proof, negative=False, focus=FOCUS):
    try:
        data = strict_json(stdout)
        diagnostics = [strict_json(line) for line in stderr.splitlines() if line]
        need(data.get("verus") == VERIFIER, "exact verifier identity")
        result = data["verification-results"]
        expected = {"encountered-error": negative, "encountered-vir-error": False,
                    "errors": 1 if negative else 0, "verified": 0 if negative else VERIFIED_COUNT,
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
        need(focus in {FOCUS, STATE_FOCUS}, "one known theorem target")
        logical = [row for row in errors if row.get("message") == "postcondition not satisfied"]
        abort = [row for row in errors if row.get("message") == "aborting due to 1 previous error"
                 and row.get("spans") == []]
        need(len(logical) == len(abort) == 1 and len(errors) == 2,
             "postcondition failure, not parse/timeout/overflow/tool failure")
        notes = {
            "verifying root module (selected functions)",
            "function body check: not all errors may have been reported; rerun with a higher value for --multiple-errors to find other potential errors in this function",
        }
        need(all(row.get("message") in notes for row in diagnostics if row.get("level") == "note"),
             "known selected-function diagnostics only")
        lines = proof.read_text().splitlines()
        macro, body_path = (MACRO, BODY) if focus == FOCUS else (STATE_MACRO, EXECUTE)
        first = next(i + 1 for i, line in enumerate(lines) if line.strip().startswith("fn " + focus + "("))
        call = next(i + 1 for i, line in enumerate(lines) if line.strip().startswith(macro + "!("))
        body = (proof.parent / "../src/gfx942_integer_semantics_v1" / body_path.name).resolve()
        definition = next(i + 1 for i, line in enumerate(body.read_text().splitlines())
                          if line.startswith("macro_rules! " + macro + " {"))
        spans = logical[0].get("spans", [])
        contract = any(span.get("is_primary") is True
                       and Path(span.get("file_name", "")).resolve() == proof
                       and first < span.get("line_start", 0) < call for span in spans)
        expansion = any(
            Path(span.get("file_name", "")).resolve() == body
            and span.get("expansion", {}).get("macro_decl_name") == macro + "!"
            and Path(span["expansion"]["span"]["file_name"]).resolve() == proof
            and span["expansion"]["span"]["line_start"] == call
            and Path(span["expansion"]["def_site_span"]["file_name"]).resolve() == body
            and span["expansion"]["def_site_span"]["line_start"] == definition
            for span in spans if isinstance(span.get("expansion"), dict))
        return contract and expansion
    except (ValueError, KeyError, TypeError, AttributeError, StopIteration, OSError):
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

    def run(name, command, accept):
        need(source_snapshot() == before, "source continuity before " + name)
        status, stdout, stderr = owner.run_owned(command, 130, out / name, env)
        receipt = strict_json((out / name / "record.json").read_text())
        accepted = bool(receipt.get("group_absent") is True and accept(status, stdout, stderr))
        rows.append({"name": name, "status": status, "accepted": accepted})
        print(name + ": " + ("PASS" if accepted else "FAIL"), flush=True)
        need(source_snapshot() == before, "source continuity after " + name)
        need(accepted, "rejected stage " + name)

    def closure(status, stdout, stderr):
        return status == 0 and stdout == "PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n" and not stderr

    def prove(name, body, execute, negative=False, focus=FOCUS):
        staged = out / (name + "-source")
        inputs = {BODY: body.encode("ascii"), EXECUTE: execute.encode("ascii"), PROOF: before[str(PROOF)]}
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
        run(name, command, lambda status, stdout, stderr: classify(status, stdout, stderr, source, negative, focus))
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
        original = before[str(BODY)].decode("ascii")
        execute = before[str(EXECUTE)].decode("ascii")
        prove("positive-before", original, execute)
        if not args.positive_only:
            for name, body in mutations(original).items():
                prove("negative-" + name, body, execute, True)
            for name, body in state_mutations(execute).items():
                prove("negative-" + name, original, body, True, STATE_FOCUS)
            prove("positive-after", original, execute)
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
    expected = 4 if args.positive_only else 13
    result = dict(accepted=error is None and unchanged and len(rows) == expected
                  and all(row["accepted"] for row in rows),
                  full_campaign=not args.positive_only, scope=SCOPE, stages=rows, error=error,
                  source_unchanged=unchanged, establishes_hardware_isa_conformance=False,
                  grants_application_authority=False, verified_obligations=VERIFIED_COUNT,
                  arithmetic_mutants=3, state_mutants=5, control_tests=CONTROL_COUNT)
    save(out / "result.json", result)
    print(support.json.dumps(result, indent=2))
    return 0 if result["accepted"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
