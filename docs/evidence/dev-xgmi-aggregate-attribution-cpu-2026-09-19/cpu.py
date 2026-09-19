#!/usr/bin/env python3
"""Record and replay scoped CPU qualification of aggregate XGMI attribution."""

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
PREFIX = "docs/evidence/dev-xgmi-aggregate-attribution-cpu-2026-09-19/"
NATIVE = "docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py"
SELECTOR = "docs/evidence/dev-sdma-direct-readback-cpu-2026-09-18/source.py"
HELPERS = {
    NATIVE: "df740a80c83c94af02f7e7dfb1203dcf7c1eafbde0010e41219e4aac40b34bb7",
    SELECTOR: "a7a8b8e73aa1a0817e13f9e575369403ece42f4eb702fd1ec04d9bc57fcff953",
}
STATIC = ("cpu.py", "test_cpu.py", "PROTOCOL.md")
ENV = """env CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 CARGO_TERM_COLOR=never
RUST_TEST_THREADS=2 CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_PROFILE_DEV_DEBUG=0
CARGO_PROFILE_DEV_DEBUG_ASSERTIONS=true CARGO_PROFILE_DEV_OVERFLOW_CHECKS=true
CARGO_PROFILE_TEST_OPT_LEVEL=1 CARGO_PROFILE_TEST_DEBUG=0
CARGO_PROFILE_TEST_DEBUG_ASSERTIONS=true CARGO_PROFILE_TEST_OVERFLOW_CHECKS=true""".split()
EXAMPLE = "gfx942-runtime-xgmi-peer-benchmark"
SOURCE_FILE = "crates/fe2o3-runtime/examples/" + EXAMPLE + ".rs"
BACKEND = "crates/fe2o3-runtime/src/kfd_backend"
FMT_FILES = (
    SOURCE_FILE, BACKEND + ".rs", BACKEND + "/xgmi_batch.rs",
    BACKEND + "/xgmi_batch/tests.rs", BACKEND + "/xgmi_diagnostic.rs",
    BACKEND + "/xgmi_batch_diagnostic.rs",
    BACKEND + "/xgmi_batch/tests/profile_equivalence.rs",
)
REQUIRED_SOURCE = set(FMT_FILES) | {
    "Cargo.toml", "Cargo.lock", "rust-toolchain.toml",
    "crates/fe2o3-runtime/Cargo.toml",
    BACKEND + "/xgmi_batch/tests/admission_scaling.rs",
    BACKEND + "/xgmi_batch/tests/dependency_scaling.rs",
    "crates/cargo-fe2o3/tests/unsafe_source_policy.rs",
    "scripts/unsafe-source-baseline.json",
}


def family(prefix, names):
    return {prefix + name for name in names.split()}


BATCH_TESTS = family("kfd_backend::xgmi_batch::tests::", """
ready_submits_once_and_retry_only_waits_with_the_original_deadline
every_typed_operation_outcome_closes_before_returning_custody
admission_requires_an_exact_set_but_preserves_native_fifo_order
admission_separates_caller_errors_from_corrupt_indexes
admission_leaves_dependency_blocked_successors_outside_the_batch
native_attempt_unwind_aborts_without_dropping_the_panic_payload
dependency_indexes_retain_and_wake_blocked_successors_exactly
""") | family("kfd_backend::xgmi_batch::tests::admission_scaling::", """
bounded_ready_permutations_match_the_frozen_reference
bounded_metadata_mutations_match_the_frozen_reference
malformed_requests_are_invalid_before_any_scratch_reservation
either_scratch_reservation_failure_precedes_later_classification
injected_success_reserves_both_indexes_and_matches_the_reference
late_index_corruption_precedes_subset_or_foreign_caller_errors
sixty_five_thousand_active_blocked_records_preserve_rosters
admission_scaling_profile_rows
""") | family("kfd_backend::xgmi_batch::tests::dependency_scaling::", """
one_active_scan_counts_each_source_record_once_for_relevant_duplicates
selected_and_unselected_duplicate_dependencies_preserve_reference_semantics
asymmetric_dependency_predicates_and_requested_order_match_the_reference
dependency_use_count_overflow_is_reported_without_wrapping
late_waiter_and_count_corruptions_match_the_reference
empty_selection_is_valid_without_scratch_reservation
scratch_capacity_precedes_predicate_mismatch_without_mutating_inputs
successful_scratch_reservation_is_single_and_nonmutating
maximum_healthy_requested_dependency_breadth_is_accepted
dependency_scaling_profile_rows
""") | family("kfd_backend::xgmi_batch::tests::profile_equivalence::", """
profiling_preserves_every_typed_outcome_close_error_and_custody_order
profiling_preserves_timeout_retry_without_resubmission_or_deadline_extension
profiling_preserves_submit_wait_and_close_panic_payload_identity
""") | family("kfd_backend::xgmi_batch_diagnostic::tests::", """
disabled_timer_runs_each_operation_once_without_recording
enabled_missing_or_duplicate_span_is_permanently_unavailable
enabled_timer_preserves_results_and_all_phase_slots
timer_preserves_error_and_original_panic_identity
duplicate_phase_is_permanently_unavailable
recorder_preallocates_exact_roster_and_preserves_identity_order
recorder_rejects_unsupported_or_noncanonical_calls
errors_wrong_identity_and_incomplete_calls_never_record_success
missing_overflow_and_impossible_totals_are_invalid
extraction_requires_complete_teardown_and_is_single_use
""")
DIAGNOSTIC_TESTS = family("kfd_backend::xgmi_diagnostic::tests::", """
exact_two_direction_roster_preserves_identity_without_reallocation
unsupported_aggregate_invalidates_even_a_previously_complete_capture
admission_and_unsupported_shape_are_bounded
errors_unwinds_and_wrong_identity_never_yield_a_complete_capture
invalid_duration_is_not_zero_or_silently_accepted
pending_exhaustion_and_foreign_poll_invalidate_without_growing_storage
extraction_requires_shutdown_quiescence_and_is_single_use
real_unwind_leaves_pending_and_cannot_be_rehabilitated
production_records_only_after_all_native_custody_branches_settle
""")
EXAMPLE_TESTS = family("tests::", """
aggregate_and_ordinary_depth_bounds_are_distinct
aggregate_classification_includes_all_aggregate_modes
aggregate_hot_only_requires_depth_one
canaries_bind_the_exact_inner_copy_region
diagnostic_submission_roster_is_bounded_before_native_open
aggregate_diagnostic_call_roster_is_bounded_before_native_open
existing_modes_preserve_both_phases_and_report_schemas
patterns_distinguish_round_slot_and_direction percentile_uses_nearest_rank
progress_flags_are_explicit_and_mutually_exclusive_before_native_open
""")
SAFETY_TESTS = set("""unsafe_source_matches_reviewed_inventory
comments_literals_and_documentation_do_not_count
all_constructs_and_macro_templates_are_counted malformed_tokens_fail_closed
additions_removals_and_moves_require_baseline_review""".split())
SAFETY_IGNORED = {
    "refresh_reviewed_unsafe_inventory":
        "explicit maintenance command; review the resulting baseline diff"
}
PYTHON_TESTS = set("""test_exact_commands_and_profile
test_list_rosters_reject_omission_substitution_and_duplicates
test_success_output_rejects_false_success_and_wrong_counts
test_receipts_reject_type_confusion_tampering_and_reordering
test_binding_and_source_reject_rebased_mutations
test_inventory_and_seal_reject_unaccounted_files
test_json_rejects_duplicates_and_nonfinite_values""".split())


def need(value, message):
    if not value:
        raise RuntimeError(message)


def sha(path):
    path = Path(path)
    need(path.is_file() and not path.is_symlink(), "ordinary file: " + str(path))
    return hashlib.sha256(path.read_bytes()).hexdigest()


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        need(key not in result, "duplicate JSON key: " + key)
        result[key] = value
    return result


def parse_json(text):
    def invalid(value):
        raise RuntimeError("nonfinite JSON value: " + value)
    return json.loads(text, object_pairs_hook=unique_object, parse_constant=invalid)


def read(path):
    sha(path)
    return parse_json(Path(path).read_text())


def same_json(left, right):
    return json.dumps(left, sort_keys=True, allow_nan=False) == json.dumps(
        right, sort_keys=True, allow_nan=False
    )


for name, digest in HELPERS.items():
    need(sha(ROOT / name) == digest, "unauthenticated helper: " + name)
spec = importlib.util.spec_from_file_location("aggregate_cpu_recorder", ROOT / NATIVE)
N = importlib.util.module_from_spec(spec)
spec.loader.exec_module(N)


def commands(root):
    root = Path(root)
    python = ["python3", "-I", "-B"]
    cargo = ENV + ["cargo", "test", "--frozen", "-p", "fe2o3-runtime"]
    result = [
        ("source-before", python + [str(root / SELECTOR)], 60),
        ("rustc", ["rustc", "-vV"], 30),
        ("cargo", ["cargo", "-V"], 30),
    ]
    for target in ("gnu", "musl"):
        target_args = [] if target == "gnu" else ["--target", "x86_64-unknown-linux-musl"]
        command = cargo + ["--all-features", *target_args, "--lib", "--"]
        result.append((target + "-runtime-roster", command + ["--list"], 1800))
        for label, test_filter in (("batch", "kfd_backend::xgmi_batch"),
                                   ("diagnostic", "kfd_backend::xgmi_diagnostic")):
            result.append((target + "-" + label, command + [test_filter], 1800))
    for target in ("gnu", "musl", "feature-off"):
        features = ["--no-default-features"] if target == "feature-off" else ["--all-features"]
        target_args = ["--target", "x86_64-unknown-linux-musl"] if target == "musl" else []
        command = cargo + features + target_args + ["--example", EXAMPLE]
        result.append((target + "-example-roster", command + ["--", "--list"], 1800))
        result.append((target + "-example", command, 1800))
    safety = ENV + ["cargo", "test", "--frozen", "-p", "cargo-fe2o3",
                    "--test", "unsafe_source_policy"]
    result.extend([
        ("safety-inventory-roster", safety + ["--", "--list"], 1800),
        ("safety-inventory", safety, 1800),
        ("verifier-tests", python + [str(root / PREFIX / "test_cpu.py"), "-v"], 120),
        ("fmt", ["rustfmt", "--edition", "2024", "--check", "--config",
                 "skip_children=true", *FMT_FILES], 180),
        ("clippy", ENV + ["cargo", "clippy", "--frozen", "-p", "fe2o3-runtime",
                           "--all-features", "--lib", "--example", EXAMPLE,
                           "--", "-D", "warnings"], 1800),
        ("source-after", python + [str(root / SELECTOR)], 60),
    ])
    return result


STAGES = tuple(name for name, _, _ in commands(ROOT))


def source_snapshot(data):
    need(type(data) is dict and set(data) == {"base", "files"}, "source snapshot keys")
    need(type(data["base"]) is str and re.fullmatch(r"[0-9a-f]{40}", data["base"]), "source base")
    files = data["files"]
    need(type(files) is dict and REQUIRED_SOURCE <= set(files), "required source roster")
    for name, digest in files.items():
        need(type(name) is str and not name.startswith("/")
             and all(part not in ("", ".", "..") for part in name.split("/")), "relative source path")
        need(type(digest) is str and re.fullmatch(r"[0-9a-f]{64}", digest), "source digest")
    return data


def tool_manifest(archive):
    for name, digest in HELPERS.items():
        need(sha(ROOT / name) == digest, "frozen helper identity")
    return dict(HELPERS, **{PREFIX + name: sha(Path(archive) / name) for name in STATIC})


def receipt(archive, stage, command, timeout, previous):
    folder = Path(archive) / "raw" / stage
    row = read(folder / "receipt.json")
    keys = """command cwd started_ns timeout_seconds pid exit error group_absent
environment stdin_sha256 finished_ns stdout_sha256 stderr_sha256""".split()
    need(type(row) is dict and set(row) == set(keys), "receipt keys: " + stage)
    need(type(row["command"]) is list and row["command"] == command, "exact command: " + stage)
    need(type(row["timeout_seconds"]) is int and row["timeout_seconds"] == timeout, "timeout")
    need(type(row["cwd"]) is str and Path(row["cwd"]).is_absolute(), "absolute receipt cwd")
    for key in ("started_ns", "finished_ns", "pid"):
        need(type(row[key]) is int and row[key] > 0, "positive integer: " + key)
    need(row["finished_ns"] >= row["started_ns"], "ordered receipt times")
    need(previous is None or row["started_ns"] >= previous, "ordered stages")
    need(type(row["exit"]) is int and row["exit"] == 0, "zero exit: " + stage)
    need(row["error"] is None and row["group_absent"] is True, "closed stage: " + stage)
    need(row["environment"] is None and row["stdin_sha256"] is None, "ambient/stdin: " + stage)
    for stream in ("stdout", "stderr"):
        need(row[stream + "_sha256"] == sha(folder / stream), "stream hash: " + stage)
    return row


def roster(output):
    lines = [line for line in output.splitlines() if line]
    need(lines, "nonempty test listing")
    summary = re.fullmatch(r"([0-9]+) tests, 0 benchmarks", lines[-1])
    need(summary is not None, "exact test-list summary")
    names = []
    for line in lines[:-1]:
        match = re.fullmatch(r"([a-zA-Z0-9_:]+): test", line)
        need(match is not None, "exact test-list row")
        names.append(match[1])
    need(len(names) == len(set(names)) == int(summary[1]) and str(len(names)) == summary[1],
         "unique exact test-list count")
    return set(names)


def passing_tests(output, expected, filtered=0, ignored=None):
    ignored = {} if ignored is None else ignored
    count = len(expected) + len(ignored)
    lines = [line for line in output.splitlines() if line]
    need(len(lines) == count + 2 and lines[0] == f"running {count} tests", "exact test output shape")
    found = {}
    for line in lines[1:-1]:
        match = re.fullmatch(r"test ([a-zA-Z0-9_:]+) \.\.\. (.+)", line)
        need(match is not None and match[1] not in found, "unique exact test outcome")
        found[match[1]] = match[2]
    wanted = dict.fromkeys(expected, "ok")
    wanted.update({name: "ignored, " + reason for name, reason in ignored.items()})
    need(found == wanted, "exact successful test roster")
    summary = (rf"test result: ok\. {len(expected)} passed; 0 failed; {len(ignored)} ignored; "
               rf"0 measured; {filtered} filtered out; finished in [0-9]+\.[0-9]+s")
    need(re.fullmatch(summary, lines[-1]) is not None, "exact successful test summary")


def python_tests(output):
    names = re.findall(r"^(test_[a-z0-9_]+) \([^\n]+\) \.\.\. ok$", output, re.M)
    need(len(names) == len(set(names)) and set(names) == PYTHON_TESTS, "exact Python test roster")
    need(len(re.findall(r"^test_", output, re.M)) == len(names), "no failed or skipped calibration")
    need(re.search(rf"\nRan {len(names)} tests in [0-9]+\.[0-9]+s\n\nOK\n\Z", output),
         "exact Python success summary")


def derive(archive=HERE, binding=None, live=False):
    archive = Path(archive)
    need(binding is None or type(binding) is dict, "binding object")
    execution_root = Path(binding["execution_root"]) if binding else ROOT
    need(execution_root.is_absolute() and ".." not in execution_root.parts, "execution root")
    previous, rows = None, {}
    for stage, command, timeout in commands(execution_root):
        row = receipt(archive, stage, command, timeout, previous)
        need(row["cwd"] == str(execution_root), "one execution root")
        previous, rows[stage] = row["finished_ns"], row
    before, after = archive / "raw/source-before/stdout", archive / "raw/source-after/stdout"
    need(before.read_bytes() == after.read_bytes(), "unchanged full source snapshot")
    source = source_snapshot(read(before))
    tools = tool_manifest(archive)
    need(same_json(read(archive / "tools.json"), tools), "unchanged qualification tools")
    def stdout(stage):
        return (archive / "raw" / stage / "stdout").read_text()
    rosters = {}
    for target in ("gnu", "musl"):
        names = roster(stdout(target + "-runtime-roster"))
        rosters[target + "-runtime"] = sorted(names)
        for label, test_filter, expected in (
            ("batch", "kfd_backend::xgmi_batch", BATCH_TESTS),
            ("diagnostic", "kfd_backend::xgmi_diagnostic", DIAGNOSTIC_TESTS),
        ):
            need({name for name in names if test_filter in name} == expected,
                 "exact selected runtime roster: " + label)
            passing_tests(stdout(target + "-" + label), expected, len(names) - len(expected))
    need(rosters["gnu-runtime"] == rosters["musl-runtime"], "cross-target runtime roster")
    for target in ("gnu", "musl", "feature-off"):
        names = roster(stdout(target + "-example-roster"))
        need(names == EXAMPLE_TESTS, "exact example roster")
        rosters[target + "-example"] = sorted(names)
        passing_tests(stdout(target + "-example"), names)
    names = roster(stdout("safety-inventory-roster"))
    need(names == SAFETY_TESTS | set(SAFETY_IGNORED), "exact unsafe inventory roster")
    rosters["safety-inventory"] = sorted(names)
    passing_tests(stdout("safety-inventory"), SAFETY_TESTS, ignored=SAFETY_IGNORED)
    python_tests((archive / "raw/verifier-tests/stderr").read_text())
    for stage, marker in (("rustc", "rustc "), ("cargo", "cargo ")):
        need(stdout(stage).startswith(marker), "toolchain identity: " + stage)
    result = {
        "schema": "fe2o3.xgmi-aggregate-attribution-cpu.v1",
        "commands": len(STAGES), "execution_root": str(execution_root),
        "source_base": source["base"], "source_files": source["files"],
        "source_snapshot_sha256": sha(before), "tools": tools, "rosters": rosters,
        "tests": {"batch_each_target": len(BATCH_TESTS),
                  "diagnostic_each_target": len(DIAGNOSTIC_TESTS),
                  "example_each_configuration": len(EXAMPLE_TESTS),
                  "safety_passed": len(SAFETY_TESTS), "safety_ignored": len(SAFETY_IGNORED),
                  "verifier": len(PYTHON_TESTS)},
        "toolchain": {"rustc_stdout_sha256": rows["rustc"]["stdout_sha256"],
                      "cargo_stdout_sha256": rows["cargo"]["stdout_sha256"]},
        "native_execution": False, "performance_acceptance": False, "formal_refinement": False,
    }
    if binding is not None:
        need(same_json(binding, result), "exact typed binding")
    if live:
        output = subprocess.check_output(["python3", "-I", "-B", str(ROOT / SELECTOR)], cwd=ROOT)
        current = source_snapshot(parse_json(output))
        need(current["files"] == source["files"], "live full source map")
        for name, digest in source["files"].items():
            need(sha(ROOT / name) == digest, "live ordinary source file: " + name)
    return result


def expected_paths():
    files = set(STATIC) | {"binding.json", "tools.json"}
    directories = {"raw"}
    for stage in STAGES:
        directories.add("raw/" + stage)
        files |= {f"raw/{stage}/{name}" for name in ("receipt.json", "stdout", "stderr")}
    return files, directories


def inventory(archive, sealed):
    files, directories = set(), set()
    for path in Path(archive).rglob("*"):
        need(not path.is_symlink(), "no archive symlinks")
        relative = path.relative_to(archive).as_posix()
        need(path.is_dir() or path.is_file(), "ordinary archive node")
        (directories if path.is_dir() else files).add(relative)
    expected_files, expected_directories = expected_paths()
    if sealed:
        expected_files.add("SHA256SUMS")
    need(files == expected_files and directories == expected_directories, "exact archive closure")


def seal(archive=HERE, create=False):
    archive = Path(archive)
    if create:
        inventory(archive, False)
        with (archive / "SHA256SUMS").open("x", encoding="utf-8") as output:
            output.writelines(f"{sha(archive / name)}  {name}\n" for name in sorted(expected_paths()[0]))
    inventory(archive, True)
    parsed = {}
    for line in (archive / "SHA256SUMS").read_text().splitlines():
        match = re.fullmatch(r"([0-9a-f]{64})  ([^\n]+)", line)
        need(match is not None and match[2] not in parsed, "unique seal syntax")
        parsed[match[2]] = match[1]
    need(set(parsed) == expected_paths()[0], "exact seal roster")
    for name, digest in parsed.items():
        need(sha(archive / name) == digest, "sealed digest: " + name)


def verify(archive=HERE, live=False, allow_unsealed=False):
    archive = Path(archive)
    report = derive(archive, read(archive / "binding.json"), live)
    if not allow_unsealed or (archive / "SHA256SUMS").exists():
        seal(archive)
    else:
        inventory(archive, False)
    return report


def prepare_binding(archive=HERE):
    report = derive(archive, live=True)
    N.write_json(Path(archive) / "binding.json", report)
    return verify(archive, live=True, allow_unsealed=True)


def record():
    need(set(path.name for path in HERE.iterdir()) == set(STATIC), "fresh packet only")
    N.write_json(HERE / "tools.json", tool_manifest(HERE))
    recorder = N.Recorder(HERE / "raw", ROOT)
    for number in N.MANAGED:
        signal.signal(number, N.interrupted)
    for name, command, timeout in commands(ROOT):
        recorder.run(name, command, timeout)
    prepare_binding(HERE)
    seal(HERE, create=True)
    return verify(HERE, live=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--record", action="store_true")
    mode.add_argument("--prepare-binding", action="store_true")
    mode.add_argument("--seal", action="store_true")
    mode.add_argument("--allow-unsealed", action="store_true")
    parser.add_argument("--live", action="store_true")
    args = parser.parse_args()
    need(not (args.live and (args.record or args.prepare_binding)), "record and preparation already require live equality")
    if args.record:
        report = record()
    elif args.prepare_binding:
        report = prepare_binding()
    else:
        report = verify(live=args.live, allow_unsealed=args.allow_unsealed or args.seal)
        if args.seal:
            seal(create=True)
    print(json.dumps({"schema": report["schema"], "commands": report["commands"],
                      "tests": report["tests"], "verified": True}, sort_keys=True))


if __name__ == "__main__":
    main()
