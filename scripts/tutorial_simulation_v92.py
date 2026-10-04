"""Current V18 observations joined to a live V91 census and same-input references.

This does not reinterpret the pending BundleV7/KIR12 manifest contracts. A
finite CPU comparison is not proof, runtime, launch, or hardware authority.
"""
from __future__ import annotations

import hashlib
import json
import math
import os
from pathlib import Path
import stat
import struct

MAX_DOCUMENT = 16 * 1024 * 1024
MAX_GRAPH = 64 * 1024 * 1024
WIDTHS = {"u8": 1, "u16": 2, "bf16": 2, "u32": 4, "i32": 4, "f32": 4,
          "u64": 8, "i64": 8, "index": 8}
NO_AUTHORITY = {"authority": False, "hardwareExecuted": False,
                "grantsCompilerOrLaunchAuthority": False}
DEVICE_TARGETS = {"gfx942": "gfx942:xnack-", "gfx950": "gfx950:xnack-"}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def simulation_command(executable, graph, request, output, cpu):
    # The census names a CPU; the simulator requires the corresponding complete
    # ProductionAmdTargetProfileV1 target, including the fixed xnack state.
    require(type(cpu) is str and cpu in DEVICE_TARGETS, "unsupported production census CPU")
    return [str(executable), "--diagnostic-kir-v18", str(graph),
            "--diagnostic-target", DEVICE_TARGETS[cpu], "--request", str(request),
            "--output", str(output)]


def strict_json(data):
    require(isinstance(data, bytes) and 0 < len(data) <= MAX_DOCUMENT, "JSON byte bound")

    def pairs(rows):
        result = {}
        for key, value in rows:
            require(key not in result, "duplicate JSON field")
            result[key] = value
        return result

    def constant(value):
        raise ValueError(f"nonfinite JSON number: {value}")

    return json.loads(data, object_pairs_hook=pairs, parse_constant=constant)


def exact(value, fields, label):
    require(type(value) is dict and set(value) == set(fields), f"{label} fields")


def read_file(path, maximum=MAX_DOCUMENT):
    fd = os.open(path, os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW | os.O_CLOEXEC)
    with os.fdopen(fd, "rb") as stream:
        before = os.fstat(stream.fileno())
        require(stat.S_ISREG(before.st_mode) and 0 < before.st_size <= maximum,
                "regular bounded input required")
        data = stream.read(before.st_size + 1)
        after = os.fstat(stream.fileno())
    identity = lambda s: (s.st_dev, s.st_ino, s.st_mode, s.st_uid, s.st_gid,
                          s.st_nlink, s.st_size, s.st_mtime_ns, s.st_ctime_ns)
    require(len(data) == before.st_size and identity(before) == identity(after)
            and identity(after) == identity(os.stat(path, follow_symlinks=False)), "input changed")
    return data, {"sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}


def pin_executable(path):
    fd = os.open(path, os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW | os.O_CLOEXEC)
    with os.fdopen(fd, "rb") as stream:
        before = os.fstat(stream.fileno())
        require(stat.S_ISREG(before.st_mode) and before.st_mode & 0o111
                and 0 < before.st_size <= 512 * 1024 * 1024, "bounded executable required")
        digest, remaining = hashlib.sha256(), before.st_size
        while remaining:
            chunk = stream.read(min(remaining, 65536))
            require(chunk, "executable truncated")
            digest.update(chunk)
            remaining -= len(chunk)
        require(stream.read(1) == b"", "executable grew")
        after = os.fstat(stream.fileno())
    identity = lambda s: (s.st_dev, s.st_ino, s.st_mode, s.st_uid, s.st_gid,
                          s.st_nlink, s.st_size, s.st_mtime_ns, s.st_ctime_ns)
    require(identity(before) == identity(after) == identity(os.stat(path, follow_symlinks=False)),
            "executable changed")
    return {"sha256": digest.hexdigest(), "bytes": before.st_size}


def graph_identity(data):
    domain = b"FE2O3/VERIFIED-CANONICAL-KERNEL-IR/V18\0"
    return hashlib.sha256(struct.pack("<I", len(domain)) + domain
                          + struct.pack("<HQ", 1, len(data)) + data).hexdigest()


def hex_bytes(value):
    require(type(value) is str and value.startswith("0x") and len(value) % 2 == 0
            and len(value) <= MAX_DOCUMENT and all(c in "0123456789abcdef" for c in value[2:]),
            "canonical bounded hex required")
    return bytes.fromhex(value[2:])


def digest_array(value):
    require(type(value) is list and len(value) == 32
            and all(type(x) is int and 0 <= x <= 255 for x in value), "digest array")
    return bytes(value).hex()


def command_passed(outcome):
    return (outcome.get("status") == "cargo-completed-unqualified"
            and outcome.get("exitCode") == 0 and outcome.get("logComplete") is True
            and outcome.get("directChildReaped") is True)


def compare(request_bytes, reference, simulation, expected_graph, target):
    """Compare all buffer bytes, including inputs and unchanged output padding."""
    request = strict_json(request_bytes)
    exact(request, ("schema", "kernel", "grid", "workgroup", "arguments"), "current request")
    require(request["schema"] == "fe2o3-simulation-request-v1", "request schema")
    exact(reference, ("schema", "request_sha256", "kernel", "outputs", "authority"), "reference")
    require(reference["schema"] == "fe2o3-tutorial-reference-result-v92"
            and reference["authority"] is False
            and digest_array(reference["request_sha256"]) == hashlib.sha256(request_bytes).hexdigest()
            and reference["kernel"] == request["kernel"], "same-input reference binding")
    require(type(simulation) is dict and simulation.get("schema") == "fe2o3-simulation-result-v1"
            and simulation.get("status") == "ok" and simulation.get("authority") == "observation_only"
            and simulation.get("simulated") is True
            and all(simulation.get(key) is False for key in
                    ("hardware_observed", "hardware_validation", "performance_prediction")),
            "simulator observation header")
    require(simulation.get("kir") == expected_graph, "actual V18 identity differs from live receipt")
    require(target in ("gfx942", "gfx950") and simulation.get("target_profile") == {
        "identity": f"amdgpu_{target}_little_endian_v2", "index_bits": 64,
        "max_workgroup_invocations": 1024}, "simulator target differs from actual compilation")
    require(simulation.get("shared_buffers") == [], "unexpected shared backing")
    require(simulation.get("conflict_assessment") == {"status": "no_conflicts_observed"},
            "incomplete or conflicting simulation")
    require(simulation.get("schedule", {}).get("coverage", {}).get("complete") is True,
            "incomplete schedule")
    arguments = request["arguments"]
    actual = simulation.get("arguments")
    require(type(arguments) is list and len(arguments) <= 64 and type(actual) is list
            and len(arguments) == len(actual), "complete actual argument census")
    outputs = reference["outputs"]
    require(type(outputs) is list and len(outputs) <= len(arguments), "reference output count")
    by_index = {}
    for row in outputs:
        exact(row, ("argument", "element", "bytes", "absolute_tolerance", "relative_tolerance"), "reference output")
        index = row["argument"]
        require(type(index) is int and 0 <= index < len(arguments) and index not in by_index,
                "unique output argument")
        for key in ("absolute_tolerance", "relative_tolerance"):
            require(type(row[key]) in (float, int) and math.isfinite(row[key])
                    and 0 <= row[key] <= 0.001, "finite reference tolerance bound")
        by_index[index] = row
    checked = 0
    for index, (argument, observed) in enumerate(zip(arguments, actual)):
        require(type(argument) is dict and type(observed) is dict, "argument object")
        if argument.get("kind") == "scalar":
            exact(argument, ("kind", "type", "bits"), "scalar")
            require(observed == argument and index not in by_index, "scalar changed")
            continue
        exact(argument, ("kind", "element", "access", "alignment", "bytes"), "buffer")
        require(argument["kind"] == "buffer" and argument["element"] in WIDTHS,
                "current reference buffer profile")
        exact(observed, ("kind", "value"), "observed buffer")
        require(observed["kind"] == "buffer", "observed buffer kind")
        value = observed["value"]
        exact(value, ("element", "access", "alignment", "bytes", "initialized"), "observed bytes")
        require(all(value[key] == argument[key] for key in ("element", "access", "alignment")),
                "actual buffer ABI changed")
        before = hex_bytes(argument["bytes"])
        after = hex_bytes(value["bytes"])
        require(len(before) == len(after) and len(after) % WIDTHS[argument["element"]] == 0,
                "actual byte extent mismatch")
        mask = bytearray([255] * ((len(after) + 7) // 8))
        if len(after) % 8:
            mask[-1] = (1 << (len(after) % 8)) - 1
        require(hex_bytes(value["initialized"]) == mask, "actual output initialization differs")
        if argument["access"] == "read_only":
            require(index not in by_index and before == after, "readonly input changed")
        else:
            require(argument["access"] in ("write_only", "read_write") and index in by_index,
                    "complete writable output census")
            row = by_index.pop(index)
            require(row["element"] == argument["element"], "reference output element mismatch")
            wanted = hex_bytes(row["bytes"])
            require(len(wanted) == len(after), "reference output extent mismatch")
            if argument["element"] == "f32" and (row["absolute_tolerance"] or row["relative_tolerance"]):
                for (got,), (want,) in zip(struct.iter_unpack("<f", after), struct.iter_unpack("<f", wanted)):
                    require(math.isfinite(got) and math.isfinite(want)
                            and abs(got - want) <= row["absolute_tolerance"] + row["relative_tolerance"] * abs(want),
                            "CPU reference numerical mismatch")
            else:
                require(row["absolute_tolerance"] == row["relative_tolerance"] == 0,
                        "non-f32 reference must be exact")
                require(wanted == after, "CPU reference byte mismatch")
        checked += len(after)
    require(not by_index, "unconsumed reference output")
    return {"requestSha256": hashlib.sha256(request_bytes).hexdigest(),
            "kernel": request["kernel"], "checkedBufferBytes": checked,
            "canonicalKirVersion": 18, **NO_AUTHORITY}


def run_case(case, graph_directory, request_path, reference_executable, simulator_executable,
             output, command_runner, *, environment, cwd, timeout=120):
    """Only called after the same invocation's complete production census passes."""
    require(case["status"] == "production-compile-census-pass" and not case["expectedNegative"],
            "positive live production census required")
    request_bytes, request_pin = read_file(request_path)
    request = strict_json(request_bytes)
    rows = case["productionCensus"]["rows"]
    matches = [(row, kernel) for row in rows for kernel in row["kernels"]
               if kernel["logical_name"] == request["kernel"]]
    require(len(matches) == 1, "request kernel must join one actual production root")
    row, kernel = matches[0]
    require(row["target"] == case["fixture"]["target"], "simulation census CPU substitution")
    graph, length = row["graphs"][3]
    graph_id = digest_array(graph)
    path = graph_directory / (graph_id + ".kir-v18")
    graph_bytes, graph_pin = read_file(path, MAX_GRAPH)
    require(len(graph_bytes) == length and graph_identity(graph_bytes) == graph_id,
            "captured V18 graph differs from actual source/optimizer receipt")
    tools = [Path(reference_executable), Path(simulator_executable)]
    tool_pins = [pin_executable(tool) for tool in tools]
    output.mkdir(mode=0o700, parents=False, exist_ok=False)
    # The adapter reads the identical pinned request file, not a regenerated seed.
    reference_log = output / "reference.log"
    reference = command_runner([str(reference_executable), "execute", str(request_path)], cwd,
                               environment, reference_log, timeout, MAX_DOCUMENT)
    require(command_passed(reference),
            "same-input CPU reference command failed")
    reference_result = strict_json(read_file(reference_log)[0])
    observations = []
    for ordinal in range(2):
        result_path = output / f"simulation-{ordinal}.json"
        outcome = command_runner(simulation_command(simulator_executable, path, request_path,
                                                     result_path, row["target"]),
                                 cwd, environment, output / f"simulation-{ordinal}.log", timeout, MAX_DOCUMENT)
        require(command_passed(outcome), "actual V18 simulation failed")
        payload, pin = read_file(result_path)
        result = strict_json(payload)
        comparison = compare(request_bytes, reference_result, result,
                             {"sha256": graph_id, "canonical_bytes": length}, case["fixture"]["target"])
        observations.append((payload, pin, comparison, outcome))
    require(observations[0][0] == observations[1][0], "same-input simulation was not deterministic")
    require(read_file(request_path)[1] == request_pin and read_file(path, MAX_GRAPH)[1] == graph_pin
            and [pin_executable(tool) for tool in tools] == tool_pins,
            "simulation inputs/tools changed")
    return {"schema": "fe2o3-tutorial-current-simulation-v92", "request": request_pin,
            "graph": graph_pin, "canonicalGraphIdentity": graph_id,
            "kernelContract": kernel["contract"], "productionFinalization": row["finalization"],
            "tools": tool_pins, "referenceCommand": reference,
            "observations": [{"result": pin, "comparison": comparison, "execution": outcome}
                             for _, pin, comparison, outcome in observations],
            "sameInputReferencePassed": True, "simulationPassed": True, **NO_AUTHORITY}
