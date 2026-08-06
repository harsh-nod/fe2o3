#!/usr/bin/env python3

from __future__ import annotations

import fcntl
import json
import os
import socket
import sys
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path
from unittest import mock

SCRIPT_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPT_DIR))

import attestation  # noqa: E402
import policy_state  # noqa: E402
from common import EvidenceError, typed_identity  # noqa: E402

COMMIT = "34" * 20
TARGET = "gfx942:sramecc+:xnack-"
POLICY_EPOCH = 7
CAMPAIGN = "release-campaign-42"


def identity(domain: str, label: str) -> str:
    return typed_identity(domain, label.encode("ascii"))


POLICY_7 = identity(policy_state.POLICY_DOMAIN, "policy-7")
POLICY_8 = identity(policy_state.POLICY_DOMAIN, "policy-8")


def subjects_for(
    role: str, suffix: str = "default"
) -> tuple[attestation.SubjectIdentity, ...]:
    return tuple(
        attestation.SubjectIdentity(
            name,
            identity(domain, f"{role}:{name}:{suffix}"),
        )
        for name, domain in sorted(attestation.SUBJECT_IDENTITY_DOMAINS[role].items())
    )


def payload_for(
    role: str,
    *,
    campaign: str = CAMPAIGN,
    policy_identity: str = POLICY_7,
    policy_epoch: int = POLICY_EPOCH,
    suffix: str = "default",
) -> attestation.AttestationPayloadV1:
    subjects = subjects_for(role, suffix)
    build_context = attestation.derive_build_context_identity(
        source_commit=COMMIT,
        target=TARGET,
        role=role,
        policy_identity=policy_identity,
        policy_epoch=policy_epoch,
        campaign_nonce=campaign,
        subjects=subjects,
    )
    payload = attestation.AttestationPayloadV1(
        role=role,
        signer_identity=f"{role}-release-ci",
        source_commit=COMMIT,
        target=TARGET,
        issued_at=1_800_000_000,
        expires_at=1_800_003_600,
        policy_identity=policy_identity,
        policy_epoch=policy_epoch,
        campaign_nonce=campaign,
        build_context_identity=build_context,
        subjects=subjects,
    )
    return attestation.AttestationPayloadV1.from_bytes(payload.canonical_bytes())


def release_context(
    *,
    campaign: str = CAMPAIGN,
    policy_identity: str = POLICY_7,
    policy_epoch: int = POLICY_EPOCH,
    suffixes: dict[str, str] | None = None,
    bindings: tuple[policy_state.ReleaseAttestationBindingV1, ...] | None = None,
) -> policy_state.ReleaseContextIdentityV1:
    if bindings is None:
        selected: list[policy_state.ReleaseAttestationBindingV1] = []
        for role in policy_state.REQUIRED_RELEASE_ROLES:
            payload = payload_for(
                role,
                campaign=campaign,
                policy_identity=policy_identity,
                policy_epoch=policy_epoch,
                suffix=(suffixes or {}).get(role, "default"),
            )
            selected.append(
                policy_state.ReleaseAttestationBindingV1(
                    role=role,
                    build_context_identity=payload.build_context_identity,
                    signer_identity=payload.signer_identity,
                    attestation_identity=payload.identity(),
                )
            )
        bindings = tuple(selected)
    return policy_state.ReleaseContextIdentityV1(
        source_commit=COMMIT,
        target=TARGET,
        campaign_nonce=campaign,
        policy_identity=policy_identity,
        policy_epoch=policy_epoch,
        attestations=bindings,
    )


def attempt(
    context: policy_state.ReleaseContextIdentityV1, nonce: str = "attempt-1"
) -> policy_state.OperationAttemptIdentityV1:
    return policy_state.derive_operation_attempt_identity(
        release_context_identity=context.identity(), attempt_nonce=nonce
    )


class InjectedCrash(RuntimeError):
    pass


class PolicyStateTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        os.chmod(self.root, 0o700)
        self.context = release_context()
        self.attempt = attempt(self.context)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def store(
        self, fault_injector: policy_state.FaultInjector | None = None
    ) -> policy_state.LocalPolicyStateStoreV2:
        return policy_state.LocalPolicyStateStoreV2(
            self.root, fault_injector=fault_injector
        )

    def initialize(self) -> policy_state.LocalPolicyStateObservationV2:
        with self.store() as store:
            return store.pin_policy(POLICY_7, POLICY_EPOCH)

    def state_path(self) -> Path:
        return self.root / policy_state.STATE_FILE

    def lock_path(self) -> Path:
        return self.root / policy_state.LOCK_FILE

    def temp_path(self) -> Path:
        return self.root / policy_state.TEMP_FILE

    def replace_state_bytes(self, data: bytes) -> None:
        self.state_path().write_bytes(data)
        os.chmod(self.state_path(), 0o600)

    def empty_state(self, generation: int = 1) -> policy_state.PolicyReplayStateV2:
        return policy_state.PolicyReplayStateV2(
            policy_identity=POLICY_7,
            policy_epoch=POLICY_EPOCH,
            generation=generation,
            consumptions=(),
            pending_consumption=None,
        )

    def consumption(
        self,
        context: policy_state.ReleaseContextIdentityV1 | None = None,
        attempt_identity: policy_state.OperationAttemptIdentityV1 | None = None,
    ) -> policy_state.ReleaseConsumptionV1:
        selected = context or self.context
        return policy_state.ReleaseConsumptionV1(
            campaign_nonce=selected.campaign_nonce,
            release_context_identity=selected.identity(),
            operation_attempt_identity=(
                attempt_identity or attempt(selected)
            ).identity(),
        )

    def pending_after_crash(self) -> None:
        target = "plan-consumption:after-rename"

        def inject(point: str) -> None:
            if point == target:
                raise InjectedCrash(point)

        with self.store(inject) as store, self.assertRaises(InjectedCrash):
            store.consume_once(
                release_context=self.context,
                operation_attempt=self.attempt,
            )

    def assert_rejected_state(self, data: bytes) -> None:
        self.replace_state_bytes(data)
        with self.store() as store, self.assertRaises(EvidenceError):
            store.observe()

    def test_g2_and_g5_share_one_complete_campaign_aggregate(self) -> None:
        self.assertEqual(
            policy_state.REQUIRED_RELEASE_ROLES,
            tuple(sorted(attestation.ROLES)),
        )
        bindings = {binding.role: binding for binding in self.context.attestations}
        g2 = payload_for("g2-worker")
        g5 = payload_for("g5-publication")
        self.assertEqual(g2.campaign_nonce, g5.campaign_nonce)
        self.assertEqual(
            bindings["g2-worker"].build_context_identity,
            g2.build_context_identity,
        )
        self.assertEqual(
            bindings["g5-publication"].attestation_identity,
            g5.identity(),
        )
        self.assertEqual(tuple(bindings), policy_state.REQUIRED_RELEASE_ROLES)

        self.initialize()
        with self.store() as store:
            result = store.consume_once(
                release_context=self.context,
                operation_attempt=self.attempt,
            )
            observed = store.observe()
        self.assertIsInstance(result, policy_state.FreshConsumptionObservationV1)
        self.assertIsNotNone(observed)
        assert observed is not None
        self.assertEqual(len(observed.state.consumptions), 1)
        self.assertEqual(observed.state.generation, 3)
        self.assertIsNone(observed.state.pending_consumption)

    def test_aggregate_rejects_omitted_permuted_and_duplicate_roles(self) -> None:
        complete = self.context.attestations
        for bindings in (
            complete[:-1],
            tuple(reversed(complete)),
            tuple(sorted((*complete[:-1], complete[0]))),
        ):
            with self.subTest(bindings=tuple(item.role for item in bindings)):
                with self.assertRaises(EvidenceError):
                    release_context(bindings=bindings)

    def test_every_aggregate_substitution_changes_identity(self) -> None:
        baseline = self.context.identity()
        substitutions = (
            replace(self.context, source_commit="56" * 20),
            replace(self.context, target="gfx950"),
            release_context(campaign="another-campaign"),
            release_context(policy_identity=POLICY_8, policy_epoch=8),
            release_context(suffixes={"g2-worker": "changed"}),
            replace(
                self.context,
                attestations=tuple(
                    sorted(
                        (
                            replace(
                                self.context.attestations[0],
                                signer_identity="different-signer",
                            ),
                            *self.context.attestations[1:],
                        )
                    )
                ),
            ),
        )
        for changed in substitutions:
            with self.subTest(changed=changed):
                self.assertNotEqual(changed.identity(), baseline)

    def test_cross_role_binding_substitution_changes_aggregate_and_replay_rejects(
        self,
    ) -> None:
        original = list(self.context.attestations)
        g2 = next(item for item in original if item.role == "g2-worker")
        g5 = next(item for item in original if item.role == "g5-publication")
        swapped = [
            replace(g5, role="g2-worker")
            if item.role == "g2-worker"
            else replace(g2, role="g5-publication")
            if item.role == "g5-publication"
            else item
            for item in original
        ]
        cross_role = release_context(bindings=tuple(sorted(swapped)))
        self.assertNotEqual(cross_role.identity(), self.context.identity())

        self.initialize()
        with self.store() as store:
            store.consume_once(
                release_context=self.context,
                operation_attempt=self.attempt,
            )
            with self.assertRaisesRegex(EvidenceError, "campaign nonce"):
                store.consume_once(
                    release_context=cross_role,
                    operation_attempt=attempt(cross_role, "attempt-2"),
                )

    def test_aggregate_identity_is_canonical_and_domain_separated(self) -> None:
        self.assertEqual(self.context.identity(), release_context().identity())
        self.assertTrue(
            self.context.identity().startswith(
                f"{policy_state.RELEASE_CONTEXT_DOMAIN}-"
            )
        )
        self.assertNotEqual(
            self.context.identity(),
            typed_identity(
                policy_state.STATE_DOMAIN, self.context.canonical_preimage()
            ),
        )

    def test_operation_attempt_is_canonical_and_bound_to_exact_aggregate(self) -> None:
        first = attempt(self.context)
        self.assertEqual(first.identity(), attempt(self.context).identity())
        other_context = release_context(campaign="campaign-2")
        other = attempt(other_context)
        self.assertNotEqual(first.identity(), other.identity())
        self.initialize()
        with (
            self.store() as store,
            self.assertRaisesRegex(EvidenceError, "does not bind"),
        ):
            store.consume_once(
                release_context=self.context,
                operation_attempt=other,
            )

    def test_release_context_must_match_pinned_policy(self) -> None:
        self.initialize()
        other_policy = release_context(policy_identity=POLICY_8, policy_epoch=8)
        with (
            self.store() as store,
            self.assertRaisesRegex(EvidenceError, "pinned policy"),
        ):
            store.consume_once(
                release_context=other_policy,
                operation_attempt=attempt(other_policy),
            )

    def test_initial_pin_and_monotonic_policy_advance(self) -> None:
        initial = self.initialize()
        self.assertEqual(initial.state.generation, 1)
        with self.store() as store:
            retry = store.pin_policy(POLICY_7, POLICY_EPOCH)
            with self.assertRaisesRegex(EvidenceError, "rollback"):
                store.pin_policy(POLICY_8, POLICY_EPOCH - 1)
            with self.assertRaisesRegex(EvidenceError, "without an epoch advance"):
                store.pin_policy(POLICY_8, POLICY_EPOCH)
            advanced = store.pin_policy(POLICY_8, POLICY_EPOCH + 1)
        self.assertEqual(retry.outcome, "already-pinned")
        self.assertEqual(advanced.outcome, "advanced")
        self.assertEqual(advanced.state.generation, 2)

    def test_fresh_replay_rejects_same_and_new_attempts(self) -> None:
        self.initialize()
        with self.store() as store:
            store.consume_once(
                release_context=self.context,
                operation_attempt=self.attempt,
            )
            for attempt_identity in (self.attempt, attempt(self.context, "attempt-2")):
                with self.subTest(attempt_identity=attempt_identity):
                    with self.assertRaisesRegex(EvidenceError, "replay is forbidden"):
                        store.consume_once(
                            release_context=self.context,
                            operation_attempt=attempt_identity,
                        )

    def test_only_exact_attempt_can_observe_completed_recovery(self) -> None:
        self.initialize()
        with self.store() as store:
            store.consume_once(
                release_context=self.context,
                operation_attempt=self.attempt,
            )
            recovery = store.resume_consumption(
                release_context=self.context,
                operation_attempt=self.attempt,
            )
            with self.assertRaisesRegex(EvidenceError, "exactly match"):
                store.resume_consumption(
                    release_context=self.context,
                    operation_attempt=attempt(self.context, "attempt-2"),
                )
        self.assertIsInstance(recovery, policy_state.ConsumptionRecoveryObservationV1)
        self.assertEqual(recovery.outcome, "completion-observed")

    def test_resume_requires_a_durably_registered_attempt(self) -> None:
        self.initialize()
        with (
            self.store() as store,
            self.assertRaisesRegex(EvidenceError, "not durably registered"),
        ):
            store.resume_consumption(
                release_context=self.context,
                operation_attempt=self.attempt,
            )

    def test_pending_attempt_requires_resume_and_exact_attempt(self) -> None:
        self.initialize()
        self.pending_after_crash()
        with self.store() as store:
            for selected in (self.attempt, attempt(self.context, "attempt-2")):
                with self.subTest(selected=selected):
                    with self.assertRaisesRegex(EvidenceError, "use resume"):
                        store.consume_once(
                            release_context=self.context,
                            operation_attempt=selected,
                        )
            with self.assertRaisesRegex(EvidenceError, "exactly match"):
                store.resume_consumption(
                    release_context=self.context,
                    operation_attempt=attempt(self.context, "attempt-2"),
                )
            recovered = store.resume_consumption(
                release_context=self.context,
                operation_attempt=self.attempt,
            )
        self.assertEqual(recovered.outcome, "resumed-and-completed")

    def test_policy_cannot_advance_with_pending_attempt(self) -> None:
        self.initialize()
        self.pending_after_crash()
        with self.store() as store, self.assertRaisesRegex(EvidenceError, "pending"):
            store.pin_policy(POLICY_8, POLICY_EPOCH + 1)

    def test_policy_pin_crash_boundaries_recover_old_or_new_state(self) -> None:
        before_rename = {
            "before-write",
            "after-write",
            "before-file-fsync",
            "after-file-fsync",
            "before-rename",
        }
        for boundary in policy_state.DURABILITY_BOUNDARIES:
            with self.subTest(boundary=boundary):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    os.chmod(root, 0o700)
                    target = f"pin-policy:{boundary}"

                    def inject(point: str) -> None:
                        if point == target:
                            raise InjectedCrash(point)

                    with policy_state.LocalPolicyStateStoreV2(
                        root, fault_injector=inject
                    ) as store:
                        with self.assertRaises(InjectedCrash):
                            store.pin_policy(POLICY_7, POLICY_EPOCH)
                    with policy_state.LocalPolicyStateStoreV2(root) as recovered:
                        result = recovered.pin_policy(POLICY_7, POLICY_EPOCH)
                    expected = (
                        "initialized" if boundary in before_rename else "already-pinned"
                    )
                    self.assertEqual(result.outcome, expected)

    def test_policy_advance_crash_boundaries_recover_old_or_new_state(self) -> None:
        before_rename = {
            "before-write",
            "after-write",
            "before-file-fsync",
            "after-file-fsync",
            "before-rename",
        }
        for boundary in policy_state.DURABILITY_BOUNDARIES:
            with self.subTest(boundary=boundary):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    os.chmod(root, 0o700)
                    with policy_state.LocalPolicyStateStoreV2(root) as store:
                        store.pin_policy(POLICY_7, POLICY_EPOCH)
                    target = f"advance-policy:{boundary}"

                    def inject(point: str) -> None:
                        if point == target:
                            raise InjectedCrash(point)

                    with policy_state.LocalPolicyStateStoreV2(
                        root, fault_injector=inject
                    ) as store:
                        with self.assertRaises(InjectedCrash):
                            store.pin_policy(POLICY_8, POLICY_EPOCH + 1)
                    with policy_state.LocalPolicyStateStoreV2(root) as recovered:
                        result = recovered.pin_policy(POLICY_8, POLICY_EPOCH + 1)
                    expected = (
                        "advanced" if boundary in before_rename else "already-pinned"
                    )
                    self.assertEqual(result.outcome, expected)

    def test_plan_crash_boundaries_require_fresh_or_resume_by_durable_outcome(
        self,
    ) -> None:
        before_rename = {
            "before-write",
            "after-write",
            "before-file-fsync",
            "after-file-fsync",
            "before-rename",
        }
        for boundary in policy_state.DURABILITY_BOUNDARIES:
            with self.subTest(boundary=boundary):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    os.chmod(root, 0o700)
                    context = release_context()
                    attempt_identity = attempt(context)
                    with policy_state.LocalPolicyStateStoreV2(root) as store:
                        store.pin_policy(POLICY_7, POLICY_EPOCH)

                    target = f"plan-consumption:{boundary}"

                    def inject(point: str) -> None:
                        if point == target:
                            raise InjectedCrash(point)

                    with policy_state.LocalPolicyStateStoreV2(
                        root, fault_injector=inject
                    ) as store:
                        with self.assertRaises(InjectedCrash):
                            store.consume_once(
                                release_context=context,
                                operation_attempt=attempt_identity,
                            )
                    with policy_state.LocalPolicyStateStoreV2(root) as recovered:
                        if boundary in before_rename:
                            with self.assertRaisesRegex(
                                EvidenceError, "not durably registered"
                            ):
                                recovered.resume_consumption(
                                    release_context=context,
                                    operation_attempt=attempt_identity,
                                )
                            result = recovered.consume_once(
                                release_context=context,
                                operation_attempt=attempt_identity,
                            )
                            self.assertIsInstance(
                                result, policy_state.FreshConsumptionObservationV1
                            )
                        else:
                            with self.assertRaisesRegex(EvidenceError, "use resume"):
                                recovered.consume_once(
                                    release_context=context,
                                    operation_attempt=attempt_identity,
                                )
                            result = recovered.resume_consumption(
                                release_context=context,
                                operation_attempt=attempt_identity,
                            )
                            self.assertEqual(result.outcome, "resumed-and-completed")
                    self.assertFalse((root / policy_state.TEMP_FILE).exists())

    def test_completion_crash_boundaries_resume_exactly(self) -> None:
        before_rename = {
            "before-write",
            "after-write",
            "before-file-fsync",
            "after-file-fsync",
            "before-rename",
        }
        for boundary in policy_state.DURABILITY_BOUNDARIES:
            with self.subTest(boundary=boundary):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    os.chmod(root, 0o700)
                    context = release_context()
                    attempt_identity = attempt(context)
                    with policy_state.LocalPolicyStateStoreV2(root) as store:
                        store.pin_policy(POLICY_7, POLICY_EPOCH)

                    target = f"complete-consumption:{boundary}"

                    def inject(point: str) -> None:
                        if point == target:
                            raise InjectedCrash(point)

                    with policy_state.LocalPolicyStateStoreV2(
                        root, fault_injector=inject
                    ) as store:
                        with self.assertRaises(InjectedCrash):
                            store.consume_once(
                                release_context=context,
                                operation_attempt=attempt_identity,
                            )
                    with policy_state.LocalPolicyStateStoreV2(root) as recovered:
                        result = recovered.resume_consumption(
                            release_context=context,
                            operation_attempt=attempt_identity,
                        )
                    expected = (
                        "resumed-and-completed"
                        if boundary in before_rename
                        else "completion-observed"
                    )
                    self.assertEqual(result.outcome, expected)
                    self.assertFalse((root / policy_state.TEMP_FILE).exists())

    def test_resume_crash_boundaries_are_exactly_idempotent(self) -> None:
        before_rename = {
            "before-write",
            "after-write",
            "before-file-fsync",
            "after-file-fsync",
            "before-rename",
        }
        for boundary in policy_state.DURABILITY_BOUNDARIES:
            with self.subTest(boundary=boundary):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    os.chmod(root, 0o700)
                    context = release_context()
                    attempt_identity = attempt(context)
                    with policy_state.LocalPolicyStateStoreV2(root) as store:
                        store.pin_policy(POLICY_7, POLICY_EPOCH)

                    plan_target = "plan-consumption:after-rename"

                    def stop_after_plan(point: str) -> None:
                        if point == plan_target:
                            raise InjectedCrash(point)

                    with policy_state.LocalPolicyStateStoreV2(
                        root, fault_injector=stop_after_plan
                    ) as store:
                        with self.assertRaises(InjectedCrash):
                            store.consume_once(
                                release_context=context,
                                operation_attempt=attempt_identity,
                            )

                    resume_target = f"resume-consumption:{boundary}"

                    def stop_resume(point: str) -> None:
                        if point == resume_target:
                            raise InjectedCrash(point)

                    with policy_state.LocalPolicyStateStoreV2(
                        root, fault_injector=stop_resume
                    ) as store:
                        with self.assertRaises(InjectedCrash):
                            store.resume_consumption(
                                release_context=context,
                                operation_attempt=attempt_identity,
                            )
                    with policy_state.LocalPolicyStateStoreV2(root) as recovered:
                        result = recovered.resume_consumption(
                            release_context=context,
                            operation_attempt=attempt_identity,
                        )
                    expected = (
                        "resumed-and-completed"
                        if boundary in before_rename
                        else "completion-observed"
                    )
                    self.assertEqual(result.outcome, expected)

    def test_generation_must_cover_exact_consumption_phases(self) -> None:
        completed = self.consumption()
        with self.assertRaisesRegex(EvidenceError, "generation"):
            policy_state.PolicyReplayStateV2(
                POLICY_7, POLICY_EPOCH, 2, (completed,), None
            )
        with self.assertRaisesRegex(EvidenceError, "generation"):
            policy_state.PolicyReplayStateV2(POLICY_7, POLICY_EPOCH, 1, (), completed)
        legal_completed = policy_state.PolicyReplayStateV2(
            POLICY_7, POLICY_EPOCH, 3, (completed,), None
        )
        self.assertGreaterEqual(
            legal_completed.generation, 1 + len(legal_completed.consumptions)
        )

    def test_impossible_duplicate_and_pending_states_are_rejected(self) -> None:
        first = self.consumption()
        different_context = release_context(campaign="campaign-2")
        second = self.consumption(
            different_context, attempt(different_context, "attempt-2")
        )
        duplicate_campaign = replace(second, campaign_nonce=first.campaign_nonce)
        duplicate_context = replace(
            second, release_context_identity=first.release_context_identity
        )
        duplicate_attempt = replace(
            second, operation_attempt_identity=first.operation_attempt_identity
        )
        for duplicate, message in (
            (duplicate_campaign, "campaign nonce"),
            (duplicate_context, "release context"),
            (duplicate_attempt, "operation attempt"),
        ):
            with (
                self.subTest(message=message),
                self.assertRaisesRegex(EvidenceError, message),
            ):
                policy_state.PolicyReplayStateV2(
                    POLICY_7,
                    POLICY_EPOCH,
                    5,
                    tuple(sorted((first, duplicate))),
                    None,
                )
        with self.assertRaisesRegex(EvidenceError, "canonically sorted"):
            policy_state.PolicyReplayStateV2(
                POLICY_7, POLICY_EPOCH, 5, (first, second), None
            )
        with self.assertRaisesRegex(EvidenceError, "campaign nonce|release context"):
            policy_state.PolicyReplayStateV2(POLICY_7, POLICY_EPOCH, 4, (first,), first)

    def test_transition_validator_rejects_generation_skip_and_illegal_edges(
        self,
    ) -> None:
        initial = self.empty_state()
        candidate = self.consumption()
        planned = policy_state.PolicyReplayStateV2(
            POLICY_7, POLICY_EPOCH, 2, (), candidate
        )
        completed = policy_state.PolicyReplayStateV2(
            POLICY_7, POLICY_EPOCH, 3, (candidate,), None
        )
        policy_state._require_legal_transition(initial, planned, "plan-consumption")
        policy_state._require_legal_transition(
            planned, completed, "complete-consumption"
        )

        skipped = replace(planned, generation=3)
        with self.assertRaisesRegex(EvidenceError, "exactly once"):
            policy_state._require_legal_transition(initial, skipped, "plan-consumption")
        with self.assertRaisesRegex(EvidenceError, "exactly once|illegal"):
            policy_state._require_legal_transition(
                initial, completed, "plan-consumption"
            )
        with self.assertRaisesRegex(EvidenceError, "illegal"):
            policy_state._require_legal_transition(
                initial, replace(planned, policy_identity=POLICY_8), "plan-consumption"
            )
        with self.assertRaisesRegex(EvidenceError, "illegal"):
            policy_state._require_legal_transition(
                planned,
                replace(completed, consumptions=()),
                "complete-consumption",
            )

    def test_policy_advance_transition_preserves_consumptions_exactly(self) -> None:
        completed = self.consumption()
        previous = policy_state.PolicyReplayStateV2(
            POLICY_7, POLICY_EPOCH, 3, (completed,), None
        )
        legal = policy_state.PolicyReplayStateV2(
            POLICY_8, POLICY_EPOCH + 1, 4, (completed,), None
        )
        policy_state._require_legal_transition(previous, legal, "advance-policy")
        with self.assertRaisesRegex(EvidenceError, "illegal"):
            policy_state._require_legal_transition(
                previous, replace(legal, consumptions=()), "advance-policy"
            )

    def test_canonical_state_rejects_impossible_generation_even_with_valid_checksum(
        self,
    ) -> None:
        completed = self.consumption()
        legal = policy_state.PolicyReplayStateV2(
            POLICY_7, POLICY_EPOCH, 3, (completed,), None
        )
        value = json.loads(legal.canonical_bytes())
        value["payload"]["generation"] = 2
        body = {
            "domain": value["domain"],
            "payload": value["payload"],
            "schema_version": value["schema_version"],
        }
        value["checksum"] = typed_identity(
            policy_state.INTEGRITY_DOMAIN,
            policy_state.canonical_json_bytes(body),
        )
        self.assert_rejected_state(policy_state.canonical_json_bytes(value))

    def test_state_codec_rejects_malformed_noncanonical_tampered_and_oversized(
        self,
    ) -> None:
        for data in (b'{"broken":}\n', b"{}", b"{}\ntrailing", b"\xff\n"):
            with self.subTest(data=data):
                self.assert_rejected_state(data)
        valid = self.empty_state().canonical_bytes()
        pretty = (json.dumps(json.loads(valid), indent=2) + "\n").encode("ascii")
        self.assert_rejected_state(pretty)
        tampered = json.loads(valid)
        tampered["payload"]["generation"] = 2
        self.assert_rejected_state(policy_state.canonical_json_bytes(tampered))
        with self.state_path().open("wb") as output:
            output.truncate(policy_state.MAX_STATE_BYTES + 1)
        os.chmod(self.state_path(), 0o600)
        with self.store() as store, self.assertRaisesRegex(EvidenceError, "size bound"):
            store.observe()

    def test_ledger_capacity_fails_closed(self) -> None:
        entries = tuple(
            policy_state.ReleaseConsumptionV1(
                campaign_nonce=f"campaign-{index:04d}",
                release_context_identity=identity(
                    policy_state.RELEASE_CONTEXT_DOMAIN, f"context-{index:04d}"
                ),
                operation_attempt_identity=identity(
                    policy_state.OPERATION_ATTEMPT_DOMAIN, f"attempt-{index:04d}"
                ),
            )
            for index in range(policy_state.MAX_CONSUMPTIONS)
        )
        state = policy_state.PolicyReplayStateV2(
            POLICY_7,
            POLICY_EPOCH,
            1 + 2 * policy_state.MAX_CONSUMPTIONS,
            entries,
            None,
        )
        self.replace_state_bytes(state.canonical_bytes())
        overflow = release_context(campaign="campaign-overflow")
        with (
            self.store() as store,
            self.assertRaisesRegex(EvidenceError, "ledger is full"),
        ):
            store.consume_once(
                release_context=overflow,
                operation_attempt=attempt(overflow, "attempt-overflow"),
            )

    def test_legacy_v1_state_fails_closed(self) -> None:
        legacy = self.root / policy_state.LEGACY_STATE_FILES[0]
        legacy.write_bytes(b"{}\n")
        os.chmod(legacy, 0o600)
        with self.store() as store, self.assertRaisesRegex(EvidenceError, "migration"):
            store.observe()

    def test_interrupted_temp_is_discarded_without_becoming_state(self) -> None:
        self.temp_path().write_bytes(b'{"truncated":')
        os.chmod(self.temp_path(), 0o600)
        result = self.initialize()
        self.assertEqual(result.outcome, "initialized")
        self.assertFalse(self.temp_path().exists())

    def test_directory_symlink_permissions_and_descriptor_substitution(self) -> None:
        real = self.root / "real"
        real.mkdir(mode=0o700)
        link = self.root / "link"
        link.symlink_to(real, target_is_directory=True)
        with self.assertRaises(EvidenceError):
            policy_state.LocalPolicyStateStoreV2(link)

        store = self.store()
        original = self.root.with_name(self.root.name + "-original")
        os.rename(self.root, original)
        self.root.mkdir(mode=0o700)
        try:
            store.pin_policy(POLICY_7, POLICY_EPOCH)
            self.assertTrue((original / policy_state.STATE_FILE).is_file())
            self.assertFalse(self.state_path().exists())
        finally:
            store.close()
            self.root.rmdir()
            os.rename(original, self.root)

        store = self.store()
        os.chmod(self.root, 0o755)
        try:
            with self.assertRaisesRegex(EvidenceError, "private"):
                store.observe()
        finally:
            store.close()

    def test_state_symlink_hardlink_fifo_socket_and_device_are_rejected(self) -> None:
        for variant in ("symlink", "hardlink", "fifo", "socket", "device"):
            with self.subTest(variant=variant):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    os.chmod(root, 0o700)
                    state = root / policy_state.STATE_FILE
                    opened_socket: socket.socket | None = None
                    if variant == "symlink":
                        target = root / "target"
                        target.write_bytes(b"x")
                        os.chmod(target, 0o600)
                        state.symlink_to(target)
                    elif variant == "hardlink":
                        target = root / "target"
                        target.write_bytes(b"x")
                        os.chmod(target, 0o600)
                        os.link(target, state)
                    elif variant == "fifo":
                        os.mkfifo(state, 0o600)
                    elif variant == "socket":
                        opened_socket = socket.socket(socket.AF_UNIX)
                        opened_socket.bind(str(state))
                    else:
                        with self.assertRaises(EvidenceError):
                            policy_state._require_private_regular_metadata(
                                os.stat("/dev/null"), "policy state"
                            )
                        continue
                    try:
                        with policy_state.LocalPolicyStateStoreV2(root) as store:
                            with self.assertRaises(EvidenceError):
                                store.observe()
                    finally:
                        if opened_socket is not None:
                            opened_socket.close()

    def test_lock_and_temp_special_files_are_rejected(self) -> None:
        for filename in (policy_state.LOCK_FILE, policy_state.TEMP_FILE):
            for variant in ("symlink", "hardlink", "fifo", "socket"):
                with self.subTest(filename=filename, variant=variant):
                    with tempfile.TemporaryDirectory() as raw_root:
                        root = Path(raw_root)
                        os.chmod(root, 0o700)
                        path = root / filename
                        opened_socket: socket.socket | None = None
                        if variant == "symlink":
                            target = root / "target"
                            target.write_bytes(b"x")
                            os.chmod(target, 0o600)
                            path.symlink_to(target)
                        elif variant == "hardlink":
                            target = root / "target"
                            target.write_bytes(b"x")
                            os.chmod(target, 0o600)
                            os.link(target, path)
                        elif variant == "fifo":
                            os.mkfifo(path, 0o600)
                        else:
                            opened_socket = socket.socket(socket.AF_UNIX)
                            opened_socket.bind(str(path))
                        try:
                            with policy_state.LocalPolicyStateStoreV2(root) as store:
                                with self.assertRaises(EvidenceError):
                                    store.observe()
                        finally:
                            if opened_socket is not None:
                                opened_socket.close()

    def test_exclusive_lock_and_private_state_mode_are_enforced(self) -> None:
        self.initialize()
        descriptor = os.open(self.lock_path(), os.O_RDWR | os.O_CLOEXEC)
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
            with self.store() as store, self.assertRaisesRegex(EvidenceError, "locked"):
                store.observe()
        finally:
            os.close(descriptor)
        os.chmod(self.state_path(), 0o644)
        with self.store() as store, self.assertRaisesRegex(EvidenceError, "mode 0600"):
            store.observe()

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
        self.assertEqual(initial.state_identity, observed.state_identity)

    def test_bounded_api_inputs_and_closed_store_fail(self) -> None:
        oversized_policy = f"{POLICY_7}{'0' * 10_000}"
        with mock.patch.object(policy_state, "require_typed_identity") as validator:
            with (
                self.store() as store,
                self.assertRaisesRegex(EvidenceError, "bounded"),
            ):
                store.pin_policy(oversized_policy, POLICY_EPOCH)
            validator.assert_not_called()
        with self.assertRaisesRegex(EvidenceError, "exceeds"):
            policy_state.require_campaign_nonce("a" * 10_000)
        with self.assertRaisesRegex(EvidenceError, "absolute"):
            policy_state.LocalPolicyStateStoreV2(Path("relative"))
        store = self.store()
        store.close()
        with self.assertRaisesRegex(EvidenceError, "closed"):
            store.observe()

    def test_observations_are_forgeable_and_release_modules_remain_disconnected(
        self,
    ) -> None:
        forged = policy_state.FreshConsumptionObservationV1(
            state_identity="forged",
            release_context_identity="forged",
            operation_attempt_identity="forged",
        )
        self.assertEqual(forged.state_identity, "forged")
        for filename in ("attestation.py", "evidence.py", "reproduce.py"):
            source = (SCRIPT_DIR / filename).read_text(encoding="ascii")
            self.assertNotIn("import policy_state", source)
            self.assertNotIn("from policy_state", source)


if __name__ == "__main__":
    unittest.main()
