#!/usr/bin/env python3
"""Check CPU receipts and optional live inputs; never execute the native example."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shlex

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
PARSER = HERE.parent / "dev-kfd-native-wait-cpu-2026-09-18/verify.py"
PARSER_SHA = "13b8c2fc57cb013cd19df6621efb0cd22a79b8ce84c9539ba4258b95a8c22ddb"
BINARY = "target/x86_64-unknown-linux-musl/debug/examples/kfd-compute-aql-queue"
ENV = ["env", "CARGO_INCREMENTAL=0", "CARGO_PROFILE_DEV_DEBUG=0", "CARGO_PROFILE_TEST_DEBUG=0", "CARGO_BUILD_JOBS=2", "CARGO_TERM_COLOR=never", "RUST_TEST_THREADS=1"]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seal", action="store_true")
    parser.add_argument("--live", action="store_true")
    args = parser.parse_args()
    assert sha(PARSER) == PARSER_SHA
    spec = importlib.util.spec_from_file_location("strict_cpu_harness", PARSER)
    strict = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(strict)
    example = ["cargo", "test", "--frozen", "-p", "fe2o3-kfd", "--all-features"]
    target = ["--target", "x86_64-unknown-linux-musl"]
    inventory = ["python3", "-I", "docs/evidence/dev-sdma-direct-readback-cpu-2026-09-18/source.py"]
    commands = {
        "source-before": inventory,
        "rustc": ["rustc", "-vV"],
        "cargo": ["cargo", "-V"],
        "gnu-example": ENV + example + ["--example", "kfd-compute-aql-queue"],
        "musl-example": ENV + example + target + ["--example", "kfd-compute-aql-queue"],
        "clippy": ENV + ["cargo", "clippy", "--frozen", "-p", "fe2o3-kfd", "--all-features", "--all-targets", "--", "-D", "warnings"],
        "musl-build": ENV + ["cargo", "build"] + example[2:] + target + ["--example", "kfd-compute-aql-queue"],
        "binary": ["sha256sum", BINARY],
        "binary-kind": ["file", BINARY],
        "fmt": ["cargo", "fmt", "--all", "--check"],
        "diff": ["git", "diff", "--check"],
        "source-after": inventory,
        "source-unchanged": ["cmp", str(HERE / "raw/source-before.log"), str(HERE / "raw/source-after.log")],
    }
    finish = ""
    for name, command in commands.items():
        read = lambda suffix: (HERE / f"raw/{name}.{suffix}").read_text()
        assert shlex.split(read("command")) == command, name
        assert read("exit") == "0\n", name
        start, end = read("started").strip(), read("finished").strip()
        assert all(re.fullmatch(r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d{9}Z", t) for t in (start, end))
        assert finish <= start <= end, name
        finish = end
    expected = {"tests::" + name: "ok" for name in (
        "all_is_an_explicit_selection", "explicit_unique_ids_accept_decimal_and_hex",
        "retained_release_requires_explicit_mode_and_device_selection",
        "malformed_or_ambiguous_arguments_are_rejected",
        "single_sdma_requires_a_known_profile_and_one_explicit_device",
    )}
    for name in ("gnu-example", "musl-example"):
        assert strict.parse((HERE / f"raw/{name}.log").read_text(), 5, 0) == expected
    before = (HERE / "raw/source-before.log").read_bytes()
    assert before == (HERE / "raw/source-after.log").read_bytes()
    source = json.loads(before)
    assert source["base"] == "23da086dc5886020c404441273ad3d31ce7b1c7d"
    assert len(source["files"]) == 5555
    digest, path = (HERE / "raw/binary.log").read_text().strip().split("  ")
    assert path == BINARY and re.fullmatch(r"[0-9a-f]{64}", digest)
    assert "static-pie linked" in (HERE / "raw/binary-kind.log").read_text()
    if args.live:
        assert sha(ROOT / BINARY) == digest
        for path, value in source["files"].items():
            assert sha(ROOT / path) == value, path
    files = {}
    for path in sorted(HERE.rglob("*")):
        assert not path.is_symlink()
        if path.is_file() and path != HERE / "SHA256SUMS":
            files[path.relative_to(HERE).as_posix()] = sha(path)
    rendered = "".join(f"{digest}  {path}\n" for path, digest in files.items())
    if args.seal:
        with (HERE / "SHA256SUMS").open("x") as output:
            output.write(rendered)
    assert (HERE / "SHA256SUMS").read_text() == rendered
    print(json.dumps({"source_files": 5555, "example_tests_per_target": 5, "binary_sha256": digest, "archive_files": len(files), "live_inputs_checked": args.live, "scope": "CPU example tests and native executable build only"}))


if __name__ == "__main__":
    main()
