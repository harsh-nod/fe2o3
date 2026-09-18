#!/usr/bin/env python3
"""Local-only signed-cohort export. No SSH, GPU observation, Cargo or execution."""

import argparse
import json
from pathlib import Path
import shutil
import subprocess
import time

import protocol as P

CPU = Path("docs/evidence/dev-generic-sdma-example-cpu-2026-09-18")
SELECTED = {
    "benchmarks/runtime_gfx942/copy-host-observe.py": "source/copy-host-observe.py",
    "benchmarks/runtime_gfx942/r26-host-guard.py": "source/r26-host-guard.py",
    "crates/fe2o3-kfd/examples/kfd-compute-aql-queue.rs": "source/kfd-compute-aql-queue.rs",
    "crates/fe2o3-kfd/src/sdma/retained_release.rs": "source/retained_release.rs",
    "crates/fe2o3-kfd/src/queue_live/primary_release.rs": "source/primary_release.rs",
    "crates/fe2o3-kfd/src/queue_live/primary_release/driver.rs": "source/driver.rs",
    "docs/runtime-primary-queue-release-v1.md": "source/runtime-primary-queue-release-v1.md",
}


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    repository, output = args.repository.resolve(), args.output.resolve()
    P.need(not output.exists(), "fresh local output only")
    cpu = repository / CPU
    P.need(P.sha(cpu / "SHA256SUMS") == P.CPU_SEAL, "final CPU evidence seal")
    manifest = {}
    for line in (cpu / "SHA256SUMS").read_text().splitlines():
        digest, path = line.split("  ", 1)
        path = Path(path).as_posix()
        P.need(
            path not in manifest
            and not Path(path).is_absolute()
            and ".." not in Path(path).parts,
            "CPU manifest path",
        )
        P.need(
            (cpu / path).is_file()
            and not (cpu / path).is_symlink()
            and P.sha(cpu / path) == digest,
            "CPU evidence byte identity",
        )
        manifest[path] = digest
    P.need(
        {p.relative_to(cpu).as_posix() for p in cpu.rglob("*") if p.is_file()}
        == set(manifest) | {"SHA256SUMS"},
        "CPU archive closure",
    )
    before = cpu / "raw/source-before.log"
    after = cpu / "raw/source-after.log"
    P.need(P.sha(before) == P.sha(after) == P.COHORT_SHA, "unchanged final source maps")
    cohort = P.load(before)["files"]
    P.need(len(cohort) == 5555, "final source cohort size")
    started = time.time_ns()
    signature_command = [
        "git",
        "-c",
        "gpg.ssh.allowedSignersFile=/dev/shm/fe2o3-c4-build-20260918.hg9Ipphj/allowed-signers",
        "verify-commit",
        "--raw",
        P.COMMIT,
    ]
    signature = subprocess.run(
        signature_command,
        cwd=repository,
        capture_output=True,
        timeout=30,
    )
    P.need(signature.returncode == 0, "signed containing commit verifies")
    P.need(
        b"harmenon@amd.com" in signature.stderr
        and b"SHA256:q8oGVYZ11904aFzlMkSiEwyeSP+6hbuiZGbNVGRZVCg" in signature.stderr,
        "expected public commit signer",
    )
    process = subprocess.Popen(
        ["git", "cat-file", "--batch"],
        cwd=repository,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    matched = 0
    selected = {}
    try:
        for path, digest in cohort.items():
            P.need(
                not Path(path).is_absolute()
                and ".." not in Path(path).parts
                and "\n" not in path,
                "cohort path",
            )
            process.stdin.write(f"{P.COMMIT}:{path}\n".encode())
            process.stdin.flush()
            header = process.stdout.readline().decode().split()
            P.need(
                len(header) == 3 and header[1] == "blob", f"signed source blob: {path}"
            )
            remaining = int(header[2])
            hasher = P.hashlib.sha256()
            saved = bytearray() if path in SELECTED else None
            while remaining:
                data = process.stdout.read(min(remaining, 1024 * 1024))
                P.need(bool(data), "complete Git object")
                hasher.update(data)
                if saved is not None:
                    saved.extend(data)
                remaining -= len(data)
            P.need(process.stdout.read(1) == b"\n", "Git object boundary")
            P.need(
                hasher.hexdigest() == digest,
                f"qualified source differs from signed commit: {path}",
            )
            if saved is not None:
                selected[SELECTED[path]] = bytes(saved)
            matched += 1
        process.stdin.close()
        P.need(
            process.wait(timeout=30) == 0 and not process.stderr.read(),
            "Git source comparison completed",
        )
    finally:
        if process.poll() is None:
            process.kill()
            process.wait(timeout=5)
    binary = (
        repository
        / "target/x86_64-unknown-linux-musl/debug/examples/kfd-compute-aql-queue"
    )
    P.need(P.sha(binary) == P.BINARY_SHA, "qualified final musl binary still matches")
    output.mkdir(mode=0o700)
    P.need(output.stat().st_mode & 0o777 == 0o700, "private local staging directory")
    (output / "source").mkdir()
    (output / "cpu").mkdir()
    for path, data in selected.items():
        (output / path).write_bytes(data)
    for name in ("run.py", "protocol.py", "test_protocol.py", "PLAN.md", "prepare.py"):
        shutil.copyfile(Path(__file__).parent / name, output / name)
    for name in (
        "source-before.log",
        "source-after.log",
        "binary.log",
    ):
        shutil.copyfile(cpu / "raw" / name, output / "cpu" / name)
    shutil.copyfile(cpu / "SHA256SUMS", output / "cpu/SHA256SUMS")
    shutil.copyfile(binary, output / "queue-example")
    (output / "queue-example").chmod(0o700)
    (output / "commit-signature.stdout").write_bytes(signature.stdout)
    (output / "commit-signature.stderr").write_bytes(signature.stderr)
    write(
        output / "binding.json",
        {
            "commit": P.COMMIT,
            "source_files_matched": matched,
            "cpu_manifest_sha256": P.CPU_SEAL,
            "binary_sha256": P.BINARY_SHA,
            "cohort_sha256": P.COHORT_SHA,
            "started_ns": started,
            "finished_ns": time.time_ns(),
            "signature_command": signature_command,
            "signature_exit": signature.returncode,
            "cohort_command": ["git", "cat-file", "--batch"],
            "cohort_exit": process.returncode,
            "source_base_field_ignored_only": True,
            "native_authorized": False,
        },
    )
    payload = {
        p.relative_to(output).as_posix(): P.sha(p)
        for p in sorted(output.rglob("*"))
        if p.is_file()
    }
    write(output / "payload.json", payload)
    P.payload(output)
    print(
        json.dumps(
            {
                "local_staging": str(output),
                "directory_mode": oct(output.stat().st_mode & 0o777),
                "source_files_matched": matched,
                "binary_sha256": P.BINARY_SHA,
                "payload_sha256": P.sha(output / "payload.json"),
                "native_authorized": False,
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
