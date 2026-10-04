"""Closed same-owner bundle-declared target replies; never hardware detection."""
from debug_console_protocol import encode, exact, identity, keys, require, session_shape
from bridge_live_query_values import bounded, enum, error_payload
from bridge_runtime_values import binding, positive

TARGET_REQUEST = "fe2o3-debug-target-request-v1"
TARGET_RESPONSE = "fe2o3-debug-target-response-v1"
TARGET_BYTES = 4096


def validate_target(response, request, view):
    common = ("schema", "status", "operation", "request_id", "session")
    require(type(response) is dict and response.get("schema") == TARGET_RESPONSE and
            response.get("operation") == "inspect_declared_target" and
            type(response.get("request_id")) is int and
            response["request_id"] == request["request_id"], "declared target correlation")
    require(len(encode(response)) + 1 <= TARGET_BYTES, "declared target response cap")
    session_shape(response.get("session"))
    require(exact(response["session"], view), "target read changed full session")
    status = enum(response.get("status"), ("ok", "error"))
    if status == "error":
        keys(response, common + ("error",))
        error_payload(response["error"])
        return
    keys(response, common + ("binding", "logical_wave_width", "target"))
    binding(response["binding"], view, request["expected_binding"]["owner"])
    require(exact(response["binding"], request["expected_binding"]), "exact target owner/cursor")
    require(bounded(response["logical_wave_width"], 64) in (32, 64), "logical CPU wave width")
    target = response["target"]
    require(type(target) is dict, "declared target observation")
    if target.get("availability") == "unavailable":
        keys(target, ("availability", "reason"))
        require(target["reason"] == "raw_input_has_no_declared_gpu_target", "raw target absence")
        return
    keys(target, ("availability", "target", "provenance", "envelope_version",
                  "envelope_identity", "subject_identity", "admitted_module"))
    require(target["availability"] == "declared" and
            target["provenance"] == "verified_simulation_bundle", "content declaration only")
    enum(target["target"], ("gfx942:xnack-", "gfx950:xnack-"))
    version = bounded(target["envelope_version"], 6, 1)
    identity(target["envelope_identity"]); identity(target["subject_identity"])
    module = target["admitted_module"]
    keys(module, ("wire_version", "sha256", "canonical_bytes"))
    require(bounded(module["wire_version"], 11) == (7 if version <= 4 else 10 if version == 5 else 11),
            "exact envelope/module version")
    identity(module["sha256"]); positive(module["canonical_bytes"])
