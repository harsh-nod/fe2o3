#!/usr/bin/env python3
"""Qualify shared quiescence control, not physical completion or ownership refinement."""

import argparse
import hashlib
import os
from pathlib import Path
import shutil
import signal
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
CLASSIFIER = V / "check-completion-reconciliation-campaign.py"
CLASSIFIER_SHA = "27fe721fdacd41acaf76c89c007137548fc1f674e1602b46ecde014aeacc5d84"
BODY = Path("crates/fe2o3-runtime/src/kfd_backend/compute_quiescence_body.rs")
PROOF = V / "compute_quiescence_v1.rs"
FILES = [BODY, PROOF]
EXPECTED = {"encountered-error": False, "encountered-vir-error": False,
            "errors": 0, "is-verifying-entire-crate": True, "success": True, "verified": 12}


def need(value, message):
    if not value:
        raise ValueError(message)


def inherited():
    raw = (ROOT / CLASSIFIER).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == CLASSIFIER_SHA, "authenticated classifier")
    module = types.ModuleType("quiescence_classifier")
    module.__file__ = str(ROOT / CLASSIFIER)
    sys.modules[module.__name__] = module
    exec(compile(raw, module.__file__, "exec"), module.__dict__)
    return module


def mutations(body):
    cases = {}

    def add(name, old, new, focus="quiescence_step_v1"):
        need(body.count(old) == 1, "unique mutation site: " + name)
        cases[name] = (body.replace(old, new), "*" + focus + "*")

    add("wait-on-invalid-cursor", "QuiescenceActionV1::Invalid\n", "QuiescenceActionV1::Wait\n")
    add("complete-before-end", "else if $cursor == $len {", "else if $cursor <= $len {")
    add("deny-completion", "QuiescenceActionV1::Complete\n", "QuiescenceActionV1::Wait\n")
    add("advance-pending-owner", "else if $exact {", "else if !$exact {")
    add("deny-quiescent-advance", "QuiescenceActionV1::Advance\n", "QuiescenceActionV1::Wait\n")
    add("omit-advance", "next_cursor += 1;", "next_cursor += 0;")
    add("skip-roster-suffix", "next_cursor += 1;", "next_cursor = $len;")
    add("rewind-cursor", "next_cursor += 1;", "next_cursor = 0;")
    add("second-poll", "else if $polled {", "else if false {")
    add("forget-poll-budget", "next_polled = true;", "next_polled = false;")
    add("reset-existing-budget", "let mut next_polled = $polled;", "let mut next_polled = false;")
    add("spend-budget-on-advance", "next_cursor += 1;", "next_cursor += 1;\n                next_polled = true;")
    for name, expression in [
        ("ready-before-end", "$cursor < $len"),
        ("ready-incomplete", "$cursor <= $len"),
        ("ready-invalid", "$cursor >= $len"),
        ("ready-always", "true"),
        ("ready-never", "false"),
    ]:
        add(name, "$syntax!({ $cursor == $len })", "$syntax!({ " + expression + " })", "quiescence_complete_v1")
    need(len(cases) == len(set(cases.values())) == 17, "distinct mutation roster")
    return cases


def selection_notes(leaf):
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS, SELECTION_NOTES={
        "verifying root module (selected functions)",
        *{"verifying root module, function compute_quiescence_v1::" + name + " (selected functions)"
          for name in ("quiescence_step_v1", "quiescence_complete_v1")},
    })


def main():
    need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--verus", type=Path, required=True)
    args = parser.parse_args()
    out = args.output
    need(out.is_absolute() and out.resolve() == out and not out.exists(), "new canonical output")
    need(not out.is_relative_to(ROOT) and not ROOT.is_relative_to(out), "output outside repository")
    need(args.verus.is_absolute() and args.verus.resolve() == args.verus and args.verus.is_file(), "canonical verifier")
    os.chdir(ROOT)
    classifier = inherited()
    leaf = classifier.inherited()
    prior = leaf.load("quiescence_sources", ROOT / leaf.PRIOR, leaf.PRIOR_SHA)
    prior.INPUTS = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", "crates", "examples", "tests",
                    str(classifier.LEAF), str(classifier.LEAF.with_name("test-run.py")), str(leaf.PRIOR)]
    prior.clean_source()
    source = prior.source_tools()
    owner = leaf.load("quiescence_process_owner", ROOT / prior.CONTROLLER, prior.CONTROLLER_SHA)
    for number in owner.SIGNALS:
        signal.signal(number, owner.interrupted)
    out.mkdir()
    before = dict(commit=prior.git("rev-parse", "HEAD").decode().strip(), inputs=prior.snapshot())
    prior.save(out / "source-before.json", before)
    signer = out / "allowed-signers"
    signer.write_text(prior.SIGNER)
    env = dict(os.environ, VERUS_Z3_PATH=str(args.verus.parent / "z3"), TMPDIR=str(out))
    prior.save(out / "environment.json", {key: env.get(key) for key in
               ("PATH", "RUSTUP_TOOLCHAIN", "LD_LIBRARY_PATH", "VERUS_Z3_PATH", "TMPDIR")})
    notes = selection_notes(leaf)
    results = {}

    def phase(name, command, accept, environment=env):
        current = lambda: dict(commit=prior.git("rev-parse", "HEAD").decode().strip(), inputs=prior.snapshot())
        need(before == current(), "source continuity before " + name)
        status, stdout, stderr = owner.run_owned(command, 130, out / name, environment)
        passed = accept(status, stdout, stderr)
        results[name] = dict(passed=passed, status=status)
        prior.save(out / (name + ".json"), results[name])
        print(name + (": PASS" if passed else ": FAIL"), flush=True)
        need(before == current(), "source continuity after " + name)
        need(passed, "failed phase: " + name)

    def proof(root, focus=None):
        return ["/usr/bin/timeout", "--foreground", "--signal=TERM", "--kill-after=5", "120", str(args.verus),
                "--crate-type", "lib", "--triggers-mode", "silent", "--no-cheating", "--output-json", "--error-format=json",
                "--no-report-long-running", "--num-threads", "4", "--multiple-errors", "0",
                *([] if focus is None else ["--verify-function", focus, "--verify-root"]), str(root / PROOF)]

    def paths(root):
        return {str((root / path).resolve()) for path in FILES}

    def positive(root):
        return lambda s, o, e: classifier.proof_positive(s, o, e, prior.VERIFIER, EXPECTED, paths(root))

    def exact(expected):
        return lambda s, o, e: s == 0 and not e and o == expected

    source.authenticate(source.GIT)
    prior.signature_tool()
    phase("source-signature", [str(source.GIT), "--no-replace-objects", "--no-pager", "-c", "gpg.format=ssh",
          "-c", "gpg.ssh.program=/usr/bin/ssh-keygen", "-c", "gpg.ssh.allowedSignersFile=" + str(signer),
          "verify-commit", before["commit"]], lambda s, o, e: s == 0 and not o and e == prior.SIGNATURE, source.TOOL_ENV)
    prior.signature_tool()
    prior.save(out / "signed-inputs.json", leaf.bind_signed_blobs(prior, before))
    phase("classifier-tests", [sys.executable, "-I", "-B", str(ROOT / classifier.TEST)],
          exact("PASS: production planner campaign calibration (9 groups)\n"))
    phase("quiescence-tests", [sys.executable, "-I", "-B", str(ROOT / V / "test-compute-quiescence.py")],
          exact("PASS: quiescence control campaign calibration (4 groups)\n"))
    closure = ["/bin/sh", str(ROOT / "examples/row_softmax_v1/verify-verus-closure.sh"), str(args.verus.parent),
               str(ROOT / V / "pins/VERUS_CLOSURE_MANIFEST")]
    closure_ok = exact("PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n")
    phase("closure-before", closure, closure_ok)
    phase("proof-before", proof(ROOT), positive(ROOT))
    relocated = out / "relocated-source"
    expected = {str(path): before["inputs"][str(path)] for path in FILES}
    for path in FILES:
        destination = relocated / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / path, destination)
    need(leaf.tree(relocated) == expected, "exact signed relocation")
    prior.save(out / "relocated-inputs.json", expected)
    phase("relocated-proof", proof(relocated), positive(relocated))
    for name, (data, focus) in mutations((ROOT / BODY).read_text()).items():
        mutated = out / (name + "-source")
        shutil.copytree(relocated, mutated)
        (mutated / BODY).write_text(data)
        measured = leaf.tree(mutated)
        need(measured.keys() == expected.keys(), "exact mutant input set")
        need({p for p in expected if measured[p] != expected[p]} == {str(BODY)}, "only executable body changed")
        prior.save(out / (name + "-mutation.json"), dict(path=str(BODY), selector=focus, inputs=measured))
        phase(name, proof(mutated, focus), lambda s, o, e: classifier.logical_negative(notes, s, o, e, prior.VERIFIER, paths(mutated)))
        need(leaf.tree(mutated) == measured, "mutant continuity")
    phase("proof-after", proof(ROOT), positive(ROOT))
    phase("closure-after", closure, closure_ok)
    prior.clean_source()
    after = dict(commit=prior.git("rev-parse", "HEAD").decode().strip(), inputs=prior.snapshot())
    need(before == after and leaf.tree(relocated) == expected, "closing source continuity")
    prior.save(out / "source-after.json", after)
    prior.save(out / "results.json", results)


if __name__ == "__main__":
    main()
