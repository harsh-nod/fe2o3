#!/usr/bin/env python3
"""Reviewed prelaunch-only cleanup, called with the unchanged helper namespace."""

import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import sys

OWNED = Path("/tmp/fe2o3-r126-primary-8b0021ba-20260918.oVqzCv3c")
INVENTORY = "b987f3468f4b22f4d9f74afea62bb2abd60fcd98637aec7b07e9ca5c000874d0"


def main(helpers):
    need, emit = helpers["need"], helpers["emit"]
    need(sys.argv[1:] in (["cleanup"], ["absence"]), "exact prelaunch mode")
    os.chdir("/tmp")
    if sys.argv[1] == "absence":
        need(not os.path.lexists(OWNED), "prelaunch path still exists")
        helpers["absence"](OWNED, [])
        emit(
            {
                "record": "independent-prelaunch-absence",
                "owned": str(OWNED),
                "absent": True,
                "native_invocations": 0,
                "native_pid_roster": [],
            }
        )
        return
    need(
        OWNED.is_dir() and not OWNED.is_symlink() and OWNED.resolve() == OWNED,
        "exact ordinary prelaunch directory",
    )
    need(OWNED.stat().st_uid == os.getuid() == 1002, "observed exact owner UID")
    need(
        stat.S_IMODE(OWNED.stat().st_mode) == 0o755,
        "only the observed scp-propagated mode",
    )
    need(
        json.loads((OWNED / "owner.json").read_text()) == helpers["marker"](OWNED),
        "exact owner marker",
    )
    need(
        not os.path.lexists(OWNED / "approval.json")
        and not os.path.lexists(OWNED / "results"),
        "no approval and no native launch records",
    )
    need(
        helpers["sha"](OWNED / "payload.json") == helpers["PAYLOAD"],
        "frozen payload manifest",
    )
    files = helpers["inventory"](OWNED)
    manifest = json.loads((OWNED / "payload.json").read_text())
    need(
        set(files) == set(manifest) | {"payload.json", "owner.json"},
        "only expected payload and owner files",
    )
    need(
        all(files[path] == digest for path, digest in manifest.items()),
        "all frozen payload bytes",
    )
    digest = hashlib.sha256(
        json.dumps(files, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()
    need(digest == INVENTORY, "all bytes match the fully collected refused directory")
    helpers["absence"](OWNED, [])
    shutil.rmtree(OWNED)
    need(not os.path.lexists(OWNED), "exact prelaunch directory removed")
    emit(
        {
            "record": "exact-prelaunch-directory-removed",
            "owned": str(OWNED),
            "inventory_sha256": digest,
            "regular_files": len(files),
            "native_invocations": 0,
            "native_pid_roster": [],
        }
    )
    helpers["absence"](OWNED, [])
