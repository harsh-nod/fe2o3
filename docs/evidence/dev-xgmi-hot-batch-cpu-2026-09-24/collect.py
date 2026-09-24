#!/usr/bin/env python3
"""Collect this owned CPU run, then remove only its private build artifacts."""

import hashlib
import json
from pathlib import Path
import shutil
import stat
import subprocess


ROOT = Path(__file__).resolve().parent
OWNED = Path("/home/harsh/.codex-tmp/fe2o3-hot-batch-20260924-feYb12D3")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    source = OWNED / "cpu2"
    before = (source / "inputs-before.json").read_bytes()
    if before != (source / "inputs-after.json").read_bytes():
        raise ValueError("source bracket changed")
    records = list(source.glob("*/record.json"))
    if len(records) != 21:
        raise ValueError("incomplete campaign")
    for path in records:
        row = json.loads(path.read_text())
        if row["status"] != 0 or row["group_absent"] is not True:
            raise ValueError("failed or unreaped command")
    destination = ROOT / "raw/cpu2"
    shutil.copytree(source, destination, ignore=shutil.ignore_patterns("target"))
    original = {str(path.relative_to(source)): digest(path) for path in source.rglob("*")
                if path.relative_to(source).parts[0] != "target" and path.is_file()}
    copied = {str(path.relative_to(destination)): digest(path) for path in destination.rglob("*") if path.is_file()}
    if original != copied:
        raise ValueError("collection roster or bytes differ")
    rejected = OWNED / "cpu1"
    rejected_destination = ROOT / "raw/rejected-cpu1"
    shutil.copytree(rejected, rejected_destination, ignore=shutil.ignore_patterns("target"))
    original = {str(path.relative_to(rejected)): digest(path) for path in rejected.rglob("*")
                if path.relative_to(rejected).parts[0] != "target" and path.is_file()}
    copied = {str(path.relative_to(rejected_destination)): digest(path)
              for path in rejected_destination.rglob("*") if path.is_file()}
    if original != copied:
        raise ValueError("rejected collection roster or bytes differ")
    cleanup = []
    for path in (source / "target", rejected / "target", OWNED / "hsa-callbacks", OWNED / "hip-callbacks"):
        if path.is_symlink() or path.resolve() != path or not path.exists():
            raise ValueError("unexpected owned cleanup path")
        mode = path.stat().st_mode
        if not (stat.S_ISDIR(mode) if path.name == "target" else stat.S_ISREG(mode)):
            raise ValueError("unexpected owned cleanup type")
        size = int(subprocess.check_output(["/usr/bin/du", "-s", "-B1", str(path)]).split()[0])
        cleanup.append({"path": str(path), "allocated_bytes": size, "absent": False})
    cleanup_record = ROOT / "raw/cleanup.json"
    cleanup_record.write_text(json.dumps(cleanup, indent=2) + "\n")
    for row in cleanup:
        path = Path(row["path"])
        try:
            if path.name == "target":
                shutil.rmtree(path)
            else:
                path.unlink()
            row["absent"] = not path.exists() and not path.is_symlink()
            if not row["absent"]:
                raise ValueError("owned cleanup failed")
        finally:
            cleanup_record.write_text(json.dumps(cleanup, indent=2) + "\n")
    manifest = {str(path.relative_to(ROOT)): digest(path)
                for path in sorted((ROOT / "raw").rglob("*")) if path.is_file()}
    (ROOT / "artifacts.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(json.dumps({"artifacts": len(manifest), "removed_bytes": sum(row["allocated_bytes"] for row in cleanup)}))


if __name__ == "__main__":
    main()
