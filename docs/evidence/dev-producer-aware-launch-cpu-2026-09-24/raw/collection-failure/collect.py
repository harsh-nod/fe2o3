#!/usr/bin/env python3
"""Retain a completed CPU campaign before removing its exact owned Cargo cache."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import hashlib
import os
from pathlib import Path
import shutil
import stat
from types import ModuleType

HERE = Path(__file__).resolve().parent
VERIFY_SHA = "eb855d4cddba181c3edbe926e344a2b5dc671d2d9baf4a1542f4150c1d3e1d24"
VERIFY = HERE / "verify.py"
if not stat.S_ISREG(VERIFY.lstat().st_mode):
    raise RuntimeError("ordinary verifier required")
VERIFIER = VERIFY.read_bytes()
if hashlib.sha256(VERIFIER).hexdigest() != VERIFY_SHA:
    raise RuntimeError("verifier identity mismatch")
V = ModuleType("producer_launch_verify")
V.__file__ = str(VERIFY)
exec(compile(VERIFIER, str(VERIFY), "exec"), V.__dict__)
R = V.R


def cache_bytes(target):
    device = target.lstat().st_dev
    total = 0
    for path in (target, *target.rglob("*")):
        metadata = path.lstat()
        V.need((stat.S_ISREG(metadata.st_mode) or stat.S_ISDIR(metadata.st_mode))
               and metadata.st_uid == os.getuid() and metadata.st_dev == device and not path.is_mount(),
               "ordinary same-device owned cache entry")
        total += metadata.st_blocks * 512
    return total


def owned_iterations(raw):
    inventory = V.iteration_inventory(raw)
    V.need({path.name for path in raw.iterdir()} == {"iterations"}, "only retained development iterations")
    for path in (raw, raw / "iterations", *(raw / "iterations").iterdir()):
        V.need(path.stat().st_uid == os.getuid() and not path.is_mount(), "owned development evidence")
    return inventory


def remove_cache(target, receipt_path, allocated, write_json):
    cleanup = {"path": str(target), "allocated_bytes": allocated, "absent": False}
    write_json(receipt_path, cleanup)
    try:
        shutil.rmtree(target)
    except BaseException as primary:
        cleanup["absent"] = not target.exists() and not target.is_symlink()
        try:
            write_json(receipt_path, cleanup)
        except BaseException as secondary:
            raise primary from secondary
        raise
    cleanup["absent"] = not target.exists() and not target.is_symlink()
    write_json(receipt_path, cleanup)
    V.need(cleanup["absent"], "owned cache remains")
    return cleanup


def main():
    raw = HERE / "raw"
    initial = owned_iterations(raw)
    c = R.helpers()
    source = R.PRIVATE / "campaign1"
    target = R.PRIVATE / "target"
    for path in (R.PRIVATE, source, target):
        V.need(path.is_dir() and not path.is_symlink() and path.resolve() == path
               and path.stat().st_uid == os.getuid(), "exact owned directory")
    before = V.read(source / "inputs-before.json")
    V.need(before == V.read(source / "inputs-after.json") == R.inputs(c), "terminal unchanged source bracket")
    V.validate_campaign(source, retained=False)
    stages = list(R.stages())
    commands = source / "commands"
    V.need({path.name for path in commands.iterdir()} == {name for name, _, _ in stages}, "complete command roster")
    previous = 0
    for name, command, seconds in stages:
        folder = commands / name
        row = V.read(folder / "receipt.json")
        previous = V.receipt(row, command, seconds, previous)
        V.need(not c.B.group_exists(row["pid"]), "owned command process group remains")
        for stream in ("stdout", "stderr"):
            V.need(R.sha(folder / stream) == row[stream + "_sha256"], "terminal output identity")
    V.need({path.name for path in source.iterdir()} == {"commands", "inputs-before.json", "inputs-after.json"},
           "exact terminal campaign files")
    allocated = cache_bytes(target)
    log = R.PRIVATE / "campaign1.log"
    V.need(stat.S_ISREG(log.lstat().st_mode) and log.stat().st_uid == os.getuid()
           and log.resolve() == log, "ordinary owned campaign log")
    log_sha = R.sha(log)
    files = V.inventory(source)
    destination = raw / "campaign1"
    shutil.copytree(source, destination)
    V.need(V.inventory(destination) == files == V.inventory(source), "byte-exact campaign collection")
    shutil.copy2(HERE / "runner.py", destination / "runner.py")
    V.need(R.sha(destination / "runner.py") == before["runner"], "frozen runner collection")
    shutil.copy2(log, raw / "campaign1.log")
    V.need(R.sha(raw / "campaign1.log") == log_sha == R.sha(log), "byte-exact campaign log")
    V.validate_campaign(destination, retained=True)
    expected = initial | {"campaign1/" + name: digest for name, digest in files.items()}
    expected |= {"campaign1/runner.py": before["runner"], "campaign1.log": log_sha}
    V.need(V.inventory(raw) == expected, "complete collected evidence before removal")
    V.iteration_inventory(raw)

    V.need(cache_bytes(target) == allocated, "unchanged owned cache before removal")
    cleanup = remove_cache(target, raw / "cleanup.json", allocated, c.B.write_json)
    c.B.write_json(HERE / "artifacts.json", V.inventory(raw))
    print({"retained_campaign_files": len(files), "removed_bytes": cleanup["allocated_bytes"]})


if __name__ == "__main__":
    main()
