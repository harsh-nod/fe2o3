#!/usr/bin/env python3

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
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
SIGNER = "g2-release-ci"
BUILD = typed_identity("fe2o3-test-build-v1", b"build-42")


def identity(name: str) -> str:
    domain = f"fe2o3-test-{name.replace('_', '-')}-v1"
    return typed_identity(domain, name.encode("ascii"))


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
        self.subjects = {
            name: identity(name) for name in attestation.ROLE_SUBJECTS["g2-worker"]
        }
        self.payload = self.make_payload()
        self.payload_path = self.root / "attestation.json"
        self.payload_path.write_bytes(self.payload.canonical_bytes())
        self.signature_path = self.sign(self.payload_path)
        self.policy = self.make_policy()
        self.policy_path = self.root / "policy.json"
        self.policy_path.write_bytes(self.policy.canonical_bytes())

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def make_payload(
        self,
        *,
        role: str = "g2-worker",
        signer: str = SIGNER,
        commit: str = COMMIT,
        target: str = TARGET,
        issued_at: int = ISSUED_AT,
        expires_at: int = EXPIRES_AT,
        build_identity: str = BUILD,
        subjects: dict[str, str] | None = None,
    ) -> attestation.AttestationPayloadV1:
        selected = self.subjects if subjects is None else subjects
        return attestation.AttestationPayloadV1(
            role=role,
            signer_identity=signer,
            source_commit=commit,
            target=target,
            issued_at=issued_at,
            expires_at=expires_at,
            build_identity=build_identity,
            subjects=tuple(
                attestation.SubjectIdentity(name, selected[name])
                for name in sorted(selected)
            ),
        )

    def make_policy(
        self,
        *,
        public_key: str | None = None,
        role: str = "g2-worker",
        signer: str = SIGNER,
        verifier_identity: str | None = None,
    ) -> attestation.TrustPolicyV1:
        return attestation.TrustPolicyV1(
            verifier_identity=verifier_identity or self.verifier_identity,
            signers=(
                attestation.SignerBindingV1(
                    role, signer, public_key or self.public_key
                ),
            ),
        )

    def sign(self, payload: Path, key: Path | None = None) -> Path:
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
                attestation.SIGNATURE_NAMESPACE,
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

    def verify(self, **overrides: object) -> attestation.AuthenticatedObservationV1:
        arguments: dict[str, object] = {
            "policy_path": self.policy_path,
            "expected_policy_identity": self.policy.identity(),
            "payload_path": self.payload_path,
            "signature_path": self.signature_path,
            "expected_role": "g2-worker",
            "expected_signer_identity": SIGNER,
            "expected_source_commit": COMMIT,
            "expected_target": TARGET,
            "expected_build_identity": BUILD,
            "expected_subjects": self.subjects,
            "now": ISSUED_AT + 1,
            "timeout": 5,
        }
        arguments.update(overrides)
        return attestation.verify_signed_attestation(**arguments)  # type: ignore[arg-type]

    def rewrite_payload_object(self, value: dict[str, object]) -> None:
        self.payload_path.write_bytes(attestation.canonical_json_bytes(value))

    def test_verifies_real_ed25519_signature_with_pinned_ssh_keygen(self) -> None:
        observation = self.verify()
        self.assertEqual(observation.payload, self.payload)
        self.assertEqual(observation.policy_identity, self.policy.identity())
        self.assertEqual(observation.verifier_identity, self.verifier_identity)
        self.assertEqual(observation.attestation_identity, self.payload.identity())
        self.assertFalse(hasattr(observation, "load_authority"))
        self.assertFalse(hasattr(observation, "launch_authority"))

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
            "--expect-build-identity",
            BUILD,
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

    def test_rejects_payload_mutation_after_signing(self) -> None:
        value = self.payload.as_object()
        value["target"] = "gfx950"
        self.rewrite_payload_object(value)
        with self.assertRaisesRegex(EvidenceError, "signature verification failed"):
            self.verify(expected_target="gfx950")

    def test_rejects_signature_from_substituted_key(self) -> None:
        self.signature_path = self.sign(self.payload_path, self.other_private_key)
        with self.assertRaisesRegex(EvidenceError, "signature verification failed"):
            self.verify()

    def test_rejects_policy_key_substitution_with_original_pin(self) -> None:
        substituted = self.make_policy(public_key=self.other_public_key)
        self.policy_path.write_bytes(substituted.canonical_bytes())
        with self.assertRaisesRegex(EvidenceError, "out-of-band pin"):
            self.verify()

    def test_rejects_policy_key_substitution_even_with_new_policy_pin(self) -> None:
        substituted = self.make_policy(public_key=self.other_public_key)
        self.policy_path.write_bytes(substituted.canonical_bytes())
        with self.assertRaisesRegex(EvidenceError, "signature verification failed"):
            self.verify(expected_policy_identity=substituted.identity())

    def test_rejects_expected_role_confusion(self) -> None:
        with self.assertRaisesRegex(EvidenceError, "role does not match"):
            self.verify(
                expected_role="g5-publication",
                expected_subjects={
                    name: identity(name)
                    for name in attestation.ROLE_SUBJECTS["g5-publication"]
                },
            )

    def test_rejects_signed_role_without_policy_binding(self) -> None:
        subjects = {
            name: identity(name) for name in attestation.ROLE_SUBJECTS["g5-publication"]
        }
        payload = self.make_payload(role="g5-publication", subjects=subjects)
        self.payload_path.write_bytes(payload.canonical_bytes())
        self.signature_path = self.sign(self.payload_path)
        with self.assertRaisesRegex(EvidenceError, "no exact role/signer binding"):
            self.verify(
                expected_role="g5-publication",
                expected_subjects=subjects,
            )

    def test_rejects_expected_signer_substitution(self) -> None:
        with self.assertRaisesRegex(EvidenceError, "signer does not match"):
            self.verify(expected_signer_identity="other-release-ci")

    def test_rejects_source_commit_mismatch(self) -> None:
        with self.assertRaisesRegex(EvidenceError, "source commit does not match"):
            self.verify(expected_source_commit="56" * 20)

    def test_rejects_target_mismatch(self) -> None:
        with self.assertRaisesRegex(EvidenceError, "target does not match"):
            self.verify(expected_target="gfx950")

    def test_rejects_build_replay_mismatch(self) -> None:
        with self.assertRaisesRegex(EvidenceError, "replay bound"):
            self.verify(
                expected_build_identity=typed_identity(
                    "fe2o3-test-build-v1", b"different-build"
                )
            )

    def test_rejects_subject_identity_mismatch(self) -> None:
        subjects = dict(self.subjects)
        subjects["request"] = identity("different_request")
        with self.assertRaisesRegex(EvidenceError, "subjects do not match"):
            self.verify(expected_subjects=subjects)

    def test_rejects_missing_expected_subject(self) -> None:
        subjects = dict(self.subjects)
        subjects.pop("request")
        with self.assertRaisesRegex(EvidenceError, "role schema"):
            self.verify(expected_subjects=subjects)

    def test_rejects_extra_expected_subject(self) -> None:
        subjects = dict(self.subjects)
        subjects["extra"] = identity("extra")
        with self.assertRaisesRegex(EvidenceError, "role schema"):
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

    def test_rejects_duplicate_json_key(self) -> None:
        canonical = self.payload.canonical_bytes()
        self.payload_path.write_bytes(
            canonical.replace(b"{", b'{"domain":"substituted",', 1)
        )
        with self.assertRaisesRegex(EvidenceError, "duplicate key"):
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
        with self.assertRaisesRegex(EvidenceError, "canonical order"):
            self.verify()

    def test_rejects_wrong_role_subject_schema(self) -> None:
        value = self.payload.as_object()
        value["subjects"] = value["subjects"][:-1]  # type: ignore[index]
        self.rewrite_payload_object(value)
        with self.assertRaisesRegex(EvidenceError, "role schema"):
            self.verify()

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
        self.policy_path.write_bytes(substituted.canonical_bytes())
        with self.assertRaisesRegex(EvidenceError, "ssh-keygen identity"):
            self.verify(expected_policy_identity=substituted.identity())

    def test_rejects_malformed_public_key(self) -> None:
        value = self.policy.as_object()
        value["signers"][0]["public_key"] = "ssh-ed25519 AAAA comment"  # type: ignore[index]
        self.policy_path.write_bytes(attestation.canonical_json_bytes(value))
        with self.assertRaisesRegex(EvidenceError, "only an ssh-ed25519"):
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
                with self.assertRaisesRegex(EvidenceError, "cannot open regular file"):
                    self.verify(**{argument: link})

    def test_rejects_verifier_timeout(self) -> None:
        class TimedOutProcess:
            pid = 999999999
            returncode: int | None = None
            communicate_calls = 0

            def communicate(
                self, _input: bytes | None = None, timeout: int | None = None
            ) -> tuple[bytes, None]:
                self.communicate_calls += 1
                if self.communicate_calls == 1:
                    raise subprocess.TimeoutExpired("ssh-keygen", timeout)
                self.returncode = -9
                return b"", None

            def poll(self) -> int | None:
                return self.returncode

            def wait(self, timeout: int | None = None) -> int:
                del timeout
                self.returncode = -9
                return self.returncode

        with (
            mock.patch.object(
                attestation.subprocess, "Popen", return_value=TimedOutProcess()
            ),
            mock.patch.object(attestation, "_terminate_process_group"),
        ):
            with self.assertRaisesRegex(EvidenceError, "timed out"):
                self.verify()

    def test_verifier_uses_no_shell_and_sanitized_environment(self) -> None:
        real_popen = subprocess.Popen
        with mock.patch.object(
            attestation.subprocess, "Popen", wraps=real_popen
        ) as popen:
            self.verify()
        keyword = popen.call_args.kwargs
        self.assertNotIn("shell", keyword)
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
        self.assertRegex(keyword["executable"], r"^/proc/self/fd/[0-9]+$")
        self.assertEqual(len(keyword["pass_fds"]), 1)

    def test_rejects_invalid_timeout_bounds(self) -> None:
        for timeout in (0, attestation.MAX_VERIFIER_TIMEOUT_SECONDS + 1, True):
            with self.subTest(timeout=timeout):
                with self.assertRaisesRegex(EvidenceError, "timeout"):
                    self.verify(timeout=timeout)

    def test_all_roles_have_distinct_exact_subject_schemas(self) -> None:
        self.assertEqual(set(attestation.ROLES), set(attestation.ROLE_SUBJECTS))
        schemas = list(attestation.ROLE_SUBJECTS.values())
        self.assertEqual(len(schemas), len(set(schemas)))
        for schema in schemas:
            self.assertEqual(schema, tuple(sorted(schema)))
            self.assertLessEqual(len(schema), attestation.MAX_SUBJECTS)


if __name__ == "__main__":
    unittest.main()
