#!/usr/bin/env python3
"""Collect an interrupted owned run, then remove only its proven resources."""

import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
SOURCE_PREFIX = "docs/evidence/dev-xgmi-aggregate-attribution-mi300x-2026-09-19/"
COMMIT = "f1fb5f4877bf888b3346318cb9d53e2757e7930b"
SIGNERS = "/home/harsh/.codex-tmp/fe2o3-logical-mux-sdma-protocol-v2-20260918/allowed-signers"
SIGNERS_SHA = "ea67b34912b272ccbb46a4a83d8e00eb1a83e87832f089c1c134eed1786b560b"


def need(value, message):
    if not value:
        raise RuntimeError(message)


def read(path):
    return json.loads(path.read_bytes())


def main():
    need(sys.argv[1:] == ["collect-and-clean"], "explicit recovery command required")
    need(not (HERE / "recovery").exists(), "never overwrite recovery evidence")
    need(not Path(SIGNERS).is_symlink(), "ordinary signer list")
    need(hashlib.sha256(Path(SIGNERS).read_bytes()).hexdigest() == SIGNERS_SHA, "pinned signers")
    subprocess.run(["git", "-c", "gpg.ssh.allowedSignersFile=" + SIGNERS,
                    "verify-commit", COMMIT], cwd=ROOT, check=True)
    for name in ("campaign.py", "native.py", "results.py", "test_campaign.py",
                 "test_native.py", "test_results.py", "verify.py"):
        path = HERE / name
        need(path.is_file() and not path.is_symlink(), "ordinary signed tool")
        need(subprocess.check_output(["git", "show", COMMIT + ":" + SOURCE_PREFIX + name],
                                     cwd=ROOT) == path.read_bytes(), "signed tool bytes: " + name)
    spec = importlib.util.spec_from_file_location("owned_aggregate_recovery", HERE / "campaign.py")
    C = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(C)
    marker = read(HERE / "owner.json")
    need(marker["commit"] == COMMIT, "original source commit")
    need(C.sha(HERE / "binding.json") == marker["binding_sha256"], "original binding")
    owned = C.B.owned_path(marker, exists=False)
    out = HERE / "recovery"
    out.mkdir()
    rec = C.B.Recorder(out / "raw", ROOT)
    inventory_folder = rec.run("inventory", C.control_spec("inventory", marker), 120,
                               stdin=C.control_bytes())
    inventory = read(inventory_folder / "stdout")
    C.B.write_json(out / "inventory.json", inventory)
    rec.run("collect", ["scp", *C.SSH, "-r", "mi300x:" + str(owned / "results"),
                        str(out / "remote")], 300)
    need(C.B.inventory(out / "remote") == inventory, "complete byte-exact recovery")
    cleanup = rec.run("cleanup", C.control_spec("cleanup", marker), 120,
                      stdin=C.control_bytes())
    need(read(cleanup / "stdout") == {"removed": str(owned)}, "exact owned deletion")
    absence = rec.run("absence", C.control_spec("absence", marker), 120,
                      stdin=C.control_bytes())
    need(read(absence / "stdout") == {"path_absent": True, "processes_absent": True},
         "owned remote resources absent")
    payload = Path(read(HERE / "local-payload.json")["path"])
    need(payload.parent == ROOT.parent and re.fullmatch(r"fe2o3-peer-hot-payload-[a-z0-9_]+",
                                                       payload.name), "owned local payload path")
    need(payload.is_dir() and not payload.is_symlink(), "ordinary payload directory")
    need({path.name for path in payload.iterdir()} == {*C.N.PAYLOAD, "binding.json"},
         "exact local payload roster")
    for name in (*C.N.PAYLOAD, "binding.json"):
        path = payload / name
        need(path.is_file() and not path.is_symlink(), "ordinary payload member")
    need(C.sha(payload / "binding.json") == marker["binding_sha256"], "owned local binding")
    for name, digest in read(payload / "binding.json")["payload"].items():
        need(C.sha(payload / name) == digest, "owned payload digest")
    shutil.rmtree(payload)
    need(not payload.exists() and not payload.is_symlink(), "local payload absent")
    C.B.write_json(out / "status.json", {
        "source_commit": COMMIT, "remote_path": str(owned), "local_payload": str(payload),
        "remote_removed": True, "owned_processes_absent": True, "local_payload_removed": True,
        "collection_complete": True, "original_campaign_success": False,
        "performance_acceptance": False, "formal_refinement": False,
    })
    print("Interrupted evidence collected; owned remote and local resources absent.")


if __name__ == "__main__":
    main()
