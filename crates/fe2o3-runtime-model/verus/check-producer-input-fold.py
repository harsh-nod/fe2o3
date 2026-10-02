#!/usr/bin/env python3
"""Conditional actual fold-controller refinement, not native input validation.

The complete runtime Rust roster binds the native observation adapter and shared
fold, validation and scan macros. Only producer_input_fold_body is expanded in the proof. Each
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
SPEC = V / "producer_input_fold_spec_v1.rs"
FILES = [PROOF, SPEC, BODY]
BASE = V / "check-compute-pipeline-publication.py"
BASE_SHA = "1d4264a646983906fff5e54a2279865f5eba55413c1313698bee57064dfdfd8e"
SOURCE_TREE_SHA = "dec9139bb5bc55464fe2dd734ffdaedc5dd12dca7e5c972a50d4f33ec04ed215"
SOURCE_FILES = 346
PROOF_SHA = "b216150b8fc2b9300c9389b1dc97faf0310d72579a6cd9e2c5de1c5ecc7f1350"
SPEC_SHA = "5542b8152f39e4de0c6d3644efb8f612488c8e11158f5250c0c9ae297df38031"
BODY_SHA = "701824a7cf27d45d9ec93e36401bffd988e2d6e8e27da281868507a51f74f158"
FOLD_PREFIX_SHA = "f38d698075a14e469478805710dd6cf1058346aafc3d1b966efe0518c6a7c991"
FOLD_PREFIX_BYTES = 2003
# Fresh full unfiltered composition-lane discovery accepted this count, not mutants.
EXPECTED_VERIFIED = 13
MUTANT_COUNT = 22
MUTATION_ROSTER_SHA = "202fb6a329bcaac514974e915a48e348aeabadb4feee45b5b664cc0bbdd9aba2"
SELECTOR = "*Observations::reconcile"
SELECTION_NOTES = None  # Extraction requires fresh selector calibration.
FOLD_ANCHOR = "macro_rules! producer_input_fold_body {"
NATIVE_ANCHOR = "macro_rules! producer_dependency_contains_body {"


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(text):
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def tree_hash(sources):
    leaves = {str(path): sha(text) for path, text in sources.items()}
    return hashlib.sha256(json.dumps(leaves, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def snapshot():
    files = {PROOF, SPEC} | {path.relative_to(ROOT) for path in (ROOT / SRC).rglob("*.rs")}
    result = {}
    for path in files:
        selected = ROOT / path
        need(selected.is_file() and not selected.is_symlink() and selected.resolve() == selected,
             "ordinary exact producer-fold source path")
        result[path] = selected.read_bytes().decode("utf-8")
    return result


def audit(sources):
    implementation = {path: text for path, text in sources.items() if path.is_relative_to(SRC)}
    need(set(sources) == set(implementation) | {PROOF, SPEC}, "exact complete runtime Rust roster plus proof and spec")
    need(len(implementation) == SOURCE_FILES and tree_hash(implementation) == SOURCE_TREE_SHA,
         "reviewed runtime roster and bytes, including native adapter and helper boundary")
    need(sha(sources[PROOF]) == PROOF_SHA, "exact accepted receipt/cursor/error/frame contracts and proof")
    need(sha(sources[SPEC]) == SPEC_SHA, "exact extracted receipt/fold specifications")
    body = sources[BODY]
    need(sha(body) == BODY_SHA and body.count(NATIVE_ANCHOR) == 1,
         "exact shared body and first following helper boundary")
    prefix = body[:body.index(NATIVE_ANCHOR)]
    need(len(prefix.encode("utf-8")) == FOLD_PREFIX_BYTES and sha(prefix) == FOLD_PREFIX_SHA,
         "original 2003-byte fold prefix preserved without helper macros")
    need(sources[OWNER].count('include!("producer_input_fold_body.rs");') == 1
         and sources[OWNER].count("producer_input_fold_body!(") == 1
         and sources[OWNER].count("producer_input_validate_body!(") == 1,
         "actual runtime includes and invokes both production macros")
    need(sources[PROOF].count('include!("../../fe2o3-runtime/src/context/versions/producer_input_fold_body.rs");') == 1
         and len(re.findall(r"\binclude!\(", sources[PROOF])) == 1
         and not re.search(r"\binclude!\(", sources[BODY] + sources[SPEC]), "exact three-file executable proof closure")
    need(re.findall(r"\bmod\s+(\w+)\s*;", sources[PROOF]) == ["producer_input_fold_spec_v1"]
         and not re.search(r"\bmod\s+\w+\s*;", sources[SPEC] + sources[BODY]),
         "exact shared fold spec module")
    need(sources[PROOF].count("producer_input_fold_body!(") == 1
         and "producer_input_validate_body!(" not in sources[PROOF],
         "only actual fold controller is proved; native per-input body is not")
    need(not re.search(r"\b(?:assume|admit|assume_specification)\b|verifier::external|\buninterp\b", sources[PROOF] + sources[SPEC]),
         "no added assumed receipt/native facts or external proof bodies")


def change(body, before, after):
    need(body.count(FOLD_ANCHOR) == 1 and body.count(NATIVE_ANCHOR) == 1,
         "exact shared fold and first following helper macros")
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
    roster = {name: {"body": sha(text), "selector": focus} for name, (text, focus) in result.items()}
    need(sha(json.dumps(roster, sort_keys=True, separators=(",", ":"))) == MUTATION_ROSTER_SHA,
         "all 22 original native mutation names, bytes and selectors preserved")
    return result


def selection_notes(leaf, focus):
    need(focus == SELECTOR and SELECTION_NOTES is not None, "fresh fold selector diagnostics not yet measured")
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS, SELECTION_NOTES=SELECTION_NOTES)


def controller_source():
    raw = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BASE_SHA, "authenticated unchanged owned-process campaign")
    text = raw.decode("utf-8")
    need(text.count('"--multiple-errors", "0"') == 1, "unique reporting-only profile adaptation")
    return text.replace('"--multiple-errors", "0"', '"--multiple-errors", "1"')


def controller():
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


def campaign():
    need(SELECTION_NOTES is not None, "selection policy not yet measured")
    return controller()


if __name__ == "__main__":
    runpy.run_path(str(ROOT / V / "test-producer-input-fold.py"))
    campaign().main()
