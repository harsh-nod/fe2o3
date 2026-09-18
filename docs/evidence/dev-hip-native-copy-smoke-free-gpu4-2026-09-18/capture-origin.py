#!/usr/bin/env python3
"""Bind archived original artifacts to the closed fresh staging directory."""

import hashlib
from pathlib import Path

ORIGIN = Path("/home/harsh/.codex-tmp/hip-smoke-1890a64e1-new-20260918.STshT3l4")
HERE = Path(__file__).resolve().parent
rows = []
for source in sorted(ORIGIN.rglob("*")):
    relative = source.relative_to(ORIGIN)
    if set(relative.parts) & {".ruff_cache", "__pycache__"} or relative == Path(
        "source.tar"
    ):
        continue
    assert not source.is_symlink()
    if not source.is_file():
        continue
    original = source.read_bytes()
    assert (HERE / relative).read_bytes() == original, str(relative)
    rows.append(f"{hashlib.sha256(original).hexdigest()}  {relative}\n")
assert rows
(HERE / "origin-files.sha256").write_text("".join(rows))
print(
    f"original_files_byte_identical={len(rows)} excluded=source.tar,.ruff_cache,__pycache__"
)
