#!/usr/bin/env python3
"""Strict joins between independent physical and native API observations."""

import hashlib
import json
import re

from xgmi_peer_series_campaign import _admission


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"),
                                     allow_nan=False).encode("ascii")).hexdigest()


def parse_query(raw, backend):
    if backend not in ("kfd", "hsa", "hip") or type(raw) is not bytes or \
            len(raw) > 8192 or not raw.endswith(b"\n"):
        raise ValueError("invalid query envelope")
    lines = raw.decode("ascii").split("\n")[:-1]
    if len(lines) != (6 if backend == "kfd" else 3) or \
            lines[0] != f"schema=xgmi-peer-query-v1 backend={backend} visible_count=2":
        raise ValueError("invalid query roster")
    result = {"endpoints": [], "routes": []}
    offset = 1
    if backend == "kfd":
        match = re.fullmatch(r"boot_id=([0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}) "
                             r"generation=(0|[1-9][0-9]*)", lines[1])
        if match is None or int(match[2]) >= 1 << 64:
            raise ValueError("invalid topology incarnation")
        result.update(boot_id=match[1], generation=int(match[2]))
        offset = 2
    for index, line in enumerate(lines[offset:offset + 2]):
        match = re.fullmatch(
            rf"visible_index={index} unique_id=([0-9a-f]{{16}}) "
            r"pci_bdf=([0-9a-fA-F]{4}:[0-9a-fA-F]{2}:[0-1][0-9a-fA-F]\.[0-7]) "
            r"target=(gfx942(?::sramecc[+-])?:xnack-)" +
            (r" kfd_gpu_id=([1-9][0-9]*)" if backend == "kfd" else ""), line)
        if match is None or int(match[1], 16) == 0:
            raise ValueError("invalid observed endpoint")
        endpoint = {"visible_index": index, "unique_id": match[1],
                    "pci_bdf": match[2].lower(), "target": "gfx942:xnack-"}
        if backend == "kfd":
            endpoint["kfd_gpu_id"] = int(match[4])
            if endpoint["kfd_gpu_id"] >= 1 << 32:
                raise ValueError("invalid KFD GPU ID")
        result["endpoints"].append(endpoint)
    for key in ("unique_id", "pci_bdf"):
        if result["endpoints"][0][key] == result["endpoints"][1][key]:
            raise ValueError("aliased observed endpoint")
    if backend == "kfd":
        for line in lines[4:]:
            match = re.fullmatch(r"source_gpu_id=([1-9][0-9]*) "
                                 r"destination_gpu_id=([1-9][0-9]*) "
                                 r"engine_id=(0|[1-9][0-9]*)", line)
            if match is None or any(int(value) >= 1 << 32 for value in match.groups()):
                raise ValueError("invalid observed route")
            result["routes"].append(dict(zip(
                ("source_gpu_id", "destination_gpu_id", "engine_id"), map(int, match.groups()))))
    return result


def join_admission(physical, queries, *, inventory_sha256):
    """Physical rows must first pass the pinned raw idle/identity observer."""
    if type(physical) is not list or len(physical) != 2 or \
            type(queries) is not dict or set(queries) != {"kfd", "hip", "hsa"}:
        raise ValueError("exact independently observed pair required")
    observed = {backend: parse_query(raw, backend) for backend, raw in queries.items()}
    endpoints = []
    for index, row in enumerate(physical):
        if type(row) is not dict or set(row) != {"physical_index", "unique_id", "pci_bdf"}:
            raise ValueError("invalid physical observation roster")
        kfd = observed["kfd"]["endpoints"][index]
        if any(row[key] != kfd[key] for key in ("unique_id", "pci_bdf")):
            raise ValueError("physical inventory/KFD identity mismatch")
        endpoints.append({**row, "target": kfd["target"], "kfd_gpu_id": kfd["kfd_gpu_id"]})
    visibility = {}
    for backend in ("hip", "hsa"):
        visibility[backend] = []
        for index, endpoint in enumerate(endpoints):
            actual = observed[backend]["endpoints"][index]
            if any(actual[key] != endpoint[key] for key in ("unique_id", "pci_bdf", "target")):
                raise ValueError(f"{backend} API visibility/physical identity mismatch")
            visibility[backend].append({**endpoint, "visible_index": actual["visible_index"]})
    admission = {
        "schema": "fe2o3.xgmi-peer-series-admission-input.v1",
        "endpoints": endpoints,
        "routes": [{**route, "engine_control": "kfd-topology-admitted-directional-id"}
                   for route in observed["kfd"]["routes"]],
        "visibility": visibility,
        "evidence_sha256": {"physical_inventory": inventory_sha256,
                            **{name: hashlib.sha256(queries[backend]).hexdigest()
                               for name, backend in (("kfd_topology", "kfd"),
                                                     ("hip_visibility", "hip"),
                                                     ("hsa_visibility", "hsa"))}},
    }
    _admission(admission, [row["physical_index"] for row in physical],
               [row["unique_id"] for row in physical])
    return admission, {key: observed["kfd"][key] for key in ("boot_id", "generation")}
