#!/usr/bin/env python3
"""Retain terminal CPU attempts before removing only their private Cargo cache."""

import argparse
import hashlib
import importlib.util
import os
from pathlib import Path
import shutil
from types import ModuleType

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("directory_cpu", HERE / "runner.py")
R = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(R)
C = R.helpers()
PACKAGE = R.REPO / "docs/evidence/dev-xgmi-hot-batch-mi300x-2026-09-24/package.py"
RAW = PACKAGE.read_bytes()
if PACKAGE.is_symlink() or hashlib.sha256(RAW).hexdigest() != "c83d5a05fb8baf8904eb4a9f49c8d5f07906762aa7ea0cfc5c07ad9e51583466":
    raise RuntimeError("authenticated collection helper")
P = ModuleType("directory_collect_helpers")
P.__file__ = str(PACKAGE)
exec(compile(RAW, str(PACKAGE), "exec"), P.__dict__)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("attempt", choices=("cpu1", "cpu2"))
    args = parser.parse_args()
    source = R.PRIVATE / args.attempt
    P.need(source.is_dir() and not source.is_symlink() and source.resolve() == source
           and source.stat().st_uid == os.getuid(), "owned CPU source")
    before = P.read(source / "inputs-before.json")
    P.need(before == P.read(source / "inputs-after.json"), "terminal unchanged source bracket")
    commands = list(R.stages())
    if args.attempt == "cpu1":
        commands = commands[:4]
    P.need({path.name for path in (source / "commands").iterdir()} == {row[0] for row in commands}, "terminal command roster")
    for name, command, seconds in commands:
        folder = source / "commands" / name
        row = P.read(folder / "receipt.json")
        interrupted = args.attempt == "cpu1" and name == "gnu-focused"
        P.need(row["command"] == command and row["cwd"] == str(R.REPO)
               and row["timeout_seconds"] == seconds and row["group_absent"] is True,
               "exact terminal CPU command")
        P.need(type(row["exit"]) is int and row["exit"] == (-15 if interrupted else 0)
               and row["error"] == ("RuntimeError: interrupted by signal 15" if interrupted else None),
               "expected CPU attempt outcome")
        P.need(not C.B.group_exists(row["pid"]), "owned command group remains")
        for stream in ("stdout", "stderr"):
            P.need(P.sha(folder / stream) == row[stream + "_sha256"], "terminal stream hash")
    if args.attempt == "cpu2":
        P.need(before == R.inputs(C), "current accepted source inputs")
        P.need(not (source / "runner.py").exists(), "new runner snapshot")
        shutil.copy2(HERE / "runner.py", source / "runner.py")
    P.need(P.sha(source / "runner.py") == before["runner"], "exact attempt runner snapshot")
    P.need({path.name for path in source.iterdir()} == {"commands", "inputs-before.json", "inputs-after.json",
           "prior-delta.json", "runner.py", "target"}, "owned attempt file roster")
    target = source / "target"
    P.need(target.is_dir() and not target.is_symlink() and target.resolve() == target, "exact ordinary Cargo cache")
    paths = [target, *target.rglob("*")]
    P.need(not any(path.is_symlink() for path in paths), "no cache symlinks")
    rows = [{"path": str(target), "allocated_bytes": sum(path.stat().st_blocks * 512 for path in paths), "absent": False}]
    raw = HERE / "raw"
    raw.mkdir(exist_ok=True)
    manifest = HERE / "artifacts.json"
    if manifest.exists():
        P.need(P.read(manifest) == P.inventory(raw), "unchanged previously retained attempts")
    files = P.collect(source, raw / args.attempt)
    P.remove_owned(rows, raw / (args.attempt + "-cleanup.json"))
    pending = HERE / "artifacts.next.json"
    C.B.write_json(pending, P.inventory(raw))
    pending.replace(manifest)
    print({"attempt": args.attempt, "retained_files": len(files), "removed_bytes": rows[0]["allocated_bytes"]})


if __name__ == "__main__":
    main()
