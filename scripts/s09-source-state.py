#!/usr/bin/env python3
"""Hold and recheck one exact clean Git source state for S09 evidence."""

from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import os
import pathlib
import re
import selectors
import signal
import stat
import subprocess
import sys
import time
from collections.abc import Sequence
from types import TracebackType


GIT_OBJECT = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})")
REGULAR_GIT_MODES = frozenset(("100644", "100755"))
MAX_GIT_OUTPUT_BYTES = 64 * 1024 * 1024
MAX_STATE_BYTES = 64 * 1024 * 1024
COPY_CHUNK_BYTES = 64 * 1024
CATCHABLE_SIGNALS = (signal.SIGHUP, signal.SIGINT, signal.SIGTERM)
SIGNAL_GRACE_SECONDS = 5.0
NESTED_CLEANUP_BOUND_SECONDS = 1.0
TERMINATE_GRACE_SECONDS = 0.5
KILL_GRACE_SECONDS = 5.0
PROCESS_POLL_SECONDS = 0.01
REQUIRED_SEALS = (
    fcntl.F_SEAL_SEAL | fcntl.F_SEAL_SHRINK | fcntl.F_SEAL_GROW | fcntl.F_SEAL_WRITE
)
GIT_EXECUTABLE = pathlib.Path("/usr/bin/git")
MAX_GIT_EXECUTABLE_BYTES = 64 * 1024 * 1024
GIT_ENVIRONMENT = {
    "GIT_ATTR_NOSYSTEM": "1",
    "GIT_ASKPASS": "/bin/false",
    "GIT_CONFIG_COUNT": "0",
    "GIT_CONFIG_GLOBAL": "/dev/null",
    "GIT_CONFIG_NOSYSTEM": "1",
    "GIT_CONFIG_SYSTEM": "/dev/null",
    "GIT_NO_REPLACE_OBJECTS": "1",
    "GIT_TERMINAL_PROMPT": "0",
    "HOME": "/",
    "LANG": "C",
    "LC_ALL": "C",
    "PATH": "/usr/bin:/bin",
    "SSH_ASKPASS": "/bin/false",
    "XDG_CONFIG_HOME": "/dev/null",
}
GIT_CONFIG_ARGUMENTS = (
    "-c",
    "core.fsmonitor=false",
    "-c",
    "core.hooksPath=/dev/null",
)


class SourceStateError(Exception):
    pass


class CatchableSignalOwner:
    """Retain catchable cancellation until source revalidation completes."""

    def __init__(self) -> None:
        self.signal_number: int | None = None
        for signal_number in CATCHABLE_SIGNALS:
            signal.signal(signal_number, self._record)
        # The public supervisor owns these signals regardless of its caller's
        # inherited mask. Children consequently inherit the intended unblocked
        # mask while the installed handlers close the Popen publication window.
        signal.pthread_sigmask(signal.SIG_UNBLOCK, CATCHABLE_SIGNALS)

    def _record(self, signal_number: int, _frame: object) -> None:
        if self.signal_number is None:
            self.signal_number = signal_number

    def block_and_capture_pending(self) -> None:
        signal.pthread_sigmask(signal.SIG_BLOCK, CATCHABLE_SIGNALS)
        if self.signal_number is not None:
            return
        pending = signal.sigpending()
        for signal_number in CATCHABLE_SIGNALS:
            if signal_number in pending:
                self.signal_number = signal_number
                return


def stop_process_group(process: subprocess.Popen[bytes]) -> None:
    """Stop a still-unreaped process group without allowing PID reuse."""
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired as error:
            raise SourceStateError(
                "pinned Git process group did not exit after bounded SIGKILL"
            ) from error


def run_bounded(
    arguments: Sequence[str], descriptor: int
) -> subprocess.CompletedProcess[bytes]:
    process = subprocess.Popen(
        arguments,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        cwd="/",
        env=GIT_ENVIRONMENT,
        pass_fds=(descriptor,),
        start_new_session=True,
    )
    assert process.stdout is not None and process.stderr is not None
    selector = selectors.DefaultSelector()
    output = {"stdout": bytearray(), "stderr": bytearray()}
    deadline = time.monotonic() + 60
    try:
        for name, stream in (("stdout", process.stdout), ("stderr", process.stderr)):
            os.set_blocking(stream.fileno(), False)
            selector.register(stream, selectors.EVENT_READ, name)
        while selector.get_map():
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise subprocess.TimeoutExpired(arguments, 60)
            events = selector.select(remaining)
            if not events:
                raise subprocess.TimeoutExpired(arguments, 60)
            for key, _ in events:
                name = key.data
                destination = output[name]
                read_size = min(
                    COPY_CHUNK_BYTES,
                    MAX_GIT_OUTPUT_BYTES - len(destination) + 1,
                )
                chunk = os.read(key.fileobj.fileno(), read_size)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                destination.extend(chunk)
                if len(destination) > MAX_GIT_OUTPUT_BYTES:
                    raise SourceStateError(
                        f"Git {name} exceeds the "
                        f"{MAX_GIT_OUTPUT_BYTES}-byte source-state bound"
                    )
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise subprocess.TimeoutExpired(arguments, 60)
        returncode = process.wait(timeout=remaining)
        return subprocess.CompletedProcess(
            arguments,
            returncode,
            bytes(output["stdout"]),
            bytes(output["stderr"]),
        )
    except BaseException:
        stop_process_group(process)
        raise
    finally:
        selector.close()
        process.stdout.close()
        process.stderr.close()


def exact_file_identity(value: os.stat_result) -> tuple[int, ...]:
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


def hash_descriptor(descriptor: int, size: int) -> str:
    if not 1 <= size <= MAX_GIT_EXECUTABLE_BYTES:
        raise SourceStateError("pinned Git executable size is outside its bound")
    digest = hashlib.sha256()
    offset = 0
    while offset < size:
        try:
            chunk = os.pread(descriptor, min(COPY_CHUNK_BYTES, size - offset), offset)
        except OSError as error:
            raise SourceStateError(
                f"cannot read pinned Git executable: {error}"
            ) from error
        if not chunk:
            raise SourceStateError("pinned Git executable became truncated")
        digest.update(chunk)
        offset += len(chunk)
    return digest.hexdigest()


class PinnedGit:
    def __init__(self, path: pathlib.Path = GIT_EXECUTABLE) -> None:
        self.path = path
        self.descriptor = -1
        try:
            if path != GIT_EXECUTABLE and not path.is_absolute():
                raise SourceStateError("Git executable test pin must be absolute")
            if path.resolve(strict=True) != path:
                raise SourceStateError(
                    "Git executable must be the exact canonical path"
                )
            named = path.stat(follow_symlinks=False)
            if not stat.S_ISREG(named.st_mode) or not named.st_mode & 0o111:
                raise SourceStateError(
                    "Git executable must be an executable regular file"
                )
            self.descriptor = os.open(
                path,
                os.O_RDONLY
                | getattr(os, "O_CLOEXEC", 0)
                | getattr(os, "O_NOFOLLOW", 0),
            )
            opened = os.fstat(self.descriptor)
            if exact_file_identity(named) != exact_file_identity(opened):
                raise SourceStateError("Git executable changed while it was opened")
            self.identity = exact_file_identity(opened)
            self.sha256 = hash_descriptor(self.descriptor, opened.st_size)
            self.executable = f"/proc/self/fd/{self.descriptor}"
            self.validate()
        except (OSError, SourceStateError) as error:
            self.close()
            if isinstance(error, SourceStateError):
                raise
            raise SourceStateError(f"cannot pin Git executable: {error}") from error

    def __enter__(self) -> PinnedGit:
        return self

    def __exit__(
        self,
        exception_type: type[BaseException] | None,
        exception: BaseException | None,
        traceback: TracebackType | None,
    ) -> None:
        self.close()

    def close(self) -> None:
        if self.descriptor >= 0:
            os.close(self.descriptor)
            self.descriptor = -1

    def validate(self) -> None:
        if self.descriptor < 0:
            raise SourceStateError("pinned Git executable descriptor is closed")
        try:
            named = self.path.stat(follow_symlinks=False)
            opened = os.fstat(self.descriptor)
            proc_opened = os.stat(self.executable)
        except OSError as error:
            raise SourceStateError(
                f"cannot revalidate pinned Git executable: {error}"
            ) from error
        if (
            exact_file_identity(named) != self.identity
            or exact_file_identity(opened) != self.identity
            or exact_file_identity(proc_opened) != self.identity
            or hash_descriptor(self.descriptor, opened.st_size) != self.sha256
        ):
            raise SourceStateError("pinned Git executable identity or content changed")

    def run(self, root: pathlib.Path, *arguments: str) -> bytes:
        self.validate()
        try:
            completed = run_bounded(
                [
                    self.executable,
                    *GIT_CONFIG_ARGUMENTS,
                    "-C",
                    os.fspath(root),
                    *arguments,
                ],
                self.descriptor,
            )
        except (OSError, subprocess.SubprocessError) as error:
            raise SourceStateError(f"cannot execute pinned Git: {error}") from error
        finally:
            self.validate()
        if completed.returncode != 0:
            detail = completed.stderr[:4096].decode("utf-8", "replace").strip()
            raise SourceStateError(f"Git {' '.join(arguments)} failed: {detail}")
        return completed.stdout


def git(git_tool: PinnedGit, root: pathlib.Path, *arguments: str) -> bytes:
    try:
        return git_tool.run(root, *arguments)
    except SourceStateError as error:
        raise SourceStateError(f"Git {' '.join(arguments)} failed: {error}") from error


def decode_object(value: bytes, label: str) -> str:
    try:
        decoded = value.decode("ascii").strip()
    except UnicodeDecodeError as error:
        raise SourceStateError(f"{label} is not ASCII") from error
    if not GIT_OBJECT.fullmatch(decoded) or set(decoded) == {"0"}:
        raise SourceStateError(f"{label} is not a canonical Git object ID")
    return decoded


def canonical_root(root: pathlib.Path, git_tool: PinnedGit) -> pathlib.Path:
    try:
        canonical = root.resolve(strict=True)
        top_level = pathlib.Path(
            git(git_tool, canonical, "rev-parse", "--show-toplevel")
            .decode("utf-8")
            .strip()
        ).resolve(strict=True)
    except (OSError, UnicodeDecodeError) as error:
        raise SourceStateError(f"source root cannot be resolved: {error}") from error
    if top_level != canonical:
        raise SourceStateError("source root is not the exact Git worktree root")
    return canonical


def require_clean(root: pathlib.Path, git_tool: PinnedGit) -> None:
    status_output = git(
        git_tool,
        root,
        "status",
        "--porcelain=v1",
        "-z",
        "--untracked-files=all",
        "--ignore-submodules=none",
    )
    if status_output:
        raise SourceStateError(
            "source worktree must be exactly clean before evidence generation"
        )


def inspect(root: pathlib.Path, git_tool: PinnedGit) -> tuple[str, str]:
    state = parse_state(capture_state(root, git_tool))
    return str(state["source_commit"]), str(state["source_tree"])


def hash_open_file(
    descriptor: int, size: int, object_format: str, path: bytes
) -> tuple[str, str]:
    try:
        object_digest = hashlib.new(object_format)
    except ValueError as error:
        raise SourceStateError(
            f"unsupported Git object format {object_format!r}"
        ) from error
    object_digest.update(f"blob {size}\0".encode("ascii"))
    content_digest = hashlib.sha256()
    offset = 0
    while offset < size:
        chunk = os.pread(descriptor, min(COPY_CHUNK_BYTES, size - offset), offset)
        if not chunk:
            raise SourceStateError(
                f"tracked source became truncated: {os.fsdecode(path)!r}"
            )
        object_digest.update(chunk)
        content_digest.update(chunk)
        offset += len(chunk)
    if offset != size:
        raise SourceStateError(f"tracked source size changed: {os.fsdecode(path)!r}")
    return object_digest.hexdigest(), content_digest.hexdigest()


def file_identity(value: os.stat_result) -> dict[str, int]:
    return {
        "device": value.st_dev,
        "inode": value.st_ino,
        "mode": value.st_mode,
        "links": value.st_nlink,
        "size": value.st_size,
        "mtime_ns": value.st_mtime_ns,
        "ctime_ns": value.st_ctime_ns,
    }


def parse_index_entries(data: bytes) -> list[tuple[str, str, bytes]]:
    entries: list[tuple[str, str, bytes]] = []
    records = data.split(b"\0")
    if not records or records[-1] != b"":
        raise SourceStateError("Git tracked-path output is not NUL terminated")
    for record in records[:-1]:
        metadata, separator, path = record.partition(b"\t")
        columns = metadata.split(b" ")
        if not separator or len(columns) != 3 or not path:
            raise SourceStateError("Git tracked-path entry is malformed")
        mode_bytes, object_bytes, stage = columns
        try:
            mode = mode_bytes.decode("ascii")
            object_id = object_bytes.decode("ascii")
        except UnicodeDecodeError as error:
            raise SourceStateError("Git tracked metadata is not ASCII") from error
        if stage != b"0" or not GIT_OBJECT.fullmatch(object_id):
            raise SourceStateError(
                "Git index contains a noncanonical or unmerged entry"
            )
        if path.startswith(b"/") or any(
            component in (b"", b".", b"..") for component in path.split(b"/")
        ):
            raise SourceStateError("Git tracked path is not canonical and relative")
        entries.append((mode, object_id, path))
    if not entries:
        raise SourceStateError("Git tracked-path list is empty")
    return entries


def capture_state(root: pathlib.Path, git_tool: PinnedGit) -> bytes:
    root = canonical_root(root, git_tool)
    require_clean(root, git_tool)
    commit = decode_object(
        git(git_tool, root, "rev-parse", "--verify", "HEAD"), "source commit"
    )
    tree = decode_object(
        git(git_tool, root, "rev-parse", "--verify", "HEAD^{tree}"), "source tree"
    )
    try:
        object_format = (
            git(git_tool, root, "rev-parse", "--show-object-format")
            .decode("ascii")
            .strip()
        )
    except UnicodeDecodeError as error:
        raise SourceStateError("Git object format is not ASCII") from error
    if object_format not in {"sha1", "sha256"}:
        raise SourceStateError("Git object format is unsupported")
    raw_index = git(git_tool, root, "ls-files", "--stage", "-z")
    tracked: list[dict[str, object]] = []
    for git_mode, git_object, raw_path in parse_index_entries(raw_index):
        if git_mode not in REGULAR_GIT_MODES:
            raise SourceStateError(
                "source tree contains unsupported tracked Git mode "
                f"{git_mode}: {os.fsdecode(raw_path)!r}"
            )
        path = root / os.fsdecode(raw_path)
        flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
        try:
            descriptor = os.open(path, flags)
        except OSError as error:
            raise SourceStateError(
                f"cannot open tracked source safely: {os.fsdecode(raw_path)!r}: {error}"
            ) from error
        try:
            before = os.fstat(descriptor)
            if not stat.S_ISREG(before.st_mode) or before.st_nlink < 1:
                raise SourceStateError(
                    f"tracked Git blob is not a regular file: {os.fsdecode(raw_path)!r}"
                )
            executable = before.st_mode & 0o111 != 0
            if executable != (git_mode == "100755"):
                raise SourceStateError(
                    f"tracked source executable mode differs from Git: {os.fsdecode(raw_path)!r}"
                )
            observed_object, content_sha256 = hash_open_file(
                descriptor, before.st_size, object_format, raw_path
            )
            after = os.fstat(descriptor)
            if file_identity(before) != file_identity(after):
                raise SourceStateError(
                    f"tracked source changed while being captured: {os.fsdecode(raw_path)!r}"
                )
            if observed_object != git_object:
                raise SourceStateError(
                    f"tracked source content differs from Git: {os.fsdecode(raw_path)!r}"
                )
            tracked.append(
                {
                    "path_hex": raw_path.hex(),
                    "git_mode": git_mode,
                    "git_object": git_object,
                    "content_sha256": content_sha256,
                    **file_identity(before),
                }
            )
        finally:
            os.close(descriptor)
    if not tracked:
        raise SourceStateError("source tree contains no tracked regular Git blobs")
    require_clean(root, git_tool)
    if (
        decode_object(
            git(git_tool, root, "rev-parse", "--verify", "HEAD"), "source commit"
        )
        != commit
    ):
        raise SourceStateError("source HEAD changed while source state was captured")
    if (
        decode_object(
            git(git_tool, root, "rev-parse", "--verify", "HEAD^{tree}"),
            "source tree",
        )
        != tree
    ):
        raise SourceStateError("source tree changed while source state was captured")
    if git(git_tool, root, "ls-files", "--stage", "-z") != raw_index:
        raise SourceStateError("Git index changed while source state was captured")
    state = {
        "format": "fe2o3-s09-source-state-v1",
        "source_commit": commit,
        "source_tree": tree,
        "git_object_format": object_format,
        "git_index_sha256": hashlib.sha256(raw_index).hexdigest(),
        "tracked_regular_blobs": tracked,
    }
    encoded = (
        json.dumps(
            state, sort_keys=True, separators=(",", ":"), ensure_ascii=True
        ).encode("ascii")
        + b"\n"
    )
    if not 1 <= len(encoded) <= MAX_STATE_BYTES:
        raise SourceStateError("canonical source state exceeds its sealed bound")
    return encoded


def parse_state(data: bytes) -> dict[str, object]:
    try:
        state = json.loads(data)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise SourceStateError("sealed source state is not canonical JSON") from error
    if not isinstance(state, dict) or set(state) != {
        "format",
        "source_commit",
        "source_tree",
        "git_object_format",
        "git_index_sha256",
        "tracked_regular_blobs",
    }:
        raise SourceStateError("sealed source-state schema changed")
    if state["format"] != "fe2o3-s09-source-state-v1":
        raise SourceStateError("sealed source-state format changed")
    for field in ("source_commit", "source_tree"):
        value = state[field]
        if not isinstance(value, str) or not GIT_OBJECT.fullmatch(value):
            raise SourceStateError(f"sealed source-state {field} is malformed")
    object_format = state["git_object_format"]
    if object_format not in {"sha1", "sha256"}:
        raise SourceStateError("sealed source-state Git object format is invalid")
    expected_object_length = 40 if object_format == "sha1" else 64
    if any(
        len(str(state[field])) != expected_object_length
        for field in ("source_commit", "source_tree")
    ):
        raise SourceStateError("sealed source-state Git object length changed")
    index_sha256 = state["git_index_sha256"]
    if not isinstance(index_sha256, str) or not re.fullmatch(
        r"[0-9a-f]{64}", index_sha256
    ):
        raise SourceStateError("sealed source-state index digest is malformed")
    entries = state["tracked_regular_blobs"]
    if not isinstance(entries, list) or not entries:
        raise SourceStateError("sealed source-state tracked blob list is malformed")
    entry_fields = {
        "path_hex",
        "git_mode",
        "git_object",
        "content_sha256",
        "device",
        "inode",
        "mode",
        "links",
        "size",
        "mtime_ns",
        "ctime_ns",
    }
    prior_path: bytes | None = None
    for entry in entries:
        if not isinstance(entry, dict) or set(entry) != entry_fields:
            raise SourceStateError("sealed tracked-blob schema changed")
        path_hex = entry["path_hex"]
        try:
            raw_path = bytes.fromhex(path_hex)
        except (TypeError, ValueError) as error:
            raise SourceStateError("sealed tracked-blob path is malformed") from error
        if not raw_path or raw_path.hex() != path_hex:
            raise SourceStateError("sealed tracked-blob path is noncanonical")
        if prior_path is not None and raw_path <= prior_path:
            raise SourceStateError("sealed tracked-blob paths are not strictly ordered")
        prior_path = raw_path
        if entry["git_mode"] not in REGULAR_GIT_MODES:
            raise SourceStateError("sealed tracked-blob Git mode is invalid")
        git_object = entry["git_object"]
        if (
            not isinstance(git_object, str)
            or len(git_object) != expected_object_length
            or not GIT_OBJECT.fullmatch(git_object)
        ):
            raise SourceStateError("sealed tracked-blob Git object is malformed")
        content_sha256 = entry["content_sha256"]
        if not isinstance(content_sha256, str) or not re.fullmatch(
            r"[0-9a-f]{64}", content_sha256
        ):
            raise SourceStateError("sealed tracked-blob content digest is malformed")
        for field in (
            "device",
            "inode",
            "mode",
            "links",
            "size",
            "mtime_ns",
            "ctime_ns",
        ):
            if isinstance(entry[field], bool) or not isinstance(entry[field], int):
                raise SourceStateError(
                    f"sealed tracked-blob identity field {field!r} is malformed"
                )
        if entry["device"] < 0 or entry["inode"] <= 0 or entry["links"] < 1:
            raise SourceStateError("sealed tracked-blob identity is invalid")
    canonical = (
        json.dumps(
            state, sort_keys=True, separators=(",", ":"), ensure_ascii=True
        ).encode("ascii")
        + b"\n"
    )
    if canonical != data:
        raise SourceStateError("sealed source-state serialization is noncanonical")
    return state


def seal_state(data: bytes) -> int:
    if not hasattr(os, "memfd_create"):
        raise SourceStateError("source-state supervision requires Linux memfd_create")
    try:
        descriptor = os.memfd_create(
            "fe2o3-s09-source-state", os.MFD_CLOEXEC | os.MFD_ALLOW_SEALING
        )
        written = 0
        while written < len(data):
            count = os.write(descriptor, data[written:])
            if count <= 0:
                raise SourceStateError("sealed source-state write made no progress")
            written += count
        os.fsync(descriptor)
        fcntl.fcntl(descriptor, fcntl.F_ADD_SEALS, REQUIRED_SEALS)
        if (
            fcntl.fcntl(descriptor, fcntl.F_GET_SEALS) & REQUIRED_SEALS
            != REQUIRED_SEALS
        ):
            raise SourceStateError("sealed source state is missing required seals")
        return descriptor
    except OSError as error:
        try:
            os.close(descriptor)
        except (OSError, UnboundLocalError):
            pass
        raise SourceStateError(f"cannot seal source state: {error}") from error


def read_sealed_state(descriptor: int, size: int) -> bytes:
    try:
        data = os.pread(descriptor, size + 1, 0)
    except OSError as error:
        raise SourceStateError(f"cannot read sealed source state: {error}") from error
    if len(data) != size:
        raise SourceStateError("sealed source state changed or was truncated")
    return data


def substitute_command(command: Sequence[str], state: dict[str, object]) -> list[str]:
    substitutions = {
        "{source_commit}": str(state["source_commit"]),
        "{source_tree}": str(state["source_tree"]),
    }
    result: list[str] = []
    for argument in command:
        replaced = argument
        for placeholder, value in substitutions.items():
            replaced = replaced.replace(placeholder, value)
        if "{source_" in replaced:
            raise SourceStateError(
                f"unresolved source-state placeholder in argument: {argument}"
            )
        result.append(replaced)
    return result


def observe_unreaped_exit(process_id: int) -> int | None:
    """Return a Popen-style status without releasing the process identity."""
    try:
        result = os.waitid(
            os.P_PID,
            process_id,
            os.WEXITED | os.WNOHANG | os.WNOWAIT,
        )
    except ChildProcessError as error:
        raise SourceStateError(
            "supervised process was reaped before group cleanup"
        ) from error
    if result is None:
        return None
    if result.si_code == os.CLD_EXITED:
        return result.si_status
    if result.si_code in (os.CLD_KILLED, os.CLD_DUMPED):
        return -result.si_status
    raise SourceStateError("supervised process reported an unexpected wait status")


def process_group_members(process_group: int, leader: int) -> set[int]:
    """Find live or zombie group members other than the pinned leader."""
    members: set[int] = set()
    try:
        process_entries = pathlib.Path("/proc").iterdir()
    except OSError as error:
        raise SourceStateError(f"cannot inspect supervised process group: {error}") from error
    for entry in process_entries:
        if not entry.name.isdigit():
            continue
        try:
            value = (entry / "stat").read_text(encoding="ascii")
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            continue
        except OSError as error:
            raise SourceStateError(
                f"cannot inspect supervised process {entry.name}: {error}"
            ) from error
        closing_parenthesis = value.rfind(")")
        fields = value[closing_parenthesis + 2 :].split()
        if closing_parenthesis < 0 or len(fields) < 3:
            raise SourceStateError(
                f"cannot parse supervised process identity for {entry.name}"
            )
        try:
            member_group = int(fields[2])
            process_id = int(entry.name)
        except ValueError as error:
            raise SourceStateError(
                f"cannot parse supervised process identity for {entry.name}"
            ) from error
        if member_group == process_group and process_id != leader:
            members.add(process_id)
    return members


def signal_process_group(process_id: int, signal_number: int) -> None:
    """Signal a process group only while its unreaped leader pins the PGID."""
    own_group = os.getpgrp()
    if process_id <= 1 or process_id == own_group:
        raise SourceStateError("refusing to signal the source-state supervisor group")
    try:
        observed_group = os.getpgid(process_id)
    except ProcessLookupError as error:
        raise SourceStateError(
            "supervised process-group leader disappeared before reap"
        ) from error
    if observed_group != process_id:
        raise SourceStateError("supervised command does not own its process group")
    try:
        os.killpg(process_id, signal_number)
    except ProcessLookupError:
        # A group containing only an unreaped zombie has no signalable members.
        # Group observation is deliberately not required to make this decision.
        pass


def wait_for_unreaped_exit(
    process_id: int,
    pid_descriptor: int,
    timeout: float,
) -> int | None:
    deadline = time.monotonic() + timeout
    if pid_descriptor < 0:
        while True:
            returncode = observe_unreaped_exit(process_id)
            if returncode is not None:
                return returncode
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                return None
            time.sleep(min(PROCESS_POLL_SECONDS, remaining))

    selector = selectors.DefaultSelector()
    try:
        selector.register(pid_descriptor, selectors.EVENT_READ)
        while True:
            returncode = observe_unreaped_exit(process_id)
            if returncode is not None:
                return returncode
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                return None
            selector.select(remaining)
    finally:
        selector.close()


def wait_for_group_drain(
    process_id: int,
    timeout: float,
    owner: CatchableSignalOwner,
    forwarded_signal: int | None,
) -> int | None:
    deadline = time.monotonic() + timeout
    while True:
        if owner.signal_number is not None and forwarded_signal is None:
            signal_process_group(process_id, owner.signal_number)
            forwarded_signal = owner.signal_number
        if not process_group_members(process_id, process_id):
            return forwarded_signal
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            return forwarded_signal
        time.sleep(min(PROCESS_POLL_SECONDS, remaining))


def reap_observed_leader(
    process: subprocess.Popen[bytes],
    process_id: int,
    owner: CatchableSignalOwner,
) -> int:
    """Reap only after all numeric process-group operations are complete."""
    owner.block_and_capture_pending()
    try:
        waited_process, wait_status = os.waitpid(process_id, 0)
    except (ChildProcessError, OSError) as error:
        raise SourceStateError("cannot reap supervised process-group leader") from error
    if waited_process != process_id:
        raise SourceStateError("reaped an unexpected supervised process")
    process.returncode = os.waitstatus_to_exitcode(wait_status)
    owner.block_and_capture_pending()
    return process.returncode


def mandatory_kill_and_reap(
    process: subprocess.Popen[bytes],
    process_id: int,
    pid_descriptor: int,
    owner: CatchableSignalOwner,
) -> int:
    """Kill the protected group and reap its leader without using /proc."""
    try:
        signal_process_group(process_id, signal.SIGKILL)
    except (OSError, SourceStateError) as error:
        raise SourceStateError(
            "mandatory supervised process-group SIGKILL failed"
        ) from error
    returncode = wait_for_unreaped_exit(
        process_id, pid_descriptor, KILL_GRACE_SECONDS
    )
    if returncode is None:
        raise SourceStateError(
            "supervised process-group leader remained unreaped after bounded SIGKILL"
        )
    reaped_returncode = reap_observed_leader(process, process_id, owner)
    if reaped_returncode != returncode:
        raise SourceStateError("supervised process exit status changed before reap")
    return reaped_returncode


def run_supervised_command(
    command: Sequence[str], owner: CatchableSignalOwner
) -> int:
    if owner.signal_number is not None:
        return 0
    try:
        process = subprocess.Popen(
            command,
            stdin=None,
            stdout=None,
            stderr=None,
            start_new_session=True,
        )
    except OSError as error:
        raise SourceStateError(f"cannot execute supervised command: {error}") from error

    process_id = process.pid
    pid_descriptor = -1
    reaped = False
    returncode = 0
    lifecycle_error: BaseException | None = None
    try:
        if not hasattr(os, "pidfd_open"):
            raise SourceStateError("source-state supervision requires Linux pidfds")
        pid_descriptor = os.pidfd_open(process_id, 0)
        if process_id <= 1 or process_id == os.getpgrp():
            raise SourceStateError(
                "supervised command has an unsafe process-group identity"
            )
        try:
            observed_group = os.getpgid(process_id)
        except ProcessLookupError as error:
            raise SourceStateError(
                "supervised command disappeared before group validation"
            ) from error
        if observed_group != process_id:
            raise SourceStateError("supervised command did not create a new session")

        forwarded_signal: int | None = None
        # Publish the validated identity before observing exit. A signal caught
        # during Popen or pidfd publication is forwarded at this first checkpoint.
        if owner.signal_number is not None:
            forwarded_signal = owner.signal_number
            signal_process_group(process_id, forwarded_signal)
        observed_returncode = observe_unreaped_exit(process_id)
        while observed_returncode is None:
            if owner.signal_number is None:
                time.sleep(PROCESS_POLL_SECONDS)
                observed_returncode = observe_unreaped_exit(process_id)
                continue

            if forwarded_signal is None:
                forwarded_signal = owner.signal_number
                signal_process_group(process_id, forwarded_signal)
            observed_returncode = wait_for_unreaped_exit(
                process_id, pid_descriptor, SIGNAL_GRACE_SECONDS
            )
            if observed_returncode is None:
                signal_process_group(process_id, signal.SIGTERM)
                observed_returncode = wait_for_unreaped_exit(
                    process_id, pid_descriptor, TERMINATE_GRACE_SECONDS
                )
            if observed_returncode is None:
                signal_process_group(process_id, signal.SIGKILL)
                observed_returncode = wait_for_unreaped_exit(
                    process_id, pid_descriptor, KILL_GRACE_SECONDS
                )
            if observed_returncode is None:
                raise SourceStateError(
                    "supervised process did not exit after bounded SIGKILL"
                )

        if owner.signal_number is not None and forwarded_signal is None:
            forwarded_signal = owner.signal_number
            signal_process_group(process_id, forwarded_signal)

        # Never use /proc observation to decide whether to terminate the group.
        # Preserve enough grace for same-group profile layers to finish their
        # nested raw-guard teardown before unconditional TERM/KILL escalation.
        if SIGNAL_GRACE_SECONDS <= NESTED_CLEANUP_BOUND_SECONDS:
            raise SourceStateError(
                "signal grace does not cover nested source-state cleanup"
            )
        forwarded_signal = wait_for_group_drain(
            process_id,
            SIGNAL_GRACE_SECONDS,
            owner,
            forwarded_signal,
        )

        # The unreaped leader pins the PGID through both unconditional signals.
        signal_process_group(process_id, signal.SIGTERM)
        forwarded_signal = wait_for_group_drain(
            process_id,
            TERMINATE_GRACE_SECONDS,
            owner,
            forwarded_signal,
        )
        signal_process_group(process_id, signal.SIGKILL)
        forwarded_signal = wait_for_group_drain(
            process_id,
            KILL_GRACE_SECONDS,
            owner,
            forwarded_signal,
        )
        remaining = process_group_members(process_id, process_id)
        if remaining:
            raise SourceStateError(
                "supervised process group retained members after bounded SIGKILL: "
                + ",".join(str(member) for member in sorted(remaining))
            )

        returncode = reap_observed_leader(process, process_id, owner)
        reaped = True
        if returncode != observed_returncode:
            raise SourceStateError("supervised process exit status changed before reap")
    except BaseException as error:
        if process.returncode is not None:
            reaped = True
        lifecycle_error = error

    if not reaped:
        try:
            returncode = mandatory_kill_and_reap(
                process, process_id, pid_descriptor, owner
            )
            reaped = True
        except BaseException as cleanup_error:
            if process.returncode is not None:
                reaped = True
            if lifecycle_error is None:
                lifecycle_error = cleanup_error
            else:
                combined = SourceStateError(
                    f"{lifecycle_error}; mandatory cleanup failed: {cleanup_error}"
                )
                combined.__cause__ = cleanup_error
                lifecycle_error = combined

    try:
        if pid_descriptor >= 0:
            os.close(pid_descriptor)
    except OSError as error:
        if lifecycle_error is None:
            lifecycle_error = SourceStateError(
                f"cannot close supervised process pidfd: {error}"
            )

    if lifecycle_error is not None:
        raise lifecycle_error
    if not reaped:
        raise SourceStateError("supervised process-group leader was not reaped")
    return returncode


def supervise(
    root: pathlib.Path,
    command: Sequence[str],
    git_tool: PinnedGit,
    owner: CatchableSignalOwner,
) -> int:
    before = capture_state(root, git_tool)
    state = parse_state(before)
    descriptor = seal_state(before)
    try:
        sealed_before = read_sealed_state(descriptor, len(before))
        parse_state(sealed_before)
        returncode = 0
        lifecycle_error: BaseException | None = None
        try:
            if owner.signal_number is None:
                returncode = run_supervised_command(
                    substitute_command(command, state), owner
                )
            else:
                owner.block_and_capture_pending()
        except BaseException as error:
            lifecycle_error = error

        # A source mismatch outranks lifecycle failure, caught cancellation, and
        # child status. Signals stay latched and blocked throughout reinspection.
        owner.block_and_capture_pending()
        source_error: BaseException | None = None
        try:
            after = capture_state(root, git_tool)
            if (
                read_sealed_state(descriptor, len(before)) != sealed_before
                or after != sealed_before
            ):
                raise SourceStateError(
                    "tracked source identity or content changed during evidence generation"
                )
        except BaseException as error:
            source_error = error
        owner.block_and_capture_pending()
        if source_error is not None:
            raise source_error
        if lifecycle_error is not None:
            raise lifecycle_error
        if owner.signal_number is not None:
            return 128 + owner.signal_number
        if returncode < 0:
            return 128 - returncode
        return returncode
    finally:
        os.close(descriptor)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True, type=pathlib.Path)
    parser.add_argument("--expected-commit")
    parser.add_argument("--expected-tree")
    parser.add_argument("command", nargs=argparse.REMAINDER)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if (args.expected_commit is None) != (args.expected_tree is None):
        raise SourceStateError("expected commit and tree must be supplied together")
    signal_owner = CatchableSignalOwner() if args.command else None
    with PinnedGit() as git_tool:
        if args.command:
            assert signal_owner is not None
            if args.expected_commit is not None:
                raise SourceStateError(
                    "supervised mode does not accept expected Git objects"
                )
            if args.command[0] != "--" or len(args.command) == 1:
                raise SourceStateError("supervised command must follow --")
            return supervise(args.root, args.command[1:], git_tool, signal_owner)
        commit, tree = inspect(args.root, git_tool)
        if args.expected_commit is not None:
            if commit != args.expected_commit or tree != args.expected_tree:
                raise SourceStateError(
                    "source HEAD or tree changed during evidence generation"
                )
    print(f"source_commit\t{commit}")
    print(f"source_tree\t{tree}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except SourceStateError as error:
        print(f"s09-source-state: {error}", file=sys.stderr)
        raise SystemExit(2)
