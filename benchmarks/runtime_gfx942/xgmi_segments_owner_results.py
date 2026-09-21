#!/usr/bin/env python3
"""Exact native owner correctness receipt; explicitly not pending-dataflow support."""

FIELDS = (
    "owner_threads=1 allocations=4 streams=3 segments=65 ordered_lists=2 producer_copies=1 "
    "cancelled=not_submitted timeout_identity=retained dropped_observer=completed journal=enabled "
    "pending_dataflow=refused_before_submission dependency=completed_producer directions=both "
    "canaries=complete source=unchanged cleanup=complete performance_claim=false"
)
SCHEMA = "fe2o3.runtime.xgmi-segments-owner.v1"


def parse_receipt(raw, unique_ids):
    if (len(unique_ids) != 2 or len(set(unique_ids)) != 2
            or any(type(uid) is not int or not 0 < uid < 2**64 for uid in unique_ids)):
        raise ValueError("two distinct nonzero u64 identities required")
    expected = (f"PASS schema={SCHEMA} uid0={unique_ids[0]:016x} uid1={unique_ids[1]:016x} " + FIELDS + "\n").encode("ascii")
    if raw != expected:
        raise ValueError("native owner receipt does not match the fixed correctness contract")
    return {"schema": SCHEMA, "unique_ids": unique_ids, "ordered_lists": 2,
            "descriptor_count": 65, "owned_cleanup": True, "version_journal": True,
            "pending_dataflow": "refused_before_submission", "performance_acceptance": False,
            "formal_refinement": False}
