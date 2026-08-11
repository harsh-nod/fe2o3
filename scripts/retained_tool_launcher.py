#!/usr/bin/python3.12
"""Execute one controller-retained tool while validating the advertised path."""

from __future__ import annotations

import os
import sys
import fcntl
import hashlib


REQUIRED_SEALS = 0x0001 | 0x0002 | 0x0004 | 0x0008


def main() -> int:
    if (
        len(sys.argv) < 8
        or sys.argv[1] != "--expected"
        or sys.argv[3] != "--retained"
        or sys.argv[5] != "--sha256"
    ):
        print(
            "retained-tool-launcher: expected --expected PATH --retained PROC_FD COMMAND...",
            file=sys.stderr,
        )
        return 125
    expected = sys.argv[2]
    retained = sys.argv[4]
    expected_sha256 = sys.argv[6]
    command = sys.argv[7:]
    if not command or command[0] != expected:
        print("retained-tool-launcher: compiler pathname substitution", file=sys.stderr)
        return 125
    # The direct parent may be Ninja rather than the evidence controller. Accept
    # only a canonical proc-fd spelling and let open/exec fail closed if its
    # controller has exited or closed the retained descriptor.
    parts = retained.split("/")
    if (
        len(parts) != 5
        or parts[1] != "proc"
        or not parts[2].isdigit()
        or parts[3] != "fd"
        or not parts[4].isdigit()
        or len(expected_sha256) != 64
        or any(character not in "0123456789abcdef" for character in expected_sha256)
    ):
        print("retained-tool-launcher: invalid retained descriptor", file=sys.stderr)
        return 125
    fd = os.open(retained, os.O_RDONLY)
    try:
        opened = os.fstat(fd)
        linked = os.readlink(retained)
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
            not os.path.isfile(retained)
            or not linked.startswith("/memfd:fe2o3-configured-tool:cxx")
            or fcntl.fcntl(fd, 1034) != REQUIRED_SEALS
            or digest.hexdigest() != expected_sha256
        ):
            print("retained-tool-launcher: retained compiler is not sealed", file=sys.stderr)
            return 125
        os.execve(fd, [expected, *command[1:]], dict(os.environ))
    finally:
        os.close(fd)
    return 125


if __name__ == "__main__":
    raise SystemExit(main())
