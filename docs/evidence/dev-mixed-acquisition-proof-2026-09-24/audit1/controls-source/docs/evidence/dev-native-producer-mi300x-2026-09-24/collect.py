#!/usr/bin/env python3
"""Retain all terminal attempts and exact build inputs before owned local cleanup."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import time
from types import ModuleType

HERE = Path(__file__).resolve().parent
VERIFY_SHA = "017830908daa2a9383f67f5038606434f890635249f82fa8ddbe602dfa4598b9"


def load_verifier(path):
    if not stat.S_ISREG(path.lstat().st_mode):
        raise RuntimeError("ordinary pinned verifier")
    raw = path.read_bytes()
    if hashlib.sha256(raw).hexdigest() != VERIFY_SHA:
        raise RuntimeError("pinned verifier identity")
    module = ModuleType("native_producer_collector_verifier")
    module.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), module.__dict__)
    return module


V = load_verifier(HERE / "verify.py")


def owned_snapshot(source):
    V.need(source.resolve() == source and source.is_dir() and not source.is_symlink(), "ordinary resolved owned root")
    device = source.stat().st_dev
    result = {}
    for path in (source, *sorted(source.rglob("*"))):
        meta = path.lstat()
        V.need((stat.S_ISREG(meta.st_mode) or stat.S_ISDIR(meta.st_mode))
               and meta.st_uid == os.getuid() and meta.st_dev == device and not path.is_mount(), "ordinary same-device owned entry")
        result[path.relative_to(source).as_posix()] = (
            meta.st_mode, meta.st_uid, meta.st_dev, meta.st_ino, meta.st_size,
            V.sha(path) if stat.S_ISREG(meta.st_mode) else None,
        )
    return result


def recheck_before_removal(source, expected):
    V.need(owned_snapshot(source) == expected, "complete owned source unchanged before deletion")
    for folder in (source / "build", source / "campaign1"):
        for receipt in (folder / "commands").glob("*/receipt.json"):
            row = V.read(receipt)
            try:
                os.killpg(row["pid"], 0)
            except ProcessLookupError:
                pass
            else:
                raise RuntimeError("recorded command group exists before deletion")


def remove_owned(source, expected):
    recheck_before_removal(source, expected)
    shutil.rmtree(source)


def write(path, value):
    with path.open("x") as output:
        output.write(json.dumps(value, sort_keys=True, indent=2) + "\n")


def main():
    started = time.time_ns()
    source, destination = V.PRIVATE, HERE / "raw"
    V.need(source.resolve() == source and source.is_dir() and not source.is_symlink()
           and source.stat().st_uid == os.getuid(), "exact owned private source")
    V.need(not destination.exists() and not (HERE / "artifacts.json").exists(), "fresh immutable retention destination")
    V.need({path.name for path in source.iterdir()} == {"build", "campaign1"}, "owned attempt roster")
    initial = owned_snapshot(source)
    paths = [source, *source.rglob("*")]
    for folder, count in ((source / "build", 11), (source / "campaign1", 8)):
        V.need(len(list((folder / "commands").iterdir())) == count, "complete terminal command roster")
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
    before = V.read(source / "campaign1/protocol-before.json")
    m = V.module(source / "campaign1/protocol/prepare.py", before["prepare.py"], "native_producer_build")
    V.need(V.sha(Path(m.SIGNERS)) == m.SIGNERS_SHA, "pinned public signer before collection")
    shutil.copy2(m.SIGNERS, destination / "build/allowed-signers")
    expected["build/allowed-signers"] = m.SIGNERS_SHA
    V.need(V.inventory(destination) == expected, "complete records and public signer")
    replay = V.verify(destination)
    row = {"source": str(source), "excluded": ["build/source", "build/target"], "retained_files": len(expected),
           "retained_sha256": hashlib.sha256(json.dumps(expected, sort_keys=True).encode()).hexdigest(),
           "allocated_bytes": sum(path.stat().st_blocks * 512 for path in paths), "absent": False,
           "replay": replay, "started_ns": started, "finished_ns": None}
    write(destination / "retention-before.json", row)
    remove_owned(source, initial)
    row.update(absent=not source.exists(), finished_ns=time.time_ns())
    write(destination / "retention.json", row)
    V.need(row["absent"] is True, "owned scratch removal")
    write(HERE / "artifacts.json", V.inventory(destination))
    V.verify_retention(destination)
    print(json.dumps({"retained_files": len(expected), "removed_allocated_bytes": row["allocated_bytes"], "replay": replay}, sort_keys=True))


if __name__ == "__main__":
    main()
