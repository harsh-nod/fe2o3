#!/usr/bin/python3.12
"""Fail-closed primitives for the non-production compiler evidence lane."""

from __future__ import annotations

import ctypes
import errno
import fcntl
import hashlib
import json
import os
import resource
import selectors
import signal
import stat
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable, Mapping, Sequence


PR_SET_CHILD_SUBREAPER = 36
PR_SET_NO_NEW_PRIVS = 38
LANDLOCK_CREATE_RULESET_VERSION = 1
LANDLOCK_RULE_PATH_BENEATH = 1
LANDLOCK_ACCESS_FS_WRITE_FILE = 1 << 1
LANDLOCK_ACCESS_FS_READ_FILE = 1 << 2
LANDLOCK_ACCESS_FS_REMOVE_DIR = 1 << 4
LANDLOCK_ACCESS_FS_REMOVE_FILE = 1 << 5
LANDLOCK_ACCESS_FS_MAKE_CHAR = 1 << 6
LANDLOCK_ACCESS_FS_MAKE_DIR = 1 << 7
LANDLOCK_ACCESS_FS_MAKE_REG = 1 << 8
LANDLOCK_ACCESS_FS_MAKE_SOCK = 1 << 9
LANDLOCK_ACCESS_FS_MAKE_FIFO = 1 << 10
LANDLOCK_ACCESS_FS_MAKE_BLOCK = 1 << 11
LANDLOCK_ACCESS_FS_MAKE_SYM = 1 << 12
LANDLOCK_ACCESS_FS_REFER = 1 << 13
LANDLOCK_ACCESS_FS_TRUNCATE = 1 << 14
LANDLOCK_WRITE_ACCESS = (
    LANDLOCK_ACCESS_FS_WRITE_FILE
    | LANDLOCK_ACCESS_FS_REMOVE_DIR
    | LANDLOCK_ACCESS_FS_REMOVE_FILE
    | LANDLOCK_ACCESS_FS_MAKE_CHAR
    | LANDLOCK_ACCESS_FS_MAKE_DIR
    | LANDLOCK_ACCESS_FS_MAKE_REG
    | LANDLOCK_ACCESS_FS_MAKE_SOCK
    | LANDLOCK_ACCESS_FS_MAKE_FIFO
    | LANDLOCK_ACCESS_FS_MAKE_BLOCK
    | LANDLOCK_ACCESS_FS_MAKE_SYM
    | LANDLOCK_ACCESS_FS_REFER
    | LANDLOCK_ACCESS_FS_TRUNCATE
)
F_ADD_SEALS = 1033
F_GET_SEALS = 1034
F_SEAL_SEAL = 0x0001
F_SEAL_SHRINK = 0x0002
F_SEAL_GROW = 0x0004
F_SEAL_WRITE = 0x0008
REQUIRED_SEALS = F_SEAL_SEAL | F_SEAL_SHRINK | F_SEAL_GROW | F_SEAL_WRITE
MAX_SNAPSHOT_FILE_BYTES = 512 * 1024 * 1024
MAX_SNAPSHOT_FILES = 65536
MAX_CLOSURE_TOTAL_BYTES = 16 * 1024 * 1024 * 1024
READ_CHUNK = 1024 * 1024


class HardeningError(RuntimeError):
    pass


def canonical_json(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")


def _prctl(option: int, value: int) -> None:
    libc = ctypes.CDLL(None, use_errno=True)
    if libc.prctl(option, value, 0, 0, 0) != 0:
        code = ctypes.get_errno()
        raise OSError(code, os.strerror(code))


class _LandlockRulesetAttr(ctypes.Structure):
    _fields_ = [("handled_access_fs", ctypes.c_uint64)]


class _LandlockPathBeneathAttr(ctypes.Structure):
    _fields_ = [
        ("allowed_access", ctypes.c_uint64),
        ("parent_fd", ctypes.c_int32),
        ("reserved", ctypes.c_uint32),
    ]


def _landlock_syscalls() -> tuple[int, int, int]:
    if os.uname().machine != "x86_64":
        raise HardeningError("Landlock syscall mapping is pinned to x86_64")
    return 444, 445, 446


def _restrict_filesystem_with_landlock(
    writable_roots: Sequence[Path],
    readable_paths: Sequence[Path] | None,
    readable_roots: Sequence[Path],
) -> None:
    create_ruleset, add_rule, restrict_self = _landlock_syscalls()
    libc = ctypes.CDLL(None, use_errno=True)
    abi = libc.syscall(
        create_ruleset,
        ctypes.c_void_p(),
        ctypes.c_size_t(0),
        ctypes.c_uint(LANDLOCK_CREATE_RULESET_VERSION),
    )
    if abi < 3:
        raise HardeningError("Landlock ABI 3 or newer is required")
    handled_access = LANDLOCK_WRITE_ACCESS
    if readable_paths is not None:
        handled_access |= LANDLOCK_ACCESS_FS_READ_FILE
    ruleset_attr = _LandlockRulesetAttr(handled_access)
    ruleset_fd = libc.syscall(
        create_ruleset,
        ctypes.byref(ruleset_attr),
        ctypes.sizeof(ruleset_attr),
        ctypes.c_uint(0),
    )
    if ruleset_fd < 0:
        code = ctypes.get_errno()
        raise OSError(code, os.strerror(code))
    opened: list[int] = []
    try:
        rules: dict[Path, int] = {
            root: LANDLOCK_WRITE_ACCESS for root in writable_roots
        }
        if readable_paths is not None:
            for path in readable_paths:
                rules[path] = rules.get(path, 0) | LANDLOCK_ACCESS_FS_READ_FILE
            for root in readable_roots:
                rules[root] = rules.get(root, 0) | LANDLOCK_ACCESS_FS_READ_FILE
        for root, allowed_access in sorted(rules.items(), key=lambda item: os.fspath(item[0])):
            fd = os.open(root, os.O_PATH | os.O_CLOEXEC | os.O_NOFOLLOW)
            opened.append(fd)
            rule = _LandlockPathBeneathAttr(allowed_access, fd, 0)
            if libc.syscall(
                add_rule,
                ruleset_fd,
                LANDLOCK_RULE_PATH_BENEATH,
                ctypes.byref(rule),
                ctypes.c_uint(0),
            ) != 0:
                code = ctypes.get_errno()
                raise OSError(code, os.strerror(code))
        if libc.syscall(restrict_self, ruleset_fd, ctypes.c_uint(0)) != 0:
            code = ctypes.get_errno()
            raise OSError(code, os.strerror(code))
    finally:
        for fd in opened:
            os.close(fd)
        os.close(ruleset_fd)


def enable_subreaper_and_no_new_privs() -> None:
    _prctl(PR_SET_CHILD_SUBREAPER, 1)
    _prctl(PR_SET_NO_NEW_PRIVS, 1)


def stat_identity(value: os.stat_result) -> tuple[int, ...]:
    return (
        value.st_dev,
        value.st_ino,
        value.st_mode,
        value.st_nlink,
        value.st_uid,
        value.st_gid,
        value.st_size,
        value.st_mtime_ns,
        value.st_ctime_ns,
    )


def stat_record(value: os.stat_result) -> dict[str, int]:
    return {
        "device": value.st_dev,
        "inode": value.st_ino,
        "mode": value.st_mode & 0o7777,
        "links": value.st_nlink,
        "uid": value.st_uid,
        "gid": value.st_gid,
        "bytes": value.st_size,
        "mtime_ns": value.st_mtime_ns,
        "ctime_ns": value.st_ctime_ns,
    }


def hash_fd(fd: int, size: int) -> str:
    if size < 0 or size > MAX_SNAPSHOT_FILE_BYTES:
        raise HardeningError("retained file exceeds the per-file size bound")
    digest = hashlib.sha256()
    offset = 0
    while offset < size:
        chunk = os.pread(fd, min(READ_CHUNK, size - offset), offset)
        if not chunk:
            raise HardeningError("retained file became truncated while hashing")
        digest.update(chunk)
        offset += len(chunk)
    return digest.hexdigest()


@dataclass
class RetainedFile:
    label: str
    path: Path
    fd: int
    identity: tuple[int, ...]
    sha256: str
    require_read_only: bool

    @classmethod
    def open(
        cls,
        label: str,
        path: Path,
        *,
        require_read_only: bool = False,
        require_executable: bool = False,
        allow_hardlinks: bool = False,
    ) -> "RetainedFile":
        absolute = path.absolute()
        if path != absolute or path.is_symlink() or path.resolve(strict=True) != path:
            raise HardeningError(f"{label} path is not absolute, canonical, and direct")
        fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
        try:
            named = os.stat(path, follow_symlinks=False)
            opened = os.fstat(fd)
            if stat_identity(named) != stat_identity(opened):
                raise HardeningError(f"{label} changed while being retained")
            if not stat.S_ISREG(opened.st_mode) or (
                opened.st_nlink != 1 and not allow_hardlinks
            ):
                raise HardeningError(f"{label} is not an accepted regular file")
            if require_read_only and opened.st_mode & 0o222:
                raise HardeningError(f"{label} is mutable by file mode")
            if require_executable and not opened.st_mode & 0o111:
                raise HardeningError(f"{label} is not executable")
            digest = hash_fd(fd, opened.st_size)
            return cls(label, path, fd, stat_identity(opened), digest, require_read_only)
        except BaseException:
            os.close(fd)
            raise

    def record(self) -> dict[str, Any]:
        value = os.fstat(self.fd)
        return {
            "label": self.label,
            "path": os.fspath(self.path),
            "sha256": self.sha256,
            "stat": stat_record(value),
        }

    def revalidate(self) -> None:
        self.revalidate_identity()
        opened = os.fstat(self.fd)
        if hash_fd(self.fd, opened.st_size) != self.sha256:
            raise HardeningError(f"retained file content changed: {self.label}")

    def revalidate_identity(self) -> None:
        named = os.stat(self.path, follow_symlinks=False)
        opened = os.fstat(self.fd)
        if stat_identity(named) != self.identity or stat_identity(opened) != self.identity:
            raise HardeningError(
                f"retained file identity changed: {self.label}: fd={self.fd}: "
                f"expected={self.identity}: named={stat_identity(named)}: "
                f"opened={stat_identity(opened)}"
            )
        if self.require_read_only and opened.st_mode & 0o222:
            raise HardeningError(f"retained file became mutable: {self.label}")

    def close(self) -> None:
        if self.fd >= 0:
            try:
                os.close(self.fd)
            except OSError as error:
                if error.errno != errno.EBADF:
                    raise
            self.fd = -1


@dataclass
class SealedExecutable:
    source: RetainedFile
    fd: int
    sha256: str
    size: int

    @classmethod
    def from_retained(cls, source: RetainedFile) -> "SealedExecutable":
        source.revalidate()
        source_stat = os.fstat(source.fd)
        if not source_stat.st_mode & 0o111:
            raise HardeningError(f"executable source is not executable: {source.label}")
        fd = os.memfd_create(f"fe2o3-{source.label}", os.MFD_ALLOW_SEALING)
        try:
            offset = 0
            while offset < source_stat.st_size:
                chunk = os.pread(source.fd, min(READ_CHUNK, source_stat.st_size - offset), offset)
                if not chunk:
                    raise HardeningError("executable source truncated during sealing")
                view = memoryview(chunk)
                while view:
                    written = os.write(fd, view)
                    view = view[written:]
                offset += len(chunk)
            os.fchmod(fd, source_stat.st_mode & 0o555)
            fcntl.fcntl(fd, F_ADD_SEALS, REQUIRED_SEALS)
            if fcntl.fcntl(fd, F_GET_SEALS) != REQUIRED_SEALS:
                raise HardeningError("executable memfd did not acquire all required seals")
            os.set_inheritable(fd, True)
            if hash_fd(fd, source_stat.st_size) != source.sha256:
                raise HardeningError("sealed executable differs from its retained source")
            return cls(source, fd, source.sha256, source_stat.st_size)
        except BaseException:
            os.close(fd)
            raise

    @property
    def proc_path(self) -> str:
        return f"/proc/{os.getpid()}/fd/{self.fd}"

    def revalidate(self) -> None:
        self.source.revalidate()
        self.revalidate_identity()
        value = os.fstat(self.fd)
        if hash_fd(self.fd, value.st_size) != self.sha256:
            raise HardeningError(f"sealed executable content changed: {self.source.label}")

    def revalidate_identity(self) -> None:
        self.source.revalidate_identity()
        value = os.fstat(self.fd)
        if not stat.S_ISREG(value.st_mode) or value.st_size != self.size:
            raise HardeningError(f"sealed executable identity changed: {self.source.label}")
        if fcntl.fcntl(self.fd, F_GET_SEALS) != REQUIRED_SEALS:
            raise HardeningError(f"sealed executable lost seals: {self.source.label}")

    def close(self) -> None:
        if self.fd >= 0:
            os.close(self.fd)
            self.fd = -1


@dataclass
class SnapshotClosure:
    label: str
    source_root: Path
    root: Path
    source_files: list[RetainedFile]
    snapshot_files: list[RetainedFile]
    manifest: dict[str, Any]

    def relocate(self, destination: Path) -> None:
        """Rename an immutable snapshot while preserving every retained inode."""
        if (
            not destination.is_absolute()
            or destination.exists()
            or destination.is_symlink()
        ):
            raise HardeningError(
                f"{self.label} relocation destination must be an absent absolute path"
            )
        self.revalidate()
        old_root = self.root
        relative_paths = [retained.path.relative_to(old_root) for retained in self.snapshot_files]
        root_fd = os.open(old_root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
        try:
            opened = os.fstat(root_fd)
            named = os.stat(old_root, follow_symlinks=False)
            if stat_identity(opened) != stat_identity(named):
                raise HardeningError(f"{self.label} snapshot root changed before relocation")
            os.fchmod(root_fd, 0o755)
            try:
                old_root.rename(destination)
            finally:
                os.fchmod(root_fd, 0o555)
        finally:
            os.close(root_fd)
        self.root = destination
        for retained, relative in zip(self.snapshot_files, relative_paths, strict=True):
            retained.path = destination / relative
        if stat.S_IMODE(os.stat(destination, follow_symlinks=False).st_mode) != 0o555:
            raise HardeningError(f"{self.label} relocated snapshot root is mutable")
        self.revalidate()

    def revalidate(self) -> None:
        for retained in self.source_files:
            retained.revalidate()
        for retained in self.snapshot_files:
            retained.revalidate()

    def close(self) -> None:
        for retained in self.source_files:
            retained.close()
        for retained in self.snapshot_files:
            retained.close()


@dataclass
class RetainedClosure:
    label: str
    files: list[RetainedFile]
    manifest: dict[str, Any]

    def revalidate(self) -> None:
        for retained in self.files:
            retained.revalidate()

    def close(self) -> None:
        for retained in self.files:
            retained.close()


def capture_retained_closure(
    label: str,
    members: Iterable[tuple[str, Path]],
    metadata: Mapping[str, Any],
    *,
    max_total_bytes: int = MAX_CLOSURE_TOTAL_BYTES,
) -> RetainedClosure:
    selected = sorted(members, key=lambda item: item[0])
    if not selected or len(selected) > MAX_SNAPSHOT_FILES:
        raise HardeningError(f"{label} retained closure count is outside the bound")
    files: list[RetainedFile] = []
    labels: set[str] = set()
    total_bytes = 0
    try:
        for member_label, path in selected:
            if member_label in labels:
                raise HardeningError(f"{label} retained closure has a duplicate label")
            labels.add(member_label)
            retained = RetainedFile.open(f"{label}:{member_label}", path)
            total_bytes += os.fstat(retained.fd).st_size
            if total_bytes > max_total_bytes:
                retained.close()
                raise HardeningError(f"{label} retained closure exceeds its byte bound")
            files.append(retained)
        records = []
        for (member_label, _), retained in zip(selected, files, strict=True):
            record = retained.record()
            record["label"] = member_label
            records.append(record)
        manifest = {
            "schema": "fe2o3-retained-build-closure-v1",
            "label": label,
            "metadata": dict(metadata),
            "files": records,
            "total_bytes": total_bytes,
            "max_total_bytes": max_total_bytes,
        }
        manifest["manifest_sha256"] = hashlib.sha256(canonical_json(manifest)).hexdigest()
        all_fds = [retained.fd for retained in files]
        if len(all_fds) != len(set(all_fds)):
            raise HardeningError(f"{label} retained closure has duplicate descriptors")
        closure = RetainedClosure(label, files, manifest)
        closure.revalidate()
        return closure
    except BaseException:
        for retained in files:
            retained.close()
        raise


def _validate_relative_path(path: str) -> Path:
    relative = Path(path)
    if not path or path.startswith("/") or "\0" in path or relative != Path(os.path.normpath(path)):
        raise HardeningError(f"invalid snapshot path: {path!r}")
    if any(part in ("", ".", "..") for part in relative.parts):
        raise HardeningError(f"unsafe snapshot path: {path!r}")
    return relative


def capture_snapshot(
    label: str,
    source_root: Path,
    destination: Path,
    relative_paths: Iterable[str],
    metadata: Mapping[str, Any],
    *,
    allow_source_hardlinks: bool = False,
) -> SnapshotClosure:
    if not destination.is_absolute() or destination.exists() or destination.is_symlink():
        raise HardeningError(f"{label} snapshot destination must be an absent absolute path")
    if source_root.resolve(strict=True) != source_root or source_root.is_symlink():
        raise HardeningError(f"{label} source root is not canonical")
    paths = sorted(set(relative_paths))
    if not paths or len(paths) > MAX_SNAPSHOT_FILES:
        raise HardeningError(f"{label} snapshot file count is outside the bound")
    source_files: list[RetainedFile] = []
    snapshot_files: list[RetainedFile] = []
    destination.mkdir(mode=0o700)
    total_bytes = 0
    try:
        for raw in paths:
            relative = _validate_relative_path(raw)
            source = source_root / relative
            retained = RetainedFile.open(
                f"{label}:origin:{relative.as_posix()}",
                source,
                allow_hardlinks=allow_source_hardlinks,
            )
            total_bytes += os.fstat(retained.fd).st_size
            if total_bytes > MAX_CLOSURE_TOTAL_BYTES:
                retained.close()
                raise HardeningError(f"{label} snapshot exceeds its byte bound")
            source_files.append(retained)
            target = destination / relative
            target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
            source_stat = os.fstat(retained.fd)
            target_fd = os.open(target, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
            try:
                offset = 0
                while offset < source_stat.st_size:
                    chunk = os.pread(retained.fd, min(READ_CHUNK, source_stat.st_size - offset), offset)
                    if not chunk:
                        raise HardeningError(f"{retained.label} truncated during snapshot")
                    view = memoryview(chunk)
                    while view:
                        written = os.write(target_fd, view)
                        view = view[written:]
                    offset += len(chunk)
                os.fsync(target_fd)
                os.fchmod(target_fd, 0o555 if source_stat.st_mode & 0o111 else 0o444)
            finally:
                os.close(target_fd)
            snapshot_files.append(
                RetainedFile.open(
                    f"{label}:snapshot:{relative.as_posix()}", target, require_read_only=True
                )
            )
        directories = sorted(
            (path for path in destination.rglob("*") if path.is_dir()),
            key=lambda item: len(item.parts),
            reverse=True,
        )
        for directory in directories:
            directory.chmod(0o555)
        destination.chmod(0o555)
        records = []
        for raw, retained in zip(paths, snapshot_files, strict=True):
            record = retained.record()
            record["label"] = raw
            record["path"] = raw
            records.append(record)
        manifest = {
            "schema": "fe2o3-immutable-source-closure-v1",
            "label": label,
            "metadata": dict(metadata),
            "files": records,
            "total_bytes": total_bytes,
            "max_total_bytes": MAX_CLOSURE_TOTAL_BYTES,
        }
        manifest["manifest_sha256"] = hashlib.sha256(canonical_json(manifest)).hexdigest()
        all_fds = [retained.fd for retained in source_files + snapshot_files]
        if len(all_fds) != len(set(all_fds)):
            raise HardeningError(f"{label} snapshot retained duplicate descriptors")
        closure = SnapshotClosure(label, source_root, destination, source_files, snapshot_files, manifest)
        closure.revalidate()
        return closure
    except BaseException:
        for retained in source_files + snapshot_files:
            retained.close()
        raise


def compare_labeled_manifests(first: Mapping[str, Any], second: Mapping[str, Any]) -> None:
    def keyed(value: Mapping[str, Any]) -> dict[str, tuple[str, int, int]]:
        files = value.get("files")
        if not isinstance(files, list):
            raise HardeningError("closure manifest has no file list")
        result: dict[str, tuple[str, int, int]] = {}
        for record in files:
            label = record.get("label")
            info = record.get("stat")
            if not isinstance(label, str) or not isinstance(info, dict) or label in result:
                raise HardeningError("closure manifest labels are invalid or duplicated")
            result[label] = (record["sha256"], info["bytes"], info["mode"])
        return result

    if keyed(first) != keyed(second):
        raise HardeningError("independent closure manifests differ by labeled content")


@dataclass(frozen=True)
class CommandLimits:
    timeout_seconds: float = 300.0
    memory_bytes: int = 8 * 1024 * 1024 * 1024
    output_bytes: int = 16 * 1024 * 1024
    file_bytes: int = 2 * 1024 * 1024 * 1024
    open_files: int = 8192
    processes: int = 256
    cpu_seconds: int = 300


@dataclass
class CommandResult:
    returncode: int
    stdout: bytes
    stderr: bytes
    elapsed_seconds: float
    peak_memory_bytes: int
    peak_processes: int


def prepare_cgroup_delegation() -> Path:
    relative: Path | None = None
    for line in Path("/proc/self/cgroup").read_text("ascii").splitlines():
        if line.startswith("0::"):
            relative = Path(line[3:])
            break
    if relative is None:
        raise HardeningError("process is not in a unified cgroup-v2 hierarchy")
    root = Path("/sys/fs/cgroup") / relative.relative_to("/")
    controls = (root / "cgroup.subtree_control", root / "cgroup.procs")
    if (
        not root.is_dir()
        or root.is_symlink()
        or root.resolve(strict=True) != root
        or any(path.stat().st_uid != os.geteuid() or not os.access(path, os.W_OK) for path in controls)
    ):
        raise HardeningError(
            "controller requires a user-delegated cgroup; invoke through "
            "systemd-run --user --scope -p Delegate=yes"
        )
    available = set((root / "cgroup.controllers").read_text("ascii").split())
    if not {"memory", "pids"}.issubset(available):
        raise HardeningError("delegated cgroup lacks memory or pids controller")
    manager = root / f"controller-{os.getpid()}"
    if manager.exists():
        raise HardeningError("controller cgroup already exists")
    manager.mkdir(mode=0o700)
    try:
        (manager / "cgroup.procs").write_text(str(os.getpid()), encoding="ascii")
        (root / "cgroup.subtree_control").write_text("+memory +pids", encoding="ascii")
    except BaseException:
        try:
            manager.rmdir()
        except OSError:
            pass
        raise
    return root


class Cgroup:
    def __init__(self, parent: Path, limits: CommandLimits, sequence: int):
        self.path = parent / f"command-{sequence}"
        if self.path.exists():
            raise HardeningError("command cgroup already exists")
        self.path.mkdir(mode=0o700)
        try:
            (self.path / "memory.max").write_text(str(limits.memory_bytes), encoding="ascii")
            (self.path / "memory.oom.group").write_text("1", encoding="ascii")
            (self.path / "pids.max").write_text(str(limits.processes), encoding="ascii")
        except BaseException:
            self.close(force=True)
            raise

    def add(self, pid: int) -> None:
        (self.path / "cgroup.procs").write_text(str(pid), encoding="ascii")

    def populated(self) -> bool:
        fields = dict(
            line.split(maxsplit=1)
            for line in (self.path / "cgroup.events").read_text("ascii").splitlines()
        )
        return fields.get("populated") == "1"

    def peak(self, name: str) -> int:
        value = (self.path / name).read_text("ascii").strip()
        return int(value) if value != "max" else 0

    def kill(self) -> None:
        if self.populated():
            (self.path / "cgroup.kill").write_text("1", encoding="ascii")

    def close(self, *, force: bool = False) -> None:
        if not self.path.exists():
            return
        if force:
            try:
                self.kill()
            except OSError:
                pass
        deadline = time.monotonic() + 5.0
        while self.populated() and time.monotonic() < deadline:
            time.sleep(0.01)
        if self.populated():
            raise HardeningError("command cgroup remained populated after kill")
        self.path.rmdir()


class Supervisor:
    def __init__(self) -> None:
        enable_subreaper_and_no_new_privs()
        self.cgroup_root = prepare_cgroup_delegation()
        self.sequence = 0
        self.guards: list[RetainedFile] = []
        self.writable_roots: tuple[Path, ...] = ()

    def set_writable_roots(self, roots: Iterable[Path]) -> None:
        selected = tuple(sorted(set(roots), key=os.fspath))
        if not selected:
            raise HardeningError("supervisor write-root allowlist is empty")
        for root in selected:
            if (
                not root.is_absolute()
                or root.is_symlink()
                or root.resolve(strict=True) != root
            ):
                raise HardeningError(f"supervisor write root is not canonical: {root}")
        self.writable_roots = selected

    @staticmethod
    def _reap_descendants() -> None:
        while True:
            try:
                child, _ = os.waitpid(-1, os.WNOHANG)
            except ChildProcessError:
                return
            if child == 0:
                return

    def run(
        self,
        executable: SealedExecutable,
        arguments: Sequence[str],
        cwd: Path,
        environment: Mapping[str, str],
        *,
        limits: CommandLimits = CommandLimits(),
        inherited_fds: Iterable[int] = (),
        readable_paths: Iterable[Path] | None = None,
        readable_roots: Iterable[Path] = (),
    ) -> CommandResult:
        if not arguments or any("\0" in value for value in arguments):
            raise HardeningError("command arguments are invalid")
        if any("\0" in key or "\0" in value or "=" in key for key, value in environment.items()):
            raise HardeningError("command environment is invalid")
        executable.revalidate()
        selected_readable_paths = (
            None
            if readable_paths is None
            else tuple(sorted(set(readable_paths), key=os.fspath))
        )
        selected_readable_roots = tuple(
            sorted(set(readable_roots), key=os.fspath)
        )
        for selected in (
            *(() if selected_readable_paths is None else selected_readable_paths),
            *selected_readable_roots,
        ):
            if (
                not selected.is_absolute()
                or selected.is_symlink()
                or selected.resolve(strict=True) != selected
            ):
                raise HardeningError(f"Landlock read path is not canonical: {selected}")
        for guard in self.guards:
            guard.revalidate_identity()
        cwd_fd = os.open(cwd, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
        stdout_read, stdout_write = os.pipe2(os.O_CLOEXEC | os.O_NONBLOCK)
        stderr_read, stderr_write = os.pipe2(os.O_CLOEXEC | os.O_NONBLOCK)
        os.set_blocking(stdout_write, True)
        os.set_blocking(stderr_write, True)
        gate_read, gate_write = os.pipe2(os.O_CLOEXEC)
        self.sequence += 1
        cgroup = Cgroup(self.cgroup_root, limits, self.sequence)
        start = time.monotonic()
        pid = -1
        selector: selectors.BaseSelector | None = None
        try:
            pid = os.fork()
            if pid == 0:
                try:
                    os.close(stdout_read)
                    os.close(stderr_read)
                    os.close(gate_write)
                    null_fd = os.open("/dev/null", os.O_RDONLY | os.O_CLOEXEC)
                    os.setsid()
                    _prctl(PR_SET_NO_NEW_PRIVS, 1)
                    _restrict_filesystem_with_landlock(
                        self.writable_roots,
                        selected_readable_paths,
                        selected_readable_roots,
                    )
                    resource.setrlimit(resource.RLIMIT_AS, (limits.memory_bytes, limits.memory_bytes))
                    resource.setrlimit(resource.RLIMIT_CPU, (limits.cpu_seconds, limits.cpu_seconds + 1))
                    resource.setrlimit(resource.RLIMIT_FSIZE, (limits.file_bytes, limits.file_bytes))
                    os.dup2(stdout_write, 1)
                    os.dup2(stderr_write, 2)
                    os.dup2(null_fd, 0)
                    os.fchdir(cwd_fd)
                    allowed = {0, 1, 2, executable.fd, *inherited_fds}
                    for fd in allowed:
                        if fd > 2:
                            os.set_inheritable(fd, True)
                    if os.read(gate_read, 1) != b"1":
                        os._exit(126)
                    resource.setrlimit(
                        resource.RLIMIT_NOFILE, (limits.open_files, limits.open_files)
                    )
                    os.execve(executable.fd, list(arguments), dict(environment))
                except BaseException as error:
                    os.write(2, f"supervised exec failed: {error}\n".encode("utf-8", "replace")[:4096])
                    os._exit(127)
            os.close(stdout_write)
            os.close(stderr_write)
            os.close(gate_read)
            cgroup.add(pid)
            try:
                os.write(gate_write, b"1")
            except BrokenPipeError:
                # The pre-exec child reports its bounded error through stderr;
                # continue supervision rather than masking it at the gate.
                pass
            os.close(gate_write)
            gate_write = -1
            selector = selectors.DefaultSelector()
            selector.register(stdout_read, selectors.EVENT_READ, "stdout")
            selector.register(stderr_read, selectors.EVENT_READ, "stderr")
            output = {"stdout": bytearray(), "stderr": bytearray()}
            status: int | None = None
            failure: str | None = None
            while status is None or selector.get_map():
                elapsed = time.monotonic() - start
                if elapsed > limits.timeout_seconds and failure is None:
                    failure = "command exceeded timeout"
                    cgroup.kill()
                for key, _ in selector.select(0.02):
                    try:
                        chunk = os.read(key.fd, 65536)
                    except BlockingIOError:
                        continue
                    if not chunk:
                        selector.unregister(key.fd)
                        os.close(key.fd)
                        continue
                    target = output[key.data]
                    target.extend(chunk)
                    if sum(len(item) for item in output.values()) > limits.output_bytes and failure is None:
                        failure = "command exceeded output bound"
                        cgroup.kill()
                if status is None:
                    waited, observed = os.waitpid(pid, os.WNOHANG)
                    if waited == pid:
                        status = observed
                if failure is not None and time.monotonic() - start > limits.timeout_seconds + 5:
                    break
            if status is None:
                _, status = os.waitpid(pid, 0)
            selector.close()
            selector = None
            returncode = os.waitstatus_to_exitcode(status)
            leaked = cgroup.populated()
            if leaked:
                cgroup.kill()
            deadline = time.monotonic() + 5.0
            while cgroup.populated() and time.monotonic() < deadline:
                self._reap_descendants()
                time.sleep(0.01)
            self._reap_descendants()
            peak_memory = cgroup.peak("memory.peak")
            peak_processes = cgroup.peak("pids.peak")
            elapsed = time.monotonic() - start
            executable.revalidate()
            for guard in self.guards:
                guard.revalidate_identity()
            if failure is not None:
                raise HardeningError(failure)
            if leaked:
                raise HardeningError("command left descendants after leader exit")
            return CommandResult(
                returncode,
                bytes(output["stdout"]),
                bytes(output["stderr"]),
                elapsed,
                peak_memory,
                peak_processes,
            )
        finally:
            if selector is not None:
                selector.close()
            if pid > 0:
                try:
                    cgroup.kill()
                except OSError:
                    pass
                self._reap_descendants()
            for fd in (cwd_fd, stdout_read, stdout_write, stderr_read, stderr_write, gate_read, gate_write):
                if fd >= 0:
                    try:
                        os.close(fd)
                    except OSError as error:
                        if error.errno != errno.EBADF:
                            raise
            cgroup.close(force=True)


def adversarial_self_test() -> None:
    import copy
    import tempfile

    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary).resolve(strict=True)
        source = root / "python"
        python = Path("/usr/bin/python3.12").resolve(strict=True)
        source.write_bytes(python.read_bytes())
        source.chmod(0o555)
        retained = RetainedFile.open("self-test-python", source, require_read_only=True, require_executable=True)
        executable = SealedExecutable.from_retained(retained)
        supervisor = Supervisor()
        supervisor.set_writable_roots((root, Path("/tmp")))
        environment = {
            "LANG": "C",
            "LC_ALL": "C",
            "PATH": "/nonexistent",
            "PYTHONDONTWRITEBYTECODE": "1",
            "TZ": "UTC",
        }
        try:
            result = supervisor.run(
                executable,
                ["python", "-c", "print('retained-exec-ok')"],
                root,
                environment,
                limits=CommandLimits(timeout_seconds=5, memory_bytes=256 * 1024 * 1024),
            )
            if result.returncode != 0 or result.stdout != b"retained-exec-ok\n":
                raise AssertionError(
                    "retained executable did not run: "
                    f"returncode={result.returncode}, stderr={result.stderr!r}"
                )
            large = supervisor.run(
                executable,
                ["python", "-c", "print('z' * 200000, end='')"],
                root,
                environment,
                limits=CommandLimits(
                    timeout_seconds=5,
                    memory_bytes=256 * 1024 * 1024,
                    output_bytes=256 * 1024,
                ),
            )
            if large.returncode != 0 or large.stdout != b"z" * 200000:
                raise AssertionError("bounded output capture lost bytes")
            escape_program = r'''
import os
from pathlib import Path

root = Path(os.environ["FE2O3_ESCAPE_CGROUP_ROOT"])
targets = [root / "cgroup.procs"]
targets.extend(path / "cgroup.procs" for path in root.glob("controller-*"))

def blocked() -> bool:
    if len(targets) < 2:
        return False
    for target in targets:
        try:
            target.write_text(str(os.getpid()), encoding="ascii")
        except OSError:
            continue
        return False
    return True

read_fd, write_fd = os.pipe()
first = os.fork()
if first == 0:
    os.close(read_fd)
    os.setsid()
    second = os.fork()
    if second == 0:
        os.write(write_fd, b"blocked" if blocked() else b"escaped")
        os.close(write_fd)
        os._exit(0)
    os.close(write_fd)
    os._exit(0)
os.close(write_fd)
result = os.read(read_fd, 32)
os.close(read_fd)
os.waitpid(first, 0)
if result != b"blocked" or not blocked():
    raise SystemExit("cgroup migration escape succeeded")
print("cgroup-migration-blocked")
'''
            escape_environment = dict(environment)
            escape_environment["FE2O3_ESCAPE_CGROUP_ROOT"] = os.fspath(
                supervisor.cgroup_root
            )
            escaped = supervisor.run(
                executable,
                ["python", "-c", escape_program],
                root,
                escape_environment,
                limits=CommandLimits(
                    timeout_seconds=5,
                    memory_bytes=256 * 1024 * 1024,
                    processes=8,
                ),
            )
            if escaped.returncode != 0 or escaped.stdout != b"cgroup-migration-blocked\n":
                raise AssertionError("Landlock cgroup migration guard failed")
            busybox_source = Path("/usr/bin/busybox").resolve(strict=True)
            busybox_path = root / "busybox"
            busybox_path.write_bytes(busybox_source.read_bytes())
            busybox_path.chmod(0o555)
            allowed_path = root / "allowed-input"
            allowed_path.write_text("allowed\n", encoding="ascii")
            allowed_path.chmod(0o444)
            busybox_retained = RetainedFile.open(
                "self-test-static-busybox",
                busybox_path,
                require_read_only=True,
                require_executable=True,
            )
            busybox = SealedExecutable.from_retained(busybox_retained)
            try:
                denied_read = supervisor.run(
                    busybox,
                    ["busybox", "cat", "/etc/hostname"],
                    root,
                    environment,
                    limits=CommandLimits(timeout_seconds=5),
                    readable_paths=(),
                )
                if denied_read.returncode == 0:
                    raise AssertionError("Landlock accepted an unlisted runtime read")
                allowed_read = supervisor.run(
                    busybox,
                    ["busybox", "cat", os.fspath(allowed_path)],
                    root,
                    environment,
                    limits=CommandLimits(timeout_seconds=5),
                    readable_paths=(allowed_path,),
                )
                if allowed_read.returncode != 0 or allowed_read.stdout != b"allowed\n":
                    raise AssertionError(
                        "Landlock rejected a retained runtime read: "
                        f"returncode={allowed_read.returncode}, "
                        f"stderr={allowed_read.stderr!r}"
                    )
            finally:
                busybox.close()
                busybox_retained.close()
            probes = (
                ("hang", "import time; time.sleep(30)", CommandLimits(timeout_seconds=0.2)),
                ("output", "print('x' * 200000)", CommandLimits(timeout_seconds=5, output_bytes=1024)),
                (
                    "memory",
                    "x = bytearray(512 * 1024 * 1024); print(len(x))",
                    CommandLimits(timeout_seconds=5, memory_bytes=128 * 1024 * 1024),
                ),
                (
                    "double-fork-setsid",
                    "import os,time; p=os.fork(); "
                    "(os.setsid(), os.fork()==0 and time.sleep(30)) if p==0 else None; "
                    "os._exit(0)",
                    CommandLimits(timeout_seconds=5, processes=8),
                ),
            )
            for label, program, limits in probes:
                try:
                    observed = supervisor.run(executable, ["python", "-c", program], root, environment, limits=limits)
                    if observed.returncode == 0:
                        raise AssertionError(f"{label} probe unexpectedly succeeded")
                except HardeningError:
                    pass
            alias = root / "python-hardlink"
            os.link(source, alias)
            try:
                RetainedFile.open("hardlink", source, require_executable=True)
            except HardeningError:
                pass
            else:
                raise AssertionError("hardlinked executable was accepted")
            alias.unlink()
            try:
                os.write(executable.fd, b"x")
            except OSError as error:
                if error.errno != errno.EPERM:
                    raise
            else:
                raise AssertionError("sealed executable remained mutable")
        finally:
            executable.close()
            retained.close()
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary).resolve(strict=True)
        origin = root / "origin"
        origin.mkdir()
        (origin / "Cargo.lock").write_text("lock-v1\n", encoding="ascii")
        (origin / "src").mkdir()
        (origin / "src/main.rs").write_text("fn main() {}\n", encoding="ascii")
        first = capture_snapshot(
            "run-a-source",
            origin,
            root / "snapshot-a",
            ["src/main.rs", "Cargo.lock"],
            {"commit": "1" * 40, "tree": "2" * 40},
        )
        second = capture_snapshot(
            "run-b-source",
            origin,
            root / "snapshot-b",
            ["Cargo.lock", "src/main.rs"],
            {"commit": "1" * 40, "tree": "2" * 40},
        )
        retained_closure = capture_retained_closure(
            "retained-inputs",
            [("lock", origin / "Cargo.lock"), ("source", origin / "src/main.rs")],
            {"kind": "self-test"},
        )
        try:
            compare_labeled_manifests(first.manifest, second.manifest)
            occupied = root / "occupied-execution-root"
            occupied.mkdir()
            try:
                first.relocate(occupied)
            except HardeningError:
                pass
            else:
                raise AssertionError("preexisting relocation root was accepted")
            occupied.rmdir()
            canonical = root / "canonical-execution-root"
            first.relocate(canonical)
            if first.root != canonical or not (canonical / "Cargo.lock").is_file():
                raise AssertionError("immutable snapshot was not relocated")
            if stat.S_IMODE(canonical.stat().st_mode) != 0o555:
                raise AssertionError("relocated immutable snapshot root became mutable")
            first.relocate(root / "snapshot-a")
            substituted = copy.deepcopy(second.manifest)
            substituted["files"][0]["sha256"] = "0" * 64
            try:
                compare_labeled_manifests(first.manifest, substituted)
            except HardeningError:
                pass
            else:
                raise AssertionError("labeled source substitution was accepted")
            try:
                capture_snapshot(
                    "preexisting",
                    origin,
                    root / "snapshot-a",
                    ["Cargo.lock"],
                    {},
                )
            except HardeningError:
                pass
            else:
                raise AssertionError("preexisting snapshot root was accepted")
            (origin / "Cargo.lock").write_text("lock-v2\n", encoding="ascii")
            try:
                first.revalidate()
            except HardeningError:
                pass
            else:
                raise AssertionError("source mutation after snapshot was accepted")
        finally:
            retained_closure.close()
            first.close()
            second.close()
    print("compiler evidence retained-exec/supervisor adversarial tests: PASS")


if __name__ == "__main__":
    adversarial_self_test()
