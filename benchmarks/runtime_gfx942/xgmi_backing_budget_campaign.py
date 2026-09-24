#!/usr/bin/env python3
"""Source-bound backing-budget witness with fresh admission and exact owned cleanup."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import argparse
import hashlib
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
from types import ModuleType

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]


def authenticated_bytes(path, digest):
    if path.is_symlink() or not path.is_file():
        raise RuntimeError("ordinary helper required: " + str(path))
    raw = path.read_bytes()
    if hashlib.sha256(raw).hexdigest() != digest:
        raise RuntimeError("helper digest: " + str(path))
    return raw


def module(name, path, digest):
    raw = authenticated_bytes(path, digest)
    value = ModuleType(name)
    value.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), value.__dict__)
    return value


IMPORTS = {
    "benchmarks/runtime_gfx942/xgmi_peer_segments_campaign.py": "81e81bdbb7d900d90cce50ea2c7992cb4872f3fcc0ccc5ccbc18ccd7557d4de9",
    "benchmarks/runtime_gfx942/xgmi_peer_segments_native.py": "d47da25e4f1a542a10b1c15cd86371c83593efbf70ebc1286b3453eca7632149",
    "docs/evidence/dev-xgmi-peer-hot-mi300x-2026-09-19/native.py": "820ad87e74a1f9915c2eb7d2d7c7c6c1c451da4cecf29fcc381229324d98473b",
    "docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py": "df740a80c83c94af02f7e7dfb1203dcf7c1eafbde0010e41219e4aac40b34bb7",
    "benchmarks/runtime_gfx942/xgmi_backing_budget_native.py": "b0e5f5891e15809edc8a496f3133dadbc7c579cd39519e79dea908441a6aa126",
}
# The inherited controller imports its native helper at module initialization.
for relative, digest in IMPORTS.items():
    authenticated_bytes(ROOT / relative, digest)
C = module("backing_budget_campaign_helpers", HERE / "xgmi_peer_segments_campaign.py",
           IMPORTS["benchmarks/runtime_gfx942/xgmi_peer_segments_campaign.py"])
N = module("backing_budget_native", HERE / "xgmi_backing_budget_native.py",
           IMPORTS["benchmarks/runtime_gfx942/xgmi_backing_budget_native.py"])
B, H = N.B, N.H
EXAMPLE = "gfx942-runtime-xgmi-backing-budget-smoke"
QUALIFIED = "685879ab5b3a3e0b31c10f4a44944e0e0962e6ff"
SELECTORS = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", "crates", "examples",
             "benchmarks/runtime_gfx942", "scripts/unsafe-source-baseline.json",
             "docs/runtime-primary-queue-release-v1.md"]
GIT_ENV = {"HOME": "/home/harsh", "PATH": "/usr/bin:/bin", "LC_ALL": "C",
           "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_SYSTEM": "/dev/null", "GIT_CONFIG_GLOBAL": "/dev/null"}
GIT = ["/usr/bin/git", "--no-replace-objects", "-c", "core.fsmonitor=false", "-c", "core.untrackedCache=false"]


def git(*arguments):
    return subprocess.check_output([*GIT, *arguments], cwd=ROOT, env=GIT_ENV)


def source_identity():
    files = git("ls-files", "-z", "--", *SELECTORS).decode("utf-8").rstrip("\0").split("\0")
    return {name: H.sha(ROOT / name) for name in files}


def qualified_production(commit):
    git("merge-base", "--is-ancestor", QUALIFIED, commit)
    changed = git("diff", "--name-only", QUALIFIED, commit, "--", "crates", "examples", "Cargo.toml",
                  "Cargo.lock", "rust-toolchain.toml", ".cargo").decode().splitlines()
    H.need(set(changed) <= {"crates/fe2o3-runtime/examples/" + EXAMPLE + ".rs"}, "unchanged qualified production source")


BOOTSTRAP = '''import hashlib, json, sys
from pathlib import Path
def unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise RuntimeError("duplicate key")
        result[key] = value
    return result
marker = json.loads(sys.argv[1], object_pairs_hook=unique)
owned = Path(marker["path"])
def ordinary(name):
    path = owned / name
    if path.is_symlink() or not path.is_file():
        raise RuntimeError("ordinary bootstrap input required")
    return path.read_bytes()
binding_bytes = ordinary("binding.json")
if hashlib.sha256(binding_bytes).hexdigest() != marker["binding_sha256"]:
    raise RuntimeError("bootstrap binding digest")
binding = json.loads(binding_bytes, object_pairs_hook=unique)
raw = ordinary("native.py")
if hashlib.sha256(raw).hexdigest() != binding["payload"]["native.py"]:
    raise RuntimeError("bootstrap native digest")
sys.argv = [str(owned / "native.py"), "run", sys.argv[1]]
exec(compile(raw, sys.argv[0], "exec"), {"__name__": "__main__", "__file__": sys.argv[0]})
'''


def native_command(serialized):
    return ["ssh", "-T", *C.SSH, "mi300x",
            shlex.join(["/usr/bin/python3", "-I", "-B", "-c", BOOTSTRAP, serialized])]


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
    H.need(git("rev-parse", "--show-toplevel").decode().strip() == str(ROOT), "exact Git worktree")
    H.need(not git("status", "--porcelain"), "clean checkpoint")
    commit = git("rev-parse", "HEAD").decode().strip()
    qualified_production(commit)
    tree = git("rev-parse", commit + "^{tree}").decode().strip()
    source_before = source_identity()
    H.need(H.sha(Path(C.SIGNERS)) == C.SIGNERS_SHA, "pinned signer")
    output = Path(tempfile.mkdtemp(prefix="fe2o3-xgmi-backing-budget-results-", dir="/home/harsh/.codex-tmp"))
    print("results=" + str(output), flush=True)
    rec = B.Recorder(output / "local", ROOT)
    payload = output / "payload"
    payload.mkdir()
    env = {"HOME": "/home/harsh", "USER": "harsh", "PATH": "/home/harsh/.cargo/bin:/usr/bin:/bin",
           "LANG": "C", "LC_ALL": "C", "CARGO_TARGET_DIR": str(target), "CARGO_INCREMENTAL": "0",
           "CARGO_BUILD_JOBS": "2", "CARGO_TERM_COLOR": "never", "RUSTUP_TOOLCHAIN": "nightly-2026-04-03"}
    rec.run("source-signature", [*GIT, "-c", "gpg.ssh.allowedSignersFile=" + C.SIGNERS, "verify-commit", commit], 30, env=GIT_ENV)
    for name, command in [("rustc", ["rustc", "-vV"]), ("cargo", ["cargo", "-V"]),
                          ("build-kfd", ["cargo", "build", "--locked", "--release", "-q", "-p", "fe2o3-runtime",
                                         "--target", "x86_64-unknown-linux-musl", "--example", EXAMPLE])]:
        rec.run(name, command, 1200, env=env)
    binary = target / "x86_64-unknown-linux-musl/release/examples" / EXAMPLE
    for name, source in {"native.py": Path(N.__file__), "results.py": HERE / "xgmi_backing_budget_results.py",
                         "hot.py": Path(H.__file__), "base.py": Path(B.__file__), "kfd-owner": binary}.items():
        H.sha(source)
        shutil.copy2(source, payload / name)
    rec.run("rust-tests", ["cargo", "test", "--locked", "--release", "-q", "-p", "fe2o3-runtime",
            "--target", "x86_64-unknown-linux-musl", "--example", EXAMPLE], 1200, env=env)
    for name in ["test_xgmi_backing_budget_results.py", "test_xgmi_backing_budget_campaign.py",
                 "test_xgmi_backing_budget_native.py"]:
        rec.run(name, ["/usr/bin/python3", "-I", "-B", str(HERE / name)], 120,
                env={**env, "FE2O3_BACKING_BUDGET_RUST_BINARY": str(binary)})
    for name, command in [("after-rustc", ["rustc", "-vV"]), ("after-cargo", ["cargo", "-V"])]:
        folder = rec.run(name, command, 30, env=env)
        for stream in ("stdout", "stderr"):
            H.need(H.sha(folder / stream) == H.sha(rec.output / name.removeprefix("after-") / stream), "local toolchain continuity")
    H.need(not git("status", "--porcelain"), "clean source after build")
    H.need(git("rev-parse", "HEAD").decode().strip() == commit, "source commit continuity")
    H.need(source_identity() == source_before, "source bytes continuity")
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
        rec.run("native", native_command(serialized), 1200)
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
            H.need(not git("status", "--porcelain"), "clean source continuity")
            H.need(git("rev-parse", "HEAD").decode().strip() == commit, "commit continuity")
            H.need(source_identity() == source_before, "source inventory continuity")
        except BaseException as error:
            failures.append("local identity: " + repr(error))
        result = {"source_commit": commit, "failures": failures, "owned_cleanup": cleaned,
                  "exclusive_reservation": False, "performance_acceptance": False, "formal_refinement": False}
        B.write_json(output / "collection.json", result)
        print(json.dumps({"results": str(output), **result}), flush=True)
    H.need(not failures and cleaned, "owner correctness campaign did not qualify")


if __name__ == "__main__":
    main()
