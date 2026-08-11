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
            if args[2] != "archive":
                return
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


def wait_for_outcomes(outcomes: list[str], expected: int) -> None:
    deadline = time.monotonic() + 10
    while len(outcomes) < expected and time.monotonic() < deadline:
        time.sleep(0.01)
    assert len(outcomes) == expected, outcomes


def test_lease_initialization_recovers_every_crash_phase() -> None:
    phases = (
        "provisional-created",
        "provisional-written",
        "lease-renamed",
        "lease-published",
        "staging-mkdir",
        "staging-created",
    )
    for phase in phases:
        with tempfile.TemporaryDirectory(
            prefix=f"fe2o3-lease-{phase}-"
        ) as raw_temp:
            temp = Path(raw_temp)
            output = temp / "archive"
            child = os.fork()
            if child == 0:
                EVIDENCE.archive_lease_phase = (
                    lambda observed, phase=phase: os._exit(0)
                    if observed == phase
                    else None
                )
                EVIDENCE.ArchiveDestination(output, LEASE_DIGEST)
                os._exit(9)
            _, status = os.waitpid(child, 0)
            assert os.waitstatus_to_exitcode(status) == 0
            assert list(temp.glob(".fe2o3-archive-*"))

            with EVIDENCE.ArchiveDestination(
                output, LEASE_DIGEST
            ) as destination:
                destination.write_index(f"recovered {phase}\n".encode("ascii"))
                destination.publish()
            assert not list(temp.glob(".fe2o3-archive-*"))
            make_archive_writable(output)


def test_two_recoverers_cannot_delete_new_live_winner() -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-two-recoverers-") as raw_temp:
        temp = Path(raw_temp)
        output = temp / "archive"
        child = os.fork()
        if child == 0:
            EVIDENCE.ArchiveDestination(output, LEASE_DIGEST)
            os._exit(0)
        _, status = os.waitpid(child, 0)
        assert os.waitstatus_to_exitcode(status) == 0

        start = threading.Barrier(3)
        release = threading.Event()
        outcomes: list[str] = []
        outcomes_lock = threading.Lock()

        def recover() -> None:
            start.wait()
            try:
                with EVIDENCE.ArchiveDestination(
                    output, LEASE_DIGEST
                ) as destination:
                    with outcomes_lock:
                        outcomes.append("winner")
                    assert destination.staging_label.is_dir()
                    assert release.wait(10)
            except EVIDENCE.EvidenceError as error:
                with outcomes_lock:
                    outcomes.append(str(error))

        threads = [threading.Thread(target=recover) for _ in range(2)]
        for thread in threads:
            thread.start()
        start.wait()
        wait_for_outcomes(outcomes, 2)
        assert outcomes.count("winner") == 1, outcomes
        assert sum("staging lease is busy" in value for value in outcomes) == 1
        release.set()
        for thread in threads:
            thread.join(10)
            assert not thread.is_alive()
        assert not list(temp.glob(".fe2o3-archive-*"))


def test_stale_lease_recovery_rejects_dirent_replacement() -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-lease-dirent-race-") as raw_temp:
        temp = Path(raw_temp)
        output = temp / "archive"
        child = os.fork()
        if child == 0:
            EVIDENCE.ArchiveDestination(output, LEASE_DIGEST)
            os._exit(0)
        _, status = os.waitpid(child, 0)
        assert os.waitstatus_to_exitcode(status) == 0
        lease = next(temp.glob(".fe2o3-archive-*.lease"))
        detached = temp / "detached-stale.lease"
        real_dead = EVIDENCE.publisher_process_is_dead
        replaced = False

        def replace_after_authentication(_pid: int, _start: str) -> bool:
            nonlocal replaced
            if not replaced:
                lease.rename(detached)
                lease.write_bytes(detached.read_bytes())
                replaced = True
            return True

        EVIDENCE.publisher_process_is_dead = replace_after_authentication
        try:
            expect_evidence_error(
                lambda: EVIDENCE.ArchiveDestination(output, LEASE_DIGEST),
                "staging lease changed during recovery",
            )
        finally:
            EVIDENCE.publisher_process_is_dead = real_dead
        assert replaced


def test_global_staging_entry_boundaries_and_concurrency() -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-entry-boundary-") as raw_temp:
        temp = Path(raw_temp)
        for index in range(126):
            (temp / f".fe2o3-archive-{index:032x}.lease").touch()
        with EVIDENCE.ArchiveDestination(
            temp / "allowed", LEASE_DIGEST
        ) as destination:
            assert len(EVIDENCE.archive_staging_entries(destination.parent_fd)) == 128
        (temp / f".fe2o3-archive-{126:032x}.stage").mkdir()
        expect_evidence_error(
            lambda: EVIDENCE.ArchiveDestination(temp / "at-127", "b" * 64),
            "staging entries exceed the parent bound",
        )
        (temp / f".fe2o3-archive-{127:032x}.lease").touch()
        expect_evidence_error(
            lambda: EVIDENCE.ArchiveDestination(temp / "at-128", "c" * 64),
            "staging entries exceed the parent bound",
        )

    original_limit = EVIDENCE.MAX_ARCHIVE_STAGING_ENTRIES
    EVIDENCE.MAX_ARCHIVE_STAGING_ENTRIES = 4
    try:
        with tempfile.TemporaryDirectory(prefix="fe2o3-entry-concurrency-") as raw_temp:
            temp = Path(raw_temp)
            start = threading.Barrier(4)
            release = threading.Event()
            outcomes: list[str] = []
            outcomes_lock = threading.Lock()

            def publish(index: int) -> None:
                start.wait()
                digest = hashlib.sha256(str(index).encode("ascii")).hexdigest()
                try:
                    with EVIDENCE.ArchiveDestination(
                        temp / f"archive-{index}", digest
                    ):
                        with outcomes_lock:
                            outcomes.append("active")
                        assert release.wait(10)
                except EVIDENCE.EvidenceError as error:
                    with outcomes_lock:
                        outcomes.append(str(error))

            threads = [
                threading.Thread(target=publish, args=(index,)) for index in range(3)
            ]
            for thread in threads:
                thread.start()
            start.wait()
            wait_for_outcomes(outcomes, 3)
            assert outcomes.count("active") == 2, outcomes
            assert sum("staging entries exceed" in value for value in outcomes) == 1
            release.set()
            for thread in threads:
                thread.join(10)
                assert not thread.is_alive()
            assert not list(temp.glob(".fe2o3-archive-*"))
    finally:
        EVIDENCE.MAX_ARCHIVE_STAGING_ENTRIES = original_limit


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


def publisher_test_trust() -> EVIDENCE.TrustPolicy:
    public_key = (FIXTURES / "evidence-test-reviewer-public.pem").read_bytes()
    key = EVIDENCE.TrustedKey(
        "publisher",
        "test-publisher",
        "keys/test-publisher.pem",
        EVIDENCE.ed25519_fingerprint_bytes(public_key, "test-publisher"),
        public_key,
    )
    return EVIDENCE.TrustPolicy(
        "test", [], {("publisher", "test-publisher"): key}
    )


def write_test_publisher_receipt(
    temp: Path,
    archive: Path,
    receipt_root: Path,
    manifest: EVIDENCE.PromotionManifest,
    *,
    now: int,
) -> dict[str, str]:
    receipt_root.mkdir()
    with EVIDENCE.ArchiveSnapshot(archive, require_immutable=False) as snapshot:
        archive_identity = EVIDENCE.publisher_archive_identity(snapshot)
    context = {
        "logical_destination": "docs/parity-evidence/archive",
        "baseline_status_sha256": "1" * 64,
        "candidate_status_sha256": "2" * 64,
        "default_tip": "3" * 40,
        "candidate_head": "4" * 40,
        "challenge": "5" * 64,
    }
    unsigned = temp / "receipt.unsigned.tsv"
    unsigned.write_text(
        "publisher_contract_receipt_schema_version\t2\n"
        "publisher_identity\ttest-publisher\n"
        "publisher_key_role\tpublisher\n"
        f"destination_contract\t{EVIDENCE.PUBLISHER_CONTRACT}\n"
        f"logical_destination\t{context['logical_destination']}\n"
        f"archive_sha256\t{archive_identity}\n"
        "manifest_path\tpromotion.tsv\n"
        f"manifest_sha256\t{hashlib.sha256((archive / 'promotion.tsv').read_bytes()).hexdigest()}\n"
        f"source_commit\t{manifest.source}\n"
        f"source_tree\t{manifest.tree}\n"
        f"target\t{manifest.target}\n"
        f"hardware_lane\t{manifest.lane}\n"
        f"baseline_status_sha256\t{context['baseline_status_sha256']}\n"
        f"candidate_status_sha256\t{context['candidate_status_sha256']}\n"
        f"default_tip\t{context['default_tip']}\n"
        f"candidate_head\t{context['candidate_head']}\n"
        f"freshness_challenge\t{context['challenge']}\n"
        f"issued_at_unix\t{now}\n"
        f"expires_at_unix\t{now + 60}\n",
        encoding="ascii",
    )
    EVIDENCE.sign_payload(
        unsigned,
        receipt_root / EVIDENCE.PUBLISHER_RECEIPT_RELATIVE,
        FIXTURES / "evidence-test-reviewer-private.pem",
        "test-publisher",
        domain="test",
        role="publisher",
        repo=None,
        test_mode=True,
    )
    return context


def parse_test_publisher_receipt(
    repo: Path,
    archive: Path,
    receipt_root: Path,
    manifest: EVIDENCE.PromotionManifest,
    context: dict[str, str],
    *,
    require_fresh: bool,
    expected_domain: str = "test",
) -> EVIDENCE.PublisherReceipt:
    with EVIDENCE.ArchiveSnapshot(archive, require_immutable=False) as snapshot:
        with EVIDENCE.ArchiveSnapshot(
            receipt_root, require_immutable=False
        ) as receipt_snapshot:
            return EVIDENCE.parse_publisher_receipt(
                repo,
                snapshot,
                receipt_snapshot,
                "promotion.tsv",
                manifest,
                publisher_test_trust(),
                expected_logical_destination=context["logical_destination"],
                expected_baseline_status_sha256=context[
                    "baseline_status_sha256"
                ],
                expected_candidate_status_sha256=context[
                    "candidate_status_sha256"
                ],
                expected_default_tip=context["default_tip"],
                expected_candidate_head=context["candidate_head"],
                expected_challenge=context["challenge"],
                require_fresh=require_fresh,
                expected_domain=expected_domain,
            )


def test_publisher_receipt_is_portable_authenticated_and_replay_bound() -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-publisher-receipt-") as raw_temp:
        temp = Path(raw_temp)
        archive = temp / "archive"
        archive.mkdir()
        (archive / "logs").mkdir()
        manifest_path = archive / "promotion.tsv"
        manifest_path.write_bytes(b"fixture manifest\n")
        (archive / "logs/evidence.log").write_bytes(b"portable evidence\n")
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
        now = int(time.time())
        receipt_root = temp / "receipt"
        context = write_test_publisher_receipt(
            temp, archive, receipt_root, manifest, now=now
        )
        receipt = parse_test_publisher_receipt(
            temp, archive, receipt_root, manifest, context, require_fresh=True
        )
        assert receipt.domain == "test"
        expect_evidence_error(
            lambda: parse_test_publisher_receipt(
                temp,
                archive,
                receipt_root,
                manifest,
                context,
                require_fresh=False,
                expected_domain="production",
            ),
            "requires production-domain trust",
        )

        fresh_checkout = temp / "fresh-checkout"
        fresh_checkout.mkdir()
        copied_archive = fresh_checkout / "archive"
        copied_receipt = fresh_checkout / "receipt"
        shutil.copytree(archive, copied_archive)
        shutil.copytree(receipt_root, copied_receipt)
        parse_test_publisher_receipt(
            fresh_checkout,
            copied_archive,
            copied_receipt,
            manifest,
            context,
            require_fresh=True,
        )

        content_substitution = temp / "content-substitution"
        shutil.copytree(archive, content_substitution)
        payload = content_substitution / "logs/evidence.log"
        value = bytearray(payload.read_bytes())
        value[0] ^= 1
        payload.write_bytes(value)
        expect_evidence_error(
            lambda: parse_test_publisher_receipt(
                temp,
                content_substitution,
                receipt_root,
                manifest,
                context,
                require_fresh=False,
            ),
            "archive identity mismatch",
        )

        path_substitution = temp / "path-substitution"
        shutil.copytree(archive, path_substitution)
        (path_substitution / "logs/evidence.log").rename(
            path_substitution / "logs/substituted.log"
        )
        expect_evidence_error(
            lambda: parse_test_publisher_receipt(
                temp,
                path_substitution,
                receipt_root,
                manifest,
                context,
                require_fresh=False,
            ),
            "archive identity mismatch",
        )

        direct_placement = temp / "direct-placement"
        shutil.copytree(archive, direct_placement)
        shutil.copy2(
            receipt_root / EVIDENCE.PUBLISHER_RECEIPT_RELATIVE,
            direct_placement / EVIDENCE.PUBLISHER_RECEIPT_RELATIVE,
        )
        expect_evidence_error(
            lambda: parse_test_publisher_receipt(
                temp,
                direct_placement,
                receipt_root,
                manifest,
                context,
                require_fresh=False,
            ),
            "archive identity mismatch",
        )

        for field, expected in (
            ("logical_destination", "logical destination mismatch"),
            ("challenge", "freshness challenge mismatch"),
            ("candidate_head", "candidate head mismatch"),
            ("default_tip", "default tip mismatch"),
            ("baseline_status_sha256", "baseline transition mismatch"),
            ("candidate_status_sha256", "candidate transition mismatch"),
        ):
            replay_context = dict(context)
            replay_context[field] = (
                "docs/parity-evidence/archive/substituted"
                if field == "logical_destination"
                else "6" * len(replay_context[field])
            )
            expect_evidence_error(
                lambda replay_context=replay_context: parse_test_publisher_receipt(
                    temp,
                    archive,
                    receipt_root,
                    manifest,
                    replay_context,
                    require_fresh=False,
                ),
                expected,
            )

        extra_receipt_root = temp / "receipt-extra"
        shutil.copytree(receipt_root, extra_receipt_root)
        (extra_receipt_root / "candidate-owned.txt").write_text(
            "not protected\n", encoding="ascii"
        )
        expect_evidence_error(
            lambda: parse_test_publisher_receipt(
                temp,
                archive,
                extra_receipt_root,
                manifest,
                context,
                require_fresh=False,
            ),
            "exactly the receipt file",
        )

        real_time = EVIDENCE.time.time
        EVIDENCE.time.time = lambda: now + 61
        try:
            expect_evidence_error(
                lambda: parse_test_publisher_receipt(
                    temp,
                    archive,
                    receipt_root,
                    manifest,
                    context,
                    require_fresh=True,
                ),
                "stale or not yet valid",
            )
        finally:
            EVIDENCE.time.time = real_time


def test_candidate_archive_cannot_embed_publisher_receipt() -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-embedded-receipt-") as raw_temp:
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
        records = {
            "promotion.tsv": (
                manifest_path.stat().st_size,
                hashlib.sha256(manifest_path.read_bytes()).hexdigest(),
            )
        }
        (archive / EVIDENCE.ARCHIVE_INDEX_RELATIVE).write_bytes(
            EVIDENCE.archive_index_bytes(
                "promotion.tsv", records["promotion.tsv"][1], manifest, records
            )
        )
        (archive / EVIDENCE.PUBLISHER_RECEIPT_RELATIVE).write_bytes(
            b"candidate-owned receipt\n"
        )
        real_closure = EVIDENCE.promotion_archive_closure
        EVIDENCE.promotion_archive_closure = lambda *_: {"promotion.tsv"}
        try:
            with EVIDENCE.ArchiveSnapshot(
                archive, require_immutable=False
            ) as snapshot:
                expect_evidence_error(
                    lambda: EVIDENCE.verify_archive_index(
                        temp,
                        snapshot,
                        "promotion.tsv",
                        manifest,
                        EVIDENCE.TrustPolicy("production", [], {}),
                    ),
                    "archive index closure mismatch",
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
    test_lease_initialization_recovers_every_crash_phase()
    test_two_recoverers_cannot_delete_new_live_winner()
    test_stale_lease_recovery_rejects_dirent_replacement()
    test_global_staging_entry_boundaries_and_concurrency()
    test_second_live_publisher_fails_busy()
    test_same_uid_publisher_is_inert_for_production()
    test_publisher_receipt_is_portable_authenticated_and_replay_bound()
    test_candidate_archive_cannot_embed_publisher_receipt()
    test_bootstrap_publication_is_durable_and_no_replace()
    test_bootstrap_destination_race_has_one_winner()
    test_bootstrap_interruption_cleans_unpublished_staging()
    test_bootstrap_rejects_symlink_parent()
    print("signed parity FD and retained-key adversarial tests passed")
