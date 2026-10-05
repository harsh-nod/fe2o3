#!/usr/bin/python3
"""Portable, inert application inputs for the genuine qualification campaign.

This manifest measures transport contents, not compiler or runtime authority.
Production admission must still pin and retain the installed resources.
"""

from __future__ import annotations

import argparse
from contextlib import contextmanager
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import selectors
import stat
import subprocess
import sys
import time

from compiler_evidence_hardening import (
    HardeningError, MAX_CLOSURE_TOTAL_BYTES, MAX_SNAPSHOT_FILES,
    MAX_SNAPSHOT_FILE_BYTES, READ_CHUNK, canonical_json, hash_fd, stat_identity,
)

SCHEMA = "fe2o3-qualification-application-inputs-v1"
MANIFEST = "MANIFEST.json"
MAX_MANIFEST_BYTES = 32 * 1024 * 1024
MAX_DEPTH = 64
MAX_PATH_BYTES = 4096
GIT_TIMEOUT_SECONDS = 30
DIR_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
FILE_FLAGS = os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW | os.O_CLOEXEC
FIXTURE = "crates/cargo-fe2o3/tests/fixtures/conditional-custodian-application"
PIN_NAMES = (
    "FE2O3_AUTHORITY_CARGO_SHA256_V1", "FE2O3_AUTHORITY_RUSTC_SHA256_V1",
    "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1", "FE2O3_AUTHORITY_BACKEND_SHA256_V1",
    "FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_SHA256_V1",
)
TOOLS = {
    "cargo-fe2o3": "FE2O3_GENUINE_CARGO_FE2O3",
    "librustc_codegen_fe2o3.so": "FE2O3_BACKEND",
    "binding-trampoline": "FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_PATH_V1",
    "jq": "FE2O3_GENUINE_JQ",
}


def fail(message: str) -> None:
    raise HardeningError(message)


@dataclass
class CopyBudget:
    entries: int = 0
    bytes: int = 0

    def charge(self, inventory: dict[str, os.stat_result]) -> None:
        self.entries += len(inventory)
        self.bytes += sum(i.st_size for i in inventory.values() if stat.S_ISREG(i.st_mode))
        if self.entries > MAX_SNAPSHOT_FILES or self.bytes > MAX_CLOSURE_TOTAL_BYTES:
            fail("aggregate copy budget exceeded")


def bounded_names(directory: int, remaining: int) -> list[str]:
    names = []
    with os.scandir(directory) as entries:
        for entry in entries:
            if len(names) >= remaining:
                fail("directory exceeds entry bound")
            names.append(entry.name)
    return sorted(names)


def relative_path(value: str) -> str:
    if not isinstance(value, str) or not value or "\0" in value:
        fail("invalid relative path")
    parts = value.split("/")
    if len(parts) > MAX_DEPTH or any(part in ("", ".", "..") for part in parts):
        fail("noncanonical relative path")
    if len(value.encode("utf-8")) > MAX_PATH_BYTES:
        fail("relative path exceeds bound")
    return value


def canonical_directory(path: Path) -> Path:
    if not path.is_absolute() or path.resolve(strict=True) != path:
        fail(f"noncanonical input directory: {path}")
    if not stat.S_ISDIR(path.lstat().st_mode):
        fail(f"input is not a directory: {path}")
    return path


@contextmanager
def directory_at(root: int, relative: str = ""):
    fd = os.dup(root)
    try:
        for name in relative.split("/") if relative else ():
            child = os.open(name, DIR_FLAGS, dir_fd=fd)
            os.close(fd)
            fd = child
        yield fd
    finally:
        os.close(fd)


@contextmanager
def file_at(root: int, relative: str, *, hardlinks: bool = False):
    relative_path(relative)
    parent, _, name = relative.rpartition("/")
    with directory_at(root, parent) as directory:
        fd = os.open(name, FILE_FLAGS, dir_fd=directory)
        try:
            info = os.fstat(fd)
            if not stat.S_ISREG(info.st_mode) or (not hardlinks and info.st_nlink != 1):
                fail(f"not a single-link regular file: {relative}")
            if info.st_size > MAX_SNAPSHOT_FILE_BYTES:
                fail(f"file exceeds size bound: {relative}")
            yield fd, info
            if stat_identity(os.fstat(fd)) != stat_identity(info) or stat_identity(
                os.stat(name, dir_fd=directory, follow_symlinks=False)
            ) != stat_identity(info):
                fail(f"file changed during inspection: {relative}")
        finally:
            os.close(fd)


def inventory(root: int, *, hardlinks: bool = False) -> dict[str, os.stat_result]:
    result: dict[str, os.stat_result] = {}
    total_bytes = 0

    def walk(directory: int, path: str) -> None:
        nonlocal total_bytes
        initial = os.fstat(directory)
        result[path] = initial
        for name in bounded_names(directory, MAX_SNAPSHOT_FILES - len(result)):
            relative = relative_path(f"{path}/{name}" if path else name)
            if len(result) >= MAX_SNAPSHOT_FILES:
                fail("inventory exceeds entry bound")
            info = os.stat(name, dir_fd=directory, follow_symlinks=False)
            if stat.S_ISDIR(info.st_mode):
                with directory_at(directory, name) as child:
                    if stat_identity(os.fstat(child)) != stat_identity(info):
                        fail(f"directory changed during enumeration: {relative}")
                    walk(child, relative)
            elif stat.S_ISREG(info.st_mode) and (hardlinks or info.st_nlink == 1):
                if info.st_size > MAX_SNAPSHOT_FILE_BYTES:
                    fail(f"file exceeds size bound: {relative}")
                total_bytes += info.st_size
                if total_bytes > MAX_CLOSURE_TOTAL_BYTES:
                    fail("inventory exceeds total byte bound")
                result[relative] = info
            else:
                fail(f"unsupported inventory entry: {relative}")
        if stat_identity(os.fstat(directory)) != stat_identity(initial):
            fail(f"directory changed during enumeration: {path}")

    walk(root, "")
    return result


def copied_mode(info: os.stat_result, policy: str) -> int:
    mode = stat.S_IMODE(info.st_mode)
    if mode & 0o7000:
        fail("special permission bits are not supported")
    directory = stat.S_ISDIR(info.st_mode)
    if policy == "exact":
        required = 0o005 if directory else 0o004
        if mode & required != required:
            fail("exact-mode toolchain is not publicly readable/searchable")
        return mode
    if policy != "read-only":
        fail("unknown mode policy")
    return 0o555 if directory or mode & 0o111 else 0o444


def copy_file(source: int, target: int, path: str, expected: os.stat_result,
              mode: int, *, hardlinks: bool) -> None:
    parent, _, name = path.rpartition("/")
    with file_at(source, path, hardlinks=hardlinks) as (opened, info):
        if stat_identity(info) != stat_identity(expected):
            fail(f"source changed before copy: {path}")
        digest = hash_fd(opened, info.st_size)
        with directory_at(target, parent) as directory:
            out = os.open(name, os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW
                          | os.O_CLOEXEC, 0o600, dir_fd=directory)
            try:
                offset = 0
                while offset < info.st_size:
                    chunk = os.pread(opened, min(READ_CHUNK, info.st_size - offset), offset)
                    if not chunk:
                        fail(f"source truncated during copy: {path}")
                    view = memoryview(chunk)
                    while view:
                        written = os.write(out, view)
                        if written <= 0:
                            fail("copy made no progress")
                        view = view[written:]
                    offset += len(chunk)
                if hash_fd(out, info.st_size) != digest:
                    fail(f"copied bytes differ: {path}")
                os.fchmod(out, mode)
                os.fsync(out)
            finally:
                os.close(out)


def copy_tree(source: Path, destination: Path, policy: str, *,
              selected: list[str] | None = None, hardlinks: bool = False,
              budget: CopyBudget | None = None) -> None:
    """Bounded transport copy; does not retain custody after returning."""
    canonical_directory(source)
    source_fd = os.open(source, DIR_FLAGS)
    try:
        if selected is None:
            before = inventory(source_fd, hardlinks=hardlinks)
        else:
            names = {""}
            for raw in selected:
                path = relative_path(raw)
                names.add(path)
                names.update(str(p) for p in Path(path).parents if str(p) != ".")
                if len(names) > MAX_SNAPSHOT_FILES:
                    fail("selected source exceeds entry bound")
            before = {}
            for path in sorted(names):
                if not path:
                    before[path] = os.fstat(source_fd)
                    continue
                parent, _, name = path.rpartition("/")
                with directory_at(source_fd, parent) as directory:
                    info = os.stat(name, dir_fd=directory, follow_symlinks=False)
                    if not stat.S_ISDIR(info.st_mode) and not stat.S_ISREG(info.st_mode):
                        fail(f"unsupported selected source: {path}")
                    if stat.S_ISREG(info.st_mode) and not hardlinks and info.st_nlink != 1:
                        fail(f"hardlinked selected source: {path}")
                    before[path] = info
        if sum(i.st_size for i in before.values() if stat.S_ISREG(i.st_mode)) > MAX_CLOSURE_TOTAL_BYTES:
            fail("selected source exceeds byte bound")
        (budget if budget is not None else CopyBudget()).charge(before)
        modes = {p: copied_mode(i, policy) for p, i in before.items()}
        destination.mkdir(mode=0o700)
        target_fd = os.open(destination, DIR_FLAGS)
        try:
            for path, info in sorted(before.items()):
                if not path:
                    continue
                if stat.S_ISDIR(info.st_mode):
                    parent, _, name = path.rpartition("/")
                    with directory_at(target_fd, parent) as directory:
                        os.mkdir(name, mode=0o700, dir_fd=directory)
                else:
                    copy_file(source_fd, target_fd, path, info, modes[path], hardlinks=hardlinks)
            for path, info in before.items():
                parent, _, name = path.rpartition("/")
                with directory_at(source_fd, parent) as directory:
                    now = os.stat(name, dir_fd=directory, follow_symlinks=False) if path else os.fstat(source_fd)
                    if stat_identity(now) != stat_identity(info):
                        fail(f"source changed during snapshot: {path}")
            for path in sorted(before, key=lambda p: len(Path(p).parts), reverse=True):
                if stat.S_ISDIR(before[path].st_mode):
                    with directory_at(target_fd, path) as directory:
                        os.fchmod(directory, modes[path])
        finally:
            os.close(target_fd)
    finally:
        os.close(source_fd)


def tree_records(root: Path) -> list[dict]:
    fd = os.open(canonical_directory(root), DIR_FLAGS)
    try:
        before = inventory(fd)
        records = []
        for path, info in sorted(before.items()):
            mode = stat.S_IMODE(info.st_mode)
            if mode & 0o7000:
                fail("special permission bits are not supported")
            record = {"path": path, "mode": mode}
            if stat.S_ISDIR(info.st_mode):
                record["kind"] = "directory"
            else:
                with file_at(fd, path) as (file, current):
                    if stat_identity(current) != stat_identity(info):
                        fail(f"inventory changed before hashing: {path}")
                    record.update(kind="file", bytes=info.st_size, sha256=hash_fd(file, info.st_size))
            records.append(record)
        after = inventory(fd)
        if {p: stat_identity(i) for p, i in before.items()} != {p: stat_identity(i) for p, i in after.items()}:
            fail("inventory changed while measured")
        return records
    finally:
        os.close(fd)


def unique_object(pairs: list[tuple[str, object]]) -> dict:
    result = {}
    for key, value in pairs:
        if key in result:
            fail(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def load_json(path: Path, limit: int = MAX_MANIFEST_BYTES,
              expected_sha256: str | None = None) -> tuple[bytes, dict]:
    parent = canonical_directory(path.parent)
    fd = os.open(parent, DIR_FLAGS)
    try:
        with file_at(fd, path.name) as (file, info):
            if info.st_size > limit:
                fail("JSON input exceeds bound")
            data = os.pread(file, info.st_size + 1, 0)
            if len(data) != info.st_size:
                fail("JSON input truncated")
    finally:
        os.close(fd)
    if expected_sha256 is not None and hashlib.sha256(data).hexdigest() != expected_sha256:
        fail("manifest SHA256 differs")
    value = json.loads(data, object_pairs_hook=unique_object,
                       parse_constant=lambda value: fail(f"nonfinite JSON: {value}"))
    if not isinstance(value, dict):
        fail("JSON object required")
    return data, value


def manifest_for(data: Path, metadata: dict) -> dict:
    entries = tree_records(data)
    return {"schema": SCHEMA, "metadata": metadata, "entries": entries,
            "entry_count": len(entries),
            "total_bytes": sum(r.get("bytes", 0) for r in entries)}


def verify(bundle: Path, expected_sha256: str) -> dict:
    if not re.fullmatch(r"[0-9a-f]{64}", expected_sha256):
        fail("independent manifest SHA256 required")
    canonical_directory(bundle)
    root = os.open(bundle, DIR_FLAGS)
    try:
        if bounded_names(root, 2) != [MANIFEST, "data"] or stat.S_IMODE(os.fstat(root).st_mode) != 0o555:
            fail("bundle outer inventory/mode differs")
    finally:
        os.close(root)
    raw, value = load_json(bundle / MANIFEST, expected_sha256=expected_sha256)
    if canonical_json(value) != raw or set(value) != {"schema", "metadata", "entries", "entry_count", "total_bytes"}:
        fail("noncanonical manifest schema")
    if value.get("schema") != SCHEMA or not isinstance(value.get("metadata"), dict):
        fail("manifest profile differs")
    if stat.S_IMODE((bundle / MANIFEST).stat().st_mode) != 0o444:
        fail("manifest mode differs")
    # Compare against a fresh complete canonical inventory, not paths supplied by
    # the manifest. This also rejects aliases, duplicates, extras and unknown keys.
    actual = manifest_for(bundle / "data", value["metadata"])
    if canonical_json(actual) != raw:
        fail("bundle content/inventory differs")
    return value["metadata"]


def remove_owned_tree(path: Path, identity: tuple[int, int]) -> None:
    """Clean only the original created directory, never following substituted links."""
    def clean(directory: int) -> None:
        os.fchmod(directory, 0o700)
        with os.scandir(directory) as entries:
            for entry in entries:
                if entry.is_dir(follow_symlinks=False):
                    with directory_at(directory, entry.name) as child:
                        clean(child)
                    os.rmdir(entry.name, dir_fd=directory)
                else:
                    os.unlink(entry.name, dir_fd=directory)

    with_path = os.open(path, DIR_FLAGS)
    try:
        if stat_identity(os.fstat(with_path))[:2] != identity:
            fail("owned cleanup root was replaced")
        clean(with_path)
        if stat_identity(path.lstat())[:2] != identity:
            fail("owned cleanup root changed")
        path.rmdir()
    finally:
        os.close(with_path)


def publish_manifest(bundle: Path, metadata: dict) -> str:
    raw = canonical_json(manifest_for(bundle / "data", metadata))
    if len(raw) > MAX_MANIFEST_BYTES:
        fail("manifest exceeds size bound")
    with (bundle / MANIFEST).open("xb") as output:
        output.write(raw)
        output.flush()
        os.fsync(output.fileno())
    (bundle / MANIFEST).chmod(0o444)
    bundle.chmod(0o555)
    digest = hashlib.sha256(raw).hexdigest()
    verify(bundle, digest)
    return digest


def prepare_tree(source: Path, bundle: Path, policy: str) -> str:
    canonical_directory(bundle.parent)
    if bundle.is_relative_to(source):
        fail("tree bundle must be outside its source")
    bundle.mkdir(mode=0o700)
    identity = stat_identity(bundle.stat())[:2]
    try:
        copy_tree(source, bundle / "data", policy)
        return publish_manifest(bundle, {"profile": "tree", "mode_policy": policy})
    except BaseException:
        remove_owned_tree(bundle, identity)
        raise


def tracked_source_paths(repo: Path) -> list[str]:
    # Git metadata is input discovery, not a command supplied by the bundle.
    command = ["/usr/bin/git", "--no-optional-locks", "-C", str(repo), "ls-files", "-z",
               "--", ".", ":(exclude)docs/evidence/**"]
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                               env={"PATH": "/usr/bin:/bin", "LANG": "C", "LC_ALL": "C",
                                    "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null"})
    output = bytearray()
    deadline = time.monotonic() + GIT_TIMEOUT_SECONDS
    try:
        assert process.stdout is not None
        os.set_blocking(process.stdout.fileno(), False)
        with selectors.DefaultSelector() as selector:
            selector.register(process.stdout, selectors.EVENT_READ)
            while selector.get_map():
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    fail("source inventory command timed out")
                for key, _ in selector.select(remaining):
                    chunk = os.read(key.fd, 65536)
                    if not chunk:
                        selector.unregister(key.fd)
                        break
                    output.extend(chunk)
                    if len(output) > MAX_MANIFEST_BYTES:
                        fail("source inventory command exceeded output bound")
            if process.wait(timeout=max(0, deadline - time.monotonic())) != 0:
                fail("source inventory command failed")
    finally:
        if process.poll() is None:
            process.kill()
        process.wait()
        if process.stdout is not None:
            process.stdout.close()
    if not output.endswith(b"\0"):
        fail("source inventory is empty or truncated")
    paths = output.decode().split("\0")[:-1]
    if len(paths) > MAX_SNAPSHOT_FILES or paths != sorted(set(paths)):
        fail("source inventory is oversized, duplicate or unsorted")
    return paths


def prepare_application(repo: Path, environment: dict, bundle: Path) -> str:
    canonical_directory(repo)
    if not re.fullmatch(r"/home/(?:[A-Za-z0-9_.-]+/)+[A-Za-z0-9_.-]+", str(repo)):
        fail("source projection requires a canonical path below a private home")
    for key in PIN_NAMES:
        if not re.fullmatch(r"[0-9a-f]{64}", environment.get(key, "")):
            fail(f"missing authority pin: {key}")
    rustc = Path(environment["FE2O3_AUTHORITY_RUSTC_PATH_V1"])
    if rustc.name != "rustc" or rustc.parent.name != "bin":
        fail("rustc must use the bin/rustc plus sibling lib layout")
    rust = rustc.parent.parent
    if Path(environment["CARGO"]) != rust / "bin/cargo":
        fail("Cargo must belong to the selected compiler toolchain")
    if Path(environment["FE2O3_GENUINE_APPLICATION"]) != repo / FIXTURE:
        fail("application is not the qualification fixture at the source root")
    paths = tracked_source_paths(repo)
    if not paths or "Cargo.toml" not in paths or f"{FIXTURE}/Cargo.lock" not in paths:
        fail("source inventory lacks the fixture/workspace manifests")
    canonical_directory(bundle.parent)
    bundle.mkdir(mode=0o700)
    identity = stat_identity(bundle.stat())[:2]
    try:
        budget = CopyBudget(entries=8)
        data = bundle / "data"
        data.mkdir(mode=0o700)
        copy_tree(repo, data / "source", "read-only", selected=paths, budget=budget)
        (data / "rust").mkdir(mode=0o700)
        copy_tree(rust / "lib", data / "rust/lib", "exact", budget=budget)
        copy_tree(rust / "bin", data / "rust/bin", "read-only", selected=["cargo", "rustc"], budget=budget)
        (data / "rust").chmod(0o555)
        (data / "cargo").mkdir(mode=0o700)
        for part in ("registry", "git"):
            copy_tree(Path(environment[f"FE2O3_GENUINE_CARGO_{part.upper()}"]),
                      data / f"cargo/{part}", "read-only", hardlinks=part == "git", budget=budget)
        (data / "cargo").chmod(0o555)
        tools = data / "tools"
        tools.mkdir(mode=0o700)
        for name, variable in TOOLS.items():
            source = Path(environment[variable])
            temporary = tools / f".{name}"
            copy_tree(source.parent, temporary, "read-only", selected=[source.name],
                      hardlinks=True, budget=budget)
            # Renaming these private copied files never changes the original input.
            temporary.chmod(0o700)
            (temporary / source.name).rename(tools / name)
            temporary.rmdir()
        tools.chmod(0o555)
        pins = {key: environment[key] for key in PIN_NAMES}
        pinned_files = {
            "FE2O3_AUTHORITY_CARGO_SHA256_V1": "rust/bin/cargo",
            "FE2O3_AUTHORITY_RUSTC_SHA256_V1": "rust/bin/rustc",
            "FE2O3_AUTHORITY_BACKEND_SHA256_V1": "tools/librustc_codegen_fe2o3.so",
            "FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_SHA256_V1": "tools/binding-trampoline",
        }
        descriptor = os.open(data, DIR_FLAGS)
        try:
            for key, path in pinned_files.items():
                with file_at(descriptor, path) as (file, info):
                    if hash_fd(file, info.st_size) != pins[key]:
                        fail(f"copied authority input differs: {key}")
        finally:
            os.close(descriptor)
        data.chmod(0o555)
        return publish_manifest(bundle, {
            "profile": "genuine-application", "source_root": str(repo),
            "fixture": FIXTURE, "pins": pins,
            "source_excluded_prefix": "docs/evidence/",
            "host_linker": "installed-host-premise-not-in-this-bundle",
        })
    except BaseException:
        remove_owned_tree(bundle, identity)
        raise


def verify_application(bundle: Path, expected_sha256: str, environment: dict) -> str:
    metadata = verify(bundle, expected_sha256)
    if set(metadata) != {"profile", "source_root", "fixture", "pins",
                         "source_excluded_prefix", "host_linker"}:
        fail("application metadata schema differs")
    if metadata["profile"] != "genuine-application" or metadata["fixture"] != FIXTURE:
        fail("application metadata profile differs")
    if metadata["source_excluded_prefix"] != "docs/evidence/" or metadata["host_linker"] != "installed-host-premise-not-in-this-bundle":
        fail("application metadata scope differs")
    source_root = metadata["source_root"]
    if not isinstance(source_root, str) or not re.fullmatch(r"/home/(?:[A-Za-z0-9_.-]+/)+[A-Za-z0-9_.-]+", source_root):
        fail("invalid application source projection root")
    relative_path(source_root[1:])
    pins = metadata["pins"]
    if not isinstance(pins, dict) or set(pins) != set(PIN_NAMES):
        fail("application pin roster differs")
    for key in PIN_NAMES:
        if not isinstance(pins[key], str) or not re.fullmatch(r"[0-9a-f]{64}", pins[key]) or environment.get(key) != pins[key]:
            fail(f"application pin differs from caller: {key}")
    return source_root


def stage_application(source: Path, expected_sha256: str, destination: Path,
                      environment: dict) -> str:
    if os.geteuid() != 0 or os.getegid() != 0:
        fail("private application staging requires real root credentials")
    parent = canonical_directory(destination.parent).stat()
    if parent.st_uid != 0 or parent.st_gid != 0 or parent.st_mode & 0o022:
        fail("private stage parent must be root-owned and non-writable by other roles")
    source_root = verify_application(source, expected_sha256, environment)
    # Fresh inodes sever every transport alias. Exact modes retain the runtime
    # transcript; root ownership and the private mount enforce role separation.
    if destination.exists() or destination.is_symlink():
        fail("private stage destination already exists")
    descriptor = os.open(source, DIR_FLAGS)
    try:
        entries = inventory(descriptor)
    finally:
        os.close(descriptor)
    capacity = os.statvfs(destination.parent)
    required = sum(i.st_size for i in entries.values() if stat.S_ISREG(i.st_mode))
    required += len(entries) * max(capacity.f_frsize, 4096)
    if required > capacity.f_bavail * capacity.f_frsize:
        fail("insufficient space for private application stage")
    destination.mkdir(mode=0o700)
    identity = stat_identity(destination.stat())[:2]
    try:
        copy_tree(source, destination / "copied", "exact")
        staged = destination / "copied"
        if verify_application(staged, expected_sha256, environment) != source_root:
            fail("private staged source projection changed")
        fd = os.open(staged, DIR_FLAGS)
        try:
            if any(i.st_uid != 0 or i.st_gid != 0 for i in inventory(fd).values()):
                fail("private stage contains foreign-owned inputs")
        finally:
            os.close(fd)
        # The parent stays root-owned; only the closed copied tree is exposed.
        destination.chmod(0o555)
        return source_root
    except BaseException:
        remove_owned_tree(destination, identity)
        raise


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    tree = commands.add_parser("prepare-tree")
    tree.add_argument("source", type=Path)
    tree.add_argument("bundle", type=Path)
    tree.add_argument("policy", choices=("exact", "read-only"))
    app = commands.add_parser("prepare-application")
    app.add_argument("repo", type=Path)
    app.add_argument("environment", type=Path)
    app.add_argument("bundle", type=Path)
    check = commands.add_parser("verify")
    check.add_argument("bundle", type=Path)
    check.add_argument("sha256")
    projection = commands.add_parser("verify-application")
    projection.add_argument("bundle", type=Path)
    projection.add_argument("sha256")
    stage = commands.add_parser("stage-application")
    stage.add_argument("bundle", type=Path)
    stage.add_argument("sha256")
    stage.add_argument("destination", type=Path)
    args = parser.parse_args()
    if args.command == "prepare-tree":
        print(prepare_tree(args.source, args.bundle, args.policy))
    elif args.command == "prepare-application":
        _, environment = load_json(args.environment, 65536)
        print(prepare_application(args.repo, environment, args.bundle))
    elif args.command == "verify":
        print(canonical_json(verify(args.bundle, args.sha256)).decode(), end="")
    elif args.command == "verify-application":
        print(verify_application(args.bundle, args.sha256, dict(os.environ)))
    else:
        print(stage_application(args.bundle, args.sha256, args.destination, dict(os.environ)))


if __name__ == "__main__":
    try:
        main()
    except (HardeningError, OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"qualification input bundle: {error}", file=sys.stderr)
        sys.exit(1)
