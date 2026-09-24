#!/usr/bin/env python3
"""Collect native1 evidence byte-exactly before deleting its owned build state."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat

HERE = Path(__file__).resolve().parent
OWNED = Path("/home/harsh/.codex-tmp/fe2o3-hot-batch-20260924-feYb12D3/native1")
TRANSIENT = {"source", "target", "build-source.tar.gz"}
COMMIT = "8dc128357ecd55e1ba4f2866eb075899481f9aa0"


def need(value, message):
    if not value:
        raise RuntimeError(message)


def sha(path):
    need(path.is_file() and not path.is_symlink(), "ordinary input file")
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read(path):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            need(key not in result, "duplicate JSON key")
            result[key] = value
        return result
    need(path.is_file() and not path.is_symlink(), "ordinary JSON input")
    return json.loads(path.read_text(), object_pairs_hook=unique)


def inventory(root, *, exclude=frozenset()):
    result = {}
    for entry in root.iterdir():
        if entry.name in exclude:
            continue
        paths = [entry, *entry.rglob("*")] if entry.is_dir() and not entry.is_symlink() else [entry]
        for path in paths:
            need(not path.is_symlink(), "symlink in retained evidence")
            if path.is_dir():
                continue
            need(path.is_file(), "ordinary evidence")
            result[str(path.relative_to(root))] = sha(path)
    return result


def collect(source, destination):
    before = inventory(source, exclude=TRANSIENT)
    destination.mkdir()
    for name in sorted({Path(name).parts[0] for name in before}):
        path = source / name
        if path.is_dir():
            shutil.copytree(path, destination / name)
        else:
            shutil.copy2(path, destination / name)
    need(inventory(destination) == before == inventory(source, exclude=TRANSIENT), "byte-exact retained collection")
    return before


def cleanup_paths(root):
    need(root.is_dir() and not root.is_symlink() and root.resolve() == root
         and root.stat().st_uid == os.getuid(), "exact owned local directory")
    marker = read(root / "owner.json")
    need(type(marker) is dict and set(marker) == {"path", "commit", "binding_sha256"}, "exact owner marker")
    need(marker["commit"] == COMMIT and type(marker["path"]) is str
         and re.fullmatch(r"/home/harsh/fe2o3-hot-batch-20260924\.[0-9a-f]{16}", marker["path"]) is not None,
         "canonical campaign owner")
    need(type(marker["binding_sha256"]) is str and sha(root / "binding.json") == marker["binding_sha256"],
         "authenticated cleanup binding")
    binding = read(root / "binding.json")
    need(binding["commit"] == COMMIT, "cleanup source commit")
    need(inventory(root / "source") == binding["local_source_files"], "unchanged cleanup source")
    need(sha(root / "build-source.tar.gz") == read(root / "source-archive.json")["sha256"], "unchanged cleanup archive")
    rows = []
    for name in ("source", "target", "build-source.tar.gz"):
        path = root / name
        mode = path.lstat().st_mode
        need(not path.is_symlink() and path.resolve() == path, "ordinary owned cleanup path")
        need(stat.S_ISREG(mode) if name.endswith(".gz") else stat.S_ISDIR(mode), "cleanup type")
        entries = [path, *path.rglob("*")] if path.is_dir() else [path]
        rows.append({"path": str(path), "allocated_bytes": sum(p.lstat().st_blocks * 512 for p in entries), "absent": False})
    return rows


def remove_owned(rows, receipt):
    def save():
        receipt.write_text(json.dumps(rows, indent=2) + "\n")
    need(not receipt.exists(), "new cleanup receipt")
    save()
    for row in rows:
        path = Path(row["path"])
        try:
            if path.is_dir():
                shutil.rmtree(path)
            else:
                path.unlink()
        finally:
            row["absent"] = not path.exists() and not path.is_symlink()
            save()
        need(row["absent"], "owned cleanup path remains")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--recovered", action="store_true", help="retain a rejected transport cohort after separate recovery")
    args = parser.parse_args()
    need(OWNED.resolve() == OWNED and OWNED.parent.name == "fe2o3-hot-batch-20260924-feYb12D3", "fixed private run root")
    collection = read(OWNED / "collection.json")
    result_root = OWNED / "recovery1" if args.recovered else OWNED
    if args.recovered:
        need(collection["failures"] and collection["owned_cleanup"] is False, "preserved rejected controller")
        recovery = read(result_root / "result.json")
        need(recovery["collected"] is True and recovery["owned_cleanup"] is True and recovery["failures"] == []
             and recovery["campaign_accepted"] is False, "complete scoped recovery")
    else:
        need(collection["failures"] == [] and collection["owned_cleanup"] is True, "complete remote collection/cleanup")
    need(read(result_root / "remote/finished.json")["failures"] == [], "successful native controller")
    folders = [OWNED / "local", result_root / "remote"]
    if args.recovered:
        folders.append(result_root / "commands")
    for folder in folders:
        for path in folder.glob("*/receipt.json"):
            row = read(path)
            expected_exit = 255 if args.recovered and folder == OWNED / "local" and path.parent.name in ("native", "inventory") else 0
            need(type(row["exit"]) is int and row["exit"] == expected_exit and row["error"] is None
                 and row["group_absent"] is True, "completed reaped command")
    need(inventory(result_root / "remote") == read(result_root / "remote-inventory.json"), "complete remote inventory")
    rows = cleanup_paths(OWNED)
    raw = HERE / "raw"
    raw.mkdir()
    copied = collect(OWNED, raw / "native1")
    remove_owned(rows, raw / "local-cleanup.json")
    (HERE / "artifacts.json").write_text(json.dumps(inventory(raw), indent=2, sort_keys=True) + "\n")
    print(json.dumps({"retained_files": len(copied), "removed_bytes": sum(row["allocated_bytes"] for row in rows)}))


if __name__ == "__main__":
    main()
