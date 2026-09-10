#!/usr/bin/env python3
"""Check bounded copy/drain evidence consistency, not producer authenticity or overlap."""
import argparse
import functools
import hashlib
import json
import pathlib
import re

SCHEMA = "fe2o3.runtime.drain-capture-copy.v1"
MAX_OUTPUT_BYTES = 262144
BODY = 1024 * 1024 + 257
DATA = BODY + 514
VERIFY_OFFSET = DATA + 193
CAPTURE = VERIFY_OFFSET + DATA + 211
PROFILE = {"body_bytes": BODY, "data_bytes": DATA, "capture_bytes": CAPTURE,
           "input_offset": 131, "device_offset": 137, "output_offset": 139,
           "verify_offset": VERIFY_OFFSET}
CELLS = tuple((cutoff, streams, observers)
              for cutoff in ("queued", "native-retained")
              for streams in (1, 2) for observers in ("retained", "dropped"))
ROW_KEYS = {"submission", "stream", "source", "destination", "source_offset",
            "destination_offset", "byte_len", "dependencies", "phase",
            "native_receipt", "runtime_membership", "native_packets"}
CASE_KEYS = {"ordinal", "cutoff", "streams", "observers", "profile", "context",
             "resources", "baseline", "at_cutoff", "capture_before", "capture_after",
             "copies", "drain", "completed_observers", "graph_sha256", "credits",
             "output_sha256", "device_sha256", "body_sha256", "cleanup"}


class ValidationError(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise ValidationError(message)


def keys(value, expected):
    require(type(value) is dict and set(value) == expected, "unexpected evidence fields")


def integer(value, minimum=0, maximum=(1 << 64) - 1):
    return type(value) is int and minimum <= value <= maximum


def digest(value):
    return type(value) is str and re.fullmatch(r"[0-9a-f]{64}", value) is not None and value != "0" * 64


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, "duplicate JSON field")
        result[key] = value
    return result


def reject_number(value):
    raise ValidationError("floating-point or nonfinite JSON number")


def expected_membership(value):
    message = bytearray(b"fe2o3.r66.runtime-retained-membership.v1\0copy\0")
    message.extend(bytes.fromhex(value["native_receipt"]))
    for key in ("submission", "stream", "source", "destination", "source_offset",
                "destination_offset", "byte_len"):
        message.extend(value[key].to_bytes(8, "little"))
    return hashlib.sha256(message).hexdigest()


def row(value):
    keys(value, ROW_KEYS)
    for key in ("submission", "stream", "source", "destination", "byte_len"):
        require(integer(value[key], 1), "invalid copy identity or extent")
    for key in ("source_offset", "destination_offset"):
        require(integer(value[key]), "invalid copy offset")
    dependencies = value["dependencies"]
    require(type(dependencies) is list and len(dependencies) <= 8
            and all(integer(item, 1) for item in dependencies)
            and len(set(dependencies)) == len(dependencies)
            and value["submission"] not in dependencies, "invalid dependency roster")
    require(integer(value["native_packets"], 0, 1), "unsupported packet count")
    if value["phase"] == "ready":
        require(value["native_receipt"] is None and value["runtime_membership"] is None
                and value["native_packets"] == 0, "queued copy claims native publication")
    else:
        require(value["phase"] == "directional-published"
                and digest(value["native_receipt"]) and digest(value["runtime_membership"])
                and value["native_receipt"] != value["runtime_membership"]
                and value["native_packets"] == 1, "missing exact native copy receipt")
        require(value["runtime_membership"] == expected_membership(value),
                "runtime membership digest does not bind exact copy coordinates")


def observation(value):
    keys(value, {"publication_ids", "native_retained_copies", "copies"})
    history, copies = value["publication_ids"], value["copies"]
    require(type(history) is list and len(history) <= 32
            and all(integer(item, 1) for item in history)
            and len(set(history)) == len(history), "invalid publication history")
    require(type(copies) is list and len(copies) <= 8, "invalid active-copy roster")
    for copy in copies:
        row(copy)
    ids = [copy["submission"] for copy in copies]
    require(ids == sorted(set(ids)), "active copies are duplicated or unordered")
    published = [copy for copy in copies if copy["phase"] == "directional-published"]
    require(integer(value["native_retained_copies"], 0, 1)
            and value["native_retained_copies"] == len(published), "native retained-count mismatch")
    require(all(copy["submission"] in history for copy in published), "receipt absent from history")
    require(all(copy["submission"] not in history for copy in copies if copy["phase"] == "ready"),
            "queued copy was already published")


@functools.lru_cache(maxsize=8)
def expected_hashes(ordinal):
    body = bytes((index * 29 + index // 257 + ordinal * 17) % 251 for index in range(BODY))
    device = bytearray([0xA5]) * DATA
    device[137:137 + BODY] = body
    output = bytearray([0xD3]) * CAPTURE
    output[139:139 + BODY] = body
    output[VERIFY_OFFSET:VERIFY_OFFSET + DATA] = device
    return {"output_sha256": hashlib.sha256(output).hexdigest(),
            "device_sha256": hashlib.sha256(device).hexdigest(),
            "body_sha256": hashlib.sha256(body).hexdigest()}


def expected_credits():
    return {"requested_bytes": 2 * DATA + CAPTURE, "allocation_records": 3,
            "capture_reserved": CAPTURE, "after_extraction": CAPTURE,
            "after_shutdown": CAPTURE, "after_disposal": 0,
            "reply_after_disposal": 0, "snapshot_after_shutdown": 0,
            "allocation_records_after_cleanup": 0}


def validate_output(output):
    require(type(output) is str and len(output) <= MAX_OUTPUT_BYTES
            and output.endswith("\n") and output.count("\n") == 1,
            "expected one bounded JSON record with final newline")
    try:
        require(len(output.encode("utf-8")) <= MAX_OUTPUT_BYTES, "evidence byte limit exceeded")
        document = json.loads(output, object_pairs_hook=unique_object,
                              parse_float=reject_number, parse_constant=reject_number)
    except (ValueError, TypeError, RecursionError, UnicodeError) as error:
        raise ValidationError(f"invalid drain-capture evidence: {error}") from error
    keys(document, {"schema", "owner_threads", "physical_overlap", "performance", "cases"})
    require(document["schema"] == SCHEMA and type(document["owner_threads"]) is int
            and document["owner_threads"] == 1 and document["physical_overlap"] == "unmeasured"
            and document["performance"] == "unmeasured", "unsupported qualifier claim")
    cases = document["cases"]
    require(type(cases) is list and len(cases) == 8, "incomplete eight-cell matrix")
    contexts, native_identities = set(), set()
    for ordinal, (case, cell) in enumerate(zip(cases, CELLS)):
        keys(case, CASE_KEYS)
        require(integer(case["ordinal"], 0, 7) and case["ordinal"] == ordinal
                and type(case["streams"]) is int
                and (case["cutoff"], case["streams"], case["observers"]) == cell,
                "cell order or identity mismatch")
        keys(case["profile"], set(PROFILE))
        require(all(type(value) is int for value in case["profile"].values())
                and case["profile"] == PROFILE, "copy profile mismatch")
        require(digest(case["context"]) and case["context"] not in contexts,
                "invalid or reused context")
        contexts.add(case["context"])
        resources = case["resources"]
        keys(resources, {"streams", "allocations"})
        streams, allocations = resources["streams"], resources["allocations"]
        require(type(streams) is list and len(streams) == cell[1], "stream roster mismatch")
        require(type(allocations) is list and len(allocations) == 3, "allocation roster mismatch")
        for item in streams:
            keys(item, {"runtime", "backend"})
        for item, role, size in zip(allocations, ("input", "device", "output"), (DATA, DATA, CAPTURE)):
            keys(item, {"runtime", "backend", "role", "bytes"})
            require(item["role"] == role and type(item["bytes"]) is int and item["bytes"] == size,
                    "allocation role or extent mismatch")
        for field in ("runtime", "backend"):
            ids = [item[field] for item in streams + allocations]
            require(all(integer(item, 1) for item in ids) and len(set(ids)) == len(ids),
                    "resource identities alias")
        for phase in ("baseline", "at_cutoff", "capture_before", "capture_after"):
            observation(case[phase])
        baseline, cutoff, before, after = (case[phase] for phase in
                                            ("baseline", "at_cutoff", "capture_before", "capture_after"))
        require(len(baseline["publication_ids"]) == 2 and baseline["copies"] == []
                and baseline["native_retained_copies"] == 0, "warmup is not an empty native frontier")
        require(before == after and before["copies"] == [] and before["native_retained_copies"] == 0,
                "capture changed publication history or retained work")
        copies = case["copies"]
        require(type(copies) is list and len(copies) == 3, "incomplete work receipt roster")
        input_id, device_id, output_id = (item["backend"] for item in allocations)
        expected_geometry = ((input_id, device_id, 131, 137, BODY),
                             (device_id, output_id, 137, 139, BODY),
                             (device_id, output_id, 0, VERIFY_OFFSET, DATA))
        work_ids = []
        for index, (copy, geometry) in enumerate(zip(copies, expected_geometry)):
            row(copy)
            require(copy["phase"] == "directional-published", "work lacks retained native receipt")
            require(tuple(copy[key] for key in ("source", "destination", "source_offset",
                                                "destination_offset", "byte_len")) == geometry,
                    "copy coordinate mismatch")
            stream = streams[0 if index == 0 or cell[1] == 1 else 1]["backend"]
            require(copy["stream"] == stream, "copy stream mismatch")
            require(copy["dependencies"] == ([work_ids[-1]] if index and cell[1] == 1 else []),
                    "copy dependency mismatch")
            work_ids.append(copy["submission"])
            for name in ("native_receipt", "runtime_membership"):
                require(copy[name] not in native_identities, "reused receipt or membership identity")
                native_identities.add(copy[name])
        require(len(set(work_ids)) == 3 and not set(work_ids).intersection(baseline["publication_ids"]),
                "work/warmup publication alias")
        require(before["publication_ids"] == baseline["publication_ids"] + work_ids,
                "work history omitted or duplicated publication")
        if cell[0] == "queued":
            require(cutoff == baseline, "queued cutoff already published work")
        else:
            require(cutoff["publication_ids"] == baseline["publication_ids"] + work_ids[:1]
                    and cutoff["native_retained_copies"] == 1
                    and cutoff["copies"] == [copies[0]],
                    "native cutoff lacks its exact retained frontier")
        drain = case["drain"]
        keys(drain, {"outcome", "ticks", "total_submissions", "succeeded", "pending",
                     "queued_commands_exhausted", "operations_remaining", "graph_active"})
        retained = 3 if cell[1] == 1 else 0
        require(drain["outcome"] == "quiescent" and integer(drain["ticks"], 1, 128)
                and all(type(drain[key]) is int for key in
                        ("total_submissions", "succeeded", "pending", "operations_remaining"))
                and drain["total_submissions"] == retained and drain["succeeded"] == retained
                and drain["pending"] == 0 and drain["operations_remaining"] == 0
                and drain["queued_commands_exhausted"] is True and drain["graph_active"] is False,
                "drain did not establish the complete accepted prefix")
        expected_observers = (3 if cell[1] == 1 else 1) if cell[2] == "retained" else 0
        require(type(case["completed_observers"]) is int and case["completed_observers"] == expected_observers,
                "observer disposition mismatch")
        require((case["graph_sha256"] is None) if cell[1] == 1 else digest(case["graph_sha256"]),
                "graph/standalone identity mismatch")
        keys(case["credits"], set(expected_credits()))
        require(all(type(value) is int for value in case["credits"].values())
                and case["credits"] == expected_credits(), "credit lifetime mismatch")
        for key, expected in expected_hashes(ordinal).items():
            require(case[key] == expected, f"independent full-buffer mismatch: {key}")
        require(case["cleanup"] == "complete", "native cleanup incomplete")
    return document


def read_output(path):
    with path.open("rb") as stream:
        encoded = stream.read(MAX_OUTPUT_BYTES + 1)
    require(len(encoded) <= MAX_OUTPUT_BYTES, "evidence byte limit exceeded")
    try:
        return encoded.decode("utf-8")
    except UnicodeError as error:
        raise ValidationError("evidence is not UTF-8") from error


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=pathlib.Path)
    args = parser.parse_args()
    try:
        validate_output(read_output(args.output))
    except (OSError, ValidationError) as error:
        parser.exit(1, f"drain capture rejected: {error}\n")
    print("PASS copy drain capture: 8 cells; hardware activity and overlap unmeasured")


if __name__ == "__main__":
    main()
