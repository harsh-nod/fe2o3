#!/usr/bin/env python3
"""Qualify shared queued-reader resolution under explicit storage premises, not the whole owner."""

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
BODY = Path("crates/fe2o3-runtime-model/src/context_queued_writers/read_resolution_body.rs")
PROOF = V / "context_queued_read_resolution_v1.rs"
FILES = [BODY, PROOF]
EXPECTED = {"encountered-error": False, "encountered-vir-error": False,
            "errors": 0, "is-verifying-entire-crate": True, "success": True, "verified": 24}


def need(value, message):
    if not value:
        raise ValueError(message)


def inherited():
    raw = (ROOT / CLASSIFIER).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == CLASSIFIER_SHA, "authenticated classifier")
    module = types.ModuleType("queued_resolution_classifier")
    module.__file__ = str(ROOT / CLASSIFIER)
    sys.modules[module.__name__] = module
    exec(compile(raw, module.__file__, "exec"), module.__dict__)
    return module


def mutations(body):
    cases = {}

    def add(name, old, new):
        need(body.count(old) == 1, "unique mutation site: " + name)
        cases[name] = (body.replace(old, new), "*resolve_reads*")

    add("omit-traversal", "let mut $index = 0usize;", "let mut $index = $root.read_count;")
    add("truncate-chain", "while $index < $root.read_count",
        "while $index < $root.read_count && $index == 0")
    add("lose-next", "$next = $entry.next;", "$next = None;")
    add("wrong-status", "$entry.status = $status;", "$entry.status = ContextProducerReadStatusV1::Unknown;")
    add("omit-version", "$entry.version = Some(($state.attempt_epoch, $state.content_lineage));", "$entry.version = None;")
    add("fabricate-epoch", "Some(($state.attempt_epoch, $state.content_lineage))", "Some((0, $state.content_lineage))")
    add("fabricate-lineage", "Some(($state.attempt_epoch, $state.content_lineage))", "Some(($state.attempt_epoch, 0))")
    add("failure-version", "$entry.previous = None;",
        "if $status != ContextProducerReadStatusV1::Success { $entry.version = Some((0, 0)); }\n                $entry.previous = None;")
    add("retain-previous", "$entry.previous = None;", "$entry.previous = $entry.previous;")
    add("retain-next", "$entry.next = None;", "$entry.next = $entry.next;")
    add("wrong-root", "$owner.roots[$root.writer.slot]", "$owner.roots[0]")
    add("retain-head", "$retained.read_head = None;", "$retained.read_head = $root.read_head;")
    add("retain-count", "$retained.read_count = 0;", "$retained.read_count = $root.read_count;")
    add("change-incarnation", "$entry.previous = None;",
        "$entry.reference.incarnation = 0;\n                $entry.previous = None;")
    add("early-refund", "$entry.next = None;", "$entry.next = None;\n                $owner.free_reads.push($slot);")
    add("clear-terminal", "$retained.read_count = 0;",
        "$retained.read_count = 0;\n            $owner.disposal_terminal = false;")
    need(len(cases) == len(set(cases.values())) == 16, "distinct mutation roster")
    return cases


def selection_notes(leaf):
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS, SELECTION_NOTES={
        "verifying root module (selected functions)",
        *{"verifying root module, function context_queued_read_resolution_v1::" + name + " (selected functions)"
          for name in ("resolve_reads",)},
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
    prior = leaf.load("queued_resolution_sources", ROOT / leaf.PRIOR, leaf.PRIOR_SHA)
    prior.INPUTS = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", "crates", "examples", "tests",
                    str(classifier.LEAF), str(classifier.LEAF.with_name("test-run.py")), str(leaf.PRIOR)]
    prior.clean_source()
    source = prior.source_tools()
    owner = leaf.load("queued_resolution_process_owner", ROOT / prior.CONTROLLER, prior.CONTROLLER_SHA)
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
    phase("resolution-tests", [sys.executable, "-I", "-B", str(ROOT / V / "test-queued-read-resolution.py")],
          exact("PASS: queued-read resolution campaign calibration (4 groups)\n"))
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
