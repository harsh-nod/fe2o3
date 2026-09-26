#!/usr/bin/env python3
"""Report telemetry socket prerequisites without relaxing endpoint admission."""
import json
import socket


def probe():
    left, right = socket.socketpair(socket.AF_UNIX, socket.SOCK_SEQPACKET)
    try:
        calls = (
            ("SO_DOMAIN", lambda: left.getsockopt(socket.SOL_SOCKET, socket.SO_DOMAIN)),
            ("SO_TYPE", lambda: left.getsockopt(socket.SOL_SOCKET, socket.SO_TYPE)),
            ("getpeername", left.getpeername),
            ("SO_PEERCRED", lambda: left.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12).hex()),
            ("set_SO_PASSCRED", lambda: left.setsockopt(socket.SOL_SOCKET, socket.SO_PASSCRED, 1)),
            ("get_SO_PASSCRED", lambda: left.getsockopt(socket.SOL_SOCKET, socket.SO_PASSCRED)),
        )
        rows = []
        for name, call in calls:
            try:
                rows.append(dict(operation=name, result=call()))
            except OSError as error:
                rows.append(dict(operation=name, errno=error.errno, error=str(error)))
        return rows
    finally:
        left.close()
        right.close()


if __name__ == "__main__":
    print(json.dumps(probe(), sort_keys=True, indent=2))
