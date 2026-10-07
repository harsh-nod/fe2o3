#!/usr/bin/env python3
"""Qualify shared table scans and ownership frames, not native publication authority."""

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
BODY = Path("crates/fe2o3-runtime/src/kfd_backend/compute_pipeline_publication_body.rs")
PROOF = V / "compute_pipeline_publication_v1.rs"
FILES = [BODY, PROOF]
EXPECTED = {"encountered-error": False, "encountered-vir-error": False,
            "errors": 0, "is-verifying-entire-crate": True, "success": True, "verified": 26}


def need(value, message):
    if not value:
        raise ValueError(message)


def inherited():
    raw = (ROOT / CLASSIFIER).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == CLASSIFIER_SHA, "authenticated classifier")
    module = types.ModuleType("pipeline_classifier")
    module.__file__ = str(ROOT / CLASSIFIER)
    sys.modules[module.__name__] = module
    exec(compile(raw, module.__file__, "exec"), module.__dict__)
    return module


def mutations(body):
    cases = {}

    def add(name, old, new, focus):
        need(body.count(old) == 1, "unique mutation site: " + name)
        cases[name] = (body.replace(old, new), "*" + focus)

    add("reuse-generation", "=> Some(generation),", "=> Some($slot.generation),", "vacant_generation")
    add("capacity-ignores-epoch", "&& $next.is_some() && first_vacant", "&& first_vacant", "has_capacity")
    add("lookup-ignores-owner", " && entry.active.id == $identity.submission", "", "exact_entry")
    add("frontier-ignores-generation", "|| $entry.identity.slot_generation != slot.generation", "|| false", "checked_frontier")
    add("frontier-ignores-live", "if $occupied != $live { return Err(()); }", "if false { return Err(()); }", "checked_frontier")
    add("frontier-accepts-duplicate", "if $found.is_some() { return Err(()); }", "if false { return Err(()); }", "checked_frontier")
    add("frontier-wrong-successor", "Some(epoch) => epoch.checked_add(1),", "Some(epoch) => Some(epoch),", "checked_frontier")
    add("staged-accepts-newer", "&& $entry.identity.logical_epoch >= $identity.logical_epoch", "&& false", "staged_intact")
    add("staged-rejects-own-identity", "$entry.identity != $identity\n                        && ", "", "staged_intact")
    add("stage-forgets-suffix", "$candidate = Some(($i, generation));", "$candidate = Some(($i, generation)); break;", "stage")
    add("stage-selects-last", "else if $candidate.is_none()", "else if true", "stage")
    add("stage-accepts-duplicate", "entry.active.id == $active.id || ", "", "stage")
    add("stage-substitutes-owner", "phase: RuntimeComputePipelinePhaseV1::Publishing, active: $active,",
        "phase: RuntimeComputePipelinePhaseV1::Publishing, active: ActiveSubmissionV1 { id: 0, ..$active },", "stage")
    add("stage-omits-live", "*$live += 1;", "*$live += 0;", "stage")
    add("confirm-reuses-epoch", "*$next = $identity.logical_epoch.checked_add(1);",
        "*$next = Some($identity.logical_epoch);", "confirm")
    add("confirm-skips-frontier", "if $frontier.is_none() { *$frontier = Some($identity.logical_epoch); }",
        "if true { *$frontier = Some($identity.logical_epoch); }", "confirm")
    add("withdraw-resets-generation", "let $entry = $slots[$identity.slot as usize].entry.take().unwrap();",
        "$slots[$identity.slot as usize].generation = 0;\n"
        "            let $entry = $slots[$identity.slot as usize].entry.take().unwrap();", "withdraw")
    add("withdraw-substitutes-owner", "Some($entry.active)",
        "Some(ActiveSubmissionV1 { id: 0, ..$entry.active })", "withdraw")
    need(len(cases) == len(set(cases.values())) == 18 and all(text != body for text, _ in cases.values()),
         "distinct production mutation roster")
    return cases


def selection_notes(leaf, focus=None):
    functions = ("vacant_generation", "has_capacity", "exact_entry", "checked_frontier", "staged_intact",
                 "stage", "confirm", "withdraw")
    if focus is not None:
        need(focus in {"*" + name for name in functions}, "exact mutation function")
        functions = (focus[1:],)
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS, SELECTION_NOTES={
        "verifying root module (selected functions)",
        *{"verifying root module, function compute_pipeline_publication_v1::" + name + " (selected functions)"
          for name in functions},
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
    prior = leaf.load("pipeline_sources", ROOT / leaf.PRIOR, leaf.PRIOR_SHA)
    prior.INPUTS = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", "crates", "examples", "tests",
                    str(classifier.LEAF), str(classifier.LEAF.with_name("test-run.py")), str(leaf.PRIOR)]
    prior.clean_source()
    source = prior.source_tools()
    owner = leaf.load("pipeline_process_owner", ROOT / prior.CONTROLLER, prior.CONTROLLER_SHA)
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
    phase("pipeline-tests", [sys.executable, "-I", "-B", str(ROOT / V / "test-compute-pipeline-publication.py")],
          exact("PASS: pipeline publication campaign calibration (4 groups)\n"))
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
        notes = selection_notes(leaf, focus)
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
