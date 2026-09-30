#!/usr/bin/env python3
"""Bounded selected-source transport and owned MI300X campaign supervision."""

import argparse
import contextlib
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import resource
import runpy
import secrets
import select
import shlex
import shutil
import signal
import stat
import subprocess
import sys
import tarfile
import time
import types

SCRIPT = Path(__file__).resolve() if "__file__" in globals() else None
PREFIX = "/home/harsh/fe2o3-xgmi-series-20260930."
SHARED_LOCK = Path("/home/harsh/.fe2o3-cargo-all-targets.lock")
INITIAL_PACK_LIMIT = 32 * 1024**2
SELECTED_LIMIT = 128 * 1024**2
ARCHIVE_LIMIT = 256 * 1024**2
PRIVATE_LIMIT = 12 * 1024**3
DISK_RESERVE = 40 * 1024**3
MEMORY_RESERVE = 64 * 1024**3
LOCK_WAIT = 900
CAMPAIGN_SECONDS = 5400
SIGNER = "harmenon@amd.com"
SIGNER_KEY = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAICITzoV64zd4tYeZhvOi+mnwQaxEI4rFvXeC3HxileBS"
FINGERPRINT = "SHA256:q8oGVYZ11904aFzlMkSiEwyeSP+6hbuiZGbNVGRZVCg"
GIT = ["/usr/bin/git", "--no-replace-objects", "-c", "gc.auto=0"]
UPLOAD_PACK = "git --no-replace-objects -c gc.auto=0 -c uploadpack.allowFilter=true upload-pack"
TRANSPORT_RELATIVE = "benchmarks/runtime_gfx942/xgmi_peer_series_transport.py"
NATIVE_RELATIVE = "benchmarks/runtime_gfx942/xgmi_peer_series_native.py"
SSH = ["/usr/bin/ssh", "-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=yes",
       "-o", "ControlMaster=no", "-o", "ControlPath=none", "mi300x"]


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(path):
    path = Path(path)
    need(path.is_file() and not path.is_symlink(), "ordinary file required: " + str(path))
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        need(key not in result, "duplicate JSON key")
        result[key] = value
    return result


def parse_json(raw):
    def reject(value):
        raise ValueError("nonfinite JSON: " + value)
    return json.loads(raw, object_pairs_hook=unique_object, parse_constant=reject)


def encoded(value):
    return (json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False) + "\n").encode("ascii")


def save(path, value):
    with path.open("xb") as stream:
        stream.write(encoded(value))


def digest(value):
    return hashlib.sha256(encoded(value)).hexdigest()


def canonical_name(name):
    need(type(name) is str and name and not name.startswith("/") and "\\" not in name
         and all(part not in ("", ".", "..") for part in name.split("/"))
         and all(32 <= ord(char) < 127 for char in name), "canonical relative member")
    return name


def inventory(root):
    result = {}
    for path in sorted(root.rglob("*")):
        need(not path.is_symlink(), "no archived symlinks")
        if path.is_dir():
            continue
        need(path.is_file(), "ordinary archive member")
        name = canonical_name(path.relative_to(root).as_posix())
        result[name] = {"sha256": sha(path), "bytes": path.stat().st_size,
                        "mode": stat.S_IMODE(path.stat().st_mode)}
    return result


def bounded_manifest(files, limit=ARCHIVE_LIMIT):
    need(type(files) is dict and 0 < len(files) <= 30000, "bounded exact file roster")
    total = 0
    for name, row in files.items():
        canonical_name(name)
        need(type(row) is dict and set(row) == {"sha256", "bytes", "mode"}, "member metadata roster")
        need(type(row["sha256"]) is str and re.fullmatch(r"[0-9a-f]{64}", row["sha256"]), "member digest")
        need(type(row["bytes"]) is int and 0 <= row["bytes"] <= limit, "member size")
        need(type(row["mode"]) is int and row["mode"] in (0o600, 0o644, 0o700, 0o755), "ordinary file mode")
        total += row["bytes"]
    need(total <= limit, "archive uncompressed byte ceiling")


def make_archive(path, root, files):
    bounded_manifest(files)
    with tarfile.open(path, "x:gz") as archive:
        for name, row in sorted(files.items()):
            member = root / name
            need(sha(member) == row["sha256"] and member.stat().st_size == row["bytes"], "stable archive member")
            archive.add(member, arcname=name, recursive=False)
    need(path.stat().st_size <= ARCHIVE_LIMIT, "compressed archive ceiling")
    need({name: inventory_row(root / name) for name in files} == files, "archive source readback")


def inventory_row(path):
    return {"sha256": sha(path), "bytes": path.stat().st_size,
            "mode": stat.S_IMODE(path.stat().st_mode)}


def validate_archive(path, files, destination=None):
    bounded_manifest(files)
    need(path.stat().st_size <= ARCHIVE_LIMIT, "compressed archive ceiling")
    seen = set()
    with tarfile.open(path, "r:gz") as archive:
        for member in archive:
            name = canonical_name(member.name)
            need(member.isfile() and name in files and name not in seen, "exact ordinary archive roster")
            row = files[name]
            need(member.size == row["bytes"] and member.mode == row["mode"], "archive member size/mode")
            source = archive.extractfile(member)
            need(source is not None, "readable archive member")
            with source:
                actual = hashlib.file_digest(source, "sha256").hexdigest()
            need(actual == row["sha256"], "archive member digest")
            seen.add(name)
        need(seen == set(files), "complete archive member roster")
        if destination is not None:
            archive.extractall(destination, filter="data")


def native_module():
    need(SCRIPT is not None, "repository invocation required")
    path = SCRIPT.with_name("xgmi_peer_series_native.py")
    spec = importlib.util.spec_from_file_location("series_transport_native", path)
    native = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(native)
    return native


def require_filter_capability(trace):
    lines = trace.decode("ascii").splitlines()
    capabilities = [match[1].split() for line in lines
                    if (match := re.search(r"packet:\s+\S+< fetch=([^\r\n]+)$", line))]
    need(capabilities and all("filter" in row and "shallow" in row for row in capabilities),
         "server must advertise shallow filtered fetch before transfer")
    need(not any("filtering not recognized" in line for line in lines), "unfiltered fallback refused")


def parse_objects(raw):
    result = {}
    for line in raw.decode("ascii").splitlines():
        match = re.fullmatch(r"([0-9a-f]{40}) (commit|tree|blob) (0|[1-9][0-9]*)", line)
        need(match is not None and match[1] not in result, "exact canonical object inventory")
        result[match[1]] = {"type": match[2], "bytes": int(match[3])}
    need(result, "nonempty object inventory")
    return result


def check_initial_objects(objects, commit):
    need({oid for oid, row in objects.items() if row["type"] == "commit"} == {commit},
         "exact shallow approved commit")
    need(not any(row["type"] == "blob" for row in objects.values()), "blob:none was not honored")
    need(sum(row["bytes"] for row in objects.values()) <= INITIAL_PACK_LIMIT, "initial object ceiling")


def selected_blobs(raw):
    result = {}
    for line in raw.split(b"\0")[:-1]:
        metadata, name = line.split(b"\t", 1)
        mode, kind, oid = metadata.decode("ascii").split()
        name = canonical_name(name.decode("ascii"))
        need(mode in ("100644", "100755") and kind == "blob" and name not in result,
             "ordinary unique selected Git blob")
        need(re.fullmatch(r"[0-9a-f]{40}", oid), "Git object identity")
        result[name] = oid
    need(result and raw.endswith(b"\0"), "complete selected Git roster")
    return result


def sanitized_git(gitdir, commit):
    need(gitdir.is_dir() and not gitdir.is_symlink(), "owned Git directory")
    for relative in ("objects/info/alternates", "info/grafts", "refs/replace"):
        path = gitdir / relative
        need(not path.exists() and not path.is_symlink(), "Git alias/replacement metadata refused")
    packdir = gitdir / "objects/pack"
    need(packdir.is_dir() and packdir.resolve(strict=True) == packdir, "canonical owned pack directory")
    for path in packdir.iterdir():
        info = path.lstat()
        need(path.resolve(strict=True) == path and stat.S_ISREG(info.st_mode) and info.st_nlink == 1,
             "ordinary canonical single-link pack member")
        match = re.fullmatch(r"pack-[0-9a-f]{40}\.(pack|idx|rev|promisor)", path.name)
        need(match is not None, "exact generated pack member name")
        mode = stat.S_IMODE(info.st_mode)
        need(mode == 0o600 or (match[1] != "promisor" and mode == 0o400), "expected private pack mode")
        if mode == 0o400:
            before = sha(path)
            path.chmod(0o600)
            need(sha(path) == before, "pack bytes unchanged by owned mode normalization")
    for relative in ("FETCH_HEAD", "ORIG_HEAD", "hooks", "logs", "refs/remotes"):
        path = gitdir / relative
        if path.is_dir() and not path.is_symlink():
            shutil.rmtree(path)
        elif path.exists():
            need(path.is_file() and not path.is_symlink(), "ordinary owned Git metadata")
            path.unlink()
    (gitdir / "config").write_text("[core]\nrepositoryformatversion = 0\nbare = false\nfilemode = true\n", encoding="ascii")
    need((gitdir / "HEAD").read_text("ascii") == commit + "\n", "detached approved HEAD")
    need((gitdir / "shallow").read_text("ascii") == commit + "\n", "exact shallow boundary")


def prepare(args):
    native = native_module()
    hot, _, _ = native.load_helpers()
    output = native.fresh_output(args.output)
    os.umask(0o077)
    (output / "tmp").mkdir()
    source = output / "source"
    source.mkdir()
    rec = native.FreshRecorder(output / "commands", native.ROOT, hot)
    env = hot.environment(output)
    env["GIT_CONFIG_NOSYSTEM"] = "1"
    env["GIT_CONFIG_GLOBAL"] = "/dev/null"
    env["GIT_NO_REPLACE_OBJECTS"] = "1"

    def git(name, arguments, *, cwd=None, trace=False, cap=ARCHIVE_LIMIT):
        phase_env = dict(env)
        if trace:
            phase_env["GIT_TRACE_PACKET"] = "1"
        command = ["/usr/bin/python3", "-I", "-B", str(SCRIPT), "limited-git", str(cap),
                   *GIT[1:], *(["-C", str(cwd)] if cwd else []), *arguments]
        return rec.run(name, command, 300, env=phase_env)

    git("source-head", ["rev-parse", "HEAD"])
    need((rec.output / "source-head/stdout").read_bytes() == (args.commit + "\n").encode("ascii"), "source HEAD is approved commit")
    signer = output / "allowed-signers"
    signer.write_text(SIGNER + " " + SIGNER_KEY + "\n", encoding="ascii")
    verify = ["-c", "gpg.format=ssh", "-c", "gpg.ssh.allowedSignersFile=" + str(signer), "verify-commit", args.commit]
    folder = git("signature-before", verify)
    need(FINGERPRINT in (folder / "stderr").read_text("ascii") and SIGNER in (folder / "stderr").read_text("ascii"),
         "explicit approved signer")
    selected = git("selected-tree", ["ls-tree", "-rz", args.commit, "--", *native.SOURCE_ROOTS])
    expected = selected_blobs((selected / "stdout").read_bytes())
    url = native.ROOT.as_uri()
    capability = git("filter-capability", ["-c", "protocol.version=2", "ls-remote", "--upload-pack=" + UPLOAD_PACK, url], trace=True)
    require_filter_capability((capability / "stderr").read_bytes())
    git("init", ["init", "--quiet", str(source)])
    fetch_options = ["-c", "protocol.version=2", "-c", "remote.origin.url=" + url,
                     "-c", "remote.origin.uploadpack=" + UPLOAD_PACK,
                     "-c", "remote.origin.promisor=true", "-c", "remote.origin.partialclonefilter=blob:none"]
    folder = git("filtered-fetch", [*fetch_options, "fetch", "--no-tags", "--depth=1", "--filter=blob:none", "origin", args.commit],
                 cwd=source, trace=True, cap=INITIAL_PACK_LIMIT)
    require_filter_capability((folder / "stderr").read_bytes())
    need(b"filter blob:none" in (folder / "stderr").read_bytes(), "requested blob:none filter")
    objects_command = ["cat-file", "--batch-all-objects", "--batch-check=%(objectname) %(objecttype) %(objectsize)"]
    initial = git("initial-objects", objects_command, cwd=source)
    initial_objects = parse_objects((initial / "stdout").read_bytes())
    check_initial_objects(initial_objects, args.commit)
    git("detached-head", ["update-ref", "--no-deref", "HEAD", args.commit], cwd=source)
    git("sparse-enable", ["config", "core.sparseCheckout", "true"], cwd=source)
    git("sparse-noncone", ["config", "core.sparseCheckoutCone", "false"], cwd=source)
    patterns = "".join("/" + name + ("/" if (native.ROOT / name).is_dir() else "") + "\n" for name in native.SOURCE_ROOTS)
    (source / ".git/info/sparse-checkout").write_text(patterns, encoding="ascii")
    git("selected-checkout", [*fetch_options, "read-tree", "-mu", args.commit], cwd=source)
    final = git("populated-objects", objects_command, cwd=source)
    final_objects = parse_objects((final / "stdout").read_bytes())
    need({oid for oid, row in final_objects.items() if row["type"] == "blob"} == set(expected.values()),
         "only required selected blobs were materialized")
    need({oid: row for oid, row in final_objects.items() if row["type"] != "blob"} == initial_objects,
         "no historical or foreign objects")
    need(sum(row["bytes"] for row in final_objects.values() if row["type"] == "blob") <= SELECTED_LIMIT,
         "selected source byte ceiling")
    sanitized_git(source / ".git", args.commit)
    git("signature-relocated", verify, cwd=source)
    check = git("relocated-tree", ["ls-tree", "-rz", args.commit, "--", *native.SOURCE_ROOTS], cwd=source)
    need(selected_blobs((check / "stdout").read_bytes()) == expected, "all required objects survive isolation")
    source_files = inventory(source)
    need({name for name in source_files if not name.startswith(".git/")} == set(expected), "exact selected working tree")
    for name, oid in expected.items():
        data = (source / name).read_bytes()
        need(hashlib.sha1(b"blob " + str(len(data)).encode("ascii") + b"\0" + data).hexdigest() == oid, "selected blob materialization")
    files = {"source/" + name: row for name, row in source_files.items()}
    payload = output / "source.tar.gz"
    make_archive(payload, output, files)
    binding = {"schema": "fe2o3.xgmi-series-transport.v1", "commit": args.commit,
               "devices": args.devices, "files": files, "payload_sha256": sha(payload),
               "payload_bytes": payload.stat().st_size, "selected_objects": expected,
               "signer": SIGNER, "signer_fingerprint": FINGERPRINT}
    save(output / "binding.json", binding)
    marker = {"path": PREFIX + secrets.token_hex(8), "commit": args.commit,
              "binding_sha256": sha(output / "binding.json")}
    save(output / "owner.json", marker)
    save(output / "prepare-census.json", native.fresh_census(rec, hot, os.readlink("/proc/self/ns/pid")))
    print(json.dumps({"prepared": str(output), "owner": marker, "payload_bytes": binding["payload_bytes"]}, sort_keys=True))


def owned_path(marker, *, exists=True):
    need(type(marker) is dict and set(marker) == {"path", "commit", "binding_sha256"}, "ownership marker roster")
    need(type(marker["path"]) is str and re.fullmatch(re.escape(PREFIX) + r"[0-9a-f]{16}", marker["path"]), "private owner path")
    need(re.fullmatch(r"[0-9a-f]{40}", marker["commit"]) and re.fullmatch(r"[0-9a-f]{64}", marker["binding_sha256"]),
         "owner identities")
    path = Path(marker["path"])
    need(path.resolve() == path, "canonical private owner path")
    if exists:
        info = path.lstat()
        need(stat.S_ISDIR(info.st_mode) and info.st_uid == os.getuid() and stat.S_IMODE(info.st_mode) == 0o700,
             "private owned directory")
        need(parse_json((path / "owner.json").read_bytes()) == marker, "exact ownership marker")
    else:
        need(not path.exists() and not path.is_symlink(), "fresh private owner path")
    return path


def read_binding(owned, marker):
    need(sha(owned / "binding.json") == marker["binding_sha256"], "bound transport manifest")
    value = parse_json((owned / "binding.json").read_bytes())
    need(type(value) is dict and set(value) == {"schema", "commit", "devices", "files", "payload_sha256", "payload_bytes",
                                               "selected_objects", "signer", "signer_fingerprint"}, "binding roster")
    need(value["schema"] == "fe2o3.xgmi-series-transport.v1" and value["commit"] == marker["commit"]
         and value["signer"] == SIGNER and value["signer_fingerprint"] == FINGERPRINT, "approved source binding")
    need(type(value["payload_bytes"]) is int and 0 < value["payload_bytes"] <= ARCHIVE_LIMIT
         and re.fullmatch(r"[0-9a-f]{64}", value["payload_sha256"]), "bounded payload identity")
    bounded_manifest(value["files"])
    need(all(name.startswith("source/") for name in value["files"]), "source-only payload namespace")
    need("source/" + TRANSPORT_RELATIVE in value["files"] and "source/" + NATIVE_RELATIVE in value["files"], "repo-owned entrypoints")
    return value


def read_exact(stream, count):
    result = bytearray()
    while len(result) < count:
        block = stream.read(min(1024 * 1024, count - len(result)))
        need(block, "truncated transport")
        result.extend(block)
    return bytes(result)


def receive(marker, stream):
    owned = owned_path(marker, exists=False)
    os.umask(0o077)
    owned.mkdir(mode=0o700)
    save(owned / "owner.json", marker)
    size_raw = read_exact(stream, 16)
    need(re.fullmatch(rb"[0-9a-f]{16}", size_raw), "binding length frame")
    size = int(size_raw, 16)
    need(0 < size <= 8 * 1024**2, "bounded binding frame")
    with (owned / "binding.json").open("xb") as output:
        output.write(read_exact(stream, size))
    binding = read_binding(owned, marker)
    remaining = binding["payload_bytes"]
    with (owned / "source.tar.gz").open("xb") as output:
        while remaining:
            block = read_exact(stream, min(1024 * 1024, remaining))
            output.write(block)
            remaining -= len(block)
    need(stream.read(1) == b"", "no trailing transport bytes")
    need(sha(owned / "source.tar.gz") == binding["payload_sha256"], "transport archive digest")
    validate_archive(owned / "source.tar.gz", binding["files"], owned)
    verify_source(owned, binding)
    print(json.dumps({"received": marker, "payload_sha256": binding["payload_sha256"], "files": len(binding["files"])}, sort_keys=True))


def verify_source(owned, binding):
    expected = {name.removeprefix("source/"): row for name, row in binding["files"].items()}
    need(inventory(owned / "source") == expected, "exact immutable transported source/object closure")
    gitdir = owned / "source/.git"
    for name in ("objects/info/alternates", "info/grafts", "refs/replace", "refs/remotes"):
        need(not (gitdir / name).exists() and not (gitdir / name).is_symlink(), "no packaged Git aliases/remotes")
    need((gitdir / "config").read_text("ascii") == "[core]\nrepositoryformatversion = 0\nbare = false\nfilemode = true\n",
         "minimal isolated Git config")
    for name in ("HEAD", "shallow"):
        need((gitdir / name).read_text("ascii") == binding["commit"] + "\n", "approved shallow detached source")


def lock_identity(info):
    return {"device": info.st_dev, "inode": info.st_ino, "uid": info.st_uid,
            "mode": stat.S_IMODE(info.st_mode), "links": info.st_nlink}


@contextlib.contextmanager
def shared_lock(path=SHARED_LOCK, *, wait_seconds=LOCK_WAIT):
    initial = path.lstat()
    need(path.resolve(strict=True) == path and stat.S_ISREG(initial.st_mode) and initial.st_nlink == 1
         and initial.st_uid == os.getuid(), "existing canonical single-link shared lock")
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
    acquired = False
    started = time.monotonic()
    try:
        need(lock_identity(os.fstat(descriptor)) == lock_identity(initial), "opened shared lock identity")
        while True:
            try:
                fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
                acquired = True
                break
            except BlockingIOError:
                need(time.monotonic() - started < wait_seconds, "shared build queue wait exceeded")
                time.sleep(0.25)
        need(lock_identity(path.lstat()) == lock_identity(initial), "shared lock path replaced while waiting")
        try:
            yield {**lock_identity(initial), "path": str(path), "wait_seconds": time.monotonic() - started}
        finally:
            need(lock_identity(path.lstat()) == lock_identity(os.fstat(descriptor)) == lock_identity(initial),
                 "shared lock continuity")
    finally:
        if acquired:
            fcntl.flock(descriptor, fcntl.LOCK_UN)
        os.close(descriptor)


def resource_observation(owned):
    total = 0
    for path in owned.rglob("*"):
        need(not path.is_symlink(), "private scratch alias")
        if path.is_file():
            try:
                total += path.stat().st_size
            except FileNotFoundError:
                # Cargo may rename/remove its own temporary artifacts during observation.
                continue
    disk = os.statvfs(owned)
    memory = dict(line.split(":", 1) for line in Path("/proc/meminfo").read_text("ascii").splitlines())
    available = int(memory["MemAvailable"].split()[0]) * 1024
    free = disk.f_bavail * disk.f_frsize
    need(total <= PRIVATE_LIMIT and free >= DISK_RESERVE and available >= MEMORY_RESERVE, "shared-host resource guard")
    return {"private_bytes": total, "disk_available": free, "memory_available": available}


def resource_monitor(marker, parent_descriptor, stream):
    owned = owned_path(marker)
    deadline = time.monotonic() + CAMPAIGN_SECONDS
    try:
        while True:
            ready, _, _ = select.select([stream], [], [], 5)
            if ready:
                need(stream.read(1) == b"", "monitor accepts liveness EOF only")
                return 0
            need(time.monotonic() < deadline, "bounded native campaign wall clock")
            resource_observation(owned)
    except BaseException as error:
        print(repr(error), file=sys.stderr, flush=True)
        try:
            signal.pidfd_send_signal(parent_descriptor, signal.SIGTERM)
        except ProcessLookupError:
            pass
        return 1
    finally:
        os.close(parent_descriptor)


@contextlib.contextmanager
def resource_guard(owned, marker, hot):
    need(hasattr(os, "pidfd_open") and hasattr(signal, "pidfd_send_signal"), "pidfd resource monitor required")
    folder = owned / "monitor"
    folder.mkdir(mode=0o700)
    process = None
    receipt = {"pid": None, "exit": None, "error": None, "group_absent": False,
               "namespace": os.readlink("/proc/self/ns/pid"), "started_ns": time.time_ns()}
    try:
        with (folder / "stdout").open("xb") as stdout, (folder / "stderr").open("xb") as stderr:
            descriptor = os.pidfd_open(os.getpid())
            blocked = signal.pthread_sigmask(signal.SIG_BLOCK, hot.B.MANAGED)
            try:
                command = ["/usr/bin/python3", "-I", "-B", str(SCRIPT), "monitor",
                           encoded(marker).decode("ascii").strip(), str(descriptor)]
                receipt["command"] = command
                process = subprocess.Popen(command, cwd=owned, stdin=subprocess.PIPE,
                    stdout=stdout, stderr=stderr, start_new_session=True, pass_fds=(descriptor,),
                    preexec_fn=lambda: signal.pthread_sigmask(signal.SIG_SETMASK, blocked))
                receipt["pid"] = process.pid
            finally:
                os.close(descriptor)
                signal.pthread_sigmask(signal.SIG_SETMASK, blocked)
            try:
                yield receipt
            finally:
                process.stdin.close()
                process.wait(timeout=30)
                if hot.B.group_exists(process.pid):
                    hot.B.stop(process)
                receipt["exit"] = process.returncode
                receipt["group_absent"] = not hot.B.group_exists(process.pid)
                need(receipt["group_absent"], "fresh monitor group closed")
    except BaseException as error:
        receipt["error"] = repr(error)
        raise
    finally:
        try:
            if process is not None and (process.poll() is None or hot.B.group_exists(process.pid)):
                hot.B.stop(process)
            if process is not None:
                receipt["exit"] = process.returncode
                receipt["group_absent"] = not hot.B.group_exists(process.pid)
            else:
                receipt["group_absent"] = True
        finally:
            receipt["finished_ns"] = time.time_ns()
            save(folder / "receipt.json", receipt)


def replay_fresh_closure(owned, native, hot, namespace):
    path = owned / "campaign-1/fresh-census.json"
    value = parse_json(path.read_bytes())
    rec = types.SimpleNamespace(output=owned / "campaign-1/commands", attempts=value["attempt_order"], pins=value["raw_sha256"])
    need(native.fresh_census(rec, hot, namespace) == value, "original fresh closure/raw readback")
    return sha(path)


def remote_run(marker):
    owned = owned_path(marker)
    binding = read_binding(owned, marker)
    verify_source(owned, binding)
    need(SCRIPT == owned / "source" / TRANSPORT_RELATIVE, "bound remote entrypoint")
    native = native_module()
    hot, _, _ = native.load_helpers()
    namespace = os.readlink("/proc/self/ns/pid")
    previous = {sig: signal.getsignal(sig) for sig in hot.B.MANAGED}
    prior_argv = sys.argv
    failures = []
    closed = False
    save(owned / "launch-started.json", {"marker": marker, "pid": os.getpid(), "namespace": namespace,
                                        "started_ns": time.time_ns()})
    command = [str(owned / "source" / NATIVE_RELATIVE), "--campaign", "--commit", marker["commit"],
               "--output", str(owned / "campaign-1")]
    for row in binding["devices"]:
        command += ["--device", str(row["physical_index"]), row["unique_id"], row["pci_bdf"]]
    try:
        for sig in previous:
            signal.signal(sig, hot.B.interrupted)
        with shared_lock() as identity:
            save(owned / "lock.json", identity)
            save(owned / "resources-before.json", resource_observation(owned))
            with resource_guard(owned, marker, hot) as monitor:
                try:
                    # Same public __main__ path, in the lock-owning foreground process.
                    sys.argv = command
                    runpy.run_path(command[0], run_name="__main__")
                except BaseException as error:
                    failures.append(repr(error))
            if monitor["exit"] != 0 or monitor["error"] is not None or monitor["group_absent"] is not True:
                failures.append("resource monitor did not complete without a guard failure")
            try:
                census_sha = replay_fresh_closure(owned, native, hot, namespace)
                closed = True
            except BaseException as error:
                census_sha = None
                failures.append("fresh closure: " + repr(error))
            verify_source(owned, binding)
            save(owned / "resources-after.json", resource_observation(owned))
    except BaseException as error:
        failures.append(repr(error))
    finally:
        try:
            save(owned / "launch.json", {"marker": marker, "public_argv": command, "pid": os.getpid(),
                 "namespace": namespace, "finished_ns": time.time_ns(), "failures": failures,
                 "fresh_closure": closed, "fresh_census_sha256": census_sha if closed else None,
                 "original_foreground_execution_returned": True})
        finally:
            sys.argv = prior_argv
            for sig, handler in previous.items():
                signal.signal(sig, handler)
    return 1 if failures else 0


def collect(marker):
    owned = owned_path(marker)
    binding = read_binding(owned, marker)
    verify_source(owned, binding)
    files = {}
    for name in ("owner.json", "binding.json", "launch-started.json", "launch.json", "lock.json",
                 "resources-before.json", "resources-after.json"):
        if (owned / name).exists():
            files[name] = inventory_row(owned / name)
    campaign = owned / "campaign-1"
    monitor = owned / "monitor"
    if monitor.is_dir():
        for name, row in inventory(monitor).items():
            files["monitor/" + name] = row
    if campaign.is_dir():
        for path in sorted(campaign.rglob("*")):
            relative = path.relative_to(campaign)
            if relative.parts[0] == "work" or path.is_dir():
                continue
            files["campaign-1/" + relative.as_posix()] = inventory_row(path)
    archive = owned / "evidence.tar.gz"
    make_archive(archive, owned, files)
    value = {"marker": marker, "archive_sha256": sha(archive), "archive_bytes": archive.stat().st_size,
             "files": files, "excluded_private_scratch": "campaign-1/work"}
    save(owned / "collection.json", value)
    print(encoded(value).decode("ascii"), end="")


def pull(marker):
    owned = owned_path(marker)
    value = parse_json((owned / "collection.json").read_bytes())
    need(value["marker"] == marker and sha(owned / "evidence.tar.gz") == value["archive_sha256"], "collected archive identity")
    with (owned / "evidence.tar.gz").open("rb") as stream:
        shutil.copyfileobj(stream, sys.stdout.buffer, 1024 * 1024)


def cleanup(marker, archive_sha256):
    owned = owned_path(marker)
    collection = parse_json((owned / "collection.json").read_bytes())
    need(collection["marker"] == marker and collection["archive_sha256"] == archive_sha256
         and sha(owned / "evidence.tar.gz") == archive_sha256, "exact locally collected archive")
    need({name: inventory_row(owned / name) for name in collection["files"]} == collection["files"], "unchanged collected raw files")
    launch = parse_json((owned / "launch.json").read_bytes())
    need(launch["marker"] == marker and launch["original_foreground_execution_returned"] is True
         and launch["fresh_closure"] is True, "original fresh handle closure required")
    # This replays pinned bytes only; no recorded PID is ever probed or signalled.
    need(sha(owned / "campaign-1/fresh-census.json") == launch["fresh_census_sha256"], "bound original fresh census")
    monitor = parse_json((owned / "monitor/receipt.json").read_bytes())
    need(monitor["group_absent"] is True and monitor["namespace"] == launch["namespace"], "original monitor handle closed")
    shutil.rmtree(owned)
    need(not owned.exists() and not owned.is_symlink(), "private owner path removed")
    print(json.dumps({"removed": marker["path"], "archive_sha256": archive_sha256}, sort_keys=True))


def bootstrap_command(script, mode, marker, extra=()):
    need(0 < len(script) <= 128 * 1024, "bounded bootstrap source")
    bootstrap = "import sys; n=int(sys.argv.pop(1)); source=sys.stdin.buffer.read(n); assert len(source)==n; exec(compile(source,'<owned-transport>','exec'))"
    remote = ["/usr/bin/python3", "-I", "-B", "-c", bootstrap, str(len(script)), mode,
              encoded(marker).decode("ascii").strip(), *extra]
    return SSH + [shlex.join(remote)]


def independent_replay(readback, marker, planner):
    campaign = readback / "campaign-1"
    launch = parse_json((readback / "launch.json").read_bytes())
    census = parse_json((campaign / "fresh-census.json").read_bytes())
    need(launch["marker"] == marker and launch["fresh_closure"] is True
         and launch["namespace"] == census["namespace"]
         and launch["fresh_census_sha256"] == sha(campaign / "fresh-census.json"), "original namespace/closure join")
    monitor = parse_json((readback / "monitor/receipt.json").read_bytes())
    need(monitor["group_absent"] is True and monitor["namespace"] == launch["namespace"], "collected monitor closure")
    for name, pins in census["raw_sha256"].items():
        canonical_name(name)
        need({stream: sha(campaign / "commands" / name / stream) for stream in ("stdout", "stderr", "receipt.json")} == pins,
             "collected fresh raw artifact join")
    finished = parse_json((campaign / "finished.json").read_bytes())
    if not finished["native_execution"]:
        return {"accepted": False, "failures": finished["failures"]}
    need(not finished["failures"] and not launch["failures"], "complete native success")
    plan = parse_json((campaign / "plan.json").read_bytes())
    records = parse_json((campaign / "records.json").read_bytes())
    import base64
    decoded = []
    for row in records:
        value = {key: item for key, item in row.items() if key not in ("stdout_base64", "stderr_base64")}
        value.update({stream: base64.b64decode(row[stream + "_base64"], validate=True) for stream in ("stdout", "stderr")})
        folder = campaign / "commands" / value["name"]
        receipt = parse_json((folder / "receipt.json").read_bytes())
        need(sha(folder / "receipt.json") == value["execution_receipt_sha256"]
             and receipt["exit"] == 0 and receipt["error"] is None and receipt["group_absent"] is True,
             "successful exact workload occurrence")
        expected_command = [marker["path"] + "/campaign-1/binaries/" + value["binary"], *value["arguments"]]
        need(receipt["command"] == expected_command and receipt["cwd"] == marker["path"] + "/source", "native command/source join")
        need(type(receipt["environment"]) is dict and all(
            receipt["environment"].get(key) == value["environment_overrides"].get(key)
            for key in value["clear_environment"]), "actual workload visibility/override join")
        for stream in ("stdout", "stderr"):
            need(value[stream] == (folder / stream).read_bytes(), "native stream/record join")
        decoded.append(value)
    result = planner.replay_campaign(decoded,
        environment_after=parse_json((campaign / "host-after.json").read_bytes())["environment"],
        physical_indices=plan["physical_indices"], unique_ids=plan["unique_ids"],
        admission=parse_json((campaign / "admission.json").read_bytes()),
        environment_before=parse_json((campaign / "host-before.json").read_bytes())["environment"],
        copy_bytes=plan["copy_bytes"], warmups=plan["warmups"], samples=plan["samples"])
    need(result == parse_json((campaign / "replay.json").read_bytes()), "independent 18-trial replay")
    return {"accepted": True, "trials": len(result["trials"]), "replay_sha256": digest(result)}


def execute(args):
    prepared = args.prepared
    need(prepared.is_absolute() and prepared.resolve(strict=True) == prepared, "canonical prepared payload")
    marker = parse_json((prepared / "owner.json").read_bytes())
    binding = read_binding(prepared, marker)
    verify_source(prepared, binding)
    need(SCRIPT == prepared / "source" / TRANSPORT_RELATIVE, "execute the authenticated relocated wrapper")
    need(sha(prepared / "source.tar.gz") == binding["payload_sha256"], "unchanged local payload")
    native = native_module()
    hot, planner, _ = native.load_helpers()
    rec = native.FreshRecorder(prepared / "remote-commands", prepared, hot)
    script = SCRIPT.read_bytes()
    payload = (prepared / "source.tar.gz").read_bytes()
    binding_raw = (prepared / "binding.json").read_bytes()
    wire = script + f"{len(binding_raw):016x}".encode("ascii") + binding_raw + payload
    failures = []
    previous = {sig: signal.getsignal(sig) for sig in hot.B.MANAGED}
    for sig in previous:
        signal.signal(sig, hot.B.interrupted)
    run_known_terminal = False
    try:
        rec.run("receive", bootstrap_command(script, "receive", marker), 600, stdin=wire)
        command = SSH + [shlex.join(["/usr/bin/python3", "-I", "-B",
                    marker["path"] + "/source/" + TRANSPORT_RELATIVE, "remote-run", encoded(marker).decode("ascii").strip()])]
        try:
            rec.run("remote-run", command, 7200)
        except BaseException as error:
            failures.append(repr(error))
        receipt = parse_json((rec.output / "remote-run/receipt.json").read_bytes())
        run_known_terminal = receipt["exit"] in (0, 1) and receipt["error"] is None and receipt["group_absent"] is True
        need(run_known_terminal, "original remote execution terminal is uncertain; retain private scratch")
        folder = rec.run("collect", bootstrap_command(script, "collect", marker), 180, stdin=script)
        collection = parse_json((folder / "stdout").read_bytes())
        need(collection["marker"] == marker, "collected owner join")
        folder = rec.run("pull", bootstrap_command(script, "pull", marker), 180, stdin=script)
        archive = folder / "stdout"
        need(sha(archive) == collection["archive_sha256"] and archive.stat().st_size == collection["archive_bytes"], "raw archive readback")
        destination = prepared / "readback"
        destination.mkdir(mode=0o700)
        validate_archive(archive, collection["files"], destination)
        replay = independent_replay(destination, marker, planner)
        save(prepared / "independent-replay.json", replay)
        rec.run("cleanup", bootstrap_command(script, "cleanup", marker, [collection["archive_sha256"]]), 180, stdin=script)
        rec.run("absence", bootstrap_command(script, "absence", marker), 30, stdin=script)
        need(replay["accepted"], "native campaign did not qualify")
    except BaseException as error:
        failures.append(repr(error))
    finally:
        try:
            save(prepared / "transport-census.json", native.fresh_census(rec, hot, os.readlink("/proc/self/ns/pid")))
            save(prepared / "transport-finished.json", {"marker": marker, "failures": failures,
                "original_remote_terminal": run_known_terminal, "accepted": not failures})
        finally:
            for sig, handler in previous.items():
                signal.signal(sig, handler)
    need(not failures, "transport/campaign failed: " + repr(failures))


def main():
    need(sys.flags.isolated and sys.flags.dont_write_bytecode, "use python3 -I -B")
    if len(sys.argv) >= 3 and sys.argv[1] == "limited-git":
        cap = int(sys.argv[2])
        need(cap in (INITIAL_PACK_LIMIT, ARCHIVE_LIMIT), "fixed Git file-size ceiling")
        resource.setrlimit(resource.RLIMIT_FSIZE, (cap, cap))
        os.execve(GIT[0], [GIT[0], *sys.argv[3:]], dict(os.environ))
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_subparsers(dest="mode", required=True)
    local = modes.add_parser("prepare")
    local.add_argument("--commit", required=True)
    local.add_argument("--output", type=Path, required=True)
    local.add_argument("--device", nargs=3, required=True, action="append")
    execute_parser = modes.add_parser("execute")
    execute_parser.add_argument("--prepared", type=Path, required=True)
    for mode in ("receive", "remote-run", "collect", "pull", "cleanup", "absence", "monitor"):
        remote = modes.add_parser(mode)
        remote.add_argument("marker")
        if mode == "cleanup":
            remote.add_argument("archive_sha256")
        if mode == "monitor":
            remote.add_argument("parent_descriptor", type=int)
    args = parser.parse_args()
    if args.mode == "prepare":
        command = ["--campaign", "--commit", args.commit, "--output", str(args.output)]
        for values in args.device:
            command += ["--device", *values]
        args.devices = native_module().parse_arguments(command).devices
        prepare(args)
    elif args.mode == "execute":
        execute(args)
    else:
        marker = parse_json(args.marker)
        if args.mode == "receive":
            receive(marker, sys.stdin.buffer)
        elif args.mode == "remote-run":
            return remote_run(marker)
        elif args.mode == "collect":
            collect(marker)
        elif args.mode == "pull":
            pull(marker)
        elif args.mode == "cleanup":
            cleanup(marker, args.archive_sha256)
        elif args.mode == "monitor":
            return resource_monitor(marker, args.parent_descriptor, sys.stdin.buffer)
        else:
            owned_path(marker, exists=False)
            print(json.dumps({"path_absent": marker["path"]}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
