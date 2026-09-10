#!/usr/bin/env python3
"""Check signed R66 retained-custody evidence, never physical overlap."""
import argparse
import functools
import hashlib
import json
import pathlib
import re

SCHEMA = "fe2o3.runtime.r66-retained-coexistence.v1"
MAX_OUTPUT_BYTES = 32768
PAD = 128
PROFILES = tuple((size, packets, direction, order)
                 for size, packets in ((1024 * 1024 + 257, 1), (0x003FFFE0 + 257, 2))
                 for direction in ("h2d", "d2h")
                 for order in ("compute-first", "copy-first"))
INPUT_HASHES = ("ce96f8d88572648c07a6c03d7ce49af52c637af65267645eafdd2193ee6e49b7",
                "061cc02d1e9f513366e292544724ef6592b6ca4f59cfb2464a29bd94ff71236e")
OUTPUT_HASHES = ("4a42778046c60e35849ad35fe4dc4bf39a0a4d616b75c9e62d146dbdb41ec960",
                 "49f9da5c37cd051649cf257f528b1b573b44a1937b865b05643823267579cf62")
OBSERVATION_KEYS = {"compute", "compute_membership", "copy", "copy_membership", "copy_packets"}
CASE_KEYS = {"ordinal", "bytes", "packets", "order", "direction", "first", "both",
             "after_copy", "after_compute", "canaries", "download_sha256", "device_sha256",
             "upload_sha256", "compute_input_sha256", "compute_output_sha256", "logical_credits"}


class ValidationError(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise ValidationError(message)


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, f"duplicate JSON field: {key}")
        result[key] = value
    return result


def invalid_constant(value):
    raise ValidationError(f"nonfinite JSON: {value}")


def exact_keys(value, keys):
    require(type(value) is dict and set(value) == keys, "unexpected evidence fields")


def digest(value):
    return type(value) is str and re.fullmatch(r"[0-9a-f]{64}", value) is not None and value != "0" * 64


def observation(value):
    exact_keys(value, OBSERVATION_KEYS)
    for kind in ("compute", "copy"):
        native, runtime = value[kind], value[kind + "_membership"]
        require((native is None and runtime is None) or (digest(native) and digest(runtime)
                and native != runtime), "missing or malformed exact retained identity")
    packets = value["copy_packets"]
    require(type(packets) is int and packets in (0, 1, 2), "invalid packet count")
    require((value["copy"] is None) == (packets == 0), "copy/packet roster mismatch")


@functools.lru_cache(maxsize=8)
def expected_hashes(ordinal):
    size, _, direction, _ = PROFILES[ordinal]
    body = bytes((index * 29 + index // 257 + ordinal * 17) % 251 for index in range(size))
    total = size + 2 * PAD
    upload = bytes([0xC7]) * PAD + body + bytes([0xC7]) * PAD
    device = bytes([0xA5]) * PAD + body + bytes([0xA5]) * PAD
    download = (bytes([0xD3]) * PAD + body + bytes([0xD3]) * PAD
                if direction == "d2h" else bytes([0xD3]) * total)
    return {"upload_sha256": hashlib.sha256(upload).hexdigest(),
            "device_sha256": hashlib.sha256(device).hexdigest(),
            "download_sha256": hashlib.sha256(download).hexdigest(),
            "compute_input_sha256": INPUT_HASHES[ordinal % 2],
            "compute_output_sha256": OUTPUT_HASHES[ordinal % 2]}


def expected_credits(ordinal):
    capacity = 3 * 1024 * 1024 + 4 * (PROFILES[ordinal][0] + 2 * PAD)
    return {"scope": "requested-allocation-bytes-and-records", "capacity_bytes": capacity,
            "capacity_records": 7, "full_bytes": capacity, "full_records": 7,
            "eighth_request": "capacity-rejected-unchanged", "retirement": "retained", "after_cleanup": "zero"}


def validate_output(output):
    require(type(output) is str and len(output) <= MAX_OUTPUT_BYTES and output.endswith("\n")
            and output.count("\n") == 1, "expected one bounded JSON record with final newline")
    try:
        require(len(output.encode("utf-8")) <= MAX_OUTPUT_BYTES, "R66 evidence byte limit exceeded")
        document = json.loads(output, object_pairs_hook=unique_object, parse_constant=invalid_constant)
    except (ValueError, TypeError, RecursionError) as error:
        raise ValidationError(f"invalid R66 evidence: {error}") from error
    exact_keys(document, {"schema", "cases", "owner_threads", "cleanup", "physical_overlap"})
    require(document["schema"] == SCHEMA and document["cleanup"] == "complete"
            and document["physical_overlap"] == "unmeasured"
            and type(document["owner_threads"]) is int and document["owner_threads"] == 1,
            "unsupported profile, cleanup, or overlap claim")
    cases = document["cases"]
    require(type(cases) is list and len(cases) == len(PROFILES), "incomplete profile matrix")
    identities = set()
    for ordinal, (case, profile) in enumerate(zip(cases, PROFILES)):
        exact_keys(case, CASE_KEYS)
        require(all(type(case[key]) is int for key in ("ordinal", "bytes", "packets"))
                and case["ordinal"] == ordinal
                and (case["bytes"], case["packets"], case["direction"], case["order"]) == profile
                and case["canaries"] == "complete", "case profile mismatch")
        credits = case["logical_credits"]
        exact_keys(credits, set(expected_credits(ordinal)))
        require(all(type(credits[key]) is int for key in ("capacity_bytes", "capacity_records", "full_bytes", "full_records"))
                and credits == expected_credits(ordinal), "requested-allocation credit accounting mismatch")
        for key in ("first", "both", "after_copy", "after_compute"):
            observation(case[key])
        first, both, after_copy, after_compute = (case[key] for key in
                                                 ("first", "both", "after_copy", "after_compute"))
        require(both["compute"] is not None and both["copy"] is not None
                and both["copy_packets"] == case["packets"], "missing co-retained native publication")
        first_kind = "compute" if case["order"] == "compute-first" else "copy"
        other_kind = "copy" if first_kind == "compute" else "compute"
        require(first[first_kind] == both[first_kind]
                and first[first_kind + "_membership"] == both[first_kind + "_membership"]
                and first[other_kind] is None
                and first["copy_packets"] == (case["packets"] if first_kind == "copy" else 0),
                "publication order changed retained identity")
        require(after_copy["compute"] == both["compute"]
                and after_copy["compute_membership"] == both["compute_membership"]
                and after_copy["copy"] is None, "copy retirement did not preserve exact compute custody")
        require(after_compute["compute"] is None and after_compute["copy"] is None,
                "native work retained after retirement")
        for key in ("compute", "copy", "compute_membership", "copy_membership"):
            require(both[key] not in identities, "native/runtime occurrence was reused")
            identities.add(both[key])
        for key, expected in expected_hashes(ordinal).items():
            require(case[key] == expected, f"independent full-buffer or R26 output mismatch: {key}")
    return document


def read_output(path):
    with path.open("rb") as stream:
        encoded = stream.read(MAX_OUTPUT_BYTES + 1)
    require(len(encoded) <= MAX_OUTPUT_BYTES, "R66 evidence byte limit exceeded")
    try:
        return encoded.decode("utf-8")
    except UnicodeError as error:
        raise ValidationError("R66 evidence is not UTF-8") from error


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=pathlib.Path)
    args = parser.parse_args()
    try:
        validate_output(read_output(args.output))
    except (OSError, ValidationError) as error:
        parser.exit(1, f"R66 rejected: {error}\n")
    print("PASS R66 retained custody: 8 cells; physical overlap unmeasured")


if __name__ == "__main__":
    main()
