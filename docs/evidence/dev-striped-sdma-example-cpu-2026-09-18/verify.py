#!/usr/bin/env python3
"""Read-only historical CPU audit; --seal creates a manifest exactly once."""

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
HELPER = ROOT / "docs/evidence/dev-kfd-native-wait-cpu-2026-09-18/verify.py"
SOURCE = ROOT / "docs/evidence/dev-sdma-direct-readback-cpu-2026-09-18/source.py"
PRIOR = ROOT / "docs/evidence/dev-striped-sdma-release-cpu-2026-09-18"
EXAMPLE = "crates/fe2o3-kfd/examples/kfd-compute-aql-queue.rs"
BINARY = "target/x86_64-unknown-linux-musl/debug/examples/kfd-compute-aql-queue"
RECEIPTS = {"source-before", "rustc", "cargo", "gnu-example", "musl-example", "clippy", "musl-build", "binary", "binary-kind", "fmt", "diff", "source-after", "source-unchanged", "calibration", "host-availability"}
NAMES = {
    "all_is_an_explicit_selection",
    "explicit_unique_ids_accept_decimal_and_hex",
    "host_usage_oracle_counts_live_retained_records_and_empty_refund",
    "malformed_or_ambiguous_arguments_are_rejected",
    "observation_oracle_accepts_sparse_ids_and_all_admitted_profiles",
    "observation_oracle_rejects_each_malformed_field_and_roster",
    "resource_oracle_counts_primary_and_all_sdma_owners",
    "retained_release_requires_explicit_mode_and_device_selection",
    "single_sdma_requires_a_known_profile_and_one_explicit_device",
    "striped_sdma_accepts_every_balanced_count_and_one_explicit_device",
    "striped_sdma_rejects_unsupported_or_ambiguous_arguments",
}


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def need(condition, message):
    if not condition:
        raise ValueError(message)


def load_parser():
    need(sha(HELPER) == "13b8c2fc57cb013cd19df6621efb0cd22a79b8ce84c9539ba4258b95a8c22ddb", "pinned harness parser")
    spec = importlib.util.spec_from_file_location("cpu_harness", HELPER)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.parse


def harness(text):
    rows = load_parser()(text, 11, 0)
    need(rows == {"tests::" + name: "ok" for name in NAMES}, "exact example roster")
    return rows


def receipt(name, command=None):
    paths = {suffix: ARCHIVE / f"raw/{name}.{suffix}" for suffix in ("command", "started", "finished", "exit", "log")}
    need(all(path.is_file() and not path.is_symlink() for path in paths.values()), "complete receipt: " + name)
    need(paths["exit"].read_text() == "0\n", "successful receipt: " + name)
    actual = shlex.split(paths["command"].read_text())
    if command is not None:
        need(actual == command, "exact command: " + name)
    times = [paths[key].read_text().strip() for key in ("started", "finished")]
    need(all(re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{9}Z", value) for value in times) and times[0] <= times[1], "ordered receipt: " + name)
    return times, actual


def qualify(live=False):
    need(sha(SOURCE) == "a7a8b8e73aa1a0817e13f9e575369403ece42f4eb702fd1ec04d9bc57fcff953", "pinned source selector")
    env = ["env", "CARGO_INCREMENTAL=0", "CARGO_PROFILE_DEV_DEBUG=0", "CARGO_PROFILE_TEST_DEBUG=0", "CARGO_BUILD_JOBS=2", "CARGO_TERM_COLOR=never", "RUST_TEST_THREADS=1"]
    base = ["--frozen", "-p", "fe2o3-kfd", "--all-features"]
    target = ["--target", "x86_64-unknown-linux-musl"]
    example = ["--example", "kfd-compute-aql-queue"]
    source_command = ["python3", "-I", SOURCE.relative_to(ROOT).as_posix()]
    commands = {
        "source-before": source_command,
        "rustc": ["rustc", "-vV"],
        "cargo": ["cargo", "-V"],
        "gnu-example": env + ["cargo", "test"] + base + example,
        "musl-example": env + ["cargo", "test"] + base + target + example,
        "clippy": env + ["cargo", "clippy"] + base + ["--all-targets", "--", "-D", "warnings"],
        "musl-build": env + ["cargo", "build"] + base + target + example,
        "binary": ["sha256sum", BINARY],
        "binary-kind": ["file", BINARY],
        "fmt": ["cargo", "fmt", "--all", "--check"],
        "diff": ["git", "diff", "--check"],
        "source-after": source_command,
        "source-unchanged": None,
    }
    previous = None
    for name, command in commands.items():
        times, actual = receipt(name, command)
        need(previous is None or previous <= times[0], "qualification chronology")
        previous = times[1]
        if name == "source-unchanged":
            need(len(actual) == 3 and actual[0] == "cmp", "source comparison")
            before, after = map(Path, actual[1:])
            need(before.parent == after.parent and before.is_absolute() and before.parts[-3:] == (ARCHIVE.name, "raw", "source-before.log") and after.name == "source-after.log", "portable historical cmp operands")
    need(harness((ARCHIVE / "raw/gnu-example.log").read_text()) == harness((ARCHIVE / "raw/musl-example.log").read_text()), "same target test roster")
    for name in ("fmt", "diff", "source-unchanged"):
        need((ARCHIVE / f"raw/{name}.log").read_bytes() == b"", "empty successful check")
    before = (ARCHIVE / "raw/source-before.log").read_bytes()
    need(before == (ARCHIVE / "raw/source-after.log").read_bytes(), "unchanged complete source")
    inventory = json.loads(before)
    need(inventory["base"] == "3cc315d2a4e15b1fc74eb9ee10496ba683bdbb34" and len(inventory["files"]) == 5556, "source base and count")
    need(sha(PRIOR / "SHA256SUMS") == "943b0d047967469efcc24d0b6d126c8cd252e4a07b6b553986ed869f36cb026d", "prior retained-release CPU seal")
    manifest = dict(line.split("  ", 1)[::-1] for line in (PRIOR / "SHA256SUMS").read_text().splitlines())
    prior_source = PRIOR / "raw/source-after.log"
    need(sha(prior_source) == manifest["raw/source-after.log"], "prior final source map")
    old = json.loads(prior_source.read_text())["files"]
    current = inventory["files"]
    need(set(old) == set(current) and [path for path in current if old[path] != current[path]] == [EXAMPLE], "only the public example differs from qualified retained-release source")
    binary_line = (ARCHIVE / "raw/binary.log").read_text()
    match = re.fullmatch(r"([0-9a-f]{64})  " + re.escape(BINARY) + "\n", binary_line)
    need(match is not None, "exact non-test binary binding")
    kind = (ARCHIVE / "raw/binary-kind.log").read_text()
    need(kind.startswith(BINARY + ": ELF 64-bit LSB") and "static-pie linked" in kind, "static musl ELF observation")
    if live:
        fresh = json.loads(subprocess.check_output(["python3", "-I", str(SOURCE)], text=True))
        need(fresh["files"] == current, "live complete source identity")
        need(sha(ROOT / BINARY) == match[1], "live executable identity")
    return {"example_tests_per_target": 11, "targets": ["gnu", "musl"], "source_files": len(current), "binary_sha256": match[1], "native_execution": False}


def manifest():
    files = {}
    for path in sorted(ARCHIVE.rglob("*")):
        need(not path.is_symlink(), "ordinary archive paths")
        if path.is_file() and path.name != "SHA256SUMS":
            files[path.relative_to(ARCHIVE).as_posix()] = sha(path)
    expected = {".gitattributes", "README.md", "qualify.sh", "record.sh", "test_verify.py", "verify.py"}
    expected.update(f"raw/{name}.{suffix}" for name in RECEIPTS for suffix in ("command", "started", "finished", "exit", "log"))
    need(set(files) == expected, "exact archive and receipt membership")
    return "".join(f"{digest}  {path}\n" for path, digest in files.items())


def supplementary():
    receipt("calibration", ["python3", "-B", str(ARCHIVE.relative_to(ROOT) / "test_verify.py"), "-v"])
    calibration = (ARCHIVE / "raw/calibration.log").read_text()
    expected = "".join(f"test_{name} (__main__.HarnessTests.test_{name}) ... ok\n" for name in ("both_real_transcripts", "final_source_and_commands", "rejects_twelve_malformed_harnesses"))
    need(re.fullmatch(re.escape(expected) + r"\n-{70}\nRan 3 tests in [0-9]+\.[0-9]+s\n\nOK\n", calibration) is not None, "complete calibration transcript")
    receipt("host-availability", ["timeout", "--signal=TERM", "--kill-after=5s", "45s", "ssh", "-T", "-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "mi300x", "/opt/rocm/bin/rocm-smi", "--showuniqueid", "--showbus", "--showuse", "--showmeminfo", "vram", "--showpids", "--json"])
    availability = json.loads((ARCHIVE / "raw/host-availability.log").read_text())
    need(set(availability) == {f"card{index}" for index in range(8)} | {"system"}, "complete SMI inventory, not admission")
    for index in range(8):
        card = availability[f"card{index}"]
        need(set(card) == {"Unique ID", "GPU use (%)", "PCI Bus", "VRAM Total Memory (B)", "VRAM Total Used Memory (B)"}, "SMI fields")
        need(re.fullmatch(r"0x[0-9a-f]{16}", card["Unique ID"]) and re.fullmatch(r"[0-9A-Fa-f]{4}:[0-9A-Fa-f]{2}:[0-9A-Fa-f]{2}\.[0-7]", card["PCI Bus"]), "SMI identity shape")
        need(0 <= int(card["GPU use (%)"]) <= 100 and 0 <= int(card["VRAM Total Used Memory (B)"]) <= int(card["VRAM Total Memory (B)"]), "SMI numeric range")
        if index != 0:
            need(card["GPU use (%)"] == "0" and 280 * 1024 * 1024 <= int(card["VRAM Total Used Memory (B)"]) < 290 * 1024 * 1024, "documented transient candidate snapshot, not native admission")
    need(type(availability["system"]) is dict and all(re.fullmatch(r"PID[1-9][0-9]*", key) and isinstance(value, str) for key, value in availability["system"].items()), "reported process inventory")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--seal", action="store_true")
    mode.add_argument("--allow-unsealed", action="store_true")
    parser.add_argument("--live", action="store_true")
    args = parser.parse_args()
    result = qualify(args.live)
    supplementary()
    contents = manifest()
    seal = ARCHIVE / "SHA256SUMS"
    if args.seal:
        with seal.open("x") as output:
            output.write(contents)
    elif not args.allow_unsealed or seal.exists():
        need(seal.read_text() == contents, "exact sealed archive closure")
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
