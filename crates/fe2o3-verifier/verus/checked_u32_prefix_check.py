#!/usr/bin/env python3
"""Qualify the shared prefix fold, not the MIR/KIR adapters or launch authority."""

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
ADAPTER = FOLD.parent.with_suffix(".rs")
TEST = PROOF.with_name("checked_u32_prefix_test.py")
FOLD_SHA = "b987601f3c5008cc6333de06d6922ef79e07eea8e34871b68914144d4541cd22"
PROOF_SHA = "664b99765a754c6c9ca0e322fb194459477e7bd3dd7fe4fa6b7a4c82818b36b9"
MACRO = "checked_u32_prefix_fold_body_v1"
INVARIANT = "symbolic_after(before, steps@, index as nat) == Some(state@),"
SCOPE = ("Successful shared origin fold refines an independent concrete u32 fold for every "
         "valid common argument vector; equal initialized terminal origins imply equal values. "
         "Shared checked add proves modulo-2^32 value and overflow. No normalization-adapter, "
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


def validate(sources):
    base.validate_sources(sources)
    for path, expected in ((HELPER, HELPER_SHA), (FOLD, FOLD_SHA), (PROOF, PROOF_SHA)):
        need(digest(sources[str(path)]) == expected, "reviewed forwarding/contract: " + str(path))
    base.shared_body(base.tokens(sources[str(BODY)].decode("ascii")), MACRO)


def snapshot():
    sources = base.source_snapshot()
    for path in (HELPER, FOLD, BODY, ADAPTER, PROOF, TEST,
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


def locations(proof):
    lines = proof.read_text().splitlines()
    call = next(i + 1 for i, line in enumerate(lines) if line.strip().startswith(MACRO + "!("))
    contract = next(i + 1 for i, line in enumerate(lines) if "ensures accepted ==>" in line)
    invariant = next(i + 1 for i, line in enumerate(lines) if line.strip() == INVARIANT)
    body = (proof.parent / "../src/gfx942_local_checked_u32_add_v1/source_prefix/fold_body.rs").resolve()
    definition = next(i + 1 for i, line in enumerate(body.read_text().splitlines())
                      if line.startswith("macro_rules! " + MACRO + " {"))
    return call, contract, invariant, body, definition


def classify(status, stdout, stderr, proof, negative=False, invariant=False):
    try:
        data = strict_json(stdout)
        rows = [strict_json(line) for line in stderr.splitlines() if line]
        expected = {"encountered-error": negative, "encountered-vir-error": False,
                    "errors": 1 if negative else 0, "verified": 1 if negative else 11,
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
        notes = {"verifying root module (selected functions)",
                 "function body check: not all errors may have been reported; rerun with a higher value for --multiple-errors to find other potential errors in this function",
                 "while loop: not all errors may have been reported; rerun with a higher value for --multiple-errors to find other potential errors in this function"}
        need(all(row.get("message") in notes for row in rows if row["level"] == "note"), "known notes only")
        call, contract, loop, body, definition = locations(proof)
        need(any(span.get("is_primary") is True and Path(span.get("file_name", "")).resolve() == proof
                 and type(span.get("line_start")) is int and span["line_start"] == (loop if invariant else contract)
                 for span in logical[0].get("spans", [])), "exact fold contract span")
        expansions = [row for row in rows if row["level"] == "note" and row.get("message", "").startswith("while loop:")] if invariant else logical
        return any(base.macro_expansion(span, proof, call, MACRO, body, definition)
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

    def prove(name, changed=None, invariant=False):
        staged = out / (name + "-source")
        inputs = {path: before[str(path)] for path in (PROOF, BODY, base.BODY)}
        if changed is not None:
            inputs[BODY] = changed.encode("ascii")
        for path, data in inputs.items():
            target = staged / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
        proof = staged / PROOF
        run(name, ["/usr/bin/timeout", "--foreground", "--signal=TERM", "--kill-after=5", "120", str(verus),
                   "--crate-type", "lib", "--triggers-mode", "silent", "--no-cheating", "--output-json",
                   "--error-format=json", "--no-report-long-running", "--num-threads", "1", "--multiple-errors", "1",
                   *(["--verify-function", "*fold", "--verify-root"] if changed is not None else []), str(proof)],
            lambda status, stdout, stderr: classify(status, stdout, stderr, proof, changed is not None, invariant))
        need(all((staged / path).is_file() and not (staged / path).is_symlink()
                 and (staged / path).read_bytes() == data for path, data in inputs.items()), "staged continuity")

    error = None
    unchanged = False
    try:
        run("controls", [sys.executable, "-I", "-B", str(ROOT / TEST)],
            lambda status, stdout, stderr: status == 0 and not stdout and "\nRan 6 tests in " in stderr and stderr.endswith("\nOK\n"))
        closure("release-before")
        prove("positive-before")
        for name, (changed, invariant) in mutants(before[str(BODY)].decode("ascii")).items():
            prove("negative-" + name, changed, invariant)
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
    accepted = error is None and unchanged and len(rows) == 13 and all(row["accepted"] for row in rows)
    save(out / "result.json", dict(accepted=accepted, source_unchanged=unchanged, scope=SCOPE,
         verified_obligations=11, logical_mutants=8, controls=6, stages=rows, error=error,
         grants_application_authority=False, proves_normalization_adapters=False))
    return 0 if accepted else 1


if __name__ == "__main__":
    raise SystemExit(main())
