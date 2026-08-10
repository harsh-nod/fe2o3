#!/usr/bin/env python3
"""Create sealed Linux memfd snapshots and pass them to one child process."""

from __future__ import annotations

import argparse
import contextlib
import dataclasses
import fcntl
import hashlib
import os
import pathlib
import re
import secrets
import select
import stat
import subprocess
import sys
from collections.abc import Callable, Iterator, Sequence


MAX_SNAPSHOT_BYTES = 64 * 1024 * 1024
COPY_CHUNK_BYTES = 64 * 1024
REQUIRED_SEALS = (
    fcntl.F_SEAL_SEAL | fcntl.F_SEAL_SHRINK | fcntl.F_SEAL_GROW | fcntl.F_SEAL_WRITE
)
PROC_FD_PATH = re.compile(r"/proc/([1-9][0-9]*)/fd/([0-9]+)")


class SnapshotError(Exception):
    pass


@dataclasses.dataclass(frozen=True)
class FileIdentity:
    device: int
    inode: int
    mode: int
    links: int
    size: int
    mtime_ns: int
    ctime_ns: int

    @classmethod
    def from_stat(cls, value: os.stat_result) -> FileIdentity:
        return cls(
            value.st_dev,
            value.st_ino,
            value.st_mode,
            value.st_nlink,
            value.st_size,
            value.st_mtime_ns,
            value.st_ctime_ns,
        )


@dataclasses.dataclass
class SealedSnapshot:
    name: str
    descriptor: int
    sha256: str
    size: int
    device: int
    inode: int
    mode: int
    executable: bool
    owner_pid: int
    owner_start_time_ticks: int

    @property
    def proc_path(self) -> str:
        return f"/proc/{self.owner_pid}/fd/{self.descriptor}"

    def close(self) -> None:
        os.close(self.descriptor)


def _read_process_stat(pid: int) -> tuple[int, int]:
    path = pathlib.Path(f"/proc/{pid}/stat")
    try:
        data = path.read_text(encoding="ascii")
    except OSError as error:
        raise SnapshotError(
            f"cannot read snapshot owner state: {path}: {error}"
        ) from error
    closing = data.rfind(") ")
    if closing < 0:
        raise SnapshotError(f"snapshot owner state is malformed: {path}")
    fields = data[closing + 2 :].split()
    if len(fields) < 20 or not fields[1].isdigit() or not fields[19].isdigit():
        raise SnapshotError(f"snapshot owner state is truncated: {path}")
    return int(fields[1]), int(fields[19])


def _require_live_ancestor(pid: int) -> int:
    current = os.getpid()
    visited: set[int] = set()
    while current > 0 and current not in visited:
        visited.add(current)
        parent, start_time = _read_process_stat(current)
        if current == pid:
            return start_time
        current = parent
    raise SnapshotError("snapshot owner must be the current process or a live ancestor")


def _require_live_pidfd(pid: int) -> int:
    if not hasattr(os, "pidfd_open"):
        raise SnapshotError("sealed S09 snapshots require Linux pidfd_open")
    try:
        descriptor = os.pidfd_open(pid, 0)
    except OSError as error:
        raise SnapshotError(f"snapshot owner is not live: {pid}: {error}") from error
    poller = select.poll()
    poller.register(descriptor, select.POLLIN | select.POLLHUP | select.POLLERR)
    if poller.poll(0):
        os.close(descriptor)
        raise SnapshotError(f"snapshot owner has exited: {pid}")
    return descriptor


def _parse_proc_fd_path(path: pathlib.Path) -> tuple[int, int] | None:
    match = PROC_FD_PATH.fullmatch(os.fspath(path))
    if match is None:
        return None
    return int(match.group(1)), int(match.group(2))


def _open_inherited_snapshot(
    path: pathlib.Path,
    executable: bool,
    *,
    expected_owner_start_time_ticks: int | None = None,
    expected_owner_uid: int | None = None,
) -> tuple[int, FileIdentity]:
    parsed = _parse_proc_fd_path(path)
    if parsed is None:
        raise SnapshotError(
            "inherited snapshot path is not an exact numeric proc-fd path"
        )
    owner_pid, _ = parsed
    owner_start_time = _require_live_ancestor(owner_pid)
    if (
        expected_owner_start_time_ticks is not None
        and owner_start_time != expected_owner_start_time_ticks
    ):
        raise SnapshotError("snapshot owner PID was reused or has the wrong starttime")
    owner_uid = os.stat(f"/proc/{owner_pid}", follow_symlinks=False).st_uid
    required_uid = os.getuid() if expected_owner_uid is None else expected_owner_uid
    if owner_uid != required_uid:
        raise SnapshotError("snapshot owner has the wrong UID")
    pidfd = _require_live_pidfd(owner_pid)
    descriptor = -1
    try:
        target = os.readlink(path)
        if not (
            target.startswith("/memfd:fe2o3-s09-")
            or target.startswith("memfd:fe2o3-s09-")
        ):
            raise SnapshotError("inherited snapshot is not an S09 memfd")
        descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_CLOEXEC", 0))
        _, owner_start_time_after = _read_process_stat(owner_pid)
        if owner_start_time_after != owner_start_time:
            raise SnapshotError("snapshot owner PID changed while opening its memfd")
        poller = select.poll()
        poller.register(pidfd, select.POLLIN | select.POLLHUP | select.POLLERR)
        if poller.poll(0):
            raise SnapshotError("snapshot owner exited while opening its memfd")
        identity = FileIdentity.from_stat(os.fstat(descriptor))
        if not stat.S_ISREG(identity.mode):
            raise SnapshotError(f"snapshot source must be a regular file: {path}")
        if not 1 <= identity.size <= MAX_SNAPSHOT_BYTES:
            raise SnapshotError(
                f"snapshot source size must be within 1..{MAX_SNAPSHOT_BYTES} bytes: {path}"
            )
        if executable and identity.mode & 0o111 == 0:
            raise SnapshotError(f"snapshot source must be executable: {path}")
        try:
            seals = fcntl.fcntl(descriptor, fcntl.F_GET_SEALS)
        except OSError as error:
            raise SnapshotError(
                f"inherited snapshot source is not a sealed memfd: {path}"
            ) from error
        if seals & REQUIRED_SEALS != REQUIRED_SEALS:
            raise SnapshotError(
                f"inherited snapshot source is missing required seals: {path}"
            )
        result = descriptor
        descriptor = -1
        return result, identity
    except BaseException:
        if descriptor >= 0:
            os.close(descriptor)
        raise
    finally:
        os.close(pidfd)


def _open_source(path: pathlib.Path, executable: bool) -> tuple[int, FileIdentity]:
    if _parse_proc_fd_path(path) is not None:
        return _open_inherited_snapshot(path, executable)
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise SnapshotError(
            f"cannot open snapshot source safely: {path}: {error}"
        ) from error
    try:
        identity = FileIdentity.from_stat(os.fstat(descriptor))
        if not stat.S_ISREG(identity.mode) or identity.links < 1:
            raise SnapshotError(f"snapshot source must be a regular file: {path}")
        if not 1 <= identity.size <= MAX_SNAPSHOT_BYTES:
            raise SnapshotError(
                f"snapshot source size must be within 1..{MAX_SNAPSHOT_BYTES} bytes: {path}"
            )
        if executable and identity.mode & 0o111 == 0:
            raise SnapshotError(f"snapshot source must be executable: {path}")
        return descriptor, identity
    except BaseException:
        os.close(descriptor)
        raise


def _digest_source(descriptor: int, size: int, path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    offset = 0
    while offset < size:
        chunk = os.pread(descriptor, min(COPY_CHUNK_BYTES, size - offset), offset)
        if not chunk:
            raise SnapshotError(f"snapshot source became truncated: {path}")
        digest.update(chunk)
        offset += len(chunk)
    if offset != size:
        raise SnapshotError(f"snapshot source size changed while hashing: {path}")
    return digest.hexdigest()


def create_sealed_snapshot(
    name: str,
    path: pathlib.Path,
    *,
    executable: bool = False,
    _after_first_chunk: Callable[[], None] | None = None,
) -> SealedSnapshot:
    """Copy one coherent source object into a write-sealed anonymous file."""
    if not sys.platform.startswith("linux") or not hasattr(os, "memfd_create"):
        raise SnapshotError("sealed S09 snapshots require Linux memfd_create")
    source, before = _open_source(path, executable)
    owner_pid = os.getpid()
    _, owner_start_time_ticks = _read_process_stat(owner_pid)
    snapshot = -1
    try:
        before_digest = _digest_source(source, before.size, path)
        snapshot = os.memfd_create(
            f"fe2o3-s09-{name}",
            os.MFD_CLOEXEC | os.MFD_ALLOW_SEALING,
        )
        digest = hashlib.sha256()
        offset = 0
        first = True
        while offset < before.size:
            chunk = os.pread(
                source, min(COPY_CHUNK_BYTES, before.size - offset), offset
            )
            if not chunk:
                raise SnapshotError(f"snapshot source became truncated: {path}")
            digest.update(chunk)
            written = 0
            while written < len(chunk):
                count = os.write(snapshot, chunk[written:])
                if count <= 0:
                    raise SnapshotError("sealed snapshot write made no progress")
                written += count
            offset += len(chunk)
            if first and _after_first_chunk is not None:
                first = False
                _after_first_chunk()
        after_digest = _digest_source(source, before.size, path)
        after = FileIdentity.from_stat(os.fstat(source))
        if (
            before != after
            or offset != before.size
            or before_digest != digest.hexdigest()
            or before_digest != after_digest
        ):
            raise SnapshotError(f"snapshot source changed while being copied: {path}")
        os.fchmod(snapshot, 0o500 if executable else 0o400)
        os.lseek(snapshot, 0, os.SEEK_SET)
        fcntl.fcntl(snapshot, fcntl.F_ADD_SEALS, REQUIRED_SEALS)
        seals = fcntl.fcntl(snapshot, fcntl.F_GET_SEALS)
        if seals & REQUIRED_SEALS != REQUIRED_SEALS:
            raise SnapshotError(
                "sealed snapshot is missing required write/resize seals"
            )
        identity = os.fstat(snapshot)
        return SealedSnapshot(
            name,
            snapshot,
            digest.hexdigest(),
            before.size,
            identity.st_dev,
            identity.st_ino,
            identity.st_mode,
            executable,
            owner_pid,
            owner_start_time_ticks,
        )
    except OSError as error:
        if snapshot >= 0:
            os.close(snapshot)
        raise SnapshotError(
            f"cannot create sealed snapshot for {path}: {error}"
        ) from error
    except BaseException:
        if snapshot >= 0:
            os.close(snapshot)
        raise
    finally:
        os.close(source)


@contextlib.contextmanager
def sealed_snapshots(
    inputs: Sequence[tuple[str, pathlib.Path, bool]],
) -> Iterator[dict[str, SealedSnapshot]]:
    snapshots: dict[str, SealedSnapshot] = {}
    try:
        for name, path, executable in inputs:
            if name in snapshots:
                raise SnapshotError(f"duplicate snapshot name: {name}")
            snapshots[name] = create_sealed_snapshot(name, path, executable=executable)
        yield snapshots
    finally:
        for snapshot in snapshots.values():
            snapshot.close()


def _parse_input(
    value: str, executable_names: frozenset[str]
) -> tuple[str, pathlib.Path, bool]:
    name, separator, raw_path = value.partition("=")
    if (
        not separator
        or not name
        or not raw_path
        or not name.replace("_", "a").isalnum()
    ):
        raise SnapshotError(f"snapshot input must be NAME=PATH: {value}")
    return name, pathlib.Path(raw_path), name in executable_names


def _substitute(
    command: Sequence[str], snapshots: dict[str, SealedSnapshot]
) -> list[str]:
    result: list[str] = []
    for argument in command:
        replaced = argument
        for name, snapshot in snapshots.items():
            values = {
                name: snapshot.proc_path,
                f"{name}_device": str(snapshot.device),
                f"{name}_inode": str(snapshot.inode),
                f"{name}_mode": str(snapshot.mode),
                f"{name}_size": str(snapshot.size),
                f"{name}_owner_pid": str(snapshot.owner_pid),
                f"{name}_owner_start_time_ticks": str(
                    snapshot.owner_start_time_ticks
                ),
            }
            for field, value in values.items():
                replaced = replaced.replace("{" + field + "}", value)
        if "{" in replaced or "}" in replaced:
            raise SnapshotError(
                f"unresolved snapshot placeholder in argument: {argument}"
            )
        result.append(replaced)
    return result


def verify_sealed_path(
    name: str,
    path: pathlib.Path,
    *,
    executable: bool = False,
    _expected_owner_start_time_ticks: int | None = None,
    _expected_owner_uid: int | None = None,
) -> None:
    if _parse_proc_fd_path(path) is None:
        raise SnapshotError(
            f"sealed snapshot {name} must use an exact numeric proc-fd path"
        )
    descriptor, _ = _open_inherited_snapshot(
        path,
        executable,
        expected_owner_start_time_ticks=_expected_owner_start_time_ticks,
        expected_owner_uid=_expected_owner_uid,
    )
    try:
        if os.fstat(descriptor).st_size <= 0:
            raise SnapshotError(f"sealed snapshot {name} is empty")
    finally:
        os.close(descriptor)


def export_sealed_snapshot(snapshot: SealedSnapshot, destination: pathlib.Path) -> None:
    if not destination.is_absolute():
        raise SnapshotError("snapshot export destination must be absolute")
    parent = destination.parent
    try:
        parent_metadata = parent.stat(follow_symlinks=False)
        canonical_parent = parent.resolve(strict=True)
    except OSError as error:
        raise SnapshotError(f"cannot inspect snapshot export parent: {error}") from error
    if (
        not stat.S_ISDIR(parent_metadata.st_mode)
        or canonical_parent != parent
        or parent_metadata.st_uid != os.geteuid()
        or stat.S_IMODE(parent_metadata.st_mode) != 0o700
    ):
        raise SnapshotError(
            "snapshot export parent must be canonical, caller-owned, and mode 0700"
        )
    if destination.exists() or destination.is_symlink():
        raise SnapshotError("snapshot export destination must not already exist")

    temporary = parent / (
        f".{destination.name}.tmp-{os.getpid()}-{secrets.token_hex(16)}"
    )
    descriptor = -1
    linked = False
    try:
        descriptor = os.open(
            temporary,
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            0o600,
        )
        digest = hashlib.sha256()
        offset = 0
        while offset < snapshot.size:
            chunk = os.pread(
                snapshot.descriptor,
                min(COPY_CHUNK_BYTES, snapshot.size - offset),
                offset,
            )
            if not chunk:
                raise SnapshotError("sealed snapshot became truncated during export")
            digest.update(chunk)
            written = 0
            while written < len(chunk):
                count = os.write(descriptor, chunk[written:])
                if count <= 0:
                    raise SnapshotError("snapshot export write made no progress")
                written += count
            offset += len(chunk)
        if offset != snapshot.size or digest.hexdigest() != snapshot.sha256:
            raise SnapshotError("snapshot export does not match the sealed source")
        os.fsync(descriptor)
        os.fchmod(descriptor, 0o400)
        os.link(temporary, destination, follow_symlinks=False)
        linked = True
        os.unlink(temporary)
        exported_identity = FileIdentity.from_stat(os.fstat(descriptor))
        directory = os.open(parent, os.O_RDONLY | getattr(os, "O_CLOEXEC", 0))
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
        reopened = os.open(
            destination,
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
        )
        try:
            final_identity = FileIdentity.from_stat(os.fstat(reopened))
            if (
                final_identity != exported_identity
                or final_identity.links != 1
                or stat.S_IMODE(final_identity.mode) != 0o400
                or _digest_source(reopened, snapshot.size, destination)
                != snapshot.sha256
            ):
                raise SnapshotError("retained snapshot export failed identity checks")
        finally:
            os.close(reopened)
    except OSError as error:
        raise SnapshotError(f"cannot export sealed snapshot: {error}") from error
    finally:
        if descriptor >= 0:
            os.close(descriptor)
        try:
            temporary.unlink()
        except FileNotFoundError:
            pass
        if sys.exc_info()[0] is not None and linked:
            try:
                destination.unlink()
            except FileNotFoundError:
                pass


def _parse_export(value: str) -> tuple[str, pathlib.Path]:
    name, separator, raw_path = value.partition("=")
    if (
        not separator
        or not name
        or not raw_path
        or not name.replace("_", "a").isalnum()
    ):
        raise SnapshotError(f"snapshot export must be NAME=PATH: {value}")
    return name, pathlib.Path(raw_path)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", action="append", default=[])
    parser.add_argument("--executable", action="append", default=[])
    parser.add_argument("--verify-only", action="store_true")
    parser.add_argument("--export", action="append", default=[])
    parser.add_argument("command", nargs=argparse.REMAINDER)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    executable_names = frozenset(args.executable)
    inputs = [_parse_input(value, executable_names) for value in args.input]
    if not inputs:
        raise SnapshotError("at least one snapshot input is required")
    if not executable_names.issubset(name for name, _, _ in inputs):
        raise SnapshotError("every executable name must identify a snapshot input")
    exports = [_parse_export(value) for value in args.export]
    if args.verify_only:
        if exports:
            raise SnapshotError("verify-only mode does not accept exports")
        if args.command:
            raise SnapshotError("verify-only mode does not accept a command")
        for name, path, executable in inputs:
            verify_sealed_path(name, path, executable=executable)
        return 0
    if exports:
        if args.command:
            raise SnapshotError("snapshot export mode does not accept a command")
        if len({name for name, _ in exports}) != len(exports):
            raise SnapshotError("snapshot export names must be unique")
        input_names = {name for name, _, _ in inputs}
        if any(name not in input_names for name, _ in exports):
            raise SnapshotError("every export must identify a snapshot input")
        with sealed_snapshots(inputs) as snapshots:
            for name, destination in exports:
                export_sealed_snapshot(snapshots[name], destination)
        return 0
    if not args.command or args.command[0] != "--" or len(args.command) == 1:
        raise SnapshotError("snapshot command must follow --")
    with sealed_snapshots(inputs) as snapshots:
        command = _substitute(args.command[1:], snapshots)
        descriptors = tuple(snapshot.descriptor for snapshot in snapshots.values())
        return subprocess.run(command, pass_fds=descriptors, check=False).returncode


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except SnapshotError as error:
        print(f"s09-pinned-snapshot: {error}", file=sys.stderr)
        raise SystemExit(2)
