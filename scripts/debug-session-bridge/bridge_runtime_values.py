"""Exact bounded validators for additive CPU runtime observation replies.

These checks are a client projection, never source/proof/hardware authority.
Original schemas and canonical decimal identity strings are preserved.
"""
from debug_console_protocol import exact, keys, require, session_shape
from bridge_live_query_values import (U64, bounded, closed, completeness, decimal,
                                      enum, error_payload, lane)

RUNTIME_REQUEST = "fe2o3-debug-runtime-observation-request-v1"
RUNTIME_RESPONSE = "fe2o3-debug-runtime-observation-response-v1"
RESOURCE_REQUEST = "fe2o3-debug-resource-request-v2"
RESOURCE_RESPONSE = "fe2o3-debug-resource-response-v2"
UNAVAILABLE = frozenset(("not_requested", "policy_disabled", "no_selected_record",
    "not_checkpoint", "no_matching_operation", "aggregate_record", "legacy_stack_unavailable",
    "identity_invariant", "prefix_truncated", "invalid_join", "allocation_failure",
    "observation_stopped", "sequence_overflow", "not_captured", "work_limit"))
CUTOFF = frozenset(("row_limit", "frame_limit", "transition_limit", "byte_limit",
    "allocation_failure", "invalid_capacity", "validation_work_limit"))


def positive(value):
    result = decimal(value)
    require(result > 0, "nonzero runtime decimal identity")
    return result


def owner(value):
    keys(value, ("backend_session", "capture_instance"))
    positive(value["backend_session"])
    positive(value["capture_instance"])


def binding(value, view, expected_owner=None):
    keys(value, ("owner", "cursor"))
    owner(value["owner"])
    require(exact(value["cursor"], view["cursor"]), "exact runtime cursor")
    if expected_owner is not None:
        require(exact(value["owner"], expected_owner), "runtime owner changed")


def triple(value):
    keys(value, ("allocation", "storage_slot", "generation"))
    for name in ("allocation", "storage_slot", "generation"):
        positive(value[name])
    return tuple(value[name] for name in ("allocation", "storage_slot", "generation"))


def coordinates(value, *, strings):
    require(type(value) is list and len(value) == 3, "three runtime coordinates")
    return [decimal(item) if strings else bounded(item, (1 << 32)-1) for item in value]


def invocation(value):
    keys(value, ("global", "workgroup", "local", "workgroup_size", "workgroup_count", "launch_extent"))
    arrays = {name: coordinates(value[name], strings=name not in ("local", "workgroup_size"))
              for name in value}
    for axis in range(3):
        width, extent = arrays["workgroup_size"][axis], arrays["launch_extent"][axis]
        group, local = arrays["workgroup"][axis], arrays["local"][axis]
        require(width > 0 and extent > 0 and
                arrays["workgroup_count"][axis] == (extent + width - 1)//width and
                group < arrays["workgroup_count"][axis] and local < width, "runtime hierarchy bounds")
        global_index = group*width+local
        require(global_index <= U64 and global_index < extent and
                arrays["global"][axis] == global_index, "runtime hierarchy identity")
    return arrays


def invocation_lane(value, scope):
    arrays = invocation(value)
    lane(scope)
    require(scope["workgroup"] == arrays["workgroup"] and
            scope["logical_workitem"] == arrays["global"], "runtime lane hierarchy")
    x, y, z = arrays["local"]
    sx, sy, _sz = arrays["workgroup_size"]
    flat = (z*sy+y)*sx+x
    require(flat <= U64 and flat//scope["wave_width"] == scope["wave"] and
            flat % scope["wave_width"] == scope["lane"], "runtime logical lane")


def site(value, *, creation=False):
    closed(value, ("function_ordinal", "block"), ("operation",)) if creation else keys(
        value, ("function_ordinal", "block", "operation"))
    decimal(value["function_ordinal"])
    bounded(value["block"], (1 << 32)-1)
    if "operation" in value:
        bounded(value["operation"], (1 << 32)-1)


def origin(value):
    require(type(value) is dict, "runtime origin object")
    if value.get("availability") == "available":
        keys(value, ("availability", "identity"))
        identity = value["identity"]
        keys(identity, ("activation", "attempt", "site"))
        positive(identity["activation"]); positive(identity["attempt"]); site(identity["site"])
        return identity
    keys(value, ("availability", "reason"))
    require(value["availability"] == "unavailable", "runtime origin availability")
    enum(value["reason"], UNAVAILABLE)
    return None


def coverage(value, event, *, available=False):
    require(type(value) is dict, "runtime metadata coverage")
    kind = enum(value.get("coverage"), ("disabled", "complete", "prefix_truncated", "invalid_join"))
    if kind == "prefix_truncated":
        keys(value, ("coverage", "retained_records", "reason"))
        retained = decimal(value["retained_records"])
        enum(value["reason"], CUTOFF)
        if available:
            require(retained >= event, "runtime coverage must include selected record")
    else:
        keys(value, ("coverage",))
        if available:
            require(kind == "complete", "unavailable metadata cannot produce available rows")


def capture_completeness(value, event, *, selected=True):
    completeness(value)
    if selected and value["status"] == "truncated":
        require(value["emitted_events"] >= event, "capture prefix includes selected record")


def frames(value, observed_origin):
    require(type(value) is dict, "runtime frames object")
    if value.get("availability") == "unavailable":
        keys(value, ("availability", "reason")); enum(value["reason"], UNAVAILABLE)
        return False
    keys(value, ("availability", "frames"))
    require(value["availability"] == "captured" and type(value["frames"]) is list and
            0 < len(value["frames"]) <= 4096, "bounded complete runtime frame roster")
    seen = set()
    previous = None
    for depth, row in enumerate(value["frames"]):
        closed(row, ("legacy_depth", "function_ordinal", "block", "activation", "operation", "parent"),
               ("next_operation",))
        require(bounded(row["legacy_depth"], (1 << 32)-1) == depth, "runtime depth join coordinate")
        decimal(row["function_ordinal"]); bounded(row["block"], (1 << 32)-1)
        if "next_operation" in row:
            bounded(row["next_operation"], (1 << 32)-1)
        activation = positive(row["activation"])
        require(activation not in seen, "distinct active frame identity")
        seen.add(activation)
        operation = row["operation"]
        require(type(operation) is dict, "runtime frame operation")
        state = enum(operation.get("state"), ("ready", "active_operation", "suspended"))
        if state == "ready":
            keys(operation, ("state",))
        else:
            keys(operation, ("state", "attempt", "site"))
            positive(operation["attempt"]); site(operation["site"])
            require(operation["site"]["function_ordinal"] == row["function_ordinal"],
                    "frame operation function")
        parent = row["parent"]
        if depth == 0:
            require(exact(parent, {"parent": "root"}), "root has no fabricated caller")
        else:
            keys(parent, ("parent", "activation", "attempt", "call_site"))
            require(parent["parent"] == "caller", "runtime caller kind")
            positive(parent["activation"]); positive(parent["attempt"]); site(parent["call_site"])
            require(parent["activation"] == previous["activation"] and
                    positive(parent["activation"]) < activation and
                    exact(previous["operation"], {"state": "suspended", "attempt": parent["attempt"],
                                                  "site": parent["call_site"]}), "actual suspended caller")
        if depth+1 < len(value["frames"]):
            require(state == "suspended", "non-top frame must be suspended")
        previous = row
    if observed_origin is not None:
        require(previous["activation"] == observed_origin["activation"] and
                exact(previous["operation"], {"state": "active_operation",
                    "attempt": observed_origin["attempt"], "site": observed_origin["site"]}),
                "exact operation origin/top frame join")
    return True


def validate_runtime(response, request, view, selected=None):
    """Returns its validated owner/binding; no prior frame/value is ever filled in."""
    common = ("schema", "status", "request_id", "session")
    require(type(response) is dict and response.get("schema") == RUNTIME_RESPONSE and
            type(response.get("request_id")) is int and response["request_id"] == request["request_id"],
            "runtime response correlation")
    session_shape(response.get("session"))
    require(exact(response["session"], view), "runtime read changed full session")
    status = enum(response.get("status"), ("ok", "unavailable", "error"))
    if status == "error":
        keys(response, common+("error",)); error_payload(response["error"])
        return None
    capture_completeness(response.get("completeness"), view["cursor"]["event_sequence"],
                         selected=status == "ok")
    if status == "unavailable":
        closed(response, common+("completeness", "reason"), ("binding",))
        enum(response["reason"], UNAVAILABLE)
        observed = response.get("binding")
        if observed is not None:
            binding(observed, view, request.get("expected_owner"))
        else:
            require("binding" not in response and "expected_owner" not in request,
                    "no null or omitted expected owner binding")
        return observed
    keys(response, common+("binding", "invocation", "origin", "frames", "allocation_watermark",
                          "completeness", "origin_coverage", "frame_coverage", "lifecycle_coverage"))
    binding(response["binding"], view, request.get("expected_owner"))
    event = bounded(view["cursor"]["event_sequence"], minimum=1)
    invocation(response["invocation"])
    observed_origin = origin(response["origin"])
    captured_frames = frames(response["frames"], observed_origin)
    watermark = response["allocation_watermark"]
    if type(watermark) is dict and watermark.get("availability") == "available":
        keys(watermark, ("availability", "through_sequence"))
        decimal(watermark["through_sequence"])
        available_lifecycle = True
    else:
        keys(watermark, ("availability", "reason"))
        require(watermark["availability"] == "unavailable", "runtime lifecycle availability")
        enum(watermark["reason"], UNAVAILABLE)
        available_lifecycle = False
    coverage(response["origin_coverage"], event, available=observed_origin is not None)
    coverage(response["frame_coverage"], event, available=captured_frames)
    coverage(response["lifecycle_coverage"], event, available=available_lifecycle)
    if selected is not None:
        invocation_lane(response["invocation"], selected["scope"])
        if observed_origin is not None:
            legacy = selected["site"]["kir"]
            require(decimal(observed_origin["site"]["function_ordinal"]) == legacy["function_ordinal"]
                    and observed_origin["site"]["operation"] == legacy["point"]["operation_ordinal"],
                    "runtime origin/selected operation join")
            # Runtime block is a BlockId, NOT the legacy block_ordinal.
    return response["binding"]
