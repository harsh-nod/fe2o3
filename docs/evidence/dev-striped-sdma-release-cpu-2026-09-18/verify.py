#!/usr/bin/env python3
"""Verify complete CPU receipts and the one-line test-only qualification bridge."""

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shlex
import subprocess
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
PARSER = HERE.parent / "dev-kfd-native-wait-cpu-2026-09-18/verify.py"
INVENTORY = "docs/evidence/dev-sdma-direct-readback-cpu-2026-09-18/source.py"
ENV = [
    "env", "CARGO_INCREMENTAL=0", "CARGO_PROFILE_DEV_DEBUG=0",
    "CARGO_PROFILE_TEST_DEBUG=0", "CARGO_BUILD_JOBS=2",
    "CARGO_TERM_COLOR=never", "RUST_TEST_THREADS=1",
]
BASE = "8379c253d990e1e9d9a85ae35c7ad542bf0634ac"
PREFIX = "queue::live::construction_primary::integration_tests::release_cases::"
FIXTURE = "crates/fe2o3-kfd/src/sdma/retained_release/generic_fixture.rs"
sys.dont_write_bytecode = True
STRIPED = {
    "constructed_striped_release_checks_every_last_owner_field_before_effects",
    "constructed_striped_release_keeps_real_late_cleanup_prefix_for_every_owner",
    "constructed_striped_release_orders_all_balanced_counts_and_refunds_backing",
    "constructed_striped_release_rejects_malformed_rosters_without_effects",
    "constructed_striped_release_retains_every_callback_failure_prefix",
    "constructed_striped_release_retains_mutated_destroy_inputs",
    "retained_striped_release_custody_has_bounded_inline_size",
}
GENERIC = {
    "constructed_generic_release_failures_keep_original_owner_and_completed_prefix",
    "constructed_generic_release_late_native_resource_failure_keeps_real_prefix",
    "constructed_generic_release_orders_original_owner_and_refunds_all_backing",
    "constructed_generic_release_rejects_malformed_or_pending_owner_without_effects",
    "constructed_generic_release_retains_mutated_destroy_argument_and_raw_outcome",
}
CLEANUP = {
    "shared_memory::tests::pristine_abort::cleanup_tests::sdma::" + name
    for name in (
        "sdma_cleanup_all_currentness_boundaries_retain_unsettled_disposal",
        "sdma_cleanup_all_native_failures_retain_exact_prefix_and_panic",
        "sdma_cleanup_exact_two_phase_order_and_account_refund",
        "sdma_cleanup_projection_and_commit_boundaries_keep_native_receipts_separate",
        "sdma_cleanup_requires_six_revision_headroom_before_native_effects",
    )
} | {"sdma::tests::sdma_cleanup_preserves_original_certificate_box_through_native_disposal"}


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def commands(final):
    test = ENV + ["cargo", "test", "--frozen", "-p", "fe2o3-kfd", "--all-features"]
    release = (["--lib", "--", "constructed_generic_release", "striped_sdma_cases"]
               if final else ["--lib", "integration_tests::release_cases"])
    roster = (["--lib", "--", "--list", "constructed_generic_release", "striped_sdma_cases"]
              if final else release + ["--", "--list"])
    cleanup = ["--lib", "sdma_cleanup"]
    musl = ["--target", "x86_64-unknown-linux-musl"]
    inventory = ["python3", "-I", INVENTORY]
    clippy = ENV + ["cargo", "clippy", "--frozen", "-p", "fe2o3-kfd",
                    "--all-features", "--all-targets", "--", "-D", "warnings"]
    result = {
        "source-before": inventory,
        "rustc": ["rustc", "-vV"],
        "cargo": ["cargo", "-V"],
    }
    if final:
        result["clippy"] = clippy
    result.update({
        "gnu-roster": test + roster,
        "gnu-release": test + release,
        "gnu-cleanup": test + cleanup,
        "musl-roster": test + musl + roster,
        "musl-release": test + musl + release,
        "musl-cleanup": test + musl + cleanup,
    })
    if not final:
        result.update({"clippy": clippy, "source-after-prelint": inventory})
        return result
    result.update({
        "fmt": ["cargo", "fmt", "--all", "--check"],
        "diff": ["git", "diff", "--check"],
        "source-after": inventory,
        "source-unchanged": ["cmp", f"docs/evidence/{HERE.name}/raw/source-before.log",
                             f"docs/evidence/{HERE.name}/raw/source-after.log"],
        "runtime-smoke": ENV + ["cargo", "test", "--frozen", "-p", "fe2o3-runtime",
                                "--all-features", "--lib",
                                "kfd_backend::retained_release_tests::runtime_"],
        "staged-whitespace": ["git", "diff", "--cached", "--check", "--", ".",
                              f":(exclude)docs/evidence/{HERE.name}/raw/*.command",
                              f":(exclude)docs/evidence/{HERE.name}/prelint/*.command"],
    })
    result["staged-whitespace-final"] = result["staged-whitespace"]
    return result


def receipts(final, finish=""):
    directory = HERE / ("raw" if final else "prelint")
    logs, expected_receipts = {}, set()
    for name, command in commands(final).items():
        def read(suffix):
            path = directory / f"{name}.{suffix}"
            expected_receipts.add(path.name)
            assert path.is_file() and not path.is_symlink(), path
            return path.read_text()
        recorded = shlex.split(read("command"))
        if name == "source-unchanged":
            assert len(recorded) == 3 and recorded[0] == "cmp"
            operands = [Path(value) for value in recorded[1:]]
            assert all(path.is_absolute() for path in operands)
            assert operands[0].parent == operands[1].parent
            for path, expected in zip(operands, command[1:], strict=True):
                suffix = Path(expected).parts
                assert path.parts[-len(suffix):] == suffix
            recorded = command
        assert recorded == command, (directory.name, name)
        code = 101 if not final and name == "clippy" else 0
        if final and name == "staged-whitespace":
            code = 2
        assert read("exit") == f"{code}\n", (directory.name, name)
        start, end = read("started").strip(), read("finished").strip()
        assert all(re.fullmatch(r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d{9}Z", t)
                   for t in (start, end))
        assert finish <= start <= end, (directory.name, name)
        finish = end
        logs[name] = read("log")
    assert {p.name for p in directory.iterdir()} == expected_receipts
    return logs, finish


def harnesses(strict, logs, final):
    passed, filtered = (12, 1412) if final else (113, 1311)
    results = {}
    for target in ("gnu", "musl"):
        rows = strict.parse(logs[f"{target}-release"], passed, 0, filtered)
        assert all(name.startswith(PREFIX) for name in rows)
        assert {name.removeprefix(PREFIX + "striped_sdma_cases::") for name in rows
                if name.startswith(PREFIX + "striped_sdma_cases::")} == STRIPED
        if final:
            expected = {PREFIX + "striped_sdma_cases::" + name for name in STRIPED}
            expected |= {PREFIX + "generic_sdma_cases::" + name for name in GENERIC}
            assert rows.keys() == expected
        roster = re.findall(r"^(\S+): test$", logs[f"{target}-roster"], re.MULTILINE)
        assert len(roster) == len(set(roster)) == passed
        assert set(roster) == rows.keys()
        assert logs[f"{target}-roster"].endswith(f"{passed} tests, 0 benchmarks\n")
        results[target] = rows
        results[target + "-cleanup"] = strict.parse(logs[f"{target}-cleanup"], 6, 0, 1418)
        assert results[target + "-cleanup"].keys() == CLEANUP
    assert results["gnu"] == results["musl"]
    assert results["gnu-cleanup"] == results["musl-cleanup"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seal", action="store_true")
    parser.add_argument("--live", action="store_true")
    args = parser.parse_args()
    assert sha(PARSER) == "13b8c2fc57cb013cd19df6621efb0cd22a79b8ce84c9539ba4258b95a8c22ddb"
    assert sha(ROOT / INVENTORY) == "a7a8b8e73aa1a0817e13f9e575369403ece42f4eb702fd1ec04d9bc57fcff953"
    spec = importlib.util.spec_from_file_location("strict_cpu_harness", PARSER)
    strict = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(strict)
    prelint, finish = receipts(False)
    logs, _ = receipts(True, finish)
    assert "error: replacing an " in prelint["clippy"]
    assert "20 => scoped!(engine_index, None)" in prelint["clippy"]
    assert sha(HERE / "prelint/clippy.log") == "32b364e96ec37307e0474daee656cbbccce245435053c4577304d3b98f68aeea"
    endings = {
        "prelint/gnu-cleanup": 13, "prelint/gnu-release": 120,
        "prelint/musl-cleanup": 13, "prelint/musl-release": 120,
        "raw/gnu-cleanup": 13, "raw/gnu-release": 19,
        "raw/musl-cleanup": 13, "raw/musl-release": 19, "raw/runtime-smoke": 13,
    }
    assert logs["staged-whitespace"] == "".join(
        f"docs/evidence/{HERE.name}/{name}.log:{line}: new blank line at EOF.\n"
        for name, line in endings.items())
    assert (HERE / ".gitattributes").read_text() == (
        "raw/*.log whitespace=-blank-at-eof\nprelint/*.log whitespace=-blank-at-eof\n")
    harnesses(strict, prelint, False)
    harnesses(strict, logs, True)
    runtime = strict.parse(logs["runtime-smoke"], 4, 0, 1121)
    assert runtime == {
        "kfd_backend::retained_release_tests::" + name: "ok"
        for name in (
            "runtime_retired_shutdown_is_inert_without_poison_or_native_work",
            "runtime_multi_device_shutdown_retries_after_a_child_rejection",
            "runtime_auxiliary_teardown_errors_and_panics_seal_reentry_without_destroyed_events",
            "runtime_pool_trim_error_and_panic_terminalize_before_returning",
        )
    }
    mutations_count = 0
    for cohort, passed, filtered in ((prelint, 113, 1311), (logs, 12, 1412)):
        original = cohort["gnu-release"]
        first = next(line for line in original.splitlines() if line.startswith("test "))
        mutations = [
            "", original.rsplit("test result:", 1)[0], original + "unexpected payload\n",
            original.replace(f"{passed} passed; 0 failed", f"{passed - 1} passed; 1 failed"),
            original.replace(first + "\n", "", 1),
            original.replace(first + "\n", first + "\n" + first + "\n", 1),
        ]
        for corrupted in mutations:
            try:
                strict.parse(corrupted, passed, 0, filtered)
            except (AssertionError, IndexError):
                mutations_count += 1
            else:
                raise AssertionError("corrupt receipt accepted")
    assert prelint["source-before"] == prelint["source-after-prelint"]
    assert logs["source-before"] == logs["source-after"]
    previous, source = (json.loads(cohort["source-before"]) for cohort in (prelint, logs))
    assert previous["base"] == source["base"] == BASE
    assert previous["files"].keys() == source["files"].keys()
    assert len(source["files"]) == 5556
    assert {name for name in source["files"]
            if previous["files"][name] != source["files"][name]} == {FIXTURE}
    snapshot = (HERE / "source/generic_fixture.rs").read_bytes()
    before, after = b"20 => scoped!(engine_index, None)", b"20 => scoped!(engine_index, take)"
    assert snapshot.count(after) == 1 and before not in snapshot
    assert hashlib.sha256(snapshot).hexdigest() == source["files"][FIXTURE]
    assert hashlib.sha256(snapshot.replace(after, before)).hexdigest() == previous["files"][FIXTURE]
    if args.live:
        fresh = json.loads(subprocess.check_output(
            [sys.executable, "-I", str(ROOT / INVENTORY)], cwd=ROOT, text=True))
        assert fresh["files"] == source["files"], "qualified source roster or content changed"
    files = {}
    for path in sorted(HERE.rglob("*")):
        assert not path.is_symlink(), path
        if path.is_file() and path != HERE / "SHA256SUMS":
            files[path.relative_to(HERE).as_posix()] = sha(path)
    rendered = "".join(f"{digest}  {name}\n" for name, digest in files.items())
    if args.seal:
        with (HERE / "SHA256SUMS").open("x") as output:
            output.write(rendered)
    assert (HERE / "SHA256SUMS").read_text() == rendered
    print(json.dumps({
        "source_files": len(source["files"]), "prelint_tests_per_target": 119,
        "final_tests_per_target": 18, "gnu_runtime_smoke_tests": len(runtime),
        "production_sources_unchanged": True, "test_only_bridge_lines": 1,
        "targets": ["GNU", "musl"], "corrupt_receipts_rejected": mutations_count,
        "archive_files": len(files), "live_sources_checked": args.live,
        "scope": "CPU development only; no native, formal or performance claim",
    }))


if __name__ == "__main__":
    main()
