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
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
SRC = Path("crates/fe2o3-runtime/src")
MODEL = Path("crates/fe2o3-runtime-model/src")
OWNER = SRC / "context/versions/producer_readers.rs"
BODY = SRC / "context/versions/producer_input_fold_body.rs"
PROOF = V / "context_producer_input_validate_v1.rs"
FILES = [PROOF, BODY]
DECLARATIONS = [MODEL / name for name in (
    "context_version_journal/declarations.rs",
    "context_version_journal/enrollment_declarations.rs",
    "context_read_leases/declarations.rs",
    "context_producer_reads/declarations.rs",
    "context_queued_writers/read_declarations.rs",
)]
BASE = V / "check-compute-pipeline-publication.py"
BASE_SHA = "1d4264a646983906fff5e54a2279865f5eba55413c1313698bee57064dfdfd8e"
SOURCE_TREE_SHA = "74c99e4ce41f591fe74e6ed9af89c5b24883af40efda7546ba62531714f476bf"
SOURCE_FILES = 332
PROOF_SHA = "8f0b816a5d9e4e08e5598a538273a9f25bb918633543a2b71a8f778ae0727c4d"
EXPECTED_VERIFIED = 42
SELECTOR = "*Observations::validate"
SCAN_SELECTORS = ("*producer_dependency_contains_v1", "*producer_source_pair_contains_v1")
SELECTION_NOTES = {
    focus: {"verifying root module (selected functions)"}
    for focus in (SELECTOR, *SCAN_SELECTORS)
}
MUTANT_COUNT = 38


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(text):
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def tree_hash(sources):
    return hashlib.sha256(json.dumps({str(path): sha(text) for path, text in sources.items()},
                                    sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def snapshot():
    paths = {PROOF, *DECLARATIONS} | {p.relative_to(ROOT) for p in (ROOT / SRC).rglob("*.rs")}
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
    proof = sources[PROOF]
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
        need(sources[PROOF].count(macro + "!(verus_exec_expr, " + values + ", " + needle + ", index,") == 1,
             "proof invokes the identical scan macro: " + name)
    need(".any(" not in sources[BODY], "membership no longer depends on concrete iterator default contracts")
    need(sources[BODY].count("producer_dependency_contains_v1(") == 2
         and sources[BODY].count("producer_source_pair_contains_v1(") == 1,
         "two dependency sites and one full source-pair site")


def audit(sources):
    implementation = {p: text for p, text in sources.items() if p != PROOF}
    need(set(sources) == ({p for p in sources if p.is_relative_to(SRC)} | set(DECLARATIONS) | {PROOF}),
         "exact runtime plus schema plus proof roster")
    need(len(implementation) == SOURCE_FILES and tree_hash(implementation) == SOURCE_TREE_SHA,
         "reviewed whole runtime and exact native value schema source")
    need(sha(sources[PROOF]) == PROOF_SHA, "reviewed exact per-input proof and contracts")
    schemas(sources)
    scan_bridges(sources)
    forwarders = {
        "observe_active_lookup": "self.versions.journal.lookup_producer_read(reference)",
        "observe_active_status": "self.versions.journal.producer_read_status(reference)",
        "observe_queued_lookup": "self.versions.journal.lookup_queued_producer_read(reference)",
        "observe_queued_status": "self.versions.journal.queued_producer_read_status(reference)",
        "observe_live": "self.versions.validate_live(id, record)",
        "observe_expected_credit": "self.context.allocation_admission.has_expected_credit(id, device, byte_len)",
    }
    for name, expression in forwarders.items():
        need(normalized(block(sources[OWNER], "fn " + name + "(")) == normalized("{" + expression + "}"),
             "one exact native typed helper forwarding call: " + name)
    need(sources[OWNER].count('include!("producer_input_fold_body.rs");') == 1
         and sources[OWNER].count("producer_input_validate_body!(") == 1,
         "actual production includes and invokes validator")
    proof = sources[PROOF]
    need(proof.count('include!("../../fe2o3-runtime/src/context/versions/producer_input_fold_body.rs");') == 1
         and len(re.findall(r"\binclude!\(", proof)) == 1
         and not re.search(r"\binclude!\(", sources[BODY]), "exact two-file executable proof closure")
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
    return result


def selection_notes(leaf, focus):
    need(focus in (SELECTOR, *SCAN_SELECTORS) and SELECTION_NOTES is not None, "selector diagnostics not yet measured")
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS, SELECTION_NOTES=SELECTION_NOTES[focus])


def controller_source():
    data = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(data).hexdigest() == BASE_SHA, "unchanged authenticated campaign owner")
    return data.decode("utf-8")


def campaign():
    audit(snapshot())
    need(type(EXPECTED_VERIFIED) is int and EXPECTED_VERIFIED == 42, "exact measured full positive count")
    need(SELECTION_NOTES is not None, "selection policy not yet measured")
    module = types.ModuleType("producer_input_validate_campaign")
    module.__file__ = str(ROOT / BASE)
    sys.modules[module.__name__] = module
    exec(compile(controller_source(), module.__file__, "exec"), module.__dict__)
    module.FILES, module.PROOF, module.BODY = FILES, PROOF, BODY
    module.EXPECTED = dict(module.EXPECTED, verified=EXPECTED_VERIFIED)
    module.mutations, module.selection_notes = mutations, selection_notes
    return module


if __name__ == "__main__":
    runpy.run_path(str(ROOT / V / "test-producer-input-validate.py"))
    campaign().main()
