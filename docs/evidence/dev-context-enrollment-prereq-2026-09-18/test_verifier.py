#!/usr/bin/env python3
"""Adverse archive fixtures: no proof reruns and no original receipt modifications."""

import argparse
import importlib.util
import json
from pathlib import Path
import shutil
import sys
import tempfile

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location(
    "enrollment_archive_audit", HERE / "verify.py"
)
V = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = V
exec(
    compile((HERE / "verify.py").read_bytes(), str(HERE / "verify.py"), "exec"),
    V.__dict__,
)


def json_field(path, keys, value):
    data = V.load(path)
    target = data
    for key in keys[:-1]:
        target = target[key]
    target[keys[-1]] = value
    path.write_text(json.dumps(data) + "\n")


def diagnostic_field(path, keys, value):
    lines = path.read_text().splitlines()
    data = json.loads(lines[0])
    target = data
    for key in keys[:-1]:
        target = target[key]
    target[keys[-1]] = value
    lines[0] = json.dumps(data)
    path.write_text("\n".join(lines) + "\n")


def reject(callback, message):
    try:
        callback()
    except ValueError as error:
        V.need(
            message in str(error),
            f"wrong calibration rejection: {error}; expected {message}",
        )
    else:
        raise ValueError(f"accepted adverse archive: {message}")


def fixture_changes():
    positive = Path("corrected/positive_before/solver")
    negative = Path("corrected/foreign_precedence/solver")
    return [
        (
            "positive_count",
            positive / "stdout.log",
            ["verification-results", "verified"],
            169,
            "exact whole-crate solver result",
        ),
        (
            "negative_count",
            negative / "stdout.log",
            ["verification-results", "verified"],
            168,
            "exact whole-crate solver result",
        ),
        (
            "boolean_error_count",
            negative / "stdout.log",
            ["verification-results", "errors"],
            True,
            "exact whole-crate solver result",
        ),
        (
            "partial_crate",
            negative / "stdout.log",
            ["verification-results", "is-verifying-entire-crate"],
            False,
            "exact whole-crate solver result",
        ),
        (
            "timeout_exit",
            negative / "record.json",
            ["status"],
            124,
            "normal expected exit",
        ),
        (
            "cleanup_failure",
            negative / "record.json",
            ["group_absent"],
            False,
            "recorded process group absent",
        ),
        (
            "reversed_timestamps",
            negative / "record.json",
            ["finished_ns"],
            1,
            "ordered realtime timestamps",
        ),
        (
            "command_change",
            negative / "record.json",
            ["command"],
            ["verus", "--verify-only-module", "x"],
            "exact historical command",
        ),
        (
            "overclaim",
            Path("corrected/report.json"),
            ["full_batch_enrollment_verified"],
            True,
            "narrow corrected report",
        ),
        (
            "after_identity_change",
            Path("corrected/inputs-after.json"),
            [str(V.ORIGINAL / V.PROOFS / V.SUBJECT)],
            "0" * 64,
            "unchanged corrected inputs",
        ),
        (
            "preliminary_checker_change",
            Path("preliminary/inputs-before.json"),
            [str(V.ORIGINAL / V.PROOFS / V.CHECKER)],
            "0" * 64,
            "preliminary identity delta",
        ),
        (
            "environment_change",
            Path("corrected/environment.json"),
            ["PATH"],
            "/tmp/untrusted",
            "exact recorded environment",
        ),
    ]


def calibrate(root):
    V.audit(root, require_manifest=False)
    V.need(
        V.source_match(root, root / "source") == 24, "snapshot source-match positive"
    )
    passed = set()
    with tempfile.TemporaryDirectory(prefix="fe2o3-enrollment-calibration-") as parent:
        clone = Path(parent) / "archive"
        shutil.copytree(root, clone)
        (clone / "SHA256SUMS").unlink(missing_ok=True)

        def trial(name, relative, mutation, expected, check=None):
            path = clone / relative
            original = path.read_bytes() if path.exists() else None
            try:
                mutation(path)
                reject(
                    check or (lambda: V.audit(clone, require_manifest=False)), expected
                )
            finally:
                if original is None:
                    path.unlink(missing_ok=True)
                else:
                    path.write_bytes(original)
            passed.add(name)

        for name, path, keys, value, error in fixture_changes():
            trial(name, path, lambda p, k=keys, v=value: json_field(p, k, v), error)
        diagnostic = Path("corrected/foreign_precedence/solver/stderr.log")
        trial(
            "compiler_error",
            diagnostic,
            lambda p: diagnostic_field(p, ["message"], "mismatched types"),
            "intended postcondition failure",
        )
        trial(
            "diagnostic_path",
            diagnostic,
            lambda p: diagnostic_field(p, ["spans", 0, "file_name"], "/tmp/other.rs"),
            "diagnostic source path",
        )
        trial(
            "diagnostic_byte_offset",
            diagnostic,
            lambda p: diagnostic_field(p, ["spans", 0, "byte_start"], -1),
            "diagnostic byte range",
        )
        trial(
            "restored_mutation",
            Path("corrected/foreign_precedence") / V.SUBJECT,
            lambda p: p.write_bytes(
                (clone / "source" / V.PROOFS / V.SUBJECT).read_bytes()
            ),
            "exact reversible case source",
        )
        trial(
            "include_change",
            Path("corrected/positive_before") / V.INCLUDES[0],
            lambda p: p.write_bytes(p.read_bytes() + b"\n"),
            "exact inherited source",
        )
        trial(
            "derived_body_change",
            Path("corrected/positive_before/audited-enrollment-body.rs"),
            lambda p: p.write_bytes(p.read_bytes() + b"\n"),
            "derived audit body",
        )
        trial(
            "preliminary_fabricated_completion",
            Path("preliminary/report.json"),
            lambda p: p.write_text("{}\n"),
            "exact preliminary receipt closure",
        )
        trial(
            "closure_output",
            Path("corrected/closure-after/stdout.log"),
            lambda p: p.write_text("PASS\n"),
            "exact closure result",
        )
        trial(
            "source_snapshot_change",
            Path("source") / V.PROOFS / V.SUBJECT,
            lambda p: p.write_bytes(p.read_bytes() + b"\n"),
            "snapshot:",
        )

        current = Path(parent) / "current"
        shutil.copytree(clone / "source", current)
        current_subject = current / V.PROOFS / V.SUBJECT
        current_subject.write_bytes(current_subject.read_bytes() + b"\n")
        reject(lambda: V.source_match(clone, current), "current source drift")
        passed.add("current_source_drift")

        manifest = clone / "SHA256SUMS"
        manifest.write_text(
            "".join(f"{V.sha(clone / n)}  {n}\n" for n in sorted(V.files(clone)))
        )
        V.manifest(clone)
        trial(
            "manifest_hash",
            Path("corrected/report.json"),
            lambda p: p.write_text("{}\n"),
            "manifest hash:",
            lambda: V.manifest(clone),
        )
        trial(
            "manifest_extra",
            Path("unlisted.txt"),
            lambda p: p.write_text("extra\n"),
            "exact manifest closure",
            lambda: V.manifest(clone),
        )
        trial(
            "manifest_missing",
            Path("corrected/report.json"),
            lambda p: p.unlink(),
            "exact manifest closure",
            lambda: V.manifest(clone),
        )
        trial(
            "manifest_duplicate",
            Path("SHA256SUMS"),
            lambda p: p.write_text(
                p.read_text() + p.read_text().splitlines()[0] + "\n"
            ),
            "unique normalized manifest path",
            lambda: V.manifest(clone),
        )
        trial(
            "manifest_symlink",
            Path("unlisted-link"),
            lambda p: p.symlink_to("corrected/report.json"),
            "no symlink:",
            lambda: V.manifest(clone),
        )
    V.need(passed == set(V.CALIBRATIONS), "exact calibration coverage")
    return {"passed": list(V.CALIBRATIONS), "count": len(passed)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--unsealed", action="store_true")
    args = parser.parse_args()
    if not args.unsealed:
        V.manifest(HERE)
    print(json.dumps(calibrate(HERE), indent=2))


if __name__ == "__main__":
    main()
