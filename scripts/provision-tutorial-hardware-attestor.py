#!/usr/bin/env python3
"""Provision an untrusted lane key and proposed policy outside the repository."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys

import tutorial_hardware_receipt as receipt


REPO_ROOT = Path(__file__).resolve().parent.parent


class ProvisionError(ValueError):
    """The requested attestor material cannot be provisioned safely."""


def fail(message: str) -> None:
    raise ProvisionError(message)


def publish(path: Path, payload: bytes, mode: int) -> None:
    parent = path.parent
    if parent.resolve(strict=True) != parent or parent.is_symlink():
        fail("publication parent must be an exact non-symlink directory")
    parent_fd = os.open(
        parent,
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0),
    )
    try:
        descriptor = os.open(
            path.name,
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            mode,
            dir_fd=parent_fd,
        )
        try:
            with os.fdopen(descriptor, "wb") as output:
                output.write(payload)
                output.flush()
                os.fsync(output.fileno())
            os.fsync(parent_fd)
        except BaseException:
            try:
                os.unlink(path.name, dir_fd=parent_fd)
            except FileNotFoundError:
                pass
            raise
    except BaseException:
        raise
    finally:
        os.close(parent_fd)


def provision(
    output_directory: Path,
    lane: str,
    target: str,
    reservation: str,
    attestor: str,
) -> dict[str, Path]:
    receipt._identity(lane, "hardware lane")
    receipt._identity(reservation, "hardware reservation")
    receipt._identity(attestor, "hardware attestor")
    if receipt.TARGET.fullmatch(target) is None:
        fail("hardware target is not canonical")
    if not output_directory.is_absolute() or output_directory != Path(
        os.path.normpath(str(output_directory))
    ):
        fail("output directory must be absolute and lexically normalized")
    parent = output_directory.parent
    try:
        parent_metadata = parent.lstat()
        parent_resolved = parent.resolve(strict=True)
    except OSError as error:
        fail(f"cannot resolve output parent: {error}")
    if (
        parent.is_symlink()
        or not stat.S_ISDIR(parent_metadata.st_mode)
        or parent_resolved != parent
    ):
        fail("output parent must be a real, non-symlink directory")
    if output_directory.is_relative_to(REPO_ROOT) or parent.is_relative_to(REPO_ROOT):
        fail("attestor material and trust policy must be outside the repository")
    if output_directory.exists() or output_directory.is_symlink():
        fail("output directory must not already exist")

    output_directory.mkdir(mode=0o700)
    private_key = output_directory / "attestor-private.pem"
    pending_private_key = output_directory / ".attestor-private.pem.pending"
    public_key = output_directory / "attestor-public.pem"
    policy = output_directory / "proposed-trust-policy-v1.json"
    try:
        process = receipt._openssl(
            ["genpkey", "-algorithm", "Ed25519", "-out", str(pending_private_key)]
        )
        if process.returncode != 0:
            fail("OpenSSL failed to generate the Ed25519 attestor key")
        pending_private_key.chmod(0o600)
        pending_fd = os.open(
            pending_private_key,
            os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0),
        )
        try:
            os.fsync(pending_fd)
        finally:
            os.close(pending_fd)
        output_fd = os.open(
            output_directory,
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_NOFOLLOW", 0),
        )
        try:
            os.rename(
                pending_private_key.name,
                private_key.name,
                src_dir_fd=output_fd,
                dst_dir_fd=output_fd,
            )
            os.fsync(output_fd)
        finally:
            os.close(output_fd)
        public_payload = receipt._derive_public_key(private_key)
        publish(public_key, public_payload, 0o644)
        policy_payload = receipt.canonical(
            receipt.trust_policy_document(
                [
                    {
                        "attestorIdentity": attestor,
                        "lane": lane,
                        "publicKeyPath": str(public_key),
                        "reservationIdentity": reservation,
                        "target": target,
                    }
                ]
            )
        )
        publish(policy, policy_payload, 0o600)
        receipt.load_trust_policy(policy)
    except BaseException:
        shutil.rmtree(output_directory, ignore_errors=True)
        raise
    return {"private_key": private_key, "public_key": public_key, "policy": policy}


def main(arguments: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--attestor-identity", required=True)
    parser.add_argument(
        "--acknowledge-proposed-policy-is-not-installed",
        action="store_true",
        help=argparse.SUPPRESS,
    )
    parser.add_argument("--lane", required=True)
    parser.add_argument("--output-directory", required=True, type=Path)
    parser.add_argument("--reservation", required=True)
    parser.add_argument("--target", required=True)
    options = parser.parse_args(arguments)
    if not options.acknowledge_proposed_policy_is_not_installed:
        print(
            "tutorial hardware attestor provisioning: explicitly acknowledge that the "
            "generated policy is only a proposal and will not be installed or trusted",
            file=sys.stderr,
        )
        return 2
    try:
        paths = provision(
            options.output_directory,
            options.lane,
            options.target,
            options.reservation,
            options.attestor_identity,
        )
    except (OSError, ProvisionError, receipt.HardwareReceiptError, subprocess.SubprocessError) as error:
        print(f"tutorial hardware attestor provisioning: {error}", file=sys.stderr)
        return 1
    print(f"private key: {paths['private_key']}")
    print(f"public key: {paths['public_key']}")
    print(f"proposed policy (not installed or trusted): {paths['policy']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
