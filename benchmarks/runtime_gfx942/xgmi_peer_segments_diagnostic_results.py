#!/usr/bin/env python3
"""Validate teardown-gated ordered-list host attribution, never device timing."""

import importlib.util
from pathlib import Path
import re

SPEC = importlib.util.spec_from_file_location(
    "segments_results", Path(__file__).with_name("xgmi_peer_segments_results.py"))
R = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(R)

SCHEMA = "fe2o3.xgmi-ordered-segments-host-attribution.v1"
PHASES = ("admission_validation_ns", "preparation_ns", "opening_currentness_ns",
          "submission_ns", "wait_ns", "closing_currentness_ns", "settlement_ns")
PAIR_PHASES = ("source_before_ns", "peer_before_ns", "topology_discovery_ns",
               "route_and_equality_ns", "source_after_ns", "peer_after_ns")
TOPOLOGY_PHASES = ("topology_tree_ns", "topology_initial_identity_ns",
                   "topology_render_correlation_ns", "topology_closing_identity_ns")
PAIR_FIELDS = (*PAIR_PHASES, "pair_total_ns", *TOPOLOGY_PHASES, "topology_total_ns")
TIMINGS = (*PHASES, "total_ns", *(f"{side}_{field}"
           for side in ("opening", "closing") for field in PAIR_FIELDS))


def _ns(value):
    if not re.fullmatch(r"0|[1-9][0-9]{0,19}", value) or int(value) >= 2**64:
        raise ValueError("canonical u64 host interval required")
    return int(value)


def parse_receipt(raw, *, diagnostic, backend, unique_ids, useful_bytes,
                  descriptor_count, warmups, samples):
    """Mode, geometry and identities must come from the admitted command."""
    if type(diagnostic) is not bool or backend != "kfd":
        raise ValueError("explicit KFD diagnostic mode required")
    controls = dict(backend=backend, unique_ids=unique_ids, useful_bytes=useful_bytes,
                    descriptor_count=descriptor_count, warmups=warmups, samples=samples)
    if not diagnostic:
        return {"receipt": R.parse_receipt(raw, **controls), "observations": []}
    _, bands = R.geometry(useful_bytes, descriptor_count, warmups, samples)
    count = bands * 2
    if type(raw) is not bytes or not raw or len(raw) > 1_000_000 or not raw.endswith(b"\n"):
        raise ValueError("bounded newline-terminated diagnostic receipt required")
    lines = raw.splitlines(keepends=True)
    if len(lines) != count * 2 + 1 or any(not line.endswith(b"\n") for line in lines):
        raise ValueError("incomplete or extra joined diagnostic roster")
    receipt = R.parse_receipt(b"".join(lines[count:]), **controls)
    observations = []
    for ordinal, raw_line in enumerate(lines[:count]):
        try:
            row = R._fields(raw_line[:-1].decode("ascii"))
        except UnicodeDecodeError as error:
            raise ValueError("ASCII diagnostic record required") from error
        if any(field not in row for field in TIMINGS):
            raise ValueError("missing diagnostic interval")
        timing = {field: _ns(row.pop(field)) for field in TIMINGS}
        band, direction = divmod(ordinal, 2)
        expected = {
            "schema": SCHEMA, "backend": "kfd", "ordinal": str(ordinal),
            "backend_submission": str(ordinal + 7),
            "source_uid": f"{unique_ids[direction]:016x}",
            "destination_uid": f"{unique_ids[1 - direction]:016x}",
            "descriptor_count": str(descriptor_count), "useful_bytes": str(useful_bytes),
            "submission_calls": str(descriptor_count), "wait_calls": str(descriptor_count),
            "authority": "none", "teardown": "explicit",
            "timing": "backend-ordered-segments-currentness-host-only",
        }
        if row != expected:
            raise ValueError("diagnostic identity, call counts or scope mismatch")
        total = timing["total_ns"]
        facade = receipt["all_list_ns_by_direction"][direction][band]
        if not 0 < total <= facade or sum(timing[field] for field in PHASES) > total:
            raise ValueError("outer intervals exceed backend/facade total")
        for side in ("opening", "closing"):
            pair = {field: timing[f"{side}_{field}"] for field in PAIR_FIELDS}
            if (sum(pair[field] for field in PAIR_PHASES) > pair["pair_total_ns"]
                    or pair["pair_total_ns"] > timing[f"{side}_currentness_ns"]
                    or sum(pair[field] for field in TOPOLOGY_PHASES) > pair["topology_total_ns"]
                    or pair["topology_total_ns"] > pair["topology_discovery_ns"]):
                raise ValueError("inconsistent nested currentness intervals")
        observations.append({"ordinal": ordinal, "backend_submission": ordinal + 7,
                             "band": band, "direction": direction,
                             "population": "prime" if band == 0 else "warmup" if band <= warmups else "sample",
                             **timing})
    return {"receipt": receipt, "observations": observations}
