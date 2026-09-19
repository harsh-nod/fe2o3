#!/usr/bin/env python3
"""Pinned source, command, and transcript contract for peer-batch evidence."""

import sys

if not sys.flags.isolated:
    raise RuntimeError("run with python3 -I")

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]

# Exact signed source and sealed CPU prerequisite for this development run.
SOURCE_COMMIT = "71b85d9e68d3f7f1d56af8e82cb946068547b85b"
CPU_SEAL_SHA256 = "1a22011c43756df691b7e8823bd529c75435cb0c2703e2c3c8bd44d0af384b8c"
TOOLING_BRANCH = "refs/heads/codex/r65-runtime-drain-versions"
CPU_RELATIVE = "docs/evidence/dev-xgmi-peer-batch-cpu-2026-09-18"

TRANSPORT_SHA256 = "8cb41d1d4e0e943c6693b16313bddce23a470e31d57e95922406bd9d20131c7a"
TRANSPORT_SOURCE = (
    ROOT
    / "docs/evidence/dev-topology-prechecked-comparison-mi300x-2026-09-18/protocol.py"
)
HELPERS = {
    "transport.py": (
        TRANSPORT_SOURCE,
        TRANSPORT_SHA256,
    ),
    "attribution.py": (
        ROOT
        / "docs/evidence/dev-xgmi-retirement-attribution-mi300x-2026-09-18/campaign.py",
        "94acb8893839768bf2a6247a81f8504f6d90c35729dc6670ebda4dd474f1acab",
    ),
    "base.py": (
        ROOT / "docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py",
        "df740a80c83c94af02f7e7dfb1203dcf7c1eafbde0010e41219e4aac40b34bb7",
    ),
    "provider.py": (
        ROOT
        / "docs/evidence/dev-topology-prechecked-comparison-mi300x-2026-09-18/provider.py",
        "88648e9aa06e8508d711c20cda0a22f83154cdbcd689b09aa4832cac57f8044f",
    ),
}


def need(value, message):
    if not value:
        raise RuntimeError(message)


def sha(path):
    path = Path(path)
    need(path.is_file() and not path.is_symlink(), "ordinary file: " + str(path))
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_pinned(path, digest, name):
    need(sha(path) == digest, "pinned helper: " + str(path))
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


transport_path = HERE / "transport.py"
if not transport_path.exists():
    transport_path = TRANSPORT_SOURCE
T = load_pinned(transport_path, TRANSPORT_SHA256, "peer_batch_transport")
B, C = T.B, T.C
read, same_json = T.read, T.same_json
SOURCE_PATHS = T.SOURCE_PATHS
SIGNERS, SIGNERS_SHA, SSH = T.SIGNERS, T.SIGNERS_SHA, T.SSH

PREFIX = "/home/harsh/fe2o3-xgmi-peer-batch-20260918."
LOCAL_PREFIX = "fe2o3-xgmi-peer-batch-20260918."
B.PREFIX = PREFIX
STATIC_INPUTS = ("protocol.py", "campaign.py", "verify.py", "test_campaign.py")
PAYLOAD = (
    "protocol.py",
    "campaign.py",
    "transport.py",
    "attribution.py",
    "base.py",
    "provider.py",
    "source.tar.gz",
)
BINARY = "target/release/examples/gfx942-runtime-xgmi-peer-benchmark"
PHASES = [
    ["d1-ordinary-a1", 1, "ordinary", 10, 30, 240],
    ["d1-aggregate-b1", 1, "aggregate", 10, 30, 240],
    ["d1-aggregate-b2", 1, "aggregate", 10, 30, 240],
    ["d1-ordinary-a2", 1, "ordinary", 10, 30, 240],
    ["d2-ordinary-a1", 2, "ordinary", 10, 30, 240],
    ["d2-aggregate-b1", 2, "aggregate", 10, 30, 240],
    ["d2-aggregate-b2", 2, "aggregate", 10, 30, 240],
    ["d2-ordinary-a2", 2, "ordinary", 10, 30, 240],
    ["d63-aggregate-correctness", 63, "aggregate", 0, 1, 900],
]
PLAN = {
    "order": PHASES,
    "bytes": 1_048_576,
    "settled_seconds": 2,
    "delayed_seconds": 20,
    "devices": [1, 2],
    "same_binary": True,
    "performance_acceptance": False,
}


def configured():
    need(re.fullmatch(r"[0-9a-f]{40}", SOURCE_COMMIT), "configure source commit")
    need(
        re.fullmatch(r"[0-9a-f]{64}", CPU_SEAL_SHA256),
        "configure CPU archive seal",
    )
    need(
        re.fullmatch(r"refs/heads/[A-Za-z0-9._/-]+", TOOLING_BRANCH),
        "configure published tooling branch",
    )


def json_digest(value):
    return hashlib.sha256(
        json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()


def toolchain_binding(value):
    need(
        isinstance(value, dict)
        and set(value) == {"rustc_stdout_sha256", "cargo_stdout_sha256"}
        and all(
            isinstance(digest, str) and re.fullmatch(r"[0-9a-f]{64}", digest)
            for digest in value.values()
        ),
        "exact CPU toolchain binding",
    )
    return dict(value)


def validate_toolchain_receipts(folder, expected):
    expected = toolchain_binding(expected)
    for name in ("rustc", "cargo"):
        receipt = Path(folder) / name
        stderr = receipt / "stderr"
        need(
            stderr.is_file() and not stderr.is_symlink() and stderr.read_bytes() == b"",
            "empty native toolchain stderr: " + name,
        )
        need(
            sha(receipt / "stdout") == expected[name + "_stdout_sha256"],
            "CPU/native toolchain equality: " + name,
        )


def cpu_binding():
    configured()
    archive = ROOT / CPU_RELATIVE
    need(sha(archive / "SHA256SUMS") == CPU_SEAL_SHA256, "pinned CPU seal")
    manifest = "".join(
        digest + "  " + name + "\n"
        for name, digest in B.inventory(archive).items()
        if name != "SHA256SUMS"
    )
    need((archive / "SHA256SUMS").read_text() == manifest, "complete CPU seal")
    value = read(archive / "binding.json")
    need(
        set(value)
        == {
            "schema",
            "execution_root",
            "source_base",
            "source_snapshot_sha256",
            "source_files",
            "rosters",
            "tools",
            "document",
            "toolchain",
            "commands",
        }
        and value["schema"] == "fe2o3.xgmi-peer-batch-cpu.v1"
        and re.fullmatch(r"[0-9a-f]{40}", value["source_base"])
        and type(value["source_files"]) is int
        and value["source_files"] > 0,
        "exact CPU binding",
    )
    toolchain_binding(value["toolchain"])
    need(
        re.fullmatch(r"[0-9a-f]{64}", value["source_snapshot_sha256"]),
        "CPU source snapshot identity",
    )
    return value


def cpu_source():
    binding = cpu_binding()
    before = ROOT / CPU_RELATIVE / "raw/source-before/stdout"
    after = ROOT / CPU_RELATIVE / "raw/source-after/stdout"
    need(
        sha(before) == binding["source_snapshot_sha256"]
        and before.read_bytes() == after.read_bytes(),
        "exact qualified before/after source snapshot",
    )
    source = read(before)
    need(
        set(source) == {"base", "files"}
        and source["base"] == binding["source_base"]
        and len(source["files"]) == binding["source_files"],
        "CPU source snapshot schema",
    )
    T.file_map(source["files"])
    return source["files"]


def source_package():
    value, data = T.materialize(ROOT, SOURCE_COMMIT, cpu_source())
    value["tar_bytes"] = len(data)
    return value, data


def pack(destination):
    value, data = source_package()
    need(destination.name == "source.tar.gz", "canonical source archive name")
    with destination.open("xb") as output:
        output.write(data)
    return value


def devices(indices=(1, 2)):
    return C.devices(list(indices))


def environment(owned):
    value = C.environment(owned)
    value["CARGO_TARGET_DIR"] = str(owned / "target")
    return value


def observer_command(device):
    return C.observer_command(device)


def build_specs(owned):
    source, env = owned / "source", environment(owned)
    return [
        ("rustc", ["rustc", "-vV"], 30, source, env),
        ("cargo", ["cargo", "-V"], 30, source, env),
        ("kernel", ["uname", "-a"], 30, source, env),
        ("rocm", ["cat", "/opt/rocm/.info/version"], 30, source, env),
        (
            "build",
            [
                "cargo",
                "build",
                "--frozen",
                "--release",
                "-p",
                "fe2o3-runtime",
                "--example",
                "gfx942-runtime-xgmi-peer-benchmark",
            ],
            1200,
            source,
            env,
        ),
    ]


def phase_specs(owned, selected, phase):
    label, depth, mode, warmups, samples, timeout = phase
    source, env = owned / "source", environment(owned)
    result = [
        (
            label + "-before-gpu" + str(device[0]),
            observer_command(device),
            100,
            source,
            env,
        )
        for device in selected
    ]
    command = [
        str(owned / BINARY),
        *(device[2] for device in selected),
        "1048576",
        str(depth),
        str(warmups),
        str(samples),
    ]
    if mode == "aggregate":
        command.append("--aggregate-peer-batch")
    result.append((label, command, timeout, source, env))
    for suffix in ("settled", "delayed"):
        result.extend(
            (
                label + "-" + suffix + "-gpu" + str(device[0]),
                observer_command(device),
                100,
                source,
                env,
            )
            for device in selected
        )
    return result


def remote_specs(owned, selected):
    result = build_specs(owned) + [
        spec for phase in PHASES for spec in phase_specs(owned, selected, phase)
    ]
    need(len(result) == 68, "exact remote command count")
    need(len(result) == len({spec[0] for spec in result}), "unique remote commands")
    return result


def native_timeout():
    bounds = sum(spec[2] for spec in remote_specs(Path(PREFIX + "0" * 16), devices()))
    return bounds + len(PHASES) * 22 + 600


def integer(value, *, positive=False):
    need(
        isinstance(value, str) and re.fullmatch(r"0|[1-9][0-9]{0,19}", value),
        "bounded canonical decimal",
    )
    result = int(value)
    need(result <= (1 << 64) - 1 and (not positive or result > 0), "u64 value")
    return result


def key_values(line):
    pairs = [part.split("=") for part in line.split(" ")]
    need(all(len(pair) == 2 and all(pair) for pair in pairs), "key-value row")
    result = dict(pairs)
    need(len(result) == len(pairs), "unique result keys")
    return result


def parse_endpoint(data, device):
    need(
        isinstance(data, bytes)
        and data.endswith(b"\n")
        and b"\r" not in data
        and b"\0" not in data,
        "complete ASCII endpoint transcript",
    )
    lines = data.decode("ascii").splitlines()
    need(len(lines) == 2, "exact observer transcript")
    observation, complete = (T.parse_json(line) for line in lines)
    need(
        complete
        == {
            "schema": "fe2o3.copy-host-observation.v1",
            "record": "complete",
            "observations": 1,
            "refused": 0,
            "all_endpoints_admitted": True,
            "performance_accepted": False,
        },
        "one complete admitted endpoint",
    )
    need(
        set(observation)
        == {
            "schema",
            "record",
            "index",
            "gpu_index",
            "pci_bdf",
            "unique_id",
            "started",
            "finished",
            "status",
            "pids",
            "sysfs",
            "endpoint_admitted",
            "reasons",
            "selected_pids",
            "scope",
            "visibility_filters",
            "vram_limit_exclusive",
        }
        and observation["schema"] == "fe2o3.copy-host-observation.v1"
        and observation["record"] == "observation"
        and observation["index"] == 0
        and (
            observation["gpu_index"],
            observation["pci_bdf"],
            observation["unique_id"],
        )
        == tuple(device)
        and observation["endpoint_admitted"] is True
        and observation["reasons"] == []
        and observation["selected_pids"] == []
        and observation["scope"]
        == "sequential-endpoint-observations-not-continuous-monitoring-or-reservation"
        and observation["visibility_filters"] == "removed-for-cli"
        and observation["vram_limit_exclusive"] == 512 * 1024 * 1024,
        "exact strict endpoint identity and policy",
    )
    metrics = {
        "gpu_busy_percent",
        "mem_busy_percent",
        "mem_info_gtt_used",
        "mem_info_vis_vram_used",
        "mem_info_vram_used",
        "unique_id",
    }
    snapshots = observation["sysfs"]
    need(isinstance(snapshots, list) and len(snapshots) == 3, "three sysfs reads")
    for snapshot in snapshots:
        need(
            set(snapshot) == {"started", "finished", "path", "values", "errors"}
            and snapshot["path"] == "/sys/bus/pci/devices/" + device[1]
            and snapshot["errors"] == {}
            and isinstance(snapshot["values"], dict)
            and set(snapshot["values"]) == metrics
            and snapshot["values"]["unique_id"].lower() == device[2][2:].lower(),
            "exact raw sysfs identity",
        )
        values = snapshot["values"]
        need(
            integer(values["gpu_busy_percent"]) == 0
            and integer(values["mem_busy_percent"]) == 0
            and integer(values["mem_info_vram_used"]) < 512 * 1024 * 1024,
            "raw sysfs idle and VRAM",
        )
        for name in ("mem_info_gtt_used", "mem_info_vis_vram_used"):
            integer(values[name])
    return observation


def fixed_fields(selected, phase, measurement):
    _, depth, mode, warmups, samples, _ = phase
    aggregate = mode == "aggregate"
    result = {
        "backend": "kfd",
        "schema": (
            "fe2o3.xgmi-peer-aggregate-benchmark.v1"
            if aggregate
            else "fe2o3.xgmi-peer-benchmark.v1"
        ),
        "surface": "runtime-facade",
        "unique_ids": ",".join(device[2][2:] for device in selected),
        "target": "gfx942:xnack-",
        "bytes": "1048576",
        "depth": str(depth),
        "queue_depth": str(depth),
        "batch_size": str(depth),
        "direction": "forward-then-reverse",
        "outstanding_depth": str(depth),
        "engine_parallelism": "ordered-single-sdma",
        "warmups": str(warmups),
        "samples": str(samples),
        "measurement": measurement,
        "peer_access": "topology-xgmi",
        "mapping_lifetime": (
            "host-access-between-rounds"
            if measurement == "remap-per-round"
            else "persistent-no-host-access-between-timed-rounds"
        ),
        "prime_batches": "0" if measurement == "remap-per-round" else "1",
        "doorbells_per_batch": "1",
        "progress": (
            "explicit-exact-roster-aggregate-wait"
            if aggregate
            else "explicit-flush-then-wait"
        ),
        "background_progress": "false",
        "forward_engine": "topology-selected",
        "reverse_engine": "topology-selected",
        "canaries": "pass",
        "teardown": "explicit",
        "timing": (
            "facade-enqueue-through-aggregate-close"
            if aggregate
            else "facade-enqueue-flush-through-observed-completion"
        ),
    }
    if aggregate:
        result["aggregate_roster"] = "exact-round-submissions"
    return result


def parse_transcript(data, selected, phase):
    need(
        isinstance(data, bytes)
        and data.endswith(b"\n")
        and b"\r" not in data
        and b"\0" not in data,
        "complete ASCII result transcript",
    )
    lines = data.decode("ascii").splitlines()
    need(len(lines) == 2, "exact two-row benchmark transcript")
    metrics = {
        direction + suffix
        for direction in ("forward", "reverse")
        for suffix in ("_p50_ns", "_p95_ns", "_p50_GBps")
    }
    rows = []
    for line, measurement in zip(lines, ("remap-per-round", "persistent-hot")):
        row, fixed = key_values(line), fixed_fields(selected, phase, measurement)
        need(
            set(row) == set(fixed) | metrics
            and all(row[key] == value for key, value in fixed.items()),
            "exact mode, controls, identity and result schema",
        )
        depth = phase[1]
        for direction in ("forward", "reverse"):
            median = integer(row[direction + "_p50_ns"], positive=True)
            upper = integer(row[direction + "_p95_ns"], positive=True)
            bandwidth = row[direction + "_p50_GBps"]
            need(upper >= median, "ordered latency quantiles")
            need(
                re.fullmatch(r"(?:0|[1-9][0-9]*)\.[0-9]{3}", bandwidth),
                "finite rounded bandwidth",
            )
            need(
                abs(float(bandwidth) - 1_048_576 * depth / median) <= 0.00050001,
                "bandwidth agrees with latency",
            )
        rows.append(row)
    return rows


def helper_identities():
    result = {}
    for name, (_, digest) in HELPERS.items():
        path = helper_source(name)
        need(sha(path) == digest, "authenticated payload helper: " + name)
        result[name] = digest
    return result


def helper_source(name):
    need(name in HELPERS, "known helper name")
    local = HERE / name
    return local if local.exists() else HELPERS[name][0]


def validate_binding(binding, cpu=None):
    configured()
    need(
        set(binding)
        == {
            "schema",
            "tooling_commit",
            "source_commit",
            "cpu",
            "tools",
            "helpers",
            "devices",
            "plan",
            "source",
            "payload",
        }
        and binding["schema"] == "fe2o3.xgmi-peer-batch-native.v1",
        "exact native binding schema",
    )
    need(
        re.fullmatch(r"[0-9a-f]{40}", binding["tooling_commit"])
        and binding["source_commit"] == SOURCE_COMMIT,
        "signed commit identities",
    )
    cpu_record = binding["cpu"]
    need(
        set(cpu_record)
        == {
            "relative",
            "seal_sha256",
            "binding_sha256",
            "source_snapshot_sha256",
            "qualified_source_base",
            "toolchain",
        }
        and cpu_record["relative"] == CPU_RELATIVE
        and cpu_record["seal_sha256"] == CPU_SEAL_SHA256
        and re.fullmatch(r"[0-9a-f]{40}", cpu_record["qualified_source_base"])
        and all(
            re.fullmatch(r"[0-9a-f]{64}", cpu_record[name])
            for name in ("binding_sha256", "source_snapshot_sha256")
        ),
        "bounded CPU prerequisite identities",
    )
    toolchain_binding(cpu_record["toolchain"])
    if cpu is not None:
        need(
            cpu_record
            == {
                "relative": CPU_RELATIVE,
                "seal_sha256": CPU_SEAL_SHA256,
                "binding_sha256": sha(ROOT / CPU_RELATIVE / "binding.json"),
                "source_snapshot_sha256": cpu["source_snapshot_sha256"],
                "qualified_source_base": cpu["source_base"],
                "toolchain": toolchain_binding(cpu["toolchain"]),
            },
            "exact local CPU prerequisite",
        )
    need(binding["devices"] == devices(), "exact physical device pair")
    need(same_json(binding["plan"], PLAN), "exact bounded execution plan")
    source = binding["source"]
    need(
        set(source)
        == {
            "tree",
            "source_files",
            "source_modes",
            "tar_sha256",
            "tar_bytes",
        }
        and (cpu is None or source["source_files"] == cpu_source())
        and set(source["source_modes"]) == set(source["source_files"])
        and all(
            mode in ("100644", "100755") for mode in source["source_modes"].values()
        )
        and re.fullmatch(r"[0-9a-f]{40}", source["tree"])
        and re.fullmatch(r"[0-9a-f]{64}", source["tar_sha256"])
        and type(source["tar_bytes"]) is int
        and 0 < source["tar_bytes"] <= T.MAX_TRANSPORT_BYTES,
        "exact bounded signed source transport",
    )
    need(
        set(binding["tools"]) == set(STATIC_INPUTS)
        and binding["helpers"] == helper_identities()
        and set(binding["payload"]) == set(PAYLOAD),
        "exact tool, helper and payload rosters",
    )
    for values in (binding["tools"], binding["payload"]):
        need(
            all(
                isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value)
                for value in values.values()
            ),
            "SHA-256 identities",
        )
    for name in ("protocol.py", "campaign.py"):
        need(binding["payload"][name] == binding["tools"][name], "same launch tool")
    for name, digest in binding["helpers"].items():
        need(binding["payload"][name] == digest, "same authenticated helper")
    need(
        binding["payload"]["source.tar.gz"] == source["tar_sha256"],
        "same signed source archive",
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    value = pack(args.destination)
    print(json.dumps(value, sort_keys=True))


if __name__ == "__main__":
    main()
