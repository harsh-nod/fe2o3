#!/usr/bin/env python3
"""Audit bounded CPU receipts, never execute archived commands."""

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shlex
import subprocess
import sys

sys.dont_write_bytecode = True
ARCHIVE = Path(__file__).resolve().parent
ROOT = ARCHIVE.parents[2]
SOURCE = ROOT / "docs/evidence/dev-sdma-direct-readback-cpu-2026-09-18/source.py"
PARSER = ROOT / "docs/evidence/dev-kfd-native-wait-cpu-2026-09-18/verify.py"
FILTERS = ["constructed_directional_release", "constructed_generic_release", "striped_sdma_cases", "constructed_combined_release", "queue::live::primary_release::tests", "sdma_cleanup"]
PREFIX = "queue::live::construction_primary::integration_tests::release_cases::"
GROUPS = {
    PREFIX + "combined_sdma_cases::": [
        "constructed_combined_release_orders_all_counts_and_refunds_both_sets",
        "constructed_combined_release_preflights_every_field_in_both_sets",
        "constructed_combined_release_rejects_wrong_profiles_and_cross_set_ids",
        "constructed_combined_release_retains_both_sets_at_every_callback_failure",
        "constructed_combined_release_retains_real_late_cleanup_at_set_boundary",
    ],
    PREFIX + "generic_sdma_cases::": [
        "constructed_generic_release_failures_keep_original_owner_and_completed_prefix",
        "constructed_generic_release_late_native_resource_failure_keeps_real_prefix",
        "constructed_generic_release_orders_original_owner_and_refunds_all_backing",
        "constructed_generic_release_rejects_malformed_or_pending_owner_without_effects",
        "constructed_generic_release_retains_mutated_destroy_argument_and_raw_outcome",
    ],
    PREFIX + "sdma_cases::": [
        "constructed_directional_release_failures_keep_prefix_and_do_not_complete_primary",
        "constructed_directional_release_late_native_resource_failure_keeps_real_prefix",
        "constructed_directional_release_orders_original_owners_and_refunds_all_backing",
        "constructed_directional_release_rejects_bad_second_owner_before_any_effect",
        "constructed_directional_release_retains_mutated_destroy_argument_and_raw_outcome",
    ],
    PREFIX + "striped_sdma_cases::": [
        "constructed_striped_release_checks_every_last_owner_field_before_effects",
        "constructed_striped_release_keeps_real_late_cleanup_prefix_for_every_owner",
        "constructed_striped_release_orders_all_balanced_counts_and_refunds_backing",
        "constructed_striped_release_rejects_malformed_rosters_without_effects",
        "constructed_striped_release_retains_every_callback_failure_prefix",
        "constructed_striped_release_retains_mutated_destroy_inputs",
        "retained_striped_release_custody_has_bounded_inline_size",
    ],
    "queue::live::primary_release::tests::": [
        "primary_release_explicit_deferred_profile_is_not_admitted",
        "primary_release_invalid_secondary_is_error_even_when_busy",
        "primary_release_missing_owner_is_error_not_legacy_profile",
        "primary_release_unfinished_drop_aborts_and_failed_preflight_keeps_parent",
    ],
    "sdma::tests::": ["sdma_cleanup_preserves_original_certificate_box_through_native_disposal"],
    "shared_memory::tests::pristine_abort::cleanup_tests::sdma::": [
        "sdma_cleanup_all_currentness_boundaries_retain_unsettled_disposal",
        "sdma_cleanup_all_native_failures_retain_exact_prefix_and_panic",
        "sdma_cleanup_exact_two_phase_order_and_account_refund",
        "sdma_cleanup_projection_and_commit_boundaries_keep_native_receipts_separate",
        "sdma_cleanup_requires_six_revision_headroom_before_native_effects",
    ],
}
EXPECTED = {prefix + name for prefix, names in GROUPS.items() for name in names}
RUNTIME = {"kfd_backend::retained_release_tests::" + name for name in [
    "runtime_auxiliary_teardown_errors_and_panics_seal_reentry_without_destroyed_events",
    "runtime_multi_device_shutdown_retries_after_a_child_rejection",
    "runtime_pool_trim_error_and_panic_terminalize_before_returning",
    "runtime_retired_shutdown_is_inert_without_poison_or_native_work",
]}


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def parse_harness(text, expected=EXPECTED, filtered=1398):
    need(sha(PARSER) == "13b8c2fc57cb013cd19df6621efb0cd22a79b8ce84c9539ba4258b95a8c22ddb", "pinned harness parser")
    spec = importlib.util.spec_from_file_location("harness_parser", PARSER)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    rows = module.parse(text, len(expected), 0, filtered)
    need(rows == dict.fromkeys(expected, "ok"), "exact named test outcomes")
    return rows


def parse_roster(text):
    lines = text.splitlines()
    while lines and not lines[0].endswith(": test"):
        line = lines.pop(0)
        need(not line.strip() or re.fullmatch(
            r"   Compiling .+|    Finished `test` profile .+|     Running unittests .+", line
        ), "roster prelude")
    expected = [name + ": test" for name in sorted(EXPECTED)] + ["", "32 tests, 0 benchmarks"]
    need(lines == expected, "exact selected test roster")


def commands():
    env = ["env", "CARGO_INCREMENTAL=0", "CARGO_PROFILE_DEV_DEBUG=0", "CARGO_PROFILE_TEST_DEBUG=0", "CARGO_BUILD_JOBS=2", "CARGO_TERM_COLOR=never", "RUST_TEST_THREADS=1"]
    base = ["--frozen", "-p", "fe2o3-kfd", "--all-features"]
    target = ["--target", "x86_64-unknown-linux-musl"]
    source = ["python3", "-I", SOURCE.relative_to(ROOT).as_posix()]
    return {
        "source-before": source,
        "rustc": ["rustc", "-vV"],
        "cargo": ["cargo", "-V"],
        "clippy": env + ["cargo", "clippy"] + base + ["--all-targets", "--", "-D", "warnings"],
        "gnu-roster": env + ["cargo", "test"] + base + ["--lib", "--", "--list"] + FILTERS,
        "gnu-release": env + ["cargo", "test"] + base + ["--lib", "--"] + FILTERS,
        "musl-roster": env + ["cargo", "test"] + base + target + ["--lib", "--", "--list"] + FILTERS,
        "musl-release": env + ["cargo", "test"] + base + target + ["--lib", "--"] + FILTERS,
        "runtime": env + ["cargo", "test", "--frozen", "-p", "fe2o3-runtime", "--all-features", "--lib", "kfd_backend::retained_release_tests::runtime_"],
        "fmt": ["cargo", "fmt", "--all", "--check"],
        "diff": ["git", "diff", "--check"],
        "source-after": source,
        "source-unchanged": None,
        "calibration": ["python3", "-B", (ARCHIVE / "test_verify.py").relative_to(ROOT).as_posix()],
    }


def receipt(name, command, directory="raw", status=0):
    prefix = ARCHIVE / directory / name
    need(prefix.with_suffix(".exit").read_text() == f"{status}\n", "receipt status: " + name)
    actual = shlex.split(prefix.with_suffix(".command").read_text())
    if command is not None:
        need(actual == command, "exact command: " + name)
    else:
        need(len(actual) == 3 and actual[0] == "cmp", "source comparison command")
        before, after = map(Path, actual[1:])
        need(before.is_absolute() and before.parent == after.parent and
             before.parts[-3:] == (ARCHIVE.name, "raw", "source-before.log") and
             after.name == "source-after.log", "historical source comparison operands")
    times = [prefix.with_suffix("." + suffix).read_text().strip() for suffix in ("started", "finished")]
    need(all(re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{9}Z", value) for value in times)
         and times[0] <= times[1], "ordered receipt: " + name)
    return times


INTERRUPTED_COMPLETE = ["source-before", "rustc", "cargo", "clippy", "gnu-roster", "gnu-release", "musl-roster", "recovery-source"]
SETUP_REJECTED = ["source-before", "rustc", "cargo", "clippy"]


def interrupted():
    directory = ARCHIVE / "interrupted"
    specs = commands()
    last = None
    for name in INTERRUPTED_COMPLETE[:-1]:
        start, end = receipt(name, specs[name], "interrupted")
        need(last is None or last <= start, "interrupted attempt chronology")
        last = end
    parse_roster((directory / "gnu-roster.log").read_text())
    parse_roster((directory / "musl-roster.log").read_text())
    parse_harness((directory / "gnu-release.log").read_text())
    need(shlex.split((directory / "musl-release.command").read_text()) == specs["musl-release"], "interrupted musl command")
    start = (directory / "musl-release.started").read_text().strip()
    need(re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{9}Z", start) and last <= start, "interrupted start")
    need(not (directory / "musl-release.exit").exists() and not (directory / "musl-release.finished").exists(), "interrupted outcome is absent, not a pass")
    need(sha(directory / "musl-release.log") == "958f9111d8b33a80634c1a5082a0ea9cd3699a0fdb49e1ff43fdad7976bf7542", "exact recovered incomplete transcript")
    try:
        parse_harness((directory / "musl-release.log").read_text())
    except (ValueError, AssertionError, IndexError):
        pass
    else:
        raise ValueError("interrupted transcript must not qualify")
    recovery_start, recovery_end = receipt("recovery-source", specs["source-before"], "interrupted")
    need(start <= recovery_start, "recovery follows interrupted start")
    source = (directory / "source-before.log").read_bytes()
    need(source == (directory / "recovery-source.log").read_bytes() == (ARCHIVE / "raw/source-before.log").read_bytes(), "unchanged source across interruption and rerun")
    return recovery_end


def setup_rejected(previous):
    directory = ARCHIVE / "setup-rejected"
    specs = commands()
    for name in SETUP_REJECTED:
        start, end = receipt(name, specs[name], "setup-rejected", 101 if name == "clippy" else 0)
        need(previous <= start, "setup rejection chronology")
        previous = end
    need((directory / "clippy.log").read_text() ==
         "error: failed to create directory `/home/harsh/.codex-tmp/fe2o3-c4-completion-20260917/target`\n\nCaused by:\n  Not a directory (os error 20)\n", "exact pre-compilation setup rejection")
    need((directory / "source-before.log").read_bytes() == (ARCHIVE / "raw/source-before.log").read_bytes(), "unchanged setup-rejection source")
    need(previous <= (ARCHIVE / "raw/source-before.started").read_text().strip(), "qualified rerun follows setup rejection")


def qualify(live=False):
    need(sha(SOURCE) == "a7a8b8e73aa1a0817e13f9e575369403ece42f4eb702fd1ec04d9bc57fcff953", "pinned source selector")
    setup_rejected(interrupted())
    last = None
    for name, command in commands().items():
        start, end = receipt(name, command)
        need(last is None or last <= start, "qualification chronology")
        last = end
    for target in ("gnu", "musl"):
        parse_roster((ARCHIVE / f"raw/{target}-roster.log").read_text())
        parse_harness((ARCHIVE / f"raw/{target}-release.log").read_text())
    parse_harness((ARCHIVE / "raw/runtime.log").read_text(), RUNTIME, 1121)
    for name in ("fmt", "diff", "source-unchanged"):
        need((ARCHIVE / f"raw/{name}.log").read_bytes() == b"", "empty successful check")
    before = (ARCHIVE / "raw/source-before.log").read_bytes()
    need(before == (ARCHIVE / "raw/source-after.log").read_bytes(), "unchanged complete source")
    source = json.loads(before)
    need(source["base"] == "fca96f832d890f2caa0ddd239c6535596a5fcdc2" and len(source["files"]) == 5557, "source base and count")
    need(re.fullmatch(r"\.{4}\n-+\nRan 4 tests in [0-9.]+s\n\nOK\n", (ARCHIVE / "raw/calibration.log").read_text()), "complete calibration harness")
    if live:
        current = json.loads(subprocess.check_output(["python3", "-I", str(SOURCE)], text=True))
        need(current["files"] == source["files"], "live source identity")
    return {"kfd_tests_per_target": 32, "runtime_smoke_tests": 4,
            "source_files": 5557, "native_execution": False, "formal_refinement": False}


def manifest():
    paths = {}
    for path in sorted(ARCHIVE.rglob("*")):
        need(not path.is_symlink(), "ordinary archive paths")
        if path.is_file() and path != ARCHIVE / "SHA256SUMS":
            paths[path.relative_to(ARCHIVE).as_posix()] = sha(path)
    expected = {".gitattributes", "README.md", "record.sh", "qualify.sh", "verify.py", "test_verify.py"}
    expected.update(f"raw/{name}.{suffix}" for name in commands()
                    for suffix in ("command", "exit", "started", "finished", "log"))
    expected.update(f"interrupted/{name}.{suffix}" for name in INTERRUPTED_COMPLETE
                    for suffix in ("command", "exit", "started", "finished", "log"))
    expected.update(f"interrupted/musl-release.{suffix}" for suffix in ("command", "started", "log"))
    expected.update(f"setup-rejected/{name}.{suffix}" for name in SETUP_REJECTED
                    for suffix in ("command", "exit", "started", "finished", "log"))
    need(set(paths) == expected, "exact archive membership")
    return "".join(f"{digest}  {name}\n" for name, digest in paths.items())


def main():
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--seal", action="store_true")
    mode.add_argument("--allow-unsealed", action="store_true")
    parser.add_argument("--live", action="store_true")
    args = parser.parse_args()
    result = qualify(args.live)
    expected = manifest()
    seal = ARCHIVE / "SHA256SUMS"
    if args.seal:
        with seal.open("x") as stream:
            stream.write(expected)
    elif not args.allow_unsealed:
        need(seal.read_text() == expected, "exact sealed archive")
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
