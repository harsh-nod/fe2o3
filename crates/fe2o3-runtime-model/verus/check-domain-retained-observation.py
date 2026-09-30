#!/usr/bin/env python3
"""Refine actual post-lock fields, not Arc/Mutex or native authority semantics."""
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
OWNER = SRC / "domain.rs"
BODY = SRC / "domain/retained_observation_body.rs"
PROOF = V / "domain_retained_observation_v1.rs"
RECORD_PROOF = V / "retained_credit_record_v1.rs"
RECORD_BODY = SRC / "retained_charge_body.rs"
VECTOR = MODEL / "resource_vector_declarations.rs"
PHASE = MODEL / "r67_resource_credits.rs"
ARENA_DECL = SRC / "domain/arena_declarations.rs"
ARENA_BODY = SRC / "domain/arena_bodies.rs"
R75_REFERENCE = V / "r75_resource_domain_arena_v1.rs"
FILES = [PROOF, RECORD_PROOF, VECTOR, RECORD_BODY, ARENA_DECL, ARENA_BODY, BODY]
EXTRA = {VECTOR, PHASE, MODEL / "lib.rs", R75_REFERENCE,
         MODEL / "request_charge_body.rs",
         Path("crates/fe2o3-resource-accounting/Cargo.toml"),
         Path("crates/fe2o3-runtime-model/Cargo.toml"), Path("Cargo.toml"), Path("Cargo.lock")}
BASE = V / "check-compute-pipeline-publication.py"
BASE_SHA = "1d4264a646983906fff5e54a2279865f5eba55413c1313698bee57064dfdfd8e"
SOURCE_TREE_SHA = "32f0abd9f75f0ab9621bf5b000e151b6ac77b0086df4878479f3206b7be3b523"
PROOF_SHA = "5c36bade68fe9e35382e85d8cbd110adfb93e0279dc439cab8eb1043fca24f16"
RECORD_PROOF_SHA = "a303f5ceeac20d3c9f1c53228562aa452e56b24d5b7052ed9982a3de8a039a5a"
# Full no-cheating discovery measured 21 obligations on the exact PROOF_SHA.
EXPECTED_VERIFIED = 21
MUTANT_COUNT = 15


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
             "ordinary exact domain-observation source")
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


def audit(sources):
    implementation = {path: text for path, text in sources.items() if path not in (PROOF, RECORD_PROOF)}
    need(set(sources) == {path for path in sources if path.is_relative_to(SRC)} | EXTRA | {PROOF, RECORD_PROOF},
         "complete accounting Rust and explicit model/build/reference roster")
    need(tree_hash(implementation) == SOURCE_TREE_SHA, "reviewed production and R75 reference bytes")
    need(sha(sources[PROOF]) == PROOF_SHA and sha(sources[RECORD_PROOF]) == RECORD_PROOF_SHA,
         "exact observer theorem and unchanged record root")
    need(include_closure(sources) == set(FILES), "exact seven-file executable proof closure")
    need(schema(sources[OWNER], "DomainRecord") == schema(sources[PROOF], "DomainRecord"),
         "exact leaf and genuine copied record schema")
    need(schema(sources[SRC / "lib.rs"], "Record") == schema(sources[RECORD_PROOF], "Record"),
         "exact native and proof Record schema")
    need(schema(sources[PHASE], "R67CreditPhaseV1") == schema(sources[RECORD_PROOF], "Phase"),
         "exact four production credit phases")
    need("R67CreditPhaseV1 as Phase" in sources[SRC / "lib.rs"], "actual phase alias")
    need('include!("domain/retained_observation_body.rs");' in sources[OWNER], "native observer include")
    invocation = "domain_retained_observation_body_v1!(nodes,profile,records,poisoned,key,slot,owner,expected)"
    need(compact(sources[OWNER]).count(invocation) == compact(sources[PROOF]).count(invocation) == 1,
         "same hook-free concrete observer body")
    native = compact(sources[OWNER])
    forwarding = ("if!Arc::ptr_eq(&self.root,root){returnfalse;}"
                  "letOk(state)=self.root.state.lock()else{returnfalse;};"
                  "domain_retained_observation_v1(&state.nodes,state.max_depth,&state.records,"
                  "state.poisoned,self.key,slot,owner,expected,)")
    need(native.count(forwarding) == 1, "exact fresh raw-lock forwarding; no recovery or cached result")
    need(native.count("resource_domain_node_body_v1!(nodes,key)") == 1,
         "actual R75 node lookup body")
    path_call = ("resource_domain_path_extract_body_v1!(resource_domain_arena_rust_expr,nodes,profile,leaf,"
                 "path,next,depth,[],[],[],[])")
    need(native.count(path_call) == 1, "actual hook-free R75 path body")
    start, stop = "spec fn live(", "spec fn fact_of("
    reference = sources[R75_REFERENCE].split(start, 1)[1].split(stop, 1)[0].strip()
    copied = sources[PROOF].split(start, 1)[1].split("spec fn observation_matches(", 1)[0].strip()
    need(reference == copied, "existing exact R75 path contracts and ghost invariants reverified here")
    need(not re.search(r"\b(?:assume|admit)\s*\(|verifier::external", sources[PROOF] + sources[RECORD_PROOF]),
         "no assumed observation or external proof body")


def mutations(body):
    cases = {}

    def add(name, before, after):
        need(body.count(before) == 1 and before != after, "one actual observation mutation: " + name)
        cases[name] = (body.replace(before, after), "*domain_retained_observation_v1")

    add("ignore-poison", "$poisoned || ", "")
    add("invert-poison", "$poisoned || ", "!$poisoned || ")
    add("ignore-ancestry", "domain_path_v1($nodes, $profile, $key).is_none()", "false")
    add("check-root-ancestry", "domain_path_v1($nodes, $profile, $key)", "domain_path_v1($nodes, $profile, ROOT)")
    add("force-four-level-profile", "domain_path_v1($nodes, $profile, $key)", "domain_path_v1($nodes, 4, $key)")
    add("drop-slot-bound", "$slot >= $records.len()", "false")
    add("off-by-one-slot-bound", "$slot >= $records.len()", "$slot > $records.len()")
    add("read-first-record", "match &$records[$slot]", "match &$records[0]")
    add("accept-empty-record", "None => false", "None => true")
    add("ignore-leaf", "record.leaf == $key", "true")
    add("ignore-leaf-generation", "record.leaf == $key", "record.leaf.slot == $key.slot")
    add("ignore-leaf-slot", "record.leaf == $key", "record.leaf.generation == $key.generation")
    add("ignore-credit-record", "record.credit.matches_retained_charge($owner, $expected)", "true")
    add("replace-owner-with-zero", "record.credit.matches_retained_charge($owner, $expected)",
        "record.credit.matches_retained_charge(0, $expected)")
    add("substitute-record-charge", "record.credit.matches_retained_charge($owner, $expected)",
        "record.credit.matches_retained_charge($owner, record.credit.charge)")
    need(len(cases) == len(set(cases.values())) == MUTANT_COUNT, "distinct actual observer controls")
    return cases


def selection_notes(leaf, focus):
    need(focus == "*domain_retained_observation_v1", "exact observer selector")
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS | {
        "precondition not met: index in bounds for this access",
    }, SELECTION_NOTES={
        "verifying root module (selected functions)",
        "verifying root module, function domain_retained_observation_v1::domain_retained_observation_v1 (selected functions)",
    })


def controller_source():
    raw = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BASE_SHA, "authenticated strict process campaign")
    source = raw.decode()
    before, after = '"--multiple-errors", "0"', '"--multiple-errors", "1"'
    need(source.count(before) == 1, "exact first-logical-error selection")
    return source.replace(before, after)


def controller():
    module = types.ModuleType("domain_retained_observation_campaign")
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
    runpy.run_path(str(ROOT / V / "test-domain-retained-observation.py"))
    campaign().main()
