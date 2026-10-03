#!/usr/bin/env python3
"""Actual per-input validation against typed, individually reached observations.

The source guard includes complete runtime Rust plus the compared model schemas.
Native lookup/status/live/account implementations are forwarding boundaries, not
verified by this packet. The executable proof uses the actual validation macro;
the included older fold macro is not expanded and needs separate requalification
because its containing source file changed. No shared-Arc snapshot, native
freshness, mutex recovery, unwind, allocation, compiler or ISA theorem is claimed.
"""
import hashlib
import json
from pathlib import Path
import re
import runpy
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
SRC = Path("crates/fe2o3-runtime/src")
MODEL = Path("crates/fe2o3-runtime-model/src")
OWNER = SRC / "context/versions/producer_readers.rs"
BODY = SRC / "context/versions/producer_input_fold_body.rs"
JOURNAL_BODY = SRC / "context/versions/producer_journal_observer_bodies.rs"
PROOF = V / "context_producer_input_validate_v1.rs"
DEFINITIONS = V / "producer_input_validate_definitions_v1.rs"
RUNTIME_DECLARATIONS = V / "producer_input_runtime_declarations_v1.rs"
JOURNAL_DECLARATIONS = V / "producer_input_journal_comparison_declarations_v1.rs"
OUTCOMES = V / "producer_input_outcome_spec_v1.rs"
PARTS = [DEFINITIONS, RUNTIME_DECLARATIONS, JOURNAL_DECLARATIONS, OUTCOMES]
FILES = [PROOF, *PARTS, BODY]
DECLARATIONS = [MODEL / name for name in (
    "context_version_journal/declarations.rs",
    "context_version_journal/enrollment_declarations.rs",
    "context_read_leases/declarations.rs",
    "context_producer_reads/declarations.rs",
    "context_queued_writers/read_declarations.rs",
)]
BASE = V / "check-compute-pipeline-publication.py"
BASE_SHA = "1d4264a646983906fff5e54a2279865f5eba55413c1313698bee57064dfdfd8e"
SOURCE_TREE_SHA = "488239d1b1e3d144a9497986d77096d657800a5467dad8f9d78b7bb8c86de57d"
SOURCE_FILES = 414
PROOF_SHA = "750a1ae6be20bed3c6dc9b3ebaa6cca4a869713b3187da8dabe32091b8fa5817"
PART_PINS = {
    DEFINITIONS: "44e8a46f132d86d610e693a2a5a877988b125b9019595b373d976774efcdca49",
    RUNTIME_DECLARATIONS: "25221501f5f3d7ec63fad45e14e015c01edb993f57881b5d61cc4274c34469db",
    JOURNAL_DECLARATIONS: "5b1ddfe8dea991d6e3a17d98da8774746913e5fc6e65153e4901d6a2d1cee3eb",
    OUTCOMES: "f6fb6dd9739e4a6c30e2053350e66422dded9d9a54e436c8e8dd10495aff7634",
}
EXPECTED_VERIFIED = None  # Changed closure: predecessor 42 is not inherited.
SELECTOR = "*Observations::validate"
SCAN_SELECTORS = ("*producer_dependency_contains_v1", "*producer_source_pair_contains_v1")
SELECTION_NOTES = None  # Extraction requires fresh selector calibration.
MUTANT_COUNT = 38
MUTATION_ROSTER_SHA = "d93b43692de73a490b43149b4f6016542980aa68e069775a364bd67f9b757703"


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(text):
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def tree_hash(sources):
    return hashlib.sha256(json.dumps({str(path): sha(text) for path, text in sources.items()},
                                    sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def snapshot():
    paths = {PROOF, *PARTS, *DECLARATIONS} | {p.relative_to(ROOT) for p in (ROOT / SRC).rglob("*.rs")}
    result = {}
    for path in paths:
        selected = ROOT / path
        need(selected.is_file() and not selected.is_symlink() and selected.resolve() == selected,
             "ordinary exact validator source path")
        result[path] = selected.read_bytes().decode("utf-8")
    return result


def block(text, anchor):
    need(text.count(anchor) == 1, "unique source anchor: " + anchor)
    start = text.index("{", text.index(anchor))
    depth = 1
    end = start + 1
    while depth and end < len(text):
        depth += (text[end] == "{") - (text[end] == "}")
        end += 1
    need(depth == 0, "balanced source anchor")
    return text[start:end]


def normalized(text):
    text = re.sub(r"//[^\n]*", "", text)
    text = re.sub(r"\bpub(?:\([^)]*\))?\s*", "", text)
    text = re.sub(r"\s+", "", text)
    return text.replace("fe2o3_runtime_model::", "").replace(",}", "}")


def schemas(sources):
    proof = "\n".join(sources[path] for path in PARTS)
    pairs = {
        SRC / "context.rs": ["RuntimeMemoryKindV1", "RuntimeAccessV1", "RuntimeMemoryRegionV1", "AllocationRecordV1"],
        SRC / "context/peer_custody.rs": ["ScalarPeerDependencyV1"],
        SRC / "context/versions/readers.rs": ["ContextReadSourceV1"],
        OWNER: ["ProducerReadRequestV1", "ProducerInputV1"],
        DECLARATIONS[0]: ["ContextWriterKindV1", "ContextWriterKeyV1", "ContextWriterReferenceV1",
                          "ContextAllocationKeyV1", "ContextJournalDeviceKeyV1", "ContextAllocationReferenceV1",
                          "ContextAllocationWriteV1", "ContextVersionJournalErrorV1"],
        DECLARATIONS[1]: ["ContextAllocationEnrollmentV1"],
        DECLARATIONS[2]: ["ContextAllocationReadV1"],
        DECLARATIONS[3]: ["ContextProducerReadV1", "ContextProducerReadReferenceV1", "ContextProducerReadStatusV1"],
        DECLARATIONS[4]: ["ContextQueuedProducerReadV1", "ContextQueuedProducerReadReferenceV1"],
    }
    for path, names in pairs.items():
        for name in names:
            anchor = re.search(r"\b(struct|enum) " + name + r"\s*\{", sources[path])
            need(anchor is not None, "native concrete schema " + name)
            need(normalized(block(sources[path], anchor.group())) == normalized(block(proof, anchor.group().split("{")[0].strip())),
                 "all concrete fields and variants match: " + name)
    native = sources[SRC / "context.rs"]
    need("#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]" in native,
         "actual lexicographic identity derive remains explicit compiler trust")
    for name in ("RuntimeSubmissionIdV1", "RuntimeAllocationIdV1", "RuntimeDeviceIdV1", "RuntimeStreamIdV1", "RuntimeEventIdV1"):
        need("runtime_id!(" + name + ");" in native, "actual complete two-coordinate identity " + name)
        need(normalized(block(proof, "struct " + name)) == "{context_generation:u64,local:u64}", "full identity projection")
    need(normalized(block(native, "macro_rules! runtime_id" )).count("context_generation:u64") >= 1,
         "native identity declaration bound")
    enrollment = block(sources[SRC / "context/versions.rs"], "pub(super) fn enrollment(")
    need(normalized(enrollment) in normalized(proof), "complete native enrollment expression, not a device-only Boolean")


def scan_bridges(sources):
    for name, macro, values, needle in (
        ("producer_dependency_contains_v1", "producer_dependency_contains_body", "dependencies", "dependency"),
        ("producer_source_pair_contains_v1", "producer_source_pair_contains_body", "sources", "source"),
    ):
        native = block(sources[OWNER], "fn " + name + "(")
        expected = "{" + macro + "!(completion_journal_rust_syntax," + values + "," + needle + ",index,[])}"
        need(normalized(native) == expected, "one private actual scan body: " + name)
        need(sources[OUTCOMES].count(macro + "!(verus_exec_expr, " + values + ", " + needle + ", index,") == 1,
             "proof invokes the identical scan macro: " + name)
    need(".any(" not in sources[BODY], "membership no longer depends on concrete iterator default contracts")
    need(sources[BODY].count("producer_dependency_contains_v1(") == 2
         and sources[BODY].count("producer_source_pair_contains_v1(") == 1,
         "two dependency sites and one full source-pair site")


def closure(sources, root, expected):
    reached, pending = set(), [root]
    while pending:
        path = pending.pop()
        if path in reached:
            continue
        need(path in expected and path in sources, "declared recursive proof input")
        reached.add(path)
        text = sources[path]
        includes = re.findall(r'\binclude!\("([^"\n]+)"\);', text)
        modules = re.findall(r'\bmod\s+(\w+)\s*;', text)
        need(text.count("include!(") == len(includes) and "#[path" not in text,
             "literal closed include edges")
        for name in includes + [name + ".rs" for name in modules]:
            target = (ROOT / path.parent / name).resolve()
            need(target.is_relative_to(ROOT), "contained proof source edge")
            pending.append(target.relative_to(ROOT))
    need(reached == set(expected), "complete exact recursive proof closure")


def audit(sources):
    implementation = {p: text for p, text in sources.items() if p not in (PROOF, *PARTS)}
    need(set(sources) == ({p for p in sources if p.is_relative_to(SRC)} | set(DECLARATIONS) | {PROOF, *PARTS}),
         "exact runtime plus schema plus proof roster")
    need(len(implementation) == SOURCE_FILES and tree_hash(implementation) == SOURCE_TREE_SHA,
         "reviewed whole runtime and exact native value schema source")
    need(sha(sources[PROOF]) == PROOF_SHA, "reviewed exact per-input proof and contracts")
    need(set(PART_PINS) == set(PARTS) and all(sha(sources[path]) == digest for path, digest in PART_PINS.items()),
         "reviewed exact factored validator definitions")
    schemas(sources)
    scan_bridges(sources)
    need(sha(sources[JOURNAL_BODY]) == "5fb7f1572c41a6f2c6dffa74f040e870bb4ae874133c127ee5ae87d52c3fce59",
         "exact extracted journal forwarding bodies; this leaf theorem remains conditional")
    forwarders = {
        "observe_active_lookup": "producer_observe_active_lookup_body_v1!(self, reference)",
        "observe_active_status": "producer_observe_active_status_body_v1!(self, reference)",
        "observe_queued_lookup": "producer_observe_queued_lookup_body_v1!(self, reference)",
        "observe_queued_status": "producer_observe_queued_status_body_v1!(self, reference)",
        "observe_live": "self.versions.validate_live(id, record)",
        "observe_expected_credit": "self.context.allocation_admission.has_expected_credit(id, device, byte_len)",
    }
    for name, expression in forwarders.items():
        need(normalized(block(sources[OWNER], "fn " + name + "(")) == normalized("{" + expression + "}"),
             "one exact native typed helper forwarding call: " + name)
    need(sources[OWNER].count('include!("producer_input_fold_body.rs");') == 1
         and sources[OWNER].count("producer_input_validate_body!(") == 1,
         "actual production includes and invokes validator")
    need(sources[PROOF].count('include!("producer_input_validate_definitions_v1.rs");') == 1
         and len(re.findall(r"\binclude!\(", sources[PROOF])) == 1,
         "thin leaf root includes exact shared definitions")
    proof = "\n".join(sources[path] for path in PARTS)
    closure(sources, PROOF, FILES)
    need(len(FILES) == 6, "changed six-file executable proof closure")
    need(not re.search(r"\bmod\s+\w+\s*;", sources[PROOF] + proof + sources[BODY]),
         "no undeclared proof module")
    need(proof.count("producer_input_validate_body!(") == 1 and "producer_input_fold_body!(" not in proof,
         "actual validator, not the earlier abstract fold theorem")
    need(not re.search(r"\b(?:assume|admit|assume_specification)\b|verifier::external|\buninterp\b", proof),
         "no extra assumed native validity or opaque proof bodies")
    need("SEQUENCE_PLACEHOLDER" not in proof, "no unfinished executable proof stub")


def mutations(body):
    start = body.index("macro_rules! producer_input_validate_body {")
    prefix, selected = body[:start], body[start:]
    rows = []

    def add(name, before, after):
        need(selected.count(before) == 1 and before != after, "unique meaningful validator mutation " + name)
        rows.append((name, prefix + selected.replace(before, after)))

    for family in ("active", "queued"):
        lookup = "$observations.observe_" + family + "_lookup(reference)?"
        status = "$observations.observe_" + family + "_status(reference)?"
        cursor = "$" + family + "_index"
        add(family + "-lookup-error-substituted", lookup,
            "match $observations.observe_" + family + "_lookup(reference) { Ok(value) => value, Err(_error) => return Err(E::InvalidReference) }")
        add(family + "-status-error-substituted", status,
            "match $observations.observe_" + family + "_status(reference) { Ok(value) => value, Err(_error) => return Err(E::InvalidReference) }")
        add(family + "-lookup-duplicated", lookup + " != request",
            "{ let _first = " + lookup + "; " + lookup + " } != request")
        add(family + "-cursor-not-advanced", "*" + cursor + " += 1;", "let _unchanged_cursor = *" + cursor + ";")
        add(family + "-lookup-value-ignored", lookup + " != request", "{ let _observed = " + lookup + "; false }")
        member = ".queued_requests" if family == "queued" else ".requests"
        add(family + "-stored-request-ignored", "$root" + member + ".get(*" + cursor + ") != Some(&request)", "false")
    add("bound-root-ignored", "if !bound", "if { let _ = bound; false }")
    for branch in ("launch", "peer"):
        add(branch + "-dependency-membership-ignored",
            "producer_dependency_contains_v1(\n                                &" + branch + ".dependencies,\n                                &input.dependency,\n                            )", "true")
    add("launch-absent-root-accepted", "None => false,\n                }\n            } else",
        "None => true,\n                }\n            } else")
    add("peer-absent-root-accepted", "None => false,\n                }\n            };\n            if !bound",
        "None => true,\n                }\n            };\n            if !bound")
    add("producer-local-order-ignored", "input.dependency.submission.local >= $id.local", "false")
    add("allocation-order-ignored", "$index > 0\n                    && $root.inputs[$index - 1].source.region.allocation >= source.region.allocation", "false")
    add("record-map-ignored", "$context.allocations.get(&source.region.allocation) != Some(&source.record)", "false")
    add("backend-membership-ignored", "!$context\n                    .backend_allocations\n                    .contains(&source.record.backend_allocation)", "false")
    credit = "$observations.observe_expected_credit(\n                    source.region.allocation,\n                    source.record.device,\n                    source.record.byte_len,\n                )"
    add("credit-observation-skipped", credit, "true")
    add("credit-answer-ignored", credit, "{ let _answer = " + credit + "; true }")
    live = "$observations.observe_live(source.region.allocation, &source.record)?"
    add("live-error-substituted", live, "match $observations.observe_live(source.region.allocation, &source.record) { Ok(value) => value, Err(_error) => return Err(E::InvalidReference) }")
    add("live-observation-skipped", live, "allocation.allocation")
    add("extent-ignored", "allocation.byte_extent != source.record.byte_len", "false")
    add("offset-ignored", "byte_offset != source.region.byte_offset", "{ let _ = byte_offset; false }")
    add("length-ignored", "byte_len != source.region.byte_len", "{ let _ = byte_len; false }")
    result = {name: (value, SELECTOR) for name, value in rows}

    def scan_add(name, macro, focus, before, after):
        original = block(body, "macro_rules! " + macro + " {")
        need(original.count(before) == 1 and before != after, "unique meaningful scan mutation " + name)
        result[name] = (body.replace(original, original.replace(before, after)), focus)

    for kind, macro, values, focus in (
        ("dependency", "producer_dependency_contains_body", "$dependencies", SCAN_SELECTORS[0]),
        ("source", "producer_source_pair_contains_body", "$sources", SCAN_SELECTORS[1]),
    ):
        scan_add(kind + "-scan-match-rejected", macro, focus, "return true;", "return false;")
        scan_add(kind + "-scan-exhaustion-accepted", macro, focus, "            false\n", "            true\n")
    scan_add("dependency-scan-predicate-ignored", "producer_dependency_contains_body", SCAN_SELECTORS[0],
             "if &$dependencies[$index] == $dependency {", "if { let _ = $dependency; true } {")
    scan_add("dependency-scan-first-mismatch-terminal", "producer_dependency_contains_body", SCAN_SELECTORS[0],
             "$index += 1;", "$index += 1;\n                if $index < $dependencies.len() { return false; }")
    scan_add("dependency-scan-local-only", "producer_dependency_contains_body", SCAN_SELECTORS[0],
             "&$dependencies[$index] == $dependency", "$dependencies[$index].submission.local == $dependency.submission.local")
    scan_add("source-scan-region-ignored", "producer_source_pair_contains_body", SCAN_SELECTORS[1],
             "original.region == $source.region && original.record == $source.record", "original.record == $source.record")
    scan_add("source-scan-record-ignored", "producer_source_pair_contains_body", SCAN_SELECTORS[1],
             "original.region == $source.region && original.record == $source.record", "original.region == $source.region")
    scan_add("source-scan-first-mismatch-terminal", "producer_source_pair_contains_body", SCAN_SELECTORS[1],
             "$index += 1;", "$index += 1;\n                if $index < $sources.len() { return false; }")
    need(len(result) == MUTANT_COUNT and len({value for value, _focus in result.values()}) == MUTANT_COUNT,
         "exact distinct actual-body mutation roster")
    roster = {name: {"body": sha(text), "selector": focus} for name, (text, focus) in result.items()}
    need(sha(json.dumps(roster, sort_keys=True, separators=(",", ":"))) == MUTATION_ROSTER_SHA,
         "all 38 original native mutation names, bytes and selectors preserved")
    return result


def selection_notes(leaf, focus):
    need(focus in (SELECTOR, *SCAN_SELECTORS) and SELECTION_NOTES is not None, "selector diagnostics not yet measured")
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS, SELECTION_NOTES=SELECTION_NOTES[focus])


def controller_source():
    data = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(data).hexdigest() == BASE_SHA, "unchanged authenticated campaign owner")
    text = data.decode("utf-8")
    need(text.count('"--multiple-errors", "0"') == 1, "unique reporting-only profile adaptation")
    return text.replace('"--multiple-errors", "0"', '"--multiple-errors", "1"')


def controller():
    audit(snapshot())
    raise ValueError("changed six-file validator closure requires fresh discovery and a reviewed campaign")


def campaign():
    need(SELECTION_NOTES is not None, "selection policy not yet measured")
    return controller()


if __name__ == "__main__":
    runpy.run_path(str(ROOT / V / "test-producer-input-validate.py"))
    campaign().main()
