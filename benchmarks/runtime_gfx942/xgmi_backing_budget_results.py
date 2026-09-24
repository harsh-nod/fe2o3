#!/usr/bin/env python3
"""Exact budget witness receipt, not aggregate, performance or refinement evidence."""

FIELDS = (
    "device_limits=8192:3,24576:2 coherent_limits=4096:1,8192:2 "
    "pressure_dimensions=inferred-byte-record capacity_rejections=2 retries=2 "
    "fresh_identity=context directed_copies=2 statuses=2-succeeded published_snapshots=2 checked_bytes=36875 "
    "guards=complete n2_mapping_delta=0 n1=0/0-4096/0-4096/4096-0/0 "
    "request_credits=restored charges=zero cleanup=complete native_execution=true "
    "exclusive_reservation=false performance_claim=false formal_refinement=false aggregate_bound=false"
)
SCHEMA = "fe2o3.runtime.xgmi-backing-budget.v1"


def parse_receipt(raw, unique_ids):
    if (type(unique_ids) is not list or len(unique_ids) != 2
            or any(type(uid) is not int or not 0 < uid < 2**64 for uid in unique_ids)
            or unique_ids[0] == unique_ids[1]):
        raise ValueError("two distinct nonzero u64 identities required")
    expected = (f"PASS schema={SCHEMA} uid0={unique_ids[0]:016x} uid1={unique_ids[1]:016x} " + FIELDS + "\n").encode("ascii")
    if type(raw) is not bytes or raw != expected:
        raise ValueError("native backing budget receipt does not match the fixed correctness contract")
    return {"schema": SCHEMA, "unique_ids": list(unique_ids), "directed_copies": 2,
            "capacity_rejections": 2, "retries": 2, "checked_bytes": 36875,
            "pressure_dimensions": "inferred-byte-record", "fresh_identity": "context",
            "runtime_cleanup": True, "native_execution": True, "exclusive_reservation": False,
            "performance_acceptance": False, "formal_refinement": False, "aggregate_bound": False}
