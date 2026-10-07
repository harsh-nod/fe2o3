#!/usr/bin/env python3
"""Construct exact full-root diagnostic inputs; construction qualifies no kills."""
import hashlib
import json
from pathlib import Path
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
GUARD = V / "check-producer-journal-composition.py"
GUARD_SHA = '298cd082d5d0e88252b6df47395ecb4de63e7a30709b513fb7b80cb4558ae0c0'
COUNTS = {"leaf": 38, "conditional": 21, "concrete": 30}
ROOT_COUNTS = {"leaf": 42, "conditional": 64, "concrete": 218}
ADAPTER_NAMES = (
    "per-input-answer-index-zero", "actual-launch-flag-inverted",
    "actual-submission-generation-zero", "actual-family-cursors-swapped",
    "returned-error-substituted", "returned-pending-demoted", "returned-unknown-demoted",
    "consumed-only-on-success", "receipt-advance-derived-from-success",
    "trace-prefix-dropped", "trace-prefix-reversed", "receipt-credit-answer-inverted",
    "observed-credit-allocation-generation-zero", "observed-credit-allocation-local-zero",
    "observed-credit-device-generation-zero", "observed-credit-device-local-zero",
    "observed-credit-byte-length-zero", "actual-active-count-cross-wired",
    "actual-queued-count-cross-wired", "composed-unknown-stops-later-validation",
    "composed-error-swallowed",
)
FORWARD_NAMES = (
    "active_lookup-constant-error", "active_lookup-error-coerced", "active_lookup-prefetch-status",
    "active_status-constant-error", "active_status-error-coerced",
    "queued_lookup-constant-error", "queued_lookup-error-coerced",
    "queued_status-constant-error", "queued_status-error-coerced",
)
ROSTER_SHA = "5c504d6df333222ec3279e35f1f8fb549e50a2943e92debef58f8b4b6a24d137"


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(text):
    return hashlib.sha256(text.encode()).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def guard():
    path = ROOT / GUARD
    need(path.is_file() and not path.is_symlink() and path.resolve() == path, "ordinary source guard")
    raw = path.read_bytes()
    need(hashlib.sha256(raw).hexdigest() == GUARD_SHA, "exact frozen concrete guard")
    module = types.ModuleType("frozen_concrete_guard")
    module.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), module.__dict__)
    return module


def span(text, scope, anchor, leaf):
    """Use the inherited pinned-source block helper, not a general Rust parser."""
    need(not scope or text.count(scope) == 1, "unique implementation scope")
    base = text.index(scope) if scope else 0
    part = text[base:]
    block = leaf.block(part, anchor)
    start = base + part.index("{", part.index(anchor))
    need(text[start:start + len(block)] == block, "exact implementation block")
    return start, start + len(block)


def in_body(original, changed, region):
    start, end = region
    need(0 <= start < end <= len(original) and original[start] == "{" and original[end - 1] == "}",
         "bounded implementation body")
    tail = original[end:]
    need(original != changed and changed[:start] == original[:start]
         and (changed[-len(tail):] == tail if tail else True),
         "nonvacuous change only inside intended body; contracts and other source unchanged")


def replacement(original, before, after):
    need(before != after and original.count(before) == 1, "one exact nonvacuous replacement")
    return original.replace(before, after)


def construct(sources=None):
    g = guard()
    sources = g.snapshot() if sources is None else dict(sources)
    g.audit(sources)
    frozen = dict(sources)
    leaf = g.helper("check-producer-input-validate.py")
    conditional = g.helper("check-producer-input-composition.py")
    wrapper = g.helper("check-producer-journal-observers.py")
    closures = {"leaf": leaf.FILES, "conditional": conditional.FILES, "concrete": g.files()}
    roots = {"leaf": leaf.PROOF, "conditional": conditional.PROOF, "concrete": g.PROOF}
    need({family: len(paths) for family, paths in closures.items()} ==
         {"leaf": 7, "conditional": 9, "concrete": 47}, "exact changed proof closures")
    rows = {}

    def add(family, name, path, changed, boundary, scope, anchor, origin_selector=None,
            label="implementation-contract", before=None, after=None):
        key = family + "/" + name
        original = sources[path]
        region = span(original, scope, anchor, leaf)
        in_body(original, changed, region)
        altered = span(changed, scope, anchor, leaf)
        need(altered[0] == region[0] and changed[altered[1]:] == original[region[1]:],
             "changed implementation block retains exact external suffix")
        need(key not in rows and path in closures[family], "unique case inside actual root closure")
        if before is None:
            before, after = original[region[0]:region[1]], changed[altered[0]:altered[1]]
        need(replacement(original, before, after) == changed, "exact source replacement custody")
        rows[key] = {"family": family, "name": name, "path": path, "text": changed,
                     "original_sha256": sha(original), "before": before, "after": after,
                     "root": roots[family], "closure": tuple(closures[family]),
                     "origin_selector": origin_selector, "capture_selector": None,
                     "boundary": boundary, "boundary_label": label,
                     "implementation_span": region, "implementation_scope": scope,
                     "implementation_anchor": anchor}

    for name, (text, selector) in leaf.mutations(sources[leaf.BODY]).items():
        if selector == leaf.SELECTOR:
            macro, boundary = "producer_input_validate_body", "Observations::validate"
        elif selector == leaf.SCAN_SELECTORS[0]:
            macro, boundary = "producer_dependency_contains_body", "producer_dependency_contains_v1"
        else:
            need(selector == leaf.SCAN_SELECTORS[1], "known original leaf selector")
            macro, boundary = "producer_source_pair_contains_body", "producer_source_pair_contains_v1"
        add("leaf", name, leaf.BODY, text, boundary, "", "macro_rules! " + macro + " {", selector)

    originals = conditional.mutations(conditional.snapshot())
    need(tuple(originals) == ADAPTER_NAMES, "all original 21 conditional names and order")
    for name, row in originals.items():
        boundary = row["boundary"]
        anchor = ("macro_rules! producer_input_fold_body {" if row["path"] == conditional.BODY
                  else "fn " + boundary.split("::")[-1] + "(")
        add("conditional", name, row["path"], row["text"], boundary, "", anchor,
            row["selector"], before=row["before"], after=row["after"])

    def concrete(name, before, after, method="validate", observer=False, label="implementation-contract"):
        path = g.PROOF
        owner = "Observations" if observer else "Composition"
        scope = "struct " + owner + "<'a,"
        add("concrete", name, path, replacement(sources[path], before, after),
            "concrete_composition::" + owner + "::" + method, scope, "fn " + method + "(",
            label=label, before=before, after=after)

    concrete("per-input-answer-index-zero", "let external = self.returns[index];", "let external = self.returns[0];")
    constructor = "root: self.root, id: self.id, consumer: self.consumer, launch: self.launch,"
    concrete("actual-launch-flag-inverted", constructor, constructor.replace("launch: self.launch", "launch: !self.launch"))
    concrete("actual-submission-generation-zero", constructor,
             constructor.replace("id: self.id,", "id: RuntimeSubmissionIdV1 { context_generation: 0, local: self.id.local },"))
    call = "let result = validation.validate(index, active, queued);"
    concrete("actual-family-cursors-swapped", call, call.replace("index, active, queued", "index, queued, active"))
    ending = "            result\n        }\n\n        #[verifier::spinoff_prover]\n        fn reconcile"
    for name, value in (
        ("returned-error-substituted", "match result { Ok(status) => Ok(status), Err(_) => Err(ContextVersionJournalErrorV1::InvalidReference) }"),
        ("returned-pending-demoted", "match result { Ok(ContextProducerReadStatusV1::Pending) => Ok(ContextProducerReadStatusV1::Success), other => other }"),
        ("returned-unknown-demoted", "match result { Ok(ContextProducerReadStatusV1::Unknown) => Ok(ContextProducerReadStatusV1::NoEffect), other => other }"),
    ):
        concrete(name, ending, ending.replace("            result\n", "            " + value + "\n"))
    concrete("consumed-only-on-success", "self.consumed@ = (index + 1) as nat;",
             "self.consumed@ = if result is Ok { (index + 1) as nat } else { index as nat };")
    receipt = ("self.receipts@ = self.receipts@.push(receipt(self.root.inputs@[index as int].request,\n"
               "                    active_before, queued_before, *active, *queued, result, validation.calls@, external.credit));")
    changed = ("let actual = receipt(self.root.inputs@[index as int].request,\n"
               "                    active_before, queued_before, *active, *queued, result, validation.calls@, external.credit);\n"
               "                self.receipts@ = self.receipts@.push(fold::InputObservation {\n"
               "                    family: actual.family, advanced: result is Ok, result: actual.result, credit: actual.credit });")
    concrete("receipt-advance-derived-from-success", receipt, changed)
    concrete("trace-prefix-dropped", "self.calls@ = self.calls@ + validation.calls@;", "self.calls@ = validation.calls@;")
    concrete("trace-prefix-reversed", "self.calls@ = self.calls@ + validation.calls@;", "self.calls@ = validation.calls@ + self.calls@;")
    concrete("receipt-credit-answer-inverted", receipt, receipt.replace("external.credit", "!external.credit"))
    append = "proof { self.calls@ = self.calls@.push(Call::Credit(allocation, device, bytes)); }"
    for name, value in (
        ("allocation-generation", "RuntimeAllocationIdV1 { context_generation: 0, local: allocation.local }, device, bytes"),
        ("allocation-local", "RuntimeAllocationIdV1 { context_generation: allocation.context_generation, local: 0 }, device, bytes"),
        ("device-generation", "allocation, RuntimeDeviceIdV1 { context_generation: 0, local: device.local }, bytes"),
        ("device-local", "allocation, RuntimeDeviceIdV1 { context_generation: device.context_generation, local: 0 }, bytes"),
        ("byte-length", "allocation, device, 0"),
    ):
        concrete("observed-credit-" + name + "-zero", append,
                 append.replace("allocation, device, bytes", value), "observe_expected_credit", True,
                 "external-credit-argument-ghost-trace")
    concrete("actual-active-count-cross-wired", "{ self.root.references.len() }",
             "{ self.root.queued_references.len() }", "active_count")
    concrete("actual-queued-count-cross-wired", "{ self.root.queued_references.len() }",
             "{ self.root.references.len() }", "queued_count")
    for name in ADAPTER_NAMES[-2:]:
        row = originals[name]
        add("concrete", name, row["path"], row["text"], "concrete_composition::Composition::reconcile",
            "", "macro_rules! producer_input_fold_body {", before=row["before"], after=row["after"])
    forwards = wrapper.mutations(sources[wrapper.BODY])
    need(tuple(forwards) == FORWARD_NAMES, "exact nine actual forwarding faults")
    for name, (text, method) in forwards.items():
        kind = name.split("-")[0]
        add("concrete", name, wrapper.BODY, text, method.replace("native_observers::", "concrete_composition::"),
            "", "macro_rules! producer_observe_" + kind + "_body_v1 {", method,
            "wrapper-ghost-trace-only" if name == "active_lookup-prefetch-status" else "actual-journal-result-equality")
    need(sources == frozen and g.snapshot() == frozen, "construction leaves complete source unchanged")
    validate(rows, sources, leaf)
    return rows, sources


def inventory(rows):
    return {key: {**{name: row[name] for name in (
        "family", "name", "original_sha256", "origin_selector", "capture_selector", "boundary",
        "boundary_label", "implementation_span", "implementation_scope", "implementation_anchor")},
        "path": str(row["path"]), "root": str(row["root"]),
        "closure": [str(path) for path in row["closure"]], "sha256": sha(row["text"]),
        "before_sha256": sha(row["before"]), "after_sha256": sha(row["after"])} for key, row in rows.items()}


def validate(rows, sources, leaf):
    need({family: sum(row["family"] == family for row in rows.values()) for family in COUNTS} == COUNTS,
         "exact 38/21/30 full-root case counts")
    need(len(rows) == 89 and len({(row["family"], row["path"], row["text"]) for row in rows.values()}) == 89,
         "89 distinct root/body cases")
    for key, row in rows.items():
        need(key == row["family"] + "/" + row["name"] and row["capture_selector"] is None,
             "canonical full-root case; original selector is provenance only")
        original = sources[row["path"]]
        need(sha(original) == row["original_sha256"] and row["path"] in row["closure"]
             and row["root"] in row["closure"], "source/root/changed-input custody")
        need(replacement(original, row["before"], row["after"]) == row["text"], "exact mutation delta")
        need(span(original, row["implementation_scope"], row["implementation_anchor"], leaf) == row["implementation_span"],
             "exact intended body coordinates")
        in_body(original, row["text"], row["implementation_span"])
    for name in FORWARD_NAMES:
        expected = "wrapper-ghost-trace-only" if name == "active_lookup-prefetch-status" else "actual-journal-result-equality"
        need(rows["concrete/" + name]["boundary_label"] == expected, "eight result-equality versus one ghost-only scope")
    need({row["name"] for row in rows.values() if row["family"] == "conditional"} == set(ADAPTER_NAMES)
         and {row["name"] for row in rows.values() if row["family"] == "concrete"} == set(ADAPTER_NAMES + FORWARD_NAMES),
         "complete unfiltered conditional and concrete rosters")


def checked():
    rows, sources = construct()
    need(sha(canonical(inventory(rows))) == ROSTER_SHA, "reviewed immutable 89-case roster")
    return rows, sources


if __name__ == "__main__":
    cases, sources = checked()
    print(json.dumps({"source_only": True, "qualified_kills": 0, "counts": COUNTS,
                      "roster_sha256": ROSTER_SHA, "source_files": len(sources),
                      "full_root_only": True}, sort_keys=True))
