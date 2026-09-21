#!/usr/bin/env python3
"""Authenticate producer-custody proofs and invariant sensitivity, not refinement."""

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
BASE_OBLIGATIONS = 181
INHERITED_COUNT = 156
POSITIVE_COUNT = BASE_OBLIGATIONS + 1
DEPENDENCIES = {
    "context_read_invariant_v1.rs": "CONTEXT_READ_INVARIANT_SHA256",
    "context_read_commit_v1.rs": "CONTEXT_READ_COMMIT_SHA256",
    "context_read_preflight_v1.rs": "CONTEXT_READ_PREFLIGHT_SHA256",
    "context_version_journal_issuance_v1.rs": "CONTEXT_VERSION_JOURNAL_ISSUANCE_SHA256",
}
PREFIX = "// Producer-read custody candidate: logical contents, not Rust/native refinement.\n"
INCLUDE = 'include!("context_read_invariant_v1.rs");\n'
SUBJECT = """
verus! {
pub open spec fn producer_mutation_prestate_v1(contents: ProducerReadContentsV1) -> bool { producer_invariant_v1(contents) }
pub fn producer_invariant_subject_v1(contents: &mut ProducerReadContentsV1)
    requires producer_mutation_prestate_v1(*old(contents)),
    ensures producer_invariant_v1(*final(contents)),
{
    let _watermark = contents.next_incarnation;
    return ();
}
}
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


def pinned_snapshots(pin_names):
    snapshots = {}
    for path, name in pin_names.items():
        pin = PINS / name
        pin_bytes = pin.read_bytes()
        expected = pin_bytes.decode("ascii").strip()
        data = path.read_bytes()
        require(len(expected) == 64 and digest(data) == expected, f"snapshot pin: {path.name}")
        snapshots[path], snapshots[pin] = data, pin_bytes
    return snapshots


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


INVARIANT = pinned_module("producer_invariant_utilities", HERE / "check-read-invariant.py",
                          PINS / "READ_INVARIANT_CHECKER_SHA256")
COMMIT, PREFLIGHT, BASE, POLICY = INVARIANT.COMMIT, INVARIANT.PREFLIGHT, INVARIANT.BASE, INVARIANT.POLICY
PREFLIGHT.POSITIVE_COUNT = POSITIVE_COUNT


def mutations():
    anchor = "    let _watermark = contents.next_incarnation;"
    changes = [
        ("lost_free_slot", "contents.free.clear();"),
        ("duplicate_free_slot", "if contents.free.len() > 0 { let slot = contents.free[0]; contents.free.push(slot); }"),
        ("producer_count", "contents.counts.set(0, 0);"),
        ("watermark_zero", "contents.next_incarnation = 0;"),
        ("watermark_rollback", "contents.next_incarnation = 1;"),
    ]
    for name, assignment in [
        ("physical_slot", "entry.reference.slot = contents.reservations.len();"),
        ("consumer_context", "entry.reference.consumer.context_generation = 0;"),
        ("consumer_before_producer", "entry.reference.consumer.local = entry.request.producer.key.local;"),
        ("producer_identity", "entry.request.producer.key.local = 0;"),
        ("request_version", "entry.request.read.attempt_epoch = 0;"),
    ]:
        changes.append((name, "if let Some(mut entry) = contents.reservations[0] { "
                        + assignment + " contents.reservations.set(0, Some(entry)); }"))
    changes.extend([
        ("duplicate_incarnation", "if contents.reservations.len() > 1 { if let (Some(first), Some(mut second)) = (contents.reservations[0], contents.reservations[1]) { second.reference.incarnation = first.reference.incarnation; contents.reservations.set(1, Some(second)); } }"),
        ("live_allocation_removed", "contents.stable.journal.allocations.set(0, None);"),
    ])
    return [BASE.Mutation(name, "producer_invariant_subject_v1", anchor, "    " + body,
                          "producer_invariant_v1", POSITIVE_COUNT - 1)
            for name, body in changes]


def audit_source(source: str, output: Path) -> None:
    require(source.isascii(), "ASCII proof source")
    require(source.startswith(PREFIX + INCLUDE) and source.count(INCLUDE) == 1,
            "exact initial pinned include")
    for name, pin in DEPENDENCIES.items():
        check_pin(output / name, PINS / pin)
    stripped = output / "audited-producer-body.rs"
    stripped.write_text(PREFIX + "\n" + source[len(PREFIX + INCLUDE):], encoding="ascii")
    POLICY.scan(stripped)
    INVARIANT.audit_source((output / "context_read_invariant_v1.rs").read_text(), output)
    for name, pin in DEPENDENCIES.items():
        check_pin(output / name, PINS / pin)


def campaign(source: Path, verus: Path, timeout: int, output: Path):
    source, verus = source.resolve(strict=True), verus.resolve(strict=True)
    require(1 <= timeout <= 300, "timeout must be 1 through 300")
    require(verus.name == "verus", "named Verus executable required")
    pin_names = {
        source: "CONTEXT_PRODUCER_READ_INVARIANT_SHA256",
        Path(__file__): "PRODUCER_READ_INVARIANT_CHECKER_SHA256",
        **{HERE / name: pin for name, pin in DEPENDENCIES.items()},
        Path(INVARIANT.__file__): "READ_INVARIANT_CHECKER_SHA256",
        Path(COMMIT.__file__): "READ_COMMIT_CHECKER_SHA256",
        Path(PREFLIGHT.__file__): "READ_PREFLIGHT_CHECKER_SHA256",
        Path(BASE.__file__): "JOURNAL_ISSUANCE_CHECKER_SHA256",
        Path(POLICY.__file__): "PROOF_SOURCE_CHECKER_SHA256",
        verus: "VERUS_SHA256",
        PINS / "VERUS_CLOSURE_MANIFEST": "VERUS_CLOSURE_MANIFEST_SHA256",
    }
    snapshots = pinned_snapshots(pin_names)
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
    original = snapshots[source].decode("ascii") + SUBJECT
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
            candidate = out / "producer.rs"
            expected = (BASE.mutate(original, mutation) if mutation else original).encode("ascii")
            candidate.write_bytes(expected)
            for dependency, data in dependencies.items():
                (out / dependency).write_bytes(data)
            audit_source(expected.decode("ascii"), out)
            generated = {str(candidate): digest(expected),
                         **{str(out / dependency): digest(data) for dependency, data in dependencies.items()}}
            for part in ["producer", "invariant", "commit", "preflight"]:
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
            print(f"PASS: producer invariant {name}", flush=True)
    require({str(path): digest(path.read_bytes()) for path in inputs} == identities,
            "final source/tool identities")
    result = {"qualified": True, "obligations": BASE_OBLIGATIONS, "inherited": INHERITED_COUNT,
              "test_obligations": 1, "invariant_mutations": len(mutations()), "inputs": identities}
    (output / "finished.json").write_text(json.dumps(result, indent=2) + "\n")
    print(f"PRODUCER_READ_INVARIANT_OK obligations={BASE_OBLIGATIONS} inherited={INHERITED_COUNT} "
          f"new={BASE_OBLIGATIONS - INHERITED_COUNT} test_obligations=1 invariant_mutations={len(mutations())}",
          flush=True)


def self_test(source: str):
    original = source + SUBJECT
    for case in mutations():
        BASE.postcondition_bounds(BASE.mutate(original, case), case)
    INVARIANT.self_test((HERE / "context_read_invariant_v1.rs").read_text())
    rejected = 0
    with tempfile.TemporaryDirectory(prefix="fe2o3-producer-invariant-audit-") as temporary:
        output = Path(temporary)
        snapshot_source, snapshot_pin = output / "snapshot.rs", output / "snapshot.pin"
        snapshot_source.write_bytes(b"original\n")
        snapshot_pin.write_text(digest(b"original\n") + "\n")
        captured = pinned_snapshots({snapshot_source: str(snapshot_pin)})
        snapshot_source.write_bytes(b"replacement\n")
        require(captured[snapshot_source] == b"original\n", "immutable source snapshot")
        for invalid_pin in [captured[snapshot_pin], b"0\n", b"\xff\n"]:
            snapshot_pin.write_bytes(invalid_pin)
            try:
                pinned_snapshots({snapshot_source: str(snapshot_pin)})
            except ValueError:
                rejected += 1
            else:
                raise ValueError("snapshot auditor accepted unauthenticated bytes")
        for name in DEPENDENCIES:
            (output / name).write_bytes((HERE / name).read_bytes())
        audit_source(original, output)
        for invalid in [
            original.replace(INCLUDE, 'include!("foreign.rs");\n'), "\n" + original,
            original + INCLUDE, "/*\n" + original + "\n*/",
            original + '\ninclude!("foreign.rs");\n', original + "\nmod foreign;\n",
            original + "\n#[cfg(false)] fn hidden() {}\n",
            original + "\nverus! { proof fn bypass() { assume(false); } }\n",
        ]:
            try:
                audit_source(invalid, output)
            except (ValueError, POLICY.ScanError):
                rejected += 1
            else:
                raise ValueError("producer invariant auditor accepted adverse source")
        for name in DEPENDENCIES:
            path = output / name
            data = path.read_bytes()
            path.write_bytes(data + b"\n")
            try:
                audit_source(original, output)
            except (ValueError, POLICY.ScanError):
                rejected += 1
            else:
                raise ValueError("recursive dependency substitution accepted")
            finally:
                path.write_bytes(data)
        audit_source(original, output)
    print(f"PASS: producer invariant self-test ({len(mutations())} reversible invariant mutations, "
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
