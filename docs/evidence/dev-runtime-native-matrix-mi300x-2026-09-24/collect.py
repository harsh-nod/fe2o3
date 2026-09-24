#!/usr/bin/env python3
"""Retain all terminal attempts and exact build inputs before owned local cleanup."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import time

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("native_matrix_verify", HERE / "verify.py")
V = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(V)


def write(path, value):
    path.write_text(json.dumps(value, sort_keys=True, indent=2) + "\n")


def main():
    started = time.time_ns()
    source, destination = V.PRIVATE, HERE / "raw"
    V.need(source.resolve() == source and source.is_dir() and not source.is_symlink()
           and source.stat().st_uid == os.getuid(), "exact owned private source")
    V.need(not destination.exists() and not (HERE / "artifacts.json").exists(), "fresh immutable retention destination")
    V.need({path.name for path in source.iterdir()} == {"build", "campaign", "campaign2", "campaign3"}, "owned attempt roster")
    paths = [source, *source.rglob("*")]
    V.need(all(not path.is_symlink() and (path.is_dir() or path.is_file()) and path.stat().st_uid == os.getuid()
               for path in paths), "ordinary owned scratch tree")
    for folder in (source / "build", source / "campaign", source / "campaign2", source / "campaign3"):
        for receipt in (folder / "commands").glob("*/receipt.json"):
            row = V.read(receipt)
            V.need(row["group_absent"] is True and type(row["exit"]) is int and row["error"] is None, "terminal local command")
            try:
                os.killpg(row["pid"], 0)
            except ProcessLookupError:
                pass
            else:
                raise RuntimeError("recorded group exists; do not delete its working tree")
    V.verify(source)
    excluded = [source / "build/source", source / "build/target"]
    expected = {path.relative_to(source).as_posix(): V.sha(path) for path in paths
                if path.is_file() and not any(path.is_relative_to(folder) for folder in excluded)}
    destination.mkdir()
    for name in expected:
        target = destination / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source / name, target)
    V.need(V.inventory(destination) == expected, "byte-exact local collection before deletion")
    before = V.read(source / "campaign3/protocol-before.json")
    m = V.module(source / "campaign3/protocol/prepare.py", before["prepare.py"], "native_matrix_build")
    V.need(V.sha(Path(m.SIGNERS)) == m.SIGNERS_SHA, "pinned public signer before collection")
    shutil.copy2(m.SIGNERS, destination / "build/allowed-signers")
    expected["build/allowed-signers"] = m.SIGNERS_SHA
    V.need(V.inventory(destination) == expected, "complete records and public signer")
    replay = V.verify(destination)
    row = {"source": str(source), "excluded": ["build/source", "build/target"], "retained_files": len(expected),
           "retained_sha256": hashlib.sha256(json.dumps(expected, sort_keys=True).encode()).hexdigest(),
           "allocated_bytes": sum(path.stat().st_blocks * 512 for path in paths), "absent": False,
           "replay": replay, "started_ns": started, "finished_ns": None}
    write(destination / "retention.json", row)
    shutil.rmtree(source)
    row.update(absent=not source.exists(), finished_ns=time.time_ns())
    write(destination / "retention.json", row)
    V.need(row["absent"] is True, "owned scratch removal")
    write(HERE / "artifacts.json", V.inventory(destination))
    V.verify_retention(destination)
    print(json.dumps({"retained_files": len(expected), "removed_allocated_bytes": row["allocated_bytes"], "replay": replay}, sort_keys=True))


if __name__ == "__main__":
    main()
