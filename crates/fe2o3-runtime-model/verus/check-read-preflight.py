#!/usr/bin/env python3
"""Authenticated, source-bound V4-J2 preflight campaign; no commit/native claims."""

from __future__ import annotations

import argparse
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import py_compile
import signal
import sys
import tempfile

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
PINS = HERE / "pins"
POSITIVE_COUNT = 103
INHERITED_COUNT = 69
DEPENDENCY = "context_version_journal_issuance_v1.rs"
PREFIX = ('// V4-J2 development: concrete reader contents and ordered preflight only.\n'
          '// Commit preservation, Rust correspondence, storage and native authority are separate.\n')
IMPORT = f'#[path = "{DEPENDENCY}"]\nmod issuance;\n'


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
    # SourceFileLoader may consume a stale timestamp-valid .pyc even with -B.
    exec(compile(verified, str(path), "exec"), module.__dict__)
    check_pin(path, pin)
    return module


# Reuse unchanged, pinned body/span/process utilities, not their J1-only counts.
BASE = pinned_module("reader_journal_utilities", HERE / "check-journal-issuance.py", PINS / "JOURNAL_ISSUANCE_CHECKER_SHA256")
POLICY = pinned_module("reader_source_policy", ROOT / "examples/wave64_collectives_v1/check-proof-source.py",
                       PINS / "PROOF_SOURCE_CHECKER_SHA256")


def mutations():
    def item(name, function, before, after, postcondition="exact_decision_v1"):
        return BASE.Mutation(name, function, before, after, postcondition, POSITIVE_COUNT - 1)

    constructor = "reader_constructor_exec_v1"
    constructor_post = "reader_constructor_relation_v1"
    acquire_frame = "    acquire_preflight_exec_v1(contents, consumer, requests, output.as_slice())"
    release_frame = "    release_preflight_exec_v1(contents, consumer, references, evidence_consumer, observed_free_capacity)"
    return [
        item("constructor_lease_extent", constructor, "let leases = vacant_contents_exec_v1(reads);",
             "let leases = vacant_contents_exec_v1(writers);", constructor_post),
        item("constructor_count_extent", constructor, "let readers = zero_readers_exec_v1(allocations);",
             "let readers = zero_readers_exec_v1(reads);", constructor_post),
        item("constructor_free_contents", constructor, "let free_reads = free_contents_exec_v1(reads);",
             "let free_reads = Vec::new();", constructor_post),
        item("constructor_incarnation", constructor, "next_incarnation: 1", "next_incarnation: 2", constructor_post),
        item("allocation_backlink", "allocation_lookup_exec_v1",
             "if !same_allocation_exec_v1(member.allocation, reference)", "if false"),
        item("read_empty_range", "validate_read_exec_v1",
             "if request.byte_len == 0 { return Err(ReadErrorV1::InvalidExtent); }",
             "if request.byte_len == 0 { return Ok(()); }"),
        item("read_pending_writer", "validate_read_exec_v1",
             "if entry.pending_member.is_some() { return Err(ReadErrorV1::AllocationBusy); }",
             "if entry.pending_member.is_some() { return Ok(()); }"),
        item("read_version", "validate_read_exec_v1",
             "if entry.attempt_epoch != request.attempt_epoch\n        || entry.content_lineage != request.content_lineage { return Err(ReadErrorV1::InvalidState); }",
             "if entry.attempt_epoch != request.attempt_epoch\n        || entry.content_lineage != request.content_lineage { return Ok(()); }"),
        item("lease_identity", "lease_lookup_exec_v1",
             "if !same_read_reference_exec_v1(entry.reference, reference) { return Err(ReadErrorV1::InvalidReference); }",
             "if !same_read_reference_exec_v1(entry.reference, reference) { return Ok(entry.request); }"),
        item("capacity_zero_incarnation", "validate_capacity_exec_v1",
             "contents.next_incarnation == 0 || contents.next_incarnation.checked_add(count as u64).is_none()",
             "contents.next_incarnation.checked_add(count as u64).is_none()"),
        item("acquire_consumer", "acquire_header_exec_v1",
             "if consumer.context_generation != contents.journal.context_generation { return Err(ReadErrorV1::ForeignContext); }",
             "if consumer.context_generation != contents.journal.context_generation { return Ok(()); }"),
        item("acquire_dirty_output", "acquire_header_exec_v1",
             "if !output_vacant_exec_v1(output) { return Err(ReadErrorV1::InvalidState); }",
             "if !output_vacant_exec_v1(output) { return Ok(()); }"),
        item("acquire_canonical", "acquire_item_exec_v1",
             "if !order_lt_exec_v1(prior, key) { return Err(ReadErrorV1::NonCanonicalRoster); }",
             "if !order_lt_exec_v1(prior, key) { return Ok(ReadScanV1 { previous: Some(key), group: 0 }); }"),
        item("acquire_occupied_slot", "acquire_item_exec_v1", "if contents.leases[slot].is_some()", "if false"),
        item("release_evidence", "release_header_exec_v1",
             "if !same_key_exec_v1(evidence_consumer, consumer) { return Err(ReadErrorV1::SettlementEvidenceMismatch); }",
             "if !same_key_exec_v1(evidence_consumer, consumer) { return Ok(()); }"),
        item("release_physical_headroom", "release_header_exec_v1",
             "count > contents.leases.len() || count > observed_free_capacity", "count > contents.leases.len()"),
        item("release_group_count", "release_item_exec_v1",
             "if contents.readers[request.allocation.slot] < group", "if false"),
        item("acquire_journal_frame", "acquire_preflight_contents_exec_v1", acquire_frame,
             acquire_frame.replace("    acquire", "    let result = acquire") + ";\n    contents.journal.registration_watermark = 0;\n    result",
             "acquire_preflight_relation_v1"),
        item("acquire_output_frame", "acquire_preflight_contents_exec_v1", acquire_frame,
             acquire_frame.replace("    acquire", "    let result = acquire") + ";\n    if output.len() > 0 { output[0] = None; }\n    result",
             "acquire_preflight_relation_v1"),
        item("release_incarnation_frame", "release_preflight_contents_exec_v1", release_frame,
             release_frame.replace("    release", "    let result = release") + ";\n    contents.next_incarnation = 0;\n    result",
             "release_preflight_relation_v1"),
        item("unread_busy_class", "require_unread_exec_v1",
             "if contents.readers[reference.slot] != 0 { return Err(ReadErrorV1::AllocationBusy); }",
             "if contents.readers[reference.slot] != 0 { return Err(ReadErrorV1::InvalidState); }"),
    ]


def audit_source(source: str, dependency: Path, stripped: Path) -> None:
    require(source.isascii(), "ASCII proof source")
    require(source.startswith(PREFIX + IMPORT) and source.count(IMPORT) == 1, "exact initial pinned import")
    check_pin(dependency, PINS / "CONTEXT_VERSION_JOURNAL_ISSUANCE_SHA256")
    stripped.write_text(PREFIX + "\n\n" + source[len(PREFIX + IMPORT):], encoding="ascii")
    POLICY.scan(stripped)
    POLICY.scan(dependency)
    check_pin(dependency, PINS / "CONTEXT_VERSION_JOURNAL_ISSUANCE_SHA256")


def check_result(status, stdout, stderr, source, path, mutation):
    require(status == (1 if mutation else 0), "normal expected exit required")
    report = BASE.unique_json(stdout)
    expected = {"encountered-error": mutation is not None, "encountered-vir-error": False,
                "success": mutation is None, "verified": mutation.verified if mutation else POSITIVE_COUNT,
                "errors": 1 if mutation else 0, "is-verifying-entire-crate": True}
    actual = report["verification-results"]
    require(actual == expected and all(type(actual[key]) is type(value) for key, value in expected.items()),
            "exact whole-crate summary")
    diagnostics = [BASE.unique_json(line) for line in stderr.splitlines()]
    if mutation is None:
        require(not diagnostics, "positive diagnostic output")
        return
    require(len(diagnostics) in (1, 2), "exact diagnostic count")
    if len(diagnostics) == 2:
        footer = diagnostics[1]
        require(footer["$message_type"] == "diagnostic" and footer["message"] == "aborting due to 1 previous error"
                and footer["level"] == "error" and not footer["spans"] and not footer["children"]
                and footer["code"] is None, "exact error footer")
    diagnostic = diagnostics[0]
    require(diagnostic["$message_type"] == "diagnostic" and diagnostic["level"] == "error"
            and diagnostic["message"] == "postcondition not satisfied" and diagnostic["code"] is None
            and not diagnostic["children"], "intended postcondition error")
    spans = diagnostic["spans"]
    require(len(spans) == 2, "exact postcondition/exit spans")
    for span in spans:
        BASE.check_span(span, source, path)
    primary = [span for span in spans if span["is_primary"] is True]
    secondary = [span for span in spans if span["is_primary"] is False]
    require(len(primary) == len(secondary) == 1, "one primary and one exit")
    post, exit_span = primary[0], secondary[0]
    require((post["byte_start"], post["byte_end"]) == BASE.postcondition_bounds(source, mutation)
            and post["label"] == "failed this postcondition", "exact intended postcondition")
    _, body, end = BASE.function_bounds(source, mutation.function)
    require(body <= exit_span["byte_start"] < exit_span["byte_end"] <= end, "exit inside target body")
    require(exit_span["label"] in ("at this exit", "at the end of the function body"), "expected exit label")


def campaign(source: Path, verus: Path, timeout: int, output: Path):
    source, verus = source.resolve(strict=True), verus.resolve(strict=True)
    require(1 <= timeout <= 300, "timeout must be 1 through 300")
    require(verus.name == "verus", "named Verus executable required")
    check_pin(source, PINS / "CONTEXT_READ_PREFLIGHT_SHA256")
    check_pin(Path(__file__), PINS / "READ_PREFLIGHT_CHECKER_SHA256")
    check_pin(verus, PINS / "VERUS_SHA256")
    dependency = HERE / DEPENDENCY
    check_pin(dependency, PINS / "CONTEXT_VERSION_JOURNAL_ISSUANCE_SHA256")
    closure = ROOT / "examples/row_softmax_v1/verify-verus-closure.sh"
    manifest = PINS / "VERUS_CLOSURE_MANIFEST"
    check_pin(manifest, PINS / "VERUS_CLOSURE_MANIFEST_SHA256")
    require(digest(closure.read_bytes()) == "c0f5f201dca9ea6b3fa953884cdfaca8ca38413ad2a9de7700b3aaeb3a610d0c", "closure checker pin")
    inputs = [source, dependency, Path(__file__), Path(BASE.__file__), Path(POLICY.__file__),
              closure, manifest, verus, verus.parent / "rust_verify", verus.parent / "z3"]
    inputs.extend(PINS / name for name in ["CONTEXT_READ_PREFLIGHT_SHA256", "READ_PREFLIGHT_CHECKER_SHA256",
                  "CONTEXT_VERSION_JOURNAL_ISSUANCE_SHA256", "JOURNAL_ISSUANCE_CHECKER_SHA256",
                  "PROOF_SOURCE_CHECKER_SHA256", "VERUS_CLOSURE_MANIFEST_SHA256", "VERUS_SHA256"])
    identities = {str(path): digest(path.read_bytes()) for path in inputs}
    output.mkdir()
    (output / "inputs.json").write_text(json.dumps(identities, indent=2) + "\n")
    environment = {"HOME": os.environ.get("HOME", "/nonexistent")}
    for name in ("RUSTUP_HOME", "CARGO_HOME"):
        environment[name] = os.environ.get(name, str(Path(environment["HOME"]) / (".rustup" if name == "RUSTUP_HOME" else ".cargo")))
    environment["PATH"] = environment["CARGO_HOME"] + "/bin:/usr/bin:/bin"
    environment["VERUS_Z3_PATH"] = str(verus.parent / "z3")
    original, original_dependency = source.read_text(), dependency.read_bytes()
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
            candidate, candidate_dependency = out / "reader.rs", out / DEPENDENCY
            expected = (BASE.mutate(original, mutation) if mutation else original).encode("ascii")
            candidate.write_bytes(expected)
            candidate_dependency.write_bytes(original_dependency)
            audit_source(expected.decode("ascii"), candidate_dependency, out / "audited-reader-body.rs")
            generated = {str(path): digest(path.read_bytes()) for path in [candidate, candidate_dependency, out / "audited-reader-body.rs"]}
            (out / "sources.json").write_text(json.dumps(generated, indent=2) + "\n")
            command = ["/usr/bin/timeout", "--foreground", "--signal=TERM", "--kill-after=5", str(timeout),
                       str(verus), "--crate-type", "lib", "--triggers-mode", "silent", "--no-cheating",
                       "--output-json", "--error-format=json", "--num-threads", "4", str(candidate)]
            require(candidate.read_bytes() == expected and candidate_dependency.read_bytes() == original_dependency,
                    "exact solver inputs before")
            status, stdout, stderr = BASE.run_owned(command, timeout + 10, out / "solver", environment)
            require({path: digest(Path(path).read_bytes()) for path in generated} == generated, "generated source replacement")
            check_result(status, stdout, stderr, expected.decode("ascii"), candidate, mutation)
            require({str(path): digest(path.read_bytes()) for path in inputs} == identities, "source/tool replacement")
            print(f"PASS: reader preflight {name}", flush=True)
    require({str(path): digest(path.read_bytes()) for path in inputs} == identities, "final source/tool identities")
    print(f"READ_PREFLIGHT_OK obligations={POSITIVE_COUNT} inherited={INHERITED_COUNT} new={POSITIVE_COUNT - INHERITED_COUNT} executable_mutations={len(mutations())}", flush=True)


def self_test(source: str):
    for case in mutations():
        BASE.postcondition_bounds(BASE.mutate(source, case), case)
    case = mutations()[0]
    changed = BASE.mutate(source, case)
    path = Path("/owned/reader.rs")
    start, end = BASE.postcondition_bounds(changed, case)
    _, body, finish = BASE.function_bounds(changed, case.function)

    def span(begin, finish, primary, label):
        first, last = changed.count("\n", 0, begin) + 1, changed.count("\n", 0, finish) + 1
        return {"file_name": str(path), "byte_start": begin, "byte_end": finish,
                "line_start": first, "line_end": last, "column_start": begin - changed.rfind("\n", 0, begin),
                "column_end": finish - changed.rfind("\n", 0, finish), "is_primary": primary,
                "label": label, "expansion": None,
                "text": [{"text": line} for line in changed.splitlines()[first - 1:last]]}

    report = {"verification-results": {"encountered-error": True, "encountered-vir-error": False,
              "success": False, "verified": POSITIVE_COUNT - 1, "errors": 1, "is-verifying-entire-crate": True}}
    diagnostic = {"$message_type": "diagnostic", "message": "postcondition not satisfied", "code": None,
                  "level": "error", "children": [], "spans": [span(start, end, True, "failed this postcondition"),
                  span(body, finish, False, "at the end of the function body")]}
    stdout, stderr = json.dumps(report), json.dumps(diagnostic)
    check_result(1, stdout, stderr, changed, path, case)
    positive = copy.deepcopy(report)
    positive["verification-results"].update({"encountered-error": False, "success": True, "verified": POSITIVE_COUNT, "errors": 0})
    check_result(0, json.dumps(positive), "", source, path, None)
    rejected = 0

    def rejects(status=1, out=stdout, err=stderr):
        nonlocal rejected
        try:
            check_result(status, out, err, changed, path, case)
        except (ValueError, KeyError, TypeError):
            rejected += 1
        else:
            raise ValueError("parser accepted adverse diagnostic")

    for status in [0, 2, 101, 124, 137, -9, -15]:
        rejects(status=status)
    for field, value in [("verified", 69), ("verified", 102.0), ("errors", 2), ("errors", True),
                         ("encountered-vir-error", True), ("is-verifying-entire-crate", False), ("success", 0)]:
        bad = copy.deepcopy(report)
        bad["verification-results"][field] = value
        rejects(out=json.dumps(bad))
    for field, value in [("message", "precondition not satisfied"), ("code", {"code": "E0308"}),
                         ("level", "warning"), ("children", [{"message": "solver timed out"}])]:
        bad = copy.deepcopy(diagnostic)
        bad[field] = value
        rejects(err=json.dumps(bad))
    for field, value in [("file_name", "/owned/" + DEPENDENCY), ("byte_start", start + 1),
                         ("byte_end", end - 1), ("line_start", 1), ("column_start", 1),
                         ("label", "different postcondition"), ("text", []), ("expansion", {})]:
        bad = copy.deepcopy(diagnostic)
        bad["spans"][0][field] = value
        rejects(err=json.dumps(bad))
    rejects(out=stdout + stdout)
    rejects(out=stdout.replace('"errors": 1', '"errors": 1, "errors": 1'))
    rejects(err=stderr + "\n" + stderr)
    rejects(err=stderr + "\nsolver timed out")
    rejects(err="")
    bad = copy.deepcopy(diagnostic)
    bad["spans"][1] = span(0, 2, False, "at this exit")
    rejects(err=json.dumps(bad))
    with tempfile.TemporaryDirectory(prefix="fe2o3-reader-audit-") as temporary:
        stripped = Path(temporary) / "body.rs"
        audit_source(source, HERE / DEPENDENCY, stripped)
        for invalid in [source.replace(IMPORT, IMPORT.replace(DEPENDENCY, "foreign.rs")),
                        "/*\n" + source + "\n*/", source + "\nmod foreign;\n",
                        source + '\ninclude!("foreign.rs");\n', source + '\n#[cfg(false)] fn hidden() {}\n',
                        source + '\nverus! { proof fn bypass() { assume(false); } }\n']:
            try:
                audit_source(invalid, HERE / DEPENDENCY, stripped)
            except (ValueError, POLICY.ScanError):
                rejected += 1
            else:
                raise ValueError("import/source auditor accepted adverse source")
        module_path, module_pin = Path(temporary) / "cached.py", Path(temporary) / "cached.pin"
        module_path.write_bytes(b"VALUE = 2\n")
        original_stat = module_path.stat()
        py_compile.compile(str(module_path), doraise=True)
        module_path.write_bytes(b"VALUE = 1\n")
        os.utime(module_path, ns=(original_stat.st_atime_ns, original_stat.st_mtime_ns))
        module_pin.write_text(digest(module_path.read_bytes()) + "\n")
        require(pinned_module("reader_cache_self_test", module_path, module_pin).VALUE == 1,
                "pinned loader must ignore timestamp-valid stale bytecode")
        del sys.modules["reader_cache_self_test"]
    BASE.process_self_test()
    print(f"PASS: reader self-test ({len(mutations())} mutations, {rejected} adverse diagnostics/sources rejected)")


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
