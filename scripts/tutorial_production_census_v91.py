"""Check same-invocation V89/V90 observations; never reinterpret V7/KIR12 evidence."""

import copy
import hashlib
import json
import os
from pathlib import Path
import re
import stat

KIND = "production-census-v91"
PREFIX = b"[cargo-fe2o3] production-census-v91 "
MAX_BODY = 512 * 1024
MAX_INPUT = 64 * 1024 * 1024
CONFIGS = ("FE2O3_PRODUCTION_BUILD_CONFIG_V1", "FE2O3_PRODUCTION_BUILD_CONFIG_V2")
FIELDS = "schema config unit session invocation generation finalization target descriptor_version typed_receipt_version policy compiler compiler_subject compiler_receipt compiler_policy rustc_invocation semantic_source source_ssa graphs typed_execution proof_runtime proof_tools generated_source artifact artifact_bytes descriptor kernels authority".split()
HASH_FIELDS = "config unit invocation finalization compiler_subject compiler_receipt compiler_policy rustc_invocation semantic_source source_ssa typed_execution proof_runtime generated_source artifact descriptor".split()


def require(condition, message):
    if not condition:
        raise ValueError("production census: " + message)


def canonical(value):
    return json.dumps(value, separators=(",", ":"), ensure_ascii=False, allow_nan=False).encode()


def object_pairs(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, "duplicate JSON field")
        result[key] = value
    return result


def decode(raw):
    return json.loads(raw, object_pairs_hook=object_pairs,
                      parse_constant=lambda _: (_ for _ in ()).throw(ValueError("nonfinite JSON")))


def read_file(path, limit=MAX_INPUT, *, retain=True):
    path = Path(path)
    require(path.is_absolute(), "input path is not absolute")
    fd = os.open(path, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW)
    try:
        before = os.fstat(fd)
        require(stat.S_ISREG(before.st_mode) and 0 < before.st_size <= limit, "input extent/type")
        chunks, remaining, digest = [], before.st_size, hashlib.sha256()
        while remaining:
            chunk = os.read(fd, min(remaining, 65536))
            require(bool(chunk), "truncated input")
            digest.update(chunk)
            if retain:
                chunks.append(chunk)
            remaining -= len(chunk)
        require(not os.read(fd, 1), "growing input")
        after = os.fstat(fd)
        named = os.stat(path, follow_symlinks=False)
        identity = lambda s: (s.st_dev, s.st_ino, s.st_mode, s.st_uid, s.st_gid,
                              s.st_size, s.st_mtime_ns, s.st_ctime_ns)
        require(identity(before) == identity(after) == identity(named), "input changed while reading")
        raw = b"".join(chunks)
        return raw, {"path": str(path), "sha256": digest.hexdigest(),
                     "bytes": before.st_size, "identity": list(identity(before))}
    finally:
        os.close(fd)


def hash_bytes(value, length=32):
    require(isinstance(value, list) and len(value) == length
            and all(type(n) is int and 0 <= n < 256 for n in value) and any(value), "invalid identity")
    return bytes(value)


def hex_hash(value):
    require(isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) and value != "0" * 64,
            "invalid declared SHA-256")
    return bytes.fromhex(value)


def domain_hash(fields):
    digest = hashlib.sha256()
    for field in fields:
        digest.update(len(field).to_bytes(8, "little"))
        digest.update(field)
    return digest.digest()


def config_identity(raw, config):
    worker = config["worker"]
    fields = [b"fe2o3-build-config-transitive-v2", b"production-v2", raw,
              hex_hash(worker["sha256"]), worker["byte_len"].to_bytes(8, "little"),
              hex_hash(worker["worker_build_identity"]), hex_hash(worker["llvm_build_identity"]),
              len(config["providers"]).to_bytes(8, "little")]
    pins = []
    _, pin = read_file(worker["path"], retain=False)
    require(pin["sha256"] == worker["sha256"] and pin["bytes"] == worker["byte_len"], "worker substitution")
    pins.append(pin)
    require(len(config["providers"]) <= 128, "provider count")
    kinds = {"llvm-bitcode": 1, "amdgpu-relocatable": 2, "llvm-text-ir": 3}
    total = 0
    for provider in config["providers"]:
        require(type(provider["byte_len"]) is int and 0 < provider["byte_len"] <= MAX_INPUT - total,
                "aggregate provider bytes")
        total += provider["byte_len"]
        payload, pin = read_file(provider["path"])
        require(pin["sha256"] == provider["sha256"] and pin["bytes"] == provider["byte_len"], "provider substitution")
        fields.extend([bytes([kinds[provider["kind"]]]), hex_hash(pin["sha256"]),
                       len(payload).to_bytes(8, "little"), payload])
        pins.append(pin)
    return domain_hash(fields), pins


def prepare(root, fixture, cargo_fe2o3, environment, output, ordinal):
    """Derive only the observed library selector; all worker/provider policy stays exact."""
    supplied = [environment[name] for name in CONFIGS if environment.get(name)]
    require(len(supplied) == 1, "requires one explicit production config template")
    raw, template_pin = read_file(supplied[0], 1024 * 1024)
    template = decode(raw)
    require(template["format"] in ("fe2o3-production-build-config-v1", "fe2o3-production-build-config-v2"),
            "unknown configuration family")
    config = copy.deepcopy(template)
    inputs = fixture["compilerInput"]
    workspace = (root / inputs["cargoLockPath"]).parent.resolve()
    source = ((root / inputs["packageManifest"]).parent / inputs["cargoTarget"]["sourcePath"]).resolve()
    unit = {"crate_name": inputs["cargoTarget"]["name"], "source": source.relative_to(workspace).as_posix(),
            "working_directory": str(workspace)}
    config.update(format="fe2o3-production-build-config-v2", observation={"kind": KIND}, units=[unit])
    encoded = canonical(config)
    require(len(encoded) <= 1024 * 1024, "config byte bound")
    path = output / f"{ordinal:04d}.production-v2.json"
    with path.open("xb") as stream:
        stream.write(encoded)
    identity, pins = config_identity(encoded, config)
    _, config_pin = read_file(path, 1024 * 1024)
    expected_unit = domain_hash([b"FE2O3/PRODUCTION-SOURCE-ISA-UNIT/V1\0", identity,
        unit["crate_name"].encode(), unit["source"].encode(), unit["working_directory"].encode()])
    compiler = []
    pin_specs = [(environment["CARGO"], "FE2O3_AUTHORITY_CARGO_SHA256_V1"),
        (environment["FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_PATH_V1"], "FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_SHA256_V1"),
        (str(cargo_fe2o3), None), (environment["FE2O3_AUTHORITY_RUSTC_PATH_V1"], "FE2O3_AUTHORITY_RUSTC_SHA256_V1")]
    for executable, declared in pin_specs:
        _, pin = read_file(executable, 512 * 1024 * 1024, retain=False)
        require(os.access(executable, os.X_OK), "compiler input is not executable")
        if declared:
            require(hex_hash(environment[declared]).hex() == pin["sha256"], "compiler executable substitution")
        pins.append(pin)
        compiler.append(list(bytes.fromhex(pin["sha256"])))
    compiler.append(list(hex_hash(environment["FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1"])))
    _, backend = read_file(environment["FE2O3_BACKEND"], 512 * 1024 * 1024, retain=False)
    require(backend["sha256"] == environment["FE2O3_AUTHORITY_BACKEND_SHA256_V1"], "backend substitution")
    pins.extend([backend, template_pin, config_pin])
    compiler.append(list(bytes.fromhex(backend["sha256"])))
    child = dict(environment)
    for name in (*CONFIGS, "FE2O3_PRODUCTION_BUILD_EXPECTED_ID_V1", "FE2O3_PRODUCTION_BUILD_EXPECTED_ID_V2"):
        child.pop(name, None)
    child[CONFIGS[1]] = str(path)
    child["FE2O3_PRODUCTION_BUILD_EXPECTED_ID_V2"] = identity.hex()
    return child, {"config": list(identity), "units": [list(expected_unit)], "compiler": compiler,
                   "target": fixture["target"], "symbols": sorted(inputs["kernelSymbols"]),
                   "pins": pins, "selector": unit}


def check_pins(expected):
    for pin in expected["pins"]:
        _, current = read_file(pin["path"], max(MAX_INPUT, pin["bytes"]), retain=False)
        require(current == pin, "input changed after Cargo")


def check_log(raw, expected):
    lines = [line for line in raw.splitlines() if line.startswith(PREFIX)]
    require(len(lines) == 1, "missing or duplicate completed broker census")
    match = re.fullmatch(re.escape(PREFIX) + rb"frames=([1-9][0-9]*) missing=0 failure=0 encoding=hex:([0-9a-f]+) authority=observation-only", lines[0])
    require(match is not None, "incomplete or malformed broker census")
    require(len(match[2]) <= 2 * MAX_BODY and len(match[2]) % 2 == 0, "aggregate byte bound")
    body = bytes.fromhex(match[2].decode())
    rows = decode(body)
    require(isinstance(rows, list) and len(rows) == int(match[1]) == len(expected["units"]), "complete unit count")
    require(canonical(rows) == body, "noncanonical collection")
    units, sessions, symbols = [], [], []
    for row in rows:
        require(isinstance(row, dict) and list(row) == FIELDS, "census schema fields/order")
        require(row["schema"] == KIND and row["authority"] is False and row["policy"] == 11
                and row["descriptor_version"] == 89 and row["typed_receipt_version"] == 90, "wrong schema/policy/authority")
        for name in HASH_FIELDS:
            hash_bytes(row[name])
        sessions.append(hash_bytes(row["session"], 16))
        require(type(row["generation"]) is int and 0 < row["generation"] < 2**64, "generation")
        require(row["config"] == expected["config"] and row["target"] == expected["target"]
                and row["compiler"] == expected["compiler"], "configuration/target/compiler substitution")
        units.append(row["unit"])
        require(len(row["graphs"]) == 4 and len(row["proof_tools"]) == 5, "graph/tool count")
        for identity in row["proof_tools"]:
            hash_bytes(identity)
        for identity, length in row["graphs"]:
            hash_bytes(identity)
            require(type(length) is int and 0 < length <= MAX_INPUT, "graph extent")
        require(type(row["artifact_bytes"]) is int and 0 < row["artifact_bytes"] <= MAX_INPUT, "artifact extent")
        kernels = row["kernels"]
        require(isinstance(kernels, list) and 0 < len(kernels) <= 256, "kernel count")
        roots, outputs, entries = [], [], []
        for kernel in kernels:
            require(list(kernel) == ["logical_name", "entry_name", "root", "output_function", "contract"], "kernel fields")
            hash_bytes(kernel["contract"])
            for name in (kernel["logical_name"], kernel["entry_name"]):
                require(isinstance(name, str) and 0 < len(name) <= 512 and all(33 <= ord(c) < 127 for c in name), "kernel name")
            require(type(kernel["root"]) is int and type(kernel["output_function"]) is int
                    and 0 <= kernel["output_function"] < 2**32, "kernel coordinates")
            roots.append(kernel["root"])
            outputs.append(kernel["output_function"])
            entries.append(kernel["entry_name"])
            symbols.append(kernel["logical_name"])
        require(sorted(roots) == list(range(len(kernels))) and len(set(outputs)) == len(kernels)
                and len(set(entries)) == len(kernels), "incomplete/duplicate kernel roots")
    require(units == sorted(expected["units"]) and len(set(sessions)) == 1, "unit/session census")
    require(sorted(symbols) == expected["symbols"] and len(set(symbols)) == len(symbols), "manifest kernel census")
    return {"status": "production-compile-census-pass", "rows": rows,
            "collectionSha256": hashlib.sha256(body).hexdigest(), "authority": False}


def reconcile(log_path, outcome, expected):
    require(outcome["exitCode"] == 0 and outcome["logComplete"] is True
            and outcome["directChildReaped"] is True, "Cargo did not complete successfully")
    raw, pin = read_file(log_path, 16 * 1024 * 1024)
    require(pin["sha256"] == outcome["logSha256"] and pin["bytes"] == outcome["logBytes"], "Cargo log substitution")
    check_pins(expected)
    return check_log(raw, expected)
