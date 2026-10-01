#!/usr/bin/env python3
"""Conditional actual fold/validator composition source and mutation bindings.

Mutation construction is not verification. All composition mutants must run the
complete four-file closure; selecting only the caller would skip changed callees.
The native journal/live/credit implementations remain explicit open boundaries.
"""
import hashlib
import json
from pathlib import Path
import re
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
SRC = Path("crates/fe2o3-runtime/src")
PROOF = V / "context_producer_input_composition_v1.rs"
DEFINITIONS = V / "producer_input_validate_definitions_v1.rs"
SPEC = V / "producer_input_fold_spec_v1.rs"
BODY = SRC / "context/versions/producer_input_fold_body.rs"
FILES = [PROOF, DEFINITIONS, SPEC, BODY]
PROOF_PINS = {
    PROOF: "bca4d7dfa5611cd993d810dd36be7d15225b7b97c524cc68760be71596a37b60",
    DEFINITIONS: "18628bbcab588eeae7302fe39f2ade8bbc27f8506cc9ee440bc35381e17aa58b",
    SPEC: "5542b8152f39e4de0c6d3644efb8f612488c8e11158f5250c0c9ae297df38031",
    BODY: "701824a7cf27d45d9ec93e36401bffd988e2d6e8e27da281868507a51f74f158",
}
CHECKER_PINS = {
    "check-producer-input-validate.py": "2e82fcea3d00ebaf7599e3bf46627b239d80503f5324b0e0e22d72dcce637293",
    "check-producer-input-fold.py": "7bdf453325e7bdc8044acce2306bca367058e5b4f4a6a31f022d11c456f77eef",
}
EXPECTED_VERIFIED = 64
MUTANT_COUNT = 21
MUTATION_ROSTER_SHA = "3568fd947902e7db6f3b556b1a39e851ef49501932896fbfe6ae1482fefe4fb2"
DIAGNOSTICS = V / "producer-input-diagnostics-v1.py"
DIAGNOSTICS_SHA = "318a29f9067618c2168a8e1d3585b1e8e3f8c78abe40359d52b0a20458238be4"
DIAGNOSTIC_FIXTURES = V / "producer-input-diagnostic-fixtures-v1.json"
# Captures calibrate parsing; only a fresh signed campaign can qualify kills.
DIAGNOSTIC_CALIBRATION = {
    "fixture_sha256": "50bbc125bf95aa9ee283d89a6aa784c8e8b935b1fb2d106ee942a3dc35c3041d",
    "fixture_generator_sha256": "8c0cd832b66d27fc4102b093418e1d04baa4e40b1270977b67af08c343713cff",
    "original_interrupted_result_sha256": "0456177f95c05639185731f1929b749264fe7497f83ecb7fc60cd2060cc88820",
    "continuation_result_sha256": "87042b1898bde383704e3b54e96318206fe37fd3877c496b5cc86e1ec4e3ee27",
    "selected_fixtures": 4, "full_composition_fixtures": 21, "captured_kills_qualified": False,
}


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(text):
    return hashlib.sha256(text.encode()).hexdigest()


def load_checker(name):
    path = ROOT / V / name
    need(path.is_file() and not path.is_symlink() and path.resolve() == path, "ordinary checker source")
    raw = path.read_bytes()
    need(hashlib.sha256(raw).hexdigest() == CHECKER_PINS[name], "reviewed helper checker bytes")
    module = types.ModuleType(name.replace("-", "_").replace(".py", ""))
    module.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), module.__dict__)
    return module


def diagnostic_classifier():
    path = ROOT / DIAGNOSTICS
    fixture = ROOT / DIAGNOSTIC_FIXTURES
    need(all(item.is_file() and not item.is_symlink() and item.resolve() == item for item in (path, fixture)),
         "ordinary strict diagnostic predicate and fixture corpus")
    raw = path.read_bytes()
    need(hashlib.sha256(raw).hexdigest() == DIAGNOSTICS_SHA
         and hashlib.sha256(fixture.read_bytes()).hexdigest() == DIAGNOSTIC_CALIBRATION["fixture_sha256"],
         "reviewed strict classifier and observed diagnostic corpus")
    module = types.ModuleType("producer_input_strict_diagnostics")
    module.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), module.__dict__)
    return module


def snapshot():
    leaf = load_checker("check-producer-input-validate.py")
    sources = leaf.snapshot()
    del sources[leaf.PROOF]
    for path in (PROOF, SPEC):
        selected = ROOT / path
        need(selected.is_file() and not selected.is_symlink() and selected.resolve() == selected,
             "ordinary exact composition proof source")
        sources[path] = selected.read_bytes().decode()
    return sources


def audit(sources):
    leaf = load_checker("check-producer-input-validate.py")
    implementation = {path: text for path, text in sources.items() if path.is_relative_to(SRC)}
    need(set(leaf.DECLARATIONS).issubset(sources), "all native value schemas present")
    schemas = {path: sources[path] for path in leaf.DECLARATIONS}
    need(set(sources) == set(implementation) | set(schemas) | {PROOF, DEFINITIONS, SPEC},
         "exact complete runtime/schema and composition source roster")
    need(len(implementation) == 328 and len(schemas) == 5
         and leaf.tree_hash({**implementation, **schemas}) == leaf.SOURCE_TREE_SHA,
         "exact complete native/schema binding")
    need(all(sha(sources[path]) == digest for path, digest in PROOF_PINS.items()),
         "exact accepted four-file composition closure")
    leaf.schemas(sources)
    leaf.scan_bridges(sources)
    need(re.findall(r'\binclude!\("([^"]+)"\);', sources[PROOF])
         == ["producer_input_validate_definitions_v1.rs"], "exact composition include edge")
    need(re.findall(r"\bmod\s+(\w+)\s*;", sources[PROOF]) == ["producer_input_fold_spec_v1"],
         "exact composition spec edge")
    need(re.findall(r'\binclude!\("([^"]+)"\);', sources[DEFINITIONS])
         == ["../../fe2o3-runtime/src/context/versions/producer_input_fold_body.rs"],
         "exact shared definitions to actual body edge")
    need(not re.search(r"\binclude!|\bmod\s+\w+\s*;", sources[SPEC] + sources[BODY]),
         "no additional source closure edges")
    need(not re.search(r"\b(?:assume|admit|assume_specification)\b|verifier::external|\buninterp\b",
                       "\n".join(sources[path] for path in FILES)), "no new assumptions or external bodies")
    need(type(EXPECTED_VERIFIED) is int and EXPECTED_VERIFIED == 64,
         "exact fresh full composition discovery count")


def mutations(sources):
    audit(sources)
    rows = {}

    def add(name, path, before, after, boundary):
        text = sources[path]
        need(name not in rows and text.count(before) == 1 and before != after,
             "one exact nonvacuous mutation: " + name)
        rows[name] = {"path": path, "text": text.replace(before, after),
                      "before": before, "after": after, "boundary": boundary, "selector": None}

    call = "let result = validation.validate(self.id, self.consumer, self.launch, index, active, queued);"
    add("per-input-answer-index-zero", PROOF,
        "let answers = self.returns[index];", "let answers = self.returns[0];", "Composition::validate")
    add("actual-launch-flag-inverted", PROOF, call,
        call.replace("self.launch", "!self.launch"), "Composition::validate")
    add("actual-submission-generation-zero", PROOF, call,
        call.replace("self.id,", "RuntimeSubmissionIdV1 { context_generation: 0, local: self.id.local },"),
        "Composition::validate")
    add("actual-family-cursors-swapped", PROOF, call,
        call.replace("index, active, queued", "index, queued, active"), "Composition::validate")
    ending = "        result\n    }\n\n    fn reconcile"
    for name, replacement in (
        ("returned-error-substituted", "match result { Ok(status) => Ok(status), Err(_) => Err(ContextVersionJournalErrorV1::InvalidReference) }"),
        ("returned-pending-demoted", "match result { Ok(ContextProducerReadStatusV1::Pending) => Ok(ContextProducerReadStatusV1::Success), other => other }"),
        ("returned-unknown-demoted", "match result { Ok(ContextProducerReadStatusV1::Unknown) => Ok(ContextProducerReadStatusV1::NoEffect), other => other }"),
    ):
        add(name, PROOF, ending, ending.replace("        result\n", "        " + replacement + "\n"),
            "Composition::validate")
    add("consumed-only-on-success", PROOF,
        "self.consumed@ = (index + 1) as nat;",
        "self.consumed@ = if result is Ok { (index + 1) as nat } else { index as nat };",
        "Composition::validate")
    observed = ("self.receipts@ = self.receipts@.push(receipt(self.owner.root.inputs@[index as int].request,\n"
                "                active_before, queued_before, *active, *queued, result, validation.calls@, answers.credit));")
    replacement = ("let actual = receipt(self.owner.root.inputs@[index as int].request,\n"
                   "                active_before, queued_before, *active, *queued, result, validation.calls@, answers.credit);\n"
                   "            self.receipts@ = self.receipts@.push(fold::InputObservation {\n"
                   "                family: actual.family, advanced: result is Ok, result: actual.result, credit: actual.credit });")
    add("receipt-advance-derived-from-success", PROOF, observed, replacement, "Composition::validate")
    add("trace-prefix-dropped", PROOF, "self.calls@ = self.calls@ + validation.calls@;",
        "self.calls@ = validation.calls@;", "Composition::validate")
    add("trace-prefix-reversed", PROOF, "self.calls@ = self.calls@ + validation.calls@;",
        "self.calls@ = validation.calls@ + self.calls@;", "Composition::validate")
    add("receipt-credit-answer-inverted", PROOF, observed,
        observed.replace("answers.credit", "!answers.credit"), "Composition::validate")
    append = "{ proof { self.calls@ = self.calls@.push(Call::Credit(allocation, device, bytes)); } self.returns.credit }"
    for name, before, after in (
        ("allocation-generation", "allocation, device, bytes", "RuntimeAllocationIdV1 { context_generation: 0, local: allocation.local }, device, bytes"),
        ("allocation-local", "allocation, device, bytes", "RuntimeAllocationIdV1 { context_generation: allocation.context_generation, local: 0 }, device, bytes"),
        ("device-generation", "allocation, device, bytes", "allocation, RuntimeDeviceIdV1 { context_generation: 0, local: device.local }, bytes"),
        ("device-local", "allocation, device, bytes", "allocation, RuntimeDeviceIdV1 { context_generation: device.context_generation, local: 0 }, bytes"),
        ("byte-length", "allocation, device, bytes", "allocation, device, 0"),
    ):
        add("observed-credit-" + name + "-zero", DEFINITIONS, append,
            append.replace(before, after), "Observations::observe_expected_credit")
    add("actual-active-count-cross-wired", PROOF, "{ self.owner.root.references.len() }",
        "{ self.owner.root.queued_references.len() }", "Composition::active_count")
    add("actual-queued-count-cross-wired", PROOF, "{ self.owner.root.queued_references.len() }",
        "{ self.owner.root.references.len() }", "Composition::queued_count")
    fold = load_checker("check-producer-input-fold.py")
    original_fold = fold.mutations(sources[BODY])
    for name in ("unknown-stops-later-validation", "error-swallowed-as-success"):
        changed, _selector = original_fold[name]
        original = sources[BODY]
        start, end = original.index(fold.FOLD_ANCHOR), original.index(fold.NATIVE_ANCHOR)
        altered_end = changed.index(fold.NATIVE_ANCHOR)
        add("composed-" + ("error-swallowed" if name.startswith("error-") else name), BODY,
            original[start:end], changed[start:altered_end], "Composition::reconcile")
    need(len(rows) == MUTANT_COUNT and len({(row["path"], row["text"]) for row in rows.values()}) == MUTANT_COUNT,
         "exact 21 distinct candidate mutations; no logical qualification inferred")
    need(sha(json.dumps(mutation_inventory(rows), sort_keys=True, separators=(",", ":"))) == MUTATION_ROSTER_SHA,
         "reviewed exact source, delta and intended-boundary roster")
    return rows


def mutation_inventory(rows):
    return {name: {"path": str(row["path"]), "sha256": sha(row["text"]),
                   "before_sha256": sha(row["before"]), "after_sha256": sha(row["after"]),
                   "boundary": row["boundary"], "selector": row["selector"]}
            for name, row in rows.items()}
