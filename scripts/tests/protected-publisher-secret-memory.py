#!/usr/bin/env python3
"""Exercise the shipped enrollment process's Linux secret-memory boundary."""

import json
import os
import socket
import subprocess
import sys
import tempfile
import time
from pathlib import Path


def canonical(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def main() -> int:
    binary = Path(sys.argv[1]).resolve()
    with tempfile.TemporaryDirectory(prefix="fe2o3-secret-memory-") as raw:
        root = Path(raw)
        os.chmod(root, 0o700)
        config_path = root / "config.json"
        artifact_path = root / "enrollment.json"
        config = {
            "allowed_actor_ids": ["74956"],
            "audience": "https://publisher.example/github-actions",
            "caller_workflow_path": ".github/workflows/parity-promotion.yml",
            "default_branch": "main",
            "enrollment_artifact_path": str(artifact_path),
            "environment": "protected-publisher",
            "issuer": "https://token.actions.githubusercontent.com",
            "jwks_cache_seconds": 300,
            "jwks_url": "https://token.actions.githubusercontent.com/.well-known/jwks",
            "ledger_path": str(root / "publisher.ledger"),
            "listen": "127.0.0.1:9443",
            "max_inflight_requests": 32,
            "max_ledger_bytes": 1048576,
            "max_receipts": 100,
            "network_deadline_milliseconds": 5000,
            "protected_workflow_path": ".github/workflows/parity-publisher-gate.yml",
            "repository": "powderluv/fe2o3",
            "repository_id": "1233498266",
            "repository_owner_id": "74956",
            "request_deadline_milliseconds": 10000,
            "schema_version": 2,
            "signature_domain": "production",
            "signing_key_id": "operator-publisher-v1",
            "signing_key_path": str(root / "publisher.pem"),
        }
        config_path.write_bytes(canonical(config))
        os.chmod(config_path, 0o600)

        writer, reader = socket.socketpair(socket.AF_UNIX, socket.SOCK_STREAM)
        reader.setblocking(False)
        marker = b"F2O3_SYNTHETIC_AF_UNIX_TOKEN_" + os.urandom(32).hex().encode()
        child = subprocess.Popen(
            [
                str(binary),
                "--enroll",
                "--config",
                str(config_path),
                "--token-fd",
                str(reader.fileno()),
                "--artifact",
                str(artifact_path),
            ],
            pass_fds=(reader.fileno(),),
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env={"PATH": "/usr/bin:/bin"},
        )
        reader.close()
        try:
            writer.sendall(marker)
            deadline = time.monotonic() + 2
            while time.monotonic() < deadline and child.poll() is None:
                try:
                    with open(f"/proc/{child.pid}/mem", "rb", buffering=0):
                        pass
                except PermissionError:
                    break
                time.sleep(0.01)
            else:
                raise RuntimeError("publisher did not establish /proc memory denial")
            if child.poll() is not None:
                raise RuntimeError(f"publisher exited before memory probe: {child.returncode}")

            cmdline = Path(f"/proc/{child.pid}/cmdline").read_bytes()
            try:
                environ = Path(f"/proc/{child.pid}/environ").read_bytes()
            except PermissionError:
                environ = b""
            if marker in cmdline or marker in environ:
                raise RuntimeError("synthetic token entered argv or environment")
            limits = Path(f"/proc/{child.pid}/limits").read_text(encoding="ascii")
            core = next(line for line in limits.splitlines() if line.startswith("Max core file size"))
            fields = core.split()
            if fields[-3:-1] != ["0", "0"]:
                raise RuntimeError(f"core limits are not both zero: {core}")
            status = Path(f"/proc/{child.pid}/status").read_text(encoding="ascii")
            uid = int(next(line for line in status.splitlines() if line.startswith("Uid:")).split()[1])
            if uid != os.geteuid():
                raise RuntimeError("probe child does not have the caller's effective UID")
        finally:
            writer.close()
            child.kill()
            child.wait()
    print("protected publisher secret-memory probe: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
