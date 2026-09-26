#!/usr/bin/env python3
"""Qualify the shared finite-graph planner, not the complete Context adapters."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
BODY = Path("crates/fe2o3-runtime/src/context/completion_reconciliation_body.rs")
LEAF = Path("docs/evidence/dev-completion-leaf-outcomes-2026-09-25/run.py")
LEAF_SHA = "3cf248a9d642713ea738f58b882c7ccd7c38d8b5ccc6cbfbe3dfe246b03f412c"
TEST = V / "test-completion-reconciliation-campaign.py"
FILES = [BODY] + [V / ("context_completion_reconciliation" + suffix + "_v1.rs")
                  for suffix in ("", "_graph", "_validation", "_effects", "_path", "_leaf", "_planner")]


def need(value, message):
    if not value:
        raise ValueError(message)


def inherited():
    raw = (ROOT / LEAF).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == LEAF_SHA, "authenticated leaf controller")
    module = types.ModuleType("planner_campaign_leaf")
    module.__file__ = str(ROOT / LEAF)
    sys.modules[module.__name__] = module
    exec(compile(raw, module.__file__, "exec"), module.__dict__)
    return module


def mutations(body):
    """Each mutation changes only executable production tokens, not proof hooks."""
    cases = {}

    def add(name, old, new):
        need(body.count(old) == 1, "unique mutation site: " + name)
        cases[name] = body.replace(old, new)

    for name, call in (("ordinary", "require_ordinary_submission_v1"),
                       ("custody", "check_operation_custody_v1")):
        add("omit-" + name, "                    $context." + call + "($id)?;\n", "")
    for name, flag, call in (("peer", "directed_peer_copy", "validate_pending_peer_copy_roots_v1"),
                             ("producer", "producer_launch", "validate_pending_producer_launch_roots_v1")):
        old = ("                    if !$record.status.is_terminal() && $record." + flag + " {\n"
               "                        let result = $context." + call + "($id);\n"
               "                        $context.journal_result_v1(result)?;\n"
               "                    }\n")
        add("omit-" + name + "-roots", old, "")
    add("observe-wrong-backend", "backend: $record.backend_submission,\n                        });",
        "backend: 0,\n                        });")
    add("observe-wrong-selected-id", "id: $id,\n                            backend:",
        "id: $requested,\n                            backend:")
    add("descend-wrong-id", "$id = $dependency.submission;", "$id = $requested;")
    add("push-wrong-parent", "$path[$length] = $id;", "$path[$length] = $dependency.submission;")
    add("pop-wrong-parent", "$id = $path[$length];", "$id = $requested;")
    add("cache-after-descent", "$id = $dependency.submission;\n                            $validated = None;",
        "$id = $dependency.submission;\n                            $validated = Some($id);")
    add("cache-after-pop", "$id = $path[$length];\n                    $validated = None;",
        "$id = $path[$length];\n                    $validated = Some($id);")
    add("cursor-skips-producer", ".cursor += 1;", ".cursor += 2;")
    cursor_branch = """RuntimeCompletionStatusV1::Succeeded => {
                            $context.producer_completion_state_mut_v1($id)
                                .expect("retained success-gated state")
                                .cursor += 1;
                            $($after_advance)*
                        }
                        RuntimeCompletionStatusV1::Pending => {"""
    add("cursor-crosses-pending", cursor_branch,
        cursor_branch.replace("Succeeded", "SWAP").replace("Pending", "Succeeded").replace("SWAP", "Pending"))
    add("quiescent-predecessor-promoted", "RuntimeCompletionStatusV1::QuiescentWithoutResult => {\n"
        "                            let result = $context.transition_submission_status(\n"
        "                                $id,\n                                RuntimeCompletionStatusV1::QuiescentWithoutResult,",
        "RuntimeCompletionStatusV1::QuiescentWithoutResult => {\n"
        "                            let result = $context.transition_submission_status(\n"
        "                                $id,\n                                RuntimeCompletionStatusV1::Succeeded,")
    add("pending-input-promoted", "None | Some(ContextProducerReadStatusV1::Success) => {",
        "None | Some(ContextProducerReadStatusV1::Success) | Some(ContextProducerReadStatusV1::Pending) => {")
    add("no-effect-input-promoted", "None | Some(ContextProducerReadStatusV1::Success) => {",
        "None | Some(ContextProducerReadStatusV1::Success) | Some(ContextProducerReadStatusV1::NoEffect) => {")
    add("failed-physical-observation-promoted", "Some(BackendPollV1::Succeeded) => {}",
        "Some(BackendPollV1::Succeeded) | Some(BackendPollV1::Failed { .. }) => {}")
    add("ignore-success-settlement-error", "let result = $context.transition_submission_status($id, RuntimeCompletionStatusV1::Succeeded);\n"
        "                        $($after_settlement)*\n                        result?;",
        "let result = $context.transition_submission_status($id, RuntimeCompletionStatusV1::Succeeded);\n"
        "                        $($after_settlement)*\n                        let _ = result;")
    add("wrong-local-yield", "$context.submissions[&$requested].status", "$context.submissions[&$id].status")
    add("short-fuel", "let mut $remaining = 2 * MAX_RUNTIME_DEPENDENCIES_V1 + 1;",
        "let mut $remaining = 2 * MAX_RUNTIME_DEPENDENCIES_V1;")
    add("early-yield", "while $remaining > 0", "while $remaining > 1")
    need(len(cases) == 21 and len(set(cases.values())) == 21 and body not in cases.values(),
         "distinct production mutation roster")
    return cases


def main():
    need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--verus", type=Path, required=True)
    args = parser.parse_args()
    out = args.output
    need(out.is_absolute() and out.resolve() == out and not out.exists(), "new canonical output")
    need(not out.is_relative_to(ROOT) and not ROOT.is_relative_to(out), "output outside repository")
    need(args.verus.is_absolute() and args.verus.is_file() and not args.verus.is_symlink(), "ordinary absolute verifier")
    os.chdir(ROOT)
    leaf = inherited()
    prior = leaf.load("planner_campaign_sources", ROOT / leaf.PRIOR, leaf.PRIOR_SHA)
    prior.INPUTS = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", "crates", "examples", "tests",
                    str(LEAF), str(LEAF.with_name("test-run.py")), str(leaf.PRIOR),
                    "docs/runtime-completion-reconciliation-v1.md"]
    prior.clean_source()
    source = prior.source_tools()
    owner = leaf.load("planner_campaign_owner", ROOT / prior.CONTROLLER, prior.CONTROLLER_SHA)
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
        need(before == dict(commit=prior.git("rev-parse", "HEAD").decode().strip(), inputs=prior.snapshot()),
             "source continuity before " + name)
        status, stdout, stderr = owner.run_owned(command, 130, out / name, environment)
        passed = accept(status, stdout, stderr)
        results[name] = dict(passed=passed, status=status)
        prior.save(out / (name + ".json"), results[name])
        print(name + (": PASS" if passed else ": FAIL"), flush=True)
        need(before == dict(commit=prior.git("rev-parse", "HEAD").decode().strip(), inputs=prior.snapshot()),
             "source continuity after " + name)
        need(passed, "failed phase: " + name)

    def positive(status, stdout, stderr):
        if status != 0 or stderr:
            return False
        result = json.loads(stdout)
        return (json.dumps(result.get("verification-results"), sort_keys=True) == json.dumps(leaf.PROOF_RESULT, sort_keys=True)
                and json.dumps(result.get("verus"), sort_keys=True) == json.dumps(prior.VERIFIER, sort_keys=True))

    def proof(root, focus=None):
        return ["/usr/bin/timeout", "--foreground", "--signal=TERM", "--kill-after=5", "120", str(args.verus),
                "--crate-type", "lib", "--triggers-mode", "silent", "--no-cheating", "--output-json", "--error-format=json",
                "--no-report-long-running", "--num-threads", "4",
                *([] if focus is None else ["--verify-function", focus, "--verify-root"]), str(root / FILES[1])]

    def exact(expected):
        return lambda status, stdout, stderr: status == 0 and not stderr and stdout == expected

    source.authenticate(source.GIT)
    prior.signature_tool()
    phase("source-signature", [str(source.GIT), "--no-replace-objects", "--no-pager", "-c", "gpg.format=ssh",
          "-c", "gpg.ssh.program=/usr/bin/ssh-keygen", "-c", "gpg.ssh.allowedSignersFile=" + str(signer),
          "verify-commit", before["commit"]], lambda s, o, e: s == 0 and not o and e == prior.SIGNATURE, source.TOOL_ENV)
    prior.signature_tool()
    prior.save(out / "signed-inputs.json", leaf.bind_signed_blobs(prior, before))
    phase("inherited-classifier-tests", [sys.executable, "-I", "-B", str(ROOT / LEAF.with_name("test-run.py"))],
          exact("PASS: leaf outcome negative classifier (12 groups)\n"))
    phase("campaign-tests", [sys.executable, "-I", "-B", str(ROOT / TEST)],
          exact("PASS: production planner campaign calibration (6 groups)\n"))
    phase("source-tests", [sys.executable, "-I", "-B", str(ROOT / V / "test-completion-reconciliation-source.py")],
          exact("PASS: completion planner source calibration (8 groups)\n"))
    phase("source-body", [sys.executable, "-I", "-B", str(ROOT / V / "check-completion-reconciliation-source.py"),
                          "--output", str(out / "body-comparison")], exact("PASS: allowlisted completion planner syntax correspondence\n"))
    closure = ["/bin/sh", str(ROOT / "examples/row_softmax_v1/verify-verus-closure.sh"), str(args.verus.parent),
               str(ROOT / V / "pins/VERUS_CLOSURE_MANIFEST")]
    closure_ok = exact("PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n")
    phase("closure-before", closure, closure_ok)
    phase("proof-before", proof(ROOT), positive)
    relocated = out / "relocated-source"
    expected = {str(path): before["inputs"][str(path)] for path in FILES}
    for path in FILES:
        destination = relocated / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / path, destination)
    need(leaf.tree(relocated) == expected, "exact signed relocation")
    prior.save(out / "relocated-inputs.json", expected)
    phase("relocated-proof", proof(relocated), positive)
    need(leaf.tree(relocated) == expected, "relocated source unchanged")
    body = (ROOT / BODY).read_text()
    roster = [(name, BODY, data, "*plan_completion_step_v1*") for name, data in mutations(body).items()]
    for name, path, old, new, focus in leaf.MUTANTS:
        raw = (ROOT / path).read_text()
        need(raw.count(old) == 1, "unique inherited mutation")
        roster.append(("leaf-" + name, path, raw.replace(old, new), focus))
    for name, path, data, focus in roster:
        mutated = out / (name + "-source")
        shutil.copytree(relocated, mutated)
        (mutated / path).write_text(data)
        measured = leaf.tree(mutated)
        need(measured.keys() == expected.keys(), "exact mutant input set")
        need({p for p in expected if measured[p] != expected[p]} == {str(path)}, "one changed source")
        prior.save(out / (name + "-mutation.json"), dict(path=str(path), selector=focus, inputs=measured))
        paths = {str((mutated / path).resolve()) for path in FILES}
        phase(name, proof(mutated, focus), lambda s, o, e: leaf.logical_negative(s, o, e, prior.VERIFIER, paths))
        need(leaf.tree(mutated) == measured, "mutant continuity")
    phase("proof-after", proof(ROOT), positive)
    phase("closure-after", closure, closure_ok)
    prior.clean_source()
    after = dict(commit=prior.git("rev-parse", "HEAD").decode().strip(), inputs=prior.snapshot())
    need(before == after, "closing source continuity")
    prior.save(out / "source-after.json", after)
    prior.save(out / "results.json", results)


if __name__ == "__main__":
    main()
