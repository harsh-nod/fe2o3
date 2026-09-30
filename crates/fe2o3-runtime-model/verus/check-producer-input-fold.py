#!/usr/bin/env python3
"""Conditional actual fold-controller refinement, not native input validation.

The complete runtime Rust roster binds the native observation adapter and both
shared macros. Only producer_input_fold_body is expanded in the proof. Each
replay receipt must correspond to an actual reached validation, family advance
and exact return. Its credit metadata grants no accounting/native authority.
Opaque local owner/error values are framed without claiming an Arc interior
snapshot, native freshness, poison recovery, allocation, unwind or ISA proof.
"""
import hashlib
import json
from pathlib import Path
import re
import runpy
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
SRC = Path("crates/fe2o3-runtime/src")
OWNER = SRC / "context/versions/producer_readers.rs"
BODY = SRC / "context/versions/producer_input_fold_body.rs"
PROOF = V / "context_producer_input_fold_v1.rs"
FILES = [PROOF, BODY]
BASE = V / "check-compute-pipeline-publication.py"
BASE_SHA = "1d4264a646983906fff5e54a2279865f5eba55413c1313698bee57064dfdfd8e"
SOURCE_TREE_SHA = "19ec915583c9bac2da4ffeba186fe13747b6747ad7065655fe49ac726ad88671"
SOURCE_FILES = 327
PROOF_SHA = "0588fd557956b177b7b7be56fda006f25a82e509e23c40920907fecb78830184"
# Full unfiltered --no-cheating discovery V5 accepted this count, not the mutants.
EXPECTED_VERIFIED = 13
MUTANT_COUNT = 22
SELECTOR = "*Observations::reconcile"
FOLD_ANCHOR = "macro_rules! producer_input_fold_body {"
NATIVE_ANCHOR = "\nmacro_rules! producer_input_validate_body {"


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(text):
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def tree_hash(sources):
    leaves = {str(path): sha(text) for path, text in sources.items()}
    return hashlib.sha256(json.dumps(leaves, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def snapshot():
    files = {PROOF} | {path.relative_to(ROOT) for path in (ROOT / SRC).rglob("*.rs")}
    result = {}
    for path in files:
        selected = ROOT / path
        need(selected.is_file() and not selected.is_symlink() and selected.resolve() == selected,
             "ordinary exact producer-fold source path")
        result[path] = selected.read_bytes().decode("utf-8")
    return result


def audit(sources):
    implementation = {path: text for path, text in sources.items() if path.is_relative_to(SRC)}
    need(set(sources) == set(implementation) | {PROOF}, "exact complete runtime Rust roster plus proof")
    need(len(implementation) == SOURCE_FILES and tree_hash(implementation) == SOURCE_TREE_SHA,
         "reviewed runtime roster and bytes, including native adapter and helper boundary")
    need(sha(sources[PROOF]) == PROOF_SHA, "exact accepted receipt/cursor/error/frame contracts and proof")
    need(sources[OWNER].count('include!("producer_input_fold_body.rs");') == 1
         and sources[OWNER].count("producer_input_fold_body!(") == 1
         and sources[OWNER].count("producer_input_validate_body!(") == 1,
         "actual runtime includes and invokes both production macros")
    need(sources[PROOF].count('include!("../../fe2o3-runtime/src/context/versions/producer_input_fold_body.rs");') == 1
         and len(re.findall(r"\binclude!\(", sources[PROOF])) == 1
         and not re.search(r"\binclude!\(", sources[BODY]), "exact two-file executable proof closure")
    need(sources[PROOF].count("producer_input_fold_body!(") == 1
         and "producer_input_validate_body!(" not in sources[PROOF],
         "only actual fold controller is proved; native per-input body is not")
    need(not re.search(r"\b(?:assume|admit|assume_specification)\b|verifier::external|\buninterp\b", sources[PROOF]),
         "no added assumed receipt/native facts or external proof bodies")


def change(body, before, after):
    need(body.count(FOLD_ANCHOR) == 1 and body.count(NATIVE_ANCHOR) == 1,
         "exact shared fold and native validation macros")
    start, end = body.index(FOLD_ANCHOR), body.index(NATIVE_ANCHOR)
    need(start < end, "fold macro precedes native helper")
    selected = body[start:end]
    need(selected.count(before) == 1 and before != after, "one meaningful fold-only mutation")
    return body[:start] + selected.replace(before, after) + body[end:]


def mutations(body):
    rows = []

    def add(name, before, after):
        rows.append((name, before, after))

    validate = "$observations.validate($index, &mut $active_index, &mut $queued_index)"
    add("error-substitutes-invalid-reference", "let status = " + validate + "?;",
        "let status = match " + validate + " { Ok(value) => value, Err(_error) => return Err($invalid_reference) };")
    add("error-swallowed-as-success", "let status = " + validate + "?;",
        "let status = match " + validate + " { Ok(value) => value, Err(_error) => ContextProducerReadStatusV1::Success };")
    add("unknown-stops-later-validation", "                $index += 1;",
        "                if let ContextProducerReadStatusV1::Unknown = $aggregate { return Ok($aggregate); }\n"
        "                $index += 1;")
    for label, variant, weaker in (("unknown", "Unknown", "NoEffect"),
                                   ("no-effect", "NoEffect", "Pending"),
                                   ("pending", "Pending", "Success")):
        prefix = "(ContextProducerReadStatusV1::" + variant + ", _)\n                    | "
        add(label + "-forgets-prior-aggregate", prefix, "")
        output = "| (_, ContextProducerReadStatusV1::" + variant + ") => ContextProducerReadStatusV1::" + variant + ","
        add(label + "-demoted", output, output.rsplit(variant, 1)[0] + weaker + ",")
    add("initial-aggregate-unknown", "let mut $aggregate = ContextProducerReadStatusV1::Success;",
        "let mut $aggregate = ContextProducerReadStatusV1::Unknown;")
    for label, cursor in (("input", "$index"), ("active", "$active_index"), ("queued", "$queued_index")):
        add("initial-" + label + "-cursor-one", "let mut " + cursor + " = 0usize;",
            "let mut " + cursor + " = 1usize;")
    add("validation-family-cursors-swapped", validate,
        "$observations.validate($index, &mut $queued_index, &mut $active_index)")
    active = "$active_index != $observations.active_count()"
    queued = "$queued_index != $observations.queued_count()"
    add("final-active-count-omitted", active, "false")
    add("final-queued-count-omitted", queued, "false")
    add("final-counts-require-both-mismatches", "|| " + queued, "&& " + queued)
    add("final-active-count-cross-wired", active, "$active_index != $observations.queued_count()")
    add("final-queued-count-cross-wired", queued, "$queued_index != $observations.active_count()")
    add("final-count-error-promoted", "return Err($invalid_reference);", "return Ok($aggregate);")
    add("validation-duplicated-at-same-index", "let status = " + validate + "?;",
        "let _duplicate = " + validate + "?;\n                let status = " + validate + "?;")
    add("success-stops-after-first-input", "                $index += 1;",
        "                $index += 1;\n                if $index > 0 { return Ok($aggregate); }")
    result = {name: (change(body, before, after), SELECTOR) for name, before, after in rows}
    need(len(result) == len(rows) == MUTANT_COUNT and len(set(result.values())) == MUTANT_COUNT,
         "distinct actual shared fold mutation roster")
    return result


def selection_notes(leaf, focus):
    need(focus == SELECTOR, "exact shared fold reconciliation selector")
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS, SELECTION_NOTES={
        "verifying root module (selected functions)",
        "verifying root module, function context_producer_input_fold_v1::Observations::reconcile (selected functions)",
    })


def controller_source():
    raw = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BASE_SHA, "authenticated unchanged owned-process campaign")
    return raw.decode("utf-8")


def campaign():
    audit(snapshot())
    need(type(EXPECTED_VERIFIED) is int and EXPECTED_VERIFIED == 13, "exact measured full positive count")
    module = types.ModuleType("producer_input_fold_campaign")
    module.__file__ = str(ROOT / BASE)
    sys.modules[module.__name__] = module
    exec(compile(controller_source(), module.__file__, "exec"), module.__dict__)
    module.FILES, module.PROOF, module.BODY = FILES, PROOF, BODY
    module.EXPECTED = dict(module.EXPECTED, verified=EXPECTED_VERIFIED)
    module.mutations, module.selection_notes = mutations, selection_notes
    return module


if __name__ == "__main__":
    runpy.run_path(str(ROOT / V / "test-producer-input-fold.py"))
    campaign().main()
