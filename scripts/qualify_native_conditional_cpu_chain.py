#!/usr/bin/env python3
"""Inert native CPU-chain planning and strict proof-only result observation.

No command in this module provisions, starts services, opens GPU devices or grants
proof authority. A proof-only application result is not manager/cgroup cleanup.
"""
from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
_spec = importlib.util.spec_from_file_location(
    "native_application_qualification", ROOT / "scripts/qualify_native_conditional_application.py")
application = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(application)
SCHEMA = "fe2o3.native-conditional-proof-only.v1"
MAX_LOG = 4 * 1024 * 1024
PROFILE = {"target": "gfx942:xnack-", "proof_only": True, "gpu_execution": False,
           "native_launches": 0, "native_copies": 0, "application_proof_observed": True,
           "currentness_verified": True, "physical_overlap_measured": False,
           "all_host_devices_qualified": False}
FIELDS = set(application.BLOBS + application.IDENTITIES) | set(PROFILE) | {"schema"}


def startup_plan():
    """Same original compiler-first order; no host systemd requirement or action."""
    return application.startup_helper().phase_plan()


def validate_result(value):
    if not isinstance(value, dict) or set(value) != FIELDS or value["schema"] != SCHEMA:
        raise ValueError("exact native CPU proof-only result required")
    for key, expected in PROFILE.items():
        if type(value[key]) is not type(expected) or value[key] != expected:
            raise ValueError(f"proof-only result differs: {key}")
    application.validate_provenance(value)
    return value


def result_from_log(raw):
    if not isinstance(raw, bytes) or not 0 < len(raw) <= MAX_LOG:
        raise ValueError("bounded nonempty proof-only log required")
    found = []
    for line in raw.decode("utf-8", errors="strict").splitlines():
        if not line.lstrip().startswith("{"):
            continue
        try:
            value = json.loads(line, object_pairs_hook=application.unique_object)
        except ValueError:
            if "fe2o3.native-conditional" in line or "fe2o3.genuine" in line:
                raise ValueError("malformed native CPU result") from None
            continue
        if not isinstance(value, dict):
            continue
        schema = value.get("schema", "")
        if isinstance(schema, str) and schema.startswith(("fe2o3.native-conditional", "fe2o3.genuine")):
            found.append(validate_result(value))
    if len(found) != 1:
        raise ValueError("exactly one proof-only application result required")
    return found[0]


def reconcile_application(outcome, raw):
    """Checks captured output only; never constructs a live-custody capability."""
    if (outcome["status"] != "cargo-completed-unqualified"
            or type(outcome["exitCode"]) is not int or outcome["exitCode"] != 0
            or outcome["logComplete"] is not True or outcome["directChildReaped"] is not True
            or type(outcome["logBytes"]) is not int or outcome["logBytes"] != len(raw)
            or outcome["logSha256"] != hashlib.sha256(raw).hexdigest()):
        raise ValueError("exact successful proof-only application capture required")
    return result_from_log(raw)


def command(cargo, source, target, producer_source):
    if (not isinstance(producer_source, str) or not 0 < len(producer_source.encode("utf-8")) <= 4096
            or any(c in producer_source for c in "\0\n\r")):
        raise ValueError("exact bounded producer source spelling required")
    return [str(cargo), "authority", "release", "--native", "run", "--native-application-proof-custodian",
            "--manifest-path", str(Path(source) / application.FIXTURE / "Cargo.toml"),
            "--target-dir", str(target), "--offline", "--frozen", "--bin",
            "native-conditional-proof-only", "--", "--native-v5-proof-only",
            "--producer-source", producer_source]
