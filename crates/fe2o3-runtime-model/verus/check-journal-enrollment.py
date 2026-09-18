#!/usr/bin/env python3
"""Qualify enrollment prerequisites only, never the unrefined sorting phase."""

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import sys

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
PINS = HERE / "pins"
PREFIX = (
    "// Enrollment prerequisites: exact admission prefix and conditional commit suffix.\n"
    "// The production sorting/search middle phase is not refined by this packet.\n"
)
INCLUDE = 'include!("context_read_invariant_v1.rs");\n'
COUNT = 168
DEPENDENCIES = {
    "context_read_invariant_v1.rs": "CONTEXT_READ_INVARIANT_SHA256",
    "context_read_commit_v1.rs": "CONTEXT_READ_COMMIT_SHA256",
    "context_read_preflight_v1.rs": "CONTEXT_READ_PREFLIGHT_SHA256",
    "context_version_journal_issuance_v1.rs": "CONTEXT_VERSION_JOURNAL_ISSUANCE_SHA256",
}


def need(condition, message):
    if not condition:
        raise ValueError(message)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def inherited():
    path = HERE / "check-read-invariant.py"
    data = path.read_bytes()
    need(
        hashlib.sha256(data).hexdigest()
        == (PINS / "READ_INVARIANT_CHECKER_SHA256").read_text().strip(),
        "pinned inherited checker",
    )
    spec = importlib.util.spec_from_file_location("enrollment_j4", path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    exec(compile(data, str(path), "exec"), module.__dict__)
    return module


def mutations(base):
    header = "enrollment_header_exec_v1"
    entry = "enrollment_entry_error_exec_v1"
    commit = "enrollment_commit_suffix_exec_v1"
    return [
        base.Mutation(
            "foreign_precedence",
            entry,
            "entry.key.context_generation != context || entry.device.context_generation != context",
            "entry.key.context_generation != context && entry.device.context_generation != context",
            "enrollment_entry_error_v1",
            COUNT - 1,
        ),
        base.Mutation(
            "allocation_error_identity",
            entry,
            "} else if !issuable_id_exec_v1(entry.key.local) {\n        Some(EnrollmentErrorV1::InvalidAllocationId)",
            "} else if !issuable_id_exec_v1(entry.key.local) {\n        Some(EnrollmentErrorV1::InvalidDeviceId)",
            "enrollment_entry_error_v1",
            COUNT - 1,
        ),
        base.Mutation(
            "shape_error_identity",
            header,
            "if entries.len() > capacity || output.len() != entries.len() {\n        return Err(EnrollmentErrorV1::RosterCapacity);",
            "if entries.len() > capacity || output.len() != entries.len() {\n        return Err(EnrollmentErrorV1::InvalidState);",
            "enrollment_header_decision_v1",
            COUNT - 1,
        ),
        base.Mutation(
            "empty_batch_precedence",
            header,
            "if entries.len() == 0 { return Ok(()); }",
            "if entries.len() == 0 { return Err(EnrollmentErrorV1::InvalidState); }",
            "enrollment_header_decision_v1",
            COUNT - 1,
        ),
        base.Mutation(
            "rejection_output_mutation",
            header,
            "if output[index].is_some() { return Err(EnrollmentErrorV1::InvalidState); }",
            "if output[index].is_some() { output[index] = None; return Err(EnrollmentErrorV1::InvalidState); }",
            "final(output)@ == old(output)@",
            COUNT - 1,
        ),
        base.Mutation(
            "retained_free_prefix",
            commit,
            "contents.journal.allocation_free.truncate(remaining);",
            "contents.journal.allocation_free.truncate(0);",
            "final(contents).journal.allocation_free@ == old(contents).journal.allocation_free@.subrange(0, remaining as int)",
            COUNT - 1,
        ),
        base.Mutation(
            "writer_watermark_frame",
            commit,
            "contents.journal.allocation_free.truncate(remaining);",
            "contents.journal.allocation_free.truncate(remaining);\n    contents.journal.registration_watermark = 0;",
            "enrollment_untouched_journal_v1",
            COUNT - 1,
        ),
    ]


def check_result(base, status, stdout, stderr, source, path, mutation):
    need(status == (1 if mutation else 0), "normal expected solver exit")
    result = base.unique_json(stdout)["verification-results"]
    expected = {
        "encountered-error": mutation is not None,
        "encountered-vir-error": False,
        "success": mutation is None,
        "verified": COUNT - 1 if mutation else COUNT,
        "errors": 1 if mutation else 0,
        "is-verifying-entire-crate": True,
    }
    need(
        result == expected
        and all(type(result[key]) is type(value) for key, value in expected.items()),
        "exact whole-crate solver result",
    )
    diagnostics = [base.unique_json(line) for line in stderr.splitlines()]
    if mutation is None:
        need(not diagnostics, "no positive diagnostics")
        return
    need(len(diagnostics) in (1, 2), "one error and optional compiler footer")
    if len(diagnostics) == 2:
        footer = diagnostics.pop()
        need(
            footer["message"] == "aborting due to 1 previous error"
            and footer["level"] == "error"
            and footer["code"] is None
            and not footer["spans"]
            and not footer["children"],
            "exact footer",
        )
    diagnostic = diagnostics[0]
    need(
        diagnostic["message"] == "postcondition not satisfied"
        and diagnostic["level"] == "error"
        and diagnostic["code"] is None
        and not diagnostic["children"],
        "intended postcondition failure",
    )
    spans = diagnostic["spans"]
    need(len(spans) == 2, "exact postcondition and exit spans")
    for span in spans:
        base.check_span(span, source, path)
    primary = [span for span in spans if span["is_primary"] is True]
    exits = [span for span in spans if span["is_primary"] is False]
    need(len(primary) == len(exits) == 1, "one primary and one exit")
    start, body, end = base.function_bounds(source, mutation.function)
    post, exit_span = primary[0], exits[0]
    need(
        start <= post["byte_start"] < post["byte_end"] <= body
        and mutation.postcondition in source[post["byte_start"] : post["byte_end"]]
        and post["label"] == "failed this postcondition",
        "exact target function and specified postcondition",
    )
    need(
        body <= exit_span["byte_start"] < exit_span["byte_end"] <= end
        and exit_span["label"] in ("at this exit", "at the end of the function body"),
        "exit inside mutated executable body",
    )


def campaign(args):
    need(__debug__, "assertions must be enabled")
    j4 = inherited()
    base = j4.BASE
    source_path = HERE / "context_version_journal_enrollment_v1.rs"
    source = source_path.read_text()
    need(
        source.isascii() and source.startswith(PREFIX + INCLUDE), "exact pinned include"
    )
    need(source.count(INCLUDE) == 1, "single inherited include")
    cases = (
        [("positive_before", None)]
        + [(m.name, m) for m in mutations(base)]
        + [("positive_after", None)]
    )
    candidates = {
        name: base.mutate(source, mutation) if mutation else source
        for name, mutation in cases
    }
    verus = args.verus.resolve(strict=True)
    need(verus.name == "verus", "real Verus executable")
    j4.check_pin(verus, PINS / "VERUS_SHA256")
    j4.check_pin(
        PINS / "VERUS_CLOSURE_MANIFEST", PINS / "VERUS_CLOSURE_MANIFEST_SHA256"
    )
    closure = HERE.parents[2] / "examples/row_softmax_v1/verify-verus-closure.sh"
    need(
        sha(closure)
        == "c0f5f201dca9ea6b3fa953884cdfaca8ca38413ad2a9de7700b3aaeb3a610d0c",
        "pinned Verus closure checker",
    )
    for name, pin in DEPENDENCIES.items():
        j4.check_pin(HERE / name, PINS / pin)
    output = args.output.resolve()
    output.mkdir()
    identities = {
        str(path): sha(path)
        for path in [
            source_path,
            Path(__file__),
            *[HERE / name for name in DEPENDENCIES],
            *[
                Path(module.__file__)
                for module in (j4, j4.COMMIT, j4.PREFLIGHT, base, j4.POLICY)
            ],
            verus,
            verus.parent / "rust_verify",
            verus.parent / "z3",
            PINS / "VERUS_CLOSURE_MANIFEST",
            closure,
            *[PINS / pin for pin in DEPENDENCIES.values()],
            *[
                PINS / pin
                for pin in (
                    "VERUS_SHA256",
                    "VERUS_CLOSURE_MANIFEST_SHA256",
                    "READ_INVARIANT_CHECKER_SHA256",
                    "READ_COMMIT_CHECKER_SHA256",
                    "READ_PREFLIGHT_CHECKER_SHA256",
                    "JOURNAL_ISSUANCE_CHECKER_SHA256",
                    "PROOF_SOURCE_CHECKER_SHA256",
                )
            ],
        ]
    }
    (output / "inputs-before.json").write_text(json.dumps(identities, indent=2) + "\n")
    environment = {"HOME": os.environ.get("HOME", "/nonexistent")}
    for key, default in (("RUSTUP_HOME", ".rustup"), ("CARGO_HOME", ".cargo")):
        environment[key] = os.environ.get(key, str(Path(environment["HOME"]) / default))
    environment["PATH"] = environment["CARGO_HOME"] + "/bin:/usr/bin:/bin"
    environment["VERUS_Z3_PATH"] = str(verus.parent / "z3")
    (output / "environment.json").write_text(json.dumps(environment, indent=2) + "\n")
    closure_command = [
        "/bin/sh",
        str(closure),
        str(verus.parent),
        str(PINS / "VERUS_CLOSURE_MANIFEST"),
    ]
    status, _, _ = base.run_owned(
        closure_command, 120, output / "closure-before", environment
    )
    need(status == 0, "complete pinned Verus distribution before campaign")
    for name, mutation in cases:
        case = output / name
        case.mkdir()
        current = candidates[name]
        path = case / source_path.name
        path.write_text(current, encoding="ascii")
        for dependency in DEPENDENCIES:
            (case / dependency).write_bytes((HERE / dependency).read_bytes())
        j4.audit_source((case / "context_read_invariant_v1.rs").read_text(), case)
        stripped = case / "audited-enrollment-body.rs"
        stripped.write_text(PREFIX + current[len(PREFIX + INCLUDE) :], encoding="ascii")
        j4.POLICY.scan(stripped)
        command = [
            "/usr/bin/timeout",
            "--foreground",
            "--signal=TERM",
            "--kill-after=5",
            str(args.timeout),
            str(verus),
            "--crate-type",
            "lib",
            "--triggers-mode",
            "silent",
            "--no-cheating",
            "--output-json",
            "--error-format=json",
            "--num-threads",
            "2",
            str(path),
        ]
        status, stdout, stderr = base.run_owned(
            command, args.timeout + 10, case / "solver", environment
        )
        need(path.read_text() == current, "case source unchanged")
        for dependency, pin in DEPENDENCIES.items():
            j4.check_pin(case / dependency, PINS / pin)
        check_result(base, status, stdout, stderr, current, path, mutation)
        print(name + ": PASS", flush=True)
    status, _, _ = base.run_owned(
        closure_command, 120, output / "closure-after", environment
    )
    need(status == 0, "complete pinned Verus distribution after campaign")
    after = {path: sha(Path(path)) for path in identities}
    (output / "inputs-after.json").write_text(json.dumps(after, indent=2) + "\n")
    need(after == identities, "all campaign inputs unchanged")
    report = {
        "scope": "admission prefix and conditional commit suffix only",
        "positive_obligations": COUNT,
        "inherited_obligations": 155,
        "negative_cases": len(cases) - 2,
        "full_batch_enrollment_verified": False,
        "rust_sorting_search_refined": False,
        "native_or_performance_acceptance": False,
    }
    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--verus", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--timeout", type=int, default=180)
    args = parser.parse_args()
    need(1 <= args.timeout <= 300, "timeout must be 1 through 300 seconds")
    module = inherited().BASE
    for signum in module.SIGNALS:
        signal.signal(signum, module.interrupted)
    campaign(args)


if __name__ == "__main__":
    main()
