#!/usr/bin/env python3
"""Source-only binding for the closed selector proof, not a solver result.

The three-file Verus closure shares the executable selector and enum with Rust.
Constructor forwarding, the unchanged scan and custody are separately source
reviewed and CPU tested, not proved by this selector. No scheduler, syscall,
hardware completion, timing bound or performance acceptance is implied.
"""
import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[3]
SRC = Path("crates/fe2o3-kfd/src")
V = Path("crates/fe2o3-runtime-model/verus")
DECL = SRC / "sdma/retained_pair_cadence_declarations.rs"
BODY = SRC / "sdma/retained_pair_cadence_body.rs"
RUST = SRC / "sdma/retained_pair_cadence.rs"
PROOF = V / "retained_pair_cadence_v1.rs"
WAIT = SRC / "wait.rs"
WAIT_ARITHMETIC = SRC / "wait_arithmetic.rs"
WAIT_BODY = SRC / "wait_arithmetic_body.rs"
FILES = [PROOF, DECL, BODY]
PINS = {
    DECL: "540520a35407a0dbd9c3210af733cbc07137f80643210557baf066c7d5f91591",
    BODY: "cdb801be2d8d07515097317ed2829126dfe113467db689169ef34eaa4fad7acf",
    RUST: "dea23a3c0fb84703dfbd0627f742ada2eb097f617ad679e4a59c39ddd0177eb4",
    PROOF: "95f04710b73a65c8d6d272a849f650e36d04a531b8504fe4b409cd20c07c8cbe",
    SRC / "lib.rs": "507c8a7ac5bdee14da36e5d825e4c1934404acbd37eddc68954c575d45d668f2",
    SRC / "sdma.rs": "b9fbeb93aa28a11e6bc81ad0d94ff396a459fb65613d281039774dab70c3e7b9",
    SRC / "sdma/retained_pair.rs": "69e1b72a7f31ec1461f627a4fd381093985f570a8e6a71540f3bf27230b35547",
    SRC / "sdma/retained_pair_operation_body.rs": "54236918b01d7167878c4827e28ab0c4a103e3f3b069d311b56931daad86b7ac",
    SRC / "sdma/retained_pair_policy_v1.txt": "18cfe1c56d270d9cab1cdc2f67a2b26b35e7cf1540a2962e4dc2f5cb42155b61",
    Path("crates/fe2o3-kfd/examples/kfd-xgmi-retained-host-diagnostic.rs"): "788f520f4905610142afc98cd4186037d9ca883f1b2313c27223985e90def1fd",
    Path("benchmarks/runtime_gfx942/xgmi_retained_host_diagnostic.py"): "2240bd0a94d35f65171c4814397ae91ef9552d6917ac311c4da16ea577c82a4b",
    WAIT: "6c586d97d7dbb4cf9be4f06d279f80758afe05d4d6b772ae85d372d8c28aac0c",
    WAIT_ARITHMETIC: "1d4dc372f72480f102aaa07f2f6fc81b3034cf561b85849bb81ff292f90ad605",
    WAIT_BODY: "1d4e8a628e42cf8dd7607f6902d55f92dc0df8a4d1e4fc69afd3f6844f006c57",
}


def need(condition, message):
    if not condition:
        raise ValueError(message)


def sha(text):
    return hashlib.sha256(text.encode("ascii")).hexdigest()


def snapshot():
    result = {}
    for path in PINS:
        selected = ROOT / path
        need(selected.is_file() and not selected.is_symlink() and selected.resolve() == selected,
             "ordinary exact source path")
        result[path] = selected.read_bytes().decode("ascii")
    return result


def audit(sources):
    need(set(sources) == set(PINS), "exact reviewed source roster")
    for path, digest in PINS.items():
        need(sha(sources[path]) == digest, "reviewed source bytes: " + str(path))
    wait = sources[WAIT]
    # Bind the extracted numeric implementation too, not just its caller.
    need(re.findall(r'include!\("([^"]+)"\);', wait) == [WAIT_ARITHMETIC.name] and
         re.findall(r'include!\("([^"]+)"\);', sources[WAIT_ARITHMETIC]) == [WAIT_BODY.name],
         "exact wait arithmetic include chain")
    for path, prefix in ((RUST, ""), (PROOF, "../../fe2o3-kfd/src/sdma/")):
        includes = re.findall(r'include!\("([^"]+)"\);', sources[path])
        need(includes == [prefix + DECL.name, prefix + BODY.name], "exact two shared includes")
        need(sources[path].count("retained_pair_cadence_ceiling_body_v1!(cadence)") == 1,
             "actual shared decision invoked exactly once")
    for path in FILES:
        need(not re.search(r'\b(?:assume|admit)\s*\(|external_body|\brequires\b', sources[path]),
             "no trusted or weakened selector contract")
    need(not re.search(r'\binclude!\(', sources[DECL] + sources[BODY]), "closed leaf proof inputs")
    rust = sources[RUST].split("#[cfg(test)]", 1)[0]
    need("Self::Ordinary1ms => MonotonicWaitV1::until(deadline)" in rust,
         "ordinary constructor remains ordinary")
    need("until_with_active_spin_floor" not in rust and "Instant::now()" not in rust,
         "no active floor or fresh deadline")
    need("Duration::from_nanos(self.sleep_ceiling_ns())" in rust,
         "short constructor uses actual shared selector")
    sdma = sources[SRC / "sdma.rs"]
    scan = sdma.split("    fn wait_many_xgmi_with_timer<", 1)[1].split("    fn observe_progress_in_current_scope(", 1)[0]
    need(scan.count("let deadline = deadline.resolve()?;") == 1 and
         scan.count("let mut wait = cadence.cursor(deadline);") == 1,
         "one resolution and one existing scan cursor")
    outer = sdma.split("    fn wait_batch_with_timer<", 1)[1].split("    fn wait_for_with_currentness(", 1)[0]
    need(outer.count("Self::validate_route_currentness(") == 2 and
         "Gfx942XgmiBatchWaitFailureV1::CompletedCurrentnessIndeterminate" in outer,
         "paired checks and completed-on-closing-failure custody")
    pair = sources[SRC / "sdma/retained_pair.rs"]
    for name in ("wait_batch_for_cadence_experiment_v1", "wait_batch_for_cadence_diagnostic_v1"):
        need('#[cfg(feature = "hardware-diagnostic")]\n    pub fn ' + name in pair,
             "separately named gated experiment entrypoint")
    ordinary = pair.split("    pub fn wait_batch_for_cadence_experiment_v1(", 1)[1].split("    /// Success-only", 1)[0]
    need("run_operation(&mut self.scope.context" in ordinary and
         "&mut XgmiWaitTimer::<false>::new()" in ordinary and "Instant::now()" not in ordinary,
         "ordinary path has original custody and no diagnostic clock")
    return {"source_binding": True, "solver_executed": False, "formal_refinement": False,
            "performance_acceptance": False, "proof_scope": "closed ceiling selection only",
            "proof_closure": [str(path) for path in FILES],
            "sources": {str(path): sha(text) for path, text in sorted(sources.items())}}


def mutations(body):
    rows = {}
    for name, before, after in (
        ("ordinary-selects-short", "Ordinary1ms => 1_000_000u64", "Ordinary1ms => 25_000u64"),
        ("short-selects-zero", "Ceiling25us => 25_000u64", "Ceiling25us => 0u64"),
        ("short-exceeds-ceiling", "Ceiling25us => 25_000u64", "Ceiling25us => 1_000_001u64"),
    ):
        need(body.count(before) == 1, "unique executable selector mutation")
        rows[name] = (body.replace(before, after), "*retained_pair_cadence_ceiling_v1")
    need(len(set(rows.values())) == 3, "three distinct logical mutation candidates")
    return rows


if __name__ == "__main__":
    print(json.dumps(audit(snapshot()), sort_keys=True, indent=2))
