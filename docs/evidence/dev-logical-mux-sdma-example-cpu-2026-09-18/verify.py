#!/usr/bin/env python3
"""Audit historical CPU receipts; --live also checks current source and ELF."""

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
HELPER = ROOT / "docs/evidence/dev-combined-sdma-example-cpu-2026-09-18/verify.py"


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


if sha(HELPER) != "072f755e9edd7044b1a4e26164bffe6f1456c5f7731d45a6eede49a06df0eedd":
    raise ValueError("pinned receipt/manifest helper")
spec = importlib.util.spec_from_file_location("prior_example", HELPER)
R = importlib.util.module_from_spec(spec)
spec.loader.exec_module(R)
R.ARCHIVE = ARCHIVE
R.RECEIPTS = R.RECEIPTS | {"manifest-calibration"}
need = R.need
NAMES = R.NAMES | {
    "logical_mux_accepts_every_admitted_lane_count_and_explicit_device",
    "logical_mux_rejects_unsupported_or_ambiguous_arguments",
    "logical_mux_oracle_accepts_two_sparse_native_ids_for_every_lane_count",
    "logical_mux_oracle_rejects_each_owner_field_roster_and_lane_mutation",
}
PRIOR = ROOT / "docs/evidence/dev-logical-mux-sdma-release-cpu-2026-09-18"


def harness(text):
    rows = R.load_parser()(text, 19, 0)
    need(rows == {"tests::" + name: "ok" for name in NAMES}, "exact example roster")
    return rows


def qualify(live=False):
    need(sha(R.SOURCE) == "a7a8b8e73aa1a0817e13f9e575369403ece42f4eb702fd1ec04d9bc57fcff953", "pinned source selector")
    env = ["env", "CARGO_INCREMENTAL=0", "CARGO_PROFILE_DEV_DEBUG=0", "CARGO_PROFILE_TEST_DEBUG=0", "CARGO_BUILD_JOBS=2", "CARGO_TERM_COLOR=never", "RUST_TEST_THREADS=1"]
    base = ["--frozen", "-p", "fe2o3-kfd", "--all-features"]
    target = ["--target", "x86_64-unknown-linux-musl"]
    example = ["--example", "kfd-compute-aql-queue"]
    source = ["python3", "-I", R.SOURCE.relative_to(ROOT).as_posix()]
    commands = {
        "source-before": source,
        "rustc": ["rustc", "-vV"],
        "cargo": ["cargo", "-V"],
        "gnu-example": env + ["cargo", "test"] + base + example,
        "musl-example": env + ["cargo", "test"] + base + target + example,
        "clippy": env + ["cargo", "clippy"] + base + ["--all-targets", "--", "-D", "warnings"],
        "musl-build": env + ["cargo", "build"] + base + target + example,
        "binary": ["sha256sum", R.BINARY],
        "binary-kind": ["file", R.BINARY],
        "fmt": ["cargo", "fmt", "--all", "--check"],
        "diff": ["git", "diff", "--check"],
        "source-after": source,
        "source-unchanged": None,
    }
    previous = None
    for name, command in commands.items():
        times, actual = R.receipt(name, command)
        need(previous is None or previous <= times[0], "qualification chronology")
        previous = times[1]
        if name == "source-unchanged":
            need(len(actual) == 3 and actual[0] == "cmp", "source comparison")
            before, after = map(Path, actual[1:])
            need(before.parent == after.parent and before.is_absolute() and before.parts[-3:] == (ARCHIVE.name, "raw", "source-before.log") and after.name == "source-after.log", "portable historical cmp operands")
    need(harness((ARCHIVE / "raw/gnu-example.log").read_text()) == harness((ARCHIVE / "raw/musl-example.log").read_text()), "same target roster")
    for name in ("fmt", "diff", "source-unchanged"):
        need((ARCHIVE / f"raw/{name}.log").read_bytes() == b"", "empty successful check")
    before = (ARCHIVE / "raw/source-before.log").read_bytes()
    need(before == (ARCHIVE / "raw/source-after.log").read_bytes(), "unchanged source")
    inventory = json.loads(before)
    need(inventory["base"] == "95ed0301cb532dd4bd762ec4410f0bd9a61814cc" and len(inventory["files"]) == 5558, "source base/count")
    need(sha(PRIOR / "SHA256SUMS") == "07a7fed5628811fcc961ddf1bd6513cc96147c380effc4663db01e691865998b", "prior release CPU seal")
    manifest = dict(line.split("  ", 1)[::-1] for line in (PRIOR / "SHA256SUMS").read_text().splitlines())
    need(sha(PRIOR / "raw/source-after.log") == manifest["raw/source-after.log"], "prior source map")
    old = json.loads((PRIOR / "raw/source-after.log").read_text())["files"]
    current = inventory["files"]
    need(set(old) == set(current) and [p for p in current if old[p] != current[p]] == [R.EXAMPLE], "only example differs from qualified runtime")
    match = re.fullmatch(r"([0-9a-f]{64})  " + re.escape(R.BINARY) + "\n", (ARCHIVE / "raw/binary.log").read_text())
    need(match is not None, "exact non-test executable binding")
    kind = (ARCHIVE / "raw/binary-kind.log").read_text()
    need(kind.startswith(R.BINARY + ": ELF 64-bit LSB") and "static-pie linked" in kind, "static musl ELF")
    if live:
        fresh = json.loads(subprocess.check_output(["python3", "-I", str(R.SOURCE)], text=True))
        need(fresh["files"] == current, "live complete source identity")
        need(sha(ROOT / R.BINARY) == match[1], "live executable identity")
    return {"example_tests_per_target": 19, "source_files": len(current), "binary_sha256": match[1], "native_execution": False, "formal_refinement": False}


def supplementary():
    times, _ = R.receipt("calibration", ["python3", "-B", str(ARCHIVE.relative_to(ROOT) / "test_verify.py"), "-v"])
    need(R.receipt("source-unchanged")[0][1] <= times[0], "calibration chronology")
    expected = "".join(f"test_{name} (__main__.HarnessTests.test_{name}) ... ok\n" for name in ("both_real_transcripts", "final_source_and_commands", "rejects_twelve_malformed_harnesses", "seal_modes"))
    need(re.fullmatch(re.escape(expected) + r"\n-{70}\nRan 4 tests in [0-9]+\.[0-9]+s\n\nOK\n", (ARCHIVE / "raw/calibration.log").read_text()) is not None, "complete calibration transcript")
    final_times, _ = R.receipt("manifest-calibration", ["python3", "-B", str(ARCHIVE.relative_to(ROOT) / "test_manifest.py"), "-v"])
    need(times[1] <= final_times[0], "final manifest calibration chronology")
    final_expected = expected.replace("__main__.HarnessTests", "test_verify.HarnessTests")
    final_expected += "test_exact_membership (__main__.ManifestTests.test_exact_membership) ... ok\n"
    need(re.fullmatch(re.escape(final_expected) + r"\n-{70}\nRan 5 tests in [0-9]+\.[0-9]+s\n\nOK\n", (ARCHIVE / "raw/manifest-calibration.log").read_text()) is not None, "complete final calibration transcript")


def expected_paths():
    paths = {".gitattributes", "README.md", "qualify.sh", "record.sh", "test_verify.py", "test_manifest.py", "verify.py"}
    paths.update(f"raw/{name}.{suffix}" for name in R.RECEIPTS for suffix in ("command", "started", "finished", "exit", "log"))
    return paths


def manifest(archive=ARCHIVE):
    files = {}
    for path in sorted(archive.rglob("*")):
        need(not path.is_symlink(), "ordinary archive paths")
        if path.is_file() and path != archive / "SHA256SUMS":
            files[path.relative_to(archive).as_posix()] = sha(path)
    need(set(files) == expected_paths(), "exact archive and receipt membership")
    return "".join(f"{digest}  {path}\n" for path, digest in files.items())


def verify_seal(path, contents, seal=False, allow_unsealed=False):
    if seal:
        with path.open("x") as output:
            output.write(contents)
    elif path.exists():
        need(path.read_text() == contents, "exact sealed archive closure")
    else:
        need(allow_unsealed, "missing archive seal")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--seal", action="store_true")
    mode.add_argument("--allow-unsealed", action="store_true")
    parser.add_argument("--live", action="store_true")
    args = parser.parse_args()
    result = qualify(args.live)
    supplementary()
    verify_seal(ARCHIVE / "SHA256SUMS", manifest(), args.seal, args.allow_unsealed)
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
