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
F_ADD_SEALS = 1033
F_GET_SEALS = 1034
F_SEAL_SEAL = 0x0001
F_SEAL_SHRINK = 0x0002
F_SEAL_GROW = 0x0004
F_SEAL_WRITE = 0x0008
REQUIRED_SEALS = F_SEAL_SEAL | F_SEAL_SHRINK | F_SEAL_GROW | F_SEAL_WRITE
MAX_SNAPSHOT_FILE_BYTES = 512 * 1024 * 1024
MAX_SNAPSHOT_FILES = 32768
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
            if not stat.S_ISREG(opened.st_mode) or opened.st_nlink != 1:
                raise HardeningError(f"{label} is not a single-link regular file")
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
) -> RetainedClosure:
    selected = sorted(members, key=lambda item: item[0])
    if not selected or len(selected) > MAX_SNAPSHOT_FILES:
        raise HardeningError(f"{label} retained closure count is outside the bound")
    files: list[RetainedFile] = []
    labels: set[str] = set()
    try:
        for member_label, path in selected:
            if member_label in labels:
                raise HardeningError(f"{label} retained closure has a duplicate label")
            labels.add(member_label)
            files.append(RetainedFile.open(f"{label}:{member_label}", path))
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
    try:
        for raw in paths:
            relative = _validate_relative_path(raw)
            source = source_root / relative
            retained = RetainedFile.open(f"{label}:origin:{relative.as_posix()}", source)
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
    ) -> CommandResult:
        if not arguments or any("\0" in value for value in arguments):
            raise HardeningError("command arguments are invalid")
        if any("\0" in key or "\0" in value or "=" in key for key, value in environment.items()):
            raise HardeningError("command environment is invalid")
        executable.revalidate()
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
                    os.setsid()
                    _prctl(PR_SET_NO_NEW_PRIVS, 1)
                    resource.setrlimit(resource.RLIMIT_AS, (limits.memory_bytes, limits.memory_bytes))
                    resource.setrlimit(resource.RLIMIT_CPU, (limits.cpu_seconds, limits.cpu_seconds + 1))
                    resource.setrlimit(resource.RLIMIT_FSIZE, (limits.file_bytes, limits.file_bytes))
                    os.dup2(stdout_write, 1)
                    os.dup2(stderr_write, 2)
                    null_fd = os.open("/dev/null", os.O_RDONLY | os.O_CLOEXEC)
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
            os.write(gate_write, b"1")
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
        environment = {"LANG": "C", "LC_ALL": "C", "PATH": "/nonexistent", "TZ": "UTC"}
        try:
            result = supervisor.run(
                executable,
                ["python", "-c", "print('retained-exec-ok')"],
                root,
                environment,
                limits=CommandLimits(timeout_seconds=5, memory_bytes=256 * 1024 * 1024),
            )
            if result.returncode != 0 or result.stdout != b"retained-exec-ok\n":
                raise AssertionError("retained executable did not run")
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
