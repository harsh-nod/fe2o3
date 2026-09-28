"""Reconstruct exported depth/profile joins; private custody cuts remain in-process checks."""
import hashlib
import struct

HSACO = "3a25e364dd1e1931d1a16c24b37aa998df2c6ef1cbcf0ec2afb6372cbc878bab"
SIGNATURE = "558897b2c24edacb9a0d83a630d6f8480a74095771211fafc0fdb823d476c9a7"


def output_hashes():
    left = [index * 0.5 for index in range(1024)]
    right = [(index % 256) * 0.25 for index in range(1024)]
    return [hashlib.sha256(b"".join(struct.pack("<f", value) for value in values) * 1024).hexdigest()
            for values in (left, right, [a + b for a, b in zip(left, right)])] * 2


def membership(rows):
    digest = hashlib.sha256(b"fe2o3.scale-retained-depth-membership.v1\0" + len(rows).to_bytes(8, "little"))
    for row in rows:
        for value in [row["lane"], row["submission"], row["stream"], row["kernel"], *row["allocations"]]:
            digest.update(value.to_bytes(8, "little"))
        predecessor = row["ordered_predecessor"]
        digest.update(bytes([predecessor is not None]))
        digest.update((predecessor or 0).to_bytes(8, "little"))
        digest.update(bytes([0 if row["pipeline_phase"] is None else 1]))
        digest.update(bytes(row["native_receipt"]))
        digest.update(bytes(row["dispatch_shape"]))
    return list(digest.digest())


def check(value, profile, p):
    keys = {"schema", "unique_id", "hsaco_sha256", "depth_per_lane", "native_retained", "membership", "receipts",
            "host_table_peak_records", "host_table_peak_payload_bytes", "final_buffer_sha256", "runtime_capacity_negative",
            "native_capacity_negative", "unfinished_gpu_count", "physical_overlap", "host_table_final_records", "cleanup"}
    p.need(type(value) is dict and set(value) == keys, "exact scale receipt schema")
    fixed = {"schema": "fe2o3.scale-retained-depth-development.v1", "unique_id": p.UID,
             "hsaco_sha256": list(bytes.fromhex(HSACO)), "depth_per_lane": 1024, "native_retained": 2048,
             "host_table_peak_records": 10, "host_table_final_records": 0, "cleanup": "complete",
             "runtime_capacity_negative": "unchanged-retained-cut",
             "native_capacity_negative": "rejected-before-side-effect-1024-each-lane",
             "unfinished_gpu_count": None, "physical_overlap": "unmeasured", "final_buffer_sha256": output_hashes()}
    p.need(p.same({key: value[key] for key in fixed}, fixed), "fixed scale claims and independent output hashes")
    p.need(0 < p.uint(value["host_table_peak_payload_bytes"]) <= 64 * 1024**2, "bounded host-table payload")
    rows = value["receipts"]
    p.need(type(rows) is list and len(rows) == 2048, "full receipt roster")
    fields = {"submission", "lane", "stream", "kernel", "allocations", "ordered_predecessor", "pipeline_phase",
              "native_receipt", "dispatch_shape"}
    for index, row in enumerate(rows):
        lane, ordinal = divmod(index, 1024)
        first = rows[lane * 1024]
        p.need(type(row) is dict and set(row) == fields and p.same(row["lane"], lane), "ordered lane-group rows")
        for key in ("submission", "stream", "kernel"):
            p.need(p.uint(row[key]) > 0, "nonzero receipt coordinate")
        p.need(type(row["allocations"]) is list and len(row["allocations"]) == 3
               and all(p.uint(ref) > 0 for ref in row["allocations"]), "three allocation coordinates")
        for key in ("native_receipt", "dispatch_shape"):
            blob = row[key]
            p.need(type(blob) is list and len(blob) == 32 and all(p.uint(n, 8) >= 0 for n in blob)
                   and any(blob), "nonzero digest bytes")
        p.need(row["stream"] == first["stream"] and row["kernel"] == rows[0]["kernel"]
               and row["allocations"] == first["allocations"] and row["dispatch_shape"] == first["dispatch_shape"], "stable lane recipe")
        p.need(p.same(row["ordered_predecessor"], None if ordinal == 0 else rows[index - 1]["submission"])
               and row["pipeline_phase"] == (None if ordinal == 0 else "published"), "exact predecessor/phase chain")
    p.need(len({row["submission"] for row in rows}) == 2048
           and len({bytes(row["native_receipt"]) for row in rows}) == 2048, "distinct submissions and native digests")
    p.need(rows[0]["stream"] != rows[1024]["stream"]
           and len(set(rows[0]["allocations"] + rows[1024]["allocations"])) == 6, "distinct lane resources")
    p.need(p.same(value["membership"], membership(rows)), "reconstructed membership digest")
    groups, maps = p.validate(profile)
    counts = {"native_queue_created": 2, "native_queue_destroyed": 2, "stream_created": 2, "stream_destroyed": 2,
              "allocation_created": 6, "allocation_released": 6, "host_write": 6, "host_read": 6,
              "module_loaded": 1, "module_unloaded": 1, "kernel_resolved": 1,
              "dispatch_published": 2048, "dispatch_completed": 2048, "submission_released": 2048}
    p.need({kind: len(entries) for kind, entries in groups.items()} == counts, "exact scale profile roster")
    p.need(p.same(groups["module_loaded"][0]["event"]["artifact"], p.content_sha(4872, HSACO)), "exact vecadd artifact")
    kernel = groups["kernel_resolved"][0]["event"]
    p.need(p.same(kernel["name"], p.content(b"vecadd"))
           and p.same(kernel["signature"], p.content(bytes.fromhex(SIGNATURE))), "exact vecadd kernel contract")
    scope = profile["capture_scope"]
    ids = [[p.resource(scope, 6, row["submission"]) for row in rows[lane * 1024:(lane + 1) * 1024]] for lane in range(2)]
    p.need(set(ids[0] + ids[1]) == set(maps["dispatch_published"]), "exact receipt/profile dispatch join")
    for row in rows:
        pub = maps["dispatch_published"][p.resource(scope, 6, row["submission"])]["event"]
        p.need(pub["queue"] == p.resource(scope, 1, row["lane"] + 1)
               and pub["stream"] == p.resource(scope, 2, row["stream"])
               and pub["kernel"] == p.resource(scope, 5, row["kernel"]), "exact native lane/stream/kernel join")
        expected_bindings = [{"allocation": p.resource(scope, 3, ref), "access": access, "byte_offset": 0,
                              "byte_len": 4194304, "kernarg_byte_offset": index * 16}
                             for index, (ref, access) in enumerate(zip(row["allocations"], ("read", "read", "write")))]
        p.need(p.same(pub["bindings"], expected_bindings)
               and p.same(pub["dispatch_shape"], p.content(bytes(row["dispatch_shape"]))), "binding and shape joins")
        p.need(p.same(pub["launch"], {"grid": [1048576, 1, 1], "workgroup": [256, 1, 1], "dynamic_shared_bytes": 0}), "vecadd geometry")
    p.need(groups["dispatch_published"][-1]["sequence"] < groups["dispatch_completed"][0]["sequence"]
           and groups["dispatch_completed"][-1]["sequence"] < groups["submission_released"][0]["sequence"]
           and groups["submission_released"][-1]["sequence"] < groups["native_queue_destroyed"][0]["sequence"], "full prefix before completion/release/destruction")
    for lane_ids in ids:
        selected = set(lane_ids)
        for kind in ("dispatch_published", "dispatch_completed"):
            p.need([row["event"]["dispatch"] for row in groups[kind] if row["event"]["dispatch"] in selected] == lane_ids,
                   "per-lane publication/completion order")
    allocations = [p.resource(scope, 3, ref) for ref in rows[0]["allocations"] + rows[1024]["allocations"]]
    for kind in ("host_write", "host_read"):
        p.need([row["event"]["allocation"] for row in groups[kind]] == allocations, "six whole-buffer host accesses")
        for row in groups[kind]:
            p.need(p.same(row["event"]["byte_offset"], 0)
                   and p.same(row["event"]["content"], {"state": "range_only", "byte_len": 4194304}), "whole vecadd extent")
    p.need(groups["host_write"][-1]["sequence"] < groups["dispatch_published"][0]["sequence"]
           and groups["dispatch_completed"][-1]["sequence"] < groups["host_read"][0]["sequence"], "input/output access ordering")
    return {"retained_receipts": 2048, "events": len(profile["events"]), "unfinished_gpu_count": None,
            "physical_overlap_measured": False, "private_custody_cuts": "in-process assertions only"}
