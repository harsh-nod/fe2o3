#!/usr/bin/env python3
"""Adversarial descriptor and retained-key tests for signed parity evidence."""

from __future__ import annotations

import hashlib
import importlib.util
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
from concurrent.futures import ThreadPoolExecutor


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "parity_signed_evidence", ROOT / "scripts/parity-signed-evidence.py"
)
assert SPEC is not None and SPEC.loader is not None
EVIDENCE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = EVIDENCE
SPEC.loader.exec_module(EVIDENCE)
FIXTURES = ROOT / "scripts/tests/fixtures"


def bootstrap_args(output: Path) -> SimpleNamespace:
    return SimpleNamespace(
        output_root=output,
        attestor_public_key=FIXTURES / "evidence-test-attestor-public.pem",
        attestor_key_id="production-attestor",
        reviewer_public_key=FIXTURES / "evidence-test-reviewer-public.pem",
        reviewer_key_id="production-reviewer",
    )


def expect_evidence_error(function: object, expected: str) -> None:
    try:
        function()
    except EVIDENCE.EvidenceError as error:
        assert expected in str(error), str(error)
    else:
        raise AssertionError(f"expected evidence error containing: {expected}")


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


def test_openat2_architecture_preflight_is_explicit() -> None:
    assert EVIDENCE.openat2_syscall_number("x86_64") == 437
    expect_evidence_error(
        lambda: EVIDENCE.openat2_syscall_number("unsupported-test-architecture"),
        "has no openat2 ABI",
    )


def test_archive_file_and_cumulative_byte_limits() -> None:
    original_file_limit = EVIDENCE.MAX_ARCHIVE_FILE_BYTES
    original_total_limit = EVIDENCE.MAX_ARCHIVE_TOTAL_BYTES
    try:
        EVIDENCE.MAX_ARCHIVE_FILE_BYTES = 16
        EVIDENCE.MAX_ARCHIVE_TOTAL_BYTES = 32
        with tempfile.TemporaryDirectory(prefix="fe2o3-file-bound-") as raw_temp:
            archive = Path(raw_temp) / "archive"
            archive.mkdir()
            (archive / "oversized.bin").write_bytes(b"x" * 17)
            expect_evidence_error(
                lambda: EVIDENCE.ArchiveSnapshot(
                    archive, require_immutable=False
                ),
                "archive file exceeds the byte limit",
            )

        EVIDENCE.MAX_ARCHIVE_TOTAL_BYTES = 15
        with tempfile.TemporaryDirectory(prefix="fe2o3-total-bound-") as raw_temp:
            archive = Path(raw_temp) / "archive"
            archive.mkdir()
            (archive / "one.bin").write_bytes(b"x" * 8)
            (archive / "two.bin").write_bytes(b"y" * 8)
            expect_evidence_error(
                lambda: EVIDENCE.ArchiveSnapshot(
                    archive, require_immutable=False
                ),
                "archive exceeds the cumulative byte limit",
            )
    finally:
        EVIDENCE.MAX_ARCHIVE_FILE_BYTES = original_file_limit
        EVIDENCE.MAX_ARCHIVE_TOTAL_BYTES = original_total_limit


def test_bootstrap_publication_is_durable_and_no_replace() -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-bootstrap-publish-") as raw_temp:
        temp = Path(raw_temp)
        fsync_calls: list[int] = []
        real_fsync = EVIDENCE.os.fsync

        def recording_fsync(descriptor: int) -> None:
            fsync_calls.append(descriptor)
            real_fsync(descriptor)

        EVIDENCE.os.fsync = recording_fsync
        output = temp / "trust"
        try:
            EVIDENCE.bootstrap_production_trust(bootstrap_args(output))
        finally:
            EVIDENCE.os.fsync = real_fsync
        assert len(fsync_calls) >= 8
        policy = output / "docs/parity-evidence/trust-policy-v2.tsv"
        EVIDENCE.validate_production_trust(output, policy)

        for kind in ("file", "directory", "symlink"):
            target = temp / f"existing-{kind}"
            if kind == "file":
                target.write_text("occupied\n", encoding="ascii")
            elif kind == "directory":
                target.mkdir()
            else:
                target.symlink_to(output, target_is_directory=True)
            expect_evidence_error(
                lambda target=target: EVIDENCE.bootstrap_production_trust(
                    bootstrap_args(target)
                ),
                "production trust bootstrap output already exists",
            )


def test_bootstrap_destination_race_has_one_winner() -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-bootstrap-race-") as raw_temp:
        output = Path(raw_temp) / "trust"

        def attempt() -> str:
            try:
                EVIDENCE.bootstrap_production_trust(bootstrap_args(output))
                return "published"
            except EVIDENCE.EvidenceError as error:
                return str(error)

        with ThreadPoolExecutor(max_workers=2) as pool:
            outcomes = list(pool.map(lambda _: attempt(), range(2)))
        assert outcomes.count("published") == 1, outcomes
        assert sum("output already exists" in value for value in outcomes) == 1
        assert not list(output.parent.glob(".fe2o3-trust-*"))


def test_bootstrap_interruption_cleans_unpublished_staging() -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-bootstrap-interrupt-") as raw_temp:
        temp = Path(raw_temp)
        output = temp / "trust"
        real_rename = EVIDENCE.rename_noreplace_at

        def interrupt(*_: object) -> None:
            raise EVIDENCE.EvidenceError("injected publication interruption")

        EVIDENCE.rename_noreplace_at = interrupt
        try:
            expect_evidence_error(
                lambda: EVIDENCE.bootstrap_production_trust(bootstrap_args(output)),
                "injected publication interruption",
            )
        finally:
            EVIDENCE.rename_noreplace_at = real_rename
        assert not output.exists() and not output.is_symlink()
        assert not list(temp.glob(".fe2o3-trust-*"))


def test_bootstrap_rejects_symlink_parent() -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-bootstrap-parent-") as raw_temp:
        temp = Path(raw_temp)
        real_parent = temp / "real-parent"
        real_parent.mkdir()
        linked_parent = temp / "linked-parent"
        linked_parent.symlink_to(real_parent, target_is_directory=True)
        expect_evidence_error(
            lambda: EVIDENCE.bootstrap_production_trust(
                bootstrap_args(linked_parent / "trust")
            ),
            "bootstrap parent must be a real directory",
        )
        assert not (real_parent / "trust").exists()


if __name__ == "__main__":
    test_verification_retains_authenticated_key_bytes()
    test_archive_snapshot_never_reopens_replaced_root_path()
    test_archive_snapshot_detects_replaced_entry()
    test_archive_snapshot_rejects_symlink_traversal()
    test_openat2_architecture_preflight_is_explicit()
    test_archive_file_and_cumulative_byte_limits()
    test_bootstrap_publication_is_durable_and_no_replace()
    test_bootstrap_destination_race_has_one_winner()
    test_bootstrap_interruption_cleans_unpublished_staging()
    test_bootstrap_rejects_symlink_parent()
    print("signed parity FD and retained-key adversarial tests passed")
