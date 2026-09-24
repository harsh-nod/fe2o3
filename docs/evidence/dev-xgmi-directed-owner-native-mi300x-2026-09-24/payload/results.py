#!/usr/bin/env python3
"""Exact native directed-owner receipt, with no performance or refinement claim."""

FIELDS = (
    "owner_threads=1 allocations=5 streams=4 directed_copies=4 dependency_edges=4 max_depth=3 "
    "routes=forward-reverse-reverse-forward journal=enabled pending_dataflow=supported "
    "adapter=join-tracked events=released-before-progress statuses=4-succeeded rejected=0 "
    "checked_bytes=327680 payload_bytes=131072 guard_bytes=131072 source_bytes=65536 "
    "canaries=complete source=unchanged cleanup=complete native_execution=true "
    "exclusive_reservation=false performance_claim=false formal_refinement=false"
)
SCHEMA = "fe2o3.runtime.xgmi-directed-owner.v1"


def parse_receipt(raw, unique_ids):
    if (type(unique_ids) is not list or len(unique_ids) != 2
            or any(type(uid) is not int or not 0 < uid < 2**64 for uid in unique_ids)
            or unique_ids[0] == unique_ids[1]):
        raise ValueError("two distinct nonzero u64 identities required")
    expected = (f"PASS schema={SCHEMA} uid0={unique_ids[0]:016x} uid1={unique_ids[1]:016x} " + FIELDS + "\n").encode("ascii")
    if type(raw) is not bytes or raw != expected:
        raise ValueError("native directed owner receipt does not match the fixed correctness contract")
    return {"schema": SCHEMA, "unique_ids": list(unique_ids), "directed_copies": 4,
            "dependency_edges": 4, "checked_bytes": 327680, "owned_cleanup": True,
            "version_journal": True, "pending_dataflow": "supported", "native_execution": True,
            "events": "released-before-progress", "exclusive_reservation": False,
            "performance_acceptance": False, "formal_refinement": False}
