#!/usr/bin/env python3
"""Bounded, owned-directory KFD aggregate host-attribution experiment."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("run with python3 -I -B")

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
HOT_SHA = "820ad87e74a1f9915c2eb7d2d7c7c6c1c451da4cecf29fcc381229324d98473b"
HOT_SOURCE = ROOT / "docs/evidence/dev-xgmi-peer-hot-mi300x-2026-09-19/native.py"
BASE_SHA = "df740a80c83c94af02f7e7dfb1203dcf7c1eafbde0010e41219e4aac40b34bb7"
PREFIX = "/home/harsh/fe2o3-xgmi-aggregate-attribution-20260919."
PAYLOAD = (
    "native.py",
    "results.py",
    "hot.py",
    "base.py",
    "source.tar.gz",
)
DEVICES = [
    (1, "0000:26:00.0", "0xab83d2ffef0d3cdf"),
    (2, "0000:46:00.0", "0xd2e26fef80cf5c33"),
]
PLAN = {
    "order": ["off", "on", "on", "off"],
    "bytes": 1_048_576,
    "depth": 1,
    "warmups": 10,
    "samples": 30,
    "devices": [1, 2],
    "settled_seconds": 2,
    "delayed_seconds": 20,
    "performance_acceptance": False,
}
BINARY = "target/release/examples/gfx942-runtime-xgmi-peer-benchmark"
IDENTITY_COMMANDS = [
    ("rustc", ["rustc", "-vV"], 30),
    ("cargo", ["cargo", "-V"], 30),
    ("rocm", ["cat", "/opt/rocm/.info/version"], 30),
]
OBSERVER = "benchmarks/runtime_gfx942/copy-host-observe.py"
REQUIRED_SOURCE = {
    OBSERVER,
    "crates/fe2o3-runtime/Cargo.toml",
    "crates/fe2o3-runtime/examples/gfx942-runtime-xgmi-peer-benchmark.rs",
    "crates/fe2o3-runtime/src/kfd_backend.rs",
    "crates/fe2o3-runtime/src/kfd_backend/xgmi_batch.rs",
    "crates/fe2o3-runtime/src/kfd_backend/xgmi_batch_diagnostic.rs",
}


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
    need(spec is not None and spec.loader is not None, "loadable helper: " + str(path))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


hot_path = HERE / "hot.py"
if not hot_path.exists():
    hot_path = HOT_SOURCE
H = load_pinned(hot_path, HOT_SHA, "aggregate_attribution_hot_runner")
B = H.B
B.PREFIX = PREFIX


def same_json(left, right):
    return json.dumps(left, sort_keys=True) == json.dumps(right, sort_keys=True)


def environment(owned):
    return H.environment(owned)


def build_specs(owned):
    return IDENTITY_COMMANDS + [
        (
            "build-kfd",
            [
                "cargo",
                "build",
                "--frozen",
                "--release",
                "-p",
                "fe2o3-runtime",
                "--features",
                "hardware-diagnostic",
                "--example",
                "gfx942-runtime-xgmi-peer-benchmark",
            ],
            1200,
        )
    ]


def final_specs(_owned):
    return [
        ("after-" + name, command, seconds)
        for name, command, seconds in IDENTITY_COMMANDS
    ]


def trial_specs(owned):
    uids = [device[2] for device in DEVICES]
    arguments = [str(PLAN[key]) for key in ("bytes", "depth", "warmups", "samples")]
    result = []
    for ordinal, mode in enumerate(PLAN["order"], 1):
        flag = (
            "--aggregate-peer-batch-hot-diagnose"
            if mode == "on"
            else "--aggregate-peer-batch-hot-only"
        )
        result.append(
            (
                f"{ordinal}-{mode}",
                mode,
                [str(owned / BINARY), *uids, *arguments, flag],
                environment(owned),
            )
        )
    return result


def observe_spec(label, index, bdf, uid):
    return H.observe_spec(label, index, bdf, uid)


def checked_binding(owned, marker):
    need(sha(owned / "binding.json") == marker["binding_sha256"], "bound native binding")
    binding = H.parse_json((owned / "binding.json").read_text())
    need(
        type(binding) is dict
        and set(binding) == {"commit", "source_files", "payload", "plan"}
        and binding["commit"] == marker["commit"]
        and same_json(binding["plan"], PLAN),
        "exact native binding and plan",
    )
    files = binding["source_files"]
    need(type(files) is dict and files, "nonempty qualified source map")
    for name, digest in files.items():
        need(
            type(name) is str
            and name
            and not name.startswith("/")
            and all(part not in ("", ".", "..") for part in name.split("/"))
            and not any(ord(character) < 32 or character == "\\" for character in name)
            and type(digest) is str
            and re.fullmatch(r"[0-9a-f]{64}", digest),
            "canonical qualified source identity",
        )
    need(REQUIRED_SOURCE <= set(files), "complete aggregate benchmark source closure")
    need(files[OBSERVER] == H.OBSERVER_SHA, "pinned qualified endpoint observer")
    payload = binding["payload"]
    need(type(payload) is dict and set(payload) == set(PAYLOAD), "exact payload roster")
    for name, digest in payload.items():
        need(sha(owned / name) == digest, "unchanged payload: " + name)
    need(payload["hot.py"] == HOT_SHA, "bound endpoint and hot-run helper")
    need(payload["base.py"] == BASE_SHA, "bound process-custody helper")
    return binding


def binary_observation(owned, *, complete):
    path = owned / BINARY
    if complete or path.exists() or path.is_symlink():
        return {BINARY: sha(path)}
    return {}


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
            secondary.append(
                {"stage": stage, "error": f"{type(error).__name__}: {error}"}
            )

    def identities():
        checked_binding(owned, marker)
        B.source_clean(source, binding)
        if binaries is not None:
            need(
                binary_observation(owned, complete=True) == binaries,
                "unchanged KFD benchmark ELF",
            )

    def observe(label):
        failures = []
        for index, bdf, uid in DEVICES:
            try:
                folder = rec.run(
                    label + "-gpu" + str(index),
                    observe_spec(label, index, bdf, uid),
                    100,
                    env=env,
                )
                need((folder / "stderr").read_bytes() == b"", "empty observer stderr")
                H.parse_endpoint((folder / "stdout").read_bytes(), index, bdf, uid)
            except BaseException as error:
                failures.append(error)
        if failures:
            raise failures[0]

    try:
        need(
            {path.name for path in owned.iterdir()}
            == {"owner.json", "binding.json", "results", *PAYLOAD},
            "exact initial owned closure",
        )
        for name in ("owner.json", "binding.json", *PAYLOAD):
            need(stat.S_ISREG((owned / name).lstat().st_mode), "ordinary initial payload")
        binding = checked_binding(owned, marker)
        parser = load_pinned(
            owned / "results.py",
            binding["payload"]["results.py"],
            "aggregate_attribution_results",
        )
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
        for name, mode, command, phase_env in trial_specs(owned):
            identities()
            trial_failure = None
            try:
                observe(name + "-before")
                folder = rec.run(name, command, 300, env=phase_env)
                need((folder / "stderr").read_bytes() == b"", "empty workload stderr")
                result = parser.parse_result(
                    (folder / "stdout").read_bytes(),
                    mode,
                    [device[2] for device in DEVICES],
                )
                results.append({"trial": name, "mode": mode, "result": result})
            except BaseException as error:
                trial_failure = error
            trial_failure = B.settled_postflight(observe, name, trial_failure)
            if trial_failure is not None:
                raise trial_failure
        need(len(results) == len(PLAN["order"]), "complete four-trial result roster")
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
                        need(
                            sha(folder / stream) == sha(initial / stream),
                            "unchanged toolchain: " + name,
                        )
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
                "exclusive_reservation": False,
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
