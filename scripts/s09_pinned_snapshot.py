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
import stat
import subprocess
import sys
from collections.abc import Callable, Iterator, Sequence


MAX_SNAPSHOT_BYTES = 64 * 1024 * 1024
COPY_CHUNK_BYTES = 64 * 1024
REQUIRED_SEALS = (
    fcntl.F_SEAL_SEAL | fcntl.F_SEAL_SHRINK | fcntl.F_SEAL_GROW | fcntl.F_SEAL_WRITE
)


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
    executable: bool

    @property
    def proc_path(self) -> str:
        return f"/proc/self/fd/{self.descriptor}"

    def close(self) -> None:
        os.close(self.descriptor)


def _open_source(path: pathlib.Path, executable: bool) -> tuple[int, FileIdentity]:
    inherited_snapshot = _is_proc_fd_path(path)
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if not inherited_snapshot:
        flags |= getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise SnapshotError(
            f"cannot open snapshot source safely: {path}: {error}"
        ) from error
    try:
        identity = FileIdentity.from_stat(os.fstat(descriptor))
        if not stat.S_ISREG(identity.mode) or (
            not inherited_snapshot and identity.links < 1
        ):
            raise SnapshotError(f"snapshot source must be a regular file: {path}")
        if not 1 <= identity.size <= MAX_SNAPSHOT_BYTES:
            raise SnapshotError(
                f"snapshot source size must be within 1..{MAX_SNAPSHOT_BYTES} bytes: {path}"
            )
        if executable and identity.mode & 0o111 == 0:
            raise SnapshotError(f"snapshot source must be executable: {path}")
        if inherited_snapshot:
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
        return descriptor, identity
    except BaseException:
        os.close(descriptor)
        raise


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
    snapshot = -1
    try:
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
        after = FileIdentity.from_stat(os.fstat(source))
        if before != after or offset != before.size:
            raise SnapshotError(f"snapshot source changed while being copied: {path}")
        os.fchmod(snapshot, 0o500 if executable else 0o400)
        os.lseek(snapshot, 0, os.SEEK_SET)
        fcntl.fcntl(snapshot, fcntl.F_ADD_SEALS, REQUIRED_SEALS)
        seals = fcntl.fcntl(snapshot, fcntl.F_GET_SEALS)
        if seals & REQUIRED_SEALS != REQUIRED_SEALS:
            raise SnapshotError(
                "sealed snapshot is missing required write/resize seals"
            )
        return SealedSnapshot(
            name, snapshot, digest.hexdigest(), before.size, executable
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
            replaced = replaced.replace("{" + name + "}", snapshot.proc_path)
        if "{" in replaced or "}" in replaced:
            raise SnapshotError(
                f"unresolved snapshot placeholder in argument: {argument}"
            )
        result.append(replaced)
    return result


def verify_sealed_path(
    name: str, path: pathlib.Path, *, executable: bool = False
) -> None:
    if not _is_proc_fd_path(path):
        raise SnapshotError(
            f"sealed snapshot {name} must use an inherited /proc/self/fd path"
        )
    try:
        descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_CLOEXEC", 0))
    except OSError as error:
        raise SnapshotError(f"cannot open sealed snapshot {name}: {error}") from error
    try:
        metadata = os.fstat(descriptor)
        if (
            not stat.S_ISREG(metadata.st_mode)
            or not 1 <= metadata.st_size <= MAX_SNAPSHOT_BYTES
        ):
            raise SnapshotError(f"sealed snapshot {name} is not a bounded regular file")
        if executable and metadata.st_mode & 0o111 == 0:
            raise SnapshotError(f"sealed snapshot {name} is not executable")
        try:
            seals = fcntl.fcntl(descriptor, fcntl.F_GET_SEALS)
        except OSError as error:
            raise SnapshotError(
                f"sealed snapshot {name} is missing required seals"
            ) from error
        if seals & REQUIRED_SEALS != REQUIRED_SEALS:
            raise SnapshotError(f"sealed snapshot {name} is missing required seals")
    except OSError as error:
        raise SnapshotError(f"cannot verify sealed snapshot {name}: {error}") from error
    finally:
        os.close(descriptor)


def _is_proc_fd_path(path: pathlib.Path) -> bool:
    value = os.fspath(path)
    prefix = "/proc/self/fd/"
    return value.startswith(prefix) and value[len(prefix) :].isdigit()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", action="append", default=[])
    parser.add_argument("--executable", action="append", default=[])
    parser.add_argument("--verify-only", action="store_true")
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
    if args.verify_only:
        if args.command:
            raise SnapshotError("verify-only mode does not accept a command")
        for name, path, executable in inputs:
            verify_sealed_path(name, path, executable=executable)
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
