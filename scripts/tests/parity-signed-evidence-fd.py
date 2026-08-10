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
            EVIDENCE.verify_signed(forged, trust, "attestor")
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
        EVIDENCE.verify_signed(authentic, trust, "attestor")


if __name__ == "__main__":
    test_verification_retains_authenticated_key_bytes()
    print("signed parity retained-key adversarial tests passed")
