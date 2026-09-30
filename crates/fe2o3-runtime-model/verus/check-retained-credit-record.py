#!/usr/bin/env python3
"""Refine the shared borrowed record predicate, not account/currentness authority.

The proof uses pinned concrete schema matching and vstd equality/array contracts.
No ledger lock, Arc identity, ancestry, retained-token ownership or freshness is
modeled. A successful record match does not authenticate a Context allocation.
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
SRC = Path("crates/fe2o3-resource-accounting/src")
OWNER = SRC / "lib.rs"
BODY = SRC / "retained_charge_body.rs"
MODEL = Path("crates/fe2o3-runtime-model/src")
VECTOR = MODEL / "resource_vector_declarations.rs"
PHASE = MODEL / "r67_resource_credits.rs"
PROOF = V / "retained_credit_record_v1.rs"
FILES = [PROOF, VECTOR, BODY]
EXTRA = {VECTOR, PHASE, MODEL / "lib.rs",
         MODEL / "request_charge_body.rs",
         Path("crates/fe2o3-resource-accounting/Cargo.toml"),
         Path("crates/fe2o3-runtime-model/Cargo.toml"),
         Path("Cargo.toml"), Path("Cargo.lock")}
BASE = V / "check-compute-pipeline-publication.py"
BASE_SHA = "1d4264a646983906fff5e54a2279865f5eba55413c1313698bee57064dfdfd8e"
SOURCE_TREE_SHA = "336ce2f3dbeb2b78c005a1e997c265a9da995ce6d0e6c9c416a406bf665353a9"
PROOF_SHA = "a303f5ceeac20d3c9f1c53228562aa452e56b24d5b7052ed9982a3de8a039a5a"
# Full no-cheating discovery measured seven obligations on the exact PROOF_SHA.
EXPECTED_VERIFIED = 7
MUTANT_COUNT = 11


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(text):
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def tree_hash(sources):
    leaves = {str(path): sha(text) for path, text in sources.items()}
    return hashlib.sha256(json.dumps(leaves, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def snapshot():
    files = {PROOF} | EXTRA | {path.relative_to(ROOT) for path in (ROOT / SRC).rglob("*.rs")}
    result = {}
    for path in files:
        selected = ROOT / path
        need(selected.is_file() and not selected.is_symlink() and selected.resolve() == selected,
             "ordinary exact retained-credit source path")
        result[path] = selected.read_bytes().decode("utf-8")
    return result


def schema(text, name):
    found = re.findall(r"\b(?:struct|enum) " + re.escape(name) + r" \{([^{}]*)\}", text)
    need(len(found) == 1, "one concrete schema: " + name)
    return re.sub(r"\s+", "", found[0]).replace("R67ResourceVectorV1", "ResourceVectorV1")


def audit(sources):
    implementation = {path: text for path, text in sources.items() if path != PROOF}
    need(set(sources) == {path for path in sources if path.is_relative_to(SRC)} | EXTRA | {PROOF},
         "complete resource-accounting Rust roster and explicit model/build inputs")
    need(tree_hash(implementation) == SOURCE_TREE_SHA, "reviewed production source roster and bytes")
    need(sha(sources[PROOF]) == PROOF_SHA, "exact record theorem and equality/compiler boundary")
    need(sources[OWNER].count('include!("retained_charge_body.rs");') == 1,
         "one production shared-body include")
    invocation = "retained_credit_record_matches_body!(self, owner, expected)"
    need(sources[OWNER].count(invocation) == sources[PROOF].count(invocation) == 1,
         "actual borrowed predicate invokes the same complete body")
    need(schema(sources[OWNER], "Record") == schema(sources[PROOF], "Record"),
         "exact Record owner/vector/phase schema")
    need(schema(sources[PHASE], "R67CreditPhaseV1") == schema(sources[PROOF], "Phase"),
         "exact four production phases")
    need("R67CreditPhaseV1 as Phase" in sources[OWNER], "production phase alias")
    need('include!("resource_vector_declarations.rs");' in sources[PHASE]
         and 'include!("../src/resource_vector_declarations.rs");' in sources[PROOF],
         "actual complete nineteen-coordinate vector declaration")
    need('include!("../../fe2o3-resource-accounting/src/retained_charge_body.rs");' in sources[PROOF]
         and len(re.findall(r"\binclude!\(", sources[PROOF])) == 2
         and not re.search(r"\binclude!\(", sources[BODY] + sources[VECTOR]),
         "exact three-file executable proof closure")
    need("pub const fn counts(&self) -> &[u64; R67_RESOURCE_DIMENSIONS_V1] {\n        &self.counts\n    }"
         in sources[PHASE], "real immutable vector projection used by partial-vector controls")
    need(not re.search(r"\b(?:assume|admit)\s*\(|verifier::external", sources[PROOF]),
         "no assumed record truth or external proof body")


def mutations(body):
    cases = {}

    def add(name, before, after):
        need(body.count(before) == 1 and before != after, "one actual predicate mutation: " + name)
        cases[name] = (body.replace(before, after), "*Record::matches_retained_charge")

    add("accept-zero-owner", "$owner != 0", "true")
    add("ignore-owner-identity", "$record.owner == $owner", "true")
    add("ignore-phase", "$record.phase == Phase::Retained", "true")
    for phase in ("Reserved", "Quarantined", "Vacant"):
        add("accept-" + phase.lower(), "Phase::Retained", "Phase::" + phase)
    add("ignore-complete-charge", "$record.charge == $expected", "true")
    add("invert-complete-charge", "$record.charge == $expected", "$record.charge != $expected")
    add("compare-first-coordinate-only", "$record.charge == $expected",
        "$record.charge.counts()[0] == $expected.counts()[0]")
    add("compare-last-coordinate-only", "$record.charge == $expected",
        "$record.charge.counts()[18] == $expected.counts()[18]")
    add("refuse-valid-record", "$owner != 0", "false")
    need(len(cases) == len(set(cases.values())) == MUTANT_COUNT, "distinct actual-body control roster")
    return cases


def selection_notes(leaf, focus):
    need(focus == "*Record::matches_retained_charge", "exact record predicate selector")
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS, SELECTION_NOTES={
        "verifying root module (selected functions)",
        "verifying root module, function retained_credit_record_v1::Record::matches_retained_charge (selected functions)",
    })


def controller_source():
    raw = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BASE_SHA, "authenticated owned-process campaign")
    source = raw.decode("utf-8")
    before, after = '"--multiple-errors", "0"', '"--multiple-errors", "1"'
    need(source.count(before) == 1, "exact first-logical-error selection")
    return source.replace(before, after)


def controller():
    module = types.ModuleType("retained_credit_record_campaign")
    module.__file__ = str(ROOT / BASE)
    sys.modules[module.__name__] = module
    exec(compile(controller_source(), module.__file__, "exec"), module.__dict__)
    module.FILES, module.PROOF, module.BODY = FILES, PROOF, BODY
    module.mutations, module.selection_notes = mutations, selection_notes
    return module


def campaign():
    audit(snapshot())
    need(type(EXPECTED_VERIFIED) is int and EXPECTED_VERIFIED > 0, "measured positive count required")
    module = controller()
    module.EXPECTED = dict(module.EXPECTED, verified=EXPECTED_VERIFIED)
    return module


if __name__ == "__main__":
    runpy.run_path(str(ROOT / V / "test-retained-credit-record.py"))
    campaign().main()
