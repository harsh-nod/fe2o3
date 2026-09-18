#!/usr/bin/env python3
"""Remove only this campaign's exact owned directory after its processes exit."""

import json
import os
from pathlib import Path
import shutil
import time

OWNED = Path("/tmp/fe2o3-kfd-matched-9b9265c69-20260918.EOQ4SkyD")
MARKER = "fe2o3-kfd-matched-9b9265c6919cb8dff9506f2c6ffa7b7f2538905f\n"


def scan():
    references = []
    for process in Path("/proc").iterdir():
        if not process.name.isdecimal() or int(process.name) == os.getpid():
            continue
        try:
            if process.stat().st_uid != os.getuid():
                continue
            entries = [
                process / "exe",
                process / "cwd",
                *list((process / "fd").iterdir()),
            ]
            for entry in entries:
                try:
                    target = os.readlink(entry)
                except FileNotFoundError:
                    continue
                if target == str(OWNED) or target.startswith(str(OWNED) + "/"):
                    references.append(
                        {"pid": int(process.name), "kind": entry.name, "target": target}
                    )
        except (FileNotFoundError, ProcessLookupError):
            continue
    return references


def main():
    os.chdir("/tmp")
    assert OWNED.is_dir() and not OWNED.is_symlink() and OWNED.resolve() == OWNED
    assert (
        OWNED.stat().st_uid == os.getuid() and (OWNED / "owner").read_text() == MARKER
    )
    references = scan()
    print(
        json.dumps(
            {
                "record": "before-cleanup",
                "utc_ns": time.time_ns(),
                "owned": str(OWNED),
                "references": references,
            }
        ),
        flush=True,
    )
    assert not references
    size = sum(
        item.stat().st_size
        for item in OWNED.rglob("*")
        if item.is_file() and not item.is_symlink()
    )
    shutil.rmtree(OWNED)
    assert not OWNED.exists()
    references = scan()
    print(
        json.dumps(
            {
                "record": "after-cleanup",
                "utc_ns": time.time_ns(),
                "owned": str(OWNED),
                "removed_regular_bytes": size,
                "absent": True,
                "references": references,
            }
        ),
        flush=True,
    )
    assert not references


if __name__ == "__main__":
    main()
