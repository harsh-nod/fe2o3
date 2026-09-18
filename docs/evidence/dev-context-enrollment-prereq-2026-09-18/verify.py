#!/usr/bin/env python3
"""Audit retained enrollment proof receipts without executing Verus or Cargo."""

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path, PurePosixPath
import re
import shutil
import sys
import tempfile

sys.dont_write_bytecode = True
ORIGINAL = Path("/home/harsh/.codex-tmp/fe2o3-c4-completion-20260917")
RUNS = {
    "corrected": Path(
        "/home/harsh/.codex-tmp/fe2o3-enrollment-prereq-20260918-qualified"
    ),
    "preliminary": Path(
        "/home/harsh/.codex-tmp/fe2o3-enrollment-prereq-20260918-first"
    ),
}
TOOL = Path("/home/harsh/.local/opt/verus-0.2026.08.09.92f466f")
PROOFS = Path("crates/fe2o3-runtime-model/verus")
SUBJECT = "context_version_journal_enrollment_v1.rs"
CHECKER = "check-journal-enrollment.py"
OLD_CHECKER = "e4efbd92a6ed127ea3ed7a70ff93c5f3e7a9df0c47eaf770a8bac5002f1848a0"
PINNED_CHECKER = "8a7ed73f19fa80a2cd8144f6f56d54cb68cf19ce169da657181acfca58c2026a"
INCLUDES = (
    "context_read_invariant_v1.rs",
    "context_read_commit_v1.rs",
    "context_read_preflight_v1.rs",
    "context_version_journal_issuance_v1.rs",
)
HELPERS = (
    "check-read-invariant.py",
    "check-read-commit.py",
    "check-read-preflight.py",
    "check-journal-issuance.py",
)
PINS = (
    "CONTEXT_READ_INVARIANT_SHA256",
    "CONTEXT_READ_COMMIT_SHA256",
    "CONTEXT_READ_PREFLIGHT_SHA256",
    "CONTEXT_VERSION_JOURNAL_ISSUANCE_SHA256",
    "VERUS_SHA256",
    "VERUS_CLOSURE_MANIFEST_SHA256",
    "READ_INVARIANT_CHECKER_SHA256",
    "READ_COMMIT_CHECKER_SHA256",
    "READ_PREFLIGHT_CHECKER_SHA256",
    "JOURNAL_ISSUANCE_CHECKER_SHA256",
    "PROOF_SOURCE_CHECKER_SHA256",
)
CLOSURE = Path("examples/row_softmax_v1/verify-verus-closure.sh")
SOURCE_FILES = tuple(
    [PROOFS / name for name in (SUBJECT, CHECKER, *INCLUDES, *HELPERS)]
    + [PROOFS / "pins" / pin for pin in PINS]
    + [
        PROOFS / "pins/VERUS_CLOSURE_MANIFEST",
        Path("examples/wave64_collectives_v1/check-proof-source.py"),
        CLOSURE,
    ]
)
ARCHIVE_RELATIVE = Path("docs/evidence/dev-context-enrollment-prereq-2026-09-18")
NAMES = (
    "positive_before",
    "foreign_precedence",
    "allocation_error_identity",
    "shape_error_identity",
    "empty_batch_precedence",
    "rejection_output_mutation",
    "retained_free_prefix",
    "writer_watermark_frame",
    "positive_after",
)
CASE_FILES = {
    SUBJECT,
    *INCLUDES,
    "audited-enrollment-body.rs",
    "audited-invariant-body.rs",
    "audited-commit-body.rs",
    "audited-preflight-body.rs",
    "solver/record.json",
    "solver/stdout.log",
    "solver/stderr.log",
}
REPORT = {
    "scope": "admission prefix and conditional commit suffix only",
    "positive_obligations": 168,
    "inherited_obligations": 155,
    "negative_cases": 7,
    "full_batch_enrollment_verified": False,
    "rust_sorting_search_refined": False,
    "native_or_performance_acceptance": False,
}
CLOSURE_OUTPUT = (
    "PASS: pinned Verus release closure matched at this measurement "
    "(190 files, 129019839 bytes)\n"
)


def need(condition, message):
    if not condition:
        raise ValueError(message)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def pairs(items):
    result = {}
    for key, value in items:
        need(key not in result, "duplicate JSON key")
        result[key] = value
    return result


def invalid_constant(value):
    raise ValueError(f"non-JSON constant: {value}")


def load(path):
    return json.loads(
        path.read_text(), object_pairs_hook=pairs, parse_constant=invalid_constant
    )


def equal(actual, expected, message):
    need(
        json.dumps(actual, sort_keys=True) == json.dumps(expected, sort_keys=True),
        message,
    )


def files(root):
    need(root.is_dir() and not root.is_symlink(), "ordinary archive directory")
    result = set()
    for path in root.rglob("*"):
        need(not path.is_symlink(), f"no symlink: {path}")
        need(path.is_file() or path.is_dir(), f"ordinary archive entry: {path}")
        if path.is_file():
            result.add(path.relative_to(root).as_posix())
    return result


def manifest(root):
    text = (root / "SHA256SUMS").read_text(encoding="ascii")
    need(text.endswith("\n"), "manifest trailing newline")
    entries = {}
    for line in text.splitlines():
        match = re.fullmatch(r"([0-9a-f]{64})  ([A-Za-z0-9_./-]+)", line)
        need(match is not None, "manifest record")
        digest, name = match.groups()
        parts = PurePosixPath(name)
        need(
            not parts.is_absolute()
            and ".." not in parts.parts
            and parts.as_posix() == name
            and name != "SHA256SUMS"
            and name not in entries,
            "unique normalized manifest path",
        )
        entries[name] = digest
    need(list(entries) == sorted(entries), "sorted manifest")
    need(files(root) == set(entries) | {"SHA256SUMS"}, "exact manifest closure")
    for name, digest in entries.items():
        need(sha(root / name) == digest, f"manifest hash: {name}")
    return len(entries)


def source_inputs(root):
    corrected = root / "corrected"
    before = load(corrected / "inputs-before.json")
    equal(load(corrected / "inputs-after.json"), before, "unchanged corrected inputs")
    keys = {str(ORIGINAL / name) for name in SOURCE_FILES}
    keys |= {str(TOOL / name) for name in ("verus", "rust_verify", "z3")}
    need(type(before) is dict and set(before) == keys, "exact 27-input roster")
    need(
        all(
            type(h) is str and re.fullmatch(r"[0-9a-f]{64}", h) for h in before.values()
        ),
        "input hash types",
    )
    snapshot = root / "source"
    need(files(snapshot) == {p.as_posix() for p in SOURCE_FILES}, "snapshot closure")
    for name in SOURCE_FILES:
        need(sha(snapshot / name) == before[str(ORIGINAL / name)], f"snapshot: {name}")
    need(
        sha(snapshot / PROOFS / CHECKER) == PINNED_CHECKER, "qualified checker identity"
    )
    release = (snapshot / PROOFS / "pins/VERUS_CLOSURE_MANIFEST").read_text()
    for name in ("verus", "rust_verify", "z3"):
        lines = [
            line
            for line in release.splitlines()
            if line.startswith(f"required={name}|")
        ]
        need(len(lines) == 1, "unique release tool")
        need(
            lines[0].split("|")[-1] == before[str(TOOL / name)], "release tool identity"
        )
    old = dict(before)
    old[str(ORIGINAL / PROOFS / CHECKER)] = OLD_CHECKER
    equal(
        load(root / "preliminary/inputs-before.json"), old, "preliminary identity delta"
    )
    return before


def checker(root):
    path = root / "source" / PROOFS / CHECKER
    spec = importlib.util.spec_from_file_location("archived_enrollment_checker", path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    exec(compile(path.read_bytes(), str(path), "exec"), module.__dict__)
    j4 = module.inherited()
    return module, j4


def record(directory, command, status):
    value = load(directory / "record.json")
    need(
        set(value)
        == {
            "command",
            "started_ns",
            "finished_ns",
            "process_group",
            "group_absent",
            "status",
        },
        "closed normal process receipt fields",
    )
    equal(value["command"], command, "exact historical command")
    need(
        type(value["status"]) is int and value["status"] == status,
        "normal expected exit",
    )
    need(value["group_absent"] is True, "recorded process group absent")
    need(
        type(value["process_group"]) is int and value["process_group"] > 0,
        "process group",
    )
    need(
        type(value["started_ns"]) is int
        and type(value["finished_ns"]) is int
        and 0 < value["started_ns"] <= value["finished_ns"],
        "ordered realtime timestamps",
    )
    return value


def closure(root, run, phase):
    directory = root / run / phase
    receipt = record(
        directory,
        [
            "/bin/sh",
            str(ORIGINAL / CLOSURE),
            str(TOOL),
            str(ORIGINAL / PROOFS / "pins/VERUS_CLOSURE_MANIFEST"),
        ],
        0,
    )
    need(
        (directory / "stdout.log").read_text() == CLOSURE_OUTPUT, "exact closure result"
    )
    need((directory / "stderr.log").read_bytes() == b"", "no closure diagnostics")
    return receipt


def case(root, run, name, subject, mutation, module, j4):
    directory = root / run / name
    need(files(directory) == CASE_FILES, f"exact case files: {run}/{name}")
    original_path = RUNS[run] / name / SUBJECT
    candidate = j4.BASE.mutate(subject, mutation) if mutation else subject
    need((directory / SUBJECT).read_text() == candidate, "exact reversible case source")
    for include in INCLUDES:
        need(
            (directory / include).read_bytes()
            == (root / "source" / PROOFS / include).read_bytes(),
            "exact inherited source",
        )
    # The historical source audit writes derived bodies; regenerate only in owned scratch.
    with tempfile.TemporaryDirectory(prefix="fe2o3-enrollment-audit-") as scratch:
        scratch = Path(scratch)
        for include in INCLUDES:
            shutil.copyfile(directory / include, scratch / include)
        j4.audit_source((scratch / INCLUDES[0]).read_text(), scratch)
        body = module.PREFIX + candidate[len(module.PREFIX + module.INCLUDE) :]
        (scratch / "audited-enrollment-body.rs").write_text(body, encoding="ascii")
        j4.POLICY.scan(scratch / "audited-enrollment-body.rs")
        for derived in scratch.glob("audited-*.rs"):
            need(
                derived.read_bytes() == (directory / derived.name).read_bytes(),
                "derived audit body",
            )
    command = [
        "/usr/bin/timeout",
        "--foreground",
        "--signal=TERM",
        "--kill-after=5",
        "180",
        str(TOOL / "verus"),
        "--crate-type",
        "lib",
        "--triggers-mode",
        "silent",
        "--no-cheating",
        "--output-json",
        "--error-format=json",
        "--num-threads",
        "2",
        str(original_path),
    ]
    status = 1 if mutation else 0
    receipt = record(directory / "solver", command, status)
    module.check_result(
        j4.BASE,
        status,
        (directory / "solver/stdout.log").read_text(),
        (directory / "solver/stderr.log").read_text(),
        candidate,
        original_path,
        mutation,
    )
    return receipt


def audit(root, require_manifest=True):
    root = root.resolve()
    if require_manifest:
        manifest(root)
    else:
        files(root)
    source_inputs(root)
    module, j4 = checker(root)
    subject = (root / "source" / PROOFS / SUBJECT).read_text()
    need(
        subject.isascii() and subject.startswith(module.PREFIX + module.INCLUDE),
        "subject header",
    )
    need(subject.count(module.INCLUDE) == 1, "one inherited include")
    mutations = {item.name: item for item in module.mutations(j4.BASE)}
    need(tuple(mutations) == NAMES[1:-1], "seven exact mutation cases")
    environments = {
        "HOME": "/home/harsh",
        "RUSTUP_HOME": "/home/harsh/.rustup",
        "CARGO_HOME": "/home/harsh/.cargo",
        "PATH": "/home/harsh/.cargo/bin:/usr/bin:/bin",
        "VERUS_Z3_PATH": str(TOOL / "z3"),
    }
    intervals = []
    for run, names in (("preliminary", NAMES[:2]), ("corrected", NAMES)):
        expected = {"inputs-before.json", "environment.json"}
        phases = (
            ("closure-before",)
            if run == "preliminary"
            else ("closure-before", "closure-after")
        )
        if run == "corrected":
            expected |= {"inputs-after.json", "report.json"}
        expected |= {f"{name}/{file}" for name in names for file in CASE_FILES}
        expected |= {
            f"{phase}/{file}"
            for phase in phases
            for file in ("record.json", "stdout.log", "stderr.log")
        }
        need(files(root / run) == expected, f"exact {run} receipt closure")
        equal(
            load(root / run / "environment.json"),
            environments,
            "exact recorded environment",
        )
        receipts = [closure(root, run, "closure-before")]
        receipts += [
            case(root, run, name, subject, mutations.get(name), module, j4)
            for name in names
        ]
        if run == "corrected":
            receipts.append(closure(root, run, "closure-after"))
            equal(load(root / run / "report.json"), REPORT, "narrow corrected report")
        need(
            len({r["process_group"] for r in receipts}) == len(receipts),
            "distinct recorded process groups",
        )
        for left, right in zip(receipts, receipts[1:]):
            need(left["finished_ns"] <= right["started_ns"], "serial receipt ordering")
        intervals.append((receipts[0]["started_ns"], receipts[-1]["finished_ns"]))
    need(intervals[0][1] < intervals[1][0], "preliminary precedes corrected campaign")
    result = {
        "corrected_positive_runs": 2,
        "verified_per_positive": 168,
        "inherited_obligations": 155,
        "new_obligations": 13,
        "corrected_negative_runs": 7,
        "verified_per_negative": 167,
        "intended_errors_per_negative": 1,
        "preliminary_retained_solver_runs": 2,
        "closed_owned_process_receipts": 14,
        "current_tool_installation_revalidated": False,
        "complete_batch_enrollment_verified": False,
    }
    if require_manifest:
        packaging(root, result)
    return result


CALIBRATIONS = (
    "positive_count",
    "negative_count",
    "boolean_error_count",
    "partial_crate",
    "compiler_error",
    "diagnostic_path",
    "diagnostic_byte_offset",
    "timeout_exit",
    "cleanup_failure",
    "reversed_timestamps",
    "command_change",
    "restored_mutation",
    "include_change",
    "derived_body_change",
    "overclaim",
    "after_identity_change",
    "preliminary_checker_change",
    "preliminary_fabricated_completion",
    "closure_output",
    "environment_change",
    "source_snapshot_change",
    "current_source_drift",
    "manifest_hash",
    "manifest_extra",
    "manifest_missing",
    "manifest_duplicate",
    "manifest_symlink",
)


def packaging(root, result):
    expected_files = {
        f"{name}/{file}"
        for name in ("portable-audit", "calibration", "source-match")
        for file in ("record.json", "stdout.log", "stderr.log")
    }
    need(files(root / "packaging") == expected_files, "packaging receipt closure")
    script = ORIGINAL / ARCHIVE_RELATIVE
    recipes = (
        ("portable-audit", ["verify.py", "--unsealed"], result),
        (
            "calibration",
            ["test_verifier.py", "--unsealed"],
            {"passed": list(CALIBRATIONS), "count": len(CALIBRATIONS)},
        ),
        (
            "source-match",
            ["verify.py", "--unsealed", "--source-root", str(ORIGINAL)],
            {**result, "current_source_files_matched": len(SOURCE_FILES)},
        ),
    )
    receipts = []
    for name, arguments, expected in recipes:
        command = ["/usr/bin/python3", "-B", str(script / arguments[0]), *arguments[1:]]
        directory = root / "packaging" / name
        receipts.append(record(directory, command, 0))
        need((directory / "stderr.log").read_bytes() == b"", "no packaging diagnostics")
        equal(load(directory / "stdout.log"), expected, "exact packaging result")
    for left, right in zip(receipts, receipts[1:]):
        need(left["finished_ns"] <= right["started_ns"], "serial packaging commands")


def source_match(root, repository):
    before = source_inputs(root)
    for relative in SOURCE_FILES:
        path = repository / relative
        need(
            path.is_file() and not path.is_symlink(), f"current source file: {relative}"
        )
        need(
            sha(path) == before[str(ORIGINAL / relative)],
            f"current source drift: {relative}",
        )
    return len(SOURCE_FILES)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", type=Path)
    parser.add_argument(
        "--unsealed",
        action="store_true",
        help="pre-seal semantic audit; no archive integrity claim",
    )
    args = parser.parse_args()
    root = Path(__file__).resolve().parent
    result = audit(root, require_manifest=not args.unsealed)
    if args.source_root:
        result["current_source_files_matched"] = source_match(
            root, args.source_root.resolve()
        )
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
