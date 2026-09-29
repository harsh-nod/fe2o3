#!/usr/bin/env python3
"""Pure series trial planning/replay, not a launcher or native acceptance gate.

The caller authenticates source/binaries and all raw observation/command receipts,
enforces host admission and postflight, and collects before owned cleanup. The
digests below bind recorded inputs; matching them is not remote attestation or a
source-to-module proof. No environment assumption is inferred from gfx942 alone.
"""

import hashlib
import json
import re

from xgmi_peer_series_results import POLICY_SHA256, PROFILE, parse_result


DEPTHS = (1, 16, 32)
ORDER = ("kfd", "hsa", "hip", "hip", "hsa", "kfd")
BINARIES = {"kfd": "kfd-series", "hsa": "hsa-series", "hip": "hip-series"}
ASSUMPTION = "reviewed-mi300x-amdgpu61613-ordinary-lifetime"
_CLEAR_ENVIRONMENT = [
    "LD_PRELOAD", "LD_LIBRARY_PATH", "HIP_VISIBLE_DEVICES", "ROCR_VISIBLE_DEVICES",
    "CUDA_VISIBLE_DEVICES", "GPU_DEVICE_ORDINAL", "HSA_XNACK", "HSA_ENABLE_SDMA",
    "HSA_ENABLE_PEER_SDMA",
]
_EXCLUDED_CHANGES = [
    "administrative-repartition", "hive-reconfiguration", "hotplug",
    "privileged-criu", "foreign-same-process-kfd-drm-mutation",
]
_IDENTITY = {
    "kernel_release": "6.8.0-124-generic",
    "amdgpu_version": "6.16.13",
    "amdgpu_srcversion": "A6F143BEC60C0AFC3263226",
    "loaded_build_id": "4cd22e1f91450b8d9da1fc7bbbc02ee412e202d9",
    "installed_build_id": "4cd22e1f91450b8d9da1fc7bbbc02ee412e202d9",
    "module_compressed_sha256": "e5a327a8f46459e07ee3f59cc991d16feee17103e199d39149823879b7fcff0b",
    "module_elf_sha256": "61317154cee502ea97a74818879dff4b20abf8f074a2f4d19a94288e25d4ac3a",
    "package": "amdgpu-dkms",
    "package_version": "1:6.16.13.30300400-2341068.24.04",
    "source_tree": "amdgpu-6.16.13-2341068.24.04",
}
_SOURCES = {
    "include/uapi/linux/kfd_ioctl.h": "b3721c1a428a32bb9994af579432af48c44fa65abb860049f11a63a5c093235d",
    "amd/amdkfd/kfd_chardev.c": "f9a8805c5d479faee25e457051aa428e4bb523ecf1c7b1618a6a5f79ca5d7bba",
    "amd/amdkfd/kfd_process.c": "d76db8cbb546aa23dffb33b1d04244037e12246b49b752303194c68dd685e409",
    "amd/amdkfd/kfd_queue.c": "fb4b2a5c9e6981222873bcd7aca7e9c1397cba8f1a6b33634d2a48d4427fe062",
    "amd/amdkfd/kfd_process_queue_manager.c": "8526e258824dbe145e4209cf0fed26463729234ba24369f39e3413e7e6e028db",
    "amd/amdkfd/kfd_device_queue_manager.c": "d61e53a78c1855c4badefbebb6c6ec52702be8cfe072253341c277337641c682",
    "amd/amdkfd/kfd_device.c": "ccf20227c5cdd5b258758f50f61bbc1008a09ea776c101f035f83963e7d23037",
    "amd/amdkfd/kfd_smi_events.c": "2d786562fe1e97b8257841b755106c8bce47658a2aa3b439ce4e0178323004bd",
    "amd/amdkfd/kfd_events.c": "295114e5bacb3be94cdc17b6760e893198ee51d1c77d5837cfab999c3823485a",
    "amd/amdkfd/kfd_topology.c": "6a1453f8f70a9fba549694b71db132eb80679d7fbb8d0eb7af9dd8e7b669f802",
    "amd/amdkfd/kfd_priv.h": "f991330031c14725b2be0636ec1896ab530dc3d07d530ebd4f47efff97a82a99",
    "amd/amdgpu/amdgpu_amdkfd_gpuvm.c": "c7cca2ee47a08c99bb73906662d82dd7d0b5738468fbef54848e5e6dd62ba50d",
    "amd/amdgpu/amdgpu_amdkfd.c": "ce2d3a70928a267431313e5f0ad76ee2ebc5c8a724308e2cd89a7a67a1959c07",
    "amd/amdgpu/amdgpu_xgmi.c": "0cffaa04b5e6afc7167919da4685761bdb0fe0ceb9ff0ad271a5b50252f5869b",
    "amd/amdgpu/aqua_vanjaram.c": "76f5ee0b5fbc4b5ebe5886893f968dc8a2956c0b52cc6384c00570253b656758",
    "amd/amdgpu/amdgpu_gmc.c": "1e7d17878f0dfa57713fe76fe0d33bd3f231c3666d86b1937b6987cbc639cfa7",
    "amd/amdgpu/amdgpu_gfx.c": "f1b7435c593c08c4b0f4113a016d88e5b3ddbc729d8bdd3d15e61e8c731f27fe",
    "amd/amdgpu/amdgpu_drv.c": "414e5eb04a4ea17247c4596f452999340c3ea7e842c691cc41839d6cd3dff8da",
    "amd/amdgpu/amdgpu_device.c": "4d0edc4b714c005e911596e0e2e616be7fdbbb3526069938e4cc078eaba83673",
    "amd/amdgpu/amdgpu_vm.c": "123a1b8080bf32120610808ae85cbdf4b7b10df3ce33f5488f2598cf54df3349",
    "amd/amdgpu/amdgpu_amdkfd.h": "ddcf1aef9b4d7d26aaec36b97b2fde49452db3f0049ef4c90e57fa17c04cd7ca",
    "amd/amdxcp/amdgpu_xcp_drv.c": "fa4eefa6b24e033adf2f22ff14b742e347407d28ad05c3baa2ba1c09dc515f27",
}


def _roster(value, keys, name):
    if type(value) is not dict or set(value) != set(keys):
        raise ValueError(f"invalid {name} field roster")


def _integer(value, minimum, maximum, name):
    if type(value) is not int or not minimum <= value <= maximum:
        raise ValueError(f"invalid {name}")
    return value


def _digest(value):
    if type(value) is not str or re.fullmatch(r"[0-9a-f]{64}", value) is None:
        raise ValueError("invalid evidence digest")
    return value


def _json_digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"),
                                     allow_nan=False).encode("ascii")).hexdigest()


def validate_environment(observation):
    """Require the reviewed observation and acknowledgement, not attestation."""
    _roster(observation, ("schema", "identity", "source_sha256", "evidence_sha256",
                          "environment_assumption", "excluded_changes"), "environment")
    if observation["schema"] != "fe2o3.xgmi-peer-reviewed-environment-observation.v1" or \
            observation["identity"] != _IDENTITY or observation["source_sha256"] != _SOURCES or \
            observation["environment_assumption"] != ASSUMPTION or \
            observation["excluded_changes"] != _EXCLUDED_CHANGES:
        raise ValueError("environment differs from the reviewed ordinary-lifetime assumption")
    evidence = observation["evidence_sha256"]
    _roster(evidence, ("kernel", "loaded_module", "installed_module", "package_and_source"),
            "environment evidence")
    for digest in evidence.values():
        _digest(digest)
    return _json_digest(observation)


def _admission(admission, physical_indices, unique_ids):
    _roster(admission, ("schema", "endpoints", "routes", "visibility", "evidence_sha256"),
            "admission")
    if admission["schema"] != "fe2o3.xgmi-peer-series-admission-input.v1":
        raise ValueError("unexpected admission schema")
    if type(physical_indices) is not list or len(physical_indices) != 2 or \
            type(unique_ids) is not list or len(unique_ids) != 2:
        raise ValueError("exact selected pair required")
    for index in physical_indices:
        _integer(index, 0, (1 << 31) - 1, "physical index")
    if physical_indices[0] == physical_indices[1] or unique_ids[0] == unique_ids[1]:
        raise ValueError("selected pair must be distinct")
    for uid in unique_ids:
        if type(uid) is not str or re.fullmatch(r"[0-9a-f]{16}", uid) is None or int(uid, 16) == 0:
            raise ValueError("canonical nonzero physical UID required")
    endpoints = admission["endpoints"]
    if type(endpoints) is not list or len(endpoints) != 2:
        raise ValueError("exact endpoint roster required")
    for position, endpoint in enumerate(endpoints):
        _roster(endpoint, ("physical_index", "unique_id", "pci_bdf", "kfd_gpu_id", "target"),
                "endpoint")
        _integer(endpoint["physical_index"], 0, (1 << 31) - 1, "endpoint index")
        _integer(endpoint["kfd_gpu_id"], 1, (1 << 32) - 1, "KFD GPU ID")
        if endpoint["physical_index"] != physical_indices[position] or \
                endpoint["unique_id"] != unique_ids[position] or \
                endpoint["target"] != "gfx942:xnack-" or \
                type(endpoint["pci_bdf"]) is not str or \
                re.fullmatch(r"[0-9a-f]{4}:[0-9a-f]{2}:[0-1][0-9a-f]\.[0-7]",
                             endpoint["pci_bdf"]) is None:
            raise ValueError("selected physical endpoint mismatch")
    for key in ("pci_bdf", "kfd_gpu_id"):
        if endpoints[0][key] == endpoints[1][key]:
            raise ValueError("aliased endpoint identity")
    routes = admission["routes"]
    if type(routes) is not list or len(routes) != 2:
        raise ValueError("exact directional route roster required")
    for direction, route in enumerate(routes):
        _roster(route, ("source_gpu_id", "destination_gpu_id", "engine_id", "engine_control"),
                "route")
        for key in ("source_gpu_id", "destination_gpu_id"):
            _integer(route[key], 1, (1 << 32) - 1, key)
        _integer(route["engine_id"], 0, (1 << 32) - 1, "engine ID")
        if route["source_gpu_id"] != endpoints[direction]["kfd_gpu_id"] or \
                route["destination_gpu_id"] != endpoints[1 - direction]["kfd_gpu_id"] or \
                route["engine_control"] != "kfd-topology-admitted-directional-id":
            raise ValueError("directional engine route mismatch")
    visibility = admission["visibility"]
    _roster(visibility, ("hip", "hsa"), "visibility")
    for backend in ("hip", "hsa"):
        expected = [{**endpoint, "visible_index": index} for index, endpoint in enumerate(endpoints)]
        observed = visibility[backend]
        if type(observed) is not list or len(observed) != 2:
            raise ValueError(f"{backend} filtered index/physical identity mismatch")
        for actual, wanted in zip(observed, expected):
            _roster(actual, wanted, "visible endpoint")
            if any(actual[key] != value or type(actual[key]) is not type(value)
                   for key, value in wanted.items()):
                raise ValueError(f"{backend} filtered index/physical identity mismatch")
    evidence = admission["evidence_sha256"]
    _roster(evidence, ("physical_inventory", "kfd_topology", "hip_visibility", "hsa_visibility"),
            "admission evidence")
    for digest in evidence.values():
        _digest(digest)
    return endpoints, routes


def trial_specs(*, physical_indices, unique_ids, admission, environment_before,
                copy_bytes=1048576, warmups=2, samples=10):
    """Return fixed 18-trial specs; caller must bind them to authenticated ELF paths."""
    environment_digest = validate_environment(environment_before)
    endpoints, routes = _admission(admission, physical_indices, unique_ids)
    _integer(copy_bytes, 1, 0x003F_FFE0, "copy size")
    _integer(warmups, 0, (1 << 64) - 1, "warmup count")
    _integer(samples, 1, (1 << 64) - 1, "sample count")
    if warmups + samples >= (1 << 64) - 1:
        raise ValueError("trial arithmetic overflow")
    visibility = ",".join(map(str, physical_indices))
    uid_arguments = [f"0x{uid}" for uid in unique_ids]
    trials = []
    for depth in DEPTHS:
        for position, backend in enumerate(ORDER):
            shape = [str(copy_bytes), str(depth), str(warmups), str(samples)]
            environment = {"HSA_XNACK": "0"}
            if backend == "kfd":
                arguments = [*uid_arguments, *shape, "--retained-pair-series-reviewed-mi300x"]
            else:
                arguments = ["0", "1", *shape, *uid_arguments, "--persistent-series"]
                environment["HIP_VISIBLE_DEVICES" if backend == "hip" else "ROCR_VISIBLE_DEVICES"] = visibility
            trials.append({
                "name": f"d{depth:02d}-t{position + 1:02d}-{backend}", "backend": backend,
                "depth": depth, "binary": BINARIES[backend], "arguments": arguments,
                "clear_environment": list(_CLEAR_ENVIRONMENT), "environment_overrides": environment,
            })
    return {
        "schema": "fe2o3.xgmi-peer-series-plan.v1",
        "claim": "input-and-receipt-consistency-only",
        "qualification_profile": PROFILE, "qualification_policy_sha256": POLICY_SHA256,
        "environment_before_sha256": environment_digest, "admission_sha256": _json_digest(admission),
        "physical_indices": list(physical_indices), "unique_ids": list(unique_ids),
        "kfd_gpu_ids": [endpoint["kfd_gpu_id"] for endpoint in endpoints],
        "kfd_engines": [route["engine_id"] for route in routes],
        "copy_bytes": copy_bytes, "warmups": warmups, "samples": samples, "trials": trials,
        "engine_matching": "not-claimed-hip-hsa-runtime-selected-unknown",
        "timing_scope": "native-api-not-runtime-facade",
        "payload_staging": "outside-samples-persistent-pattern-final-readback-only",
        "kfd_scope_entry_finish": "outside-samples",
        "kfd_operational_fences": "inside-samples",
    }


def replay_campaign(records, *, environment_after, **inputs):
    """Validate complete command/row joins; never claims process or hardware safety."""
    plan = trial_specs(**inputs)
    environment_after_digest = validate_environment(environment_after)
    if type(records) is not list or len(records) != len(plan["trials"]):
        raise ValueError("campaign requires every ordered trial")
    seen = set()
    parsed = []
    for spec, record in zip(plan["trials"], records):
        _roster(record, (*spec.keys(), "returncode", "stdout", "stderr", "execution_receipt_sha256"),
                "trial receipt")
        for key, value in spec.items():
            if record[key] != value or type(record[key]) is not type(value):
                raise ValueError(f"trial command/order mismatch: {key}")
        if type(record["returncode"]) is not int or record["returncode"] != 0 or \
                type(record["stdout"]) is not bytes or type(record["stderr"]) is not bytes or \
                record["stderr"]:
            raise ValueError("unsuccessful or malformed trial output")
        execution = _digest(record["execution_receipt_sha256"])
        if execution in seen:
            raise ValueError("duplicate execution receipt")
        seen.add(execution)
        backend = spec["backend"]
        native = {"kfd_gpu_ids": plan["kfd_gpu_ids"], "kfd_engines": plan["kfd_engines"]} \
            if backend == "kfd" else {}
        fields = parse_result(record["stdout"], backend=backend,
                              unique_ids=[f"0x{uid}" for uid in plan["unique_ids"]],
                              copy_bytes=plan["copy_bytes"], depth=spec["depth"],
                              warmups=plan["warmups"], samples=plan["samples"], **native)
        parsed.append({"name": spec["name"], "fields": fields,
                       "stdout_sha256": hashlib.sha256(record["stdout"]).hexdigest(),
                       "execution_receipt_sha256": execution})
    return {"schema": "fe2o3.xgmi-peer-series-consistency-replay.v1",
            "claim": "input-and-receipt-consistency-only", "plan_sha256": _json_digest(plan),
            "environment_after_sha256": environment_after_digest, "trials": parsed}
