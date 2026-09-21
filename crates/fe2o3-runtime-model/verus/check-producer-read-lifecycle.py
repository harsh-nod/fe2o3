#!/usr/bin/env python3
"""Authenticate logical producer lifecycle execution, not Rust/native refinement."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import sys
import tempfile

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
PINS = HERE / "pins"
POSITIVE_COUNT = 254
INHERITED_COUNT = 180
HEADER = """// Producer-read lifecycle candidate: logical execution, not Rust/native refinement.
mod custody {
    include!("context_producer_read_invariant_v1.rs");
    pub use self::issuance::*;
}
use custody::*;
use vstd::prelude::*;
"""


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def check_pin(path: Path, pin: Path) -> None:
    expected = pin.read_text().strip()
    require(len(expected) == 64 and digest(path.read_bytes()) == expected,
            f"source pin: {path.name}")


def pinned_module(name: str, path: Path, pin: Path):
    verified = path.read_bytes()
    require(digest(verified) == pin.read_text().strip(), f"module source pin: {path.name}")
    spec = importlib.util.spec_from_file_location(name, path)
    require(spec is not None and spec.loader is not None, "module loader")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    exec(compile(verified, str(path), "exec"), module.__dict__)
    check_pin(path, pin)
    return module


PRODUCER = pinned_module("lifecycle_producer_utilities", HERE / "check-producer-read-invariant.py",
                         PINS / "PRODUCER_READ_INVARIANT_CHECKER_SHA256")
INVARIANT, COMMIT, PREFLIGHT, BASE, POLICY = (
    PRODUCER.INVARIANT, PRODUCER.COMMIT, PRODUCER.PREFLIGHT, PRODUCER.BASE, PRODUCER.POLICY)
PREFLIGHT.POSITIVE_COUNT = POSITIVE_COUNT
DEPENDENCIES = {
    "context_producer_read_invariant_v1.rs": "CONTEXT_PRODUCER_READ_INVARIANT_SHA256",
    **PRODUCER.DEPENDENCIES,
}


def mutations():
    cases = []

    def add(name, function, before, after, postcondition="exact_decision_v1"):
        cases.append(BASE.Mutation(name, function, before, after, postcondition, POSITIVE_COUNT - 1))

    for name, before, after in [
        ("pending_as_success", "WriterEntryV1::Pending { .. } => Ok(ProducerStatusV1::Pending)",
         "WriterEntryV1::Pending { .. } => Ok(ProducerStatusV1::Success)"),
        ("unknown_as_pending", "WriterEntryV1::Unknown { .. } => Ok(ProducerStatusV1::Unknown)",
         "WriterEntryV1::Unknown { .. } => Ok(ProducerStatusV1::Pending)"),
        ("noeffect_as_success", "else if entry.content_lineage == read.content_lineage { Ok(ProducerStatusV1::NoEffect) }",
         "else if entry.content_lineage == read.content_lineage { Ok(ProducerStatusV1::Success) }"),
    ]:
        add(name, "producer_status_exec_v1", before, after)
    device = """    if entry.device.context_generation != read.device.context_generation || entry.device.local != read.device.local {
        return Err(ReadErrorV1::AllocationDeviceMismatch);
    }
"""
    extent = "    if entry.byte_extent != read.byte_extent { return Err(ReadErrorV1::AllocationExtentMismatch); }\n"
    add("device_extent_order", "producer_status_exec_v1", device + extent, extent + device)
    add("nonpending_admitted", "producer_validate_exec_v1",
        "Ok(_) => Err(ReadErrorV1::AllocationBusy),", "Ok(_) => Ok(()),")
    add("producer_kind_omitted", "producer_acquire_header_exec_v1",
        "if !issuable_id_exec_v1(consumer.local) || !matches!(consumer.kind, WriterKindV1::Submission)",
        "if !issuable_id_exec_v1(consumer.local)")
    add("producer_equal_consumer", "producer_acquire_item_exec_v1",
        "request.producer.key.local >= consumer.local", "request.producer.key.local > consumer.local")
    capacity = "    if count > remaining { return Err(ReadErrorV1::MemberCapacity); }\n"
    add("shared_capacity_ignores_stable", "producer_capacity_exec_v1", capacity,
        "    let _ = remaining;\n    if count > contents.free.len() { return Err(ReadErrorV1::MemberCapacity); }\n")
    epoch = """    if contents.next_incarnation == 0 || contents.next_incarnation.checked_add(count as u64).is_none() {
        return Err(ReadErrorV1::EpochExhausted);
    }
"""
    add("capacity_epoch_order", "producer_capacity_exec_v1", capacity + epoch, epoch + capacity)
    evidence = "    if !same_key_exec_v1(evidence_consumer, consumer) { return Err(ReadErrorV1::SettlementEvidenceMismatch); }\n"
    empty = "    if count == 0 { return Err(ReadErrorV1::RosterCapacity); }\n"
    add("release_evidence_order", "producer_release_header_exec_v1", evidence + empty, empty + evidence)
    context = "    if consumer.context_generation != contents.stable.journal.context_generation { return Err(ReadErrorV1::ForeignContext); }\n"
    identity = "    if !issuable_id_exec_v1(consumer.local) || !matches!(consumer.kind, WriterKindV1::Submission) { return Err(ReadErrorV1::InvalidWriterId); }\n"
    add("identity_before_context", "producer_acquire_header_exec_v1", context + identity, identity + context)
    for kind, args in [
        ("acquire", "requests[index], index, state"), ("release", "references[index], state"),
    ]:
        item = f"""        match producer_{kind}_item_exec_v1(contents, consumer, {args}) {{
            Err(error) => return Err(error), Ok(next) => state = next,
        }}"""
        add(f"{kind}_scan_error_accepted", f"producer_{kind}_preflight_exec_v1", item,
            item.replace("Err(error) => return Err(error)", "Err(_) => return Ok(())"))
    for kind in ("acquire", "release"):
        function = f"producer_{kind}_contents_exec_v1"
        postcondition = f"producer_{kind}_execution_relation_v1"
        add(f"{kind}_error_state", function, "Err(error) => return Err(error)",
            "Err(error) => { contents.next_incarnation = 0; return Err(error); }", postcondition)
        if kind == "acquire":
            add("acquire_error_output", function, "Err(error) => return Err(error)",
                "Err(error) => { output.clear(); return Err(error); }", postcondition)
            commit = "    producer_acquire_commit_exec_v1(contents, consumer, requests, output);"
            add("acquire_commit_counter", function, commit,
                commit + "\n    contents.next_incarnation = 0;", postcondition)
            add("acquire_commit_output", function, commit,
                commit + "\n    output.clear();", postcondition)
        else:
            commit = "    producer_release_commit_exec_v1(contents, references);"
            add("release_commit_free", function, commit,
                commit + "\n    contents.free.clear();", postcondition)
    return cases


def audit_source(source: str, output: Path) -> None:
    require(source.isascii(), "ASCII proof source")
    require(source.startswith(HEADER) and source.count(HEADER) == 1, "exact initial custody module")
    for name, pin in DEPENDENCIES.items():
        check_pin(output / name, PINS / pin)
    stripped = output / "audited-lifecycle-body.rs"
    stripped.write_text("use vstd::prelude::*;\n" + source[len(HEADER):], encoding="ascii")
    POLICY.scan(stripped)
    PRODUCER.audit_source((output / "context_producer_read_invariant_v1.rs").read_text(), output)
    for name, pin in DEPENDENCIES.items():
        check_pin(output / name, PINS / pin)


def campaign(source: Path, verus: Path, timeout: int, output: Path):
    source, verus = source.resolve(strict=True), verus.resolve(strict=True)
    require(1 <= timeout <= 300, "timeout must be 1 through 300")
    require(verus.name == "verus", "named Verus executable required")
    pin_names = {
        source: "CONTEXT_PRODUCER_READ_LIFECYCLE_SHA256",
        Path(__file__): "PRODUCER_READ_LIFECYCLE_CHECKER_SHA256",
        **{HERE / name: pin for name, pin in DEPENDENCIES.items()},
        Path(PRODUCER.__file__): "PRODUCER_READ_INVARIANT_CHECKER_SHA256",
        Path(INVARIANT.__file__): "READ_INVARIANT_CHECKER_SHA256",
        Path(COMMIT.__file__): "READ_COMMIT_CHECKER_SHA256",
        Path(PREFLIGHT.__file__): "READ_PREFLIGHT_CHECKER_SHA256",
        Path(BASE.__file__): "JOURNAL_ISSUANCE_CHECKER_SHA256",
        Path(POLICY.__file__): "PROOF_SOURCE_CHECKER_SHA256",
        verus: "VERUS_SHA256",
        PINS / "VERUS_CLOSURE_MANIFEST": "VERUS_CLOSURE_MANIFEST_SHA256",
    }
    snapshots = PRODUCER.pinned_snapshots(pin_names)
    closure = ROOT / "examples/row_softmax_v1/verify-verus-closure.sh"
    snapshots[closure] = closure.read_bytes()
    require(digest(snapshots[closure])
            == "c0f5f201dca9ea6b3fa953884cdfaca8ca38413ad2a9de7700b3aaeb3a610d0c",
            "closure checker pin")
    for path in [verus.parent / "rust_verify", verus.parent / "z3"]:
        snapshots[path] = path.read_bytes()
    identities = {str(path): digest(data) for path, data in snapshots.items()}
    inputs = list(snapshots)
    require({str(path): digest(path.read_bytes()) for path in inputs} == identities,
            "authenticated input snapshot changed")
    output.mkdir()
    (output / "inputs.json").write_text(json.dumps(identities, indent=2) + "\n")
    environment = {"HOME": os.environ.get("HOME", "/nonexistent")}
    for name, default in [("RUSTUP_HOME", ".rustup"), ("CARGO_HOME", ".cargo")]:
        environment[name] = os.environ.get(name, str(Path(environment["HOME"]) / default))
    environment["PATH"] = environment["CARGO_HOME"] + "/bin:/usr/bin:/bin"
    environment["VERUS_Z3_PATH"] = str(verus.parent / "z3")
    original = snapshots[source].decode("ascii")
    dependencies = {name: snapshots[HERE / name] for name in DEPENDENCIES}
    cases = [("positive_before", None), *[(item.name, item) for item in mutations()],
             ("positive_after", None)]
    for phase in ["before", "after"]:
        status, _, _ = BASE.run_owned(["/bin/sh", str(closure), str(verus.parent),
                                      str(PINS / "VERUS_CLOSURE_MANIFEST")],
                                     120, output / f"closure-{phase}", environment)
        require(status == 0, "authenticated Verus distribution")
        if phase == "after":
            break
        for name, mutation in cases:
            out = output / name
            out.mkdir()
            candidate = out / "context_producer_read_lifecycle_v1.rs"
            expected = (BASE.mutate(original, mutation) if mutation else original).encode("ascii")
            candidate.write_bytes(expected)
            for dependency, data in dependencies.items():
                (out / dependency).write_bytes(data)
            audit_source(expected.decode("ascii"), out)
            generated = {str(candidate): digest(expected),
                         **{str(out / dependency): digest(data) for dependency, data in dependencies.items()}}
            for part in ["lifecycle", "producer", "invariant", "commit", "preflight"]:
                path = out / f"audited-{part}-body.rs"
                generated[str(path)] = digest(path.read_bytes())
            (out / "sources.json").write_text(json.dumps(generated, indent=2) + "\n")
            command = ["/usr/bin/timeout", "--foreground", "--signal=TERM", "--kill-after=5", str(timeout),
                       str(verus), "--crate-type", "lib", "--triggers-mode", "silent", "--no-cheating",
                       "--output-json", "--error-format=json", "--num-threads", "4", str(candidate)]
            require({path: digest(Path(path).read_bytes()) for path in generated} == generated,
                    "exact solver inputs before")
            require({str(path): digest(path.read_bytes()) for path in inputs} == identities,
                    "source/tool replacement before solver")
            status, stdout, stderr = BASE.run_owned(command, timeout + 10, out / "solver", environment)
            require({path: digest(Path(path).read_bytes()) for path in generated} == generated,
                    "generated source replacement")
            PREFLIGHT.check_result(status, stdout, stderr, expected.decode("ascii"), candidate, mutation)
            require({str(path): digest(path.read_bytes()) for path in inputs} == identities,
                    "source/tool replacement")
            print(f"PASS: producer lifecycle {name}", flush=True)
    require({str(path): digest(path.read_bytes()) for path in inputs} == identities,
            "final source/tool identities")
    result = {"qualified": True, "obligations": POSITIVE_COUNT, "inherited": INHERITED_COUNT,
              "executable_mutations": len(mutations()), "inputs": identities}
    (output / "finished.json").write_text(json.dumps(result, indent=2) + "\n")
    print(f"PRODUCER_READ_LIFECYCLE_OK obligations={POSITIVE_COUNT} inherited={INHERITED_COUNT} "
          f"new={POSITIVE_COUNT - INHERITED_COUNT} executable_mutations={len(mutations())}", flush=True)


def self_test(source: str):
    for case in mutations():
        BASE.postcondition_bounds(BASE.mutate(source, case), case)
    PRODUCER.self_test((HERE / "context_producer_read_invariant_v1.rs").read_text())
    rejected = 0
    with tempfile.TemporaryDirectory(prefix="fe2o3-producer-lifecycle-audit-") as temporary:
        output = Path(temporary)
        for name in DEPENDENCIES:
            (output / name).write_bytes((HERE / name).read_bytes())
        audit_source(source, output)
        for invalid in [
            source.replace('include!("context_producer_read_invariant_v1.rs");', 'include!("foreign.rs");'),
            source.replace('include!("context_producer_read_invariant_v1.rs");', 'include!("/owned/context_producer_read_invariant_v1.rs");'),
            source.replace("pub use self::issuance::*;", "pub use foreign::*;"),
            source.replace("use custody::*;", "use foreign::*;"),
            source.replace("mod custody {", "#[cfg(false)]\nmod custody {"),
            source.replace("mod custody {", "#[allow(unused)]\nmod custody {"),
            "\n" + source, "/*\n" + source + "\n*/", source + HEADER,
            source + '\ninclude!("foreign.rs");\n', source + "\nmod foreign;\n",
            source + "\nmod foreign {}\n", source + "\n#[cfg(false)] fn hidden() {}\n",
            source + "\nverus! { proof fn bypass() { assume(false); } }\n",
        ]:
            try:
                audit_source(invalid, output)
            except (ValueError, POLICY.ScanError):
                rejected += 1
            else:
                raise ValueError("producer lifecycle auditor accepted adverse source")
        for name in DEPENDENCIES:
            path = output / name
            data = path.read_bytes()
            path.write_bytes(data + b"\n")
            try:
                audit_source(source, output)
            except (ValueError, POLICY.ScanError):
                rejected += 1
            else:
                raise ValueError("recursive dependency substitution accepted")
            finally:
                path.write_bytes(data)
        audit_source(source, output)
    print(f"PASS: producer lifecycle self-test ({len(mutations())} executable mutations, "
          f"{rejected} additional adverse sources rejected)")


def main():
    for signum in BASE.SIGNALS:
        signal.signal(signum, BASE.interrupted)
    signal.pthread_sigmask(signal.SIG_UNBLOCK, BASE.SIGNALS)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("source", type=Path)
    parser.add_argument("verus", type=Path, nargs="?")
    parser.add_argument("timeout", type=int, nargs="?")
    parser.add_argument("output", type=Path, nargs="?")
    args = parser.parse_args()
    if args.self_test:
        require(args.verus is None and args.timeout is None and args.output is None, "self-test arguments")
        self_test(args.source.read_text())
    else:
        require(args.verus is not None and args.timeout is not None and args.output is not None,
                "campaign arguments")
        campaign(args.source, args.verus, args.timeout, args.output)


if __name__ == "__main__":
    main()
