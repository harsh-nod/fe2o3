#!/usr/bin/env python3
"""Export the exact three Git blobs, without reading working-tree source."""

import hashlib
import json
from pathlib import Path
import subprocess
import tarfile

BASE = "1890a64e1911a2a346c5a9e70a8ff5ad45b19231"
REPO = Path("/home/harsh/.codex-tmp/fe2o3-c4-completion-20260917")
STAGE = Path(__file__).resolve().parent
PATHS = [
    "benchmarks/runtime_gfx942/async_copy_hip.cpp",
    "benchmarks/runtime_gfx942/copy-host-observe.py",
    "benchmarks/runtime_gfx942/native_benchmark_args.hpp",
]


def git(*args):
    return subprocess.check_output(["git", "-C", str(REPO), *args])


def main():
    assert not (STAGE / "source.tar").exists()
    assert git("rev-parse", f"{BASE}^{{commit}}").decode().strip() == BASE
    subprocess.run(
        ["git", "-C", str(REPO), "archive", "--format=tar",
         f"--output={STAGE / 'source.tar'}", BASE, *PATHS], check=True,
    )
    records = []
    with tarfile.open(STAGE / "source.tar") as archive:
        files = [member for member in archive.getmembers() if member.isfile()]
        assert [member.name for member in files] == PATHS
        assert all(member.isfile() or member.isdir() for member in archive.getmembers())
        for member in files:
            content = archive.extractfile(member).read()
            original = git("show", f"{BASE}:{member.name}")
            assert content == original
            blob = git("rev-parse", f"{BASE}:{member.name}").decode().strip()
            records.append({"path": member.name, "git_blob": blob,
                            "sha256": hashlib.sha256(content).hexdigest(),
                            "bytes": len(content)})
    (STAGE / "source-files.sha256").write_text(
        "".join(f"{row['sha256']}  {row['path']}\n" for row in records)
    )
    result = {
        "schema": "fe2o3.hip-smoke-source-export.v1", "commit": BASE,
        "tree": git("rev-parse", f"{BASE}^{{tree}}").decode().strip(),
        "files": records,
        "source_tar_sha256": hashlib.sha256((STAGE / "source.tar").read_bytes()).hexdigest(),
        "manifest_sha256": hashlib.sha256((STAGE / "source-files.sha256").read_bytes()).hexdigest(),
        "scope": "three exact committed Git blobs; no working-tree source; no mock",
    }
    (STAGE / "source-export.json").write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
