#!/usr/bin/env python3
"""Collect a fixed six-trial diagnostic and clean only its owned remote tree."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import argparse
import importlib.util
import json
from pathlib import Path
import secrets
import shlex
import shutil
import signal
import subprocess
import tarfile
import tempfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
SPEC = importlib.util.spec_from_file_location("segments_native", HERE / "xgmi_peer_segments_native.py")
N = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(N)
B, H = N.B, N.H
SSH = ["-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "-o", "ServerAliveInterval=10", "-o", "ServerAliveCountMax=3"]
SOURCE_FILES = ["benchmarks/runtime_gfx942/" + name for name in (
    "xgmi_peer_hip.cpp", "xgmi_peer_hsa.cpp", "native_benchmark_args.hpp",
    "xgmi_peer_benchmark_common.hpp", "xgmi_peer_segments_common.hpp", "copy-host-observe.py",
)]
SIGNERS = "/home/harsh/.codex-tmp/fe2o3-logical-mux-sdma-protocol-v2-20260918/allowed-signers"
SIGNERS_SHA = "ea67b34912b272ccbb46a4a83d8e00eb1a83e87832f089c1c134eed1786b560b"


def source_identity():
    files = subprocess.check_output(["git", "ls-files", "-z", "--", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml",
        ".cargo", "crates", "examples", "benchmarks/runtime_gfx942", "scripts/unsafe-source-baseline.json",
        "docs/runtime-primary-queue-release-v1.md"], cwd=ROOT).decode("utf-8").rstrip("\0").split("\0")
    return {name: H.sha(ROOT / name) for name in files}


def settle_remote(control, *, collected, native_attempted):
    if native_attempted and not collected:
        return False, ["remote receipts retained for recovery"]
    failures = []
    for name in ("cleanup", "absence"):
        try:
            control(name)
        except BaseException as error:
            failures.append(name + ": " + repr(error))
    return not failures, failures


def control_bytes():
    source = Path(B.__file__).read_bytes()
    H.need(H.sha(Path(B.__file__)) == H.BASE_SHA, "pinned process controller")
    return ("import hashlib\n" + f"source = {source!r}\n"
            + f"assert hashlib.sha256(source).hexdigest() == {H.BASE_SHA!r}\n"
            + "scope = {'__name__': 'owned_control'}\n"
            + "exec(compile(source, 'owned_control', 'exec'), scope)\n"
            + f"scope['PREFIX'] = {N.PREFIX!r}\n"
            + "scope['main']()\n").encode("ascii")


def main():
    for number in B.MANAGED:
        signal.signal(number, B.interrupted)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--device", action="append", required=True, help="index,pci-bdf,unique-id; exactly two")
    args = parser.parse_args()
    devices = []
    for value in args.device:
        index, bdf, uid = value.split(",")
        import re
        H.need(re.fullmatch(r"[0-7]", index) and re.fullmatch(r"0000:[0-9a-f]{2}:00\.0", bdf)
               and re.fullmatch(r"0x[0-9a-f]{16}", uid) and int(uid, 16) != 0, "canonical device identity")
        devices.append([int(index), bdf, uid])
    H.need(len(devices) == 2 and all(len({d[i] for d in devices}) == 2 for i in range(3)), "two distinct endpoints")
    target = args.target_dir.resolve(strict=True)
    H.need(not subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT), "clean source checkpoint")
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    source_before = source_identity()
    tree = subprocess.check_output(["git", "rev-parse", commit + "^{tree}"], cwd=ROOT, text=True).strip()
    H.need(H.sha(Path(SIGNERS)) == SIGNERS_SHA, "pinned source signer")
    output = Path(tempfile.mkdtemp(prefix="fe2o3-xgmi-segments-results-", dir="/home/harsh/.codex-tmp"))
    print("results=" + str(output), flush=True)
    rec = B.Recorder(output / "local", ROOT)
    payload = output / "payload"
    payload.mkdir()
    env = {"HOME": "/home/harsh", "USER": "harsh", "PATH": "/home/harsh/.cargo/bin:/usr/bin:/bin",
           "LANG": "C", "LC_ALL": "C", "CARGO_TARGET_DIR": str(target), "CARGO_INCREMENTAL": "0",
           "CARGO_BUILD_JOBS": "2", "CARGO_TERM_COLOR": "never", "RUSTUP_TOOLCHAIN": "nightly-2026-04-03"}
    rec.run("source-signature", ["git", "-c", "gpg.ssh.allowedSignersFile=" + SIGNERS, "verify-commit", commit], 30)
    for name, command in [("rustc", ["rustc", "-vV"]), ("cargo", ["cargo", "-V"]),
                          ("build-kfd", ["cargo", "build", "--locked", "--release", "-q", "-p", "fe2o3-runtime",
                                         "--target", "x86_64-unknown-linux-musl", "--example", "gfx942-runtime-xgmi-segments-benchmark"])]:
        rec.run(name, command, 1200, env=env)
    binary = target / "x86_64-unknown-linux-musl/release/examples/gfx942-runtime-xgmi-segments-benchmark"
    copies = {"native.py": HERE / "xgmi_peer_segments_native.py", "results.py": HERE / "xgmi_peer_segments_results.py",
              "hot.py": Path(H.__file__), "base.py": Path(B.__file__), "kfd-segments": binary}
    for name, path in copies.items():
        H.sha(path)
        shutil.copy2(path, payload / name)
    for name, command in [("after-rustc", ["rustc", "-vV"]), ("after-cargo", ["cargo", "-V"])]:
        folder = rec.run(name, command, 30, env=env)
        for stream in ("stdout", "stderr"):
            H.need(H.sha(folder / stream) == H.sha(rec.output / name.removeprefix("after-") / stream), "unchanged local toolchain")
    rec.run("rust-tests", ["cargo", "test", "--locked", "--release", "-q", "-p", "fe2o3-runtime",
        "--target", "x86_64-unknown-linux-musl", "--example", "gfx942-runtime-xgmi-segments-benchmark"], 1200, env=env)
    for name in ("test_xgmi_peer_segments.py", "test_xgmi_peer_segments_results.py", "test_xgmi_peer_segments_campaign.py"):
        rec.run(name, ["/usr/bin/python3", "-I", "-B", str(HERE / name)], 120,
                env={**env, "FE2O3_SEGMENTS_RUST_BINARY": str(binary)})
    H.need(not subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT), "clean source after build")
    H.need(subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip() == commit, "same commit after build")
    H.need(source_identity() == source_before, "unchanged KFD and comparator source after build")
    source_files = {name: H.sha(ROOT / name) for name in SOURCE_FILES}
    with tarfile.open(payload / "source.tar.gz", "w:gz") as archive:
        for name in SOURCE_FILES:
            archive.add(ROOT / name, arcname=name, recursive=False)
    binding = {"commit": commit, "git_tree": tree, "local_source_files": source_before,
               "payload": B.inventory(payload), "source_files": source_files, "devices": devices,
               "controls": N.CONTROLS, "order": list(N.ORDER), "target": "x86_64-unknown-linux-musl",
               "profile": "release default opt-level=3", "features": "default", "local_build_environment": env,
               "source_archive_scope": "remote comparator and observer inputs; KFD uses signed full source checkpoint"}
    B.write_json(output / "binding.json", binding)
    marker = {"path": N.PREFIX + secrets.token_hex(8), "commit": commit, "binding_sha256": H.sha(output / "binding.json")}
    B.write_json(output / "owner.json", marker)
    serialized = json.dumps(marker, sort_keys=True, separators=(",", ":"))
    created = False
    native_attempted = False
    collected = False
    cleaned = False
    failures = []

    def control(name):
        return rec.run(name, ["ssh", "-T", *SSH, "mi300x", shlex.join(["/usr/bin/python3", "-I", "-B", "-", name, serialized])],
                       120, stdin=control_bytes())

    try:
        # A failed create can leave its private marker behind; attempt scoped recovery.
        created = True
        control("create")
        rec.run("upload", ["scp", "-q", *SSH, "--", *(str(payload / name) for name in sorted(N.PAYLOAD)),
                           str(output / "binding.json"), "mi300x:" + marker["path"] + "/"], 120)
        native_attempted = True
        rec.run("native", ["ssh", "-T", *SSH, "mi300x", shlex.join(["/usr/bin/python3", "-I", "-B",
                         marker["path"] + "/native.py", "run", serialized])], 6000)
    except BaseException as error:
        failures.append(repr(error))
    finally:
        if created:
            try:
                folder = control("inventory")
                remote_inventory = H.parse_json((folder / "stdout").read_bytes())
                rec.run("collect", ["scp", "-q", "-r", *SSH, "--", "mi300x:" + marker["path"] + "/results", str(output / "remote")], 120)
                H.need(B.inventory(output / "remote") == remote_inventory, "byte-exact remote collection")
                B.write_json(output / "remote-inventory.json", remote_inventory)
                collected = True
            except BaseException as error:
                failures.append("collection: " + repr(error))
            cleaned, cleanup_failures = settle_remote(control, collected=collected, native_attempted=native_attempted)
            failures.extend(error + "; owned=" + marker["path"] for error in cleanup_failures)
        try:
            H.need(H.sha(binary) == binding["payload"]["kfd-segments"], "unchanged local binary")
            H.need(not subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT), "unchanged clean source")
            H.need(subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip() == commit, "unchanged commit")
            H.need(source_identity() == source_before, "unchanged local source inventory")
        except BaseException as error:
            failures.append("local identity: " + repr(error))
        result = {"source_commit": commit, "failures": failures, "owned_cleanup": cleaned,
                  "exclusive_reservation": False, "performance_acceptance": False}
        B.write_json(output / "collection.json", result)
        print(json.dumps({"results": str(output), **result}), flush=True)
    H.need(not failures and cleaned, "campaign did not qualify")


if __name__ == "__main__":
    main()
