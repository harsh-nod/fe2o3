"""Closed bridge-side checks for first-page CPU resource/source-variable queries.

This is a bounded client projection, not Rust admission, proof or source authority.
The existing console owns framing, lossless JSON and V1 session validation.
"""
import re
import unicodedata

from debug_console_protocol import ProtocolError, exact, identity, keys, require, uint

RESOURCE_REQUEST = "fe2o3-debug-resource-request-v1"
RESOURCE_RESPONSE = "fe2o3-debug-resource-response-v1"
VARIABLE_REQUEST = "fe2o3-debug-source-variable-request-v2"
VARIABLE_RESPONSE = "fe2o3-debug-source-variable-response-v2"
U64 = (1 << 64) - 1
VALUE_UNAVAILABLE = frozenset(("not_represented", "not_captured", "optimized_out",
    "outside_capture_scope", "not_in_scope", "not_live", "uninitialized", "truncated",
    "unsupported_by_backend", "requires_authenticated_map"))
SOURCE_UNAVAILABLE = frozenset(("source_map_v2_required", "variables_not_captured",
    "outside_capture_scope", "checkpoint_not_captured", "frame_unavailable", "name_not_in_scope"))
RESOURCE_UNAVAILABLE = frozenset(("no_selected_record", "not_checkpoint", "frame_limit",
    "value_limit", "allocation_limit", "memory_byte_limit", "allocation_failure", "not_captured"))
SPACES = frozenset(("global", "workgroup", "private", "constant", "generic"))


def closed(value, required, optional=()):
    require(type(value) is dict and set(required) <= set(value) <= set(required) | set(optional),
            "closed query object")


def bounded(value, maximum=U64, minimum=0):
    require(minimum <= uint(value) <= maximum, "query integer bound")
    return value


def enum(value, choices):
    require(type(value) is str and value in choices, "query enum")
    return value


def decimal(value):
    require(type(value) is str and re.fullmatch(r"0|[1-9][0-9]{0,19}", value),
            "query canonical decimal")
    result = int(value)
    bounded(result)
    return result


def text(value):
    require(type(value) is str and 0 < len(value.encode("utf8")) <= 256 and
            not any(unicodedata.category(character) == "Cc" for character in value), "query text")


def allocation(value, generation_zero=True):
    keys(value, ("ordinal", "generation"))
    bounded(value["ordinal"], minimum=1)
    bounded(value["generation"])
    if generation_zero:
        require(value["generation"] == 0, "CPU allocation generation")
    return (value["ordinal"], value["generation"])


def lane(value):
    keys(value, ("level", "workgroup", "wave", "lane", "logical_workitem",
                 "active_mask", "wave_width", "interpretation"))
    require(value["level"] == "lane" and value["interpretation"] == "logical_visualization",
            "concrete logical CPU lane")
    for name, maximum in (("workgroup", (1 << 32)-1), ("logical_workitem", U64)):
        require(type(value[name]) is list and len(value[name]) == 3, "three coordinates")
        for coordinate in value[name]:
            bounded(coordinate, maximum)
    bounded(value["wave"], (1 << 32)-1)
    width = bounded(value["wave_width"], 64, 1)
    require(width in (32, 64), "logical wave width")
    selected = bounded(value["lane"], width-1)
    mask = bounded(value["active_mask"], (1 << width)-1, 1)
    require(mask & (1 << selected) != 0, "selected active lane")


def kir_site(value):
    keys(value, ("function_ordinal", "block_ordinal", "point"))
    bounded(value["function_ordinal"])
    bounded(value["block_ordinal"])
    keys(value["point"], ("kind", "operation_ordinal"))
    require(value["point"]["kind"] == "operation", "captured operation site")
    bounded(value["point"]["operation_ordinal"])


def anchor(value, view, framed=False):
    keys(value, ("cursor", "scope", "site") + (("frame", "occurrence") if framed else ()))
    require(exact(value["cursor"], view["cursor"]), "query anchor cursor")
    require(uint(value["cursor"]["event_sequence"]) > 0, "nonzero selected event")
    lane(value["scope"])
    keys(value["site"], ("kir", "source"))
    kir_site(value["site"]["kir"])
    source = value["site"]["source"]
    require(type(source) is dict, "source location object")
    if source.get("status") == "resolved":
        keys(source, ("status", "location"))
        location = source["location"]
        keys(location, ("map_identity", "provenance", "file_identity", "byte_start", "byte_end"))
        identity(location["map_identity"]); identity(location["file_identity"])
        enum(location["provenance"], ("caller_bound", "compiler_bundle_bound"))
        require(uint(location["byte_start"]) < uint(location["byte_end"]), "source range")
    else:
        keys(source, ("status", "reason"))
        require(source["status"] == "unavailable", "source availability")
        enum(source["reason"], ("absent", "many_to_one", "requires_authenticated_map",
                               "not_represented", "optimized_out", "outside_capture_scope", "truncated"))
    if framed:
        require(type(value["frame"]) is int and value["frame"] == 1 and
                type(value["occurrence"]) is int and value["occurrence"] == 1, "legacy source frame")


def completeness(value):
    require(type(value) is dict, "capture completeness")
    if value.get("status") == "complete":
        keys(value, ("status",))
    else:
        closed(value, ("status", "reason", "emitted_events"), ("dropped_events",))
        require(value["status"] == "truncated", "capture completeness status")
        enum(value["reason"], ("event_limit", "byte_limit", "resident_limit", "producer_failure", "user_stopped"))
        bounded(value["emitted_events"])
        if "dropped_events" in value:
            bounded(value["dropped_events"])


def error_payload(value):
    keys(value, ("stage", "code", "message", "state_changed"))
    enum(value["stage"], ("framing", "protocol", "session", "backend", "output"))
    enum(value["code"], ("invalid_json", "invalid_request", "unsupported_schema", "stale_revision",
                        "invalid_state", "invalid_cursor", "resource_limit", "backend_failure",
                        "response_too_large", "output_failure"))
    text(value["message"])
    require(value["state_changed"] is False, "query error changed state")


def resource_response(response, request, selected, inventory):
    """Return freshly validated allocation rows, or None. Never follow next_token."""
    status = response["status"]
    common = ("schema", "status", "request_id", "operation", "session")
    if status == "error":
        keys(response, common + ("error",))
        error_payload(response["error"])
        return None
    if status == "unavailable":
        closed(response, common + ("reason", "completeness"), ("required",))
        reason = enum(response["reason"], RESOURCE_UNAVAILABLE)
        completeness(response["completeness"])
        capture_reason = reason not in ("no_selected_record", "not_checkpoint")
        require(capture_reason == ("required" in response), "resource unavailable extent")
        if capture_reason:
            decimal(response["required"])
        if request["operation"] == "query_memory_accesses":
            require(reason == "no_selected_record", "resource access unavailable reason")
        return None
    keys(response, common + ("snapshot", "page", "result", "physical_registers"))
    require(status == "ok" and exact(response["snapshot"], selected), "resource snapshot substitution")
    require(response["physical_registers"] == "not_represented", "physical register authority")
    page = response["page"]
    closed(page, ("source_count", "scanned", "completeness"), ("next_token",))
    total, scanned = bounded(page["source_count"]), bounded(page["scanned"], 64)
    require(scanned <= total and (scanned != 0 or total == 0), "resource scan extent")
    completeness(page["completeness"])
    if "next_token" in page:
        token = page["next_token"]
        require(type(token) is str and re.fullmatch(r"[A-Za-z0-9_.-]{1,128}", token) and
                0 < scanned < total, "opaque resource next token")
    result = response["result"]
    allocations = request["operation"] == "query_allocations"
    tag, field = ("allocations", "allocations") if allocations else ("memory_accesses", "accesses")
    keys(result, ("result", field))
    require(result["result"] == tag and type(result[field]) is list and
            len(result[field]) <= min(16, scanned), "resource result page")
    if allocations:
        identities = set()
        for row in result[field]:
            keys(row, ("allocation", "address_space", "access", "alignment", "capacity_bytes",
                       "snapshot_bytes_available", "initialization_available",
                       "owning_scope", "lifetime", "physical_base"))
            aid = allocation(row["allocation"])
            require(aid not in identities, "duplicate inventory allocation")
            identities.add(aid)
            require(row["address_space"] == "global", "global inventory filter")
            enum(row["access"], ("read_only", "write_only", "read_write"))
            alignment = bounded(row["alignment"], (1 << 32)-1, 1)
            require(alignment & (alignment-1) == 0, "allocation alignment")
            decimal(row["capacity_bytes"])
            require(row["snapshot_bytes_available"] is True and row["initialization_available"] is True,
                    "allocation capture flags")
            require(all(row[name] == "not_represented" for name in ("owning_scope", "lifetime", "physical_base")),
                    "allocation authority")
        return result[field]
    require(total <= selected["cursor"]["event_sequence"], "access prefix after selected snapshot")
    if page["completeness"]["status"] == "truncated":
        require(total <= page["completeness"]["emitted_events"], "access truncated prefix")
    expected = request["filter"]["allocation"]
    require(allocation(expected) in inventory, "current inventory membership")
    previous = 0
    for row in result[field]:
        keys(row, ("occurrence", "allocation", "range", "address_space", "access",
                   "call_frame", "operation_occurrence", "source_association"))
        allocation(row["allocation"])
        require(exact(row["allocation"], expected) and row["address_space"] == "global", "access filter substitution")
        enum(row["access"], ("read", "write_committed", "atomic_read",
                            "atomic_write_committed", "atomic_read_write_committed"))
        require(all(row[name] == "not_represented" for name in
                    ("call_frame", "operation_occurrence", "source_association")), "access authority")
        keys(row["range"], ("byte_offset", "byte_len"))
        offset, length = decimal(row["range"]["byte_offset"]), decimal(row["range"]["byte_len"])
        require(length > 0 and offset + length <= U64, "access range overflow")
        occurrence = row["occurrence"]
        keys(occurrence, ("record_ordinal", "event_sequence", "scope", "site", "schedule"))
        record, event = bounded(occurrence["record_ordinal"]), bounded(occurrence["event_sequence"], minimum=1)
        require(record < U64 and record+1 == event and previous < event <= total, "access occurrence order")
        previous = event
        lane(occurrence["scope"])
        require(occurrence["scope"]["wave_width"] == selected["scope"]["wave_width"], "access wave width")
        kir_site(occurrence["site"])
        keys(occurrence["schedule"], ("identity", "decision_ordinal"))
        enum(occurrence["schedule"]["identity"], ("workgroup_major_local_zyx_serial_v1",
             "workgroup_major_local_zyx_cooperative_v1", "workgroup_major_seeded_runnable_cooperative_v1"))
        bounded(occurrence["schedule"]["decision_ordinal"])
    return None


def hex_bytes(value, count):
    require(type(value) is str and len(value) == 2 + 2*count and
            re.fullmatch(r"0x[0-9a-f]*", value), "source value byte extent")


def value_availability(value):
    """Validate the bounded V1 value representation embedded by SourceVariableV2."""
    require(type(value) is dict, "source value availability")
    status = enum(value.get("status"), ("captured", "unavailable", "redacted"))
    if status != "captured":
        keys(value, ("status", "reason"))
        enum(value["reason"], VALUE_UNAVAILABLE if status == "unavailable" else ("native_address", "runtime_handle", "policy"))
        return status == "redacted" or value["reason"] in ("uninitialized", "not_live", "truncated")
    keys(value, ("status", "value_type", "value", "provenance"))
    require(value["provenance"] == "simulated_observation", "source value provenance")
    kind, data = value["value_type"], value["value"]
    require(type(kind) is dict and type(data) is dict, "source typed value")
    name = enum(kind.get("kind"), ("bool", "integer", "index", "float", "pointer", "bytes", "aggregate"))
    if name in ("bool", "integer", "index", "float"):
        keys(kind, ("kind",) if name == "bool" else ("kind", "signed", "bits") if name == "integer" else ("kind", "bits"))
        bits = 1 if name == "bool" else bounded(kind["bits"], 4096, 1)
        if name == "integer":
            require(type(kind["signed"]) is bool, "integer sign")
        if name == "index":
            require(bits in (32, 64), "index width")
        if name == "float":
            require(bits in (16, 32, 64), "float width")
        keys(data, ("encoding", "bits"))
        require(data["encoding"] == "bits" and type(data["bits"]) is str and
                re.fullmatch(r"0x[0-9a-f]+", data["bits"]) and len(data["bits"]) == 2+(bits+3)//4 and
                int(data["bits"][2:], 16) < (1 << bits), "source scalar bit encoding")
    elif name == "pointer":
        keys(kind, ("kind", "address_space"))
        enum(kind["address_space"], SPACES)
        keys(data, ("encoding", "allocation", "byte_offset"))
        require(data["encoding"] == "allocation_relative_pointer", "source pointer encoding")
        allocation(data["allocation"], generation_zero=False)
        bounded(data["byte_offset"])
    else:
        keys(kind, ("kind", "byte_len") if name == "bytes" else ("kind", "aggregate", "elements", "byte_len"))
        size = bounded(kind["byte_len"], 128*1024, 1)
        if name == "aggregate":
            enum(kind["aggregate"], ("struct", "tuple", "array", "slice"))
            bounded(kind["elements"])
        keys(data, ("encoding", "bytes", "initialized"))
        require(data["encoding"] == "bytes", "source byte encoding")
        hex_bytes(data["bytes"], size)
        hex_bytes(data["initialized"], (size+7)//8)
        if size % 8:
            require(int(data["initialized"][-2:], 16) >> (size % 8) == 0, "source initialization padding")
    return True


def variable_response(response, selected, frame):
    """Preserve returned next_cursor as an observation only; never construct a follow-up."""
    common = ("schema", "status", "request_id", "operation", "session")
    status = response["status"]
    if status == "error":
        keys(response, common + ("error",))
        error_payload(response["error"])
        return
    if status == "unavailable":
        keys(response, common + ("reason",))
        enum(response["reason"], SOURCE_UNAVAILABLE)
        return
    closed(response, common + ("snapshot", "values"), ("next_cursor",))
    require(status == "ok", "source response status")
    expected = {**selected, "frame": 1, "occurrence": 1}
    require(exact(response["snapshot"], expected), "source legacy frame refinement")
    anchor(response["snapshot"], response["session"], framed=True)
    values = response["values"]
    require(type(values) is list and len(values) <= 16, "source first page bound")
    identities = set()
    for row in values:
        keys(row, ("variable_identity", "name", "function_ordinal", "scope_identity",
                   "scope_depth", "generation", "availability"))
        identity(row["variable_identity"]); identity(row["scope_identity"]); text(row["name"])
        require(row["variable_identity"] not in identities, "duplicate source variable identity")
        identities.add(row["variable_identity"])
        bounded(row["function_ordinal"], (1 << 32)-1)
        require(row["function_ordinal"] == frame["function_ordinal"], "source caller function")
        bounded(row["scope_depth"], (1 << 32)-1); bounded(row["generation"])
        availability = row["availability"]
        require(type(availability) is dict, "variable availability")
        if availability.get("status") == "ambiguous":
            keys(availability, ("status",))
        else:
            keys(availability, ("status", "value"))
            require(availability["status"] == "value", "variable value status")
            if value_availability(availability["value"]):
                require(row["generation"] > 0, "source live storage generation")
    if "next_cursor" in response:
        cursor = response["next_cursor"]
        keys(cursor, ("query_identity", "position"))
        identity(cursor["query_identity"])
        require(type(cursor["position"]) is int and cursor["position"] == len(values) == 16,
                "source first-page cursor position")
