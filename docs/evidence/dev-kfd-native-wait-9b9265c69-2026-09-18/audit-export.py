#!/usr/bin/env python3
"""Bind every exported byte to a named committed Git blob, not the worktree."""

import hashlib
from pathlib import Path
import subprocess
import tarfile

HERE = Path(__file__).resolve().parent
REPO = "/home/harsh/.codex-tmp/fe2o3-c4-completion-20260917"
COMMIT = "9b9265c6919cb8dff9506f2c6ffa7b7f2538905f"
SOURCE_SCOPE = [
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "crates",
    "examples",
    "benchmarks/runtime_gfx942",
]
VALIDATORS = [
    "docs/evidence/dev-kfd-native-wait-mi300x-2026-09-18/summarize.py",
    "docs/evidence/dev-kfd-copy-progress-mi300x-2026-09-18/summarize.py",
]


def roster(scope):
    raw = subprocess.check_output(
        ["git", "-C", REPO, "ls-tree", "-r", "-z", COMMIT, "--", *scope]
    )
    result = {}
    for entry in raw.split(b"\0")[:-1]:
        descriptor, path = entry.split(b"\t", 1)
        mode, kind, identity = descriptor.decode().split()
        assert kind == "blob" and mode in ("100644", "100755")
        name = path.decode()
        assert name not in result
        result[name] = (mode, identity)
    return result


def main():
    source, extra = roster(SOURCE_SCOPE), roster(VALIDATORS)
    assert len(source) == 5543 and set(extra) == set(VALIDATORS)
    expected = source | extra
    observed, lines = set(), []
    with tarfile.open(HERE / "source.tar") as archive:
        for member in archive:
            if member.isdir():
                continue
            assert member.isfile() and member.name not in observed
            observed.add(member.name)
            data = archive.extractfile(member).read()
            mode, identity = expected[member.name]
            assert bool(member.mode & 0o111) == (mode == "100755")
            blob = b"blob " + str(len(data)).encode() + b"\0" + data
            assert hashlib.sha1(blob).hexdigest() == identity
            lines.append((member.name, hashlib.sha256(data).hexdigest()))
    assert observed == set(expected)
    manifest = "".join(f"{value}  {name}\n" for name, value in sorted(lines))
    assert manifest == (HERE / "source-files.sha256").read_text()
    print(
        f"commit={COMMIT} source_scope=5543 extra_pinned_validators=2 total_git_blobs=5545 exact_roster_and_content=true"
    )
    for path in ("benchmarks/runtime_gfx942/async_copy_hip.cpp", *VALIDATORS):
        print(f"git_blob={expected[path][1]} path={path}")


if __name__ == "__main__":
    main()
