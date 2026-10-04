#!/usr/bin/env python3
"""Compare complete observation-only simulator payloads, not compiler or GPU authority."""

import argparse
import json
import math
import os
import re
import stat
import sys

EXPECTATION_SCHEMA = "fe2o3-simulation-expectation-v1"
RESULT_SCHEMA = "fe2o3-simulation-result-v1"
MAX_EXPECTATION_BYTES = 16 * 1024 * 1024
MAX_RESULT_BYTES = 64 * 1024 * 1024
MAX_ITEMS = 4096
MAX_JSON_DEPTH = 64
MAX_JSON_NODES = 1_000_000
U32_MAX = (1 << 32) - 1
U64_MAX = (1 << 64) - 1
SCALAR_BITS = {
    "bool": 1,
    "i8": 8, "u8": 8,
    "i16": 16, "u16": 16, "f16": 16, "bf16": 16,
    "i32": 32, "u32": 32, "f32": 32,
    "i64": 64, "u64": 64, "index": 64, "f64": 64,
    "i128": 128, "u128": 128,
}
ACCESS = {"read_only", "write_only", "read_write"}
BUFFER_KEYS = {"element", "access", "alignment", "bytes", "initialized"}
VIEW_KEYS = {"kind", "backing", "element", "access", "alignment", "byte_offset", "elements"}
COUNT_KEYS = {
    "arguments", "shared_buffers", "invocations_executed", "workgroups_visited",
    "scheduled_slots_visited", "steps_executed", "events_emitted",
}
HEX_BYTES = re.compile(r"0x[0-9a-f]*\Z")


class CheckError(Exception):
    pass


def require(condition, message):
    if not condition:
        raise CheckError(message)


def keys(value, expected, label):
    require(type(value) is dict and value.keys() == expected, f"{label}: invalid fields")


def uint(value, maximum, label):
    require(type(value) is int and 0 <= value <= maximum, f"{label}: invalid unsigned integer")
    return value


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, "JSON contains a duplicate object key")
        result[key] = value
    return result


def reject_constant(_value):
    raise CheckError("JSON contains a non-finite number")


def finite_float(value):
    result = float(value)
    require(math.isfinite(result), "JSON contains a non-finite number")
    return result


def check_json_bounds(value):
    remaining = MAX_JSON_NODES

    def visit(node, depth):
        nonlocal remaining
        require(depth <= MAX_JSON_DEPTH, "JSON nesting exceeds the depth limit")
        remaining -= 1
        require(remaining >= 0, "JSON exceeds the node limit")
        if type(node) is dict:
            for child in node.values():
                visit(child, depth + 1)
        elif type(node) is list:
            for child in node:
                visit(child, depth + 1)

    visit(value, 0)


def read_document(path, maximum, label):
    # Open the final component without following links; nonblocking rejects FIFOs safely.
    try:
        descriptor = os.open(path, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK)
        with os.fdopen(descriptor, "rb") as stream:
            before = os.fstat(stream.fileno())
            require(stat.S_ISREG(before.st_mode), f"{label}: input must be a regular file")
            require(0 < before.st_size <= maximum, f"{label}: input exceeds its byte bounds")
            data = stream.read(maximum + 1)
            after = os.fstat(stream.fileno())
    except OSError:
        raise CheckError(f"{label}: cannot securely read input") from None
    snapshot_fields = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
    require(
        len(data) == before.st_size
        and len(data) <= maximum
        and all(getattr(before, name) == getattr(after, name) for name in snapshot_fields),
        f"{label}: input changed while being read",
    )
    try:
        document = json.loads(
            data.decode("utf-8"),
            object_pairs_hook=unique_object,
            parse_constant=reject_constant,
            parse_float=finite_float,
        )
    except (UnicodeError, json.JSONDecodeError, ValueError, RecursionError):
        raise CheckError(f"{label}: malformed or excessively nested JSON") from None
    check_json_bounds(document)
    return document


def element_bytes(element, label):
    require(type(element) is str and element in SCALAR_BITS, f"{label}: invalid scalar type")
    return (SCALAR_BITS[element] + 7) // 8


def access_alignment(value, label):
    require(type(value["access"]) is str and value["access"] in ACCESS, f"{label}: invalid access")
    alignment = uint(value["alignment"], U32_MAX, f"{label}.alignment")
    require(alignment > 0 and alignment & (alignment - 1) == 0, f"{label}: invalid alignment")


def hex_length(value, label):
    require(
        type(value) is str and len(value) % 2 == 0 and HEX_BYTES.fullmatch(value) is not None,
        f"{label}: invalid hex bytes",
    )
    return (len(value) - 2) // 2


def validate_buffer(value, label):
    keys(value, BUFFER_KEYS, label)
    width = element_bytes(value["element"], label)
    access_alignment(value, label)
    length = hex_length(value["bytes"], f"{label}.bytes")
    require(length % width == 0, f"{label}: partial scalar element")
    initialized = value["initialized"]
    packed_length = hex_length(initialized, f"{label}.initialized")
    require(packed_length == (length + 7) // 8, f"{label}: incomplete initialization mask")
    if length % 8:
        require(int(initialized[-2:], 16) >> (length % 8) == 0, f"{label}: nonzero mask padding")
    return length


def validate_payload(document, label):
    arguments = document.get("arguments")
    shared = document.get("shared_buffers")
    require(type(arguments) is list and len(arguments) <= MAX_ITEMS, f"{label}: invalid arguments")
    require(type(shared) is list and len(shared) <= MAX_ITEMS, f"{label}: invalid shared_buffers")
    backings = {}
    for index, backing in enumerate(shared):
        location = f"{label}.shared_buffers[{index}]"
        keys(backing, {"id", "buffer"}, location)
        identity = uint(backing["id"], U32_MAX, f"{location}.id")
        require(identity not in backings, f"{location}: duplicate backing identity")
        length = validate_buffer(backing["buffer"], f"{location}.buffer")
        backings[identity] = (backing["buffer"], length)
    for index, argument in enumerate(arguments):
        location = f"{label}.arguments[{index}]"
        require(type(argument) is dict, f"{location}: invalid argument")
        kind = argument.get("kind")
        if kind == "scalar":
            keys(argument, {"kind", "type", "bits"}, location)
            element_bytes(argument["type"], location)
            width = SCALAR_BITS[argument["type"]]
            bits = argument["bits"]
            require(
                type(bits) is str
                and len(bits) == 2 + (width + 3) // 4
                and bits.startswith("0x")
                and re.fullmatch(r"[0-9a-f]+", bits[2:]) is not None,
                f"{location}: invalid scalar bits",
            )
            require(int(bits[2:], 16) < 1 << width, f"{location}: scalar bits exceed width")
        elif kind == "buffer":
            keys(argument, {"kind", "value"}, location)
            validate_buffer(argument["value"], f"{location}.value")
        elif kind == "buffer_view":
            keys(argument, VIEW_KEYS, location)
            identity = uint(argument["backing"], U32_MAX, f"{location}.backing")
            require(identity in backings, f"{location}: missing backing")
            width = element_bytes(argument["element"], location)
            access_alignment(argument, location)
            offset = uint(argument["byte_offset"], U64_MAX, f"{location}.byte_offset")
            count = uint(argument["elements"], U64_MAX, f"{location}.elements")
            buffer, length = backings[identity]
            require(argument["element"] == buffer["element"], f"{location}: backing element mismatch")
            require(
                buffer["access"] == "read_write" or buffer["access"] == argument["access"],
                f"{location}: incompatible backing access",
            )
            require(buffer["alignment"] >= argument["alignment"], f"{location}: insufficient backing alignment")
            require(offset % argument["alignment"] == 0, f"{location}: unaligned view")
            require(offset + count * width <= length, f"{location}: view exceeds backing")
        else:
            raise CheckError(f"{location}: invalid argument kind")


def validate_expectation(document):
    keys(document, {"schema", "arguments", "shared_buffers"}, "expectation")
    require(document["schema"] == EXPECTATION_SCHEMA, "expectation: unsupported schema")
    validate_payload(document, "expectation")


def validate_result(document):
    require(type(document) is dict, "result: expected an object")
    expected = {
        "schema": RESULT_SCHEMA,
        "status": "ok",
        "authority": "observation_only",
        "simulated": True,
        "hardware_observed": False,
        "hardware_validation": False,
        "performance_prediction": False,
    }
    for key, value in expected.items():
        actual = document.get(key)
        require(type(actual) is type(value) and actual == value, f"result: invalid {key}")
    validate_payload(document, "result")
    counts = document.get("counts")
    keys(counts, COUNT_KEYS, "result.counts")
    for key in COUNT_KEYS:
        uint(counts[key], U64_MAX, f"result.counts.{key}")
    require(counts["arguments"] == len(document["arguments"]), "result: argument count mismatch")
    require(counts["shared_buffers"] == len(document["shared_buffers"]), "result: backing count mismatch")


def exactly_equal(expected, actual):
    if type(expected) is not type(actual):
        return False
    if type(expected) is dict:
        return expected.keys() == actual.keys() and all(
            exactly_equal(value, actual[key]) for key, value in expected.items()
        )
    if type(expected) is list:
        return len(expected) == len(actual) and all(
            exactly_equal(left, right) for left, right in zip(expected, actual)
        )
    return expected == actual


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--expectation", required=True)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--result")
    mode.add_argument("--validate-only", action="store_true")
    options = parser.parse_args()
    try:
        expectation = read_document(options.expectation, MAX_EXPECTATION_BYTES, "expectation")
        validate_expectation(expectation)
        if not options.validate_only:
            result = read_document(options.result, MAX_RESULT_BYTES, "result")
            validate_result(result)
            for key in ("arguments", "shared_buffers"):
                require(exactly_equal(expectation[key], result[key]), f"result: complete {key} mismatch")
    except (CheckError, MemoryError, RecursionError) as error:
        message = str(error) if isinstance(error, CheckError) else "input exceeds processing bounds"
        print(f"simulation expectation: {message[:512]}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
