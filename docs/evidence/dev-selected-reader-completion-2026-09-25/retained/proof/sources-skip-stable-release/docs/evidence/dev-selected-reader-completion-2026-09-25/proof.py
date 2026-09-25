#!/usr/bin/env python3
"""Source-bound selected-reader proof development, not full Context qualification."""

import argparse
import hashlib
import json
from pathlib import Path
import signal
import sys
import types


ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
C = Path("crates/fe2o3-runtime/src/context")
PREVIOUS = V / "check-completion-journal-effects.py"
PREVIOUS_SHA = "aa66c149b5f694f4604154c34bd1a63308520bd0aebcf16e6f755f80f2f2ec7d"
PROOF_ROOT = V / "context_completion_journal_paired_v1.rs"
BODY = C / "versions/settlement_bodies.rs"
SELECTED = V / "context_completion_selected_readers_v1.rs"
WITNESSES = V / "context_completion_selected_reader_witnesses_v1.rs"
VERIFIED = 1303
BASELINE = "3b911a78d586bcb69cf52c8278893bdaa1f61453"


def need(value, message):
    if not value:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def save(path, value):
    with path.open("x") as output:
        json.dump(value, output, indent=2, sort_keys=True)
        output.write("\n")


def mutations(source):
    def change(before, after):
        need(source.count(before) == 1, "unique mutation anchor")
        return source.replace(before, after).encode()

    cases = {
        "omit-root-remove": (change("            ($roots).remove(&$id);", ""), "SelectedReadersV1::release_stable_v1"),
        "omit-marker-clear": (change("record.$marker = None;", "record.$marker = record.$marker;"), "SelectedReadersV1::release_producer_v1"),
        "continue-journal-error": (change("""            ) {
                Ok(()) => (),
                Err(error) => return Err(error),
            }
            $($after_effect)*""", """            ) {
                Ok(()) => (),
                Err(_) => (),
            }
            $($after_effect)*"""), "SelectedReadersV1::release_stable_v1"),
        "empty-selected-roster": (change("consumer, &root.references,", "consumer, &[],"), "SelectedReadersV1::release_stable_v1"),
        "wrong-selected-id": (change('($roots).get(&$id).expect("validated reader root")',
            '($roots).get(&0u64).expect("validated reader root")'), "SelectedReadersV1::release_stable_v1"),
        "skip-stable-release": (change("""            match $context.release_prevalidated_submission_readers_v1($id) {
                Ok(()) => (),
                Err(error) => return Err(error),
            }
""", ""), "SelectedReadersV1::release_inputs_v1"),
    }
    reversed_order = change("match $context.release_prevalidated_submission_readers_v1($id)",
        "match $context.release_validated_submission_producer_readers_v1($id)").decode()
    anchor = "let result = $context.release_validated_submission_producer_readers_v1($id);"
    need(reversed_order.count(anchor) == 1, "unique producer release")
    cases["producer-before-stable"] = (reversed_order.replace(anchor,
        "let result = $context.release_prevalidated_submission_readers_v1($id);").encode(),
        "SelectedReadersV1::release_inputs_v1")
    return cases


def scan(stage, deps):
    projections = {
        V / "context_completion_journal_effects_v1.rs": [
            'include!("../../fe2o3-runtime/src/context/versions/settlement_bodies.rs");\n',
            'include!("../../fe2o3-runtime/src/context/completion_journal_prefix_body.rs");\n'],
        C / "completion_settlement_body.rs": ['include!("completion_journal_prefix_body.rs");\n'],
        C / "completion_journal_prefix_body.rs": [], BODY: [],
        V / "context_completion_journal_witnesses_v1.rs": [],
        SELECTED: [], WITNESSES: [],
    }
    for path, includes in projections.items():
        text = (stage / path).read_text()
        for include in includes:
            need(text.count(include) == 1, "exact shared include")
            text = text.replace(include, "")
        count = 4 if path == BODY else 1 if path == WITNESSES or path.name in (
            "completion_settlement_body.rs", "completion_journal_prefix_body.rs") else 0
        need(text.count("macro_rules!") == count, "closed macro roster")
        target = stage / ("audited-selected-" + path.name)
        target.write_text(text.replace("macro_rules!", ""))
        deps["policy"].scan(target)


def main():
    need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--verus", type=Path, required=True)
    args = parser.parse_args()
    output, verus = args.output.absolute(), args.verus.resolve(strict=True)
    need(output.resolve() == output, "ordinary output location")
    inherited = (ROOT / PREVIOUS).read_bytes()
    need(digest(inherited) == PREVIOUS_SHA, "authenticate inherited checker")
    module = types.ModuleType("selected_reader_inherited_checker")
    module.__file__ = str(ROOT / PREVIOUS)
    exec(compile(inherited, str(ROOT / PREVIOUS), "exec"), module.__dict__)
    previous = vars(module)
    loaded = previous["load"](ROOT)
    _, owner, old, lifecycle, deps, *_, paths = loaded
    base, scalar, legacy = (deps[key] for key in ("base", "scalar", "legacy"))
    paths = paths | {SELECTED, WITNESSES, C / "versions/readers.rs",
        C / "generated_issue/tests/journal_tests/reader_tests.rs", Path(__file__).relative_to(ROOT)}
    data = {p: old.ordinary(ROOT, p) for p in paths}
    head = old.git(ROOT, "rev-parse", "HEAD").decode().strip()
    changes = {SELECTED, WITNESSES, PROOF_ROOT, BODY, C / "versions/readers.rs",
        C / "versions/producer_readers.rs", C / "generated_issue/tests/journal_tests/reader_tests.rs",
        V / "context_completion_journal_effects_v1.rs", Path(__file__).relative_to(ROOT)}
    for path, content in data.items():
        need(content == old.git(ROOT, "show", head + ":" + str(path)), "committed source: " + str(path))
        if path not in changes:
            need(content == old.git(ROOT, "show", BASELINE + ":" + str(path)), "unchanged inherited source: " + str(path))
    output.mkdir(parents=True, exist_ok=False)
    signer = output / "allowed-signers"
    signer.write_text(old.SIGNER)
    old.git(ROOT, "-c", "gpg.ssh.allowedSignersFile=" + str(signer), "verify-commit", head)
    old.git(ROOT, "-c", "gpg.ssh.allowedSignersFile=" + str(signer), "verify-commit", BASELINE)
    inputs = {str(p): digest(content) for p, content in sorted(data.items())}
    save(output / "source-before.json", dict(commit=head, inputs=inputs))
    environment = dict(HOME="/home/harsh", PATH="/home/harsh/.cargo/bin:/usr/bin:/bin",
        CARGO_HOME="/home/harsh/.cargo", RUSTUP_HOME="/home/harsh/.rustup",
        VERUS_Z3_PATH=str(verus.parent / "z3"), TMPDIR=str(output))
    for number in base.SIGNALS:
        signal.signal(number, base.interrupted)
    closure = ["/bin/sh", str(ROOT / legacy.CLOSURE), str(verus.parent), str(ROOT / legacy.MANIFEST)]
    stages = [("closure-before", None), ("positive-before", None),
        *mutations(data[BODY].decode()).items(), ("positive-after", None), ("closure-after", None)]
    results = {}
    for name, mutation in stages:
        need(head == old.git(ROOT, "rev-parse", "HEAD").decode().strip()
            and data == {p: old.ordinary(ROOT, p) for p in paths}, "unchanged committed sources")
        if name.startswith("closure-"):
            status, stdout, stderr = base.run_owned(closure, 120, output / name, environment)
            need(status == 0 and not stderr and stdout ==
                "PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n", "tool closure")
            results[name] = {"status": status}
        else:
            stage = output / ("sources-" + name)
            variant = data | ({BODY: mutation[0]} if mutation else {})
            owner.materialize(stage, variant)
            scan(stage, deps)
            selection = (name, None, None, "production::completion", mutation[1]) if mutation else None
            command = legacy.command(verus, stage / PROOF_ROOT, selection)
            command[4] = "600" if mutation else "1200"
            status, stdout, stderr = base.run_owned(command, int(command[4]) + 10, output / name, environment)
            need(variant == {p: old.ordinary(stage, p) for p in paths}, "unchanged staged sources")
            observed = legacy.normalized(base, stdout, stderr, stage)
            scalar.check_verifier(observed["verus"])
            if mutation:
                lifecycle.check_negative(scalar, name, status, observed)
            else:
                need(status == 0 and not observed["diagnostics"], "clean whole-root positive")
                need(legacy.same(observed["result"], {"encountered-error": False, "encountered-vir-error": False,
                    "success": True, "verified": VERIFIED, "errors": 0, "is-verifying-entire-crate": True}), "whole-root result")
            results[name] = observed
        save(output / (name + ".accepted.json"), results[name])
        print(name + ": PASS", flush=True)
    need(data == {p: old.ordinary(ROOT, p) for p in paths}, "closing source continuity")
    save(output / "source-after.json", dict(commit=head, inputs=inputs))
    save(output / "results.json", results)


if __name__ == "__main__":
    main()
