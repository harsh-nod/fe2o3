#!/usr/bin/env python3
"""Bounded shared-host ordered-list campaign; no reservation or parity claim."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import hashlib
import importlib.util
from pathlib import Path
import resource
import stat
import tarfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
HOT_SHA = "820ad87e74a1f9915c2eb7d2d7c7c6c1c451da4cecf29fcc381229324d98473b"
PREFIX = "/home/harsh/fe2o3-xgmi-segments-20260920."
ORDER = ("kfd", "hsa", "hip", "hip", "hsa", "kfd")
CONTROLS = dict(useful_bytes=65536, descriptor_count=65, warmups=2, samples=10)
PAYLOAD = {"native.py", "results.py", "hot.py", "base.py", "source.tar.gz", "kfd-segments"}
BINARIES = {"kfd": "kfd-segments", "hip": "hip-segments", "hsa": "hsa-segments"}
IDENTITIES = [
    ("hipcc", ["/opt/rocm/bin/hipcc", "--version"], 30),
    ("g++", ["g++", "--version"], 30),
    ("rocm", ["/bin/cat", "/opt/rocm/.info/version"], 30),
]


def load(path, digest, name):
    if path.is_symlink() or not path.is_file() or hashlib.sha256(path.read_bytes()).hexdigest() != digest:
        raise RuntimeError("helper digest: " + str(path))
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


hot = HERE / "hot.py"
if not hot.exists():
    hot = ROOT / "docs/evidence/dev-xgmi-peer-hot-mi300x-2026-09-19/native.py"
H = load(hot, HOT_SHA, "segments_admission")
B = H.B
B.PREFIX = PREFIX
need, sha = H.need, H.sha


def build_specs(owned):
    return [
        ("build-hip", ["/opt/rocm/bin/hipcc", "-std=c++17", "-O3", "-Wall", "-Wextra", "-Werror",
                       "--offload-arch=gfx942", "benchmarks/runtime_gfx942/xgmi_peer_hip.cpp",
                       "-o", str(owned / BINARIES["hip"])], 180),
        ("build-hsa", ["g++", "-std=c++17", "-O3", "-Wall", "-Wextra", "-Werror",
                       "-I/opt/rocm/include", "benchmarks/runtime_gfx942/xgmi_peer_hsa.cpp",
                       "-L/opt/rocm/lib", "-Wl,-rpath,/opt/rocm/lib", "-lhsa-runtime64",
                       "-o", str(owned / BINARIES["hsa"])], 180),
    ]


def trial_specs(owned, devices):
    uids = [device[2] for device in devices]
    controls = [str(CONTROLS[key]) for key in ("useful_bytes", "descriptor_count", "warmups", "samples")]
    result = []
    for index, backend in enumerate(ORDER):
        env = H.environment(owned)
        command = [str(owned / BINARIES[backend])]
        if backend == "kfd":
            command += [*uids, *controls]
        else:
            env["HSA_XNACK"] = "0"
            env["HIP_VISIBLE_DEVICES" if backend == "hip" else "ROCR_VISIBLE_DEVICES"] = ",".join(str(d[0]) for d in devices)
            command += ["0", "1", controls[0], "1", controls[2], controls[3], *uids, "--ordered-segments", controls[1]]
        result.append((f"{index + 1}-{backend}", backend, command, env))
    return result


def run_native(marker):
    owned = B.owned_path(marker)
    need(HERE == owned, "private runner location")
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    rec = B.Recorder(owned / "results", owned)
    source, env = owned / "source", H.environment(owned)
    binding, binaries, results, failures = None, None, [], []

    def bound():
        need(sha(owned / "binding.json") == marker["binding_sha256"], "binding digest")
        value = H.parse_json((owned / "binding.json").read_bytes())
        need(value["commit"] == marker["commit"] and set(value["payload"]) == PAYLOAD, "bound source and payload roster")
        need(value["controls"] == CONTROLS and value["order"] == list(ORDER), "fixed campaign controls")
        for name, digest in value["payload"].items():
            need(sha(owned / name) == digest, "unchanged payload: " + name)
        return value

    def identities():
        need(bound() == binding, "unchanged binding")
        B.source_clean(source, binding)
        if binaries is not None:
            need({key: sha(owned / name) for key, name in BINARIES.items()} == binaries, "unchanged benchmark binaries")

    def observe(label):
        errors = []
        for index, bdf, uid in binding["devices"]:
            try:
                folder = rec.run(label + "-gpu" + str(index), H.observe_spec(label, index, bdf, uid), 100, env=env)
                need((folder / "stderr").read_bytes() == b"", "empty observer stderr")
                H.parse_endpoint((folder / "stdout").read_bytes(), index, bdf, uid)
            except BaseException as error:
                errors.append(error)
        if errors:
            raise errors[0]

    try:
        need({p.name for p in owned.iterdir()} == {"owner.json", "binding.json", "results", *PAYLOAD}, "initial payload closure")
        for path in owned.iterdir():
            if path.name != "results":
                need(stat.S_ISREG(path.lstat().st_mode), "ordinary payload file")
        binding = bound()
        parser = load(owned / "results.py", binding["payload"]["results.py"], "segments_results")
        source.mkdir()
        (owned / "tmp").mkdir()
        with tarfile.open(owned / "source.tar.gz", "r:gz") as archive:
            B.validate_members(archive.getmembers(), binding["source_files"])
            archive.extractall(source, filter="data")
        identities()
        B.write_json(rec.output / "source-before.json", B.inventory(source))
        rec.cwd = source
        for name, command, seconds in IDENTITIES + build_specs(owned):
            rec.run(name, command, seconds, env=env)
        binaries = {key: sha(owned / name) for key, name in BINARIES.items()}
        B.write_json(rec.output / "binaries.json", binaries)
        for name, backend, command, phase_env in trial_specs(owned, binding["devices"]):
            identities()
            failure = None
            try:
                observe(name + "-before")
                folder = rec.run(name, command, 180, env=phase_env)
                need((folder / "stderr").read_bytes() == b"", "empty workload stderr")
                parsed = parser.parse_receipt((folder / "stdout").read_bytes(), backend=backend,
                    unique_ids=[int(d[2], 16) for d in binding["devices"]], **CONTROLS)
                results.append({"trial": name, "result": parsed})
            except BaseException as error:
                failure = error
            failure = B.settled_postflight(observe, name, failure)
            if failure is not None:
                raise failure
        need(len(results) == len(ORDER), "complete six-trial roster")
    except BaseException as error:
        failures.append(repr(error))
    finally:
        for name, command, seconds in IDENTITIES:
            try:
                folder = rec.run("after-" + name, command, seconds, env=env)
                for stream in ("stdout", "stderr"):
                    need(sha(folder / stream) == sha(rec.output / name / stream), "unchanged tool identity")
            except BaseException as error:
                failures.append(repr(error))
        try:
            identities()
            B.write_json(rec.output / "source-after.json", B.inventory(source))
            B.write_json(rec.output / "binaries-after.json", {key: sha(owned / name) for key, name in BINARIES.items()})
        except BaseException as error:
            failures.append(repr(error))
        B.write_json(rec.output / "validated-results.json", results)
        B.write_json(rec.output / "finished.json", {"commit": marker["commit"], "failures": failures,
            "native_execution": not failures, "exclusive_reservation": False,
            "performance_acceptance": False, "formal_refinement": False})
    need(not failures, "native campaign failed: " + repr(failures))


if __name__ == "__main__":
    B.run_native = run_native
    B.main()
