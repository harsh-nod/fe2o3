#!/usr/bin/env python3
"""Refine actual independent post-lock fields, not Arc/Mutex or authority."""
import hashlib
import json
from pathlib import Path
import posixpath
import re
import runpy
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
SRC = Path("crates/fe2o3-resource-accounting/src")
MODEL = Path("crates/fe2o3-runtime-model/src")
DISPATCH = SRC / 'retained_dispatch_body.rs'
OWNER = SRC / "lib.rs"
BODY = SRC / "independent_retained_observation_body.rs"
PROOF = V / "independent_retained_observation_v1.rs"
RECORD_PROOF = V / "retained_credit_record_v1.rs"
RECORD_BODY = SRC / "retained_charge_body.rs"
VECTOR = MODEL / "resource_vector_declarations.rs"
PHASE = MODEL / "r67_resource_credits.rs"
FILES = [PROOF, RECORD_PROOF, VECTOR, RECORD_BODY, BODY]
EXTRA = {VECTOR, PHASE, MODEL / "lib.rs",
         MODEL / "request_charge_body.rs",
         Path("crates/fe2o3-resource-accounting/Cargo.toml"),
         Path("crates/fe2o3-runtime-model/Cargo.toml"), Path("Cargo.toml"), Path("Cargo.lock")}
BASE = V / "check-compute-pipeline-publication.py"
BASE_SHA = "1d4264a646983906fff5e54a2279865f5eba55413c1313698bee57064dfdfd8e"
SOURCE_TREE_SHA = "e7b90e576e829c06cd2270c05f12fe68d1fc3a54e3320561932d9a83ae2d6b23"
PROOF_SHA = "67bdf02f8ebf5d6fb51e1d29e9ed8e392698f676a33bf56038b96a5710aa1078"
RECORD_PROOF_SHA = "a303f5ceeac20d3c9f1c53228562aa452e56b24d5b7052ed9982a3de8a039a5a"
# Full no-cheating discovery measured eight obligations on this exact PROOF_SHA.
# Discovery result: ffbbdbb6323e3b54762c2ee43366ae7bc341450645b788ff256514138dad69e7.
EXPECTED_VERIFIED = 8
MUTANT_COUNT = 10


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(text):
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def tree_hash(sources):
    leaves = {str(path): sha(text) for path, text in sources.items()}
    return hashlib.sha256(json.dumps(leaves, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def snapshot():
    files = set(FILES) | EXTRA | {path.relative_to(ROOT) for path in (ROOT / SRC).rglob("*.rs")}
    result = {}
    for path in files:
        selected = ROOT / path
        need(selected.is_file() and not selected.is_symlink() and selected.resolve() == selected,
             "ordinary exact independent-observation source")
        result[path] = selected.read_bytes().decode("utf-8")
    return result


def compact(text):
    return re.sub(r"\s+", "", text)


def schema(text, name):
    found = re.findall(r"\b(?:struct|enum) " + re.escape(name) + r" \{([^{}]*)\}", text)
    need(len(found) == 1, "one concrete schema: " + name)
    return compact(found[0]).replace("R67ResourceVectorV1", "ResourceVectorV1")


def include_closure(sources):
    visited = set()

    def visit(path):
        need(path in sources, "included source is missing")
        if path in visited:
            return
        visited.add(path)
        text = sources[path]
        includes = re.findall(r'\binclude!\("([^"\n]+)"\);', text)
        need(len(includes) == len(re.findall(r"\binclude!\(", text)), "literal include closure only")
        for name in includes:
            need(not Path(name).is_absolute(), "relative proof include only")
            visit(Path(posixpath.normpath(str(path.parent / name))))

    visit(PROOF)
    return visited


def audit_dispatch_forwarding(sources):
    native = compact(sources[OWNER])
    invocation = 'resource_retained_credit_dispatch_body_v1!(self,credits,expected)'
    need(sources[OWNER].count('include!("retained_dispatch_body.rs");') == 1
         and native.count(invocation) == 1 and "->bool{" + invocation + "}" in native,
         "one exact native shared dispatch invocation and include")
    dispatch = compact(sources[DISPATCH])
    need(dispatch.startswith('macro_rules!resource_retained_credit_dispatch_body_v1{($this:ident,$credits:ident,$expected:ident)=>{{') and dispatch.endswith("}};}")
         and dispatch.count("macro_rules!") == 1, "one ident-only dispatch macro")
    forwarded = dispatch
    for argument, value in (('this', 'self'), ('credits', 'credits'), ('expected', 'expected')):
        forwarded = forwarded.replace("$" + argument, value)
    need("$" not in forwarded, "only the exact bound macro arguments")
    forwarding = ("letSome(token)=&credits.tokenelse{returnfalse;};"
                  "match(&self.0,&token.account){"
                  "(AccountHandle::Independent(account),TokenAccount::Independent(actual))"
                  "ifArc::ptr_eq(account,actual)=>{"
                  "letOk(state)=account.state.lock()else{returnfalse;};"
                  "independent_retained_observation_v1(&state.records,state.poisoned,"
                  "token.slot,token.owner,expected,)")
    need(forwarded.count(forwarding) == 1, "exact borrowed token, identity, raw lock and actual-field forwarding")


def audit(sources):
    implementation = {path: text for path, text in sources.items() if path not in (PROOF, RECORD_PROOF)}
    need(set(sources) == {path for path in sources if path.is_relative_to(SRC)} | EXTRA | {PROOF, RECORD_PROOF},
         "complete accounting Rust and explicit model/build roster")
    need(tree_hash(implementation) == SOURCE_TREE_SHA, "reviewed production and build bytes")
    need(sha(sources[PROOF]) == PROOF_SHA and sha(sources[RECORD_PROOF]) == RECORD_PROOF_SHA,
         "exact observer theorem and unchanged record root")
    need(include_closure(sources) == set(FILES), "exact five-file executable proof closure")
    need(schema(sources[OWNER], "Record") == schema(sources[RECORD_PROOF], "Record"),
         "exact native and proof Record schema")
    need(schema(sources[PHASE], "R67CreditPhaseV1") == schema(sources[RECORD_PROOF], "Phase"),
         "exact four production credit phases")
    need("R67CreditPhaseV1 as Phase" in sources[OWNER], "actual phase alias")
    need('include!("independent_retained_observation_body.rs");' in sources[OWNER], "native observer include")
    invocation = "independent_retained_observation_body_v1!(records,poisoned,slot,owner,expected)"
    need(compact(sources[OWNER]).count(invocation) == compact(sources[PROOF]).count(invocation) == 1,
         "same hook-free concrete observer body")
    audit_dispatch_forwarding(sources)
    need(not re.search(r"\b(?:assume|admit)\s*\(|verifier::external", sources[PROOF] + sources[RECORD_PROOF]),
         "no assumed observation or external proof body")


def mutations(body):
    cases = {}

    def add(name, before, after):
        need(body.count(before) == 1 and before != after, "one actual observation mutation: " + name)
        cases[name] = (body.replace(before, after), "*independent_retained_observation_v1")

    add("ignore-poison", "if $poisoned {", "if false {")
    add("invert-poison", "if $poisoned {", "if !$poisoned {")
    add("drop-slot-bound", "$slot >= $records.len()", "false")
    add("off-by-one-slot-bound", "$slot >= $records.len()", "$slot > $records.len()")
    add("read-first-record", "match &$records[$slot]", "match &$records[0]")
    add("accept-empty-record", "None => false", "None => true")
    add("ignore-credit-record", "record.matches_retained_charge($owner, $expected)", "{ let _ = record; true }")
    add("replace-owner-with-zero", "record.matches_retained_charge($owner, $expected)",
        "record.matches_retained_charge(0, $expected)")
    add("substitute-record-owner", "record.matches_retained_charge($owner, $expected)",
        "record.matches_retained_charge(record.owner, $expected)")
    add("substitute-record-charge", "record.matches_retained_charge($owner, $expected)",
        "record.matches_retained_charge($owner, record.charge)")
    need(len(cases) == len(set(cases.values())) == MUTANT_COUNT, "distinct actual observer controls")
    return cases


def selection_notes(leaf, focus):
    need(focus == "*independent_retained_observation_v1", "exact observer selector")
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS | {
        "precondition not met: index in bounds for this access",
    }, SELECTION_NOTES={
        "verifying root module (selected functions)",
        "verifying root module, function independent_retained_observation_v1::independent_retained_observation_v1 (selected functions)",
    })


def controller_source():
    raw = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BASE_SHA, "authenticated strict process campaign")
    source = raw.decode()
    before, after = '"--multiple-errors", "0"', '"--multiple-errors", "1"'
    need(source.count(before) == 1, "exact first-logical-error selection")
    return source.replace(before, after)


def controller():
    module = types.ModuleType("independent_retained_observation_campaign")
    module.__file__ = str(ROOT / BASE)
    sys.modules[module.__name__] = module
    exec(compile(controller_source(), module.__file__, "exec"), module.__dict__)
    module.FILES, module.PROOF, module.BODY = FILES, PROOF, BODY
    module.mutations, module.selection_notes = mutations, selection_notes
    return module


def campaign():
    audit(snapshot())
    need(type(EXPECTED_VERIFIED) is int and EXPECTED_VERIFIED > 0, "measured full-root positive count required")
    module = controller()
    module.EXPECTED = dict(module.EXPECTED, verified=EXPECTED_VERIFIED)
    return module


if __name__ == "__main__":
    runpy.run_path(str(ROOT / V / "test-independent-retained-observation.py"))
    campaign().main()
