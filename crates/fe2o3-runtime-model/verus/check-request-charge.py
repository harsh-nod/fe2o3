#!/usr/bin/env python3
"""Exact production request-profile construction, not live credit authority."""
import hashlib
import json
from pathlib import Path
import posixpath
import re
import runpy
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
MODEL = Path("crates/fe2o3-runtime-model")
V = MODEL / "verus"
SRC = MODEL / "src"
OWNER = SRC / "r67_resource_credits.rs"
BODY = SRC / "request_charge_body.rs"
VECTOR = SRC / "resource_vector_declarations.rs"
PROOF = V / "request_charge_v1.rs"
RUNTIME = Path("crates/fe2o3-runtime/src/resource_credits.rs")
KFD = Path("crates/fe2o3-kfd/src/resource_domains/composed.rs")
KFD_TEST = Path("crates/fe2o3-kfd/src/resource_domains/composed/tests.rs")
ACCOUNTING = Path("crates/fe2o3-resource-accounting/src/lib.rs")
FILES = [PROOF, VECTOR, BODY]
SOURCES = frozenset(FILES) | frozenset((
    OWNER, SRC / "lib.rs", RUNTIME, KFD, KFD_TEST, ACCOUNTING,
    MODEL / "Cargo.toml", Path("crates/fe2o3-runtime/Cargo.toml"),
    Path("crates/fe2o3-kfd/Cargo.toml"), Path("crates/fe2o3-resource-accounting/Cargo.toml"),
    Path("Cargo.toml"), Path("Cargo.lock"), Path("rust-toolchain.toml"),
))
BASE = V / "check-compute-pipeline-publication.py"
BASE_SHA = "1d4264a646983906fff5e54a2279865f5eba55413c1313698bee57064dfdfd8e"
SOURCE_TREE_SHA = "08ee4e214ae674a4edf2e4f0644e2baaf961eb257a59681cb581772a08e94981"
PROOF_SHA = "762360bdc1d84b360c16546b1669f040316c28fcd98b1f0005fd3d9de1516faf"
# Full no-cheating discovery measured three obligations on the exact PROOF_SHA.
EXPECTED_VERIFIED = 3
MUTANT_COUNT = 21
KINDS = (
    "LogicalPayloadBytes", "RequestedAllocationBytes", "ResidentHostAllocationBytes",
    "ResidentDeviceAllocationBytes", "ExecutableHostImageBytes", "ExecutableDeviceBytes",
    "ControlResidentBytes", "QueueResidentBytes", "SignalResidentBytes", "KernargResidentBytes",
    "QueueSlots", "SignalSlots", "KernargSlots", "OperationSlots", "ReplyBytes", "ReplyCells",
    "TerminalRecordBytes", "QuarantineBookkeepingBytes", "AllocationRecords",
)


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(text):
    return hashlib.sha256(text.encode()).hexdigest()


def tree_hash(sources):
    leaves = {str(path): sha(text) for path, text in sources.items()}
    return hashlib.sha256(json.dumps(leaves, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def snapshot():
    result = {}
    for path in SOURCES:
        selected = ROOT / path
        need(selected.is_file() and not selected.is_symlink() and selected.resolve() == selected,
             "ordinary exact request-profile source")
        result[path] = selected.read_bytes().decode()
    return result


def compact(text):
    return re.sub(r"\s+", "", text)


def include_closure(sources):
    reached = set()

    def visit(path):
        need(path in sources, "missing literal include")
        if path in reached:
            return
        reached.add(path)
        text = sources[path]
        includes = re.findall(r'\binclude!\("([^"\n]+)"\);', text)
        need(len(includes) == len(re.findall(r"\binclude!\(", text)), "literal proof includes only")
        for name in includes:
            need(not Path(name).is_absolute(), "relative proof include")
            visit(Path(posixpath.normpath(str(path.parent / name))))

    visit(PROOF)
    return reached


def audit(sources):
    need(set(sources) == SOURCES, "exact declared model/wrapper/build input roster")
    need(tree_hash({path: text for path, text in sources.items() if path != PROOF}) == SOURCE_TREE_SHA,
         "reviewed production and build bytes")
    need(sha(sources[PROOF]) == PROOF_SHA, "exact arbitrary-byte vector theorem")
    need(include_closure(sources) == set(FILES), "exact three-file executable closure")
    owner = compact(sources[OWNER])
    kinds = "#[repr(usize)]pubenumR67ResourceKindV1{" + ",".join(KINDS) + ",}"
    need(owner.count(kinds) == 1, "exact actual nineteen-coordinate ordinal schema")
    need(compact(sources[VECTOR]).count("pubconstR67_RESOURCE_DIMENSIONS_V1:usize=19;") == 1,
         "exact native vector dimension")
    need(compact(sources[VECTOR]).count("counts:[u64;R67_RESOURCE_DIMENSIONS_V1],") == 1,
         "actual native coordinate representation")
    need('include!("request_charge_body.rs");' in sources[OWNER]
         and 'include!("resource_vector_declarations.rs");' in sources[OWNER],
         "native constructor uses actual shared declarations and body")
    invocation = "resource_request_charge_body_v1!(bytes)"
    need(owner.count(invocation) == compact(sources[PROOF]).count(invocation) == 1,
         "one identical pure constructor body at each call site")
    need("pubconstfnr67_requested_allocation_charge_v1(bytes:u64)->R67ResourceVectorV1{" + invocation + "}" in owner,
         "actual native helper, not a detached proof-only constructor")
    for path, result_type, visibility in ((RUNTIME, "RuntimeResourceVectorV1", "pub(crate)"),
                                         (KFD, "ResourceVectorV1", "")):
        forwarding = (visibility + "fnrequest_charge(bytes:u64)->" + result_type
                      + "{fe2o3_runtime_model::r67_requested_allocation_charge_v1(bytes)}")
        need(compact(sources[path]).count(forwarding) == 1, "exact native request wrapper: " + str(path))
    need("pubuser67_resource_credits::*;" in compact(sources[SRC / "lib.rs"]), "existing model wildcard export")
    need("R67ResourceKindV1asResourceKindV1,R67ResourceVectorV1asResourceVectorV1," in compact(sources[ACCOUNTING]),
         "actual public accounting aliases")
    need(not re.search(r"\b(?:assume|admit|assume_specification)\b|verifier::external|\buninterp\b", sources[PROOF]),
         "no assumed vector or external proof adapter")


def mutations(body):
    array = re.findall(r"counts: \[([^\]]*)\]", body)
    need(len(array) == 1, "one concrete array constructor")
    values = [part.strip() for part in array[0].split(",")]
    need(values == ["0", "$bytes", *(["0"] * 16), "1"], "exact production profile coordinates")
    cases = {}

    def add(name, index, value):
        changed = list(values)
        changed[index] = value
        replacement = body.replace(array[0], ", ".join(changed))
        need(replacement != body, "nonvacuous coordinate mutant")
        cases[name] = (replacement, "*r67_requested_allocation_charge_v1")

    add("requested-bytes-zero", 1, "0")
    add("requested-bytes-max", 1, "u64::MAX")
    add("allocation-records-zero", 18, "0")
    add("allocation-records-two", 18, "2")
    for index in range(19):
        if index not in (1, 18):
            add(f"contaminated-coordinate-{index:02d}", index, "1")
    need(len(cases) == len(set(cases.values())) == MUTANT_COUNT, "all seventeen zero coordinates and four profile mutants")
    return cases


def selection_notes(leaf, focus):
    need(focus == "*r67_requested_allocation_charge_v1", "exact constructor selector")
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS, SELECTION_NOTES={
        "verifying root module (selected functions)",
        "verifying root module, function request_charge_v1::r67_requested_allocation_charge_v1 (selected functions)",
    })


def controller_source():
    raw = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BASE_SHA, "authenticated strict campaign")
    source = raw.decode()
    before, after = '"--multiple-errors", "0"', '"--multiple-errors", "1"'
    need(source.count(before) == 1, "exact first-error selection")
    return source.replace(before, after)


def controller():
    module = types.ModuleType("request_charge_campaign")
    module.__file__ = str(ROOT / BASE)
    sys.modules[module.__name__] = module
    exec(compile(controller_source(), module.__file__, "exec"), module.__dict__)
    module.FILES, module.PROOF, module.BODY = FILES, PROOF, BODY
    module.mutations, module.selection_notes = mutations, selection_notes
    return module


def campaign():
    audit(snapshot())
    need(type(EXPECTED_VERIFIED) is int and EXPECTED_VERIFIED > 0, "full discovery count is not measured")
    module = controller()
    module.EXPECTED = dict(module.EXPECTED, verified=EXPECTED_VERIFIED)
    return module


if __name__ == "__main__":
    runpy.run_path(str(ROOT / V / "test-request-charge.py"))
    campaign().main()
