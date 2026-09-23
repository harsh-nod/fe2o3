"""Bounded first-page ResourceV2 validation against one owned runtime binding."""
import re

from debug_console_protocol import exact, keys, require, session_shape
from bridge_live_query_values import (U64, bounded, closed, decimal, enum, error_payload,
                                      hex_bytes, kir_site)
from bridge_runtime_values import (RESOURCE_RESPONSE, UNAVAILABLE, binding, capture_completeness,
    coordinates, invocation, invocation_lane, origin, positive, site, triple)

ACCESS = frozenset(("read", "write_committed", "atomic_read",
    "atomic_write_committed", "atomic_read_write_committed"))


def descriptor(value):
    closed(value, ("identity", "address_space", "access", "alignment", "byte_len", "owning_scope"),
           ("creation_site",))
    triple(value["identity"])
    address_space = enum(value["address_space"], ("global", "constant", "private", "workgroup"))
    enum(value["access"], ("read_only", "write_only", "read_write"))
    alignment = bounded(value["alignment"], (1 << 32)-1, 1)
    require(alignment & (alignment-1) == 0, "power-of-two allocation alignment")
    decimal(value["byte_len"])
    scope = value["owning_scope"]
    require(type(scope) is dict, "actual allocation scope")
    if address_space in ("global", "constant"):
        require(exact(scope, {"scope": "dispatch"}), "dispatch allocation scope")
    elif address_space == "private":
        keys(scope, ("scope", "invocation"))
        require(scope["scope"] == "invocation", "private invocation scope")
        invocation(scope["invocation"])
    else:
        keys(scope, ("scope", "coordinate", "size", "count", "launch"))
        require(scope["scope"] == "workgroup", "workgroup allocation scope")
        group, size, count, launch = [coordinates(scope[name], strings=name != "size")
                                     for name in ("coordinate", "size", "count", "launch")]
        for axis in range(3):
            require(size[axis] > 0 and launch[axis] > 0 and
                    count[axis] == (launch[axis]+size[axis]-1)//size[axis] and
                    group[axis] < count[axis], "workgroup allocation hierarchy")
    if "creation_site" in value:
        site(value["creation_site"], creation=True)


def memory_range(value, capacity=U64, maximum=U64):
    keys(value, ("byte_offset", "byte_len"))
    offset, count = decimal(value["byte_offset"]), decimal(value["byte_len"])
    require(0 < count <= maximum and offset+count <= min(U64, capacity), "exact bounded allocation range")
    return offset, count


def page(value, count):
    closed(value, ("source_count", "source_start", "scanned"), ("next_token",))
    total, start = decimal(value["source_count"]), decimal(value["source_start"])
    scanned = bounded(value["scanned"], 64)
    require(start == 0 and count <= min(16, scanned) and scanned <= total and
            (scanned > 0 or total == 0), "bounded literal first resource page")
    if "next_token" in value:
        require(type(value["next_token"]) is str and
                re.fullmatch(r"[A-Za-z0-9_.-]{1,128}", value["next_token"]) and
                scanned < total, "opaque continuation observation only")
    else:
        require(scanned == total, "incomplete page needs explicit next token")
    return total, scanned


def lifecycle(rows, total, scanned, watermark):
    require(total == watermark and len(rows) == scanned, "literal lifecycle prefix")
    live, all_allocations, slots, released = {}, {}, {}, set()
    last_creation = 0
    for expected, row in enumerate(rows, 1):
        keys(row, ("sequence", "descriptor", "kind"))
        require(positive(row["sequence"]) == expected <= watermark, "contiguous lifecycle sequence")
        desc = row["descriptor"]
        descriptor(desc)
        aid, slot_id, generation = triple(desc["identity"])
        kind = row["kind"]
        require(type(kind) is dict, "literal allocation transition")
        tag = enum(kind.get("transition"), ("preexisting", "create", "release"))
        if tag == "release":
            keys(kind, ("transition",))
            require(aid in live and exact(live[aid], desc) and slots.get(slot_id) == aid,
                    "release exact currently live incarnation")
            del live[aid]
            released.add(aid)
            continue
        require(aid not in all_allocations and positive(aid) > last_creation,
                "fresh monotonic semantic allocation")
        last_creation = positive(aid)
        if tag == "preexisting":
            keys(kind, ("transition",))
            require(desc["owning_scope"] == {"scope": "dispatch"} and
                    "creation_site" not in desc and generation == "1" and slot_id not in slots,
                    "fresh preexisting dispatch storage")
        else:
            closed(kind, ("transition",), ("previous_allocation",))
            require(desc["owning_scope"] != {"scope": "dispatch"} and "creation_site" in desc,
                    "dynamic allocation creation facts")
            if "previous_allocation" in kind:
                predecessor = kind["previous_allocation"]
                positive(predecessor)
                require(predecessor in released and predecessor in all_allocations and
                        slots.get(slot_id) == predecessor, "actual released predecessor")
                prior = all_allocations[predecessor]
                require(prior["identity"]["storage_slot"] == slot_id and
                        positive(generation) == positive(prior["identity"]["generation"])+1 and
                        all(exact(desc[field], prior[field]) for field in
                            ("address_space", "access", "alignment", "byte_len")),
                        "actual exact-shape generation successor")
            else:
                require(generation == "1" and slot_id not in slots, "fresh backing storage slot")
        require(slot_id not in slots or slots[slot_id] in released, "no concurrent slot reuse")
        live[aid] = desc
        all_allocations[aid] = desc
        slots[slot_id] = aid


def validate_resource(response, request, view, runtime, inventory):
    """Returns validated current inventory rows, or None; never follows a token."""
    common = ("schema", "status", "request_id", "operation", "session")
    require(type(response) is dict and response.get("schema") == RESOURCE_RESPONSE and
            type(response.get("request_id")) is int and response["request_id"] == request["request_id"] and
            response.get("operation") == request["operation"], "resource-v2 response correlation")
    session_shape(response.get("session"))
    require(exact(response["session"], view), "resource-v2 read changed full session")
    status = enum(response.get("status"), ("ok", "unavailable", "error"))
    if status == "error":
        keys(response, common+("error",)); error_payload(response["error"])
        return None
    binding(response.get("binding"), view, request["expected_binding"]["owner"])
    require(exact(response["binding"], request["expected_binding"]), "resource-v2 exact expected binding")
    capture_completeness(response.get("completeness"), view["cursor"]["event_sequence"],
                         selected=status == "ok")
    require(exact(response["completeness"], runtime["completeness"]), "same capture completeness")
    if status == "unavailable":
        keys(response, common+("binding", "completeness", "reason"))
        enum(response["reason"], UNAVAILABLE)
        return None
    closed(response, common+("binding", "through_sequence", "completeness", "result"), ("page",))
    watermark = decimal(response["through_sequence"])
    require(response["through_sequence"] == runtime["allocation_watermark"]["through_sequence"],
            "resource-v2 selected lifecycle watermark")
    result = response["result"]
    require(type(result) is dict, "closed resource-v2 result")
    operation = request["operation"]
    if operation == "read_allocation_memory":
        require("page" not in response, "memory reads cannot page")
        keys(result, ("result", "memory"))
        require(result["result"] == "allocation_memory", "memory result operation")
        memory = result["memory"]
        keys(memory, ("allocation", "range", "address_space", "bytes", "initialized"))
        selection = triple(memory["allocation"])
        require(selection in inventory and exact(memory["allocation"], request["allocation"]) and
                exact(memory["range"], request["range"]), "exact current memory selection")
        allocation = inventory[selection]
        require(allocation["snapshot_bytes_available"] is True and
                allocation["initialization_available"] is True, "actual captured memory")
        desc = allocation["descriptor"]
        require(memory["address_space"] == desc["address_space"], "memory address space")
        _offset, length = memory_range(memory["range"], decimal(desc["byte_len"]), 4096)
        require(watermark > 0, "memory needs an observed allocation prefix")
        hex_bytes(memory["bytes"], length)
        hex_bytes(memory["initialized"], (length+7)//8)
        if length % 8:
            require(int(memory["initialized"][-2:], 16) >> (length % 8) == 0,
                    "zero initialization padding bits")
        return None
    tags = {"query_allocations": ("allocations", "allocations"),
            "query_allocation_lifecycle": ("allocation_lifecycle", "transitions"),
            "query_memory_accesses": ("memory_accesses", "accesses")}
    require(operation in tags, "closed resource-v2 operation")
    tag, field = tags[operation]
    keys(result, ("result", field))
    rows = result[field]
    require(result["result"] == tag and type(rows) is list, "resource-v2 result operation")
    total, scanned = page(response.get("page"), len(rows))
    require(not rows or watermark > 0, "resource rows need observed lifecycle prefix")
    if operation == "query_allocations":
        seen_allocations, seen_slots = set(), set()
        for row in rows:
            keys(row, ("descriptor", "snapshot_bytes_available", "initialization_available"))
            descriptor(row["descriptor"])
            aid, slot_id, _generation = triple(row["descriptor"]["identity"])
            require(aid not in seen_allocations and slot_id not in seen_slots, "distinct live storage")
            seen_allocations.add(aid); seen_slots.add(slot_id)
            require(type(row["snapshot_bytes_available"]) is bool and
                    type(row["initialization_available"]) is bool and
                    row["snapshot_bytes_available"] == row["initialization_available"],
                    "memory availability facts agree")
        return rows
    if operation == "query_allocation_lifecycle":
        lifecycle(rows, total, scanned, watermark)
        return None
    selection = triple(request["allocation"])
    require(selection in inventory, "actual current storage inventory membership")
    desc = inventory[selection]["descriptor"]
    require(total <= view["cursor"]["event_sequence"], "access prefix cannot read future records")
    previous = 0
    for row in rows:
        keys(row, ("occurrence", "invocation", "allocation", "range", "address_space", "access", "origin"))
        triple(row["allocation"])
        require(exact(row["allocation"], request["allocation"]) and
                row["address_space"] == desc["address_space"], "access incarnation/address space")
        memory_range(row["range"], decimal(desc["byte_len"]))
        enum(row["access"], ACCESS)
        occurrence = row["occurrence"]
        keys(occurrence, ("record_ordinal", "event_sequence", "scope", "site", "schedule"))
        ordinal = bounded(occurrence["record_ordinal"])
        event = bounded(occurrence["event_sequence"], minimum=1)
        require(ordinal < scanned and ordinal+1 == event and previous < event <= total,
                "access occurrence belongs to scanned historical prefix")
        previous = event
        invocation_lane(row["invocation"], occurrence["scope"])
        kir_site(occurrence["site"])
        schedule = occurrence["schedule"]
        keys(schedule, ("identity", "decision_ordinal"))
        enum(schedule["identity"], ("workgroup_major_local_zyx_serial_v1",
             "workgroup_major_local_zyx_cooperative_v1", "workgroup_major_seeded_runnable_cooperative_v1"))
        bounded(schedule["decision_ordinal"])
        observed_origin = origin(row["origin"])
        if observed_origin is not None:
            require(decimal(observed_origin["site"]["function_ordinal"]) ==
                    occurrence["site"]["function_ordinal"] and
                    observed_origin["site"]["operation"] == occurrence["site"]["point"]["operation_ordinal"],
                    "access operation origin joins actual legacy occurrence")
    return None
