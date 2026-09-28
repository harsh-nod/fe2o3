"""Independent profile consistency/lifecycle checks, not native authentication."""
import hashlib
import json
import re

UID = 0xab83d2ffef0d3cdf
UNAVAILABLE = ["rocprofv3_dispatch_correlation", "device_clock_timestamps", "device_copy_engine_events",
               "hardware_counters", "pc_samples", "decoded_att_events", "source_ir_isa_correlation",
               "semantic_execution_history"]
FIELDS = {
    "native_queue_created": ["queue"], "native_queue_destroyed": ["queue"],
    "stream_created": ["stream"], "stream_destroyed": ["stream"],
    "allocation_created": ["allocation", "memory_kind", "byte_len", "alignment"],
    "allocation_released": ["allocation"],
    "host_write": ["allocation", "byte_offset", "content"],
    "host_read": ["allocation", "byte_offset", "content"],
    "module_loaded": ["module", "artifact"], "module_unloaded": ["module"],
    "kernel_resolved": ["kernel", "module", "name", "signature"],
    "dispatch_published": ["dispatch", "queue", "stream", "kernel", "dispatch_shape", "launch", "bindings"],
    "dispatch_completed": ["dispatch", "host_timing"], "submission_released": ["dispatch"],
}
TIMINGS = ["preparation_ns", "bound_snapshot_ns", "authority_ns", "native_binding_ns", "publication_ns",
           "publish_to_completion_ns", "completed_readback_ns", "recycle_ns"]


def need(value, message):
    if not value:
        raise ValueError(message)


def same(a, b):
    return json.dumps(a, sort_keys=True) == json.dumps(b, sort_keys=True)


def uint(value, bits=64):
    need(type(value) is int and 0 <= value < 2**bits, "unsigned integer")
    return value


def identity(value):
    need(isinstance(value, str) and re.fullmatch("[0-9a-f]{64}", value) and value != "0" * 64,
         "nonzero canonical identity")
    return value


def domain(kind, parts):
    data = ("fe2o3.kfd-runtime-profile." + kind + ".v1\0").encode()
    return hashlib.sha256(data + b"".join(len(p).to_bytes(8, "little") + p for p in parts)).hexdigest()


def content_sha(length, digest):
    return {"digest": domain("content-claim", [length.to_bytes(8, "little"), bytes.fromhex(digest)]),
            "byte_len": length}


def content(data):
    return content_sha(len(data), hashlib.sha256(data).hexdigest())


def resource(scope, tag, handle):
    return domain("resource", [bytes.fromhex(scope), bytes([tag]), uint(handle).to_bytes(8, "little")])


def event_identity(scope, sequence, event):
    return domain("event", [bytes.fromhex(scope), sequence.to_bytes(8, "little"),
                            json.dumps(event, separators=(",", ":"), ensure_ascii=False).encode()])


def claim(value):
    need(type(value) is dict and list(value) == ["digest", "byte_len"], "content claim schema")
    identity(value["digest"])
    uint(value["byte_len"])


def validate(value):
    need(type(value) is dict and set(value) == {"schema", "schema_version", "capture_scope", "device",
         "host_content_mode", "events", "coverage", "unavailable"}, "profile schema keys")
    need(value["schema"] == "fe2o3-kfd-runtime-profile-v1" and same(value["schema_version"], 1), "profile version")
    scope = identity(value["capture_scope"])
    expected_device = {"identity": domain("device", [UID.to_bytes(8, "little"), b"gfx942:xnack-", (64).to_bytes(2, "little")]),
                       "target_profile": "gfx942:xnack-", "wave_width": 64}
    need(same(value["device"], expected_device), "exact selected profile device")
    need(value["host_content_mode"] == "range_only" and value["unavailable"] == UNAVAILABLE, "profile truth boundary")
    events = value["events"]
    need(type(events) is list and 0 < len(events) <= 16384, "bounded event list")
    need(same(value["coverage"], {"origin": "observed", "observed_events": len(events), "dropped_events": 0,
                                 "complete_runtime_operation_history": True}), "complete observed history")
    groups = {kind: [] for kind in FIELDS}
    for sequence, row in enumerate(events):
        need(type(row) is dict and set(row) == {"sequence", "identity", "origin", "event"}, "event row schema")
        need(same(row["sequence"], sequence) and row["origin"] == "observed", "observed contiguous events")
        event = row["event"]
        need(type(event) is dict and event.get("kind") in FIELDS, "known event kind")
        kind = event["kind"]
        need(list(event) == ["kind", *FIELDS[kind]], "canonical event schema")
        need(row["identity"] == event_identity(scope, sequence, event), "event content identity")
        for field in ("queue", "stream", "allocation", "module", "kernel", "dispatch"):
            if field in event:
                identity(event[field])
        for field in ("artifact", "name", "signature", "dispatch_shape"):
            if field in event:
                claim(event[field])
        if kind == "dispatch_completed":
            need(type(event["host_timing"]) is dict and list(event["host_timing"]) == TIMINGS, "host timing schema")
            for number in event["host_timing"].values():
                uint(number)
        groups[kind].append(row)

    def index(kind, field):
        rows = groups[kind]
        result = {row["event"][field]: row for row in rows}
        need(len(result) == len(rows), "unique " + kind)
        return result

    maps = {kind: index(kind, FIELDS[kind][0]) for kind in FIELDS if kind not in ("host_read", "host_write")}
    created_ids = [key for kind in ("native_queue_created", "stream_created", "allocation_created", "module_loaded",
                                    "kernel_resolved", "dispatch_published") for key in maps[kind]]
    need(len(set(created_ids)) == len(created_ids), "globally distinct resource identities")
    for start, end in (("native_queue_created", "native_queue_destroyed"), ("stream_created", "stream_destroyed"),
                       ("allocation_created", "allocation_released"), ("module_loaded", "module_unloaded"),
                       ("dispatch_published", "dispatch_completed"), ("dispatch_published", "submission_released")):
        need(maps[start].keys() == maps[end].keys(), "exact lifecycle closure: " + start)
        for key in maps[start]:
            need(maps[start][key]["sequence"] < maps[end][key]["sequence"], "ordered lifecycle: " + start)
    for kernel in maps["kernel_resolved"].values():
        module = kernel["event"]["module"]
        need(module in maps["module_loaded"], "resolved loaded module")
        need(maps["module_loaded"][module]["sequence"] < kernel["sequence"]
             < maps["module_unloaded"][module]["sequence"], "kernel lifetime")
    for allocation in maps["allocation_created"].values():
        event = allocation["event"]
        need(event["memory_kind"] == "host_visible" and uint(event["byte_len"]) > 0
             and uint(event["alignment"]) > 0, "host allocation extent")
    for kind in ("host_read", "host_write"):
        for row in groups[kind]:
            event, seq = row["event"], row["sequence"]
            key = event["allocation"]
            need(key in maps["allocation_created"], "host access allocation")
            allocation = maps["allocation_created"][key]
            need(allocation["sequence"] < seq < maps["allocation_released"][key]["sequence"], "host access lifetime")
            need(type(event["content"]) is dict and list(event["content"]) == ["state", "byte_len"]
                 and event["content"]["state"] == "range_only", "host range schema")
            need(uint(event["byte_offset"]) + uint(event["content"]["byte_len"]) <= allocation["event"]["byte_len"],
                 "host access range")
    for key, pub in maps["dispatch_published"].items():
        event, seq = pub["event"], pub["sequence"]
        launch = event["launch"]
        need(type(launch) is dict and list(launch) == ["grid", "workgroup", "dynamic_shared_bytes"], "canonical launch schema")
        for dimensions in (launch["grid"], launch["workgroup"]):
            need(type(dimensions) is list and len(dimensions) == 3 and all(uint(n, 32) > 0 for n in dimensions), "launch dimensions")
        uint(launch["dynamic_shared_bytes"], 32)
        completed = maps["dispatch_completed"][key]["sequence"]
        released = maps["submission_released"][key]["sequence"]
        need(completed < released, "completion before submission release")
        for field, start, end in (("queue", "native_queue_created", "native_queue_destroyed"),
                                  ("stream", "stream_created", "stream_destroyed")):
            ref = event[field]
            need(ref in maps[start], "dispatch resource exists")
            need(maps[start][ref]["sequence"] < seq < completed < maps[end][ref]["sequence"], "dispatch resource lifetime")
        need(released < maps["native_queue_destroyed"][event["queue"]]["sequence"], "submission before queue destroy")
        need(event["kernel"] in maps["kernel_resolved"], "dispatch resolved kernel")
        kernel = maps["kernel_resolved"][event["kernel"]]
        need(kernel["sequence"] < seq and completed < maps["module_unloaded"][kernel["event"]["module"]]["sequence"],
             "dispatch module lifetime")
        need(type(event["bindings"]) is list and event["bindings"], "nonempty bindings")
        for binding in event["bindings"]:
            need(type(binding) is dict and list(binding) == ["allocation", "access", "byte_offset", "byte_len", "kernarg_byte_offset"],
                 "binding schema")
            allocation = maps["allocation_created"].get(binding["allocation"])
            need(allocation is not None and allocation["sequence"] < seq
                 and completed < maps["allocation_released"][binding["allocation"]]["sequence"], "binding lifetime")
            need(binding["access"] in ("read", "write", "read_write") and uint(binding["byte_len"]) > 0
                 and uint(binding["byte_offset"]) + binding["byte_len"] <= allocation["event"]["byte_len"], "binding extent")
            uint(binding["kernarg_byte_offset"], 32)
    return groups, maps


ARTIFACTS = {
    "Long": content_sha(4592, "642f08b1ce18f6c3428d3dfac4c9e567b3d23c11cec0849ab47a6de8a06274a9"),
    "Short": content_sha(4600, "b3cf15845879f968b3e3b09f4a5f4bdd72d1f77466a0c048d9727da066719ee5"),
}
SIGNATURE = content(bytes.fromhex("ff196cd47ea25cd558f9e6fa8da4e895380a23f5ff55056a78ca0e456c4e20d1"))


def mixed(value, mode):
    variants = {"owner": ["Long", "Short"], "timeout": ["Long"], "drop": ["Long"],
                "backpressure": ["Short", "Long"]}[mode]
    count = len(variants)
    need(value["capture_scope"] == ("78" if mode == "owner" else "79") * 32, "mixed capture scope")
    groups, maps = validate(value)
    queues = 2 if mode == "owner" else 1
    for kind, rows in groups.items():
        expected = queues if kind.startswith("native_queue_") else count + (kind == "host_read" and mode == "backpressure")
        need(len(rows) == expected, "exact mixed event count: " + kind)
    publications = groups["dispatch_published"]
    resources = {field: [] for field in ("allocation", "kernel", "module", "stream", "dispatch")}
    for variant, pub in zip(variants, publications):
        event = pub["event"]
        kernel = maps["kernel_resolved"][event["kernel"]]["event"]
        module = maps["module_loaded"][kernel["module"]]["event"]
        need(same(module["artifact"], ARTIFACTS[variant]) and same(kernel["name"], content(("mixed_" + variant.lower()).encode()))
             and same(kernel["signature"], SIGNATURE), "exact variant artifact/name/signature")
        need(same(event["launch"], {"grid": [64, 1, 1], "workgroup": [64, 1, 1], "dynamic_shared_bytes": 0}), "mixed geometry")
        need(same(event["dispatch_shape"]["byte_len"], 32), "shape claim extent, not its preimage")
        need(len(event["bindings"]) == 1, "one whole allocation")
        allocation = event["bindings"][0]["allocation"]
        need(same(event["bindings"], [{"allocation": allocation, "access": "read_write", "byte_offset": 0,
                                       "byte_len": 384, "kernarg_byte_offset": 0}]), "mixed binding")
        created = maps["allocation_created"][allocation]["event"]
        need(same({key: created[key] for key in ("memory_kind", "byte_len", "alignment")},
                  {"memory_kind": "host_visible", "byte_len": 384, "alignment": 4}), "mixed allocation")
        for field, ref in (("allocation", allocation), ("kernel", event["kernel"]), ("module", kernel["module"]),
                           ("stream", event["stream"]), ("dispatch", event["dispatch"])):
            resources[field].append(ref)
    need(all(len(set(refs)) == count for refs in resources.values()), "distinct per-variant resources")
    expected_order = resources["dispatch"][::-1] if mode == "owner" else resources["dispatch"]
    for kind in ("dispatch_completed", "submission_released"):
        need([row["event"]["dispatch"] for row in groups[kind]] == expected_order, "exact completion/release order")
    if mode == "owner":
        need(len({row["event"]["queue"] for row in publications}) == 2
             and publications[1]["sequence"] < groups["dispatch_completed"][0]["sequence"], "two queues published before completion")
    if mode == "backpressure":
        need(publications[0]["event"]["queue"] == publications[1]["event"]["queue"]
             and groups["dispatch_completed"][0]["sequence"] < publications[1]["sequence"], "sequential queue reuse")
    allocations = resources["allocation"]
    need([row["event"]["allocation"] for row in groups["host_write"]] == allocations, "exact initial writes")
    reads = groups["host_read"]
    expected_reads = ([allocations[1]] if mode == "backpressure" else []) + allocations
    need([row["event"]["allocation"] for row in reads] == expected_reads, "exact read roster")
    for kind in ("host_write", "host_read"):
        for row in groups[kind]:
            need(same(row["event"]["byte_offset"], 0)
                 and same(row["event"]["content"], {"state": "range_only", "byte_len": 384}), "whole mixed access")
    for index, pub in enumerate(publications):
        need(groups["host_write"][index]["sequence"] < pub["sequence"], "write before publish")
        done = maps["dispatch_completed"][pub["event"]["dispatch"]]["sequence"]
        need(done < reads[index + (mode == "backpressure")]["sequence"], "completion before final read")
    if mode == "backpressure":
        need(max(groups["host_write"][1]["sequence"], groups["dispatch_completed"][0]["sequence"])
             < reads[0]["sequence"] < publications[1]["sequence"], "unissued Long read after Short completion and before publication")
    return {"events": len(value["events"]), "queues": queues, "dispatches": count, "physical_overlap_measured": False}
