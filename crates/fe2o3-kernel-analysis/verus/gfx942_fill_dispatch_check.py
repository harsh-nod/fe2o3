#!/usr/bin/env python3
"""Qualify complete shared fill-dispatch composition, not native execution authority."""
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
S = Path("crates/fe2o3-kernel-analysis/src/gfx942_fill_wave_v1")
PROOF = V / "gfx942_fill_dispatch_v1.rs"
BODY = S / "dispatch_body.rs"
RUST = S / "dispatch.rs"
TEST = V / "gfx942_fill_dispatch_test.py"
WAVE = V / "gfx942_fill_wave_check.py"
PINS = {
    WAVE: "41d376298ec25086660d01e40be8c005ac70bf41f4f209f0eb9d7547b746afcd",
    PROOF: "3eef55ff4986b65ad46e4b6530ca6517e76d56a8028936baf76153f05189915e",
    BODY: "8638b7d2860c457bebd681247c318b1aed9d478641c4b0f38200a1a0486fe06d",
    RUST: "60f4ec252490f83fd704931d6690e7551a46953ff7c82bd0a3ef80344df40f10",
}
VERIFIED = 44
CONTROL_COUNT = 11
# Exact logical failures of the reviewed shared-body mutations.
FAILURES = {
    "zero-grid": ("valid_dispatch", 0, [("postcondition not satisfied", (33, 13, 33, 45))]),
    "partial-grid": ("valid_dispatch", 0, [("postcondition not satisfied", (33, 13, 33, 45))]),
    "underlaunch": ("valid_dispatch", 0, [("postcondition not satisfied", (33, 13, 33, 45))]),
    "region-overlap": ("valid_dispatch", 0, [("postcondition not satisfied", (33, 13, 33, 45))]),
    "kernarg-register": ("initialize_entry", 1, [("invariant not satisfied before loop", (56, 13, 56, 65))]),
    "group-register": ("initialize_entry", 1, [("invariant not satisfied before loop", (57, 13, 57, 36))]),
    "partial-exec": ("initialize_entry", 1, [("invariant not satisfied before loop", (57, 38, 57, 65))]),
    "local-id": ("initialize_entry", 1, [("invariant not satisfied at end of loop body", (60, 13, 62, 74))]),
    "missing-last-group": ("execute_group", 0, [("postcondition not satisfied", (106, 9, 106, 57))]),
    "byte-group": ("dispatch_byte_after", 0, [("precondition not satisfied", (212, 13, 212, 49))]),
    "outside-byte": ("dispatch_byte_after", 0, [("postcondition not satisfied", (203, 13, 203, 59))]),
}
TARGET_MACROS = {
    "valid_dispatch": "gfx942_fill_dispatch_valid_body_v1",
    "initialize_entry": "gfx942_fill_entry_body_v1",
    "execute_group": "gfx942_fill_group_body_v1",
    "dispatch_byte_after": "gfx942_fill_dispatch_byte_body_v1",
}
SCOPE = (
    "Shared full64 dispatch validator iff acceptance, descriptor-shaped entry construction, "
    "actual wave composition, unique cross-group output coverage, exact bytes and untouched "
    "complement, and modeled termination. Constant storage and one-group byte queries. "
    "Conditional on reviewed ISA/descriptor/AMDHSA interpretation; not authenticated native "
    "entry values, memory backing, scheduling, visibility, completion, compiler refinement, "
    "multi-GPU launch authority or performance."
)


def need(value, message):
    if not value:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


need(digest((ROOT / WAVE).read_bytes()) == PINS[WAVE], "pinned wave qualification support")
spec = importlib.util.spec_from_file_location("dispatch_wave_support", ROOT / WAVE)
wave = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wave)
support = wave.support
save, strict_json = support.save, support.strict_json


def validate_sources(inputs):
    for path, expected in PINS.items():
        need(digest(inputs[str(path)]) == expected, "reviewed source: " + str(path))
    body = inputs[str(BODY)].decode("ascii")
    need(not set(wave.base.tokens(body)) & {
        "assume", "admit", "external_body", "external_fn_specification", "axiom",
        "unsafe", "cfg", "include", "fn", "mod",
    }, "closed shared bodies without trust escapes")
    need(re.findall(r"macro_rules! (\w+)", body) == list(TARGET_MACROS.values()), "exact shared bodies")


def snapshot():
    result = wave.snapshot()
    for path in [*PINS, TEST, Path(__file__).relative_to(ROOT), S / "dispatch/tests.rs",
                 Path("crates/fe2o3-hsaco/src/kernel_binding/gfx942_initial_registers.rs")]:
        selected = ROOT / path
        need(selected.is_file() and not selected.is_symlink(), "ordinary source: " + str(path))
        result[str(path)] = selected.read_bytes()
    validate_sources(result)
    return result


def mutations(body):
    cases = []
    for name, before, after, target in (
        ("zero-grid", "$input.grid[0] > 0", "true", "valid_dispatch"),
        ("partial-grid", "$input.grid[0] % 64 == 0", "$input.grid[0] % 32 == 0", "valid_dispatch"),
        ("underlaunch", "count <= $input.grid[0] as u64", "count <= u32::MAX as u64", "valid_dispatch"),
        ("region-overlap", "$input.output_base >= $input.kernarg_address + 16",
         "$input.output_base >= $input.kernarg_address", "valid_dispatch"),
        ("kernarg-register", "$state.sgprs[0] = $address as u32;",
         "$state.sgprs[0] = ($address >> 1) as u32;", "initialize_entry"),
        ("group-register", "$state.sgprs[2] = $group;", "$state.sgprs[2] = $group / 2;", "initialize_entry"),
        ("partial-exec", "$state.exec_mask = u64::MAX;", "$state.exec_mask = u64::MAX >> 1;", "initialize_entry"),
        ("local-id", "registers[0] = $lane as u32;", "registers[0] = (($lane + 1) % 64) as u32;", "initialize_entry"),
        ("missing-last-group", "$group >= $input.grid[0] / 64", "$group >= $input.grid[0] / 64 - 1", "execute_group"),
        ("byte-group", "let $group = ($index / 64) as u32;", "let $group = ($index / 32) as u32;", "dispatch_byte_after"),
        ("outside-byte", "                $before\n", "                $before ^ 1\n", "dispatch_byte_after"),
    ):
        need(body.count(before) == 1, "unique mutation site: " + name)
        cases.append((name, body.replace(before, after), target))
    need(len({body for _, body, _ in cases}) == len(cases), "distinct mutants")
    return cases


def target_info(proof, target):
    lines = proof.read_text().splitlines()
    starts = [i + 1 for i, line in enumerate(lines) if line.startswith("fn " + target + "(")]
    need(len(starts) == 1, "one selected theorem")
    start = starts[0]
    end = next((i + 1 for i in range(start, len(lines))
                if re.match(r"(?:fn |proof fn |spec fn |impl )", lines[i])), len(lines) + 1)
    macro = TARGET_MACROS[target]
    calls = [i + 1 for i, line in enumerate(lines) if macro + "!(" in line]
    need(len(calls) == 1 and start <= calls[0] < end, "exact selected shared body")
    body = (proof.parent / "../src/gfx942_fill_wave_v1/dispatch_body.rs").resolve()
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
            expected = {"encountered-error": False, "encountered-vir-error": False,
                        "errors": 0, "verified": VERIFIED, "is-verifying-entire-crate": True, "success": True}
            return status == 0 and not rows and result == expected and all(
                type(result[key]) is type(value) for key, value in expected.items())
        selected, verified, diagnostics = FAILURES[mutant]
        need(target == selected, "exact mutant target")
        expected = {"encountered-error": True, "encountered-vir-error": False,
                    "errors": 1, "verified": verified, "is-verifying-entire-crate": False}
        need(status == 1 and result == expected and all(type(result[key]) is type(value)
             for key, value in expected.items()), "exact selected failure summary")
        errors = [row for row in rows if row.get("level") == "error"]
        need(len(errors) == len(diagnostics) + 1, "exact diagnostic count")
        need(errors[-1].get("message") == "aborting due to " + str(len(diagnostics))
             + " previous error" + ("s" if len(diagnostics) != 1 else "")
             and errors[-1].get("spans") == [], "exact terminal error")
        need(all(row.get("level") in {"error", "note"} for row in rows), "no hidden warnings")
        notes = {"verifying root module (selected functions)",
                 "function body check: not all errors may have been reported; rerun with a higher value for --multiple-errors to find other potential errors in this function",
                 "while loop: not all errors may have been reported; rerun with a higher value for --multiple-errors to find other potential errors in this function"}
        need(all(row.get("message") in notes for row in rows if row.get("level") == "note"), "known notes")
        start, end, call, macro, body, definition = target_info(proof, target)
        actual = []
        for row in errors[:-1]:
            spans = [span for span in row.get("spans", []) if span.get("is_primary") is True]
            need(len(spans) == 1 and Path(spans[0].get("file_name", "")).resolve() == proof, "exact primary source")
            location = tuple(spans[0].get(key) for key in ("line_start", "column_start", "line_end", "column_end"))
            need(start <= location[0] < end, "selected theorem obligation")
            actual.append((row.get("message"), location))
        need(actual == diagnostics, "exact logical failure locations")
        if mutant == "byte-group":
            # Proof-only annotation tokens retain their source location, not a macro expansion.
            need(any(Path(span.get("file_name", "")).resolve() == proof
                     and span.get("is_primary") is False and span.get("label") == "failed precondition"
                     and tuple(span.get(key) for key in ("line_start", "column_start", "line_end", "column_end"))
                         == (86, 37, 86, 63)
                     for row in errors[:-1] for span in row.get("spans", [])), "exact group-range precondition")
        if mutant == "byte-group" or all(message == "invariant not satisfied before loop" for message, _ in diagnostics):
            need(any(row.get("message", "").startswith("function body check:")
                     and any(Path(span.get("file_name", "")).resolve() == proof and span.get("line_start") == start
                             for span in row.get("spans", [])) for row in rows), "exact enclosing function")
        else:
            need(any(wave.base.macro_expansion(span, proof, call, macro, body, definition)
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
         and digest(verus.read_bytes()) == wave.base.VERUS_HASH, "canonical pinned verifier")
    before = snapshot()
    owner = types.ModuleType("dispatch_owned_process")
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
        inputs = {path: before[str(path)] for path in [PROOF, BODY, wave.PROOF, wave.BODY]}
        if changed is not None:
            inputs[BODY] = changed.encode("ascii")
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
        need(all((staged / path).read_bytes() == data for path, data in inputs.items()), "staged source continuity")

    def closure(name):
        run(name, ["/bin/sh", str(ROOT / support.CLOSURE), str(verus.parent), str(ROOT / support.MANIFEST)],
            lambda status, stdout, stderr: status == 0 and not stderr and stdout ==
            "PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n")

    error, unchanged = None, False
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
    result = dict(accepted=error is None and unchanged and len(stages) == 16,
                  source_unchanged=unchanged, stages=stages, error=error, scope=SCOPE,
                  verified_obligations=VERIFIED, logical_mutants=11, control_tests=CONTROL_COUNT,
                  establishes_isa_or_compiler_refinement=False, grants_launch_authority=False)
    save(out / "result.json", result)
    print(support.json.dumps(result, indent=2))
    return 0 if result["accepted"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
