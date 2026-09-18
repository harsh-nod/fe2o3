#!/usr/bin/env python3
"""Read-only inventory of the exact refused prelaunch directory."""

import hashlib
import json
import os
from pathlib import Path
import stat
import time

OWNED = Path("/tmp/fe2o3-r126-primary-8b0021ba-20260918.oVqzCv3c")
COMMIT = "8b0021ba740c337b2641ecd26ed6c9375613ed32"
PAYLOAD = "b69190c2182feb82476d6be2ecd6e30e11d58c3b804e686d7ec338814d1ac4f6"


def need(condition, message):
    if not condition:
        raise RuntimeError(message)


def sha(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


need(
    OWNED.is_dir() and not OWNED.is_symlink() and OWNED.resolve() == OWNED,
    "exact ordinary owned directory",
)
info = OWNED.stat()
need(info.st_uid == os.getuid(), "exact owner UID")
owner = json.loads((OWNED / "owner.json").read_text())
need(
    owner == {"commit": COMMIT, "path": str(OWNED), "payload_sha256": PAYLOAD},
    "exact prelaunch marker",
)
need(
    not (OWNED / "approval.json").exists() and not (OWNED / "results").exists(),
    "no approval or launch artifacts",
)
need(sha(OWNED / "payload.json") == PAYLOAD, "frozen payload manifest")
manifest = json.loads((OWNED / "payload.json").read_text())
files = {}
for path in sorted(OWNED.rglob("*")):
    need(
        not path.is_symlink() and (path.is_file() or path.is_dir()),
        "ordinary nonsymlink tree",
    )
    need(path.stat().st_uid == os.getuid(), "owned tree UID")
    if path.is_file():
        files[path.relative_to(OWNED).as_posix()] = sha(path)
need(
    set(files) == set(manifest) | {"payload.json", "owner.json"},
    "exact payload and owner files only",
)
need(
    all(files[path] == digest for path, digest in manifest.items()),
    "frozen payload bytes unchanged",
)
print(
    json.dumps(
        {
            "record": "refused-prelaunch-inventory",
            "owned": str(OWNED),
            "owner": owner,
            "uid": info.st_uid,
            "gid": info.st_gid,
            "mode": oct(stat.S_IMODE(info.st_mode)),
            "files": files,
            "inventory_sha256": hashlib.sha256(
                json.dumps(files, sort_keys=True, separators=(",", ":")).encode()
            ).hexdigest(),
            "native_approval_present": False,
            "native_results_present": False,
            "utc_ns": time.time_ns(),
        },
        indent=2,
    ),
    flush=True,
)
