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
import stat
import subprocess
import sys
from collections.abc import Sequence


GIT_OBJECT = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})")
REGULAR_GIT_MODES = frozenset(("100644", "100755"))
MAX_GIT_OUTPUT_BYTES = 64 * 1024 * 1024
MAX_STATE_BYTES = 64 * 1024 * 1024
COPY_CHUNK_BYTES = 64 * 1024
REQUIRED_SEALS = (
    fcntl.F_SEAL_SEAL | fcntl.F_SEAL_SHRINK | fcntl.F_SEAL_GROW | fcntl.F_SEAL_WRITE
)


class SourceStateError(Exception):
    pass


def git(root: pathlib.Path, *arguments: str) -> bytes:
    try:
        completed = subprocess.run(
            ["git", "-C", os.fspath(root), *arguments],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=60,
            check=False,
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise SourceStateError(f"cannot execute git: {error}") from error
    if len(completed.stdout) > MAX_GIT_OUTPUT_BYTES:
        raise SourceStateError("git output exceeds the source-state bound")
    if completed.returncode != 0:
        detail = completed.stderr[:4096].decode("utf-8", "replace").strip()
        raise SourceStateError(f"git {' '.join(arguments)} failed: {detail}")
    return completed.stdout


def decode_object(value: bytes, label: str) -> str:
    try:
        decoded = value.decode("ascii").strip()
    except UnicodeDecodeError as error:
        raise SourceStateError(f"{label} is not ASCII") from error
    if not GIT_OBJECT.fullmatch(decoded) or set(decoded) == {"0"}:
        raise SourceStateError(f"{label} is not a canonical Git object ID")
    return decoded


def canonical_root(root: pathlib.Path) -> pathlib.Path:
    try:
        canonical = root.resolve(strict=True)
        top_level = pathlib.Path(
            git(canonical, "rev-parse", "--show-toplevel")
            .decode("utf-8")
            .strip()
        ).resolve(strict=True)
    except (OSError, UnicodeDecodeError) as error:
        raise SourceStateError(f"source root cannot be resolved: {error}") from error
    if top_level != canonical:
        raise SourceStateError("source root is not the exact Git worktree root")
    return canonical


def require_clean(root: pathlib.Path) -> None:
    status_output = git(
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


def inspect(root: pathlib.Path) -> tuple[str, str]:
    root = canonical_root(root)
    require_clean(root)
    commit = decode_object(git(root, "rev-parse", "--verify", "HEAD"), "source commit")
    tree = decode_object(
        git(root, "rev-parse", "--verify", "HEAD^{tree}"), "source tree"
    )
    return commit, tree


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
            raise SourceStateError("Git index contains a noncanonical or unmerged entry")
        if path.startswith(b"/") or any(
            component in (b"", b".", b"..") for component in path.split(b"/")
        ):
            raise SourceStateError("Git tracked path is not canonical and relative")
        entries.append((mode, object_id, path))
    if not entries:
        raise SourceStateError("Git tracked-path list is empty")
    return entries


def capture_state(root: pathlib.Path) -> bytes:
    root = canonical_root(root)
    require_clean(root)
    commit = decode_object(git(root, "rev-parse", "--verify", "HEAD"), "source commit")
    tree = decode_object(
        git(root, "rev-parse", "--verify", "HEAD^{tree}"), "source tree"
    )
    try:
        object_format = git(root, "rev-parse", "--show-object-format").decode(
            "ascii"
        ).strip()
    except UnicodeDecodeError as error:
        raise SourceStateError("Git object format is not ASCII") from error
    if object_format not in {"sha1", "sha256"}:
        raise SourceStateError("Git object format is unsupported")
    raw_index = git(root, "ls-files", "--stage", "-z")
    tracked: list[dict[str, object]] = []
    for git_mode, git_object, raw_path in parse_index_entries(raw_index):
        if git_mode not in REGULAR_GIT_MODES:
            continue
        path = root / os.fsdecode(raw_path)
        flags = (
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0)
        )
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
    require_clean(root)
    if decode_object(git(root, "rev-parse", "--verify", "HEAD"), "source commit") != commit:
        raise SourceStateError("source HEAD changed while source state was captured")
    if (
        decode_object(
            git(root, "rev-parse", "--verify", "HEAD^{tree}"), "source tree"
        )
        != tree
    ):
        raise SourceStateError("source tree changed while source state was captured")
    if git(root, "ls-files", "--stage", "-z") != raw_index:
        raise SourceStateError("Git index changed while source state was captured")
    state = {
        "format": "fe2o3-s09-source-state-v1",
        "source_commit": commit,
        "source_tree": tree,
        "git_object_format": object_format,
        "git_index_sha256": hashlib.sha256(raw_index).hexdigest(),
        "tracked_regular_blobs": tracked,
    }
    encoded = json.dumps(
        state, sort_keys=True, separators=(",", ":"), ensure_ascii=True
    ).encode("ascii") + b"\n"
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
        if (
            not isinstance(content_sha256, str)
            or not re.fullmatch(r"[0-9a-f]{64}", content_sha256)
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
    canonical = json.dumps(
        state, sort_keys=True, separators=(",", ":"), ensure_ascii=True
    ).encode("ascii") + b"\n"
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
        if fcntl.fcntl(descriptor, fcntl.F_GET_SEALS) & REQUIRED_SEALS != REQUIRED_SEALS:
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


def supervise(root: pathlib.Path, command: Sequence[str]) -> int:
    before = capture_state(root)
    state = parse_state(before)
    descriptor = seal_state(before)
    try:
        sealed_before = read_sealed_state(descriptor, len(before))
        parse_state(sealed_before)
        try:
            completed = subprocess.run(
                substitute_command(command, state),
                stdin=None,
                stdout=None,
                stderr=None,
                check=False,
            )
        except OSError as error:
            raise SourceStateError(f"cannot execute supervised command: {error}") from error
        after = capture_state(root)
        if read_sealed_state(descriptor, len(before)) != sealed_before or after != sealed_before:
            raise SourceStateError(
                "tracked source identity or content changed during evidence generation"
            )
        if completed.returncode < 0:
            return 128 - completed.returncode
        return completed.returncode
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
    if args.command:
        if args.expected_commit is not None:
            raise SourceStateError("supervised mode does not accept expected Git objects")
        if args.command[0] != "--" or len(args.command) == 1:
            raise SourceStateError("supervised command must follow --")
        return supervise(args.root, args.command[1:])
    commit, tree = inspect(args.root)
    if args.expected_commit is not None:
        if commit != args.expected_commit or tree != args.expected_tree:
            raise SourceStateError("source HEAD or tree changed during evidence generation")
    print(f"source_commit\t{commit}")
    print(f"source_tree\t{tree}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except SourceStateError as error:
        print(f"s09-source-state: {error}", file=sys.stderr)
        raise SystemExit(2)
