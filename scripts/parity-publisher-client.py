#!/usr/bin/env python3
"""Acquire a detached parity publisher receipt through GitHub Actions OIDC."""

from __future__ import annotations

import argparse
import base64
import binascii
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import re
import secrets
import signal
import ssl
import stat
import sys
import tempfile
import time
from typing import Any
import urllib.error
import urllib.parse
import urllib.request


REQUEST_SCHEMA_VERSION = 1
REQUEST_DOMAIN = "fe2o3-protected-publisher-request-v1"
OIDC_HOST = "token.actions.githubusercontent.com"
RECEIPT_NAME = "publisher-receipt-v2.tsv"
MAX_OIDC_RESPONSE_BYTES = 64 * 1024
MAX_SERVICE_RESPONSE_BYTES = 512 * 1024
MAX_RECEIPT_BYTES = 256 * 1024
MAX_REQUEST_BYTES = 64 * 1024
NETWORK_TIMEOUT_SECONDS = 10
MAX_TEST_DEADLINE_MILLISECONDS = 1000
MAX_BEARER_TOKEN_BYTES = 16 * 1024
MAX_OIDC_TOKEN_LIFETIME = 10 * 60
OIDC_CLOCK_SKEW = 5 * 60
TOKEN_RECOVERY_GRACE_SECONDS = 30
OIDC_ISSUER = "https://token.actions.githubusercontent.com"
OIDC_ALGORITHM = "RS256"
OIDC_POLICY_ID = "fe2o3-protected-local-merge-group-v3"
OIDC_AUTHORIZATION_SCHEMA_VERSION = 1
OIDC_EVENT = "merge_group"
OIDC_JOB = "gate"
OIDC_RUNNER_ENVIRONMENT = "github-hosted"
OIDC_DEFAULT_BRANCH = "main"
OIDC_REPOSITORY = "powderluv/fe2o3"
OIDC_REPOSITORY_ID = "1233498266"
OIDC_REPOSITORY_OWNER_ID = "74956"
OIDC_ENVIRONMENT = "protected-publisher"
CALLER_WORKFLOW_PATH = ".github/workflows/parity-promotion.yml"
PROTECTED_WORKFLOW_PATH = ".github/workflows/parity-publisher-gate.yml"
MAX_PUBLISHER_RECEIPT_LIFETIME = 24 * 60 * 60
PUBLISHER_RECEIPT_CLOCK_SKEW = 5 * 60
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
ID_RE = re.compile(r"^[a-z][a-z0-9._-]{0,63}$")
JWT_RE = re.compile(r"^[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+$")
BEARER_RE = re.compile(r"^[A-Za-z0-9._~-]+$")
HOST_RE = re.compile(
    r"^(?=.{1,253}$)(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+"
    r"[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$"
)
SAFE_TEXT_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._/@:+-]{0,511}$")
X5T_RE = re.compile(r"^[A-Za-z0-9_-]{27}$")
REF_FORBIDDEN_RE = re.compile(r"[\000-\037\177 ~^:?*\\[]")
OIDC_HEADER_KEY_SETS = (
    frozenset(("alg", "kid", "typ")),
    frozenset(("alg", "kid", "typ", "x5t")),
)
RECEIPT_FIELDS = (
    "publisher_contract_receipt_schema_version",
    "publisher_identity",
    "publisher_key_role",
    "destination_contract",
    "logical_destination",
    "archive_sha256",
    "manifest_path",
    "manifest_sha256",
    "source_commit",
    "source_tree",
    "target",
    "hardware_lane",
    "baseline_status_sha256",
    "candidate_status_sha256",
    "default_tip",
    "candidate_head",
    "freshness_challenge",
    "issued_at_unix",
    "expires_at_unix",
    "signature_schema_version",
    "signature_domain",
    "signature_role",
    "signature_algorithm",
    "signing_key_id",
    "signature_base64",
)
WORKFLOW_ENV = (
    "GITHUB_REPOSITORY",
    "GITHUB_REPOSITORY_ID",
    "FE2O3_PUBLISHER_REPOSITORY_OWNER_ID",
    "GITHUB_RUN_ID",
    "GITHUB_RUN_ATTEMPT",
    "GITHUB_RUN_NUMBER",
    "GITHUB_WORKFLOW_REF",
    "GITHUB_WORKFLOW_SHA",
    "GITHUB_WORKFLOW",
    "GITHUB_JOB",
    "GITHUB_EVENT_NAME",
    "GITHUB_REF",
    "GITHUB_SHA",
    "GITHUB_ACTOR_ID",
    "FE2O3_PUBLISHER_GITHUB_ENVIRONMENT",
    "FE2O3_PUBLISHER_DEFAULT_BRANCH",
)


class ClientError(Exception):
    pass


class DeadlineExpired(Exception):
    pass


def fail(message: str) -> None:
    raise ClientError(message)


def canonical_json(value: object) -> bytes:
    return (
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
        + "\n"
    ).encode("ascii")


def json_no_duplicates(raw: bytes, label: str) -> Any:
    def object_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        output: dict[str, Any] = {}
        for key, value in pairs:
            if key in output:
                fail(f"{label} contains a duplicate JSON member")
            output[key] = value
        return output

    try:
        return json.loads(raw.decode("utf-8"), object_pairs_hook=object_pairs)
    except (UnicodeError, json.JSONDecodeError):
        fail(f"{label} is not valid JSON")


def require_exact_keys(value: object, keys: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        fail(f"{label} has a non-canonical member set")
    return value


class Response:
    def __init__(
        self, status: int, url: str, headers: dict[str, str], body: bytes
    ) -> None:
        self.status = status
        self.url = url
        self.headers = {key.lower(): value for key, value in headers.items()}
        self.body = body


class RejectRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *_: object, **__: object) -> None:
        return None


class NetworkTransport:
    @staticmethod
    def request(
        method: str,
        url: str,
        headers: dict[str, str],
        body: bytes | None,
        limit: int,
        label: str,
        timeout: float,
    ) -> Response:
        context = ssl.create_default_context()
        context.minimum_version = ssl.TLSVersion.TLSv1_2
        opener = urllib.request.build_opener(
            urllib.request.ProxyHandler({}),
            RejectRedirect(),
            urllib.request.HTTPSHandler(context=context),
        )
        request = urllib.request.Request(
            url, data=body, headers=headers, method=method
        )
        try:
            with opener.open(request, timeout=timeout) as stream:
                status = stream.status
                final_url = stream.geturl()
                response_headers = dict(stream.headers.items())
                length_text = stream.headers.get("Content-Length")
                if length_text is not None:
                    if not length_text.isdigit() or int(length_text) > limit:
                        fail(f"{label} has an invalid response length")
                    expected_length = int(length_text)
                else:
                    expected_length = None
                value = stream.read(limit + 1)
        except urllib.error.HTTPError as error:
            if 300 <= error.code < 400:
                fail(f"{label} redirects are forbidden")
            fail(f"{label} failed")
        except (urllib.error.URLError, TimeoutError, OSError, ssl.SSLError):
            fail(f"{label} failed")
        if len(value) > limit:
            fail(f"{label} exceeds the response-size bound")
        if expected_length is not None and len(value) != expected_length:
            fail(f"{label} response is truncated")
        return Response(status, final_url, response_headers, value)


class FixtureTransport:
    def __init__(self, path: Path) -> None:
        try:
            raw = path.read_bytes()
        except OSError:
            fail("test transport fixture is unavailable")
        if len(raw) > MAX_SERVICE_RESPONSE_BYTES * 3:
            fail("test transport fixture exceeds its bound")
        value = require_exact_keys(
            json_no_duplicates(raw, "test transport fixture"),
            {"responses", "schema_version"},
            "test transport fixture",
        )
        if type(value["schema_version"]) is not int or value["schema_version"] != 1:
            fail("test transport fixture schema is invalid")
        if not isinstance(value["responses"], list):
            fail("test transport fixture schema is invalid")
        self.responses = list(value["responses"])

    def assert_consumed(self) -> None:
        if self.responses:
            fail("test transport has unused duplicate responses")

    def request(
        self,
        method: str,
        url: str,
        _headers: dict[str, str],
        _body: bytes | None,
        limit: int,
        label: str,
        _timeout: float,
    ) -> Response:
        if not self.responses:
            fail(f"{label} has no test response")
        value = require_exact_keys(
            self.responses.pop(0),
            {
                "body_base64",
                "chunk_count",
                "chunk_delay_milliseconds",
                "headers",
                "method",
                "status",
                "url",
            },
            "test transport response",
        )
        if value["method"] != method:
            fail("test transport method mismatch")
        try:
            body = base64.b64decode(value["body_base64"], validate=True)
        except (binascii.Error, ValueError, TypeError):
            fail("test transport body is malformed")
        if len(body) > limit:
            fail(f"{label} exceeds the response-size bound")
        if not isinstance(value["headers"], dict) or not all(
            isinstance(key, str) and isinstance(item, str)
            for key, item in value["headers"].items()
        ):
            fail("test transport headers are malformed")
        if (
            not isinstance(value["status"], int)
            or isinstance(value["status"], bool)
            or not isinstance(value["url"], str)
        ):
            fail("test transport response metadata is malformed")
        chunk_count = value["chunk_count"]
        chunk_delay = value["chunk_delay_milliseconds"]
        if (
            type(chunk_count) is not int
            or type(chunk_delay) is not int
            or chunk_count < 1
            or chunk_count > 1000
            or chunk_delay < 0
            or chunk_delay > 1000
        ):
            fail("test transport delay metadata is malformed")
        for _ in range(chunk_count):
            time.sleep(chunk_delay / 1000)
        return Response(value["status"], value["url"], value["headers"], body)


def deadline_signal(_signum: int, _frame: object) -> None:
    raise DeadlineExpired


def bounded_request(
    transport: NetworkTransport | FixtureTransport,
    method: str,
    url: str,
    headers: dict[str, str],
    body: bytes | None,
    limit: int,
    label: str,
    deadline: float,
) -> Response:
    remaining = deadline - time.monotonic()
    if remaining <= 0:
        fail("protected publisher acquisition deadline exceeded")
    if not all(
        hasattr(signal, name) for name in ("SIGALRM", "ITIMER_REAL", "setitimer")
    ):
        fail("monotonic deadline supervision is unavailable")
    old_delay, old_interval = signal.getitimer(signal.ITIMER_REAL)
    if old_delay != 0 or old_interval != 0:
        fail("monotonic deadline supervisor is already in use")
    old_handler = signal.getsignal(signal.SIGALRM)
    try:
        signal.signal(signal.SIGALRM, deadline_signal)
        signal.setitimer(signal.ITIMER_REAL, remaining)
        return transport.request(
            method, url, headers, body, limit, label, remaining
        )
    except DeadlineExpired:
        fail("protected publisher acquisition deadline exceeded")
    except ClientError:
        raise
    except Exception:
        fail(f"{label} failed")
    finally:
        signal.setitimer(signal.ITIMER_REAL, 0)
        signal.signal(signal.SIGALRM, old_handler)


def parse_https_url(
    url: str, expected_host: str, label: str, *, allow_query: bool
) -> str:
    try:
        parsed = urllib.parse.urlsplit(url)
        port = parsed.port
    except ValueError:
        fail(f"{label} is malformed")
    if (
        parsed.scheme != "https"
        or parsed.hostname != expected_host
        or port not in (None, 443)
        or parsed.username is not None
        or parsed.password is not None
        or parsed.fragment
        or (parsed.query and not allow_query)
        or not parsed.path.startswith("/")
    ):
        fail(f"{label} is not the exact allowlisted HTTPS origin")
    return urllib.parse.urlunsplit(parsed)


def oidc_request_url(raw_url: str, audience: str) -> str:
    url = parse_https_url(
        raw_url, OIDC_HOST, "GitHub OIDC request URL", allow_query=True
    )
    parsed = urllib.parse.urlsplit(url)
    try:
        query = urllib.parse.parse_qsl(
            parsed.query, keep_blank_values=True, strict_parsing=True
        )
    except ValueError:
        fail("GitHub OIDC request URL query is malformed")
    if any(key == "audience" for key, _ in query):
        fail("GitHub OIDC request URL already contains an audience")
    query.append(("audience", audience))
    return urllib.parse.urlunsplit(parsed._replace(query=urllib.parse.urlencode(query)))


def checked_response(
    response: Response, expected_url: str, limit: int, label: str
) -> bytes:
    if 300 <= response.status < 400:
        fail(f"{label} redirects are forbidden")
    if response.status != 200:
        fail(f"{label} failed")
    if response.url != expected_url:
        fail(f"{label} final URL mismatch")
    if len(response.body) > limit:
        fail(f"{label} exceeds the response-size bound")
    length_text = response.headers.get("content-length")
    if length_text is not None:
        if not length_text.isdigit() or int(length_text) > limit:
            fail(f"{label} has an invalid response length")
        if int(length_text) != len(response.body):
            fail(f"{label} response is truncated")
    content_type = response.headers.get("content-type", "").split(";", 1)[0].strip()
    if content_type != "application/json":
        fail(f"{label} has an invalid content type")
    return response.body


def valid_git_ref_name(ref: str) -> bool:
    if (
        not ref
        or len(ref) > 1024
        or ref == "@"
        or ref.startswith("/")
        or ref.endswith("/")
        or ref.endswith(".")
        or ref.endswith(".lock")
        or "//" in ref
        or ".." in ref
        or "@{" in ref
        or REF_FORBIDDEN_RE.search(ref)
    ):
        return False
    return not any(
        component in ("", ".", "..")
        or component.startswith(".")
        or component.endswith(".lock")
        for component in ref.split("/")
    )


def oidc_subject_component(value: str) -> str:
    return value.replace(":", "%3A")


def oidc_environment_subject(repository: str, environment_name: str) -> str:
    return (
        f"repo:{oidc_subject_component(repository)}:environment:"
        f"{oidc_subject_component(environment_name)}"
    )


def valid_branch_name(branch: str) -> bool:
    return (
        bool(branch)
        and not branch.startswith(("-", "/", "refs/"))
        and valid_git_ref_name(f"refs/heads/{branch}")
    )


def required_environment(environment: dict[str, str], name: str) -> str:
    value = environment.get(name, "")
    if not value:
        fail(f"required publisher client configuration is missing: {name}")
    return value


def validate_bearer_token(value: str, label: str) -> str:
    try:
        raw = value.encode("ascii", errors="strict")
    except UnicodeError:
        fail(f"{label} is malformed")
    if (
        not raw
        or len(raw) > MAX_BEARER_TOKEN_BYTES
        or not BEARER_RE.fullmatch(value)
    ):
        fail(f"{label} is malformed")
    return value


def decode_base64url_json(segment: str, label: str) -> dict[str, Any]:
    if not re.fullmatch(r"[A-Za-z0-9_-]+", segment):
        fail(f"{label} is malformed")
    encoded = segment.encode("ascii")
    padding = b"=" * ((4 - len(encoded) % 4) % 4)
    try:
        raw = base64.b64decode(encoded + padding, altchars=b"-_", validate=True)
    except (binascii.Error, ValueError):
        fail(f"{label} is malformed")
    if base64.urlsafe_b64encode(raw).rstrip(b"=") != encoded:
        fail(f"{label} is not canonical base64url")
    value = json_no_duplicates(raw, label)
    if not isinstance(value, dict):
        fail(f"{label} is not a JSON object")
    return value


def claim_string(
    claims: dict[str, Any], name: str, *, allow_empty: bool = False
) -> str:
    value = claims.get(name)
    if (
        type(value) is not str
        or len(value) > 1024
        or (not value and not allow_empty)
        or any(ord(character) < 0x20 or ord(character) > 0x7E for character in value)
    ):
        fail(f"GitHub OIDC claim is malformed: {name}")
    return value


def claim_integer(claims: dict[str, Any], name: str) -> int:
    value = claims.get(name)
    if type(value) is not int or value <= 0:
        fail(f"GitHub OIDC claim is malformed: {name}")
    return value


def load_evidence_module() -> Any:
    path = Path(__file__).with_name("parity-signed-evidence.py")
    spec = importlib.util.spec_from_file_location("parity_signed_evidence_client", path)
    if spec is None or spec.loader is None:
        fail("cannot load the protected evidence verifier")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def read_regular(path: Path, limit: int, label: str) -> bytes:
    try:
        info = path.lstat()
        descriptor = os.open(path, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW)
    except OSError:
        fail(f"{label} is unavailable")
    try:
        opened = os.fstat(descriptor)
        if (
            not stat.S_ISREG(info.st_mode)
            or (info.st_dev, info.st_ino) != (opened.st_dev, opened.st_ino)
            or opened.st_size > limit
        ):
            fail(f"{label} has an unsafe identity or size")
        chunks: list[bytes] = []
        remaining = limit + 1
        while remaining:
            chunk = os.read(descriptor, min(1024 * 1024, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        value = b"".join(chunks)
        if len(value) != opened.st_size:
            fail(f"{label} changed while reading")
        return value
    finally:
        os.close(descriptor)


def manifest_fields(raw: bytes) -> dict[str, str]:
    try:
        text = raw.decode("ascii")
    except UnicodeError:
        fail("promotion manifest is not ASCII")
    if not text.endswith("\n") or "\r" in text or "\0" in text:
        fail("promotion manifest is not canonical text")
    wanted = {
        "baseline_commit",
        "hardware_lane",
        "source_commit",
        "source_tree",
        "target",
    }
    output: dict[str, str] = {}
    for line in text.splitlines():
        fields = line.split("\t")
        if fields[0] in wanted:
            if len(fields) != 2 or fields[0] in output:
                fail("promotion manifest contains duplicate identity fields")
            output[fields[0]] = fields[1]
    if set(output) != wanted:
        fail("promotion manifest identity is incomplete")
    if not all(
        COMMIT_RE.fullmatch(output[name])
        for name in ("baseline_commit", "source_commit", "source_tree")
    ):
        fail("promotion manifest commit identity is malformed")
    if not SAFE_TEXT_RE.fullmatch(output["target"]) or not SAFE_TEXT_RE.fullmatch(
        output["hardware_lane"]
    ):
        fail("promotion manifest target identity is malformed")
    return output


def workflow_identity(environment: dict[str, str]) -> dict[str, str]:
    values = {name: required_environment(environment, name) for name in WORKFLOW_ENV}
    numeric = (
        "GITHUB_REPOSITORY_ID",
        "FE2O3_PUBLISHER_REPOSITORY_OWNER_ID",
        "GITHUB_RUN_ID",
        "GITHUB_RUN_ATTEMPT",
        "GITHUB_RUN_NUMBER",
        "GITHUB_ACTOR_ID",
    )
    if any(not values[name].isdigit() or int(values[name]) <= 0 for name in numeric):
        fail("GitHub workflow numeric identity is malformed")
    if not re.fullmatch(
        r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", values["GITHUB_REPOSITORY"]
    ):
        fail("GitHub repository identity is malformed")
    if values["GITHUB_REPOSITORY"] != OIDC_REPOSITORY:
        fail("GitHub repository is outside the OIDC authorization matrix")
    if (
        values["GITHUB_REPOSITORY_ID"] != OIDC_REPOSITORY_ID
        or values["FE2O3_PUBLISHER_REPOSITORY_OWNER_ID"]
        != OIDC_REPOSITORY_OWNER_ID
    ):
        fail("GitHub repository IDs are outside the OIDC authorization matrix")
    if not COMMIT_RE.fullmatch(values["GITHUB_WORKFLOW_SHA"]):
        fail("GitHub workflow SHA is malformed")
    if not COMMIT_RE.fullmatch(values["GITHUB_SHA"]):
        fail("GitHub candidate SHA is malformed")
    if values["GITHUB_EVENT_NAME"] != OIDC_EVENT or values["GITHUB_JOB"] != OIDC_JOB:
        fail("GitHub workflow is outside the production OIDC authorization matrix")
    if values["GITHUB_WORKFLOW"] != "Protected parity promotion":
        fail("GitHub workflow name is outside the OIDC authorization matrix")
    default_branch = values["FE2O3_PUBLISHER_DEFAULT_BRANCH"]
    publisher_environment = values["FE2O3_PUBLISHER_GITHUB_ENVIRONMENT"]
    if not valid_branch_name(default_branch):
        fail("GitHub default branch is malformed")
    if not valid_git_ref_name(values["GITHUB_REF"]):
        fail("GitHub ref is malformed")
    if publisher_environment != OIDC_ENVIRONMENT:
        fail("GitHub publisher environment is outside the OIDC authorization matrix")
    if default_branch != OIDC_DEFAULT_BRANCH:
        fail("GitHub default branch is outside the OIDC authorization matrix")
    queue_prefix = f"refs/heads/gh-readonly-queue/{default_branch}/"
    if not values["GITHUB_REF"].startswith(queue_prefix):
        fail("GitHub ref is outside the merge-queue authorization matrix")
    expected_workflow_ref = (
        f"{values['GITHUB_REPOSITORY']}/{CALLER_WORKFLOW_PATH}@"
        f"{values['GITHUB_REF']}"
    )
    if values["GITHUB_WORKFLOW_REF"] != expected_workflow_ref:
        fail("GitHub caller workflow ref is outside the authorization matrix")
    for name in (
        "GITHUB_WORKFLOW_REF",
        "GITHUB_JOB",
        "GITHUB_EVENT_NAME",
        "GITHUB_REF",
    ):
        if not SAFE_TEXT_RE.fullmatch(values[name]):
            fail(f"GitHub workflow identity is malformed: {name}")
    return {name.lower(): values[name] for name in WORKFLOW_ENV}


def oidc_authorization(
    token: str,
    args: argparse.Namespace,
    environment: dict[str, str],
    audience: str,
    minimum_remaining_seconds: int = NETWORK_TIMEOUT_SECONDS
    + TOKEN_RECOVERY_GRACE_SECONDS,
) -> dict[str, Any]:
    if not COMMIT_RE.fullmatch(args.default_tip) or not COMMIT_RE.fullmatch(
        args.candidate_head
    ):
        fail("publisher request commit identity is malformed")
    if len(token) > MAX_BEARER_TOKEN_BYTES or not JWT_RE.fullmatch(token):
        fail("GitHub OIDC response token is malformed")
    header_segment, claims_segment, _signature = token.split(".")
    header = decode_base64url_json(header_segment, "GitHub OIDC header")
    if not isinstance(header, dict) or frozenset(header) not in OIDC_HEADER_KEY_SETS:
        fail("GitHub OIDC header has a non-canonical member set")
    if (
        header["alg"] != OIDC_ALGORITHM
        or header["typ"] != "JWT"
        or type(header["kid"]) is not str
        or not SAFE_TEXT_RE.fullmatch(header["kid"])
    ):
        fail("GitHub OIDC header is outside the authorization matrix")
    if "x5t" in header:
        x5t = header["x5t"]
        if type(x5t) is not str or not X5T_RE.fullmatch(x5t):
            fail("GitHub OIDC x5t header is malformed")
        try:
            thumbprint = base64.b64decode(
                x5t + "=", altchars=b"-_", validate=True
            )
        except (binascii.Error, ValueError):
            fail("GitHub OIDC x5t header is malformed")
        if (
            len(thumbprint) != 20
            or base64.urlsafe_b64encode(thumbprint).rstrip(b"=").decode("ascii")
            != x5t
        ):
            fail("GitHub OIDC x5t header is malformed")
    claims = decode_base64url_json(claims_segment, "GitHub OIDC claims")
    identity = workflow_identity(environment)
    if (
        identity["github_sha"] != args.candidate_head
        or identity["github_workflow_sha"] != args.candidate_head
    ):
        fail("GitHub workflow SHA does not match the merge-group candidate")
    repository = identity["github_repository"]
    ref = identity["github_ref"]
    subject = oidc_environment_subject(
        repository, identity["fe2o3_publisher_github_environment"]
    )
    expected_strings = {
        "actor_id": identity["github_actor_id"],
        "aud": audience,
        "base_ref": "",
        "event_name": OIDC_EVENT,
        "environment": OIDC_ENVIRONMENT,
        "head_ref": "",
        "iss": OIDC_ISSUER,
        "job_workflow_ref": (
            f"{repository}/{PROTECTED_WORKFLOW_PATH}@{ref}"
        ),
        "job_workflow_sha": args.candidate_head,
        "ref": ref,
        "repository": repository,
        "repository_id": identity["github_repository_id"],
        "repository_owner": repository.split("/", 1)[0],
        "repository_owner_id": identity[
            "fe2o3_publisher_repository_owner_id"
        ],
        "run_attempt": identity["github_run_attempt"],
        "run_id": identity["github_run_id"],
        "run_number": identity["github_run_number"],
        "runner_environment": OIDC_RUNNER_ENVIRONMENT,
        "sha": args.candidate_head,
        "sub": subject,
        "workflow": identity["github_workflow"],
        "workflow_ref": identity["github_workflow_ref"],
        "workflow_sha": args.candidate_head,
    }
    resolved: dict[str, Any] = {
        "job": identity["github_job"],
        "policy_id": OIDC_POLICY_ID,
        "schema_version": OIDC_AUTHORIZATION_SCHEMA_VERSION,
    }
    check_run_id = claim_string(claims, "check_run_id")
    if not check_run_id.isdigit() or int(check_run_id) <= 0:
        fail("GitHub OIDC claim is malformed: check_run_id")
    for name, expected in expected_strings.items():
        actual = claim_string(
            claims, name, allow_empty=name in ("base_ref", "head_ref")
        )
        if actual != expected:
            fail(f"GitHub OIDC claim is outside the authorization matrix: {name}")
        resolved[name] = actual
    issued_at = claim_integer(claims, "iat")
    not_before = claim_integer(claims, "nbf")
    expires_at = claim_integer(claims, "exp")
    now = int(time.time())
    if (
        not_before > issued_at
        or expires_at <= issued_at
        or expires_at - issued_at > MAX_OIDC_TOKEN_LIFETIME
        or not_before > now + OIDC_CLOCK_SKEW
        or expires_at < now
        or expires_at - now < minimum_remaining_seconds
    ):
        fail("GitHub OIDC token freshness is outside the authorization matrix")
    claim_string(claims, "jti")
    return resolved


def stable_idempotency_key(runner_temp: Path, request_digest: str) -> str:
    if not SHA256_RE.fullmatch(request_digest):
        fail("publisher request digest is malformed")
    runner = Path(os.path.abspath(runner_temp))
    name = f".fe2o3-publisher-idempotency-{request_digest}"
    identity = lambda value: (
        value.st_dev,
        value.st_ino,
        value.st_mode,
        value.st_uid,
        value.st_gid,
        value.st_nlink,
        value.st_size,
        value.st_mtime_ns,
        value.st_ctime_ns,
    )
    try:
        directory_fd = os.open(
            runner, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC | os.O_NOFOLLOW
        )
    except OSError:
        fail("RUNNER_TEMP must be a real directory")
    try:
        directory = os.fstat(directory_fd)
        if (
            not stat.S_ISDIR(directory.st_mode)
            or directory.st_uid != os.geteuid()
            or directory.st_mode & 0o022
        ):
            fail("RUNNER_TEMP is not owner-controlled")
        try:
            descriptor = os.open(
                name,
                os.O_WRONLY
                | os.O_CREAT
                | os.O_EXCL
                | os.O_CLOEXEC
                | os.O_NOFOLLOW,
                0o600,
                dir_fd=directory_fd,
            )
        except FileExistsError:
            descriptor = -1
        except OSError:
            fail("publisher idempotency key creation failed")
        if descriptor >= 0:
            try:
                os.fchmod(descriptor, 0o600)
                write_all(
                    descriptor,
                    f"{secrets.token_hex(32)}\n".encode("ascii"),
                    "publisher idempotency key",
                )
                os.fsync(descriptor)
                os.fsync(directory_fd)
            finally:
                os.close(descriptor)
        try:
            before = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
            descriptor = os.open(
                name,
                os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW,
                dir_fd=directory_fd,
            )
        except OSError:
            fail("publisher idempotency key is unavailable")
        try:
            opened = os.fstat(descriptor)
            if (
                identity(before) != identity(opened)
                or not stat.S_ISREG(opened.st_mode)
                or opened.st_uid != os.geteuid()
                or stat.S_IMODE(opened.st_mode) != 0o600
                or opened.st_nlink != 1
                or opened.st_size != 65
            ):
                fail("publisher idempotency key metadata is invalid")
            chunks: list[bytes] = []
            total = 0
            while total <= 65:
                chunk = os.read(descriptor, 66 - total)
                if not chunk:
                    break
                chunks.append(chunk)
                total += len(chunk)
            raw = b"".join(chunks)
            if len(raw) > 65 or os.read(descriptor, 1):
                fail("publisher idempotency key exceeds its bound")
            after = os.fstat(descriptor)
            final = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
            if identity(opened) != identity(after) or identity(opened) != identity(final):
                fail("publisher idempotency key changed during read")
        finally:
            os.close(descriptor)
    finally:
        os.close(directory_fd)
    try:
        key = raw.decode("ascii").removesuffix("\n")
    except UnicodeError:
        fail("publisher idempotency key is malformed")
    if not SHA256_RE.fullmatch(key):
        fail("publisher idempotency key is malformed")
    return key


def build_request(
    args: argparse.Namespace,
    environment: dict[str, str],
    authorization: dict[str, Any],
) -> tuple[bytes, dict[str, str], Any]:
    if not COMMIT_RE.fullmatch(args.default_tip) or not COMMIT_RE.fullmatch(
        args.candidate_head
    ):
        fail("publisher request commit identity is malformed")
    if not (
        args.logical_destination == "docs/parity-evidence/archive"
        or args.logical_destination.startswith("docs/parity-evidence/archive/")
    ):
        fail("publisher logical destination is invalid")
    evidence = load_evidence_module()
    with evidence.ArchiveSnapshot(
        args.archive_root, require_immutable=False
    ) as archive:
        archive_identity = evidence.publisher_archive_identity(archive)
        manifest_raw = archive.read(args.manifest)
        manifest_digest = hashlib.sha256(manifest_raw).hexdigest()
    manifest = manifest_fields(manifest_raw)
    baseline_status = hashlib.sha256(
        read_regular(args.baseline_status, 4 * 1024 * 1024, "baseline status")
    ).hexdigest()
    candidate_status = hashlib.sha256(
        read_regular(args.candidate_status, 4 * 1024 * 1024, "candidate status")
    ).hexdigest()
    identity = workflow_identity(environment)
    request = {
        "archive_sha256": archive_identity,
        "baseline_status_sha256": baseline_status,
        "candidate_head": args.candidate_head,
        "candidate_status_sha256": candidate_status,
        "default_tip": args.default_tip,
        "hardware_lane": manifest["hardware_lane"],
        "logical_destination": args.logical_destination,
        "manifest_baseline_commit": manifest["baseline_commit"],
        "manifest_path": args.manifest,
        "manifest_sha256": manifest_digest,
        "oidc_authorization": authorization,
        "request_domain": REQUEST_DOMAIN,
        "schema_version": REQUEST_SCHEMA_VERSION,
        "source_commit": manifest["source_commit"],
        "source_tree": manifest["source_tree"],
        "target": manifest["target"],
        "workflow": identity,
    }
    raw = canonical_json(request)
    if len(raw) > MAX_REQUEST_BYTES:
        fail("publisher service request exceeds its bound")
    expected = {
        "archive_sha256": archive_identity,
        "baseline_status_sha256": baseline_status,
        "candidate_head": args.candidate_head,
        "candidate_status_sha256": candidate_status,
        "default_tip": args.default_tip,
        "hardware_lane": manifest["hardware_lane"],
        "logical_destination": args.logical_destination,
        "manifest_path": args.manifest,
        "manifest_sha256": manifest_digest,
        "source_commit": manifest["source_commit"],
        "source_tree": manifest["source_tree"],
        "target": manifest["target"],
    }
    return raw, expected, evidence


def parse_receipt(
    receipt: bytes, challenge: str, expected: dict[str, str], domain: str
) -> None:
    if len(receipt) > MAX_RECEIPT_BYTES:
        fail("publisher receipt exceeds its bound")
    try:
        text = receipt.decode("ascii")
    except UnicodeError:
        fail("publisher receipt is not ASCII")
    if not text.endswith("\n") or "\r" in text or "\0" in text:
        fail("publisher receipt is not canonical text")
    rows = [line.split("\t") for line in text.splitlines()]
    if len(rows) != len(RECEIPT_FIELDS) or any(
        len(row) != 2 or row[0] != RECEIPT_FIELDS[index]
        for index, row in enumerate(rows)
    ):
        fail("publisher receipt field order is non-canonical")
    values = {row[0]: row[1] for row in rows}
    checks = {
        "publisher_contract_receipt_schema_version": "2",
        "publisher_key_role": "publisher",
        "destination_contract": "external-protected-portable-archive-v2",
        "logical_destination": expected["logical_destination"],
        "archive_sha256": expected["archive_sha256"],
        "manifest_path": expected["manifest_path"],
        "manifest_sha256": expected["manifest_sha256"],
        "source_commit": expected["source_commit"],
        "source_tree": expected["source_tree"],
        "target": expected["target"],
        "hardware_lane": expected["hardware_lane"],
        "baseline_status_sha256": expected["baseline_status_sha256"],
        "candidate_status_sha256": expected["candidate_status_sha256"],
        "default_tip": expected["default_tip"],
        "candidate_head": expected["candidate_head"],
        "freshness_challenge": challenge,
        "signature_schema_version": "1",
        "signature_domain": domain,
        "signature_role": "publisher",
        "signature_algorithm": "ed25519",
    }
    if any(values[key] != value for key, value in checks.items()):
        fail("publisher receipt does not match the protected request")
    if (
        not ID_RE.fullmatch(values["publisher_identity"])
        or not ID_RE.fullmatch(values["signing_key_id"])
        or values["publisher_identity"] != values["signing_key_id"]
        or not values["issued_at_unix"].isdigit()
        or not values["expires_at_unix"].isdigit()
    ):
        fail("publisher receipt identity or freshness is malformed")
    try:
        signature = base64.b64decode(values["signature_base64"], validate=True)
    except (binascii.Error, ValueError):
        fail("publisher receipt signature encoding is malformed")
    if len(signature) != 64:
        fail("publisher receipt signature length is malformed")
    issued_at = int(values["issued_at_unix"])
    expires_at = int(values["expires_at_unix"])
    now = int(time.time())
    if (
        issued_at <= 0
        or expires_at <= issued_at
        or expires_at - issued_at > MAX_PUBLISHER_RECEIPT_LIFETIME
        or issued_at > now + PUBLISHER_RECEIPT_CLOCK_SKEW
        or expires_at < now
    ):
        fail("publisher receipt is stale or has an invalid lifetime")


def verify_receipt_signature(
    receipt: bytes,
    args: argparse.Namespace,
    evidence: Any,
    domain: str,
) -> None:
    trust = evidence.parse_trust_policy(args.trusted_root, args.trust_policy)
    if trust.domain != domain:
        fail("publisher receipt trust domain mismatch")
    runner_temp = Path(required_environment(dict(os.environ), "RUNNER_TEMP"))
    with tempfile.TemporaryDirectory(
        prefix="fe2o3-publisher-verify-", dir=runner_temp
    ) as raw_temp:
        root = Path(raw_temp)
        path = root / RECEIPT_NAME
        path.write_bytes(receipt)
        path.chmod(0o600)
        try:
            with evidence.ArchiveSnapshot(
                root, require_immutable=False
            ) as snapshot:
                evidence.verify_signed(snapshot, RECEIPT_NAME, trust, "publisher")
        except evidence.EvidenceError:
            fail("publisher receipt signature verification failed")


def write_all(descriptor: int, value: bytes, label: str) -> None:
    view = memoryview(value)
    while view:
        count = os.write(descriptor, view)
        if count <= 0:
            fail(f"short write while publishing {label}")
        view = view[count:]


def publish_outputs(
    receipt: bytes,
    challenge: str,
    receipt_root: Path,
    challenge_file: Path,
    runner_temp: Path,
) -> None:
    runner = Path(os.path.abspath(runner_temp))
    receipt_root = Path(os.path.abspath(receipt_root))
    challenge_file = Path(os.path.abspath(challenge_file))
    if receipt_root.parent != runner or challenge_file.parent != runner:
        fail("publisher outputs must be direct children of RUNNER_TEMP")
    if receipt_root.name in ("", ".", "..") or challenge_file.name in ("", ".", ".."):
        fail("publisher output names are invalid")
    try:
        parent_fd = os.open(
            runner, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC | os.O_NOFOLLOW
        )
    except OSError:
        fail("RUNNER_TEMP must be a real directory")
    receipt_fd = -1
    receipt_dir_fd = -1
    challenge_fd = -1
    receipt_dir_created = False
    receipt_created = False
    challenge_created = False
    try:
        for name in (receipt_root.name, challenge_file.name):
            try:
                os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
            except FileNotFoundError:
                pass
            else:
                fail("protected publisher output already exists")
        os.mkdir(receipt_root.name, 0o700, dir_fd=parent_fd)
        receipt_dir_created = True
        receipt_dir_fd = os.open(
            receipt_root.name,
            os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC | os.O_NOFOLLOW,
            dir_fd=parent_fd,
        )
        os.fchmod(receipt_dir_fd, 0o700)
        receipt_fd = os.open(
            RECEIPT_NAME,
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | os.O_CLOEXEC
            | os.O_NOFOLLOW,
            0o600,
            dir_fd=receipt_dir_fd,
        )
        receipt_created = True
        os.fchmod(receipt_fd, 0o600)
        write_all(receipt_fd, receipt, "publisher receipt")
        os.fsync(receipt_fd)
        os.fsync(receipt_dir_fd)
        challenge_fd = os.open(
            challenge_file.name,
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | os.O_CLOEXEC
            | os.O_NOFOLLOW,
            0o600,
            dir_fd=parent_fd,
        )
        challenge_created = True
        os.fchmod(challenge_fd, 0o600)
        write_all(challenge_fd, f"{challenge}\n".encode("ascii"), "challenge")
        os.fsync(challenge_fd)
        os.fsync(parent_fd)
    except BaseException:
        if challenge_fd >= 0:
            os.close(challenge_fd)
            challenge_fd = -1
        if receipt_fd >= 0:
            os.close(receipt_fd)
            receipt_fd = -1
        if challenge_created:
            os.unlink(challenge_file.name, dir_fd=parent_fd)
        if receipt_created and receipt_dir_fd >= 0:
            os.unlink(RECEIPT_NAME, dir_fd=receipt_dir_fd)
        if receipt_dir_created:
            os.rmdir(receipt_root.name, dir_fd=parent_fd)
        raise
    finally:
        if challenge_fd >= 0:
            os.close(challenge_fd)
        if receipt_fd >= 0:
            os.close(receipt_fd)
        if receipt_dir_fd >= 0:
            os.close(receipt_dir_fd)
        os.close(parent_fd)


def acquire(args: argparse.Namespace, environment: dict[str, str]) -> None:
    service_url = required_environment(environment, "FE2O3_PUBLISHER_SERVICE_URL")
    service_host = required_environment(environment, "FE2O3_PUBLISHER_SERVICE_HOST")
    audience = required_environment(environment, "FE2O3_PUBLISHER_OIDC_AUDIENCE")
    if not HOST_RE.fullmatch(service_host):
        fail("protected publisher service host is invalid")
    if not SAFE_TEXT_RE.fullmatch(audience):
        fail("protected publisher OIDC audience is invalid")
    service_url = parse_https_url(
        service_url,
        service_host,
        "protected publisher service URL",
        allow_query=False,
    )
    fixture = args.test_transport_fixture
    if args.test_domain:
        if (
            fixture is None
            or environment.get("FE2O3_PUBLISHER_CLIENT_TEST_DOMAIN") != "1"
        ):
            fail("test transport requires the explicit test-domain guard")
        domain = "test"
        transport: NetworkTransport | FixtureTransport = FixtureTransport(fixture)
        if args.test_deadline_milliseconds is None:
            deadline_seconds = NETWORK_TIMEOUT_SECONDS
        elif not 1 <= args.test_deadline_milliseconds <= MAX_TEST_DEADLINE_MILLISECONDS:
            fail("test acquisition deadline is outside its bound")
        else:
            deadline_seconds = args.test_deadline_milliseconds / 1000
    else:
        if (
            fixture is not None
            or args.test_deadline_milliseconds is not None
            or "FE2O3_PUBLISHER_CLIENT_TEST_DOMAIN" in environment
        ):
            fail("test transport is forbidden for production acquisition")
        domain = "production"
        transport = NetworkTransport()
        deadline_seconds = NETWORK_TIMEOUT_SECONDS

    oidc_url_raw = required_environment(environment, "ACTIONS_ID_TOKEN_REQUEST_URL")
    oidc_request_token = validate_bearer_token(
        required_environment(environment, "ACTIONS_ID_TOKEN_REQUEST_TOKEN"),
        "GitHub OIDC request token",
    )
    oidc_url = oidc_request_url(oidc_url_raw, audience)
    deadline = time.monotonic() + deadline_seconds
    oidc_response = bounded_request(
        transport,
        "GET",
        oidc_url,
        {"Authorization": f"bearer {oidc_request_token}", "Accept": "application/json"},
        None,
        MAX_OIDC_RESPONSE_BYTES,
        "GitHub OIDC token request",
        deadline,
    )
    oidc_raw = checked_response(
        oidc_response, oidc_url, MAX_OIDC_RESPONSE_BYTES, "GitHub OIDC token request"
    )
    oidc = require_exact_keys(
        json_no_duplicates(oidc_raw, "GitHub OIDC response"),
        {"value"},
        "GitHub OIDC response",
    )
    token = oidc["value"]
    if type(token) is not str:
        fail("GitHub OIDC response token is malformed")
    token = validate_bearer_token(token, "GitHub OIDC response token")
    authorization = oidc_authorization(
        token,
        args,
        environment,
        audience,
        math.ceil(deadline_seconds) + TOKEN_RECOVERY_GRACE_SECONDS,
    )
    request_body, expected, evidence = build_request(
        args, environment, authorization
    )
    request_digest = hashlib.sha256(request_body).hexdigest()
    idempotency_key = stable_idempotency_key(
        Path(required_environment(environment, "RUNNER_TEMP")), request_digest
    )

    service_response = bounded_request(
        transport,
        "POST",
        service_url,
        {
            "Authorization": f"Bearer {token}",
            "Accept": "application/json",
            "Content-Type": "application/json",
            "Idempotency-Key": idempotency_key,
            "User-Agent": "fe2o3-parity-publisher-client/1",
        },
        request_body,
        MAX_SERVICE_RESPONSE_BYTES,
        "protected publisher service request",
        deadline,
    )
    service_raw = checked_response(
        service_response,
        service_url,
        MAX_SERVICE_RESPONSE_BYTES,
        "protected publisher service request",
    )
    if isinstance(transport, FixtureTransport):
        transport.assert_consumed()
    service = require_exact_keys(
        json_no_duplicates(service_raw, "protected publisher response"),
        {"challenge", "publisher_receipt_base64", "request_sha256", "schema_version"},
        "protected publisher response",
    )
    if canonical_json(service) != service_raw:
        fail("protected publisher response is not canonical JSON")
    if (
        type(service["schema_version"]) is not int
        or service["schema_version"] != 1
        or service["request_sha256"] != request_digest
        or not isinstance(service["challenge"], str)
        or not SHA256_RE.fullmatch(service["challenge"])
        or not isinstance(service["publisher_receipt_base64"], str)
    ):
        fail("protected publisher response does not match the request")
    try:
        receipt = base64.b64decode(
            service["publisher_receipt_base64"], validate=True
        )
    except (binascii.Error, ValueError):
        fail("protected publisher receipt encoding is malformed")
    parse_receipt(receipt, service["challenge"], expected, domain)
    verify_receipt_signature(receipt, args, evidence, domain)
    publish_outputs(
        receipt,
        service["challenge"],
        args.receipt_root,
        args.challenge_file,
        Path(required_environment(environment, "RUNNER_TEMP")),
    )
    print("protected publisher receipt acquired")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--archive-root", type=Path, required=True)
    parser.add_argument("--manifest", required=True)
    parser.add_argument("--baseline-status", type=Path, required=True)
    parser.add_argument("--candidate-status", type=Path, required=True)
    parser.add_argument("--default-tip", required=True)
    parser.add_argument("--candidate-head", required=True)
    parser.add_argument("--logical-destination", required=True)
    parser.add_argument("--trusted-root", type=Path, required=True)
    parser.add_argument("--trust-policy", type=Path, required=True)
    parser.add_argument("--receipt-root", type=Path, required=True)
    parser.add_argument("--challenge-file", type=Path, required=True)
    parser.add_argument("--test-domain", action="store_true")
    parser.add_argument("--test-deadline-milliseconds", type=int)
    parser.add_argument("--test-transport-fixture", type=Path)
    return parser


def main() -> None:
    args = build_parser().parse_args()
    environment = dict(os.environ)
    runner_temp = environment.get("RUNNER_TEMP", "/tmp")
    os.environ.clear()
    os.environ.update(
        {
            "HOME": runner_temp,
            "LC_ALL": "C",
            "PATH": "/usr/bin:/bin",
            "RUNNER_TEMP": runner_temp,
        }
    )
    acquire(args, environment)


if __name__ == "__main__":
    try:
        main()
    except ClientError as error:
        print(f"parity publisher client: {error}", file=sys.stderr)
        raise SystemExit(2)
    except Exception:
        print("parity publisher client: protected acquisition failed", file=sys.stderr)
        raise SystemExit(2)
