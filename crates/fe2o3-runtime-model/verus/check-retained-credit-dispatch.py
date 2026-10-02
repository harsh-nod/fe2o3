#!/usr/bin/env python3
"""Serialized retained-credit dispatch; concrete identity/lock/map boundaries."""
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
MODEL = Path("crates/fe2o3-runtime-model/src")
RUNTIME = Path("crates/fe2o3-runtime/src")
ACCOUNTING = Path("crates/fe2o3-resource-accounting/src")
KFD = Path("crates/fe2o3-kfd/src")
PROOF = V / "retained_credit_dispatch_v1.rs"
BODIES = {
    "context": RUNTIME / "context/allocation_admission/retained_lookup_body.rs",
    "runtime": RUNTIME / "retained_credit_dispatch_body.rs",
    "account": ACCOUNTING / "retained_dispatch_body.rs",
    "domain": ACCOUNTING / "domain/retained_dispatch_body.rs",
    "composed": KFD / "resource_domains/composed/retained_dispatch_body.rs",
}
OWNERS = {
    "context": RUNTIME / "context/allocation_admission.rs",
    "runtime": RUNTIME / "resource_credits.rs",
    "account": ACCOUNTING / "lib.rs",
    "domain": ACCOUNTING / "domain.rs",
    "composed": KFD / "resource_domains/composed.rs",
}
INVOCATIONS = {
    "context": "context_retained_credit_lookup_body_v1!(self,id,device,byte_len)",
    "runtime": "runtime_retained_credit_dispatch_body_v1!(self,device,credits,expected)",
    "account": "resource_retained_credit_dispatch_body_v1!(self,credits,expected)",
    "domain": "domain_retained_credit_dispatch_body_v1!(self,root,slot,owner,expected)",
    "composed": "composed_retained_credit_dispatch_body_v1!(self,credit,bytes)",
}
SELECTORS = {
    "context": "*ContextAllocationAdmissionV1::has_expected_credit",
    "runtime": "*RuntimeResourceCreditAccountV1::matches_retained_charge_v1",
    "account": "*ResourceCreditAccountV1::matches_retained_charge_v1",
    "domain": "*DomainAccount::matches_retained_charge",
    "composed": "*Gfx942RequestAccountV1::matches_retained_charge_v1",
}
SELECTION_NOTES = {
    focus: frozenset({"verifying root module (selected functions)"})
    for focus in SELECTORS.values()
}
FILES = [PROOF, V / "retained_credit_record_v1.rs",
         MODEL / "resource_vector_declarations.rs", MODEL / "request_charge_body.rs",
         ACCOUNTING / "retained_charge_body.rs",
         ACCOUNTING / "independent_retained_observation_body.rs",
         ACCOUNTING / "domain/arena_declarations.rs", ACCOUNTING / "domain/arena_bodies.rs",
         ACCOUNTING / "domain/retained_observation_body.rs", *BODIES.values()]
EXTRA = {MODEL / "r67_resource_credits.rs", MODEL / "lib.rs",
         V / "domain_retained_observation_v1.rs", V / "independent_retained_observation_v1.rs",
         V / "request_charge_v1.rs", Path("Cargo.toml"), Path("Cargo.lock"),
         Path("rust-toolchain.toml"),
         *(Path("crates") / name / "Cargo.toml" for name in (
             "fe2o3-runtime-model", "fe2o3-runtime", "fe2o3-resource-accounting", "fe2o3-kfd"))}
BASE = V / "check-compute-pipeline-publication.py"
BASE_SHA = "1d4264a646983906fff5e54a2279865f5eba55413c1313698bee57064dfdfd8e"
SOURCE_TREE_SHA = "85eb865beae9eb84ee3d2070121677e15206dc186a8b5dfed4f6f5c9e02746c0"
PROOF_SHA = "84352d8aec33bb9b42611c01a298216dcd12ed6c355ed3ba7a7f410f4681a8ca"
EXPECTED_VERIFIED = 41
MUTANT_COUNT = 25


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(text):
    return hashlib.sha256(text.encode()).hexdigest()


def source_paths():
    return set(FILES) | EXTRA | {
        path.relative_to(ROOT) for directory in (RUNTIME, ACCOUNTING, KFD)
        for path in (ROOT / directory).rglob("*.rs")
    }


def tree_hash(sources):
    return hashlib.sha256(json.dumps({str(path): sha(text) for path, text in sources.items()},
                                    sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def snapshot():
    sources = {}
    for path in source_paths():
        selected = ROOT / path
        need(selected.is_file() and not selected.is_symlink() and selected.resolve() == selected,
             "ordinary exact dispatch input")
        sources[path] = selected.read_bytes().decode()
    return sources


def compact(text):
    return re.sub(r"\s+", "", text)


def include_closure(sources):
    reached = set()

    def visit(path):
        need(path in sources, "missing literal proof input")
        if path in reached:
            return
        reached.add(path)
        text = sources[path]
        includes = re.findall(r'\binclude!\("([^"\n]+)"\);', text)
        need(len(includes) == len(re.findall(r"\binclude!\(", text)), "literal includes only")
        for name in includes:
            need(not Path(name).is_absolute(), "relative proof include")
            visit(Path(posixpath.normpath(str(path.parent / name))))

    visit(PROOF)
    return reached


def audit(sources):
    need(set(sources) == source_paths(), "complete runtime/accounting/KFD Rust input roster")
    need(tree_hash({path: text for path, text in sources.items() if path != PROOF}) == SOURCE_TREE_SHA,
         "reviewed production/dependency bytes")
    need(sha(sources[PROOF]) == PROOF_SHA, "exact serialized dispatch root")
    need(include_closure(sources) == set(FILES), "exact fourteen-file executable closure")
    need(sources[PROOF].startswith(sources[V / "domain_retained_observation_v1.rs"]),
         "unchanged full domain proof prefix, not a weakened ancestry premise")
    proof = compact(sources[PROOF])
    independent = sources[V / "independent_retained_observation_v1.rs"]
    need(independent.count("verus! {") == 1, "one existing independent proof block")
    independent = independent.split("verus! {", 1)[1].rsplit("}", 1)[0]
    need(compact(independent) in proof, "unchanged independent observer contract and implementation")
    for name, invocation in INVOCATIONS.items():
        owner = compact(sources[OWNERS[name]])
        include = 'include!("' + str(BODIES[name].relative_to(OWNERS[name].parent)) + '");'
        need(include in sources[OWNERS[name]], "actual native body include: " + name)
        need(owner.count(invocation) == proof.count(invocation) == 1,
             "one native/proof invocation: " + name)
        need("->bool{" + invocation + "}" in owner,
             "native gate consists of shared body: " + name)
    need("pubstruct$name{context_generation:u64,local:u64,}" in compact(sources[RUNTIME / "context.rs"]),
         "actual complete device/allocation identity schema")
    for name in ("RuntimeDeviceIdV1", "RuntimeAllocationIdV1"):
        need("struct" + name + "{context_generation:u64,local:u64,}" in proof,
             "full proof identifier projection: " + name)
    native_kinds = re.findall(r"pub enum R67ResourceKindV1 \{([^}]+)\}", sources[MODEL / "r67_resource_credits.rs"])
    proof_kinds = re.findall(r"enum RuntimeResourceKindV1 \{([^}]+)\}", sources[PROOF])
    need(len(native_kinds) == len(proof_kinds) == 1
         and compact(native_kinds[0]) == compact(proof_kinds[0]), "exact nineteen-kind ordinal projection")
    need("self.counts[kindasusize]" in compact(sources[MODEL / "r67_resource_credits.rs"]),
         "actual ordinal vector getter")
    need("runtime_retained_credit_dispatch_body_v1!(self,device,credits,expected)" in proof,
         "actual runtime profile dispatch")
    need("resource_request_charge_body_v1!(bytes)" in proof,
         "actual canonical charge constructor")
    for path in (OWNERS["runtime"], OWNERS["composed"]):
        need("{fe2o3_runtime_model::r67_requested_allocation_charge_v1(bytes)}" in compact(sources[path]),
             "native constructor forwarding")
    need(not re.search(r"\b(?:assume|admit|assume_specification)\s*\(|verifier::external|\buninterp\b", sources[PROOF]),
         "no assumed observer or external adapter")


def mutations(sources):
    cases = {}

    def add(name, body, before, after):
        path = BODIES[body]
        text = sources[path]
        need(text.count(before) == 1 and before != after, "unique actual-body mutation: " + name)
        cases[name] = (path, text.replace(before, after), SELECTORS[body])

    add("ordinary-rejected", "context", "(None, None) => true,", "(None, None) => false,")
    add("one-sided-accepted", "context", "_ => false,", "_ => true,")
    add("lookup-bytes-zero", "context", "request_charge($byte_len)", "request_charge({ let _ = $byte_len; 0 })")
    add("lookup-bytes-max", "context", "request_charge($byte_len)", "request_charge({ let _ = $byte_len; u64::MAX })")
    add("device-inverted", "runtime", "$this.device == $device", "$this.device != $device")
    add("device-ignored", "runtime", "$this.device == $device", "({ let _ = $device; true })")
    add("device-generation-ignored", "runtime", "$this.device == $device", "$this.device.get() == $device.get()")
    add("composed-shape-ignored", "runtime", "$expected == request_charge(bytes)", "true")
    add("runtime-mixed-variant-accepted", "runtime", "_ => false,", "_ => true,")
    add("general-observer-ignored", "runtime", "account.matches_retained_charge_v1(credits, $expected)", "{ let _ = (account, credits, $expected); true }")
    add("composed-wrong-byte-forwarding", "runtime", "matches_retained_charge_v1(credits, bytes)", "matches_retained_charge_v1(credits, 0)")
    add("missing-token-accepted", "account", "let Some(token) = &$credits.token else {\n            return false;", "let Some(token) = &$credits.token else {\n            return true;")
    add("independent-identity-ignored", "account", "if Arc::ptr_eq(account, actual)", "if { let _ = actual; true }")
    add("independent-identity-inverted", "account", "if Arc::ptr_eq(account, actual)", "if !Arc::ptr_eq(account, actual)")
    add("independent-lock-failure-accepted", "account", "let Ok(state) = account.state.lock() else {\n                    return false;", "let Ok(state) = account.state.lock() else {\n                    return true;")
    add("domain-token-slot-substituted", "account", "root, token.slot, token.owner", "root, usize::MAX, token.owner")
    add("domain-identity-ignored", "domain", "!Arc::ptr_eq(&$this.root, $root)", "{ let _ = $root; false }")
    add("domain-identity-inverted", "domain", "!Arc::ptr_eq(&$this.root, $root)", "Arc::ptr_eq(&$this.root, $root)")
    add("domain-lock-failure-accepted", "domain", "let Ok(state) = $this.root.state.lock() else {\n            return false;", "let Ok(state) = $this.root.state.lock() else {\n            return true;")
    add("domain-owner-substituted", "domain", "            $owner,", "            { let _ = $owner; 0 },")
    add("domain-slot-substituted", "domain", "            $slot,", "            { let _ = $slot; usize::MAX },")
    add("typed-identity-ignored", "composed", "Arc::ptr_eq(&$this.0, &$credit.account.0)", "true")
    add("typed-identity-inverted", "composed", "Arc::ptr_eq(&$this.0, &$credit.account.0)", "!Arc::ptr_eq(&$this.0, &$credit.account.0)")
    add("typed-bytes-zero", "composed", "request_charge($bytes)", "request_charge({ let _ = $bytes; 0 })")
    add("typed-observer-ignored", "composed", "$this\n                .0\n                .account\n                .matches_retained_charge_v1(&$credit.inner, request_charge($bytes))", "{ let _ = $bytes; true }")
    need(len(cases) == len(set(cases.values())) == MUTANT_COUNT, "exact distinct five-body mutation roster")
    return cases


def selection_notes(leaf, focus):
    need(focus in SELECTORS.values(), "exact dispatch method selector")
    need(type(SELECTION_NOTES) is dict and set(SELECTION_NOTES) == set(SELECTORS.values()),
         "complete measured five-selector policy")
    notes = SELECTION_NOTES[focus]
    need(type(notes) is frozenset and notes == {"verifying root module (selected functions)"},
         "only the exact per-selector observed singleton")
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS, SELECTION_NOTES=notes)


def controller_source():
    raw = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BASE_SHA, "authenticated strict controller")
    source = raw.decode()
    before, after = '"--multiple-errors", "0"', '"--multiple-errors", "1"'
    need(source.count(before) == 1, "exact first-error selector policy")
    return source.replace(before, after)


def controller():
    module = types.ModuleType("retained_credit_dispatch_campaign")
    module.__file__ = str(ROOT / BASE)
    sys.modules[module.__name__] = module
    exec(compile(controller_source(), module.__file__, "exec"), module.__dict__)
    module.FILES, module.PROOF = FILES, PROOF
    module.selection_notes = selection_notes
    module.BODY = None
    module.mutations = mutations
    module.main = multi_body_recorder_required
    return module


def multi_body_recorder_required(*_args, **_kwargs):
    raise ValueError("multi-body proof execution requires the separately reviewed bounded recorder")


def calibration_arguments(arguments):
    need(arguments == ["--calibrate"], "source calibration requires exactly --calibrate")


def cli(arguments):
    if arguments == ["--calibrate"]:
        runpy.run_path(str(ROOT / V / "test-retained-credit-dispatch.py"))
        return 0
    need(arguments and arguments[0] == "--campaign" and "--calibrate" not in arguments,
         "use --calibrate or --campaign --verus ABS --output FRESH_ABS")
    runner = ROOT / V / "run-retained-credit-dispatch.py"
    module = types.ModuleType("retained_credit_dispatch_runner")
    module.__file__ = str(runner)
    sys.modules[module.__name__] = module
    exec(compile(runner.read_bytes(), str(runner), "exec"), module.__dict__)
    return module.main(arguments)


def campaign():
    audit(snapshot())
    need(type(EXPECTED_VERIFIED) is int and EXPECTED_VERIFIED > 0, "full discovery count is not measured")
    module = controller()
    module.EXPECTED = dict(module.EXPECTED, verified=EXPECTED_VERIFIED)
    return module


if __name__ == "__main__":
    raise SystemExit(cli(sys.argv[1:]))
