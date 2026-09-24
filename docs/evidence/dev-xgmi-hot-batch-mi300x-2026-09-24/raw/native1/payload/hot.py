#!/usr/bin/env python3
"""Bounded, owned-directory persistent-hot KFD/HSA/HIP comparison."""

import sys

if not sys.flags.isolated:
    raise RuntimeError("run with python3 -I")

import hashlib
import importlib.util
import json
from pathlib import Path
import re
import resource
import stat
import tarfile

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
BASE_SHA = "df740a80c83c94af02f7e7dfb1203dcf7c1eafbde0010e41219e4aac40b34bb7"
BASE_SOURCE = ROOT / "docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py"
OBSERVER = "benchmarks/runtime_gfx942/copy-host-observe.py"
OBSERVER_SHA = "5d37b09bf0823854c6a45b914d866c042c1e65486cd96c6301285a0147ae6c51"
PREFIX = "/home/harsh/fe2o3-xgmi-peer-hot-20260919."
PAYLOAD = ("native.py", "results.py", "base.py", "source.tar.gz")
DEVICES = [
    (1, "0000:26:00.0", "0xab83d2ffef0d3cdf"),
    (2, "0000:46:00.0", "0xd2e26fef80cf5c33"),
]
PLAN = {
    "order": ["kfd", "hsa", "hip", "hip", "hsa", "kfd"],
    "bytes": 1048576,
    "depth": 1,
    "warmups": 10,
    "samples": 30,
    "devices": [1, 2],
    "settled_seconds": 2,
    "delayed_seconds": 20,
    "performance_acceptance": False,
}
BINARIES = {
    "kfd": "target/release/examples/gfx942-runtime-xgmi-peer-benchmark",
    "hsa": "xgmi-peer-hsa",
    "hip": "xgmi-peer-hip",
}
IDENTITY_COMMANDS = [
    ("rustc", ["rustc", "-vV"], 30),
    ("cargo", ["cargo", "-V"], 30),
    ("hipcc", ["/opt/rocm/bin/hipcc", "--version"], 30),
    ("g++", ["g++", "--version"], 30),
    ("rocm", ["cat", "/opt/rocm/.info/version"], 30),
]


def need(value, message):
    if not value:
        raise RuntimeError(message)


def sha(path):
    path = Path(path)
    need(path.is_file() and not path.is_symlink(), "ordinary file: " + str(path))
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_pinned(path, digest, name):
    need(sha(path) == digest, "authenticated helper: " + str(path))
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


base_path = HERE / "base.py"
if not base_path.exists():
    base_path = BASE_SOURCE
B = load_pinned(base_path, BASE_SHA, "peer_hot_owned_runner")
B.PREFIX = PREFIX


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        need(key not in result, "duplicate JSON key")
        result[key] = value
    return result


def parse_json(text):
    def invalid_constant(value):
        raise ValueError("nonfinite JSON constant: " + value)

    return json.loads(
        text, object_pairs_hook=unique_object, parse_constant=invalid_constant
    )


def same_json(left, right):
    return json.dumps(left, sort_keys=True) == json.dumps(right, sort_keys=True)


def environment(owned):
    return {
        "HOME": "/home/harsh",
        "USER": "harsh",
        "PATH": "/home/harsh/.cargo/bin:/opt/rocm/bin:/usr/bin:/bin",
        "LANG": "C",
        "LC_ALL": "C",
        "CARGO_TARGET_DIR": str(owned / "target"),
        "CARGO_INCREMENTAL": "0",
        "CARGO_BUILD_JOBS": "2",
        "CARGO_TERM_COLOR": "never",
        "RUSTUP_TOOLCHAIN": "nightly-2026-04-03",
        "TMPDIR": str(owned / "tmp"),
    }


def build_specs(owned):
    return IDENTITY_COMMANDS + [
        (
            "build-kfd",
            [
                "cargo", "build", "--frozen", "--release", "-p", "fe2o3-runtime",
                "--example", "gfx942-runtime-xgmi-peer-benchmark",
            ],
            1200,
        ),
        (
            "build-hip",
            [
                "/opt/rocm/bin/hipcc", "-std=c++17", "-O3", "-Wall", "-Wextra",
                "-Werror", "--offload-arch=gfx942",
                "benchmarks/runtime_gfx942/xgmi_peer_hip.cpp",
                "-o", str(owned / BINARIES["hip"]),
            ],
            180,
        ),
        (
            "build-hsa",
            [
                "g++", "-std=c++17", "-O3", "-Wall", "-Wextra", "-Werror",
                "-I/opt/rocm/include", "benchmarks/runtime_gfx942/xgmi_peer_hsa.cpp",
                "-L/opt/rocm/lib", "-Wl,-rpath,/opt/rocm/lib", "-lhsa-runtime64",
                "-o", str(owned / BINARIES["hsa"]),
            ],
            180,
        ),
    ]


def final_specs(owned):
    return [("after-" + name, command, seconds) for name, command, seconds in IDENTITY_COMMANDS]


def trial_specs(owned):
    result = []
    arguments = [str(PLAN[key]) for key in ("bytes", "depth", "warmups", "samples")]
    uids = [device[2] for device in DEVICES]
    for index, backend in enumerate(PLAN["order"]):
        env = environment(owned)
        command = [str(owned / BINARIES[backend])]
        if backend == "kfd":
            command += [*uids, *arguments, "--aggregate-peer-batch-hot-only"]
        else:
            env["HSA_XNACK"] = "0"
            env["ROCR_VISIBLE_DEVICES" if backend == "hsa" else "HIP_VISIBLE_DEVICES"] = "1,2"
            command += ["0", "1", *arguments, *uids, "--persistent-hot"]
        result.append((str(index + 1) + "-" + backend, backend, command, env))
    return result


def observe_spec(label, index, bdf, uid):
    return [
        "/usr/bin/python3", "-I", OBSERVER, "--gpu-index", str(index),
        "--pci-bdf", bdf, "--unique-id", uid,
    ]


def observer_module():
    source = HERE / "source" / OBSERVER
    if not source.exists():
        source = ROOT / OBSERVER
    return load_pinned(source, OBSERVER_SHA, "peer_hot_endpoint_parser")


def stamp(value):
    need(
        type(value) is dict and set(value) == {"utc", "monotonic_ns"}
        and type(value["monotonic_ns"]) is int and value["monotonic_ns"] > 0
        and type(value["utc"]) is str
        and re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{9}Z", value["utc"]),
        "complete endpoint timestamp",
    )
    return value["monotonic_ns"]


def parse_endpoint(data, index, bdf, uid):
    need(
        type(data) is bytes and len(data) <= 4 * 1024 * 1024
        and data.endswith(b"\n") and b"\r" not in data and b"\0" not in data,
        "complete bounded ASCII endpoint transcript",
    )
    lines = data.decode("ascii").splitlines()
    need(len(lines) == 2, "exact two-row endpoint transcript")
    observation, complete = map(parse_json, lines)
    need(same_json(complete, {
        "schema": "fe2o3.copy-host-observation.v1", "record": "complete",
        "observations": 1, "refused": 0, "all_endpoints_admitted": True,
        "performance_accepted": False,
    }), "one complete admitted endpoint")
    keys = {
        "schema", "record", "index", "gpu_index", "pci_bdf", "unique_id",
        "started", "finished", "status", "pids", "sysfs", "endpoint_admitted",
        "reasons", "selected_pids", "scope", "visibility_filters",
        "vram_limit_exclusive",
    }
    need(type(observation) is dict and set(observation) == keys, "exact endpoint schema")
    fixed = {
        "schema": "fe2o3.copy-host-observation.v1", "record": "observation",
        "index": 0, "gpu_index": index, "pci_bdf": bdf, "unique_id": uid,
        "endpoint_admitted": True, "reasons": [], "selected_pids": [],
        "scope": "sequential-endpoint-observations-not-continuous-monitoring-or-reservation",
        "visibility_filters": "removed-for-cli", "vram_limit_exclusive": 512 * 1024 * 1024,
    }
    need(same_json({key: observation[key] for key in fixed}, fixed), "exact endpoint claims")
    observer = observer_module()
    snapshots = observation["sysfs"]
    need(type(snapshots) is list and len(snapshots) == 3, "three raw sysfs snapshots")
    for snapshot in snapshots:
        need(
            type(snapshot) is dict
            and set(snapshot) == {"started", "finished", "path", "values", "errors"}
            and snapshot["path"] == "/sys/bus/pci/devices/" + bdf
            and snapshot["errors"] == {} and type(snapshot["values"]) is dict
            and set(snapshot["values"]) == set(observer.METRICS),
            "complete raw sysfs capture",
        )
        values = snapshot["values"]
        need(values["unique_id"].lower() == uid[2:], "raw sysfs GPU identity")
        for name in observer.METRICS:
            if name != "unique_id":
                value = observer.decimal(values[name], 100 if name.endswith("percent") else (1 << 64) - 1)
                if name.endswith("percent"):
                    need(value == 0, "raw sysfs GPU and memory engines idle")
                elif name == "mem_info_vram_used":
                    need(value < 512 * 1024 * 1024, "raw sysfs VRAM below limit")
    prefix = ["/usr/bin/timeout", "--kill-after=5s", "20s", "/opt/rocm/bin/rocm-smi"]
    for name, arguments in (
        ("status", ["--showuse", "--showmeminfo", "vram", "--showuniqueid", "--showbus", "--json"]),
        ("pids", ["--showpidgpus"]),
    ):
        captured = observation[name]
        need(
            type(captured) is dict
            and set(captured) == {"command", "started", "finished", "exit", "error", "stdout", "stderr"}
            and captured["command"] == prefix + arguments
            and type(captured["exit"]) is int and captured["exit"] == 0
            and captured["error"] is None and type(captured["stderr"]) is str
            and not captured["stderr"].strip() and type(captured["stdout"]) is str,
            "successful exact SMI capture",
        )
    status = observer.parse_status(observation["status"]["stdout"], index, bdf, uid)
    need(status["busy_percent"] == 0 and status["vram_bytes"] < 512 * 1024 * 1024, "raw SMI idle and VRAM")
    pids = observer.parse_pids(observation["pids"]["stdout"])
    need(not any(index in devices for devices in pids.values()), "no raw selected-GPU process attachments")
    previous = stamp(observation["started"])
    for captured in (snapshots[0], observation["status"], snapshots[1], observation["pids"], snapshots[2]):
        started, finished = stamp(captured["started"]), stamp(captured["finished"])
        need(previous <= started <= finished, "chronological endpoint captures")
        previous = finished
    need(previous <= stamp(observation["finished"]), "closed endpoint chronology")
    return observation


def checked_binding(owned, marker):
    need(sha(owned / "binding.json") == marker["binding_sha256"], "bound native binding")
    binding = parse_json((owned / "binding.json").read_text())
    need(
        type(binding) is dict and set(binding) == {"commit", "source_files", "payload", "plan"}
        and binding["commit"] == marker["commit"] and same_json(binding["plan"], PLAN),
        "exact native binding and plan",
    )
    files = binding["source_files"]
    need(type(files) is dict and files, "nonempty qualified source map")
    for name, digest in files.items():
        need(
            type(name) is str and name and not name.startswith("/")
            and all(part not in ("", ".", "..") for part in name.split("/"))
            and not any(ord(character) < 32 or character == "\\" for character in name)
            and type(digest) is str and re.fullmatch(r"[0-9a-f]{64}", digest),
            "canonical qualified source identity",
        )
    need(files.get(OBSERVER) == OBSERVER_SHA, "pinned qualified endpoint observer")
    need({
        "benchmarks/runtime_gfx942/xgmi_peer_benchmark_common.hpp",
        "benchmarks/runtime_gfx942/native_benchmark_args.hpp",
        "benchmarks/runtime_gfx942/xgmi_peer_hip.cpp",
        "benchmarks/runtime_gfx942/xgmi_peer_hsa.cpp",
        "crates/fe2o3-runtime/examples/gfx942-runtime-xgmi-peer-benchmark.rs",
    } <= set(files), "complete benchmark source and header closure")
    need(type(binding["payload"]) is dict and set(binding["payload"]) == set(PAYLOAD), "exact payload roster")
    for name, digest in binding["payload"].items():
        need(sha(owned / name) == digest, "unchanged payload: " + name)
    need(binding["payload"]["base.py"] == BASE_SHA, "bound ownership helper")
    return binding


def binary_observation(owned, *, complete):
    result = {}
    for name in BINARIES.values():
        path = owned / name
        if complete or path.exists() or path.is_symlink():
            result[name] = sha(path)
    return result


def run_native(marker):
    owned = B.owned_path(marker)
    need(HERE == owned, "executing the private native runner")
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    rec = B.Recorder(owned / "results", owned)
    source, env = owned / "source", environment(owned)
    binding, binaries, failure, secondary, results = None, None, None, [], []

    def record_failure(error, stage):
        nonlocal failure
        if failure is None:
            failure = error
        else:
            secondary.append({"stage": stage, "error": f"{type(error).__name__}: {error}"})

    def identities():
        checked_binding(owned, marker)
        B.source_clean(source, binding)
        if binaries is not None:
            need(binary_observation(owned, complete=True) == binaries, "unchanged three benchmark ELFs")

    def observe(label):
        failures = []
        for index, bdf, uid in DEVICES:
            try:
                folder = rec.run(label + "-gpu" + str(index), observe_spec(label, index, bdf, uid), 100, env=env)
                need((folder / "stderr").read_bytes() == b"", "empty observer stderr")
                parse_endpoint((folder / "stdout").read_bytes(), index, bdf, uid)
            except BaseException as error:
                failures.append(error)
        if failures:
            raise failures[0]

    try:
        need({path.name for path in owned.iterdir()} == {"owner.json", "binding.json", "results", *PAYLOAD}, "exact initial owned closure")
        for name in ("owner.json", "binding.json", *PAYLOAD):
            need(stat.S_ISREG((owned / name).lstat().st_mode), "ordinary initial payload")
        binding = checked_binding(owned, marker)
        parser = load_pinned(owned / "results.py", binding["payload"]["results.py"], "peer_hot_results")
        source.mkdir()
        (owned / "tmp").mkdir()
        with tarfile.open(owned / "source.tar.gz", "r:gz") as archive:
            B.validate_members(archive.getmembers(), binding["source_files"])
            archive.extractall(source, filter="data")
        B.source_clean(source, binding)
        B.write_json(rec.output / "source-before.json", B.inventory(source))
        rec.cwd = source
        for name, command, seconds in build_specs(owned):
            rec.run(name, command, seconds, env=env)
        binaries = binary_observation(owned, complete=True)
        B.write_json(rec.output / "binaries.json", binaries)
        for name, backend, command, phase_env in trial_specs(owned):
            identities()
            trial_failure = None
            try:
                observe(name + "-before")
                folder = rec.run(name, command, 120, env=phase_env)
                need((folder / "stderr").read_bytes() == b"", "empty workload stderr")
                result = parser.parse_result((folder / "stdout").read_bytes(), backend, [device[2] for device in DEVICES])
                results.append({"trial": name, "result": result})
            except BaseException as error:
                trial_failure = error
            trial_failure = B.settled_postflight(observe, name, trial_failure)
            if trial_failure is not None:
                raise trial_failure
        need(len(results) == len(PLAN["order"]), "complete six-trial result roster")
    except BaseException as error:
        record_failure(error, "native-execution")
    finally:
        rec.cwd = source if source.is_dir() else owned
        for name, command, seconds in final_specs(owned):
            try:
                folder = rec.run(name, command, seconds, env=env)
                initial = rec.output / name.removeprefix("after-")
                if initial.is_dir():
                    for stream in ("stdout", "stderr"):
                        need(sha(folder / stream) == sha(initial / stream), "unchanged toolchain: " + name)
            except BaseException as error:
                record_failure(error, name)
        for name, snapshot in (
            ("source-after.json", lambda: B.inventory(source)),
            ("binaries-after.json", lambda: binary_observation(owned, complete=False)),
        ):
            try:
                B.write_json(rec.output / name, snapshot())
            except BaseException as error:
                record_failure(error, name)
        if binding is not None and source.is_dir():
            try:
                identities()
            except BaseException as error:
                record_failure(error, "final-identities")
        try:
            B.write_json(rec.output / "validated-results.json", results)
            finished = {
                "exit": 0 if failure is None else 1,
                "commit": marker["commit"],
                "native_execution": failure is None,
                "performance_acceptance": False,
                "formal_refinement": False,
            }
            if failure is not None:
                finished["failure"] = f"{type(failure).__name__}: {failure}"
                finished["secondary_failures"] = secondary
            B.write_json(rec.output / "finished.json", finished)
        except BaseException as error:
            record_failure(error, "result-finalization")
    if failure is not None:
        raise failure


def main():
    B.run_native = run_native
    B.main()


if __name__ == "__main__":
    main()
