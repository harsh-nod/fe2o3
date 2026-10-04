#!/usr/bin/python3.12
"""Reopen, content-pin, and descriptor-execute one configured nested tool."""

from __future__ import annotations

import os
import sys
import hashlib
import stat


def main() -> int:
    if (
        len(sys.argv) < 6
        or sys.argv[1] != "--expected"
        or sys.argv[3] != "--sha256"
    ):
        print(
            "retained-tool-launcher: expected --expected PATH --sha256 HASH COMMAND...",
            file=sys.stderr,
        )
        return 125
    expected = sys.argv[2]
    expected_sha256 = sys.argv[4]
    command = sys.argv[5:]
    if not command or command[0] != expected:
        print("retained-tool-launcher: compiler pathname substitution", file=sys.stderr)
        return 125
    if (
        not os.path.isabs(expected)
        or os.path.realpath(expected) != expected
        or os.path.islink(expected)
        or len(expected_sha256) != 64
        or any(character not in "0123456789abcdef" for character in expected_sha256)
    ):
        print("retained-tool-launcher: invalid configured compiler identity", file=sys.stderr)
        return 125
    before = os.stat(expected, follow_symlinks=False)
    fd = os.open(expected, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
    try:
        opened = os.fstat(fd)
        digest = hashlib.sha256()
        offset = 0
        while offset < opened.st_size:
            chunk = os.pread(fd, min(1024 * 1024, opened.st_size - offset), offset)
            if not chunk:
                print("retained-tool-launcher: retained compiler truncated", file=sys.stderr)
                return 125
            digest.update(chunk)
            offset += len(chunk)
        if (
            not stat.S_ISREG(opened.st_mode)
            or opened.st_nlink != 1
            or opened.st_uid != 0
            or opened.st_gid != 0
            or stat.S_IMODE(opened.st_mode) != 0o755
            or (
                before.st_dev,
                before.st_ino,
                before.st_mode,
                before.st_nlink,
                before.st_uid,
                before.st_gid,
                before.st_size,
                before.st_mtime_ns,
                before.st_ctime_ns,
            )
            != (
                opened.st_dev,
                opened.st_ino,
                opened.st_mode,
                opened.st_nlink,
                opened.st_uid,
                opened.st_gid,
                opened.st_size,
                opened.st_mtime_ns,
                opened.st_ctime_ns,
            )
            or digest.hexdigest() != expected_sha256
        ):
            print("retained-tool-launcher: configured compiler identity changed", file=sys.stderr)
            return 125
        after = os.stat(expected, follow_symlinks=False)
        if (
            after.st_dev,
            after.st_ino,
            after.st_mode,
            after.st_nlink,
            after.st_uid,
            after.st_gid,
            after.st_size,
            after.st_mtime_ns,
            after.st_ctime_ns,
        ) != (
            opened.st_dev,
            opened.st_ino,
            opened.st_mode,
            opened.st_nlink,
            opened.st_uid,
            opened.st_gid,
            opened.st_size,
            opened.st_mtime_ns,
            opened.st_ctime_ns,
        ):
            print("retained-tool-launcher: compiler changed during validation", file=sys.stderr)
            return 125
        os.execve(fd, [expected, *command[1:]], dict(os.environ))
    finally:
        os.close(fd)
    return 125


if __name__ == "__main__":
    raise SystemExit(main())
