#!/usr/bin/env python3
"""Adversarial descriptor and retained-key tests for signed parity evidence."""

from __future__ import annotations

import hashlib
import importlib.util
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import threading
import time
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
LEASE_DIGEST = "a" * 64


def bootstrap_args(output: Path) -> SimpleNamespace:
    publisher_private = output.parent / (
        f"publisher-{os.getpid()}-{threading.get_ident()}.private.pem"
    )
    publisher_public = output.parent / (
        f"publisher-{os.getpid()}-{threading.get_ident()}.public.pem"
    )
    if not publisher_public.exists():
        subprocess.run(
            ["openssl", "genpkey", "-algorithm", "Ed25519", "-out", publisher_private],
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        subprocess.run(
            [
                "openssl",
                "pkey",
                "-in",
                publisher_private,
                "-pubout",
                "-out",
                publisher_public,
            ],
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
    return SimpleNamespace(
        output_root=output,
        attestor_public_key=FIXTURES / "evidence-test-attestor-public.pem",
        attestor_key_id="production-attestor",
        reviewer_public_key=FIXTURES / "evidence-test-reviewer-public.pem",
        reviewer_key_id="production-reviewer",
        publisher_public_key=publisher_public,
        publisher_key_id="production-publisher",
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

    with tempfile.TemporaryDirectory(prefix="fe2o3-source-parent-symlink-") as raw_temp:
        temp = Path(raw_temp)
        real_parent = temp / "real-parent"
        archive = real_parent / "archive"
        archive.mkdir(parents=True)
        (archive / "record.tsv").write_bytes(b"record\n")
        linked_parent = temp / "linked-parent"
        linked_parent.symlink_to(real_parent, target_is_directory=True)
        expect_evidence_error(
            lambda: EVIDENCE.ArchiveSnapshot(
                linked_parent / "archive", require_immutable=False
            ),
            "evidence archive root must be a real directory",
        )


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


def make_archive_writable(root: Path) -> None:
    if not root.exists():
        return
    for path in sorted(root.rglob("*"), reverse=True):
        if path.is_dir():
            path.chmod(0o700)
        else:
            path.chmod(0o600)
    root.chmod(0o700)


def test_destination_parent_symlink_and_replacement_races() -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-destination-symlink-") as raw_temp:
        temp = Path(raw_temp)
        real_parent = temp / "real-parent"
        real_parent.mkdir()
        linked_parent = temp / "linked-parent"
        linked_parent.symlink_to(real_parent, target_is_directory=True)
        expect_evidence_error(
            lambda: EVIDENCE.ArchiveDestination(
                linked_parent / "archive", LEASE_DIGEST
            ),
            "destination parent contains an unsafe path component",
        )

    with tempfile.TemporaryDirectory(prefix="fe2o3-destination-race-") as raw_temp:
        temp = Path(raw_temp)
        parent = temp / "parent"
        parent.mkdir()
        with EVIDENCE.ArchiveDestination(
            parent / "archive", LEASE_DIGEST
        ) as destination:
            destination.write_index(b"authenticated index bytes\n")
            detached = temp / "detached-parent"
            parent.rename(detached)
            parent.mkdir()
            (parent / "attacker-marker").write_text("replacement\n", encoding="ascii")
            expect_evidence_error(
                destination.publish,
                "requested destination parent changed before publication",
            )
        assert not (detached / "archive").exists()
        assert not (parent / "archive").exists()
        assert not list(detached.glob(".fe2o3-archive-*"))


def test_destination_durability_order_and_fault_boundaries() -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-durability-order-") as raw_temp:
        temp = Path(raw_temp)
        source_root = temp / "source"
        source_root.mkdir()
        (source_root / "payload.bin").write_bytes(b"payload\n")
        labels: list[str] = []
        real_fsync = EVIDENCE.fsync_checked

        def recording_fsync(descriptor: int, label: str) -> None:
            labels.append(label)
            real_fsync(descriptor, label)

        EVIDENCE.fsync_checked = recording_fsync
        try:
            with EVIDENCE.ArchiveSnapshot(
                source_root, require_immutable=False
            ) as source:
                with EVIDENCE.ArchiveDestination(
                    temp / "archive", LEASE_DIGEST
                ) as destination:
                    destination.copy(source, "payload.bin")
                    destination.directory_fd("nested/leaf")
                    destination.write_index(b"index\n")
                    destination.publish()
        finally:
            EVIDENCE.fsync_checked = real_fsync
        required = [
            "copied archive file payload.bin",
            "generated archive index",
            "destination archive directory nested/leaf",
            "destination archive directory nested",
            "archive staging root",
            "archive staging parent",
            "published archive destination root",
            "published archive destination parent",
        ]
        assert [labels.index(label) for label in required] == sorted(
            labels.index(label) for label in required
        )
        make_archive_writable(temp / "archive")

    for fault, published in (
        ("archive staging root", False),
        ("published archive destination root", True),
        ("published archive destination parent", True),
    ):
        with tempfile.TemporaryDirectory(prefix="fe2o3-durability-fault-") as raw_temp:
            temp = Path(raw_temp)
            real_fsync = EVIDENCE.fsync_checked

            def failing_fsync(descriptor: int, label: str) -> None:
                if label == fault:
                    raise EVIDENCE.EvidenceError(f"injected fsync failure: {label}")
                real_fsync(descriptor, label)

            EVIDENCE.fsync_checked = failing_fsync
            try:
                expect_evidence_error(
                    lambda: publish_minimal_archive(temp / "archive"),
                    "injected fsync failure",
                )
            finally:
                EVIDENCE.fsync_checked = real_fsync
            assert (temp / "archive").exists() is published
            assert not list(temp.glob(".fe2o3-archive-*"))
            make_archive_writable(temp / "archive")


def test_retained_destination_fds_detect_same_uid_mutation() -> None:
    for relative in ("payload.bin", "archive-index-v1.tsv"):
        with tempfile.TemporaryDirectory(prefix="fe2o3-retained-destination-") as raw_temp:
            temp = Path(raw_temp)
            source_root = temp / "source"
            source_root.mkdir()
            (source_root / "payload.bin").write_bytes(b"payload\n")
            with EVIDENCE.ArchiveSnapshot(
                source_root, require_immutable=False
            ) as source:
                with EVIDENCE.ArchiveDestination(
                    temp / "archive", LEASE_DIGEST
                ) as destination:
                    destination.copy(source, "payload.bin")
                    destination.write_index(b"index\n")
                    with destination.snapshot() as snapshot:
                        snapshot.validate("payload.bin")
                        snapshot.validate("archive-index-v1.tsv")
                    attacked = destination.staging_label / relative
                    attacked.chmod(0o600)
                    attacked.write_bytes(b"same-uid replacement\n")
                    expect_evidence_error(
                        destination.publish,
                        "changed immediately before publication",
                    )
            assert not (temp / "archive").exists()
            assert not list(temp.glob(".fe2o3-archive-*"))

    with tempfile.TemporaryDirectory(prefix="fe2o3-post-rename-mutation-") as raw_temp:
        temp = Path(raw_temp)
        source_root = temp / "source"
        source_root.mkdir()
        (source_root / "payload.bin").write_bytes(b"payload\n")
        real_rename = EVIDENCE.rename_noreplace_at

        def rename_then_mutate(*args: object) -> None:
            real_rename(*args)
            attacked = temp / "archive/payload.bin"
            attacked.chmod(0o600)
            attacked.write_bytes(b"post-rename mutation\n")

        EVIDENCE.rename_noreplace_at = rename_then_mutate
        try:
            with EVIDENCE.ArchiveSnapshot(
                source_root, require_immutable=False
            ) as source:
                with EVIDENCE.ArchiveDestination(
                    temp / "archive", LEASE_DIGEST
                ) as destination:
                    destination.copy(source, "payload.bin")
                    destination.write_index(b"index\n")
                    expect_evidence_error(
                        destination.publish,
                        "changed immediately after publication",
                    )
        finally:
            EVIDENCE.rename_noreplace_at = real_rename
        assert (temp / "archive").is_dir()
        make_archive_writable(temp / "archive")


def swap_paths(left: Path, right: Path) -> None:
    temporary = left.with_name("swap-temporary")
    left.rename(temporary)
    right.rename(left)
    temporary.rename(right)


def test_retained_destination_dirents_detect_swaps() -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-payload-index-swap-") as raw_temp:
        temp = Path(raw_temp)
        source_root = temp / "source"
        source_root.mkdir()
        (source_root / "payload.bin").write_bytes(b"payload\n")
        with EVIDENCE.ArchiveSnapshot(source_root, require_immutable=False) as source:
            with EVIDENCE.ArchiveDestination(
                temp / "archive", LEASE_DIGEST
            ) as destination:
                destination.copy(source, "payload.bin")
                destination.write_index(b"index\n")
                swap_paths(
                    destination.staging_label / "payload.bin",
                    destination.staging_label / "archive-index-v1.tsv",
                )
                expect_evidence_error(
                    destination.publish,
                    "changed",
                )

    with tempfile.TemporaryDirectory(prefix="fe2o3-same-size-swap-") as raw_temp:
        temp = Path(raw_temp)
        source_root = temp / "source"
        source_root.mkdir()
        (source_root / "left.bin").write_bytes(b"L" * 16)
        (source_root / "right.bin").write_bytes(b"R" * 16)
        assert (source_root / "left.bin").stat().st_size == (
            source_root / "right.bin"
        ).stat().st_size
        with EVIDENCE.ArchiveSnapshot(source_root, require_immutable=False) as source:
            with EVIDENCE.ArchiveDestination(
                temp / "archive", LEASE_DIGEST
            ) as destination:
                destination.copy(source, "left.bin")
                destination.copy(source, "right.bin")
                destination.write_index(b"index\n")
                swap_paths(
                    destination.staging_label / "left.bin",
                    destination.staging_label / "right.bin",
                )
                expect_evidence_error(
                    destination.publish,
                    "changed",
                )

    with tempfile.TemporaryDirectory(prefix="fe2o3-rename-out-in-") as raw_temp:
        temp = Path(raw_temp)
        source_root = temp / "source"
        source_root.mkdir()
        (source_root / "payload.bin").write_bytes(b"payload\n")
        with EVIDENCE.ArchiveSnapshot(source_root, require_immutable=False) as source:
            with EVIDENCE.ArchiveDestination(
                temp / "archive", LEASE_DIGEST
            ) as destination:
                destination.copy(source, "payload.bin")
                destination.write_index(b"index\n")
                payload = destination.staging_label / "payload.bin"
                moved = destination.staging_label / "moved.bin"
                payload.rename(moved)
                time.sleep(0.01)
                moved.rename(payload)
                expect_evidence_error(
                    destination.publish,
                    "namespace changed before publication",
                )


def test_deterministic_staging_lease_recovers_after_hard_exit() -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-staging-recovery-") as raw_temp:
        temp = Path(raw_temp)
        output = temp / "archive"
        child = os.fork()
        if child == 0:
            destination = EVIDENCE.ArchiveDestination(output, LEASE_DIGEST)
            destination.write_index(b"crash-left index\n")
            os._exit(0)
        _, status = os.waitpid(child, 0)
        assert os.waitstatus_to_exitcode(status) == 0
        stages = list(temp.glob(".fe2o3-archive-*.stage"))
        leases = list(temp.glob(".fe2o3-archive-*.lease"))
        assert len(stages) == 1 and len(leases) == 1

        with EVIDENCE.ArchiveDestination(output, LEASE_DIGEST) as destination:
            assert destination.staging_label == stages[0]
            destination.write_index(b"recovered index\n")
            destination.publish()
        assert not list(temp.glob(".fe2o3-archive-*"))
        assert (output / "archive-index-v1.tsv").read_bytes() == b"recovered index\n"
        make_archive_writable(output)


def test_second_live_publisher_fails_busy() -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-live-publisher-") as raw_temp:
        temp = Path(raw_temp)
        output = temp / "archive"
        with EVIDENCE.ArchiveDestination(output, LEASE_DIGEST) as first:
            first.write_index(b"first publisher\n")
            expect_evidence_error(
                lambda: EVIDENCE.ArchiveDestination(output, LEASE_DIGEST),
                "staging lease is busy",
            )
            assert first.staging_label.is_dir()
        assert not list(temp.glob(".fe2o3-archive-*"))


def test_same_uid_publisher_is_inert_for_production() -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-production-publisher-") as raw_temp:
        temp = Path(raw_temp)
        real_parse = EVIDENCE.parse_trust_policy
        real_validate = EVIDENCE.validate_production_trust
        production = EVIDENCE.TrustPolicy("production", [], {})
        EVIDENCE.parse_trust_policy = lambda *_: production
        EVIDENCE.validate_production_trust = lambda *_: production
        args = SimpleNamespace(
            repo=temp,
            allow_test_fixtures=False,
            trusted_root=temp,
            trust_policy=temp / "trust.tsv",
        )
        try:
            expect_evidence_error(
                lambda: EVIDENCE.ingest_archive_snapshot(args, None),
                "requires an externally protected publisher contract",
            )
        finally:
            EVIDENCE.parse_trust_policy = real_parse
            EVIDENCE.validate_production_trust = real_validate

        EVIDENCE.parse_trust_policy = lambda *_: production
        args.allow_test_fixtures = True
        try:
            expect_evidence_error(
                lambda: EVIDENCE.ingest_archive_snapshot(args, None),
                "test fixture ingestion requires test-domain trust",
            )
        finally:
            EVIDENCE.parse_trust_policy = real_parse


def test_production_archive_index_requires_publisher_receipt() -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-missing-receipt-") as raw_temp:
        temp = Path(raw_temp)
        archive = temp / "archive"
        archive.mkdir()
        (archive / "promotion.tsv").write_bytes(b"manifest\n")
        (archive / EVIDENCE.ARCHIVE_INDEX_RELATIVE).write_bytes(b"index\n")
        manifest = EVIDENCE.PromotionManifest(
            "a" * 40,
            "b" * 40,
            "c" * 40,
            "gfx942",
            "mi300x-gfx942-test",
            "d" * 64,
            [],
            [],
        )
        trust = EVIDENCE.TrustPolicy("production", [], {})
        real_closure = EVIDENCE.promotion_archive_closure
        EVIDENCE.promotion_archive_closure = lambda *_: {"promotion.tsv"}
        try:
            with EVIDENCE.ArchiveSnapshot(archive, require_immutable=False) as snapshot:
                expect_evidence_error(
                    lambda: EVIDENCE.verify_archive_index(
                        temp,
                        snapshot,
                        "promotion.tsv",
                        manifest,
                        trust,
                    ),
                    "publisher-receipt-v1.tsv",
                )
        finally:
            EVIDENCE.promotion_archive_closure = real_closure


def test_publisher_receipt_is_authenticated_and_domain_separated() -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-publisher-receipt-") as raw_temp:
        temp = Path(raw_temp)
        archive = temp / "archive"
        archive.mkdir()
        manifest_path = archive / "promotion.tsv"
        manifest_path.write_bytes(b"fixture manifest\n")
        manifest = EVIDENCE.PromotionManifest(
            "a" * 40,
            "b" * 40,
            "c" * 40,
            "gfx942",
            "mi300x-gfx942-test",
            "d" * 64,
            [],
            [],
        )
        closure = {"promotion.tsv"}
        with EVIDENCE.ArchiveSnapshot(archive, require_immutable=False) as snapshot:
            archive_identity = EVIDENCE.publisher_archive_identity(
                snapshot, "promotion.tsv", manifest, closure
            )
        parent_info = os.stat(temp)
        root_info = os.stat(archive)
        now = int(time.time())
        unsigned = temp / "receipt.unsigned.tsv"
        unsigned.write_text(
            "publisher_contract_receipt_schema_version\t1\n"
            "publisher_identity\ttest-publisher\n"
            "publisher_key_role\tpublisher\n"
            f"destination_contract\t{EVIDENCE.PUBLISHER_CONTRACT}\n"
            "destination_name\tarchive\n"
            f"destination_parent_device\t{parent_info.st_dev}\n"
            f"destination_parent_inode\t{parent_info.st_ino}\n"
            f"destination_root_device\t{root_info.st_dev}\n"
            f"destination_root_inode\t{root_info.st_ino}\n"
            f"archive_sha256\t{archive_identity}\n"
            "manifest_path\tpromotion.tsv\n"
            f"manifest_sha256\t{hashlib.sha256(manifest_path.read_bytes()).hexdigest()}\n"
            f"source_commit\t{manifest.source}\n"
            f"source_tree\t{manifest.tree}\n"
            f"target\t{manifest.target}\n"
            f"hardware_lane\t{manifest.lane}\n"
            f"freshness_nonce\t{'e' * 64}\n"
            f"issued_at_unix\t{now}\n"
            f"expires_at_unix\t{now + 60}\n",
            encoding="ascii",
        )
        EVIDENCE.sign_payload(
            unsigned,
            archive / EVIDENCE.PUBLISHER_RECEIPT_RELATIVE,
            FIXTURES / "evidence-test-reviewer-private.pem",
            "test-publisher",
            domain="test",
            role="publisher",
            repo=None,
            test_mode=True,
        )
        public_key = (FIXTURES / "evidence-test-reviewer-public.pem").read_bytes()
        key = EVIDENCE.TrustedKey(
            "publisher",
            "test-publisher",
            "keys/test-publisher.pem",
            EVIDENCE.ed25519_fingerprint_bytes(public_key, "test-publisher"),
            public_key,
        )
        trust = EVIDENCE.TrustPolicy(
            "test", [], {("publisher", "test-publisher"): key}
        )
        real_closure = EVIDENCE.promotion_archive_closure
        EVIDENCE.promotion_archive_closure = lambda *_: closure
        try:
            with EVIDENCE.ArchiveSnapshot(archive, require_immutable=False) as snapshot:
                receipt = EVIDENCE.parse_publisher_receipt(
                    temp,
                    snapshot,
                    "promotion.tsv",
                    manifest,
                    trust,
                    require_fresh=True,
                    expected_domain="test",
                )
                assert receipt.domain == "test"
                expect_evidence_error(
                    lambda: EVIDENCE.parse_publisher_receipt(
                        temp,
                        snapshot,
                        "promotion.tsv",
                        manifest,
                        trust,
                        require_fresh=True,
                    ),
                    "requires production-domain trust",
                )
                real_time = EVIDENCE.time.time
                EVIDENCE.time.time = lambda: now + 61
                try:
                    expect_evidence_error(
                        lambda: EVIDENCE.parse_publisher_receipt(
                            temp,
                            snapshot,
                            "promotion.tsv",
                            manifest,
                            trust,
                            require_fresh=True,
                            expected_domain="test",
                        ),
                        "stale or not yet valid",
                    )
                finally:
                    EVIDENCE.time.time = real_time

            copied_parent = temp / "copied-parent"
            copied_parent.mkdir()
            copied_archive = copied_parent / "archive"
            shutil.copytree(archive, copied_archive)
            with EVIDENCE.ArchiveSnapshot(
                copied_archive, require_immutable=False
            ) as copied_snapshot:
                expect_evidence_error(
                    lambda: EVIDENCE.parse_publisher_receipt(
                        temp,
                        copied_snapshot,
                        "promotion.tsv",
                        manifest,
                        trust,
                        require_fresh=False,
                        expected_domain="test",
                    ),
                    "archive root identity mismatch",
                )

            relocated_parent = temp / "relocated-parent"
            relocated_parent.mkdir()
            relocated_archive = relocated_parent / "archive"
            archive.rename(relocated_archive)
            with EVIDENCE.ArchiveSnapshot(
                relocated_archive, require_immutable=False
            ) as relocated_snapshot:
                expect_evidence_error(
                    lambda: EVIDENCE.parse_publisher_receipt(
                        temp,
                        relocated_snapshot,
                        "promotion.tsv",
                        manifest,
                        trust,
                        require_fresh=False,
                        expected_domain="test",
                    ),
                    "parent identity mismatch",
                )
        finally:
            EVIDENCE.promotion_archive_closure = real_closure


def publish_minimal_archive(output: Path) -> None:
    with EVIDENCE.ArchiveDestination(output, LEASE_DIGEST) as destination:
        destination.write_index(b"index\n")
        destination.publish()


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
    test_destination_parent_symlink_and_replacement_races()
    test_destination_durability_order_and_fault_boundaries()
    test_retained_destination_fds_detect_same_uid_mutation()
    test_retained_destination_dirents_detect_swaps()
    test_deterministic_staging_lease_recovers_after_hard_exit()
    test_second_live_publisher_fails_busy()
    test_same_uid_publisher_is_inert_for_production()
    test_production_archive_index_requires_publisher_receipt()
    test_publisher_receipt_is_authenticated_and_domain_separated()
    test_bootstrap_publication_is_durable_and_no_replace()
    test_bootstrap_destination_race_has_one_winner()
    test_bootstrap_interruption_cleans_unpublished_staging()
    test_bootstrap_rejects_symlink_parent()
    print("signed parity FD and retained-key adversarial tests passed")
