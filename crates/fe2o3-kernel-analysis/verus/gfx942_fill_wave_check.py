#!/usr/bin/env python3
"""Qualify the shared closed fill-wave projection, not native launch authority."""

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
S = Path("crates/fe2o3-kernel-analysis/src")
RUST = S / "gfx942_fill_wave_v1.rs"
BODY = S / "gfx942_fill_wave_v1/body.rs"
PROOF = V / "gfx942_fill_wave_v1.rs"
TEST = V / "gfx942_fill_wave_test.py"
SUPPORT = V / "gfx942_add_u32_check.py"
SUPPORT_SHA = "a0f1ebfcbb1467c323658450e75811de861efb1d317a716f7b43014fb9207950"
# Bind reviewed declarations, preconditions and forwarding; these hashes are not proofs.
PROOF_SHA = "8bd4a3a836a2aab67f0b985491bbd6568bb2cf1bf8e07c7a4ce1e8aecff920b9"
RUST_SHA = "4134d5f521c41776047a30eef18595b273c7775cd3f988ada7a20065614990c8"
VERIFIED = 30
BODY_SHA = "58b80f2ce605d190ad94848c59b8e87496f3b0c64d47ca4115472bece2fbdf60"
CONTROL_COUNT = 14
FAILURES = {
    "kernarg-endian": ("postcondition not satisfied", [(38, 13, 38, 44)]),
    "kernarg-field": ("postcondition not satisfied", [(48, 13, 48, 42)]),
    "index-and": ("postcondition not satisfied", [(52, 13, 52, 40)]),
    "index-high-word": ("postcondition not satisfied", [(52, 42, 52, 60)]),
    "load-base-clobber": ("postcondition not satisfied", [(181, 9, 181, 81)]),
    "group-shift": ("invariant not satisfied before loop", [(207, 13, 207, 68)]),
    "inactive-vector-update": ("invariant not satisfied at end of loop body", [(209, 13, 210, 77)]),
    "inclusive-bound": ("assertion failed", [(263, 20, 265, 41)]),
    "dropped-exec": ("assertion failed", [(263, 20, 265, 41)]),
    "inverted-branch": ("postcondition not satisfied", [(183, 9, 183, 93)]),
    "address-scale": ("invariant not satisfied at end of loop body", [(228, 13, 229, 80), (226, 13, 227, 93)]),
    "pointer-truncation": ("invariant not satisfied at end of loop body", [(228, 13, 229, 80), (226, 13, 227, 93)]),
    "store-high-word": ("invariant not satisfied at end of loop body", [(228, 13, 229, 80)]),
    "wrong-terminal": ("postcondition not satisfied", [(182, 9, 182, 35)]),
    "extra-byte": ("invariant not satisfied at end of loop body", [(318, 17, 318, 76)]),
    "wrong-byte": ("invariant not satisfied at end of loop body", [(318, 17, 318, 76)]),
}
HELPER_TARGETS = {"kernarg-endian": "word", "kernarg-field": "decode_kernarg",
                  "index-and": "index_words", "index-high-word": "index_words",
                  "extra-byte": "Gfx942FillWaveExecutionV1::byte_after",
                  "wrong-byte": "Gfx942FillWaveExecutionV1::byte_after"}
TARGET_MACROS = {"word": "gfx942_fill_word_body_v1", "decode_kernarg": "gfx942_fill_decode_body_v1",
                 "index_words": "gfx942_fill_index_body_v1", "execute_wave": "gfx942_fill_wave_body_v1",
                 "Gfx942FillWaveExecutionV1::byte_after": "gfx942_fill_byte_body_v1"}
SCOPE = (
    "Shared 14-instruction closed wave projection: actual 16-byte kernarg decode, "
    "64-lane masks, register values, bounded store addresses/values, sparse byte frame "
    "and both terminating paths. Conditional on the stated projected entry and memory "
    "premises. No proof of ISA interpretation, HSACO inspection, Rust/LLVM compilation, "
    "native memory backing/visibility/completion, grid coverage, multi-GPU authority or performance."
)


def need(value, message):
    if not value:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


need(digest((ROOT / SUPPORT).read_bytes()) == SUPPORT_SHA, "pinned proof support")
spec = importlib.util.spec_from_file_location("fill_proof_support", ROOT / SUPPORT)
base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)
support = base.support
save, strict_json = support.save, support.strict_json


def validate_sources(inputs):
    need(digest(inputs[str(PROOF)]) == PROOF_SHA, "reviewed complete proof and nonvacuous premises")
    need(digest(inputs[str(RUST)]) == RUST_SHA, "reviewed Rust schemas and exact body forwarding")
    need(digest(inputs[str(BODY)]) == BODY_SHA, "reviewed single-arm bodies and annotation positions")
    body = inputs[str(BODY)].decode("ascii")
    tokens = base.tokens(body)
    need(not set(tokens) & {"assume", "admit", "external_body", "external_fn_specification",
                           "axiom", "unsafe", "cfg", "include", "fn", "mod"},
         "closed body without trust escapes or alternate builds")
    names = re.findall(r"macro_rules! (\w+)", body)
    need(names == ["gfx942_fill_pair_body_v1", "gfx942_fill_decode_body_v1",
                   "gfx942_fill_word_body_v1", "gfx942_fill_index_body_v1",
                   "gfx942_fill_wave_body_v1", "gfx942_fill_byte_body_v1"], "exact shared bodies")


def snapshot():
    paths = [RUST, BODY, PROOF, TEST, SUPPORT, base.SUPPORT, *support.PINS,
             Path(__file__).relative_to(ROOT), S / "gfx942_fill_wave_v1/tests.rs",
             Path("crates/fe2o3-hsaco/tests/fixtures/rust-fill-write-only-gfx942/kernel.hsaco")]
    result = {}
    for path in paths:
        selected = ROOT / path
        need(selected.is_file() and not selected.is_symlink(), "ordinary source: " + str(path))
        result[str(path)] = selected.read_bytes()
    need(digest(result[str(SUPPORT)]) == SUPPORT_SHA, "support continuity")
    need(digest(result[str(base.SUPPORT)]) == base.SUPPORT_HASH, "process support continuity")
    for path, expected in support.PINS.items():
        need(digest(result[str(path)]) == expected, "pinned closure: " + str(path))
    validate_sources(result)
    return result


def mutations(body):
    cases = []
    for name, before, after, target in (
        ("kernarg-endian", "($a as u32) | (($b as u32) << 8)", "($b as u32) | (($a as u32) << 8)", "word"),
        ("kernarg-field", "word($bytes[8], $bytes[9], $bytes[10], $bytes[11])",
         "word($bytes[0], $bytes[1], $bytes[2], $bytes[3])", "decode_kernarg"),
        ("index-and", "[($low | $local), $high]", "[($low & $local), $high]", "index_words"),
        ("index-high-word", "[($low | $local), $high]", "[($low | $local), 0]", "index_words"),
        ("load-base-clobber", "let read_address = pair($state.sgprs[0], $state.sgprs[1]);",
         "let read_address = pair($state.sgprs[2], 0);", "execute_wave"),
        ("group-shift", "let shifted = ($state.sgprs[2] as u64) << 6;",
         "let shifted = ($state.sgprs[2] as u64) << 5;", "execute_wave"),
        ("inactive-vector-update", "if $incoming & (1u64 << $lane) != 0 {", "if true {", "execute_wave"),
        ("inclusive-bound", "$length > index", "$length >= index", "execute_wave"),
        ("dropped-exec", "$incoming & bit != 0 && $length > index", "$length > index", "execute_wave"),
        ("inverted-branch", "if $state.exec_mask == 0 {", "if $state.exec_mask != 0 {", "execute_wave"),
        ("address-scale", "(index << 2).wrapping_add(base)", "(index << 1).wrapping_add(base)", "execute_wave"),
        ("pointer-truncation", "let base = pair(registers[2], registers[3]);", "let base = pair(registers[2], 0);", "execute_wave"),
        ("store-high-word", "value: registers[0],", "value: registers[1],", "execute_wave"),
        ("wrong-terminal", "terminal_pc: 0x44,", "terminal_pc: 0x40,", "execute_wave"),
        ("extra-byte", "$address - store.address < 4", "$address - store.address < 5", "Gfx942FillWaveExecutionV1::byte_after"),
        ("wrong-byte", "$byte = (store.value >> (8 * ($address - store.address))) as u8;",
         "$byte = ((store.value >> (8 * ($address - store.address))) as u8) ^ 1;", "Gfx942FillWaveExecutionV1::byte_after"),
    ):
        need(body.count(before) == 1, "unique mutation site: " + name)
        changed = body.replace(before, after)
        if name == "extra-byte":
            # Keep the mutant's shift defined; require a frame failure, not a safety failure.
            changed = changed.replace("store.value >> (8 * ($address - store.address))",
                                      "store.value >> ((8 * ($address - store.address)) % 32)")
        cases.append((name, changed, target))
    need(len({body for _, body, _ in cases}) == len(cases), "distinct mutants")
    return cases


def target_info(proof, target):
    lines = proof.read_text().splitlines()
    name = target.split("::")[-1]
    starts = [i + 1 for i, line in enumerate(lines) if line.lstrip().startswith("fn " + name + "(")]
    need(len(starts) == 1, "one selected theorem")
    start = starts[0]
    end = next((i + 1 for i in range(start, len(lines))
                if re.match(r"\s*(?:fn |proof fn |spec fn |impl )", lines[i])), len(lines) + 1)
    macro = TARGET_MACROS[target]
    calls = [i + 1 for i, line in enumerate(lines) if macro + "!(" in line]
    need(len(calls) == 1 and start <= calls[0] < end, "exact shared macro call in selected theorem")
    body = (proof.parent / "../src/gfx942_fill_wave_v1/body.rs").resolve()
    definitions = [i + 1 for i, line in enumerate(body.read_text().splitlines())
                   if line.startswith("macro_rules! " + macro + " {")]
    need(len(definitions) == 1, "one shared body definition")
    return start, end, calls[0], macro, body, definitions[0]


def classify(status, stdout, stderr, proof, target=None, mutant=None):
    try:
        data = strict_json(stdout)
        rows = [strict_json(line) for line in stderr.splitlines() if line]
        need(data.get("verus") == support.VERIFIER, "exact verifier identity")
        result = data["verification-results"]
        if target is None:
            expected = {
                "encountered-error": False, "encountered-vir-error": False,
                "errors": 0, "verified": VERIFIED, "is-verifying-entire-crate": True, "success": True,
            }
            return status == 0 and not rows and result == expected and all(
                type(result[key]) is type(value) for key, value in expected.items())
        need(mutant in FAILURES and target == HELPER_TARGETS.get(mutant, "execute_wave"), "exact mutant target")
        expected = {"encountered-error": True, "encountered-vir-error": False,
                    "verified": 3 if target == "execute_wave" else 1 if "::" in target else 0,
                    "errors": 1, "is-verifying-entire-crate": False}
        need(status == 1 and result == expected and all(type(result[key]) is type(value)
             for key, value in expected.items()), "exact selected failure summary")
        message, locations = FAILURES[mutant]
        errors = [row for row in rows if row.get("level") == "error"]
        failures = errors[:-1]
        # One failed verification obligation can report two violated invariants.
        need(len(failures) == len(locations) and all(row.get("message") == message for row in failures)
             and errors[-1].get("message") == "aborting due to " + str(len(failures))
                 + " previous error" + ("s" if len(failures) != 1 else "")
             and errors[-1].get("spans") == [], "logical failure, not type/parse/safety/timeout")
        need(all(row.get("level") in {"error", "note"} for row in rows), "no hidden warnings")
        notes = {"verifying root module (selected functions)",
                 "function body check: not all errors may have been reported; rerun with a higher value for --multiple-errors to find other potential errors in this function",
                 "while loop: not all errors may have been reported; rerun with a higher value for --multiple-errors to find other potential errors in this function"}
        need(all(row.get("message") in notes for row in rows if row.get("level") == "note"), "known notes only")
        start, end, call, macro, body, definition = target_info(proof, target)
        actual = []
        for row in failures:
            spans = [span for span in row.get("spans", []) if span.get("is_primary") is True]
            need(len(spans) == 1 and Path(spans[0].get("file_name", "")).resolve() == proof, "one exact proof span")
            actual.append(tuple(spans[0].get(key) for key in ("line_start", "column_start", "line_end", "column_end")))
        need(sorted(actual) == sorted(locations) and all(start <= row[0] < end for row in actual), "exact failed obligations")
        if mutant == "group-shift":
            # Verus emits no macro expansion for a before-loop invariant failure.
            need(any(row.get("message", "").startswith("function body check:")
                     and any(Path(span.get("file_name", "")).resolve() == proof and span.get("line_start") == start
                             for span in row.get("spans", [])) for row in rows), "exact enclosing function")
        else:
            need(any(base.macro_expansion(span, proof, call, macro, body, definition)
                     for row in rows for span in row.get("spans", [])), "exact shared body expansion")
        return True
    except (ValueError, KeyError, TypeError, AttributeError, OSError, IndexError):
        return False


def main():
    need(sys.flags.isolated and sys.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--verus", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    out, verus = args.output, args.verus
    need(out.is_absolute() and out.resolve() == out and not out.exists()
         and not out.is_relative_to(ROOT), "fresh output outside repository")
    need(verus.is_absolute() and verus.resolve() == verus
         and digest(verus.read_bytes()) == base.VERUS_HASH, "canonical pinned verifier")
    before = snapshot()
    owner = types.ModuleType("fill_owned_process")
    owner.__file__ = str(ROOT / support.OWNER)
    sys.modules[owner.__name__] = owner
    exec(compile(before[str(support.OWNER)], owner.__file__, "exec"), owner.__dict__)
    out.mkdir(parents=True)
    (out / "tmp").mkdir()
    for path, data in before.items():
        destination = out / "inputs" / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(data)
    save(out / "source-before.json", {path: digest(data) for path, data in before.items()})
    home = Path.home()
    cargo = Path(os.environ.get("CARGO_HOME", str(home / ".cargo"))).resolve()
    env = {"HOME": str(home), "PATH": str(cargo / "bin") + ":/usr/bin:/bin",
           "LANG": "C.UTF-8", "LC_ALL": "C.UTF-8", "CARGO_HOME": str(cargo),
           "RUSTUP_HOME": str(Path(os.environ.get("RUSTUP_HOME", str(home / ".rustup"))).resolve()),
           "VERUS_Z3_PATH": str(verus.parent / "z3"), "TMPDIR": str(out / "tmp")}
    save(out / "environment.json", env)
    stages = []
    handlers = {number: signal.getsignal(number) for number in owner.SIGNALS}
    for number in handlers:
        signal.signal(number, owner.interrupted)

    def run(name, command, accept):
        need(snapshot() == before, "source continuity before " + name)
        status, stdout, stderr = owner.run_owned(command, 130, out / name, env)
        receipt = strict_json((out / name / "record.json").read_text())
        accepted = receipt.get("group_absent") is True and accept(status, stdout, stderr)
        stages.append(dict(name=name, status=status, accepted=bool(accepted)))
        print(name + ": " + ("PASS" if accepted else "FAIL"), flush=True)
        need(snapshot() == before, "source continuity after " + name)
        need(accepted, "rejected stage " + name)

    def prove(name, changed=None, target=None):
        staged = out / (name + "-source")
        inputs = {PROOF: before[str(PROOF)], BODY: before[str(BODY)] if changed is None else changed.encode("ascii")}
        for path, data in inputs.items():
            destination = staged / path
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(data)
        source = staged / PROOF
        run(name, ["/usr/bin/timeout", "--foreground", "--kill-after=5", "120", str(verus),
                   "--crate-type", "lib", "--triggers-mode", "silent", "--no-cheating", "--output-json",
                   "--error-format=json", "--no-report-long-running", "--num-threads", "1", "--multiple-errors", "1",
                   *(["--verify-function", "*" + target, "--verify-root", "--no-auto-recommends-check"] if target else []), str(source)],
            lambda status, stdout, stderr: classify(status, stdout, stderr, source, target,
                                                    name.removeprefix("negative-") if target else None))
        need(all((staged / path).read_bytes() == data for path, data in inputs.items()), "staged proof continuity")

    def closure(name):
        run(name, ["/bin/sh", str(ROOT / support.CLOSURE), str(verus.parent), str(ROOT / support.MANIFEST)],
            lambda status, stdout, stderr: status == 0 and not stderr and stdout ==
            "PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n")

    error = None
    unchanged = False
    try:
        run("controls", [sys.executable, "-I", "-B", str(ROOT / TEST)],
            lambda status, stdout, stderr: status == 0 and not stdout
            and re.findall(r"^Ran (\d+) tests in [0-9.]+s$", stderr, re.M) == [str(CONTROL_COUNT)]
            and stderr.endswith("\nOK\n"))
        closure("release-before")
        prove("positive-before")
        for name, body, target in mutations(before[str(BODY)].decode("ascii")):
            prove("negative-" + name, body, target)
        prove("positive-after")
    except BaseException as failure:
        error = type(failure).__name__ + ": " + str(failure)
    finally:
        try:
            closure("release-after")
            after = snapshot()
            save(out / "source-after.json", {path: digest(data) for path, data in after.items()})
            unchanged = after == before
        except BaseException as failure:
            error = (error or "") + "; closing: " + type(failure).__name__ + ": " + str(failure)
        for number, handler in handlers.items():
            signal.signal(number, handler)
    result = dict(accepted=error is None and unchanged and len(stages) == 21,
                  source_unchanged=unchanged, stages=stages, error=error, scope=SCOPE,
                  verified_obligations=VERIFIED, logical_mutants=16, control_tests=CONTROL_COUNT,
                  establishes_isa_or_compiler_refinement=False, grants_launch_authority=False)
    save(out / "result.json", result)
    print(support.json.dumps(result, indent=2))
    return 0 if result["accepted"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
