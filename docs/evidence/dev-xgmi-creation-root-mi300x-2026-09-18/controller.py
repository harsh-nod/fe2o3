#!/usr/bin/env python3
"""Freeze, build, observe, collect and clean one two-GPU development run."""

import json
from pathlib import Path
import secrets
import re
import shlex
import shutil
import signal
import subprocess
import sys
import tarfile
import tempfile

sys.dont_write_bytecode = True
import native as N  # noqa: E402

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
CPU = ROOT / "docs/evidence/dev-xgmi-creation-root-cpu-2026-09-18"
SELECTOR = ROOT / "docs/evidence/dev-sdma-direct-readback-cpu-2026-09-18/source.py"
SIGNERS = (
    "/home/harsh/.codex-tmp/fe2o3-logical-mux-sdma-protocol-v2-20260918/allowed-signers"
)
SSH = [
    "-o",
    "BatchMode=yes",
    "-o",
    "ConnectTimeout=10",
    "-o",
    "ServerAliveInterval=10",
    "-o",
    "ServerAliveCountMax=3",
]


def collect_and_clean(rec, remote, marker, state):
    collected = remote("remote-inventory", "inventory")
    manifest = json.loads((collected / "stdout").read_text())
    N.need(manifest, "nonempty result inventory")
    N.write_json(HERE / "remote-inventory.json", manifest)
    rec.run(
        "collect",
        [
            "scp",
            "-r",
            *SSH,
            "mi300x:" + marker["path"] + "/results",
            str(HERE / "remote"),
        ],
        300,
    )
    N.need(
        N.inventory(HERE / "remote") == manifest,
        "complete collection matches remote digests",
    )
    state["collected"] = True
    remote("cleanup", "cleanup")
    state["cleaned"] = True
    remote("absence", "absence")
    state["absence"] = True


def main():
    for number in N.MANAGED:
        signal.signal(number, N.interrupted)
    rec = N.Recorder(HERE / "local", ROOT)
    rec.run("calibration", ["python3", "-B", str(HERE / "test_native.py")], 60)
    rec.run(
        "observer-tests",
        ["python3", "-B", "benchmarks/runtime_gfx942/test_copy_host_observe.py"],
        60,
    )
    rec.run("cpu-verify", ["python3", "-B", str(CPU / "verify.py"), "--live"], 60)
    commit = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
    ).strip()
    N.need(re.fullmatch(r"[0-9a-f]{40}", commit), "canonical source commit")
    rec.run(
        "signature",
        ["git", "-c", "gpg.ssh.allowedSignersFile=" + SIGNERS, "verify-commit", commit],
        30,
    )
    snapshot = json.loads(
        subprocess.check_output(["python3", "-I", str(SELECTOR)], cwd=ROOT)
    )
    N.need(
        snapshot["base"] == commit
        and snapshot["files"]
        == json.loads((CPU / "raw/source-before.log").read_text())["files"],
        "qualified source cohort",
    )
    paths = [
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        ".cargo",
        "crates",
        "examples",
        "benchmarks/runtime_gfx942",
        "scripts/unsafe-source-baseline.json",
        "docs/runtime-primary-queue-release-v1.md",
    ]
    clean = rec.run(
        "source-clean",
        ["git", "status", "--porcelain=v1", "--untracked-files=all", "--", *paths],
        30,
    )
    N.need(
        (clean / "stdout").read_bytes() == b"",
        "source committed without untracked inputs",
    )
    payload = Path(
        tempfile.mkdtemp(
            prefix="fe2o3-xgmi-native-20260918.", dir="/home/harsh/.codex-tmp"
        )
    )
    native_bytes = (HERE / "native.py").read_bytes()
    state = {
        "created": False,
        "native_success": False,
        "collected": False,
        "cleaned": False,
        "absence": False,
        "local_payload": str(payload),
        "failure": None,
        "native_failure": None,
    }
    marker = None
    try:
        shutil.copy2(HERE / "native.py", payload / "native.py")
        with tarfile.open(payload / "source.tar.gz", "w:gz") as archive:
            for name, digest in snapshot["files"].items():
                path = ROOT / name
                N.need(
                    path.is_file() and not path.is_symlink() and N.sha(path) == digest,
                    "ordinary qualified source",
                )
                archive.add(path, arcname=name, recursive=False)
        binding = {
            "schema": "fe2o3.xgmi-creation-native-development.v1",
            "commit": commit,
            "cpu_seal_sha256": N.sha(CPU / "SHA256SUMS"),
            "source_files": snapshot["files"],
            "payload": {
                name: N.sha(payload / name) for name in ("native.py", "source.tar.gz")
            },
            "local_tools": {
                name: N.sha(HERE / name)
                for name in (
                    "controller.py",
                    "native.py",
                    "test_native.py",
                    "verify.py",
                )
            },
            "devices": N.DEVICES,
            "bytes": 1048576,
            "depths": [1, 16],
            "warmups": 10,
            "samples": 30,
            "performance_acceptance": False,
            "formal_refinement": False,
        }
        N.write_json(payload / "binding.json", binding)
        N.write_json(HERE / "binding.json", binding)
        marker = {
            "commit": commit,
            "path": N.PREFIX + secrets.token_hex(8),
            "binding_sha256": N.sha(payload / "binding.json"),
        }
        N.write_json(HERE / "owner.json", marker)
        serialized = json.dumps(marker, sort_keys=True, separators=(",", ":"))

        def remote(name, mode, seconds=120):
            command = shlex.join(["/usr/bin/python3", "-B", "-", mode, serialized])
            return rec.run(
                name,
                ["ssh", "-T", *SSH, "mi300x", command],
                seconds,
                stdin=native_bytes,
            )

        # The path is recorded before creation so even a lost SSH reply is recoverable.
        remote("create", "create", 45)
        state["created"] = True
        rec.run(
            "upload",
            [
                "scp",
                *SSH,
                *(
                    str(payload / name)
                    for name in ("native.py", "source.tar.gz", "binding.json")
                ),
                "mi300x:" + marker["path"] + "/",
            ],
            300,
        )
        failure = None
        try:
            command = shlex.join(
                [
                    "/usr/bin/python3",
                    "-I",
                    marker["path"] + "/native.py",
                    "run",
                    serialized,
                ]
            )
            rec.run("native", ["ssh", "-T", *SSH, "mi300x", command], 2500)
            state["native_success"] = True
        except BaseException as error:
            failure = error
            state["native_failure"] = f"{type(error).__name__}: {error}"
        collect_and_clean(rec, remote, marker, state)
        if failure is not None:
            raise failure
    except BaseException as error:
        state["failure"] = f"{type(error).__name__}: {error}"
        raise
    finally:
        # Do not discard an uncollected failed run. The exact path remains recorded.
        if state["absence"]:
            shutil.rmtree(payload)
            state["local_payload_absent"] = not payload.exists()
        N.write_json(HERE / "controller-state.json", state)


if __name__ == "__main__":
    main()
