#!/usr/bin/env python3
"""Source-bound V4-J3 reader commit campaign; not a native/invariant proof."""

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
POSITIVE_COUNT = 127
INHERITED_COUNT = 103
DEPENDENCY = "context_read_preflight_v1.rs"
PREFIX = '// V4-J3 development: reader commit contents; no native authority claim.\n'
INCLUDE = f'include!("{DEPENDENCY}");\n'


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def check_pin(path: Path, pin: Path) -> None:
    expected = pin.read_text().strip()
    require(len(expected) == 64 and digest(path.read_bytes()) == expected, f"source pin: {path.name}")


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


PREFLIGHT = pinned_module("commit_preflight_utilities", HERE / "check-read-preflight.py",
                          PINS / "READ_PREFLIGHT_CHECKER_SHA256")
BASE, POLICY = PREFLIGHT.BASE, PREFLIGHT.POLICY
# Reuse the authenticated strict diagnostic parser with this crate's exact count.
# Its mutation count and source audit are not used to qualify the new commit body.
PREFLIGHT.POSITIVE_COUNT = POSITIVE_COUNT


def mutations():
    acquire = "    acquire_commit_exec_v1(contents, consumer, requests, output);"
    release = "    release_commit_exec_v1(contents, references);"
    cases = []
    for kind, anchor, function, postcondition in [
        ("acquire", acquire, "acquire_contents_exec_v1", "acquire_execution_relation_v1"),
        ("release", release, "release_contents_exec_v1", "release_execution_relation_v1"),
    ]:
        changes = [
            ("skip_commit", f"    // MUTANT: omitted {kind} commit."),
            ("journal_frame", anchor + "\n    contents.journal.registration_watermark = 0;"),
            ("incarnation", anchor + "\n    contents.next_incarnation = 0;"),
            ("free_contents", anchor + "\n    contents.free_reads.clear();"),
            ("reader_counts", anchor + "\n    if contents.readers.len() > 0 { contents.readers.set(0, 0); }"),
            ("lease_contents", anchor + "\n    if contents.leases.len() > 0 { contents.leases.set(0, None); }"),
        ]
        if kind == "acquire":
            changes.append(("output_contents", anchor + "\n    if output.len() > 0 { output.set(0, None); }"))
        for name, after in changes:
            cases.append(BASE.Mutation(f"{kind}_{name}", function, anchor, after,
                                       postcondition, POSITIVE_COUNT - 1))
        cases.append(BASE.Mutation(f"{kind}_rejection_frame", function,
                                   "Err(error) => return Err(error)",
                                   "Err(error) => { contents.next_incarnation = 0; return Err(error); }",
                                   postcondition, POSITIVE_COUNT - 1))
    return cases


def audit_source(source: str, dependency: Path, journal: Path, output: Path) -> None:
    require(source.isascii(), "ASCII proof source")
    require(source.startswith(PREFIX + INCLUDE) and source.count(INCLUDE) == 1, "exact initial pinned include")
    check_pin(dependency, PINS / "CONTEXT_READ_PREFLIGHT_SHA256")
    check_pin(journal, PINS / "CONTEXT_VERSION_JOURNAL_ISSUANCE_SHA256")
    stripped = output / "audited-commit-body.rs"
    stripped.write_text(PREFIX + "\n" + source[len(PREFIX + INCLUDE):], encoding="ascii")
    POLICY.scan(stripped)
    PREFLIGHT.audit_source(dependency.read_text(), journal, output / "audited-preflight-body.rs")
    check_pin(dependency, PINS / "CONTEXT_READ_PREFLIGHT_SHA256")
    check_pin(journal, PINS / "CONTEXT_VERSION_JOURNAL_ISSUANCE_SHA256")


def campaign(source: Path, verus: Path, timeout: int, output: Path):
    source, verus = source.resolve(strict=True), verus.resolve(strict=True)
    require(1 <= timeout <= 300, "timeout must be 1 through 300")
    require(verus.name == "verus", "named Verus executable required")
    pin_names = {
        source: "CONTEXT_READ_COMMIT_SHA256",
        Path(__file__): "READ_COMMIT_CHECKER_SHA256",
        HERE / DEPENDENCY: "CONTEXT_READ_PREFLIGHT_SHA256",
        HERE / PREFLIGHT.DEPENDENCY: "CONTEXT_VERSION_JOURNAL_ISSUANCE_SHA256",
        Path(PREFLIGHT.__file__): "READ_PREFLIGHT_CHECKER_SHA256",
        Path(BASE.__file__): "JOURNAL_ISSUANCE_CHECKER_SHA256",
        Path(POLICY.__file__): "PROOF_SOURCE_CHECKER_SHA256",
        verus: "VERUS_SHA256",
        PINS / "VERUS_CLOSURE_MANIFEST": "VERUS_CLOSURE_MANIFEST_SHA256",
    }
    for path, name in pin_names.items():
        check_pin(path, PINS / name)
    closure = ROOT / "examples/row_softmax_v1/verify-verus-closure.sh"
    manifest = PINS / "VERUS_CLOSURE_MANIFEST"
    require(digest(closure.read_bytes()) == "c0f5f201dca9ea6b3fa953884cdfaca8ca38413ad2a9de7700b3aaeb3a610d0c", "closure checker pin")
    inputs = [*pin_names, *[PINS / name for name in pin_names.values()], closure,
              verus.parent / "rust_verify", verus.parent / "z3"]
    identities = {str(path): digest(path.read_bytes()) for path in inputs}
    output.mkdir()
    (output / "inputs.json").write_text(json.dumps(identities, indent=2) + "\n")
    environment = {"HOME": os.environ.get("HOME", "/nonexistent")}
    for name in ("RUSTUP_HOME", "CARGO_HOME"):
        environment[name] = os.environ.get(name, str(Path(environment["HOME"]) / (".rustup" if name == "RUSTUP_HOME" else ".cargo")))
    environment["PATH"] = environment["CARGO_HOME"] + "/bin:/usr/bin:/bin"
    environment["VERUS_Z3_PATH"] = str(verus.parent / "z3")
    original = source.read_text()
    dependencies = {name: (HERE / name).read_bytes() for name in [DEPENDENCY, PREFLIGHT.DEPENDENCY]}
    cases = [("positive_before", None), *[(item.name, item) for item in mutations()], ("positive_after", None)]
    for phase in ["before", "after"]:
        status, _, _ = BASE.run_owned(["/bin/sh", str(closure), str(verus.parent), str(manifest)],
                                      120, output / f"closure-{phase}", environment)
        require(status == 0, "authenticated Verus distribution")
        if phase == "after":
            break
        for name, mutation in cases:
            out = output / name
            out.mkdir()
            candidate = out / "commit.rs"
            expected = (BASE.mutate(original, mutation) if mutation else original).encode("ascii")
            candidate.write_bytes(expected)
            for dependency, data in dependencies.items():
                (out / dependency).write_bytes(data)
            audit_source(expected.decode("ascii"), out / DEPENDENCY, out / PREFLIGHT.DEPENDENCY, out)
            generated = {str(path): digest(path.read_bytes()) for path in
                         [candidate, out / DEPENDENCY, out / PREFLIGHT.DEPENDENCY,
                          out / "audited-commit-body.rs", out / "audited-preflight-body.rs"]}
            (out / "sources.json").write_text(json.dumps(generated, indent=2) + "\n")
            command = ["/usr/bin/timeout", "--foreground", "--signal=TERM", "--kill-after=5", str(timeout),
                       str(verus), "--crate-type", "lib", "--triggers-mode", "silent", "--no-cheating",
                       "--output-json", "--error-format=json", "--num-threads", "4", str(candidate)]
            require({path: digest(Path(path).read_bytes()) for path in generated} == generated, "exact solver inputs before")
            status, stdout, stderr = BASE.run_owned(command, timeout + 10, out / "solver", environment)
            require({path: digest(Path(path).read_bytes()) for path in generated} == generated, "generated source replacement")
            PREFLIGHT.check_result(status, stdout, stderr, expected.decode("ascii"), candidate, mutation)
            require({str(path): digest(path.read_bytes()) for path in inputs} == identities, "source/tool replacement")
            print(f"PASS: reader commit {name}", flush=True)
    require({str(path): digest(path.read_bytes()) for path in inputs} == identities, "final source/tool identities")
    print(f"READ_COMMIT_OK obligations={POSITIVE_COUNT} inherited={INHERITED_COUNT} new={POSITIVE_COUNT - INHERITED_COUNT} executable_mutations={len(mutations())}", flush=True)


def self_test(source: str):
    for case in mutations():
        changed = BASE.mutate(source, case)
        BASE.postcondition_bounds(changed, case)
    # Exercise the unchanged parser's exact counts/spans, stale-bytecode defense,
    # recursive import audit and process ownership tests with the current count.
    PREFLIGHT.self_test((HERE / DEPENDENCY).read_text())
    rejected = 0
    with tempfile.TemporaryDirectory(prefix="fe2o3-commit-audit-") as temporary:
        output = Path(temporary)
        dependency, journal = output / DEPENDENCY, output / PREFLIGHT.DEPENDENCY
        dependency.write_bytes((HERE / DEPENDENCY).read_bytes())
        journal.write_bytes((HERE / PREFLIGHT.DEPENDENCY).read_bytes())
        audit_source(source, dependency, journal, output)
        for invalid in [source.replace(INCLUDE, INCLUDE.replace(DEPENDENCY, "foreign.rs")),
                        "\n" + source, source + INCLUDE, "/*\n" + source + "\n*/",
                        source + '\ninclude!("foreign.rs");\n', source + '\nmod foreign;\n',
                        source + '\n#[cfg(false)] fn hidden() {}\n',
                        source + '\nverus! { proof fn bypass() { assume(false); } }\n']:
            try:
                audit_source(invalid, dependency, journal, output)
            except (ValueError, POLICY.ScanError):
                rejected += 1
            else:
                raise ValueError("include/source auditor accepted adverse source")
        for path in [dependency, journal]:
            original = path.read_bytes()
            path.write_bytes(original + b"\n")
            try:
                audit_source(source, dependency, journal, output)
            except (ValueError, POLICY.ScanError):
                rejected += 1
            else:
                raise ValueError("recursive dependency substitution accepted")
            finally:
                path.write_bytes(original)
        audit_source(source, dependency, journal, output)
    print(f"PASS: commit self-test ({len(mutations())} reversible mutations, {rejected} additional adverse sources rejected)")


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
        require(args.verus is not None and args.timeout is not None and args.output is not None, "campaign arguments")
        campaign(args.source, args.verus, args.timeout, args.output)


if __name__ == "__main__":
    main()
