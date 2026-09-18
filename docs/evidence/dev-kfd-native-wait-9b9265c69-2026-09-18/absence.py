#!/usr/bin/env python3
"""Independent read-only check, executed from stdin after owned cleanup."""

import json
import os
from pathlib import Path
import time

OWNED = "/tmp/fe2o3-kfd-matched-9b9265c69-20260918.EOQ4SkyD"
references = []
for process in Path("/proc").iterdir():
    if not process.name.isdecimal() or int(process.name) == os.getpid():
        continue
    try:
        if process.stat().st_uid != os.getuid():
            continue
        for entry in [
            process / "exe",
            process / "cwd",
            *list((process / "fd").iterdir()),
        ]:
            try:
                target = os.readlink(entry)
            except FileNotFoundError:
                continue
            if target == OWNED or target.startswith(OWNED + "/"):
                references.append(
                    {"pid": int(process.name), "kind": entry.name, "target": target}
                )
    except (FileNotFoundError, ProcessLookupError):
        continue
absent = not os.path.lexists(OWNED)
print(
    json.dumps(
        {
            "record": "independent-absence",
            "utc_ns": time.time_ns(),
            "owned": OWNED,
            "absent": absent,
            "references": references,
        }
    )
)
assert absent and not references
