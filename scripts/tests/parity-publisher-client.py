#!/usr/bin/env python3
"""Adversarial tests for protected publisher receipt acquisition."""

from __future__ import annotations

import argparse
import base64
import hashlib
import http.server
import importlib.util
import json
import os
from pathlib import Path
import shutil
import socket
import ssl
import stat
import subprocess
import sys
import tempfile
import threading
import time
from typing import Any, Callable


ROOT = Path(__file__).resolve().parents[2]
CLIENT_PATH = ROOT / "scripts/parity-publisher-client.py"
EVIDENCE_PATH = ROOT / "scripts/parity-signed-evidence.py"


def load(path: Path, name: str) -> Any:
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


CLIENT = load(CLIENT_PATH, "parity_publisher_client_tested")
EVIDENCE = load(EVIDENCE_PATH, "parity_signed_evidence_publisher_test")


class LocalHttpsHandler(http.server.BaseHTTPRequestHandler):
    def do_GET(self) -> None:
        body = b'{"ok":true}\n'
        self.send_response(200)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, _format: str, *_args: object) -> None:
        pass


class Fixture:
    def __init__(self, root: Path) -> None:
        self.root = root
        self.repo = root / "repo"
        self.archive = root / "archive"
        self.trusted = root / "trusted"
        self.runner = root / "runner"
        self.repo.mkdir()
        self.archive.mkdir()
        self.trusted.mkdir()
        self.runner.mkdir()
        self.default_tip = "3" * 40
        self.candidate_head = "4" * 40
        self.challenge = "5" * 64
        self.private_key = root / "publisher-private.pem"
        self.public_key = root / "publisher-public.pem"
        subprocess.run(
            [
                "openssl",
                "genpkey",
                "-algorithm",
                "Ed25519",
                "-out",
                self.private_key,
            ],
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        subprocess.run(
            [
                "openssl",
                "pkey",
                "-in",
                self.private_key,
                "-pubout",
                "-out",
                self.public_key,
            ],
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        key_bytes = self.public_key.read_bytes()
        key_dir = self.trusted / "keys"
        key_dir.mkdir()
        shutil.copy2(self.public_key, key_dir / "test-publisher.pem")
        self.trust_policy = self.trusted / "trust.tsv"
        self.trust_policy.write_text(
            "parity_trust_policy_schema_version\t2\n"
            "trust_domain\ttest\n"
            "metadata_path_count\t0\n"
            "key_count\t1\n"
            "key\t0000\tpublisher\ttest-publisher\tkeys/test-publisher.pem\t"
            f"{hashlib.sha256(key_bytes).hexdigest()}\ted25519\n",
            encoding="ascii",
        )
        (self.archive / "logs").mkdir()
        (self.archive / "logs/evidence.log").write_bytes(b"hardware evidence\n")
        self.manifest = self.archive / "promotion.tsv"
        self.manifest.write_text(
            "promotion_manifest_schema_version\t2\n"
            f"baseline_commit\t{'1' * 40}\n"
            f"source_commit\t{'2' * 40}\n"
            f"source_tree\t{'6' * 40}\n"
            "target\tgfx942\n"
            "hardware_lane\tmi300x-gfx942-test\n"
            "result_count\t0\n"
            f"evidence_set_sha256\t{'7' * 64}\n"
            "authorization_count\t0\n",
            encoding="ascii",
        )
        self.baseline_status = root / "baseline.tsv"
        self.candidate_status = root / "candidate.tsv"
        self.baseline_status.write_bytes(b"source_commit\t" + b"1" * 40 + b"\n")
        self.candidate_status.write_bytes(b"source_commit\t" + b"2" * 40 + b"\n")
        self.service_url = "https://publisher.example.invalid/v1/receipts"
        self.audience = "https://publisher.example.invalid/github-actions"
        self.oidc_base = "https://token.actions.githubusercontent.com/" + (
            "_services/token?api-version=2.0"
        )
        self.secret_request_token = "oidc-request-secret-never-log"
        self.x5t = base64.urlsafe_b64encode(b"fixture-thumbprint!!").rstrip(
            b"="
        ).decode("ascii")
        assert len(base64.urlsafe_b64decode(self.x5t + "=")) == 20
        self.queue_ref = "refs/heads/gh-readonly-queue/main/pr-1"
        self.environment = {
            "ACTIONS_ID_TOKEN_REQUEST_TOKEN": self.secret_request_token,
            "ACTIONS_ID_TOKEN_REQUEST_URL": self.oidc_base,
            "FE2O3_PUBLISHER_CLIENT_TEST_DOMAIN": "1",
            "FE2O3_PUBLISHER_GITHUB_ENVIRONMENT": CLIENT.OIDC_ENVIRONMENT,
            "FE2O3_PUBLISHER_OIDC_AUDIENCE": self.audience,
            "FE2O3_PUBLISHER_SERVICE_HOST": "publisher.example.invalid",
            "FE2O3_PUBLISHER_SERVICE_URL": self.service_url,
            "GITHUB_ACTOR_ID": "101",
            "GITHUB_EVENT_NAME": "merge_group",
            "GITHUB_JOB": "gate",
            "GITHUB_REF": self.queue_ref,
            "GITHUB_REPOSITORY": "powderluv/fe2o3",
            "GITHUB_REPOSITORY_ID": CLIENT.OIDC_REPOSITORY_ID,
            "FE2O3_PUBLISHER_REPOSITORY_OWNER_ID": (
                CLIENT.OIDC_REPOSITORY_OWNER_ID
            ),
            "GITHUB_RUN_ATTEMPT": "1",
            "GITHUB_RUN_ID": "303",
            "GITHUB_RUN_NUMBER": "404",
            "GITHUB_SHA": self.candidate_head,
            "GITHUB_WORKFLOW_REF": (
                "powderluv/fe2o3/.github/workflows/parity-promotion.yml@"
                f"{self.queue_ref}"
            ),
            "GITHUB_WORKFLOW_SHA": self.candidate_head,
            "GITHUB_WORKFLOW": "Protected parity promotion",
            "FE2O3_PUBLISHER_DEFAULT_BRANCH": "main",
            "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
            "RUNNER_TEMP": str(self.runner),
        }
        self.oidc_token_value = self.oidc_token()

    def args(self, output: str) -> argparse.Namespace:
        return argparse.Namespace(
            archive_root=self.archive,
            baseline_status=self.baseline_status,
            candidate_head=self.candidate_head,
            candidate_status=self.candidate_status,
            challenge_file=self.runner / f"{output}-challenge",
            default_tip=self.default_tip,
            logical_destination="docs/parity-evidence/archive",
            manifest="promotion.tsv",
            receipt_root=self.runner / f"{output}-receipt",
            repo=self.repo,
            test_domain=True,
            test_deadline_milliseconds=None,
            test_transport_fixture=self.root / f"{output}-transport.json",
            trust_policy=self.trust_policy,
            trusted_root=self.trusted,
        )

    def command(
        self, args: argparse.Namespace, *, test_domain: bool = True
    ) -> list[str]:
        command = [
            sys.executable,
            str(CLIENT_PATH),
            "--repo",
            str(args.repo),
            "--archive-root",
            str(args.archive_root),
            "--manifest",
            args.manifest,
            "--baseline-status",
            str(args.baseline_status),
            "--candidate-status",
            str(args.candidate_status),
            "--default-tip",
            args.default_tip,
            "--candidate-head",
            args.candidate_head,
            "--logical-destination",
            args.logical_destination,
            "--trusted-root",
            str(args.trusted_root),
            "--trust-policy",
            str(args.trust_policy),
            "--receipt-root",
            str(args.receipt_root),
            "--challenge-file",
            str(args.challenge_file),
            "--test-transport-fixture",
            str(args.test_transport_fixture),
        ]
        if test_domain:
            command.append("--test-domain")
        if args.test_deadline_milliseconds is not None:
            command.extend(
                [
                    "--test-deadline-milliseconds",
                    str(args.test_deadline_milliseconds),
                ]
            )
        return command

    @staticmethod
    def jwt_segment(value: object) -> str:
        return base64.urlsafe_b64encode(
            json.dumps(
                value, sort_keys=True, separators=(",", ":")
            ).encode("ascii")
        ).rstrip(b"=").decode("ascii")

    def oidc_token(
        self,
        *,
        claim_overrides: dict[str, object] | None = None,
        header_overrides: dict[str, object] | None = None,
        header_removals: tuple[str, ...] = (),
    ) -> str:
        now = int(time.time())
        header: dict[str, object] = {
            "alg": "RS256",
            "kid": "fixture-key",
            "typ": "JWT",
        }
        claims: dict[str, object] = {
            "actor_id": self.environment["GITHUB_ACTOR_ID"],
            "aud": self.audience,
            "base_ref": "",
            "check_run_id": "505",
            "event_name": "merge_group",
            "environment": CLIENT.OIDC_ENVIRONMENT,
            "exp": now + 300,
            "head_ref": "",
            "iat": now,
            "iss": "https://token.actions.githubusercontent.com",
            "job_workflow_ref": (
                "powderluv/fe2o3/.github/workflows/"
                f"parity-publisher-gate.yml@{self.queue_ref}"
            ),
            "job_workflow_sha": self.candidate_head,
            "jti": "fixture-jti-0001",
            "nbf": now,
            "ref": self.queue_ref,
            "repository": "powderluv/fe2o3",
            "repository_id": self.environment["GITHUB_REPOSITORY_ID"],
            "repository_owner": "powderluv",
            "repository_owner_id": self.environment[
                "FE2O3_PUBLISHER_REPOSITORY_OWNER_ID"
            ],
            "run_attempt": self.environment["GITHUB_RUN_ATTEMPT"],
            "run_id": self.environment["GITHUB_RUN_ID"],
            "run_number": self.environment["GITHUB_RUN_NUMBER"],
            "runner_environment": "github-hosted",
            "sha": self.candidate_head,
            "sub": "repo:powderluv/fe2o3:environment:protected-publisher",
            "workflow": "Protected parity promotion",
            "workflow_ref": self.environment["GITHUB_WORKFLOW_REF"],
            "workflow_sha": self.candidate_head,
        }
        if header_overrides:
            header.update(header_overrides)
        for name in header_removals:
            header.pop(name, None)
        if claim_overrides:
            claims.update(claim_overrides)
        return ".".join(
            (
                self.jwt_segment(header),
                self.jwt_segment(claims),
                "fixture-signature",
            )
        )

    def expected(
        self,
        args: argparse.Namespace,
        *,
        token: str | None = None,
        environment: dict[str, str] | None = None,
    ) -> tuple[bytes, dict[str, str]]:
        environment = environment or self.environment
        authorization = CLIENT.oidc_authorization(
            token or self.oidc_token_value, args, environment, self.audience
        )
        request, expected, _ = CLIENT.build_request(
            args, environment, authorization
        )
        return request, expected

    def receipt(
        self,
        args: argparse.Namespace,
        overrides: dict[str, str] | None = None,
        *,
        issued_at: int | None = None,
        expires_at: int | None = None,
    ) -> bytes:
        _, expected = self.expected(args)
        values = dict(expected)
        if overrides:
            values.update(overrides)
        now = int(time.time())
        issued_at = now if issued_at is None else issued_at
        expires_at = now + 60 if expires_at is None else expires_at
        unsigned = self.root / "receipt-unsigned.tsv"
        signed = self.root / "receipt-signed.tsv"
        if signed.exists():
            signed.unlink()
        unsigned.write_text(
            "publisher_contract_receipt_schema_version\t2\n"
            "publisher_identity\ttest-publisher\n"
            "publisher_key_role\tpublisher\n"
            "destination_contract\texternal-protected-portable-archive-v2\n"
            f"logical_destination\t{values['logical_destination']}\n"
            f"archive_sha256\t{values['archive_sha256']}\n"
            f"manifest_path\t{values['manifest_path']}\n"
            f"manifest_sha256\t{values['manifest_sha256']}\n"
            f"source_commit\t{values['source_commit']}\n"
            f"source_tree\t{values['source_tree']}\n"
            f"target\t{values['target']}\n"
            f"hardware_lane\t{values['hardware_lane']}\n"
            f"baseline_status_sha256\t{values['baseline_status_sha256']}\n"
            f"candidate_status_sha256\t{values['candidate_status_sha256']}\n"
            f"default_tip\t{values['default_tip']}\n"
            f"candidate_head\t{values['candidate_head']}\n"
            f"freshness_challenge\t{self.challenge}\n"
            f"issued_at_unix\t{issued_at}\n"
            f"expires_at_unix\t{expires_at}\n",
            encoding="ascii",
        )
        EVIDENCE.sign_payload(
            unsigned,
            signed,
            self.private_key,
            "test-publisher",
            domain="test",
            role="publisher",
            repo=None,
            test_mode=True,
        )
        return signed.read_bytes()

    @staticmethod
    def response(
        method: str,
        url: str,
        body: bytes,
        *,
        chunk_count: int = 1,
        chunk_delay_milliseconds: int = 0,
        status: int = 200,
        content_length: int | None = None,
    ) -> dict[str, object]:
        return {
            "body_base64": base64.b64encode(body).decode("ascii"),
            "chunk_count": chunk_count,
            "chunk_delay_milliseconds": chunk_delay_milliseconds,
            "headers": {
                "Content-Length": str(
                    len(body) if content_length is None else content_length
                ),
                "Content-Type": "application/json",
            },
            "method": method,
            "status": status,
            "url": url,
        }

    def write_transport(
        self,
        args: argparse.Namespace,
        *,
        receipt: bytes | None = None,
        oidc_status: int = 200,
        service_status: int = 200,
        oidc_url: str | None = None,
        oidc_token: str | None = None,
        bind_oidc_token_to_request: bool = False,
        oidc_chunk_count: int = 1,
        oidc_chunk_delay_milliseconds: int = 0,
        service_url: str | None = None,
        service_raw: bytes | None = None,
        service_length: int | None = None,
        service_schema_version: object = 1,
        service_chunk_count: int = 1,
        service_chunk_delay_milliseconds: int = 0,
        fixture_schema_version: object = 1,
    ) -> None:
        request, _ = self.expected(
            args, token=oidc_token if bind_oidc_token_to_request else None
        )
        if receipt is None:
            receipt = self.receipt(args)
        oidc_body = json.dumps(
            {"value": oidc_token or self.oidc_token_value},
            separators=(",", ":"),
        ).encode("ascii")
        if service_raw is None:
            service_raw = CLIENT.canonical_json(
                {
                    "challenge": self.challenge,
                    "publisher_receipt_base64": base64.b64encode(receipt).decode(
                        "ascii"
                    ),
                    "request_sha256": hashlib.sha256(request).hexdigest(),
                    "schema_version": service_schema_version,
                }
            )
        responses = [
            self.response(
                "GET",
                oidc_url
                or CLIENT.oidc_request_url(self.oidc_base, self.audience),
                oidc_body,
                chunk_count=oidc_chunk_count,
                chunk_delay_milliseconds=oidc_chunk_delay_milliseconds,
                status=oidc_status,
            ),
            self.response(
                "POST",
                service_url or self.service_url,
                service_raw,
                chunk_count=service_chunk_count,
                chunk_delay_milliseconds=service_chunk_delay_milliseconds,
                status=service_status,
                content_length=service_length,
            ),
        ]
        args.test_transport_fixture.write_bytes(
            CLIENT.canonical_json(
                {
                    "responses": responses,
                    "schema_version": fixture_schema_version,
                }
            )
        )

    def run(
        self,
        args: argparse.Namespace,
        *,
        environment: dict[str, str] | None = None,
        test_domain: bool = True,
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            self.command(args, test_domain=test_domain),
            env=environment or self.environment,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )


def expect_failure(
    fixture: Fixture,
    name: str,
    expected: str,
    mutate: Callable[[argparse.Namespace, dict[str, str]], None],
) -> None:
    args = fixture.args(name)
    fixture.write_transport(args)
    environment = dict(fixture.environment)
    mutate(args, environment)
    process = fixture.run(args, environment=environment)
    assert process.returncode == 2, (name, process.stdout, process.stderr)
    assert expected in process.stderr, (name, process.stderr)
    assert fixture.secret_request_token not in process.stdout + process.stderr


def token_with_duplicate_claim(token: str, claim: str, value: object) -> str:
    header_segment, claims_segment, signature_segment = token.split(".")
    claims_raw = base64.urlsafe_b64decode(
        claims_segment + "=" * ((4 - len(claims_segment) % 4) % 4)
    )
    claims = json.loads(claims_raw)
    duplicate_claim_items = []
    for key in sorted(claims):
        duplicate_claim_items.append(
            f"{json.dumps(key)}:{json.dumps(claims[key], separators=(',', ':'))}"
        )
        if key == claim:
            duplicate_claim_items.append(
                f"{json.dumps(claim)}:{json.dumps(value, separators=(',', ':'))}"
            )
    duplicate_claims_segment = base64.urlsafe_b64encode(
        ("{" + ",".join(duplicate_claim_items) + "}").encode("ascii")
    ).rstrip(b"=").decode("ascii")
    return ".".join((header_segment, duplicate_claims_segment, signature_segment))


def stale_ref_subject(fixture: Fixture) -> str:
    return f"repo:powderluv/fe2o3:ref:{fixture.queue_ref}"


def test_success_and_test_domain_guard() -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-publisher-client-") as raw_temp:
        fixture = Fixture(Path(raw_temp))
        args = fixture.args("success")
        fixture.write_transport(args)
        process = fixture.run(args)
        assert process.returncode == 0, process.stderr
        receipt = args.receipt_root / CLIENT.RECEIPT_NAME
        assert stat.S_IMODE(receipt.stat().st_mode) == 0o600
        assert stat.S_IMODE(args.challenge_file.stat().st_mode) == 0o600
        assert (
            args.challenge_file.read_text(encoding="ascii")
            == fixture.challenge + "\n"
        )
        assert fixture.secret_request_token not in process.stdout + process.stderr
        assert (
            receipt.read_text(encoding="ascii") not in process.stdout + process.stderr
        )

        process = fixture.run(args)
        assert process.returncode == 2
        assert "output already exists" in process.stderr

        guarded = fixture.args("guarded")
        fixture.write_transport(guarded)
        environment = dict(fixture.environment)
        environment.pop("FE2O3_PUBLISHER_CLIENT_TEST_DOMAIN")
        process = fixture.run(guarded, environment=environment)
        assert process.returncode == 2
        assert "explicit test-domain guard" in process.stderr

        production = fixture.args("production-hook")
        fixture.write_transport(production)
        environment = dict(fixture.environment)
        environment.pop("FE2O3_PUBLISHER_CLIENT_TEST_DOMAIN")
        process = fixture.run(production, environment=environment, test_domain=False)
        assert process.returncode == 2
        assert "test transport is forbidden" in process.stderr


def test_identity_config_and_replay_rejections() -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-publisher-identity-") as raw_temp:
        fixture = Fixture(Path(raw_temp))
        cases: tuple[
            tuple[str, str, Callable[[argparse.Namespace, dict[str, str]], None]], ...
        ] = (
            (
                "missing-config",
                "configuration is missing",
                lambda _args, env: env.pop("FE2O3_PUBLISHER_SERVICE_URL"),
            ),
            (
                "wrong-host",
                "exact allowlisted HTTPS origin",
                lambda _args, env: env.__setitem__(
                    "FE2O3_PUBLISHER_SERVICE_URL",
                    "https://attacker.example.invalid/v1/receipts",
                ),
            ),
            (
                "wrong-audience",
                "final URL mismatch",
                lambda _args, env: env.__setitem__(
                    "FE2O3_PUBLISHER_OIDC_AUDIENCE",
                    "https://publisher.example.invalid/wrong-audience",
                ),
            ),
            (
                "wrong-candidate",
                "workflow SHA does not match",
                lambda args, _env: setattr(args, "candidate_head", "9" * 40),
            ),
            (
                "wrong-default",
                "response does not match the request",
                lambda args, _env: setattr(args, "default_tip", "a" * 40),
            ),
            (
                "missing-publisher-environment",
                "configuration is missing",
                lambda _args, env: env.pop("FE2O3_PUBLISHER_GITHUB_ENVIRONMENT"),
            ),
            (
                "wrong-publisher-environment",
                "publisher environment is outside",
                lambda _args, env: env.__setitem__(
                    "FE2O3_PUBLISHER_GITHUB_ENVIRONMENT", "unprotected"
                ),
            ),
            (
                "malformed-queue-ref",
                "GitHub ref is malformed",
                lambda _args, env: env.__setitem__(
                    "GITHUB_REF", f"{fixture.queue_ref}/../evil"
                ),
            ),
            (
                "replay-run",
                "authorization matrix",
                lambda _args, env: env.__setitem__("GITHUB_RUN_ATTEMPT", "2"),
            ),
            (
                "pull-request-event",
                "production OIDC authorization matrix",
                lambda _args, env: env.__setitem__(
                    "GITHUB_EVENT_NAME", "pull_request_target"
                ),
            ),
            (
                "candidate-job",
                "production OIDC authorization matrix",
                lambda _args, env: env.__setitem__("GITHUB_JOB", "verify"),
            ),
            (
                "fork-runtime",
                "repository is outside",
                lambda _args, env: env.__setitem__(
                    "GITHUB_REPOSITORY", "attacker/fe2o3"
                ),
            ),
            (
                "wrong-runtime-owner-id",
                "repository IDs are outside",
                lambda _args, env: env.__setitem__(
                    "FE2O3_PUBLISHER_REPOSITORY_OWNER_ID", "999"
                ),
            ),
        )
        for name, expected, mutate in cases:
            expect_failure(fixture, name, expected, mutate)

        for field, value in (
            ("target", "gfx999"),
            ("hardware_lane", "wrong-lane"),
            ("logical_destination", "docs/parity-evidence/archive/substituted"),
        ):
            args = fixture.args(f"wrong-{field}")
            receipt = fixture.receipt(args, {field: value})
            fixture.write_transport(args, receipt=receipt)
            process = fixture.run(args)
            assert process.returncode == 2
            assert "receipt does not match the protected request" in process.stderr

        args = fixture.args("stale-receipt")
        now = int(time.time())
        receipt = fixture.receipt(
            args, issued_at=now - 120, expires_at=now - 60
        )
        fixture.write_transport(args, receipt=receipt)
        process = fixture.run(args)
        assert process.returncode == 2
        assert "receipt is stale" in process.stderr

        args = fixture.args("status-substitution")
        fixture.write_transport(args)
        fixture.candidate_status.write_bytes(b"substituted status\n")
        process = fixture.run(args)
        assert process.returncode == 2
        assert "response does not match the request" in process.stderr


def test_oidc_authorization_matrix() -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-publisher-oidc-") as raw_temp:
        fixture = Fixture(Path(raw_temp))
        args = fixture.args("matrix-request")
        request, _ = fixture.expected(args)
        authorization = json.loads(request)["oidc_authorization"]
        assert CLIENT.OIDC_POLICY_ID == "fe2o3-protected-local-merge-group-v3"
        assert authorization["policy_id"] == CLIENT.OIDC_POLICY_ID
        assert authorization["schema_version"] == 1
        request_payload = json.loads(request)
        assert request == CLIENT.canonical_json(request_payload)
        assert request_payload["request_domain"] == CLIENT.REQUEST_DOMAIN
        assert request_payload["schema_version"] == CLIENT.REQUEST_SCHEMA_VERSION
        assert authorization["environment"] == CLIENT.OIDC_ENVIRONMENT
        assert authorization["event_name"] == "merge_group"
        assert authorization["job"] == "gate"
        assert "alg" not in authorization
        assert "kid" not in authorization
        assert "jti" not in authorization
        assert "iat" not in authorization
        assert "nbf" not in authorization
        assert "exp" not in authorization
        assert "check_run_id" not in authorization
        assert "x5t" not in authorization
        assert authorization["ref"] == fixture.queue_ref
        assert authorization["sub"] == (
            "repo:powderluv/fe2o3:environment:protected-publisher"
        )
        assert stale_ref_subject(fixture) != authorization["sub"]
        assert authorization["sha"] == fixture.candidate_head
        assert authorization["job_workflow_ref"] == (
            "powderluv/fe2o3/.github/workflows/"
            f"parity-publisher-gate.yml@{fixture.queue_ref}"
        )
        assert authorization["job_workflow_sha"] == fixture.candidate_head
        assert authorization["workflow_ref"] == (
            "powderluv/fe2o3/.github/workflows/"
            f"parity-promotion.yml@{fixture.queue_ref}"
        )
        assert authorization["workflow_sha"] == fixture.candidate_head

        assert CLIENT.oidc_environment_subject(
            "powderluv/fe2o3", "protected-publisher"
        ) == "repo:powderluv/fe2o3:environment:protected-publisher"
        assert CLIENT.oidc_environment_subject(
            "powderluv/fe2o3", "protected:publisher"
        ) == "repo:powderluv/fe2o3:environment:protected%3Apublisher"

        x5t_args = fixture.args("documented-x5t-header")
        x5t_token = fixture.oidc_token(header_overrides={"x5t": fixture.x5t})
        x5t_request, _ = fixture.expected(x5t_args, token=x5t_token)
        x5t_authorization = json.loads(x5t_request)["oidc_authorization"]
        assert "x5t" not in x5t_authorization
        fixture.write_transport(
            x5t_args,
            oidc_token=x5t_token,
            bind_oidc_token_to_request=True,
        )
        process = fixture.run(x5t_args)
        assert process.returncode == 0, process.stderr

        fresh_token = fixture.oidc_token(
            claim_overrides={"jti": "fresh-jti-for-stable-request"}
        )
        fresh_request, _ = fixture.expected(args, token=fresh_token)
        assert fresh_token != fixture.oidc_token_value
        assert fresh_request == request

        short_lived = fixture.oidc_token(
            claim_overrides={"exp": int(time.time()) + 39}
        )
        try:
            CLIENT.oidc_authorization(
                short_lived, args, fixture.environment, fixture.audience
            )
        except CLIENT.ClientError as error:
            assert "freshness" in str(error)
        else:
            raise AssertionError("short-lived token was accepted")

        claim_cases: tuple[tuple[str, object], ...] = (
            ("iss", "https://issuer.example.invalid"),
            ("aud", "https://wrong-audience.example.invalid"),
            ("repository", "attacker/fe2o3"),
            ("repository_id", "999"),
            ("repository_owner_id", "999"),
            ("workflow_ref", "attacker/workflow@refs/heads/main"),
            ("workflow_sha", "8" * 40),
            ("job_workflow_ref", "attacker/reusable@refs/heads/main"),
            ("job_workflow_sha", "8" * 40),
            (
                "workflow_ref",
                "powderluv/fe2o3/.github/workflows/"
                "parity-promotion.yml@refs/heads/main",
            ),
            ("workflow_sha", fixture.default_tip),
            (
                "job_workflow_ref",
                "powderluv/fe2o3/.github/workflows/"
                "parity-publisher-gate.yml@refs/heads/main",
            ),
            ("job_workflow_sha", fixture.default_tip),
            ("event_name", "pull_request_target"),
            ("environment", "unprotected"),
            ("ref", "refs/heads/main"),
            ("base_ref", "refs/heads/main"),
            ("check_run_id", "not-numeric"),
            ("head_ref", "refs/heads/feature"),
            ("sub", stale_ref_subject(fixture)),
            ("sub", "repo:powderluv/fe2o3:pull_request"),
            ("sub", "repo:powderluv/fe2o3:environment:unprotected"),
            (
                "sub",
                "repo:powderluv@74956/fe2o3@1233498266:"
                "environment:protected-publisher",
            ),
            (
                "sub",
                "repository_id:1233498266:repository_owner_id:74956:"
                "environment:protected-publisher",
            ),
            (
                "sub",
                "repo:powderluv/fe2o3:repository_id:1233498266:"
                "environment:protected-publisher",
            ),
            (
                "sub",
                "repo:powderluv/fe2o3-renamed:environment:protected-publisher",
            ),
            ("sub", "repo:attacker/fe2o3:ref:refs/heads/main"),
            ("sub", True),
            ("environment", True),
            ("ref", True),
            ("runner_environment", "self-hosted"),
            ("jti", ""),
            ("iat", True),
        )
        for index, (claim, value) in enumerate(claim_cases):
            case_args = fixture.args(f"wrong-claim-{index:02d}-{claim}")
            token = fixture.oidc_token(claim_overrides={claim: value})
            fixture.write_transport(case_args, oidc_token=token)
            process = fixture.run(case_args)
            assert process.returncode == 2, (claim, process.stderr)
            assert "OIDC" in process.stderr, (claim, process.stderr)

        header_cases: tuple[
            tuple[str, dict[str, object], tuple[str, ...], str], ...
        ] = (
            ("wrong-algorithm", {"alg": "HS256"}, (), "header is outside"),
            ("wrong-type", {"typ": "JOSE"}, (), "header is outside"),
            ("non-string-kid", {"kid": True}, (), "header is outside"),
            ("unknown-x5c", {"x5c": []}, (), "non-canonical member set"),
            ("unknown-jwk", {"jwk": {}}, (), "non-canonical member set"),
            (
                "unknown-x5t-s256",
                {"x5t#S256": fixture.x5t},
                (),
                "non-canonical member set",
            ),
            ("missing-typ", {}, ("typ",), "non-canonical member set"),
            ("x5t-null", {"x5t": None}, (), "x5t header is malformed"),
            ("x5t-boolean", {"x5t": True}, (), "x5t header is malformed"),
            ("x5t-short", {"x5t": "A" * 26}, (), "x5t header is malformed"),
            ("x5t-long", {"x5t": "A" * 28}, (), "x5t header is malformed"),
            (
                "x5t-padding",
                {"x5t": fixture.x5t + "="},
                (),
                "x5t header is malformed",
            ),
            (
                "x5t-noncanonical-tail",
                {"x5t": "A" * 26 + "B"},
                (),
                "x5t header is malformed",
            ),
        )
        for name, overrides, removals, expected_error in header_cases:
            case_args = fixture.args(name)
            token = fixture.oidc_token(
                header_overrides=overrides, header_removals=removals
            )
            fixture.write_transport(case_args, oidc_token=token)
            process = fixture.run(case_args)
            assert process.returncode == 2, (name, process.stderr)
            assert expected_error in process.stderr, (name, process.stderr)

        duplicate_args = fixture.args("duplicate-kid-header")
        claims_segment = fixture.oidc_token_value.split(".")[1]
        duplicate_header = base64.urlsafe_b64encode(
            b'{"alg":"RS256","kid":"first","kid":"second","typ":"JWT"}'
        ).rstrip(b"=").decode("ascii")
        duplicate_token = ".".join(
            (duplicate_header, claims_segment, "fixture-signature")
        )
        fixture.write_transport(duplicate_args, oidc_token=duplicate_token)
        process = fixture.run(duplicate_args)
        assert process.returncode == 2
        assert "duplicate JSON member" in process.stderr

        for claim in ("environment", "ref", "sub"):
            duplicate_claim_args = fixture.args(f"duplicate-{claim}-claim")
            duplicate_claim_token = token_with_duplicate_claim(
                fixture.oidc_token_value, claim, "duplicate"
            )
            fixture.write_transport(
                duplicate_claim_args, oidc_token=duplicate_claim_token
            )
            process = fixture.run(duplicate_claim_args)
            assert process.returncode == 2
            assert "duplicate JSON member" in process.stderr

        args = fixture.args("archive-substitution")
        fixture.candidate_status.write_bytes(b"source_commit\t" + b"2" * 40 + b"\n")
        fixture.write_transport(args)
        (fixture.archive / "logs/evidence.log").write_bytes(b"substituted archive\n")
        process = fixture.run(args)
        assert process.returncode == 2
        assert "response does not match the request" in process.stderr


def test_environment_subject_regression() -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-publisher-env-sub-") as raw_temp:
        fixture = Fixture(Path(raw_temp))
        args = fixture.args("environment-subject")
        token = fixture.oidc_token(
            claim_overrides={
                "sub": "repo:powderluv/fe2o3:environment:protected-publisher"
            }
        )
        authorization = CLIENT.oidc_authorization(
            token, args, fixture.environment, fixture.audience
        )
        assert authorization["sub"] == (
            "repo:powderluv/fe2o3:environment:protected-publisher"
        )
        assert authorization["ref"] == fixture.queue_ref

        stale_token = fixture.oidc_token(
            claim_overrides={"sub": stale_ref_subject(fixture)}
        )
        try:
            CLIENT.oidc_authorization(
                stale_token, args, fixture.environment, fixture.audience
            )
        except CLIENT.ClientError as error:
            assert "authorization matrix: sub" in str(error)
        else:
            raise AssertionError("stale ref-based OIDC subject was accepted")


def test_transport_failures_bounds_and_redaction() -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-publisher-transport-") as raw_temp:
        fixture = Fixture(Path(raw_temp))

        args = fixture.args("oidc-failure")
        fixture.write_transport(args, oidc_status=500)
        process = fixture.run(args)
        assert process.returncode == 2 and "OIDC token request failed" in process.stderr

        args = fixture.args("service-failure")
        fixture.write_transport(args, service_status=503)
        process = fixture.run(args)
        assert process.returncode == 2
        assert "publisher service request failed" in process.stderr

        for name, oidc_url, service_url in (
            ("oidc-redirect", "https://redirect.example.invalid/token", None),
            ("service-redirect", None, "https://redirect.example.invalid/receipt"),
        ):
            args = fixture.args(name)
            fixture.write_transport(
                args,
                oidc_status=302 if oidc_url else 200,
                service_status=302 if service_url else 200,
                oidc_url=oidc_url,
                service_url=service_url,
            )
            process = fixture.run(args)
            assert process.returncode == 2
            assert "redirects are forbidden" in process.stderr

        args = fixture.args("oversized")
        fixture.write_transport(
            args, service_raw=b"x" * (CLIENT.MAX_SERVICE_RESPONSE_BYTES + 1)
        )
        process = fixture.run(args)
        assert process.returncode == 2 and "response-size bound" in process.stderr

        args = fixture.args("truncated")
        fixture.write_transport(args, service_length=1)
        process = fixture.run(args)
        assert process.returncode == 2 and "response is truncated" in process.stderr

        args = fixture.args("duplicate")
        duplicate = (
            b'{"challenge":"a","challenge":"b","publisher_receipt_base64":"x",'
            b'"request_sha256":"c","schema_version":1}\n'
        )
        fixture.write_transport(args, service_raw=duplicate)
        process = fixture.run(args)
        assert process.returncode == 2 and "duplicate JSON member" in process.stderr

        args = fixture.args("boolean-fixture-schema")
        fixture.write_transport(args, fixture_schema_version=True)
        process = fixture.run(args)
        assert process.returncode == 2
        assert "fixture schema is invalid" in process.stderr

        args = fixture.args("boolean-service-schema")
        fixture.write_transport(args, service_schema_version=True)
        process = fixture.run(args)
        assert process.returncode == 2
        assert "response does not match the request" in process.stderr

        args = fixture.args("noncanonical-service-json")
        request, _ = fixture.expected(args)
        receipt = fixture.receipt(args)
        noncanonical_service = json.dumps(
            {
                "challenge": fixture.challenge,
                "publisher_receipt_base64": base64.b64encode(receipt).decode("ascii"),
                "request_sha256": hashlib.sha256(request).hexdigest(),
                "schema_version": 1,
            },
            indent=2,
            sort_keys=True,
        ).encode("ascii")
        fixture.write_transport(args, service_raw=noncanonical_service)
        process = fixture.run(args)
        assert process.returncode == 2
        assert "not canonical JSON" in process.stderr

        args = fixture.args("unused-duplicate-response")
        fixture.write_transport(args)
        transport = json.loads(args.test_transport_fixture.read_text("ascii"))
        transport["responses"].append(transport["responses"][-1])
        args.test_transport_fixture.write_bytes(CLIENT.canonical_json(transport))
        process = fixture.run(args)
        assert process.returncode == 2
        assert "unused duplicate responses" in process.stderr

        args = fixture.args("parser-value-error")
        fixture.write_transport(args)
        transport = json.loads(args.test_transport_fixture.read_text("ascii"))
        transport["responses"][0]["headers"]["Content-Length"] = "9" * 5000
        args.test_transport_fixture.write_bytes(CLIENT.canonical_json(transport))
        process = fixture.run(args)
        assert process.returncode == 2
        assert "protected acquisition failed" in process.stderr
        assert "Traceback" not in process.stderr

        args = fixture.args("request-token-newline")
        fixture.write_transport(args)
        environment = dict(fixture.environment)
        sentinel = "OIDC-NEWLINE-SENTINEL-NEVER-LOG"
        environment["ACTIONS_ID_TOKEN_REQUEST_TOKEN"] += f"\n{sentinel}"
        process = fixture.run(args, environment=environment)
        output = process.stdout + process.stderr
        assert process.returncode == 2
        assert "request token is malformed" in process.stderr
        assert sentinel not in output

        args = fixture.args("slow-drip")
        args.test_deadline_milliseconds = 100
        fixture.write_transport(
            args,
            oidc_chunk_count=2,
            oidc_chunk_delay_milliseconds=30,
            service_chunk_count=4,
            service_chunk_delay_milliseconds=30,
        )
        started = time.monotonic()
        process = fixture.run(args)
        elapsed = time.monotonic() - started
        assert process.returncode == 2
        assert "acquisition deadline exceeded" in process.stderr
        assert elapsed < 2, elapsed

        args = fixture.args("redaction")
        secret_receipt = b"receipt-secret-never-log"
        fixture.write_transport(args, service_raw=secret_receipt)
        process = fixture.run(args)
        output = process.stdout + process.stderr
        assert process.returncode == 2
        assert fixture.secret_request_token not in output
        assert secret_receipt.decode("ascii") not in output


def expect_network_failure(name: str, url: str) -> None:
    secret = f"{name}-secret-never-log"
    try:
        CLIENT.bounded_request(
            CLIENT.NetworkTransport(),
            "GET",
            url,
            {"Authorization": f"Bearer {secret}"},
            None,
            128,
            name,
            time.monotonic() + 1.5,
        )
    except CLIENT.ClientError as error:
        message = str(error)
        assert "failed" in message or "deadline exceeded" in message, message
        assert secret not in message
        return
    raise AssertionError(f"network failure unexpectedly succeeded: {name}")


def unused_local_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def test_network_transport_dns_connect_tls_failures() -> None:
    expect_network_failure(
        "publisher DNS request",
        "https://publisher-dns-failure.fe2o3.invalid/receipt",
    )
    expect_network_failure(
        "publisher connect request",
        f"https://127.0.0.1:{unused_local_port()}/receipt",
    )

    with tempfile.TemporaryDirectory(prefix="fe2o3-publisher-tls-") as raw_temp:
        root = Path(raw_temp)
        key = root / "key.pem"
        cert = root / "cert.pem"
        subprocess.run(
            [
                "openssl",
                "req",
                "-x509",
                "-newkey",
                "rsa:2048",
                "-nodes",
                "-days",
                "1",
                "-subj",
                "/CN=localhost",
                "-keyout",
                key,
                "-out",
                cert,
            ],
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        server = http.server.HTTPServer(("127.0.0.1", 0), LocalHttpsHandler)
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(certfile=cert, keyfile=key)
        server.socket = context.wrap_socket(server.socket, server_side=True)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            port = int(server.server_address[1])
            expect_network_failure(
                "publisher TLS request", f"https://127.0.0.1:{port}/"
            )
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=2)


def test_stable_idempotency_key_is_high_entropy_and_descriptor_checked() -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-publisher-request-key-") as raw_temp:
        runner = Path(raw_temp)
        first_digest = "a" * 64
        second_digest = "b" * 64
        first = CLIENT.stable_idempotency_key(runner, first_digest)
        assert len(first) == 64
        assert CLIENT.SHA256_RE.fullmatch(first)
        assert CLIENT.stable_idempotency_key(runner, first_digest) == first
        second = CLIENT.stable_idempotency_key(runner, second_digest)
        assert second != first

        path = runner / f".fe2o3-publisher-idempotency-{first_digest}"
        metadata = path.stat()
        assert stat.S_IMODE(metadata.st_mode) == 0o600
        assert metadata.st_nlink == 1
        os.link(path, runner / "hardlink")
        try:
            CLIENT.stable_idempotency_key(runner, first_digest)
        except CLIENT.ClientError as error:
            assert "metadata" in str(error)
        else:
            raise AssertionError("hard-linked idempotency key was accepted")

        symlink_digest = "c" * 64
        os.symlink(path, runner / f".fe2o3-publisher-idempotency-{symlink_digest}")
        try:
            CLIENT.stable_idempotency_key(runner, symlink_digest)
        except CLIENT.ClientError as error:
            assert "unavailable" in str(error)
        else:
            raise AssertionError("symlink idempotency key was accepted")


if __name__ == "__main__":
    test_success_and_test_domain_guard()
    test_identity_config_and_replay_rejections()
    test_oidc_authorization_matrix()
    test_environment_subject_regression()
    test_transport_failures_bounds_and_redaction()
    test_network_transport_dns_connect_tls_failures()
    test_stable_idempotency_key_is_high_entropy_and_descriptor_checked()
    print("protected publisher client adversarial tests passed")
