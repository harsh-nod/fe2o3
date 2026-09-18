#!/usr/bin/env python3
"""Seal the reviewed closed archive without rewriting any original artifact."""

import hashlib
from pathlib import Path
import re
import shlex
import stat
import subprocess
import sys

ROOT = Path(__file__).resolve().parent
REVIEW = {
    "format": ["ruff", "format", "--no-cache", "audit.py", "capture-origin.py"],
    "capture-origin": ["python3", "-B", "capture-origin.py"],
    "audit": ["python3", "-B", "audit.py"],
    "lint": ["ruff", "check", "--no-cache", "audit.py", "capture-origin.py"],
    "format-check": [
        "ruff",
        "format",
        "--no-cache",
        "--check",
        "audit.py",
        "capture-origin.py",
    ],
    "shell-syntax": ["bash", "-n", "review-record.sh"],
    "shellcheck": ["shellcheck", "review-record.sh"],
    "independent-root-audit": ["python3", "-B", "audit.py"],
}


def require(value, message):
    if not value:
        raise RuntimeError(message)


def members():
    files = []
    for path in sorted(ROOT.rglob("*")):
        mode = path.lstat().st_mode
        require(
            stat.S_ISDIR(mode) or stat.S_ISREG(mode), "no symlinks or special files"
        )
        require(
            not ({"__pycache__", ".ruff_cache"} & set(path.relative_to(ROOT).parts)),
            "no caches",
        )
        if stat.S_ISREG(mode) and path != ROOT / "SHA256SUMS":
            with path.open("rb") as source:
                require(source.read(4) != b"\x7fELF", "no ELF artifacts")
            require(path.name != "source.tar", "no exported source tar")
            files.append(path)
    return files


def main():
    manifest = ROOT / "SHA256SUMS"
    require(not manifest.exists(), "archive already sealed")
    suffixes = ("command", "started", "finished", "exit", "stdout", "stderr")
    require(
        {path.name for path in (ROOT / "review").iterdir()}
        == {name + "." + suffix for name in REVIEW for suffix in suffixes},
        "complete exact eight-review roster",
    )
    previous = ""
    for name, command in REVIEW.items():
        base = ROOT / "review" / name
        require(
            shlex.split(base.with_suffix(".command").read_text()) == command,
            "exact review command",
        )
        require(
            base.with_suffix(".exit").read_text() == "0\n", "successful closed review"
        )
        start, finish = (
            base.with_suffix("." + field).read_text()
            for field in ("started", "finished")
        )
        require(
            all(
                re.fullmatch(r"2026-09-18T\d{2}:\d{2}:\d{2}\.\d{9}Z\n", value)
                for value in (start, finish)
            ),
            "canonical review timestamp",
        )
        require(previous <= start < finish, "ordered review closure")
        previous = finish
    subprocess.run([sys.executable, "-B", "audit.py"], cwd=ROOT, check=True)
    files = members()
    body = "".join(
        f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.relative_to(ROOT)}\n"
        for path in files
    )
    with manifest.open("x") as output:
        output.write(body)
    require(members() == files, "unchanged closed membership")
    require(
        body
        == "".join(
            f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.relative_to(ROOT)}\n"
            for path in files
        ),
        "unchanged sealed bytes",
    )
    subprocess.run(
        ["sha256sum", "-c", "SHA256SUMS"],
        cwd=ROOT,
        stdout=subprocess.DEVNULL,
        check=True,
    )
    print(f"manifest_entries={len(files)} total_files={len(files) + 1}")
    print(f"manifest_sha256={hashlib.sha256(manifest.read_bytes()).hexdigest()}")


if __name__ == "__main__":
    main()
