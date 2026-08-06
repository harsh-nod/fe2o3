#!/usr/bin/env python3

from __future__ import annotations

import json
import os
import socket
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest import mock

SCRIPT_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPT_DIR))

import attestation  # noqa: E402
from common import EvidenceError, typed_identity  # noqa: E402

COMMIT = "34" * 20
TARGET = "gfx942:sramecc+:xnack-"
ISSUED_AT = 1_800_000_000
EXPIRES_AT = ISSUED_AT + 3600
POLICY_EPOCH = 7
CAMPAIGN_NONCE = "release-campaign-42"
SIGNER = "g2-release-ci"


def subjects_for(role: str, suffix: str = "default") -> dict[str, str]:
    return {
        name: typed_identity(domain, f"{role}:{name}:{suffix}".encode("ascii"))
        for name, domain in attestation.SUBJECT_IDENTITY_DOMAINS[role].items()
    }


class AttestationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not attestation.SSH_KEYGEN_PATH.is_file():
            raise unittest.SkipTest("/usr/bin/ssh-keygen is unavailable")
        cls.keys = tempfile.TemporaryDirectory()
        root = Path(cls.keys.name)
        cls.private_key = root / "signer"
        cls.other_private_key = root / "other-signer"
        cls._generate_key(cls.private_key)
        cls._generate_key(cls.other_private_key)
        cls.public_key = cls._read_public_key(cls.private_key.with_suffix(".pub"))
        cls.other_public_key = cls._read_public_key(
            cls.other_private_key.with_suffix(".pub")
        )
        cls.verifier_identity = attestation.measure_verifier_identity()

    @classmethod
    def tearDownClass(cls) -> None:
        cls.keys.cleanup()

    @staticmethod
    def _generate_key(path: Path) -> None:
        result = subprocess.run(
            [
                str(attestation.SSH_KEYGEN_PATH),
                "-q",
                "-t",
                "ed25519",
                "-N",
                "",
                "-f",
                str(path),
            ],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        if result.returncode != 0:
            raise unittest.SkipTest("ssh-keygen cannot generate Ed25519 test keys")

    @staticmethod
    def _read_public_key(path: Path) -> str:
        fields = path.read_text(encoding="ascii").strip().split(" ")
        return " ".join(fields[:2])

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.subjects = subjects_for("g2-worker")
        self.policy = self.make_policy()
        self.policy_path = self.root / "policy.json"
        self.policy_path.write_bytes(self.policy.canonical_bytes())
        self.payload = self.make_payload()
        self.payload_path = self.root / "attestation.json"
        self.payload_path.write_bytes(self.payload.canonical_bytes())
        self.signature_path = self.sign(self.payload_path)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def make_binding(
        self,
        *,
        role: str = "g2-worker",
        signer: str = SIGNER,
        public_key: str | None = None,
        valid_from: int = 0,
        valid_until: int = attestation.MAX_UNIX_TIMESTAMP,
    ) -> attestation.SignerBindingV1:
        key = public_key or self.public_key
        return attestation.SignerBindingV1(
            role=role,
            signer_identity=signer,
            public_key=key,
            key_identity=attestation.public_key_identity(key),
            valid_from=valid_from,
            valid_until=valid_until,
        )

    def make_policy(
        self,
        *,
        public_key: str | None = None,
        role: str = "g2-worker",
        signer: str = SIGNER,
        verifier_identity: str | None = None,
        policy_epoch: int = POLICY_EPOCH,
        signers: tuple[attestation.SignerBindingV1, ...] | None = None,
    ) -> attestation.TrustPolicyV1:
        selected = signers or (
            self.make_binding(role=role, signer=signer, public_key=public_key),
        )
        return attestation.TrustPolicyV1(
            verifier_identity=verifier_identity or self.verifier_identity,
            policy_epoch=policy_epoch,
            signers=selected,
        )

    def make_payload(
        self,
        *,
        policy: attestation.TrustPolicyV1 | None = None,
        role: str = "g2-worker",
        signer: str = SIGNER,
        commit: str = COMMIT,
        target: str = TARGET,
        issued_at: int = ISSUED_AT,
        expires_at: int = EXPIRES_AT,
        campaign_nonce: str = CAMPAIGN_NONCE,
        subjects: dict[str, str] | None = None,
    ) -> attestation.AttestationPayloadV1:
        selected_policy = self.policy if policy is None else policy
        selected_subjects = self.subjects if subjects is None else subjects
        canonical_subjects = tuple(
            attestation.SubjectIdentity(name, selected_subjects[name])
            for name in sorted(selected_subjects)
        )
        context = attestation.derive_build_context_identity(
            source_commit=commit,
            target=target,
            role=role,
            policy_identity=selected_policy.identity(),
            policy_epoch=selected_policy.policy_epoch,
            campaign_nonce=campaign_nonce,
            subjects=canonical_subjects,
        )
        return attestation.AttestationPayloadV1(
            role=role,
            signer_identity=signer,
            source_commit=commit,
            target=target,
            issued_at=issued_at,
            expires_at=expires_at,
            policy_identity=selected_policy.identity(),
            policy_epoch=selected_policy.policy_epoch,
            campaign_nonce=campaign_nonce,
            build_context_identity=context,
            subjects=canonical_subjects,
        )

    def sign(
        self,
        payload: Path,
        key: Path | None = None,
        namespace: str = attestation.SIGNATURE_NAMESPACE,
    ) -> Path:
        private_key = key or self.private_key
        signature = payload.with_name(payload.name + ".sig")
        signature.unlink(missing_ok=True)
        result = subprocess.run(
            [
                str(attestation.SSH_KEYGEN_PATH),
                "-Y",
                "sign",
                "-f",
                str(private_key),
                "-n",
                namespace,
                str(payload),
            ],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        self.assertTrue(signature.is_file())
        return signature

    def verify(
        self, **overrides: object
    ) -> attestation.VerifiedAttestationObservationV1:
        arguments: dict[str, object] = {
            "policy_path": self.policy_path,
            "expected_policy_identity": self.policy.identity(),
            "payload_path": self.payload_path,
            "signature_path": self.signature_path,
            "expected_role": "g2-worker",
            "expected_signer_identity": SIGNER,
            "expected_source_commit": COMMIT,
            "expected_target": TARGET,
            "expected_policy_epoch": POLICY_EPOCH,
            "expected_campaign_nonce": CAMPAIGN_NONCE,
            "expected_subjects": self.subjects,
            "now": ISSUED_AT + 1,
            "timeout": 5,
        }
        arguments.update(overrides)
        return attestation.verify_signed_attestation(**arguments)  # type: ignore[arg-type]

    def rewrite_payload_object(self, value: dict[str, object]) -> None:
        self.payload_path.write_bytes(attestation.canonical_json_bytes(value))

    def install_payload(
        self,
        payload: attestation.AttestationPayloadV1,
        key: Path | None = None,
        namespace: str = attestation.SIGNATURE_NAMESPACE,
    ) -> None:
        self.payload = payload
        self.payload_path.write_bytes(payload.canonical_bytes())
        self.signature_path = self.sign(self.payload_path, key, namespace)

    def install_policy(self, policy: attestation.TrustPolicyV1) -> None:
        self.policy = policy
        self.policy_path.write_bytes(policy.canonical_bytes())

    def test_verifies_real_ed25519_signature_with_pinned_ssh_keygen(self) -> None:
        observation = self.verify()
        self.assertEqual(observation.payload, self.payload)
        self.assertEqual(observation.policy_identity, self.policy.identity())
        self.assertEqual(observation.verifier_identity, self.verifier_identity)
        self.assertEqual(observation.attestation_identity, self.payload.identity())
        self.assertEqual(
            observation.key_identity,
            attestation.public_key_identity(self.public_key),
        )
        self.assertFalse(hasattr(observation, "load_authority"))
        self.assertFalse(hasattr(observation, "launch_authority"))

    def test_observation_dataclass_is_forgeable_and_has_no_authority(self) -> None:
        forged = attestation.VerifiedAttestationObservationV1(
            attestation_identity="attacker-chosen",
            policy_identity="attacker-chosen",
            verifier_identity="attacker-chosen",
            key_identity="attacker-chosen",
            payload=self.payload,
        )
        self.assertEqual(forged.attestation_identity, "attacker-chosen")
        self.assertFalse(hasattr(forged, "authorize"))

    def test_cli_verifies_authenticated_observation(self) -> None:
        arguments = [
            sys.executable,
            str(SCRIPT_DIR / "attestation.py"),
            "verify",
            "--policy",
            str(self.policy_path),
            "--expect-policy-identity",
            self.policy.identity(),
            "--payload",
            str(self.payload_path),
            "--signature",
            str(self.signature_path),
            "--expect-role",
            "g2-worker",
            "--expect-signer",
            SIGNER,
            "--expect-source-commit",
            COMMIT,
            "--expect-target",
            TARGET,
            "--expect-policy-epoch",
            str(POLICY_EPOCH),
            "--expect-campaign-nonce",
            CAMPAIGN_NONCE,
            "--now",
            str(ISSUED_AT + 1),
        ]
        for name, value in self.subjects.items():
            arguments.extend(("--expect-subject", f"{name}={value}"))
        result = subprocess.run(
            arguments, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False
        )
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        self.assertIn(b"authenticated direct-link observation", result.stdout)

    def test_build_context_changes_for_every_bound_input(self) -> None:
        baseline = self.payload.build_context_identity
        mutations: tuple[dict[str, object], ...] = (
            {"commit": "56" * 20},
            {"target": "gfx950"},
            {"campaign_nonce": "other-campaign"},
            {"subjects": subjects_for("g2-worker", "other")},
        )
        for mutation in mutations:
            with self.subTest(mutation=tuple(mutation)):
                self.assertNotEqual(
                    self.make_payload(**mutation).build_context_identity, baseline
                )
        new_policy = self.make_policy(policy_epoch=POLICY_EPOCH + 1)
        self.assertNotEqual(
            self.make_payload(policy=new_policy).build_context_identity, baseline
        )

    def test_rejects_payload_mutation_after_signing(self) -> None:
        mutated = self.make_payload(target="gfx950")
        self.payload_path.write_bytes(mutated.canonical_bytes())
        with self.assertRaisesRegex(EvidenceError, "signature verification failed"):
            self.verify(expected_target="gfx950")

    def test_rejects_wrong_signature_namespace(self) -> None:
        self.signature_path = self.sign(
            self.payload_path, namespace="fe2o3-unrelated-attestation-v1"
        )
        with self.assertRaisesRegex(EvidenceError, "signature verification failed"):
            self.verify()

    def test_rejects_signature_from_substituted_key(self) -> None:
        self.signature_path = self.sign(self.payload_path, self.other_private_key)
        with self.assertRaisesRegex(EvidenceError, "signature verification failed"):
            self.verify()

    def test_rejects_policy_key_substitution_with_original_pin(self) -> None:
        substituted = self.make_policy(public_key=self.other_public_key)
        self.policy_path.write_bytes(substituted.canonical_bytes())
        with self.assertRaisesRegex(EvidenceError, "out-of-band pin"):
            self.verify()

    def test_rejects_policy_key_substitution_with_new_pin_and_old_payload(self) -> None:
        substituted = self.make_policy(public_key=self.other_public_key)
        self.install_policy(substituted)
        with self.assertRaisesRegex(EvidenceError, "expected trust policy"):
            self.verify(expected_policy_identity=substituted.identity())

    def test_policy_rotation_requires_new_epoch_pin_and_signature(self) -> None:
        old_policy_identity = self.policy.identity()
        rotated = self.make_policy(
            public_key=self.other_public_key,
            policy_epoch=POLICY_EPOCH + 1,
        )
        payload = self.make_payload(policy=rotated)
        self.install_policy(rotated)
        self.install_payload(payload, self.other_private_key)
        with self.assertRaisesRegex(EvidenceError, "out-of-band pin"):
            self.verify(expected_policy_identity=old_policy_identity)
        observation = self.verify(
            expected_policy_identity=rotated.identity(),
            expected_policy_epoch=POLICY_EPOCH + 1,
        )
        self.assertEqual(observation.payload.policy_epoch, POLICY_EPOCH + 1)

    def test_multiple_nonoverlapping_keys_select_key_at_issue_time(self) -> None:
        signers = (
            self.make_binding(valid_until=ISSUED_AT),
            self.make_binding(
                public_key=self.other_public_key,
                valid_from=ISSUED_AT,
            ),
        )
        policy = self.make_policy(signers=signers)
        payload = self.make_payload(policy=policy)
        self.install_policy(policy)
        self.install_payload(payload, self.other_private_key)
        observation = self.verify(expected_policy_identity=policy.identity())
        self.assertEqual(
            observation.key_identity,
            attestation.public_key_identity(self.other_public_key),
        )

    def test_rejects_overlapping_keys_for_same_role_and_signer(self) -> None:
        policy = self.make_policy(
            signers=(
                self.make_binding(valid_until=ISSUED_AT + 10),
                self.make_binding(
                    public_key=self.other_public_key,
                    valid_from=ISSUED_AT,
                ),
            )
        )
        self.policy_path.write_bytes(policy.canonical_bytes())
        with self.assertRaisesRegex(EvidenceError, "intervals overlap"):
            self.verify(expected_policy_identity=policy.identity())

    def test_same_signer_cross_role_cannot_reuse_wrong_key(self) -> None:
        g5_subjects = subjects_for("g5-publication")
        policy = self.make_policy(
            signers=(
                self.make_binding(),
                self.make_binding(
                    role="g5-publication", public_key=self.other_public_key
                ),
            )
        )
        payload = self.make_payload(
            policy=policy, role="g5-publication", subjects=g5_subjects
        )
        self.install_policy(policy)
        self.install_payload(payload, self.private_key)
        with self.assertRaisesRegex(EvidenceError, "signature verification failed"):
            self.verify(
                expected_policy_identity=policy.identity(),
                expected_role="g5-publication",
                expected_subjects=g5_subjects,
            )

    def test_rejects_expected_role_confusion(self) -> None:
        with self.assertRaisesRegex(EvidenceError, "role does not match"):
            self.verify(
                expected_role="g5-publication",
                expected_subjects=subjects_for("g5-publication"),
            )

    def test_rejects_signed_role_without_policy_binding(self) -> None:
        subjects = subjects_for("g5-publication")
        payload = self.make_payload(role="g5-publication", subjects=subjects)
        self.install_payload(payload)
        with self.assertRaisesRegex(EvidenceError, "no exact role/signer/key"):
            self.verify(expected_role="g5-publication", expected_subjects=subjects)

    def test_rejects_expected_signer_substitution(self) -> None:
        with self.assertRaisesRegex(EvidenceError, "signer does not match"):
            self.verify(expected_signer_identity="other-release-ci")

    def test_rejects_source_commit_mismatch(self) -> None:
        with self.assertRaisesRegex(EvidenceError, "source commit does not match"):
            self.verify(expected_source_commit="56" * 20)

    def test_rejects_target_mismatch(self) -> None:
        with self.assertRaisesRegex(EvidenceError, "target does not match"):
            self.verify(expected_target="gfx950")

    def test_rejects_policy_epoch_mismatch(self) -> None:
        with self.assertRaisesRegex(EvidenceError, "expected epoch"):
            self.verify(expected_policy_epoch=POLICY_EPOCH + 1)

    def test_rejects_campaign_replay_mismatch(self) -> None:
        with self.assertRaisesRegex(EvidenceError, "campaign nonce"):
            self.verify(expected_campaign_nonce="other-campaign")

    def test_rejects_forged_build_context_identity(self) -> None:
        value = self.payload.as_object()
        value["build_context_identity"] = typed_identity(
            attestation.BUILD_CONTEXT_DOMAIN, b"forged"
        )
        self.rewrite_payload_object(value)
        with self.assertRaisesRegex(EvidenceError, "canonical inputs"):
            self.verify()

    def test_rejects_subject_identity_mismatch(self) -> None:
        subjects = dict(self.subjects)
        subjects["request"] = typed_identity(
            attestation.SUBJECT_IDENTITY_DOMAINS["g2-worker"]["request"],
            b"different-request",
        )
        with self.assertRaisesRegex(EvidenceError, "subjects do not match"):
            self.verify(expected_subjects=subjects)

    def test_rejects_wrong_subject_identity_domain(self) -> None:
        subjects = dict(self.subjects)
        subjects["request"] = typed_identity("fe2o3-wrong-request-v1", b"request")
        with self.assertRaisesRegex(
            EvidenceError, "must use the fe2o3-link-request-v1"
        ):
            self.verify(expected_subjects=subjects)

    def test_rejects_missing_extra_and_oversized_expected_subjects_before_sort(
        self,
    ) -> None:
        missing = dict(self.subjects)
        missing.pop("request")
        extra = dict(self.subjects)
        extra["extra"] = typed_identity("fe2o3-extra-v1", b"extra")
        oversized: dict[object, str] = {
            object(): typed_identity("fe2o3-extra-v1", str(index).encode("ascii"))
            for index in range(attestation.MAX_SUBJECTS + 1)
        }
        for subjects, message in (
            (missing, "role schema"),
            (extra, "role schema"),
            (oversized, "cardinality"),
        ):
            with self.subTest(message=message):
                with self.assertRaisesRegex(EvidenceError, message):
                    self.verify(expected_subjects=subjects)

    def test_rejects_expired_attestation(self) -> None:
        with self.assertRaisesRegex(EvidenceError, "expired"):
            self.verify(now=EXPIRES_AT)

    def test_rejects_attestation_too_far_in_future(self) -> None:
        with self.assertRaisesRegex(EvidenceError, "future"):
            self.verify(now=ISSUED_AT - attestation.MAX_CLOCK_SKEW_SECONDS - 1)

    def test_accepts_maximum_clock_skew_boundary(self) -> None:
        self.verify(now=ISSUED_AT - attestation.MAX_CLOCK_SKEW_SECONDS)

    def test_rejects_lifetime_over_seven_days(self) -> None:
        value = self.payload.as_object()
        value["expires_at"] = (
            ISSUED_AT + attestation.MAX_ATTESTATION_LIFETIME_SECONDS + 1
        )
        self.rewrite_payload_object(value)
        with self.assertRaisesRegex(EvidenceError, "lifetime"):
            self.verify()

    def test_rejects_unknown_attestation_field(self) -> None:
        value = self.payload.as_object()
        value["load_authority"] = True
        self.rewrite_payload_object(value)
        with self.assertRaisesRegex(EvidenceError, "wrong fields"):
            self.verify()

    def test_rejects_duplicate_json_key_without_echoing_key(self) -> None:
        malicious = "do-not-echo-control-\\n"
        canonical = self.payload.canonical_bytes()
        self.payload_path.write_bytes(
            canonical.replace(b"{", f'{{"{malicious}":1,"{malicious}":2,'.encode(), 1)
        )
        with self.assertRaises(EvidenceError) as raised:
            self.verify()
        self.assertIn("duplicate object key", str(raised.exception))
        self.assertNotIn(malicious, str(raised.exception))

    def test_rejects_json_recursion_and_large_integer(self) -> None:
        recursive = b'{"x":' + b"[" * 10000 + b"0" + b"]" * 10000 + b"}\n"
        large_integer = b'{"issued_at":' + b"9" * 5000 + b"}\n"
        for data in (recursive, large_integer):
            with self.subTest(size=len(data)):
                self.payload_path.write_bytes(data)
                with self.assertRaisesRegex(EvidenceError, "not valid JSON"):
                    self.verify()

    def test_rejects_noncanonical_json_whitespace(self) -> None:
        self.payload_path.write_bytes(
            (json.dumps(self.payload.as_object(), indent=2) + "\n").encode("ascii")
        )
        with self.assertRaisesRegex(EvidenceError, "canonically encoded"):
            self.verify()

    def test_rejects_non_ascii_json(self) -> None:
        self.payload_path.write_bytes(b'{"domain":"\xc3\xa9"}\n')
        with self.assertRaisesRegex(EvidenceError, "ASCII"):
            self.verify()

    def test_rejects_boolean_schema_version(self) -> None:
        value = self.payload.as_object()
        value["schema_version"] = True
        self.rewrite_payload_object(value)
        with self.assertRaisesRegex(EvidenceError, "schema_version"):
            self.verify()

    def test_rejects_permuted_subjects(self) -> None:
        value = self.payload.as_object()
        value["subjects"] = list(reversed(value["subjects"]))  # type: ignore[arg-type]
        self.rewrite_payload_object(value)
        with self.assertRaises(EvidenceError):
            self.verify()

    def test_rejects_wrong_role_subject_schema(self) -> None:
        value = self.payload.as_object()
        value["subjects"] = value["subjects"][:-1]  # type: ignore[index]
        self.rewrite_payload_object(value)
        with self.assertRaisesRegex(EvidenceError, "role schema"):
            self.verify()

    def test_rejects_identity_version_over_six_digits(self) -> None:
        value = "fe2o3-test-v1234567-sha256-" + "0" * 64
        with self.assertRaisesRegex(EvidenceError, "canonical typed"):
            attestation.require_generic_typed_identity(value, "test")

    def test_rejects_unknown_policy_field(self) -> None:
        value = self.policy.as_object()
        value["fallback_key"] = self.other_public_key
        self.policy_path.write_bytes(attestation.canonical_json_bytes(value))
        with self.assertRaisesRegex(EvidenceError, "wrong fields"):
            self.verify()

    def test_rejects_duplicate_policy_binding(self) -> None:
        value = self.policy.as_object()
        value["signers"] = [value["signers"][0], value["signers"][0]]  # type: ignore[index]
        self.policy_path.write_bytes(attestation.canonical_json_bytes(value))
        with self.assertRaisesRegex(EvidenceError, "unique"):
            self.verify(
                expected_policy_identity=typed_identity(
                    attestation.POLICY_DOMAIN, self.policy_path.read_bytes()
                )
            )

    def test_rejects_policy_verifier_path_substitution(self) -> None:
        value = self.policy.as_object()
        value["verifier_path"] = "/tmp/ssh-keygen"
        self.policy_path.write_bytes(attestation.canonical_json_bytes(value))
        with self.assertRaisesRegex(EvidenceError, "/usr/bin/ssh-keygen"):
            self.verify(
                expected_policy_identity=typed_identity(
                    attestation.POLICY_DOMAIN, self.policy_path.read_bytes()
                )
            )

    def test_rejects_policy_verifier_identity_substitution(self) -> None:
        substituted = self.make_policy(
            verifier_identity=typed_identity(attestation.VERIFIER_DOMAIN, b"other")
        )
        payload = self.make_payload(policy=substituted)
        self.install_policy(substituted)
        self.install_payload(payload)
        with self.assertRaisesRegex(EvidenceError, "ssh-keygen identity"):
            self.verify(expected_policy_identity=substituted.identity())

    def test_rejects_malformed_public_key_and_key_identity(self) -> None:
        value = self.policy.as_object()
        value["signers"][0]["public_key"] = "ssh-ed25519 AAAA comment"  # type: ignore[index]
        self.policy_path.write_bytes(attestation.canonical_json_bytes(value))
        with self.assertRaisesRegex(EvidenceError, "only an ssh-ed25519"):
            self.verify(
                expected_policy_identity=typed_identity(
                    attestation.POLICY_DOMAIN, self.policy_path.read_bytes()
                )
            )
        value = self.policy.as_object()
        value["signers"][0]["key_identity"] = typed_identity(  # type: ignore[index]
            attestation.PUBLIC_KEY_DOMAIN, b"wrong"
        )
        self.policy_path.write_bytes(attestation.canonical_json_bytes(value))
        with self.assertRaisesRegex(EvidenceError, "does not match"):
            self.verify(
                expected_policy_identity=typed_identity(
                    attestation.POLICY_DOMAIN, self.policy_path.read_bytes()
                )
            )

    def test_rejects_malformed_signature_envelope(self) -> None:
        self.signature_path.write_bytes(b"not a signature\n")
        with self.assertRaisesRegex(EvidenceError, "signature envelope"):
            self.verify()

    def test_rejects_oversized_payload_policy_and_signature(self) -> None:
        cases = (
            (self.payload_path, attestation.MAX_ATTESTATION_BYTES),
            (self.policy_path, attestation.MAX_POLICY_BYTES),
            (self.signature_path, attestation.MAX_SIGNATURE_BYTES),
        )
        for path, maximum in cases:
            with self.subTest(path=path.name):
                original = path.read_bytes()
                path.write_bytes(b"x" * (maximum + 1))
                with self.assertRaisesRegex(EvidenceError, "exceeds"):
                    self.verify()
                path.write_bytes(original)

    def test_rejects_symlinked_payload_policy_and_signature(self) -> None:
        for argument, path in (
            ("payload_path", self.payload_path),
            ("policy_path", self.policy_path),
            ("signature_path", self.signature_path),
        ):
            with self.subTest(argument=argument):
                link = self.root / f"{argument}.link"
                link.symlink_to(path)
                with self.assertRaisesRegex(EvidenceError, "cannot open"):
                    self.verify(**{argument: link})

    def test_rejects_fifo_paths_promptly(self) -> None:
        for argument in ("payload_path", "policy_path", "signature_path"):
            with self.subTest(argument=argument):
                fifo = self.root / f"{argument}.fifo"
                os.mkfifo(fifo)
                started = time.monotonic()
                with self.assertRaisesRegex(EvidenceError, "not a regular file"):
                    self.verify(**{argument: fifo})
                self.assertLess(time.monotonic() - started, 1.0)

    def test_rejects_device_paths_promptly(self) -> None:
        for argument in ("payload_path", "policy_path", "signature_path"):
            with self.subTest(argument=argument):
                started = time.monotonic()
                with self.assertRaisesRegex(EvidenceError, "not a regular file"):
                    self.verify(**{argument: Path("/dev/null")})
                self.assertLess(time.monotonic() - started, 1.0)

    def test_rejects_socket_paths_promptly(self) -> None:
        socket_path = self.root / "attestation.socket"
        server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        try:
            server.bind(str(socket_path))
            started = time.monotonic()
            with self.assertRaisesRegex(EvidenceError, "cannot open"):
                self.verify(payload_path=socket_path)
            self.assertLess(time.monotonic() - started, 1.0)
        finally:
            server.close()

    def test_caller_and_verifier_opens_are_nonblocking_and_nofollow(self) -> None:
        real_open = os.open
        observed: list[tuple[object, int]] = []

        def recording_open(
            path: object, flags: int, *args: object, **kwargs: object
        ) -> int:
            observed.append((path, flags))
            return real_open(path, flags, *args, **kwargs)  # type: ignore[arg-type]

        with mock.patch.object(attestation.os, "open", side_effect=recording_open):
            self.verify()
        caller_paths = {
            self.policy_path,
            self.payload_path,
            self.signature_path,
            attestation.SSH_KEYGEN_PATH,
        }
        matched = [
            (Path(path), flags)
            for path, flags in observed
            if Path(path) in caller_paths
        ]
        self.assertEqual({path for path, _ in matched}, caller_paths)
        for _, flags in matched:
            self.assertTrue(flags & os.O_NONBLOCK)
            self.assertTrue(flags & os.O_NOFOLLOW)

    def test_verifier_uses_sealed_memfds_no_temp_and_sanitized_environment(
        self,
    ) -> None:
        real_popen = subprocess.Popen
        inspected: list[dict[str, object]] = []

        def inspecting_popen(
            *args: object, **kwargs: object
        ) -> subprocess.Popen[bytes]:
            pass_fds = kwargs["pass_fds"]
            assert isinstance(pass_fds, tuple)
            self.assertEqual(len(pass_fds), 4)
            seals = (
                attestation.fcntl.F_SEAL_WRITE
                | attestation.fcntl.F_SEAL_GROW
                | attestation.fcntl.F_SEAL_SHRINK
                | attestation.fcntl.F_SEAL_SEAL
            )
            for descriptor in pass_fds:
                self.assertEqual(
                    attestation.fcntl.fcntl(descriptor, attestation.fcntl.F_GET_SEALS)
                    & seals,
                    seals,
                )
            inspected.append({"argv": args[0], **kwargs})
            return real_popen(*args, **kwargs)  # type: ignore[arg-type]

        hostile_tmp = self.root / "attacker-controlled-tmp"
        with (
            mock.patch.object(
                attestation.subprocess, "Popen", side_effect=inspecting_popen
            ),
            mock.patch.dict(os.environ, {"TMPDIR": str(hostile_tmp)}),
        ):
            self.verify()
        self.assertFalse(hostile_tmp.exists())
        keyword = inspected[0]
        self.assertNotIn("shell", keyword)
        self.assertEqual(keyword["cwd"], Path("/"))
        self.assertEqual(
            keyword["env"],
            {
                "HOME": "/nonexistent-fe2o3-home",
                "LANG": "C",
                "LC_ALL": "C",
                "PATH": "/usr/bin:/bin",
                "TZ": "UTC",
            },
        )
        self.assertRegex(str(keyword["executable"]), r"^/proc/self/fd/[0-9]+$")
        argv = keyword["argv"]
        assert isinstance(argv, list)
        allowed_path = argv[argv.index("-f") + 1]
        signature_path = argv[argv.index("-s") + 1]
        self.assertRegex(allowed_path, r"^/proc/self/fd/[0-9]+$")
        self.assertRegex(signature_path, r"^/proc/self/fd/[0-9]+$")

    def run_supervised_shell(
        self, command: str, *, timeout: int, output_limit: int
    ) -> attestation._ProcessResult:
        shell_path = Path("/bin/sh").resolve(strict=True)
        shell = os.open(
            shell_path,
            os.O_RDONLY | os.O_CLOEXEC | os.O_NONBLOCK | os.O_NOFOLLOW,
        )
        stdin = os.open("/dev/null", os.O_RDONLY | os.O_CLOEXEC | os.O_NONBLOCK)
        try:
            return attestation._run_bounded_process(
                executable_descriptor=shell,
                argv=["/bin/sh", "-c", command],
                pass_descriptors=(shell,),
                stdin_descriptor=stdin,
                environment={"LANG": "C", "LC_ALL": "C", "PATH": "/usr/bin:/bin"},
                timeout=timeout,
                output_limit=output_limit,
            )
        finally:
            os.close(stdin)
            os.close(shell)

    def test_real_output_flood_is_killed_at_aggregate_bound(self) -> None:
        started = time.monotonic()
        result = self.run_supervised_shell(
            "while :; do printf 'stdout-flood'; printf 'stderr-flood' >&2; done",
            timeout=5,
            output_limit=1024,
        )
        self.assertTrue(result.overflow)
        self.assertFalse(result.timed_out)
        self.assertEqual(len(result.output), 1024)
        self.assertLess(time.monotonic() - started, 2.0)

    def test_real_descendant_process_group_is_killed_on_timeout(self) -> None:
        started = time.monotonic()
        result = self.run_supervised_shell(
            "sleep 30 & wait",
            timeout=1,
            output_limit=1024,
        )
        self.assertTrue(result.timed_out)
        self.assertLess(time.monotonic() - started, 3.0)

    def test_verifier_reports_supervisor_timeout_and_output_flood(self) -> None:
        for result, message in (
            (attestation._ProcessResult(-9, b"", timed_out=True), "timed out"),
            (attestation._ProcessResult(-9, b"x", overflow=True), "output exceeded"),
        ):
            with self.subTest(message=message):
                with mock.patch.object(
                    attestation, "_run_bounded_process", return_value=result
                ):
                    with self.assertRaisesRegex(EvidenceError, message):
                        self.verify()

    def test_rejects_invalid_timeout_bounds(self) -> None:
        for timeout in (0, attestation.MAX_VERIFIER_TIMEOUT_SECONDS + 1, True):
            with self.subTest(timeout=timeout):
                with self.assertRaisesRegex(EvidenceError, "timeout"):
                    self.verify(timeout=timeout)

    def test_all_roles_have_exact_bounded_identity_domains(self) -> None:
        self.assertEqual(set(attestation.ROLES), set(attestation.ROLE_SUBJECTS))
        self.assertEqual(
            set(attestation.ROLES), set(attestation.SUBJECT_IDENTITY_DOMAINS)
        )
        schemas = list(attestation.ROLE_SUBJECTS.values())
        self.assertEqual(len(schemas), len(set(schemas)))
        for role, schema in attestation.ROLE_SUBJECTS.items():
            self.assertEqual(schema, tuple(sorted(schema)))
            self.assertEqual(
                schema, tuple(sorted(attestation.SUBJECT_IDENTITY_DOMAINS[role]))
            )
            self.assertLessEqual(len(schema), attestation.MAX_SUBJECTS)
            for domain in attestation.SUBJECT_IDENTITY_DOMAINS[role].values():
                self.assertRegex(domain, r"-v[1-9][0-9]{0,5}$")


if __name__ == "__main__":
    unittest.main()
