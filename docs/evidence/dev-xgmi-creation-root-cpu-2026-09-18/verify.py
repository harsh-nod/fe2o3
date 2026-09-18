#!/usr/bin/env python3
"""Check frozen CPU receipts; never execute an archived command."""

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import sys

sys.dont_write_bytecode = True
ARCHIVE = Path(__file__).resolve().parent
ROOT = ARCHIVE.parents[2]


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


PRIOR = ROOT / "docs/evidence/dev-sdma-creation-escrow-cpu-2026-09-18/verify.py"
need(sha(PRIOR) == "8508dc2790a072fd26883dd974dbcf432a903129081cbe24f0b131e20956c752", "pinned receipt library")
spec = importlib.util.spec_from_file_location("creation_cpu_receipts", PRIOR)
prior = importlib.util.module_from_spec(spec)
spec.loader.exec_module(prior)
helper = prior.helper
helper.ARCHIVE = ARCHIVE
verify_seal = prior.verify_seal
FILTERS = [
    "sdma::xgmi_creation::tests::", "sdma::tests::xgmi_",
    "sdma::tests::creation_guards_cover_the_first_memory_operation_and_xgmi_route_scope",
    "sdma::tests::sdma_copy_manifest_digest_is_frozen",
    "queue_linux::tests::terminal_creation_arm_poisons_on_drop_or_unwind_and_disarms_only_on_success",
    "queue::live::construction_primary::integration_tests::release_cases::sdma_creation_cases::",
]
# Counts, filtered counts, and exact ordered-name hashes are frozen after review.
ROSTERS = {
    "kfd": (30, 1420, "b0d3276411aa56c7795c89854266597b65ce62e86b78a6dd8b7d2789d4d26644"),
    "runtime": (17, 1113, "5f1c861876ac20c58eb850cf558351e68f26a1ced44c03948053c3a15df682ff"),
}


def parse_roster(text, kind):
    count, _, digest = ROSTERS[kind]
    lines = text.splitlines()
    while lines and not lines[0].endswith(": test"):
        line = lines.pop(0)
        need(not line.strip() or re.fullmatch(
            r"   Compiling .+|    Finished `test` profile .+|     Running unittests .+", line
        ), "roster prelude")
    need(len(lines) == count + 2 and lines[-2:] == ["", f"{count} tests, 0 benchmarks"], "complete roster")
    need(all(re.fullmatch(r"\S+: test", line) for line in lines[:-2]), "roster rows")
    names = [line.removesuffix(": test") for line in lines[:-2]]
    need(names == sorted(set(names)), "unique sorted roster")
    need(hashlib.sha256(("\n".join(names) + "\n").encode()).hexdigest() == digest, "exact named roster")
    return set(names)


def commands():
    specs = prior.commands()
    env = specs["clippy"][:specs["clippy"].index("cargo")]
    result = {name: specs[name] for name in ["source-before", "rustc", "cargo", "clippy", "no-default"]}
    for target in ["gnu", "musl"]:
        target_args = ["--target", "x86_64-unknown-linux-musl"] if target == "musl" else []
        base = env + ["cargo", "test", "--frozen", "-p", "fe2o3-kfd", "--all-features"] + target_args
        runtime = env + ["cargo", "test", "--frozen", "-p", "fe2o3-runtime", "--all-features"] + target_args
        result[target + "-roster"] = base + ["--lib", "--", "--list"] + FILTERS
        result[target + "-kfd"] = base + ["--lib", "--"] + FILTERS
        result[target + "-runtime-roster"] = runtime + ["--lib", "--", "--list", "kfd_backend::tests::native_xgmi_"]
        result[target + "-runtime"] = runtime + ["--lib", "--", "kfd_backend::tests::native_xgmi_"]
        result[target + "-example"] = base + ["--example", "kfd-sdma-xgmi-peer-benchmark"]
    result.update({name: specs[name] for name in ["unsafe-source", "fmt", "diff", "source-after", "source-unchanged"]})
    result["calibration"] = ["python3", "-B", "docs/evidence/dev-xgmi-creation-root-cpu-2026-09-18/test_verify.py"]
    return result


def check_receipts():
    last = None
    for name, command in commands().items():
        start, end = helper.receipt(name, command)
        need(last is None or last <= start, "qualification chronology")
        last = end


def qualify(live=False):
    need(sha(helper.SOURCE) == "a7a8b8e73aa1a0817e13f9e575369403ece42f4eb702fd1ec04d9bc57fcff953", "pinned source selector")
    check_receipts()
    for target in ["gnu", "musl"]:
        for kind, roster_suffix in [("kfd", "roster"), ("runtime", "runtime-roster")]:
            names = parse_roster((ARCHIVE / f"raw/{target}-{roster_suffix}.log").read_text(), kind)
            helper.parse_harness((ARCHIVE / f"raw/{target}-{kind}.log").read_text(), names, ROSTERS[kind][1])
        helper.parse_harness((ARCHIVE / f"raw/{target}-example.log").read_text(), set(), 0)
    parser_spec = importlib.util.spec_from_file_location("strict_harness", helper.PARSER)
    parser = importlib.util.module_from_spec(parser_spec)
    parser_spec.loader.exec_module(parser)
    need(parser.parse((ARCHIVE / "raw/unsafe-source.log").read_text(), 5, 1) == prior.UNSAFE, "exact unsafe-policy outcomes")
    for name in ["fmt", "diff", "source-unchanged"]:
        need((ARCHIVE / f"raw/{name}.log").read_bytes() == b"", "empty successful check")
    before = (ARCHIVE / "raw/source-before.log").read_bytes()
    need(before == (ARCHIVE / "raw/source-after.log").read_bytes(), "unchanged complete source")
    source = json.loads(before)
    need(source["base"] == "3813f93d3b67a627d52741c1100c82a6185f1544" and len(source["files"]) == 5562, "source base and count")
    need(re.fullmatch(r"\.{6}\n-+\nRan 6 tests in [0-9.]+s\n\nOK\n", (ARCHIVE / "raw/calibration.log").read_text()), "complete calibration harness")
    if live:
        current = json.loads(subprocess.check_output(["python3", "-I", str(helper.SOURCE)], text=True))
        need(current["files"] == source["files"], "live source identity")
    return {"kfd_tests_per_target": ROSTERS["kfd"][0], "runtime_tests_per_target": ROSTERS["runtime"][0],
            "source_files": len(source["files"]), "native_execution": False, "formal_refinement": False,
            "performance_acceptance": False}


def manifest():
    paths = {}
    for path in sorted(ARCHIVE.rglob("*")):
        need(not path.is_symlink(), "ordinary archive paths")
        if path.is_dir():
            need(path == ARCHIVE / "raw", "exact archive directories")
            continue
        need(path.is_file(), "ordinary archive files")
        if path != ARCHIVE / "SHA256SUMS":
            paths[path.relative_to(ARCHIVE).as_posix()] = sha(path)
    expected = {".gitattributes", "README.md", "record.sh", "qualify.sh", "verify.py", "test_verify.py"}
    expected.update(f"raw/{name}.{suffix}" for name in commands()
                    for suffix in ["command", "exit", "started", "finished", "log"])
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
    verify_seal(ARCHIVE / "SHA256SUMS", manifest(), create=args.seal, allow_absent=args.allow_unsealed)
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
