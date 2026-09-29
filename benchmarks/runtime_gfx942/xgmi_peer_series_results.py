#!/usr/bin/env python3
"""Strict trial-bound native directional-series parsing, separate from hot rows."""

import re

from xgmi_peer_hot_results import (
    _METRIC_KEYS, _U64_MAX, _bandwidth, _canonical_unique_ids, _positive_u64, _tokens,
)


PROFILE = "fe2o3.gfx942-xgmi-retained-pair-ordinary-lifetime.v1"
POLICY_SHA256 = "18cfe1c56d270d9cab1cdc2f67a2b26b35e7cf1540a2962e4dc2f5cb42155b61"
_COMMON = {
    "surface": "native-api", "measurement": "persistent-series",
    "direction": "forward-series-then-reverse-series", "prime_batches": "1",
    "lifetime_setup": "outside-samples", "lifetime_finish": "outside-samples",
    "validation": "final-readback", "canaries": "pass", "teardown": "explicit",
}
_NATIVE = {
    **_COMMON, "schema": "fe2o3.xgmi-peer-persistent-series-benchmark.v1",
    "mapping_lifetime": "process-persistent-hot",
    "engine_parallelism": "runtime-selected-unknown",
    "timing": "native-enqueue-through-observed-completion",
}
_CONSTANTS = {
    "hip": {
        **_NATIVE, "devices": "0,1", "peer_access": "enabled",
        "progress": "peer-async-then-stream-synchronize",
    },
    "hsa": {
        **_NATIVE, "gpu_indices": "0,1", "xnack": "disabled",
        "progress": "peer-async-then-signal-wait-reset",
    },
    "kfd": {
        **_COMMON, "schema": "fe2o3.xgmi-peer-retained-pair-series-benchmark.v1",
        "qualification_profile": PROFILE, "qualification_policy_sha256": POLICY_SHA256,
        "environment_assumption": "reviewed-mi300x-amdgpu61613-ordinary-lifetime",
        "target": "gfx942:xnack-", "mapping_lifetime": "directional-retained-pair",
        "engine_parallelism": "ordered-single-sdma", "peer_access": "topology-xgmi",
        "doorbells_per_batch": "1", "progress": "explicit-exact-roster-native-wait",
        "timing": "native-enqueue-through-paired-operational-completion",
        "operational_fences": "inside-samples", "scopes": "2",
    },
}
_SCOPE_KEYS = {f"{direction}_scope_{phase}_ns" for direction in ("forward", "reverse")
               for phase in ("entry", "finish")}


def _pair(values, *, minimum, distinct):
    if not isinstance(values, list) or len(values) != 2 or any(
        type(value) is not int or not minimum <= value <= (1 << 32) - 1 for value in values
    ) or (distinct and values[0] == values[1]):
        raise ValueError("expected a bounded exact pair")
    return values


def parse_result(raw: bytes, *, backend: str, unique_ids: list[str], copy_bytes: int,
                 depth: int, warmups: int, samples: int, kfd_gpu_ids=None,
                 kfd_engines=None) -> dict[str, str]:
    """Checks row identity only; environmental attestation is external evidence."""
    if not isinstance(backend, str) or backend not in _CONSTANTS:
        raise ValueError("unsupported backend")
    for value, minimum in ((copy_bytes, 1), (depth, 1), (warmups, 0), (samples, 1)):
        if type(value) is not int or not minimum <= value <= _U64_MAX:
            raise ValueError("trial controls must be bounded integers")
    if depth not in (1, 16, 32) or copy_bytes > 0x003F_FFE0:
        raise ValueError("unqualified matched shape")
    if warmups + samples >= _U64_MAX:
        raise ValueError("trial arithmetic overflows")
    constants = {
        **_CONSTANTS[backend], "backend": backend, "bytes": str(copy_bytes),
        "depth": str(depth), "outstanding_depth": str(depth),
        "warmups": str(warmups), "samples": str(samples),
        "forward_samples": str(samples), "reverse_samples": str(samples),
        "unique_ids": _canonical_unique_ids(unique_ids),
    }
    if backend == "kfd":
        gpu_ids = _pair(kfd_gpu_ids, minimum=1, distinct=True)
        engines = _pair(kfd_engines, minimum=0, distinct=False)
        constants.update(gpu_ids=f"{gpu_ids[0]},{gpu_ids[1]}",
                         forward_engine=str(engines[0]), reverse_engine=str(engines[1]),
                         queue_depth=str(depth), batch_size=str(depth))
    elif kfd_gpu_ids is not None or kfd_engines is not None:
        raise ValueError("KFD-only controls supplied to another backend")
    fields = _tokens(raw)
    expected = set(constants) | _METRIC_KEYS | (_SCOPE_KEYS if backend == "kfd" else {"targets"})
    if set(fields) != expected:
        raise ValueError("result field roster does not match the series producer")
    for key, value in constants.items():
        if fields[key] != value:
            raise ValueError(f"unexpected {key}")
    if backend == "hip":
        targets = fields["targets"].split(",")
        if len(targets) != 2 or targets[0] != targets[1] or re.fullmatch(
            r"gfx942:(?:sramecc[+-]:)?xnack-", targets[0]
        ) is None:
            raise ValueError("unexpected HIP targets")
    elif backend == "hsa" and fields["targets"] != "gfx942,gfx942":
        raise ValueError("unexpected HSA targets")
    if backend == "kfd":
        for key in _SCOPE_KEYS:
            _positive_u64(fields[key])
    for direction in ("forward", "reverse"):
        p50 = _positive_u64(fields[f"{direction}_p50_ns"])
        p95 = _positive_u64(fields[f"{direction}_p95_ns"])
        if p95 < p50:
            raise ValueError("p95 must not be less than p50")
        _bandwidth(fields[f"{direction}_p50_GBps"], p50, copy_bytes * depth)
    return fields
