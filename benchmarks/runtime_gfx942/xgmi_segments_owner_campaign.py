#!/usr/bin/env python3
"""Source-bound owner correctness with fresh admission and exact owned cleanup."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import argparse
import importlib.util
import json
from pathlib import Path
import re
import secrets
import shlex
import shutil
import signal
import subprocess
import tarfile
import tempfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


C = module("owner_campaign_helpers", HERE / "xgmi_peer_segments_campaign.py")
N = module("owner_native", HERE / "xgmi_segments_owner_native.py")
B, H = N.B, N.H
EXAMPLE = "gfx942-runtime-xgmi-segments-owner-smoke"


def devices_from_args(values):
    devices = []
    for value in values:
        index, bdf, uid = value.split(",")
        H.need(re.fullmatch(r"[0-7]", index) and re.fullmatch(r"0000:[0-9a-f]{2}:00\.0", bdf)
               and re.fullmatch(r"0x[0-9a-f]{16}", uid) and int(uid, 16), "canonical endpoint identity")
        devices.append([int(index), bdf, uid])
    H.need(len(devices) == 2 and all(len({d[i] for d in devices}) == 2 for i in range(3)), "two distinct endpoints")
    return devices


def main():
    for number in B.MANAGED:
        signal.signal(number, B.interrupted)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--device", action="append", required=True, help="index,pci-bdf,unique-id; exactly two")
    args = parser.parse_args()
    devices = devices_from_args(args.device)
    target = args.target_dir.resolve(strict=True)
    H.need(not subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT), "clean checkpoint")
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    tree = subprocess.check_output(["git", "rev-parse", commit + "^{tree}"], cwd=ROOT, text=True).strip()
    source_before = C.source_identity()
    H.need(H.sha(Path(C.SIGNERS)) == C.SIGNERS_SHA, "pinned signer")
    output = Path(tempfile.mkdtemp(prefix="fe2o3-xgmi-owner-results-", dir="/home/harsh/.codex-tmp"))
    print("results=" + str(output), flush=True)
    rec = B.Recorder(output / "local", ROOT)
    payload = output / "payload"
    payload.mkdir()
    env = {"HOME": "/home/harsh", "USER": "harsh", "PATH": "/home/harsh/.cargo/bin:/usr/bin:/bin",
           "LANG": "C", "LC_ALL": "C", "CARGO_TARGET_DIR": str(target), "CARGO_INCREMENTAL": "0",
           "CARGO_BUILD_JOBS": "2", "CARGO_TERM_COLOR": "never", "RUSTUP_TOOLCHAIN": "nightly-2026-04-03"}
    rec.run("source-signature", ["git", "-c", "gpg.ssh.allowedSignersFile=" + C.SIGNERS, "verify-commit", commit], 30)
    for name, command in [("rustc", ["rustc", "-vV"]), ("cargo", ["cargo", "-V"]),
                          ("build-kfd", ["cargo", "build", "--locked", "--release", "-q", "-p", "fe2o3-runtime",
                                         "--target", "x86_64-unknown-linux-musl", "--example", EXAMPLE])]:
        rec.run(name, command, 1200, env=env)
    binary = target / "x86_64-unknown-linux-musl/release/examples" / EXAMPLE
    for name, source in {"native.py": Path(N.__file__), "results.py": HERE / "xgmi_segments_owner_results.py",
                         "hot.py": Path(H.__file__), "base.py": Path(B.__file__), "kfd-owner": binary}.items():
        H.sha(source)
        shutil.copy2(source, payload / name)
    rec.run("rust-tests", ["cargo", "test", "--locked", "--release", "-q", "-p", "fe2o3-runtime",
            "--target", "x86_64-unknown-linux-musl", "--example", EXAMPLE], 1200, env=env)
    for name in ["test_xgmi_segments_owner_results.py", "test_xgmi_segments_owner_campaign.py"]:
        rec.run(name, ["/usr/bin/python3", "-I", "-B", str(HERE / name)], 120,
                env={**env, "FE2O3_OWNER_RUST_BINARY": str(binary)})
    for name, command in [("after-rustc", ["rustc", "-vV"]), ("after-cargo", ["cargo", "-V"])]:
        folder = rec.run(name, command, 30, env=env)
        for stream in ("stdout", "stderr"):
            H.need(H.sha(folder / stream) == H.sha(rec.output / name.removeprefix("after-") / stream), "local toolchain continuity")
    H.need(not subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT), "clean source after build")
    H.need(subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip() == commit, "source commit continuity")
    H.need(C.source_identity() == source_before, "source bytes continuity")
    observer = "benchmarks/runtime_gfx942/copy-host-observe.py"
    source_files = {observer: H.sha(ROOT / observer)}
    with tarfile.open(payload / "source.tar.gz", "w:gz") as archive:
        archive.add(ROOT / observer, arcname=observer, recursive=False)
    binding = {"commit": commit, "git_tree": tree, "local_source_files": source_before, "payload": B.inventory(payload),
               "source_files": source_files, "devices": devices, "controls": N.CONTROLS, "order": list(N.ORDER),
               "target": "x86_64-unknown-linux-musl", "profile": "release default opt-level=3", "features": "default",
               "local_build_environment": env, "source_archive_scope": "remote observer input; KFD uses signed full source checkpoint"}
    B.write_json(output / "binding.json", binding)
    marker = {"path": N.PREFIX + secrets.token_hex(8), "commit": commit, "binding_sha256": H.sha(output / "binding.json")}
    B.write_json(output / "owner.json", marker)
    serialized = json.dumps(marker, sort_keys=True, separators=(",", ":"))
    created = native_attempted = collected = cleaned = False
    failures = []

    def control(name):
        return rec.run(name, ["ssh", "-T", *C.SSH, "mi300x", shlex.join(["/usr/bin/python3", "-I", "-B", "-", name, serialized])],
                       120, stdin=C.control_bytes(N.PREFIX))

    try:
        created = True
        control("create")
        rec.run("upload", ["scp", "-q", *C.SSH, "--", *(str(payload / name) for name in sorted(N.PAYLOAD)),
                           str(output / "binding.json"), "mi300x:" + marker["path"] + "/"], 120)
        native_attempted = True
        rec.run("native", ["ssh", "-T", *C.SSH, "mi300x", shlex.join(["/usr/bin/python3", "-I", "-B",
                marker["path"] + "/native.py", "run", serialized])], 1200)
    except BaseException as error:
        failures.append(repr(error))
    finally:
        if created:
            try:
                folder = control("inventory")
                remote_inventory = H.parse_json((folder / "stdout").read_bytes())
                rec.run("collect", ["scp", "-q", "-r", *C.SSH, "--", "mi300x:" + marker["path"] + "/results", str(output / "remote")], 120)
                H.need(B.inventory(output / "remote") == remote_inventory, "byte-exact collection")
                B.write_json(output / "remote-inventory.json", remote_inventory)
                collected = True
            except BaseException as error:
                failures.append("collection: " + repr(error))
            cleaned, cleanup_failures = C.settle_remote(control, collected=collected, native_attempted=native_attempted)
            failures.extend(error + "; owned=" + marker["path"] for error in cleanup_failures)
        try:
            H.need(H.sha(binary) == binding["payload"]["kfd-owner"], "local ELF continuity")
            H.need(not subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT), "clean source continuity")
            H.need(subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip() == commit, "commit continuity")
            H.need(C.source_identity() == source_before, "source inventory continuity")
        except BaseException as error:
            failures.append("local identity: " + repr(error))
        result = {"source_commit": commit, "failures": failures, "owned_cleanup": cleaned,
                  "exclusive_reservation": False, "performance_acceptance": False, "formal_refinement": False}
        B.write_json(output / "collection.json", result)
        print(json.dumps({"results": str(output), **result}), flush=True)
    H.need(not failures and cleaned, "owner correctness campaign did not qualify")


if __name__ == "__main__":
    main()
