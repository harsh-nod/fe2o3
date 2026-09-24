"""Closed V20 CPU observation fields; not a canonical decoder or source admission."""
import hashlib
import re
from debug_console_protocol import exact, identity, keys, require, session_shape, uint

SCHEMA = "fe2o3-physical-cpu-bridge-v20"
RECORDS = 8192
RESPONSE_BYTES = 65536
NAMES = ("hierarchy_inspection", "kir_sites", "source_sites", "call_stack", "breakpoints",
         "watchpoints", "forward_step", "reverse_step", "pause", "deterministic_replay",
         "kir_ssa_values", "source_variable_values", "register_values",
         "allocation_relative_memory", "semantic_trace", "hardware_wave_state", "kfd_dispatch_control")
AVAILABLE = {"kir_sites", "forward_step", "reverse_step", "kir_ssa_values", "allocation_relative_memory"}


def same(a, b):
    require(exact(a, b), "physical observation identity or fields disagree")


def bounded(value, maximum):
    require(uint(value) <= maximum, "physical observation integer bound")
    return value


def session(value):
    session_shape(value)
    sequence = bounded(value["cursor"]["event_sequence"], RECORDS + 1)
    same(value["state"], "created" if sequence == 0 else "stopped")
    return sequence


def capabilities(value):
    keys(value, ("result", "capabilities"))
    same(value["result"], "capabilities")
    expected = [{"name": name, "availability": "available"} if name in AVAILABLE else
                {"name": name, "availability": "unavailable", "reason":
                 "requires_authenticated_map" if name in ("source_sites", "source_variable_values")
                 else "not_exposed_by_backend"} for name in NAMES]
    same(value["capabilities"], expected)


def stop(value, sequence, end):
    same(value, {"reason": "completed" if sequence == end else "entry" if sequence == 0 else "step",
                 "outcome": "completed" if sequence == end else "active", "exact": True})


def vector(value):
    require(type(value) is list and len(value) == 3, "physical coordinate")
    for n in value:
        bounded(n, (1 << 32) - 1)


def anchor(value, view):
    keys(value, ("cursor", "scope", "site"))
    same(value["cursor"], view["cursor"])
    scope = value["scope"]
    keys(scope, ("level", "workgroup", "wave", "lane", "logical_workitem", "active_mask",
                 "wave_width", "interpretation"))
    same(scope["level"], "lane")
    same(scope["wave"], 0)
    same(scope["wave_width"], 64)
    same(scope["interpretation"], "logical_visualization")
    same(scope["active_mask"], (1 << 64) - 1)  # Residency, NOT authored physical EXEC.
    vector(scope["workgroup"])
    vector(scope["logical_workitem"])
    lane = bounded(scope["lane"], 63)
    same(scope["workgroup"][1:], [0, 0])
    same(scope["logical_workitem"], [scope["workgroup"][0] * 64 + lane, 0, 0])
    keys(value["site"], ("kir", "source"))
    same(value["site"]["source"], {"status": "unavailable", "reason": "requires_authenticated_map"})
    site = value["site"]["kir"]
    keys(site, ("function_ordinal", "block_ordinal", "point"))
    same(site["function_ordinal"], 0)
    bounded(site["block_ordinal"], (1 << 32) - 1)
    keys(site["point"], ("kind", "operation_ordinal"))
    same(site["point"]["kind"], "operation")
    bounded(site["point"]["operation_ordinal"], 4096)
    return value


def snapshot(value, view, end):
    if type(value) is dict and value.get("status") == "unavailable":
        same(value, {"status": "unavailable", "reason": "not_captured"})
        return None
    keys(value, ("status", "snapshot"))
    same(value["status"], "captured")
    data = value["snapshot"]
    keys(data, ("anchor", "stop", "values"))
    sequence = view["cursor"]["event_sequence"]
    require(0 < sequence < (end if end is not None else RECORDS + 1), "capture event range")
    stop(data["stop"], sequence, end)
    same(data["values"], [])
    return anchor(data["anchor"], view)


def scope_for(selected):
    scope = selected["scope"]
    return {"level": "lane", "workgroup": scope["workgroup"][:], "wave": 0, "lane": scope["lane"]}


def query_identity(view):
    return hashlib.sha256(b"fe2o3-debug-physical-v20-ssa-page-v1\0" +
        bytes.fromhex(view["configuration_identity"]) +
        view["cursor"]["event_sequence"].to_bytes(8, "little") +
        view["revision"].to_bytes(8, "little")).hexdigest()


def fixed_bytes(value, count):
    require(type(value) is str and len(value) == 2 + count * 2 and
            re.fullmatch(r"0x[0-9a-f]*", value), "fixed-width physical bytes")
    return bytes.fromhex(value[2:])


def allocation(value):
    same(value, {"ordinal": 1, "generation": 0})  # Only V20's one logical output allocation.


def values(result, request, view, selected):
    fields = ("result", "snapshot", "values")
    keys(result, fields + (("next_cursor",) if "next_cursor" in result else ()))
    same(result["result"], "values")
    same(anchor(result["snapshot"], view), selected)
    same(request["scope"], scope_for(selected))
    same(request["frame"], 1)
    same(request["selector"], {"selector": "all"})
    page = request["page"]
    limit = bounded(page["limit"], 64)
    require(limit > 0, "positive physical page")
    query = query_identity(view)
    start = 0
    if "cursor" in page:
        keys(page["cursor"], ("query_identity", "position"))
        same(page["cursor"]["query_identity"], query)
        start = bounded(page["cursor"]["position"], 65536)
    rows = result["values"]
    require(type(rows) is list and len(rows) <= limit, "bounded physical SSA page")
    seen = set()
    for row in rows:
        keys(row, ("path", "availability"))
        keys(row["path"], ("root", "components"))
        same(row["path"]["components"], [])
        root = row["path"]["root"]
        keys(root, ("kind", "function_ordinal", "frame", "value_ordinal"))
        same(root["kind"], "ssa")
        same(root["function_ordinal"], 0)
        same(root["frame"], 1)
        ordinal = bounded(root["value_ordinal"], (1 << 32) - 1)
        require(ordinal not in seen, "repeated physical SSA value")
        seen.add(ordinal)
        data = row["availability"]
        if type(data) is dict and data.get("status") == "unavailable":
            same(data, {"status": "unavailable", "reason": "not_represented"})
            continue
        keys(data, ("status", "value_type", "value", "provenance"))
        same(data["status"], "captured")
        same(data["provenance"], "simulated_observation")
        ty, value = data["value_type"], data["value"]
        require(type(ty) is dict, "physical SSA type")
        if ty.get("kind") == "pointer":
            same(ty, {"kind": "pointer", "address_space": "global"})
            keys(value, ("encoding", "allocation", "byte_offset"))
            same(value["encoding"], "allocation_relative_pointer")
            allocation(value["allocation"])
            uint(value["byte_offset"])
        else:
            if ty.get("kind") == "bool":
                same(ty, {"kind": "bool"})
                width = 1
            else:
                keys(ty, ("kind", "signed", "bits"))
                same(ty["kind"], "integer")
                same(ty["signed"], False)
                require(type(ty["bits"]) is int and ty["bits"] in (32, 64), "physical SSA width")
                width = ty["bits"] // 8
            keys(value, ("encoding", "bits"))
            same(value["encoding"], "bits")
            if ty["kind"] == "bool":
                require(value["bits"] in ("0x0", "0x1"), "physical bool encoding")
            else:
                fixed_bytes(value["bits"], width)
    next_cursor = result.get("next_cursor")
    if next_cursor is not None:
        keys(next_cursor, ("query_identity", "position"))
        same(next_cursor["query_identity"], query)
        same(next_cursor["position"], start + len(rows))
        require(len(rows) == limit and len(rows) > 0, "progressing physical page")
        bounded(next_cursor["position"], 65536)
    return next_cursor


def memory(result, request, view, selected):
    keys(result, ("result", "snapshot", "memory"))
    same(result["result"], "memory")
    same(anchor(result["snapshot"], view), selected)
    data = result["memory"]
    keys(data, ("allocation", "byte_offset", "requested_bytes", "returned_bytes", "availability"))
    allocation(data["allocation"])
    same(data["allocation"], request["allocation"])
    same(data["byte_offset"], request["byte_offset"])
    same(data["requested_bytes"], request["byte_len"])
    same(data["returned_bytes"], request["byte_len"])
    count = bounded(data["returned_bytes"], 256)
    require(count > 0, "physical memory length")
    available = data["availability"]
    keys(available, ("status", "address_space", "bytes", "initialized", "truncated"))
    same(available["status"], "captured")
    same(available["address_space"], "global")
    same(available["truncated"], False)
    fixed_bytes(available["bytes"], count)
    bits = fixed_bytes(available["initialized"], (count + 7) // 8)
    if count % 8:
        require(bits[-1] >> (count % 8) == 0, "physical initialization padding")


def refusal(response, operation):
    if response["status"] == "error":
        value = response["error"]
        keys(value, ("stage", "code", "message", "state_changed"))
        same(value["stage"], "session")
        same(value["state_changed"], False)
        codes = {"invalid_cursor", "stale_revision", "resource_limit", "response_too_large", "invalid_state"}
        require(value["code"] in codes and type(value["message"]) is str and
                len(value["message"]) <= 160 and
                re.fullmatch(r"kir_v20_debug_[a-z_]+", value["message"]), "versioned physical refusal")
    else:
        value = response["unavailable"]
        keys(value, ("capability", "reason", "state_changed", "detail"))
        same(value["state_changed"], False)
        same(value["detail"], "diagnostic physical-entry V20 exposes bounded CPU observations only")
        expected = {"step": "forward_step", "seek": "forward_step",
                    "inspect_values": "kir_ssa_values", "read_memory": "allocation_relative_memory"}
        same(value["capability"], expected.get(operation))
        require(value["reason"] in ("outside_capture_scope", "not_exposed_by_backend", "truncated"),
                "physical unavailable reason")
