#!/usr/bin/env python3
"""Build signed hot-batch source, collect fixed MI300X trials, clean owned state."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import argparse
import hashlib
import json
from pathlib import Path
import secrets
import shlex
import shutil
import signal
import tarfile
from types import ModuleType

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
SOURCE_COMMIT = "8dc128357ecd55e1ba4f2866eb075899481f9aa0"
CPU = "docs/evidence/dev-xgmi-hot-batch-cpu-2026-09-24"
PRIOR_CPU = "docs/evidence/dev-topology-link-scratch-cpu-2026-09-24"
CPU_VERIFY_SHA = "576219a4db4b9edca5055630a7d37222974a1ebb051556177ecad33495e4a006"
NATIVE_SHA = "e71091bc83cbe9c254650479e68c3e3110d3d6ca7561cdc4a8fbaed9ba564302"
PROTOCOL = ("campaign.py", "native.py", "test_campaign.py", "qualify.py", "PROTOCOL.md")


def load(path, digest, name):
    raw = path.read_bytes()
    if path.is_symlink() or hashlib.sha256(raw).hexdigest() != digest:
        raise RuntimeError("authenticated helper: " + str(path))
    value = ModuleType(name)
    value.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), value.__dict__)
    return value


K = load(REPO / "benchmarks/runtime_gfx942/xgmi_backing_budget_campaign.py",
         "9b90dac696bcebe7a4cf7328d404e9edfc485ff1cd0af63b36ae4795b1dfdf76", "hot_batch_controller")
N = load(HERE / "native.py", NATIVE_SHA, "hot_batch_native")
B, H, C = N.B, N.H, K.C
EXAMPLE = "gfx942-runtime-xgmi-peer-benchmark"


def source(rec, output):
    folder = rec.run("source-tree", [*K.GIT, "ls-tree", "-rz", SOURCE_COMMIT, "--", *K.SELECTORS], 120, env=K.GIT_ENV)
    entries = {}
    for row in (folder / "stdout").read_bytes().rstrip(b"\0").split(b"\0"):
        header, raw_name = row.split(b"\t")
        mode, kind, oid = header.decode().split()
        name = raw_name.decode()
        H.need(mode in ("100644", "100755") and kind == "blob" and name not in entries, "ordinary unique source")
        entries[name] = (mode, oid)
    selectors = [value for value in K.SELECTORS if any(name == value or name.startswith(value + "/") for name in entries)]
    snapshot = output / "build-source.tar.gz"
    rec.run("source-archive", [*K.GIT, "archive", "--format=tar.gz", "--output=" + str(snapshot), SOURCE_COMMIT, *selectors],
            120, env=K.GIT_ENV)
    checkout = output / "source"
    checkout.mkdir()
    with tarfile.open(snapshot, "r:gz") as archive:
        members = archive.getmembers()
        directories = {str(parent) for name in entries for parent in Path(name).parents if str(parent) != "."}
        observed = set()
        for member in members:
            H.need(member.isfile() or member.isdir(), "ordinary source entries")
            if member.isdir():
                name = member.name.rstrip("/")
                H.need(name in directories and name not in observed, "exact parent directory")
                observed.add(name)
        B.validate_members([member for member in members if member.isfile()], entries)
        archive.extractall(checkout, filter="data")
    files = B.inventory(checkout)
    H.need(set(files) == set(entries), "complete source extraction")
    for name, (mode, oid) in entries.items():
        raw = (checkout / name).read_bytes()
        H.need(hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest() == oid, "signed source blob")
        H.need(bool((checkout / name).stat().st_mode & 0o111) == (mode == "100755"), "source mode")
    B.write_json(output / "source.json", files)
    B.write_json(output / "source-archive.json", {"sha256": H.sha(snapshot), "bytes": snapshot.stat().st_size,
                 "retention": "transient-build-input; reconstruct from signed Git objects"})
    return checkout, files


def authenticate_cpu_packets(rec):
    folder = rec.run("cpu-packet-tree", [*K.GIT, "ls-tree", "-rz", SOURCE_COMMIT, "--", CPU, PRIOR_CPU],
                     120, env=K.GIT_ENV)
    expected = {}
    for row in (folder / "stdout").read_bytes().rstrip(b"\0").split(b"\0"):
        header, raw_name = row.split(b"\t")
        mode, kind, oid = header.decode().split()
        name = raw_name.decode()
        H.need(mode in ("100644", "100755") and kind == "blob" and name not in expected,
               "ordinary unique CPU evidence")
        H.need(any(name.startswith(root + "/") for root in (CPU, PRIOR_CPU)), "CPU packet path")
        path = REPO / name
        H.need(path.is_file() and not path.is_symlink(), "ordinary CPU evidence file")
        raw = path.read_bytes()
        H.need(hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest() == oid,
               "signed CPU evidence bytes: " + name)
        H.need(bool(path.stat().st_mode & 0o111) == (mode == "100755"), "CPU evidence mode")
        expected[name] = H.sha(path)
    actual = {root + "/" + name: digest for root in (CPU, PRIOR_CPU)
              for name, digest in B.inventory(REPO / root).items()}
    H.need(actual == expected and actual, "complete signed CPU evidence inventory")
    return expected


def cpu_binding(rec, files):
    evidence = authenticate_cpu_packets(rec)
    folder = rec.run("cpu-source-map", [*K.GIT, "show", SOURCE_COMMIT + ":" + CPU + "/raw/cpu2/inputs-before.json"],
                     30, env=K.GIT_ENV)
    inputs = H.parse_json((folder / "stdout").read_bytes())
    roots = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", "crates", "benchmarks/runtime_gfx942"]
    expected = {name: digest for name, digest in files.items()
                if any(name == root or name.startswith(root + "/") for root in roots)
                and (name.endswith((".rs", ".toml", ".lock", ".json", ".py", ".cpp", ".hpp")) or "/fixtures/" in name)}
    H.need(inputs["source"] == expected, "exact signed CPU source roster")
    checker = REPO / CPU / "verify.py"
    H.need(H.sha(checker) == CPU_VERIFY_SHA, "CPU replay checker identity")
    rec.run("cpu-replay", ["/usr/bin/python3", "-I", "-B", str(checker)], 120, env=K.GIT_ENV)
    return {"commit": SOURCE_COMMIT, "packet": CPU, "input_sha256": H.sha(folder / "stdout"),
            "checker_sha256": CPU_VERIFY_SHA, "evidence_files": evidence}


def build_environment(output):
    target = output / "target"
    target.mkdir()
    return {"HOME": "/home/harsh", "USER": "harsh", "PATH": "/home/harsh/.cargo/bin:/usr/bin:/bin",
            "LANG": "C", "LC_ALL": "C", "CARGO_TARGET_DIR": str(target), "CARGO_INCREMENTAL": "0",
            "CARGO_BUILD_JOBS": "2", "CARGO_TERM_COLOR": "never", "RUSTUP_TOOLCHAIN": "nightly-2026-04-03"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--device", action="append", required=True)
    args = parser.parse_args()
    devices = K.devices_from_args(args.device)
    for number in B.MANAGED:
        signal.signal(number, B.interrupted)
    output = args.output.resolve()
    output.mkdir()
    rec = B.Recorder(output / "local", REPO)
    protocol = {name: H.sha(HERE / name) for name in PROTOCOL}
    B.write_json(output / "protocol-before.json", protocol)
    frozen = output / "protocol"
    frozen.mkdir()
    for name in protocol:
        shutil.copy2(HERE / name, frozen / name)
    payload = output / "payload"
    payload.mkdir()
    H.need(H.sha(Path(C.SIGNERS)) == C.SIGNERS_SHA, "pinned signer")
    rec.run("source-signature", [*K.GIT, "-c", "gpg.ssh.allowedSignersFile=" + C.SIGNERS, "verify-commit", SOURCE_COMMIT],
            30, env=K.GIT_ENV)
    checkout, files = source(rec, output)
    qualified = cpu_binding(rec, files)
    env = build_environment(output)
    rec.cwd = checkout
    binary = Path(env["CARGO_TARGET_DIR"]) / "x86_64-unknown-linux-musl/release/examples" / EXAMPLE
    built_sha = None
    for name, command, seconds in [
        ("rustc", ["rustc", "-vV"], 30), ("cargo", ["cargo", "-V"], 30),
        ("build-kfd", ["cargo", "build", "--offline", "--locked", "--release", "-p", "fe2o3-runtime",
                       "--target", "x86_64-unknown-linux-musl", "--example", EXAMPLE], 1800),
        ("test-kfd", ["cargo", "test", "--offline", "--locked", "--release", "-p", "fe2o3-runtime",
                      "--target", "x86_64-unknown-linux-musl", "--example", EXAMPLE], 1800),
        ("after-rustc", ["rustc", "-vV"], 30), ("after-cargo", ["cargo", "-V"], 30),
    ]:
        rec.run(name, command, seconds, env=env)
        if name == "build-kfd":
            built_sha = H.sha(binary)
            B.write_json(output / "built-kfd.json", {"sha256": built_sha})
        elif built_sha is not None:
            H.need(H.sha(binary) == built_sha, "standalone ELF unchanged after test/tool commands")
    for tool in ("rustc", "cargo"):
        for stream in ("stdout", "stderr"):
            H.need(H.sha(rec.output / tool / stream) == H.sha(rec.output / ("after-" + tool) / stream), "tool continuity")
    H.need(B.inventory(checkout) == files, "unchanged build inputs")
    copies = {"native.py": HERE / "native.py", "hot.py": Path(H.__file__), "base.py": Path(B.__file__),
              "results.py": checkout / "benchmarks/runtime_gfx942/xgmi_peer_hot_results.py", "kfd": binary}
    for name, path in copies.items():
        shutil.copy2(path, payload / name)
    H.need(H.sha(payload / "results.py") == N.PARSER_SHA, "qualified parser")
    remote_files = {name: files[name] for name in C.SOURCE_FILES}
    with tarfile.open(payload / "source.tar.gz", "w:gz") as archive:
        for name in remote_files:
            archive.add(checkout / name, arcname=name, recursive=False)
    H.need({name: H.sha(HERE / name) for name in protocol} == protocol, "unchanged protocol before native")
    binding = {"commit": SOURCE_COMMIT, "local_source_files": files, "source_files": remote_files,
               "protocol_files": protocol, "cpu_qualification": qualified, "payload": B.inventory(payload),
               "devices": devices, "controls": N.CONTROLS, "order": N.ORDER,
               "target": "x86_64-unknown-linux-musl", "profile": "release default", "features": "default",
               "local_build_environment": env, "outer_native_seconds": N.REMOTE_SECONDS}
    B.write_json(output / "binding.json", binding)
    marker = {"path": N.PREFIX + secrets.token_hex(8), "commit": SOURCE_COMMIT, "binding_sha256": H.sha(output / "binding.json")}
    B.write_json(output / "owner.json", marker)
    serialized = json.dumps(marker, sort_keys=True, separators=(",", ":"))
    rec.cwd = REPO
    attempted, collected, cleaned, failures = False, False, False, []

    def control(name):
        return rec.run(name, ["ssh", "-T", *C.SSH, "mi300x", shlex.join(["/usr/bin/python3", "-I", "-B", "-", name, serialized])],
                       120, stdin=C.control_bytes(N.PREFIX))

    try:
        control("create")
        rec.run("upload", ["scp", "-q", *C.SSH, "--", *(str(payload / name) for name in sorted(N.PAYLOAD)),
                           str(output / "binding.json"), "mi300x:" + marker["path"] + "/"], 120)
        attempted = True
        rec.run("native", K.native_command(serialized), N.REMOTE_SECONDS)
    except BaseException as error:
        failures.append(repr(error))
    finally:
        try:
            folder = control("inventory")
            expected = H.parse_json((folder / "stdout").read_bytes())
            rec.run("collect", ["scp", "-q", "-r", *C.SSH, "--", "mi300x:" + marker["path"] + "/results", str(output / "remote")], 120)
            H.need(B.inventory(output / "remote") == expected, "byte-exact collection")
            B.write_json(output / "remote-inventory.json", expected)
            collected = True
        except BaseException as error:
            failures.append("collection: " + repr(error))
        cleaned, errors = C.settle_remote(control, collected=collected, native_attempted=attempted)
        failures.extend(errors)
        try:
            H.need(B.inventory(payload) == binding["payload"], "unchanged retained payload")
            H.need(B.inventory(checkout) == files, "unchanged local source")
            H.need(H.sha(binary) == binding["payload"]["kfd"], "unchanged local executable")
            after = {name: H.sha(HERE / name) for name in protocol}
            B.write_json(output / "protocol-after.json", after)
            H.need(after == protocol, "unchanged protocol after native")
        except BaseException as error:
            failures.append("local identity: " + repr(error))
        B.write_json(output / "collection.json", {"failures": failures, "owned_cleanup": cleaned,
                     "exclusive_reservation": False, "performance_acceptance": False})
    H.need(not failures and cleaned, "campaign did not qualify")


if __name__ == "__main__":
    main()
