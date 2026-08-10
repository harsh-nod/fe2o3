#!/usr/bin/env python3
"""Adversarial descriptor and retained-key tests for signed parity evidence."""

from __future__ import annotations

import hashlib
import importlib.util
from pathlib import Path
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "parity_signed_evidence", ROOT / "scripts/parity-signed-evidence.py"
)
assert SPEC is not None and SPEC.loader is not None
EVIDENCE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = EVIDENCE
SPEC.loader.exec_module(EVIDENCE)
FIXTURES = ROOT / "scripts/tests/fixtures"


def write_policy(root: Path, public_key: bytes) -> Path:
    key = root / "keys/attestor.pem"
    key.parent.mkdir(parents=True)
    key.write_bytes(public_key)
    policy = root / "trust.tsv"
    policy.write_text(
        "parity_trust_policy_schema_version\t2\n"
        "trust_domain\ttest\n"
        "metadata_path_count\t0\n"
        "key_count\t1\n"
        f"key\t0000\tattestor\ttest-attestor\tkeys/attestor.pem\t"
        f"{hashlib.sha256(public_key).hexdigest()}\ted25519\n",
        encoding="ascii",
    )
    return policy


def test_verification_retains_authenticated_key_bytes() -> None:
    attestor_public = (FIXTURES / "evidence-test-attestor-public.pem").read_bytes()
    reviewer_public = (FIXTURES / "evidence-test-reviewer-public.pem").read_bytes()
    with tempfile.TemporaryDirectory(prefix="fe2o3-retained-key-") as raw_temp:
        temp = Path(raw_temp)
        trusted = temp / "trusted"
        policy = write_policy(trusted, attestor_public)
        trust = EVIDENCE.parse_trust_policy(trusted, policy)

        replacement = temp / "replacement.pem"
        replacement.write_bytes(reviewer_public)
        replacement.replace(trusted / "keys/attestor.pem")

        unsigned = temp / "unsigned.tsv"
        unsigned.write_text("fixture_schema_version\t1\n", encoding="ascii")
        forged = temp / "forged.tsv"
        EVIDENCE.sign_payload(
            unsigned,
            forged,
            FIXTURES / "evidence-test-reviewer-private.pem",
            "test-attestor",
            domain="test",
            role="attestor",
            repo=None,
            test_mode=True,
        )
        try:
            EVIDENCE.verify_signed(temp, forged.name, trust, "attestor")
        except EVIDENCE.EvidenceError as error:
            assert "signature verification failed" in str(error)
        else:
            raise AssertionError("replacement public key authorized a forged signature")

        authentic = temp / "authentic.tsv"
        EVIDENCE.sign_payload(
            unsigned,
            authentic,
            FIXTURES / "evidence-test-attestor-private.pem",
            "test-attestor",
            domain="test",
            role="attestor",
            repo=None,
            test_mode=True,
        )
        EVIDENCE.verify_signed(temp, authentic.name, trust, "attestor")


def test_archive_snapshot_never_reopens_replaced_root_path() -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-archive-root-race-") as raw_temp:
        temp = Path(raw_temp)
        archive = temp / "archive"
        archive.mkdir()
        authentic = b"authenticated archive bytes\n"
        (archive / "record.tsv").write_bytes(authentic)
        with EVIDENCE.ArchiveSnapshot(
            archive, require_immutable=False
        ) as snapshot:
            detached = temp / "authenticated-archive"
            archive.rename(detached)
            archive.mkdir()
            (archive / "record.tsv").write_bytes(b"replacement bytes\n")

            assert snapshot.read("record.tsv") == authentic
            destination = temp / "copied.tsv"
            snapshot.copy_to("record.tsv", destination)
            assert destination.read_bytes() == authentic


def test_archive_snapshot_detects_replaced_entry() -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-archive-entry-race-") as raw_temp:
        archive = Path(raw_temp) / "archive"
        archive.mkdir()
        record = archive / "record.tsv"
        record.write_bytes(b"authenticated archive bytes\n")
        with EVIDENCE.ArchiveSnapshot(
            archive, require_immutable=False
        ) as snapshot:
            replacement = archive / "replacement.tsv"
            replacement.write_bytes(b"replacement bytes\n")
            replacement.replace(record)
            try:
                snapshot.read("record.tsv")
            except EVIDENCE.EvidenceError as error:
                assert "changed after authentication" in str(error)
            else:
                raise AssertionError("replaced archive entry retained authenticated identity")


def test_archive_snapshot_rejects_symlink_traversal() -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-archive-symlink-") as raw_temp:
        temp = Path(raw_temp)
        archive = temp / "archive"
        archive.mkdir()
        (temp / "outside.tsv").write_bytes(b"outside\n")
        (archive / "record.tsv").symlink_to(temp / "outside.tsv")
        try:
            EVIDENCE.ArchiveSnapshot(archive, require_immutable=False)
        except EVIDENCE.EvidenceError as error:
            assert "archive contains a symlink" in str(error)
        else:
            raise AssertionError("archive snapshot followed a symlink")


if __name__ == "__main__":
    test_verification_retains_authenticated_key_bytes()
    test_archive_snapshot_never_reopens_replaced_root_path()
    test_archive_snapshot_detects_replaced_entry()
    test_archive_snapshot_rejects_symlink_traversal()
    print("signed parity FD and retained-key adversarial tests passed")
