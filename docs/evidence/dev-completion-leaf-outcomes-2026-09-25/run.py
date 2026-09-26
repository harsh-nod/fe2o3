#!/usr/bin/env python3
"""Signed-source development checks for exact finite-projection leaf outcomes."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
BODY = Path("crates/fe2o3-runtime/src/context/completion_reconciliation_body.rs")
PRIOR = Path("docs/evidence/dev-completion-reconciliation-proof-2026-09-25/run.py")
PRIOR_SHA = "9e3a1f4a81f665d9cc9f8479de0072c92e0fa3577d5b7ba1af6893d070624e76"
PROOF_RESULT = {"encountered-error": False, "encountered-vir-error": False,
                "errors": 0, "is-verifying-entire-crate": True, "success": True, "verified": 53}
LOGICAL_ERRORS = {"postcondition not satisfied", "precondition not satisfied", "assertion failed",
                  "invariant not satisfied at end of loop body", "invariant not satisfied before loop"}
MUTANTS = [
    ("skip-success-settlement", BODY,
     "let result = $context.transition_submission_status($id, RuntimeCompletionStatusV1::Succeeded);",
     "let result = Ok::<RuntimeCompletionStatusV1, RuntimeValidationErrorV1>(RuntimeCompletionStatusV1::Pending);",
     "*plan_completion_step_v1*"),
    ("promote-unknown-input", BODY,
     "Some(ContextProducerReadStatusV1::Unknown) => {\n                        let result = $context.transition_submission_status(\n                            $id,\n                            RuntimeCompletionStatusV1::QuiescentWithoutResult,",
     "Some(ContextProducerReadStatusV1::Unknown) => {\n                        let result = $context.transition_submission_status(\n                            $id,\n                            RuntimeCompletionStatusV1::Succeeded,",
     "*plan_completion_step_v1*"),
    ("omit-failure-quarantine", V / "context_completion_reconciliation_effects_v1.rs",
     "if node.settlement_failure != 0 {\n            self.quarantined = true;",
     "if node.settlement_failure != 0 {",
     "*transition_submission_status*"),
    ("reject-valid-custody", V / "context_completion_reconciliation_validation_v1.rs",
     "let invalid = RuntimeValidationErrorV1::InvalidBackendDescription;",
     "let invalid = RuntimeValidationErrorV1::InvalidBackendDescription;\n        if id.local == 19 { return Err(invalid); }",
     "*validate_custody*"),
]


def need(value, message):
    if not value:
        raise ValueError(message)


def load(name, path, expected):
    raw = path.read_bytes()
    need(hashlib.sha256(raw).hexdigest() == expected, "helper identity: " + name)
    module = types.ModuleType(name)
    module.__file__ = str(path)
    sys.modules[name] = module
    exec(compile(raw, str(path), "exec"), module.__dict__)
    return module


def tree(path):
    return {str(p.relative_to(path)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(path.rglob("*")) if p.is_file()}


def logical_negative(status, stdout, stderr, verifier, source_paths):
    if status != 1:
        return False
    try:
        result = json.loads(stdout)
        vr = result["verification-results"]
        diagnostics = [json.loads(line) for line in stderr.splitlines()]
    except (ValueError, KeyError):
        return False
    if (result.get("verus") != verifier or vr.get("encountered-error") is not True
            or vr.get("encountered-vir-error") is not False
            or vr.get("is-verifying-entire-crate") is not False
            or vr.get("success") is True or type(vr.get("errors")) is not int or vr["errors"] <= 0):
        return False
    identified = False
    for diagnostic in diagnostics:
        message = diagnostic.get("message", "")
        if diagnostic.get("level") == "error" and message in LOGICAL_ERRORS:
            spans = diagnostic.get("spans", [])
            if not any(span.get("is_primary") and str(Path(span.get("file_name", "")).resolve()) in source_paths for span in spans):
                return False
            identified = True
        elif diagnostic.get("level") == "error" and re.fullmatch(r"aborting due to \d+ previous errors?", message):
            pass
        elif diagnostic.get("level") == "note" and message == "verifying root module (selected functions)":
            pass
        elif diagnostic.get("level") == "note" and "not all errors may have been reported" in message:
            pass
        else:
            return False
    return identified


def bind_signed_blobs(prior, before):
    # Hash raw Git blob bytes, without index flags or clean/eol filters.
    need(prior.git("rev-parse", "--show-object-format") == b"sha1\n", "expected Git object format")
    entries = prior.git("ls-tree", "-r", "-z", "--full-tree", before["commit"], "--", *prior.INPUTS)
    bound = {}
    for entry in entries.split(b"\0"):
        if not entry:
            continue
        header, raw_path = entry.split(b"\t", 1)
        mode, kind, oid = header.split()
        path = os.fsdecode(raw_path)
        need(mode in (b"100644", b"100755") and kind == b"blob", "ordinary signed blob")
        need(path not in bound and path in before["inputs"], "exact signed input membership")
        source_path = ROOT / path
        need(source_path.is_file() and not source_path.is_symlink(), "ordinary measured input")
        raw = source_path.read_bytes()
        digest = hashlib.sha256(raw).hexdigest()
        need(digest == before["inputs"][path], "signed-binding source continuity")
        need(hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest() == oid.decode(),
             "measured bytes differ from signed blob: " + path)
        bound[path] = dict(git_blob=oid.decode(), sha256=digest)
    need(bound.keys() == before["inputs"].keys(), "complete signed input set")
    return bound


def main():
    need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--verus", type=Path, required=True)
    args = parser.parse_args()
    out = args.output
    need(out.is_absolute() and out.resolve() == out and not out.exists(), "new canonical output")
    need(not out.is_relative_to(ROOT) and not ROOT.is_relative_to(out), "output outside repository")
    need(args.verus.is_absolute() and args.verus.is_file(), "absolute verifier")
    os.chdir(ROOT)
    prior = load("leaf_evidence_helpers", ROOT / PRIOR, PRIOR_SHA)
    prior.INPUTS = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", "crates", "examples", "tests",
                    str(PRIOR), str(Path(__file__).relative_to(ROOT)),
                    str(Path(__file__).with_name("test-run.py").relative_to(ROOT)), "docs/runtime-completion-reconciliation-v1.md"]
    prior.clean_source()
    source = prior.source_tools()
    controller = load("leaf_owned_process", ROOT / prior.CONTROLLER, prior.CONTROLLER_SHA)
    for number in controller.SIGNALS:
        signal.signal(number, controller.interrupted)
    out.mkdir()
    before = dict(commit=prior.git("rev-parse", "HEAD").decode().strip(), inputs=prior.snapshot())
    prior.save(out / "source-before.json", before)
    signer = out / "allowed-signers"
    signer.write_text(prior.SIGNER)
    environment = dict(os.environ, VERUS_Z3_PATH=str(args.verus.parent / "z3"), TMPDIR=str(out))
    prior.save(out / "environment.json", {key: environment.get(key) for key in (
        "PATH", "RUSTUP_TOOLCHAIN", "LD_LIBRARY_PATH", "VERUS_Z3_PATH", "TMPDIR")})
    results = {}

    def phase(name, command, accept, env=environment, bound=130):
        need(before == dict(commit=prior.git("rev-parse", "HEAD").decode().strip(), inputs=prior.snapshot()), "source continuity before " + name)
        status, stdout, stderr = controller.run_owned(command, bound, out / name, env)
        passed = accept(status, stdout, stderr)
        prior.save(out / (name + ".json"), dict(passed=passed, status=status))
        results[name] = dict(passed=passed, status=status)
        print(name + (": PASS" if passed else ": FAIL"), flush=True)
        need(before == dict(commit=prior.git("rev-parse", "HEAD").decode().strip(), inputs=prior.snapshot()), "source continuity after " + name)
        need(passed, "failed phase: " + name)

    def positive(status, stdout, stderr):
        if status != 0 or stderr:
            return False
        result = json.loads(stdout)
        return (json.dumps(result.get("verification-results"), sort_keys=True) == json.dumps(PROOF_RESULT, sort_keys=True)
                and json.dumps(result.get("verus"), sort_keys=True) == json.dumps(prior.VERIFIER, sort_keys=True))

    def proof(root, focus=None):
        return ["/usr/bin/timeout", "--foreground", "--signal=TERM", "--kill-after=5", "120", str(args.verus),
                "--crate-type", "lib", "--triggers-mode", "silent", "--no-cheating", "--output-json", "--error-format=json",
                "--no-report-long-running", "--num-threads", "4",
                *([] if focus is None else ["--verify-function", focus, "--verify-root"]),
                str(root / V / "context_completion_reconciliation_v1.rs")]

    closure = ["/bin/sh", str(ROOT / "examples/row_softmax_v1/verify-verus-closure.sh"), str(args.verus.parent),
               str(ROOT / V / "pins/VERUS_CLOSURE_MANIFEST")]
    closure_ok = lambda s, o, e: s == 0 and not e and o == "PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n"
    source.authenticate(source.GIT)
    prior.signature_tool()
    phase("source-signature", [str(source.GIT), "--no-replace-objects", "--no-pager", "-c", "gpg.format=ssh",
          "-c", "gpg.ssh.program=/usr/bin/ssh-keygen", "-c", "gpg.ssh.allowedSignersFile=" + str(signer),
          "verify-commit", before["commit"]], lambda s, o, e: s == 0 and not o and e == prior.SIGNATURE, source.TOOL_ENV)
    prior.signature_tool()
    prior.save(out / "signed-inputs.json", bind_signed_blobs(prior, before))
    phase("runner-tests", [sys.executable, "-I", "-B", str(Path(__file__).with_name("test-run.py"))],
          lambda s, o, e: s == 0 and not e and o == "PASS: leaf outcome negative classifier (10 groups)\n")
    phase("source-tests", [sys.executable, "-I", "-B", str(ROOT / V / "test-completion-reconciliation-source.py")],
          lambda s, o, e: s == 0 and not e and o == "PASS: completion planner source calibration (8 groups)\n")
    phase("source-body", [sys.executable, "-I", "-B", str(ROOT / V / "check-completion-reconciliation-source.py"),
                          "--output", str(out / "body-comparison")],
          lambda s, o, e: s == 0 and not e and o == "PASS: allowlisted completion planner syntax correspondence\n")
    phase("closure-before", closure, closure_ok)
    phase("proof-before", proof(ROOT), positive)
    files = sorted((ROOT / V).glob("context_completion_reconciliation*.rs")) + [ROOT / BODY]
    relocated = out / "relocated-source"
    for path in files:
        destination = relocated / path.relative_to(ROOT)
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, destination)
    expected = {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in files}
    need(all(before["inputs"].get(path) == digest for path, digest in expected.items()), "relocation bound to signed source")
    need(tree(relocated) == expected, "exact relocated source")
    prior.save(out / "relocated-inputs.json", expected)
    phase("relocated-proof", proof(relocated), positive)
    need(tree(relocated) == expected, "unchanged relocated source")
    for name, path, old, new, focus in MUTANTS:
        mutated = out / (name + "-source")
        shutil.copytree(relocated, mutated)
        raw = (mutated / path).read_text()
        need(raw.count(old) == 1, "unique mutation site: " + name)
        (mutated / path).write_text(raw.replace(old, new))
        measured = tree(mutated)
        need(measured.keys() == expected.keys(), "exact mutant input set")
        need({p for p in expected if measured[p] != expected[p]} == {str(path)}, "one changed input")
        prior.save(out / (name + "-mutation.json"), dict(path=str(path), old=old, new=new, inputs=measured))
        source_paths = {str((mutated / path).resolve()) for path in expected}
        phase(name, proof(mutated, focus), lambda s, o, e: logical_negative(s, o, e, prior.VERIFIER, source_paths))
        need(tree(mutated) == measured, "unchanged mutant: " + name)
    phase("proof-after", proof(ROOT), positive)
    phase("closure-after", closure, closure_ok)
    phase("format", ["cargo", "fmt", "--all", "--check"], lambda s, o, e: s == 0 and not o and not e)
    prior.clean_source()
    after = dict(commit=prior.git("rev-parse", "HEAD").decode().strip(), inputs=prior.snapshot())
    need(before == after, "closing source continuity")
    prior.save(out / "source-after.json", after)
    prior.save(out / "results.json", results)


if __name__ == "__main__":
    main()
