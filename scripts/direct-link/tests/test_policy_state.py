#!/usr/bin/env python3

from __future__ import annotations

import fcntl
import json
import os
import socket
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

SCRIPT_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPT_DIR))

import policy_state  # noqa: E402
from common import EvidenceError, typed_identity  # noqa: E402


def identity(domain: str, label: str) -> str:
    return typed_identity(domain, label.encode("ascii"))


POLICY_7 = identity(policy_state.POLICY_DOMAIN, "policy-7")
POLICY_8 = identity(policy_state.POLICY_DOMAIN, "policy-8")
CONTEXT_A = identity(policy_state.BUILD_CONTEXT_DOMAIN, "context-a")
CONTEXT_B = identity(policy_state.BUILD_CONTEXT_DOMAIN, "context-b")


class InjectedCrash(RuntimeError):
    pass


class PolicyStateTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        os.chmod(self.root, 0o700)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def store(
        self, fault_injector: policy_state.FaultInjector | None = None
    ) -> policy_state.LocalPolicyStateStoreV1:
        return policy_state.LocalPolicyStateStoreV1(
            self.root, fault_injector=fault_injector
        )

    def initialize(self) -> policy_state.LocalPolicyStateObservationV1:
        with self.store() as store:
            return store.pin_policy(POLICY_7, 7)

    def state_path(self) -> Path:
        return self.root / policy_state.STATE_FILE

    def lock_path(self) -> Path:
        return self.root / policy_state.LOCK_FILE

    def temp_path(self) -> Path:
        return self.root / policy_state.TEMP_FILE

    def replace_state_bytes(self, data: bytes) -> None:
        self.state_path().write_bytes(data)
        os.chmod(self.state_path(), 0o600)

    def assert_rejected_state(self, data: bytes) -> None:
        baseline = policy_state.PolicyReplayStateV1(
            policy_identity=POLICY_7,
            policy_epoch=7,
            generation=1,
            consumptions=(),
        )
        self.replace_state_bytes(baseline.canonical_bytes())
        self.replace_state_bytes(data)
        with self.store() as store, self.assertRaises(EvidenceError):
            store.observe()

    def test_empty_store_observes_none(self) -> None:
        with self.store() as store:
            self.assertIsNone(store.observe())

    def test_initial_pin_and_exact_retry_are_idempotent(self) -> None:
        initial = self.initialize()
        self.assertEqual(initial.outcome, "initialized")
        self.assertEqual(initial.state.policy_epoch, 7)
        self.assertEqual(initial.state.generation, 1)
        with self.store() as store:
            retry = store.pin_policy(POLICY_7, 7)
            observed = store.observe()
        self.assertEqual(retry.outcome, "already-pinned")
        self.assertEqual(retry.state_identity, initial.state_identity)
        self.assertIsNotNone(observed)
        assert observed is not None
        self.assertEqual(observed.state_identity, initial.state_identity)

    def test_policy_pin_prevents_rollback_and_epoch_equivocation(self) -> None:
        self.initialize()
        with self.store() as store:
            with self.assertRaisesRegex(EvidenceError, "rollback"):
                store.pin_policy(POLICY_8, 6)
            with self.assertRaisesRegex(EvidenceError, "without an epoch advance"):
                store.pin_policy(POLICY_8, 7)
            with self.assertRaisesRegex(EvidenceError, "without a new policy identity"):
                store.pin_policy(POLICY_7, 8)

    def test_policy_advance_is_monotonic_and_retains_replay_entries(self) -> None:
        self.initialize()
        with self.store() as store:
            consumed = store.consume_once(
                policy_identity=POLICY_7,
                policy_epoch=7,
                campaign_nonce="campaign-a",
                build_context_identity=CONTEXT_A,
            )
            advanced = store.pin_policy(POLICY_8, 8)
        self.assertEqual(consumed.outcome, "consumed")
        self.assertEqual(advanced.outcome, "advanced")
        self.assertEqual(advanced.state.policy_epoch, 8)
        self.assertEqual(advanced.state.consumptions, consumed.state.consumptions)
        with (
            self.store() as store,
            self.assertRaisesRegex(EvidenceError, "does not match the pinned policy"),
        ):
            store.consume_once(
                policy_identity=POLICY_7,
                policy_epoch=7,
                campaign_nonce="campaign-b",
                build_context_identity=CONTEXT_B,
            )

    def test_replay_consumption_is_exactly_idempotent(self) -> None:
        self.initialize()
        with self.store() as store:
            first = store.consume_once(
                policy_identity=POLICY_7,
                policy_epoch=7,
                campaign_nonce="campaign-a",
                build_context_identity=CONTEXT_A,
            )
            retry = store.consume_once(
                policy_identity=POLICY_7,
                policy_epoch=7,
                campaign_nonce="campaign-a",
                build_context_identity=CONTEXT_A,
            )
        self.assertEqual(first.outcome, "consumed")
        self.assertEqual(retry.outcome, "already-consumed")
        self.assertEqual(retry.state_identity, first.state_identity)
        self.assertEqual(retry.state.generation, first.state.generation)

    def test_replay_rejects_nonce_and_context_rebinding(self) -> None:
        self.initialize()
        with self.store() as store:
            store.consume_once(
                policy_identity=POLICY_7,
                policy_epoch=7,
                campaign_nonce="campaign-a",
                build_context_identity=CONTEXT_A,
            )
            with self.assertRaisesRegex(EvidenceError, "nonce was already consumed"):
                store.consume_once(
                    policy_identity=POLICY_7,
                    policy_epoch=7,
                    campaign_nonce="campaign-a",
                    build_context_identity=CONTEXT_B,
                )
            with self.assertRaisesRegex(EvidenceError, "context was already consumed"):
                store.consume_once(
                    policy_identity=POLICY_7,
                    policy_epoch=7,
                    campaign_nonce="campaign-b",
                    build_context_identity=CONTEXT_A,
                )

    def test_consumption_requires_an_exact_existing_pin(self) -> None:
        with (
            self.store() as store,
            self.assertRaisesRegex(EvidenceError, "must be pinned"),
        ):
            store.consume_once(
                policy_identity=POLICY_7,
                policy_epoch=7,
                campaign_nonce="campaign-a",
                build_context_identity=CONTEXT_A,
            )

    def test_fault_recovery_is_idempotent_for_initial_pin(self) -> None:
        for point in policy_state.FAULT_POINTS:
            with self.subTest(point=point):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    os.chmod(root, 0o700)

                    def inject(actual: str) -> None:
                        if actual == point:
                            raise InjectedCrash(point)

                    with policy_state.LocalPolicyStateStoreV1(
                        root, fault_injector=inject
                    ) as store:
                        with self.assertRaises(InjectedCrash):
                            store.pin_policy(POLICY_7, 7)
                    with policy_state.LocalPolicyStateStoreV1(root) as recovered:
                        retry = recovered.pin_policy(POLICY_7, 7)
                        observed = recovered.observe()
                    expected = (
                        "initialized"
                        if point
                        in {
                            "before-write",
                            "after-write",
                            "before-file-fsync",
                            "after-file-fsync",
                            "before-rename",
                        }
                        else "already-pinned"
                    )
                    self.assertEqual(retry.outcome, expected)
                    self.assertIsNotNone(observed)
                    self.assertFalse((root / policy_state.TEMP_FILE).exists())

    def test_fault_recovery_is_idempotent_for_consumption(self) -> None:
        for point in policy_state.FAULT_POINTS:
            with self.subTest(point=point):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    os.chmod(root, 0o700)
                    with policy_state.LocalPolicyStateStoreV1(root) as store:
                        store.pin_policy(POLICY_7, 7)

                    def inject(actual: str) -> None:
                        if actual == point:
                            raise InjectedCrash(point)

                    with policy_state.LocalPolicyStateStoreV1(
                        root, fault_injector=inject
                    ) as store:
                        with self.assertRaises(InjectedCrash):
                            store.consume_once(
                                policy_identity=POLICY_7,
                                policy_epoch=7,
                                campaign_nonce="campaign-a",
                                build_context_identity=CONTEXT_A,
                            )
                    with policy_state.LocalPolicyStateStoreV1(root) as recovered:
                        retry = recovered.consume_once(
                            policy_identity=POLICY_7,
                            policy_epoch=7,
                            campaign_nonce="campaign-a",
                            build_context_identity=CONTEXT_A,
                        )
                    expected = (
                        "consumed"
                        if point
                        in {
                            "before-write",
                            "after-write",
                            "before-file-fsync",
                            "after-file-fsync",
                            "before-rename",
                        }
                        else "already-consumed"
                    )
                    self.assertEqual(retry.outcome, expected)
                    self.assertFalse((root / policy_state.TEMP_FILE).exists())

    def test_short_writes_are_completed(self) -> None:
        real_write = os.write

        def short_write(descriptor: int, data: bytes) -> int:
            return real_write(descriptor, data[: max(1, len(data) // 3)])

        with mock.patch.object(policy_state.os, "write", side_effect=short_write):
            initial = self.initialize()
        with self.store() as store:
            observed = store.observe()
        self.assertIsNotNone(observed)
        assert observed is not None
        self.assertEqual(observed.state_identity, initial.state_identity)

    def test_interrupted_temp_is_removed_without_becoming_state(self) -> None:
        self.temp_path().write_bytes(b'{"truncated":')
        os.chmod(self.temp_path(), 0o600)
        with self.store() as store:
            result = store.pin_policy(POLICY_7, 7)
        self.assertEqual(result.outcome, "initialized")
        self.assertFalse(self.temp_path().exists())

    def test_state_directory_is_descriptor_pinned_against_substitution(self) -> None:
        store = self.store()
        original = self.root.with_name(self.root.name + "-original")
        os.rename(self.root, original)
        self.root.mkdir(mode=0o700)
        try:
            result = store.pin_policy(POLICY_7, 7)
            self.assertEqual(result.outcome, "initialized")
            self.assertTrue((original / policy_state.STATE_FILE).is_file())
            self.assertFalse((self.root / policy_state.STATE_FILE).exists())
        finally:
            store.close()
            for child in self.root.iterdir():
                child.unlink()
            self.root.rmdir()
            os.rename(original, self.root)

    def test_final_and_intermediate_directory_symlinks_are_rejected(self) -> None:
        real = self.root / "real"
        real.mkdir(mode=0o700)
        final_link = self.root / "final-link"
        final_link.symlink_to(real, target_is_directory=True)
        with self.assertRaises(EvidenceError):
            policy_state.LocalPolicyStateStoreV1(final_link)

        child = real / "child"
        child.mkdir(mode=0o700)
        intermediate = self.root / "intermediate"
        intermediate.symlink_to(real, target_is_directory=True)
        with self.assertRaises(EvidenceError):
            policy_state.LocalPolicyStateStoreV1(intermediate / "child")

    def test_nonprivate_directory_is_rejected(self) -> None:
        os.chmod(self.root, 0o755)
        with self.assertRaisesRegex(EvidenceError, "must be private"):
            self.store()

    def test_state_symlink_hardlink_fifo_socket_and_device_are_rejected(self) -> None:
        variants = ("symlink", "hardlink", "fifo", "socket", "device")
        for variant in variants:
            with self.subTest(variant=variant):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    os.chmod(root, 0o700)
                    state = root / policy_state.STATE_FILE
                    cleanup_socket: socket.socket | None = None
                    if variant == "symlink":
                        target = root / "target"
                        target.write_bytes(b"x")
                        os.chmod(target, 0o600)
                        state.symlink_to(target)
                    elif variant == "hardlink":
                        source = root / "source"
                        source.write_bytes(b"x")
                        os.chmod(source, 0o600)
                        os.link(source, state)
                    elif variant == "fifo":
                        os.mkfifo(state, 0o600)
                    elif variant == "socket":
                        cleanup_socket = socket.socket(socket.AF_UNIX)
                        cleanup_socket.bind(str(state))
                    else:
                        with self.assertRaises(EvidenceError):
                            policy_state._require_private_regular_metadata(
                                os.stat("/dev/null"), "policy state"
                            )
                        continue
                    try:
                        with policy_state.LocalPolicyStateStoreV1(root) as store:
                            with self.assertRaises(EvidenceError):
                                store.observe()
                    finally:
                        if cleanup_socket is not None:
                            cleanup_socket.close()

    def test_lock_symlink_hardlink_fifo_and_socket_are_rejected(self) -> None:
        for variant in ("symlink", "hardlink", "fifo", "socket"):
            with self.subTest(variant=variant):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    os.chmod(root, 0o700)
                    lock = root / policy_state.LOCK_FILE
                    cleanup_socket: socket.socket | None = None
                    if variant == "symlink":
                        target = root / "target"
                        target.write_bytes(b"x")
                        os.chmod(target, 0o600)
                        lock.symlink_to(target)
                    elif variant == "hardlink":
                        target = root / "target"
                        target.write_bytes(b"x")
                        os.chmod(target, 0o600)
                        os.link(target, lock)
                    elif variant == "fifo":
                        os.mkfifo(lock, 0o600)
                    else:
                        cleanup_socket = socket.socket(socket.AF_UNIX)
                        cleanup_socket.bind(str(lock))
                    try:
                        with policy_state.LocalPolicyStateStoreV1(root) as store:
                            with self.assertRaises(EvidenceError):
                                store.observe()
                    finally:
                        if cleanup_socket is not None:
                            cleanup_socket.close()

    def test_unsafe_recovery_temp_types_are_rejected(self) -> None:
        for variant in ("symlink", "hardlink", "fifo", "socket"):
            with self.subTest(variant=variant):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    os.chmod(root, 0o700)
                    temp = root / policy_state.TEMP_FILE
                    cleanup_socket: socket.socket | None = None
                    if variant == "symlink":
                        target = root / "target"
                        target.write_bytes(b"x")
                        os.chmod(target, 0o600)
                        temp.symlink_to(target)
                    elif variant == "hardlink":
                        target = root / "target"
                        target.write_bytes(b"x")
                        os.chmod(target, 0o600)
                        os.link(target, temp)
                    elif variant == "fifo":
                        os.mkfifo(temp, 0o600)
                    else:
                        cleanup_socket = socket.socket(socket.AF_UNIX)
                        cleanup_socket.bind(str(temp))
                    try:
                        with policy_state.LocalPolicyStateStoreV1(root) as store:
                            with self.assertRaises(EvidenceError):
                                store.observe()
                    finally:
                        if cleanup_socket is not None:
                            cleanup_socket.close()

    def test_nonprivate_state_file_is_rejected(self) -> None:
        self.initialize()
        os.chmod(self.state_path(), 0o644)
        with self.store() as store, self.assertRaisesRegex(EvidenceError, "mode 0600"):
            store.observe()

    def test_exclusive_lock_rejects_a_second_cooperating_process(self) -> None:
        self.initialize()
        descriptor = os.open(self.lock_path(), os.O_RDWR | os.O_CLOEXEC)
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
            with (
                self.store() as store,
                self.assertRaisesRegex(EvidenceError, "locked by another process"),
            ):
                store.observe()
        finally:
            os.close(descriptor)

    def test_malformed_truncated_trailing_and_oversized_state_are_rejected(
        self,
    ) -> None:
        malformed = (b'{"broken":}\n', b"{}", b"{}\ntrailing", b"\xff\n")
        for data in malformed:
            with self.subTest(data=data[:16]):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    os.chmod(root, 0o700)
                    state = root / policy_state.STATE_FILE
                    state.write_bytes(data)
                    os.chmod(state, 0o600)
                    with policy_state.LocalPolicyStateStoreV1(root) as store:
                        with self.assertRaises(EvidenceError):
                            store.observe()

        self.initialize()
        with self.state_path().open("wb") as output:
            output.truncate(policy_state.MAX_STATE_BYTES + 1)
        os.chmod(self.state_path(), 0o600)
        with self.store() as store, self.assertRaisesRegex(EvidenceError, "size bound"):
            store.observe()

    def test_noncanonical_duplicate_and_checksum_tampering_are_rejected(self) -> None:
        state = policy_state.PolicyReplayStateV1(
            policy_identity=POLICY_7,
            policy_epoch=7,
            generation=1,
            consumptions=(),
        )
        canonical = state.canonical_bytes()
        parsed = json.loads(canonical)
        noncanonical = (json.dumps(parsed, indent=2) + "\n").encode("ascii")
        self.assert_rejected_state(noncanonical)

        duplicate = canonical.replace(
            b'{"checksum":', b'{"domain":"duplicate","checksum":', 1
        )
        self.assert_rejected_state(duplicate)

        parsed["payload"]["generation"] = 2
        tampered = policy_state.canonical_json_bytes(parsed)
        self.assert_rejected_state(tampered)

    def test_wrong_fields_domains_versions_and_integer_forms_are_rejected(self) -> None:
        state = policy_state.PolicyReplayStateV1(
            policy_identity=POLICY_7,
            policy_epoch=7,
            generation=1,
            consumptions=(),
        )
        base = json.loads(state.canonical_bytes())
        mutations: list[dict[str, object]] = []
        wrong_domain = json.loads(json.dumps(base))
        wrong_domain["domain"] = "wrong-v1"
        mutations.append(wrong_domain)
        wrong_version = json.loads(json.dumps(base))
        wrong_version["schema_version"] = 2
        mutations.append(wrong_version)
        extra = json.loads(json.dumps(base))
        extra["extra"] = 1
        mutations.append(extra)
        boolean_epoch = json.loads(json.dumps(base))
        boolean_epoch["payload"]["policy_epoch"] = True
        mutations.append(boolean_epoch)
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                self.assert_rejected_state(policy_state.canonical_json_bytes(mutation))

        huge_integer = state.canonical_bytes().replace(
            b'"generation":1', b'"generation":123456789012345678901'
        )
        self.assert_rejected_state(huge_integer)

    def test_unsorted_and_duplicate_consumptions_are_rejected(self) -> None:
        first = policy_state.ReplayConsumptionV1("campaign-b", CONTEXT_B)
        second = policy_state.ReplayConsumptionV1("campaign-a", CONTEXT_A)
        with self.assertRaisesRegex(EvidenceError, "not canonically sorted"):
            policy_state.PolicyReplayStateV1(POLICY_7, 7, 1, (first, second))
        with self.assertRaisesRegex(EvidenceError, "reuses a campaign nonce"):
            policy_state.PolicyReplayStateV1(
                POLICY_7,
                7,
                1,
                tuple(
                    sorted(
                        (
                            policy_state.ReplayConsumptionV1("campaign-a", CONTEXT_A),
                            policy_state.ReplayConsumptionV1("campaign-a", CONTEXT_B),
                        )
                    )
                ),
            )

    def test_replay_ledger_capacity_fails_closed_without_unbounded_growth(self) -> None:
        entries = tuple(
            policy_state.ReplayConsumptionV1(
                f"campaign-{index:04d}",
                identity(policy_state.BUILD_CONTEXT_DOMAIN, f"context-{index:04d}"),
            )
            for index in range(policy_state.MAX_CONSUMPTIONS)
        )
        state = policy_state.PolicyReplayStateV1(POLICY_7, 7, 1, entries)
        self.replace_state_bytes(state.canonical_bytes())
        with (
            self.store() as store,
            self.assertRaisesRegex(EvidenceError, "ledger is full"),
        ):
            store.consume_once(
                policy_identity=POLICY_7,
                policy_epoch=7,
                campaign_nonce="campaign-overflow",
                build_context_identity=identity(
                    policy_state.BUILD_CONTEXT_DOMAIN, "context-overflow"
                ),
            )

    def test_more_than_maximum_consumptions_is_rejected_before_construction(
        self,
    ) -> None:
        entries = tuple(
            policy_state.ReplayConsumptionV1(
                f"campaign-{index:04d}",
                identity(policy_state.BUILD_CONTEXT_DOMAIN, f"context-{index:04d}"),
            )
            for index in range(policy_state.MAX_CONSUMPTIONS + 1)
        )
        with self.assertRaisesRegex(EvidenceError, "cardinality bound"):
            policy_state.PolicyReplayStateV1(POLICY_7, 7, 1, entries)

    def test_store_rejects_relative_paths_and_use_after_close(self) -> None:
        with self.assertRaisesRegex(EvidenceError, "absolute path"):
            policy_state.LocalPolicyStateStoreV1(Path("relative"))
        store = self.store()
        store.close()
        with self.assertRaisesRegex(EvidenceError, "closed"):
            store.observe()

    def test_observations_are_explicitly_forgeable_data(self) -> None:
        state = policy_state.PolicyReplayStateV1(POLICY_7, 7, 1, ())
        forged = policy_state.LocalPolicyStateObservationV1(
            outcome="made-up", state_identity="made-up", state=state
        )
        self.assertEqual(forged.outcome, "made-up")


if __name__ == "__main__":
    unittest.main()
