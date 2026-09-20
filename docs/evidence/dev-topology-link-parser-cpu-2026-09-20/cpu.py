#!/usr/bin/env python3
"""Record/replay CPU regression evidence for fixed-schema link parsing."""

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
PREFIX = "docs/evidence/dev-topology-link-parser-cpu-2026-09-20/"
HELPER = "docs/evidence/dev-xgmi-aggregate-attribution-cpu-2026-09-19/cpu.py"
HELPER_SHA = "a0b39f6821352fa3f62c9fc72484519cd7a27b4c65d54e1f77f13c44a6b23686"
path = ROOT / HELPER
if path.is_symlink() or hashlib.sha256(path.read_bytes()).hexdigest() != HELPER_SHA:
    raise RuntimeError("unauthenticated CPU receipt helper")
spec = importlib.util.spec_from_file_location("link_cpu_helpers", path)
C = importlib.util.module_from_spec(spec)
spec.loader.exec_module(C)
need, sha, read = C.need, C.sha, C.read

STATIC = ("cpu.py", "test_cpu.py", "PROTOCOL.md")
DOC = "docs/runtime-topology-link-parser-v1.md"
FMT = (
    "crates/fe2o3-kfd/src/topology.rs",
    "crates/fe2o3-kfd/src/topology/link_properties.rs",
    "crates/fe2o3-kfd/src/topology/tests/link_properties.rs",
)
FILTERS = ("topology::tests::", "currentness::",
           "shared_memory::pair_currentness::tests::", "device::tests::")
NEW_TESTS = C.family("topology::tests::link_properties::", """
distinct_fields_bounds_and_property_order_match_generic_parser
every_nonempty_schema_subset_preserves_first_missing_key
duplicate_numeric_range_and_unknown_precedence_match_generic_parser
malformed_lines_terminators_and_overfull_input_preserve_error_order
full_discovery_matches_reference_links_and_is_invariant_to_property_order
endpoint_and_range_validation_order_and_zero_maximum_are_unchanged
""")
CONFIGURATIONS = ("gnu", "musl", "feature-off")
PYTHON_TESTS = {"test_commands", "test_test_output", "test_inventory", "test_typed_binding"}
SELECTED_COUNT = 93
SELECTED_SHA = "1e39828c233a0fd04e52cffddc797c63e1aec199ccc3cc27b3001ca4a582a725"


def commands(root, target):
    root = Path(root)
    need(re.fullmatch(r"/dev/shm/fe2o3-link-parser-build-20260920\.[A-Za-z0-9]{8}/target", target),
         "owned build target shape")
    env = C.ENV + ["CARGO_TARGET_DIR=" + target]
    python = ["python3", "-I", "-B"]
    result = [("source-before", python + [str(root / C.SELECTOR)], 60),
              ("rustc", ["rustc", "-vV"], 30), ("cargo", ["cargo", "-V"], 30)]
    for configuration in CONFIGURATIONS:
        args = ["--no-default-features"] if configuration == "feature-off" else ["--all-features"]
        args += ["--target", "x86_64-unknown-linux-musl" if configuration == "musl"
                 else "x86_64-unknown-linux-gnu"]
        command = env + ["cargo", "test", "--frozen", "-p", "fe2o3-kfd", *args, "--lib", "--"]
        result += [(configuration + "-roster", command + ["--list"], 1800),
                   (configuration + "-tests", command + list(FILTERS), 1800)]
    safety = env + ["cargo", "test", "--frozen", "-p", "cargo-fe2o3", "--test", "unsafe_source_policy"]
    result += [
        ("safety-roster", safety + ["--", "--list"], 1800),
        ("safety-tests", safety, 1800),
        ("verifier-tests", python + [str(root / PREFIX / "test_cpu.py"), "-v"], 120),
        ("fmt", ["rustfmt", "--edition", "2024", "--check", "--config", "skip_children=true", *FMT], 180),
        ("clippy", env + ["cargo", "clippy", "--frozen", "-p", "fe2o3-kfd", "-p", "fe2o3-runtime",
                          "--all-features", "--lib", "--", "-D", "warnings"], 1800),
        ("source-after", python + [str(root / C.SELECTOR)], 60),
    ]
    return result


def tools():
    paths = dict(C.HELPERS, **{HELPER: HELPER_SHA})
    for name, digest in paths.items():
        need(sha(ROOT / name) == digest, "unchanged authenticated helper: " + name)
    paths.update({PREFIX + name: sha(HERE / name) for name in STATIC})
    paths[DOC] = sha(ROOT / DOC)
    return paths


def selected_tests(full):
    selected = {name for name in full if any(part in name for part in FILTERS)}
    canonical = "\n".join(sorted(selected)) + "\n"
    need(len(selected) == SELECTED_COUNT and hashlib.sha256(canonical.encode()).hexdigest() == SELECTED_SHA,
         "pinned selected test roster")
    need(NEW_TESTS <= selected, "all new differential tests selected")
    return selected


def derive(archive, execution_root, target, live=False):
    archive, execution_root = Path(archive), Path(execution_root)
    need(execution_root.is_absolute() and ".." not in execution_root.parts, "execution root")
    previous = None
    for stage, command, seconds in commands(execution_root, target):
        row = C.receipt(archive, stage, command, seconds, previous)
        need(row["cwd"] == str(execution_root), "one execution root")
        previous = row["finished_ns"]
    before = archive / "raw/source-before/stdout"
    need(before.read_bytes() == (archive / "raw/source-after/stdout").read_bytes(), "unchanged source")
    source = C.source_snapshot(read(before))
    need(set(FMT) <= set(source["files"]), "new parser/test source included")
    need(C.same_json(read(archive / "tools.json"), tools()), "unchanged tools")

    def output(stage, stream="stdout"):
        return (archive / "raw" / stage / stream).read_text()

    rosters = {}
    for configuration in CONFIGURATIONS:
        full = C.roster(output(configuration + "-roster"))
        selected = selected_tests(full)
        C.passing_tests(output(configuration + "-tests"), selected, len(full) - len(selected))
        rosters[configuration] = sorted(selected)
    need(len({tuple(names) for names in rosters.values()}) == 1, "identical selected configuration rosters")
    need(C.roster(output("safety-roster")) == C.SAFETY_TESTS | set(C.SAFETY_IGNORED), "unsafe roster")
    C.passing_tests(output("safety-tests"), C.SAFETY_TESTS, ignored=C.SAFETY_IGNORED)
    python_output = output("verifier-tests", "stderr")
    names = re.findall(r"^(test_[a-z_]+) \([^\n]+\) \.\.\. ok$", python_output, re.M)
    need(len(names) == len(set(names)) and set(names) == PYTHON_TESTS, "verifier test roster")
    need(len(re.findall(r"^test_", python_output, re.M)) == len(names), "no other verifier outcomes")
    need(re.search(r"\nRan 4 tests in [0-9]+\.[0-9]+s\n\nOK\n\Z", python_output), "verifier success")
    need(output("rustc").startswith("rustc ") and output("cargo").startswith("cargo "), "toolchain identity")
    if live:
        current = C.source_snapshot(C.parse_json(subprocess.check_output(
            ["python3", "-I", "-B", str(ROOT / C.SELECTOR)], cwd=ROOT)))
        need(current["files"] == source["files"], "live source map")
        for name, digest in source["files"].items():
            need(sha(ROOT / name) == digest, "ordinary live source: " + name)
    return {"schema": "fe2o3.topology-link-parser-cpu.v1", "execution_root": str(execution_root),
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
                      "tests_each_configuration": len(report["rosters"]["gnu"]),
                      "native_execution": False, "performance_acceptance": False, "formal_refinement": False}))


if __name__ == "__main__":
    main()
