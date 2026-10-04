#!/usr/bin/env python3
"""Verify a reference-service response with the production client parser."""

from __future__ import annotations

import argparse
import base64
import importlib.util
import json
import os
from pathlib import Path
import sys


sys.dont_write_bytecode = True


ROOT = Path(__file__).resolve().parents[2]
CLIENT_PATH = ROOT / "scripts/parity-publisher-client.py"


def load_client():
    spec = importlib.util.spec_from_file_location(
        "parity_publisher_client_service_conformance", CLIENT_PATH
    )
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load publisher client")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--response", type=Path, required=True)
    parser.add_argument("--expected", type=Path, required=True)
    parser.add_argument("--trusted-root", type=Path, required=True)
    parser.add_argument("--trust-policy", type=Path, required=True)
    parser.add_argument("--runner-temp", type=Path, required=True)
    args = parser.parse_args()

    client = load_client()
    response_raw = args.response.read_bytes()
    response = client.require_exact_keys(
        client.json_no_duplicates(response_raw, "service response"),
        {
            "challenge",
            "publisher_receipt_base64",
            "request_sha256",
            "schema_version",
        },
        "service response",
    )
    if client.canonical_json(response) != response_raw:
        raise RuntimeError("service response is not canonical JSON")
    expected = json.loads(args.expected.read_text(encoding="ascii"))
    receipt = base64.b64decode(
        response["publisher_receipt_base64"], validate=True
    )
    client.parse_receipt(receipt, response["challenge"], expected, "test")
    os.environ["RUNNER_TEMP"] = str(args.runner_temp)
    verifier_args = argparse.Namespace(
        trusted_root=args.trusted_root,
        trust_policy=args.trust_policy,
    )
    client.verify_receipt_signature(
        receipt, verifier_args, client.load_evidence_module(), "test"
    )
    print("protected publisher client-service conformance: PASS")


if __name__ == "__main__":
    main()
