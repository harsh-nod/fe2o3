#!/usr/bin/env python3
"""Record/replay scoped CPU qualification for runtime currentness capture."""

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import signal
import subprocess
import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("run with python3 -I -B")

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
PREFIX = "docs/evidence/dev-xgmi-currentness-capture-cpu-2026-09-20/"
HELPER = "docs/evidence/dev-xgmi-aggregate-attribution-cpu-2026-09-19/cpu.py"
HELPER_SHA = "a0b39f6821352fa3f62c9fc72484519cd7a27b4c65d54e1f77f13c44a6b23686"
path = ROOT / HELPER
if path.is_symlink() or hashlib.sha256(path.read_bytes()).hexdigest() != HELPER_SHA:
    raise RuntimeError("unauthenticated CPU receipt helper")
spec = importlib.util.spec_from_file_location("capture_cpu_helpers", path)
C = importlib.util.module_from_spec(spec)
spec.loader.exec_module(C)
need, sha, read = C.need, C.sha, C.read

STATIC = ("cpu.py", "test_cpu.py", "PROTOCOL.md")
DOC = "docs/runtime-xgmi-currentness-capture-v1.md"
FMT = (
    C.SOURCE_FILE, C.BACKEND + ".rs", C.BACKEND + "/xgmi_batch.rs",
    C.BACKEND + "/xgmi_batch_diagnostic.rs",
    C.BACKEND + "/xgmi_batch_diagnostic/currentness_tests.rs",
    C.BACKEND + "/xgmi_batch/tests/profile_equivalence.rs",
)
FILTERS = ("kfd_backend::xgmi_batch", "kfd_backend::xgmi_diagnostic")
NEW_RUNTIME = C.family("kfd_backend::xgmi_batch_diagnostic::currentness_tests::", """
both_modes_preallocate_once_preserve_identity_and_enforce_capacity
both_modes_reject_noncanonical_admission_and_preserve_pending_custody
malformed_finish_invalidates_without_append_or_state_advancement
each_missing_nested_field_overflow_and_containment_failure_invalidates_capture
nested_and_outer_containment_allow_exact_equality_without_double_counting
extraction_mode_mismatch_preserves_complete_incomplete_and_invalid_captures
extraction_precedence_requires_teardown_in_both_modes_even_without_a_recorder
""") | C.family("kfd_backend::xgmi_batch::tests::profile_equivalence::", """
currentness_adapter_preserves_all_outcomes_close_errors_and_custody
currentness_adapter_preserves_timeout_retry_and_absolute_deadline
currentness_adapter_preserves_panic_identity_without_publishing_detail
unavailable_detail_does_not_change_success_or_release_custody
""")
FEATURE_RECORDERS = C.family("kfd_backend::xgmi_batch_diagnostic::tests::", """
recorder_preallocates_exact_roster_and_preserves_identity_order
recorder_rejects_unsupported_or_noncanonical_calls
errors_wrong_identity_and_incomplete_calls_never_record_success
missing_overflow_and_impossible_totals_are_invalid
extraction_requires_complete_teardown_and_is_single_use
""")
NEW_EXAMPLE = C.family("tests::", """
currentness_flag_is_feature_gated_and_exclusive_before_native_open
currentness_mode_preserves_hot_only_summary_with_distinct_diagnostic_label
""")
FORMAT_TEST = "tests::currentness_rows_preserve_every_identity_and_distinct_nested_interval"
CONFIGURATIONS = ("gnu", "musl", "feature-off")
PYTHON_TESTS = {"test_commands", "test_rosters", "test_test_output", "test_inventory", "test_typed_binding"}


def expected_runtime(configuration):
    if configuration == "feature-off":
        return C.BATCH_TESTS - FEATURE_RECORDERS
    return C.BATCH_TESTS | C.DIAGNOSTIC_TESTS | NEW_RUNTIME


def expected_example(configuration):
    return C.EXAMPLE_TESTS | NEW_EXAMPLE | (set() if configuration == "feature-off" else {FORMAT_TEST})


def commands(root, target):
    root = Path(root)
    need(re.fullmatch(r"/dev/shm/fe2o3-link-parser-build-20260920\.[A-Za-z0-9]{8}/target", target),
         "owned build target shape")
    env = C.ENV + ["CARGO_TARGET_DIR=" + target]
    python = ["python3", "-I", "-B"]
    result = [("source-before", python + [str(root / C.SELECTOR)], 60),
              ("rustc", ["rustc", "-vV"], 30), ("cargo", ["cargo", "-V"], 30)]
    for configuration in CONFIGURATIONS:
        features = ["--no-default-features"] if configuration == "feature-off" else ["--all-features"]
        args = features + ["--target", "x86_64-unknown-linux-musl" if configuration == "musl"
                           else "x86_64-unknown-linux-gnu"]
        command = env + ["cargo", "test", "--frozen", "-p", "fe2o3-runtime", *args]
        result += [
            (configuration + "-runtime-roster", command + ["--lib", "--", "--list"], 1800),
            (configuration + "-runtime", command + ["--lib", "--", *FILTERS], 1800),
            (configuration + "-example-roster", command + ["--example", C.EXAMPLE, "--", "--list"], 1800),
            (configuration + "-example", command + ["--example", C.EXAMPLE], 1800),
        ]
    safety = env + ["cargo", "test", "--frozen", "-p", "cargo-fe2o3", "--test", "unsafe_source_policy"]
    result += [
        ("safety-roster", safety + ["--", "--list"], 1800),
        ("safety-tests", safety, 1800),
        ("verifier-tests", python + [str(root / PREFIX / "test_cpu.py"), "-v"], 120),
        ("fmt", ["rustfmt", "--edition", "2024", "--check", "--config", "skip_children=true", *FMT], 180),
    ]
    for configuration, features in (("gnu", "--all-features"), ("feature-off", "--no-default-features")):
        result.append((configuration + "-clippy", env + ["cargo", "clippy", "--frozen",
                       "-p", "fe2o3-runtime", features, "--lib", "--tests", "--example", C.EXAMPLE,
                       "--", "-D", "warnings"], 1800))
    result.append(("source-after", python + [str(root / C.SELECTOR)], 60))
    return result


def tools():
    paths = dict(C.HELPERS, **{HELPER: HELPER_SHA})
    for name, digest in paths.items():
        need(sha(ROOT / name) == digest, "unchanged authenticated helper: " + name)
    paths.update({PREFIX + name: sha(HERE / name) for name in STATIC})
    paths[DOC] = sha(ROOT / DOC)
    return paths


def selected_tests(full, configuration):
    selected = {name for name in full if any(part in name for part in FILTERS)}
    need(selected == expected_runtime(configuration), "exact selected runtime test roster")
    return selected


def derive(archive, execution_root, target, live=False):
    archive, execution_root = Path(archive), Path(execution_root)
    need(execution_root.is_absolute() and ".." not in execution_root.parts, "execution root")
    previous = None
    for stage, command, seconds in commands(execution_root, target):
        row = C.receipt(archive, stage, command, seconds, previous)
        need(row["cwd"] == str(execution_root), "one execution root")
        previous = row["finished_ns"]
        need(not re.search(r"^warning:", (archive / "raw" / stage / "stderr").read_text(), re.M),
             "no compiler warnings: " + stage)
    before = archive / "raw/source-before/stdout"
    need(before.read_bytes() == (archive / "raw/source-after/stdout").read_bytes(), "unchanged source")
    source = C.source_snapshot(read(before))
    need(set(FMT) <= set(source["files"]), "all changed runtime source included")
    need(C.same_json(read(archive / "tools.json"), tools()), "unchanged tools")

    def output(stage, stream="stdout"):
        return (archive / "raw" / stage / stream).read_text()

    rosters = {}
    for configuration in CONFIGURATIONS:
        full = C.roster(output(configuration + "-runtime-roster"))
        selected = selected_tests(full, configuration)
        C.passing_tests(output(configuration + "-runtime"), selected, len(full) - len(selected))
        examples = C.roster(output(configuration + "-example-roster"))
        need(examples == expected_example(configuration), "exact example roster")
        C.passing_tests(output(configuration + "-example"), examples)
        rosters[configuration] = {"runtime": sorted(selected), "example": sorted(examples)}
    need(C.roster(output("safety-roster")) == C.SAFETY_TESTS | set(C.SAFETY_IGNORED), "unsafe roster")
    C.passing_tests(output("safety-tests"), C.SAFETY_TESTS, ignored=C.SAFETY_IGNORED)
    python_output = output("verifier-tests", "stderr")
    names = re.findall(r"^(test_[a-z_]+) \([^\n]+\) \.\.\. ok$", python_output, re.M)
    need(len(names) == len(set(names)) and set(names) == PYTHON_TESTS, "verifier test roster")
    need(len(re.findall(r"^test_", python_output, re.M)) == len(names), "no other verifier outcomes")
    need(re.search(r"\nRan 5 tests in [0-9]+\.[0-9]+s\n\nOK\n\Z", python_output), "verifier success")
    need(output("rustc").startswith("rustc ") and output("cargo").startswith("cargo "), "toolchain identity")
    if live:
        current = C.source_snapshot(C.parse_json(subprocess.check_output(
            ["python3", "-I", "-B", str(ROOT / C.SELECTOR)], cwd=ROOT)))
        need(current["files"] == source["files"], "live source map")
        for name, digest in source["files"].items():
            need(sha(ROOT / name) == digest, "ordinary live source: " + name)
    return {"schema": "fe2o3.xgmi-currentness-capture-cpu.v1", "execution_root": str(execution_root),
            "target": target, "source": source, "source_sha256": sha(before), "tools": tools(),
            "rosters": rosters, "commands": len(commands(execution_root, target)),
            "native_execution": False, "performance_acceptance": False, "formal_refinement": False}


def expected_paths(target):
    stages = [name for name, _, _ in commands(ROOT, target)]
    return (set(STATIC) | {"binding.json", "tools.json"} |
            {f"raw/{stage}/{item}" for stage in stages for item in ("receipt.json", "stdout", "stderr")},
            {"raw"} | {"raw/" + stage for stage in stages})


def inventory(archive, target, sealed):
    files, directories = set(), set()
    for path in archive.rglob("*"):
        need(not path.is_symlink() and (path.is_file() or path.is_dir()), "ordinary archive node")
        (directories if path.is_dir() else files).add(path.relative_to(archive).as_posix())
    expected_files, expected_directories = expected_paths(target)
    if sealed:
        expected_files.add("SHA256SUMS")
    need(files == expected_files and directories == expected_directories, "exact archive closure")


def seal(archive, target, create=False):
    inventory(archive, target, not create)
    canonical = "".join(f"{sha(archive / name)}  {name}\n" for name in sorted(expected_paths(target)[0]))
    path = archive / "SHA256SUMS"
    if create:
        with path.open("x") as output:
            output.write(canonical)
    need(path.read_text() == canonical, "exact seal")


def verify(live=False):
    binding = read(HERE / "binding.json")
    report = derive(HERE, binding["execution_root"], binding["target"], live)
    need(C.same_json(report, binding), "exact typed binding")
    seal(HERE, binding["target"])
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--record-target")
    parser.add_argument("--live", action="store_true")
    args = parser.parse_args()
    if args.record_target:
        need(set(path.name for path in HERE.iterdir()) == set(STATIC), "fresh packet only")
        C.N.write_json(HERE / "tools.json", tools())
        recorder = C.N.Recorder(HERE / "raw", ROOT)
        for number in C.N.MANAGED:
            signal.signal(number, C.N.interrupted)
        for stage, command, seconds in commands(ROOT, args.record_target):
            recorder.run(stage, command, seconds)
        C.N.write_json(HERE / "binding.json", derive(HERE, ROOT, args.record_target, live=True))
        seal(HERE, args.record_target, create=True)
    report = verify(live=args.live or bool(args.record_target))
    print(json.dumps({"verified": True, "commands": report["commands"],
                      "tests": {key: {suite: len(names) for suite, names in value.items()}
                                for key, value in report["rosters"].items()},
                      "native_execution": False, "performance_acceptance": False, "formal_refinement": False}))


if __name__ == "__main__":
    main()
