#!/usr/bin/env python3
"""Close this evidence set after its original and review receipts are terminal."""

import hashlib
from pathlib import Path
import shlex
import subprocess
import sys

HERE = Path(__file__).resolve().parent
EXPECTED = {
    "audit": ["python3", "-B", "audit.py"],
    "python-lint": [
        "ruff",
        "check",
        "--no-cache",
        "audit.py",
        "run.py",
        "check.py",
        "cleanup.py",
        "cleanup-visible.py",
        "absence.py",
        "audit-export.py",
        "freeze-source.py",
        "freeze-scripts.py",
        "test_check.py",
    ],
    "python-format": [
        "ruff",
        "format",
        "--no-cache",
        "--check",
        "audit.py",
        "run.py",
        "check.py",
        "cleanup.py",
        "absence.py",
        "audit-export.py",
        "freeze-scripts.py",
        "test_check.py",
    ],
    "shell-lint": [
        "shellcheck",
        "record.sh",
        "create.sh",
        "prepare.sh",
        "remote-cleanup-visible.sh",
        "remote-absence.sh",
        "review-record.sh",
    ],
}


def main():
    if (HERE / "SHA256SUMS").exists():
        raise RuntimeError("archive already sealed")
    subprocess.run([sys.executable, "-B", str(HERE / "audit.py")], cwd=HERE, check=True)
    import audit

    review = HERE / "review"
    assert {path.name for path in review.iterdir()} == {
        name + "." + suffix for name in EXPECTED for suffix in audit.SUFFIXES
    }
    for name, command in EXPECTED.items():
        assert shlex.split((review / f"{name}.command").read_text()) == command
        assert (review / f"{name}.exit").read_text() == "0\n"
        assert audit.time_value(
            (review / f"{name}.started").read_text()
        ) <= audit.time_value((review / f"{name}.finished").read_text())
    paths = sorted(path for path in HERE.rglob("*") if path.is_file())
    assert all(not path.is_symlink() for path in HERE.rglob("*"))
    assert all(path.read_bytes()[:4] != b"\x7fELF" for path in paths)
    assert not any(
        part in ("__pycache__", ".ruff_cache", "binaries", "source.tar")
        for path in paths
        for part in path.relative_to(HERE).parts
    )
    manifest = "".join(
        f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.relative_to(HERE).as_posix()}\n"
        for path in paths
    )
    with (HERE / "SHA256SUMS").open("x") as output:
        output.write(manifest)
    print(
        f"entries={len(paths)} files={len(paths) + 1} manifest_sha256={hashlib.sha256(manifest.encode()).hexdigest()}"
    )


if __name__ == "__main__":
    main()
