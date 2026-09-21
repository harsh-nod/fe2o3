#!/usr/bin/env python3
"""Authenticate producer journal issuance composition, not Rust/native refinement."""

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
POSITIVE_COUNT = 201
INHERITED_COUNT = 181
HEADER = """// Producer journal issuance composition: logical contents, not Rust/native refinement.
include!("context_producer_read_invariant_v1.rs");

mod journal_issuance {
use super::*;
use super::issuance::*;
"""
FOOTER = "\n}\n\npub use journal_issuance::*;\n"


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


PRODUCER = pinned_module("journal_issuance_producer_utilities", HERE / "check-producer-read-invariant.py",
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

    def add(name, function, before, after, postcondition):
        cases.append(BASE.Mutation(name, function, before, after, postcondition, POSITIVE_COUNT - 1))

    constructor = "issued_producer_constructor_exec_v1"
    call = "let result = producer_constructor_exec_v1(context, allocations, writers, reads);"
    add("constructor_context", constructor, call,
        call.replace("(context,", "(0,"), "issued_constructor_relation_v1")
    add("constructor_read_capacity", constructor, call,
        call.replace("writers, reads)", "writers, 0)"), "issued_constructor_relation_v1")
    for kind, changes in [
        ("register", [
            ("watermark", "contents.stable.journal.registration_watermark = 0;"),
            ("reserved_count", "contents.stable.journal.reserved_count = 0;"),
            ("writer_free", "contents.stable.journal.free.clear();"),
            ("stable_incarnation", "contents.stable.next_incarnation = 0;"),
            ("producer_counts", "contents.counts.clear();"),
        ]),
        ("abort", [
            ("reserved_count", "contents.stable.journal.reserved_count = 0;"),
            ("writer_free", "contents.stable.journal.free.push(reference.slot);"),
            ("watermark", "contents.stable.journal.registration_watermark = 0;"),
            ("members", "contents.stable.journal.members.clear();"),
        ]),
    ]:
        function = f"{kind}_issued_producer_exec_v1"
        postcondition = f"issued_{kind}_relation_v1"
        for name, change in changes:
            add(f"{kind}_{name}", function, "    result\n}",
                f"    {change}\n    result\n}}", postcondition)
        add(f"{kind}_result", function, "    result\n}",
            "    Err(JournalErrorV1::InvalidState)\n}", postcondition)
    return cases


def audit_source(source: str, output: Path) -> None:
    require(source.isascii(), "ASCII proof source")
    require(source.startswith(HEADER) and source.count(HEADER) == 1, "exact initial issuance module")
    require(source.endswith(FOOTER) and source.count(FOOTER) == 1, "exact issuance module export")
    for name, pin in DEPENDENCIES.items():
        check_pin(output / name, PINS / pin)
    stripped = output / "audited-journal-issuance-body.rs"
    stripped.write_text("use vstd::prelude::*;\n" + source[len(HEADER):-len(FOOTER)], encoding="ascii")
    POLICY.scan(stripped)
    PRODUCER.audit_source((output / "context_producer_read_invariant_v1.rs").read_text(), output)
    for name, pin in DEPENDENCIES.items():
        check_pin(output / name, PINS / pin)


def campaign(source: Path, verus: Path, timeout: int, output: Path):
    source, verus = source.resolve(strict=True), verus.resolve(strict=True)
    require(1 <= timeout <= 300, "timeout must be 1 through 300")
    require(verus.name == "verus", "named Verus executable required")
    pin_names = {
        source: "CONTEXT_PRODUCER_JOURNAL_ISSUANCE_SHA256",
        Path(__file__): "PRODUCER_JOURNAL_ISSUANCE_CHECKER_SHA256",
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
            candidate = out / "context_producer_journal_issuance_v1.rs"
            expected = (BASE.mutate(original, mutation) if mutation else original).encode("ascii")
            candidate.write_bytes(expected)
            for dependency, data in dependencies.items():
                (out / dependency).write_bytes(data)
            audit_source(expected.decode("ascii"), out)
            generated = {str(candidate): digest(expected),
                         **{str(out / dependency): digest(data) for dependency, data in dependencies.items()}}
            for part in ["journal-issuance", "producer", "invariant", "commit", "preflight"]:
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
            print(f"PASS: producer journal issuance {name}", flush=True)
    require({str(path): digest(path.read_bytes()) for path in inputs} == identities,
            "final source/tool identities")
    result = {"qualified": True, "obligations": POSITIVE_COUNT, "inherited": INHERITED_COUNT,
              "executable_mutations": len(mutations()), "inputs": identities}
    (output / "finished.json").write_text(json.dumps(result, indent=2) + "\n")
    print(f"PRODUCER_JOURNAL_ISSUANCE_OK obligations={POSITIVE_COUNT} inherited={INHERITED_COUNT} "
          f"new={POSITIVE_COUNT - INHERITED_COUNT} executable_mutations={len(mutations())}", flush=True)


def self_test(source: str):
    for case in mutations():
        BASE.postcondition_bounds(BASE.mutate(source, case), case)
    PRODUCER.self_test((HERE / "context_producer_read_invariant_v1.rs").read_text())
    rejected = 0
    with tempfile.TemporaryDirectory(prefix="fe2o3-producer-journal-issuance-audit-") as temporary:
        output = Path(temporary)
        for name in DEPENDENCIES:
            (output / name).write_bytes((HERE / name).read_bytes())
        audit_source(source, output)
        for invalid in [
            source.replace('include!("context_producer_read_invariant_v1.rs");', 'include!("foreign.rs");'),
            source.replace('include!("context_producer_read_invariant_v1.rs");', 'include!("/owned/context_producer_read_invariant_v1.rs");'),
            source.replace("pub use journal_issuance::*;", "pub use foreign::*;"),
            source.replace("use super::*;", "use foreign::*;"),
            source.replace("mod journal_issuance {", "#[cfg(false)]\nmod journal_issuance {"),
            source.replace("mod journal_issuance {", "#[allow(unused)]\nmod journal_issuance {"),
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
                raise ValueError("producer journal issuance auditor accepted adverse source")
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
    print(f"PASS: producer journal issuance self-test ({len(mutations())} executable mutations, "
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
