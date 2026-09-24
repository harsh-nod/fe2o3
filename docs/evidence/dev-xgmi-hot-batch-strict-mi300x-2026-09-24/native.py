#!/usr/bin/env python3
"""Fixed matched hot-batch trials with fresh shared-host admission."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import hashlib
from pathlib import Path
import resource
import shutil
import stat
import tarfile
from types import ModuleType

HERE = Path(__file__).resolve().parent
REPO = Path("/home/harsh/.codex-tmp/fe2o3-r61-execution")
PREFIX = "/home/harsh/fe2o3-hot-batch-20260924."
HOT_SHA = "820ad87e74a1f9915c2eb7d2d7c7c6c1c451da4cecf29fcc381229324d98473b"
BASE_SHA = "df740a80c83c94af02f7e7dfb1203dcf7c1eafbde0010e41219e4aac40b34bb7"
PARSER_SHA = "52735695f6d03ab09cceaf25e01226bad71b336e86687c034018ece80694c56c"
CONTROLS = dict(bytes=1048576, warmups=10, samples=30)
FORWARD = [(backend, depth) for depth in (1, 16, 32) for backend in ("kfd", "hsa", "hip")]
ORDER = FORWARD + FORWARD[::-1]
PAYLOAD = {"native.py", "hot.py", "base.py", "results.py", "source.tar.gz", "kfd"}
BINARIES = {"kfd": "kfd", "hip": "peer-hip", "hsa": "peer-hsa"}
IDENTITIES = [("hipcc", ["/opt/rocm/bin/hipcc", "--version"], 30),
              ("g++", ["g++", "--version"], 30),
              ("rocm", ["/bin/cat", "/opt/rocm/.info/version"], 30)]
OBSERVE_SECONDS = 100
TRIAL_SECONDS = 300
BUILD_SECONDS = 180
STOP_SECONDS = 15
REMOTE_SECONDS = (2 * sum(seconds + STOP_SECONDS for _, _, seconds in IDENTITIES)
                  + 2 * (BUILD_SECONDS + STOP_SECONDS)
                  + len(ORDER) * (6 * (OBSERVE_SECONDS + STOP_SECONDS)
                                  + TRIAL_SECONDS + STOP_SECONDS + 22) + 300)


def need(value, message):
    if not value:
        raise RuntimeError(message)


def sha(path):
    need(path.is_file() and not path.is_symlink(), "ordinary file: " + str(path))
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load(path, digest, name):
    raw = path.read_bytes()
    need(not path.is_symlink() and hashlib.sha256(raw).hexdigest() == digest,
         "authenticated helper: " + str(path))
    module = ModuleType(name)
    module.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), module.__dict__)
    return module


hot = HERE / "hot.py"
if not hot.exists():
    hot = REPO / "docs/evidence/dev-xgmi-peer-hot-mi300x-2026-09-19/native.py"
H = load(hot, HOT_SHA, "hot_batch_endpoint")
B = H.B
B.PREFIX = PREFIX


def builds(owned):
    return [
        ("build-hip", ["/opt/rocm/bin/hipcc", "-std=c++17", "-O3", "-Wall", "-Wextra", "-Werror",
                       "--offload-arch=gfx942", "benchmarks/runtime_gfx942/xgmi_peer_hip.cpp",
                       "-o", str(owned / BINARIES["hip"])], BUILD_SECONDS),
        ("build-hsa", ["g++", "-std=c++17", "-O3", "-Wall", "-Wextra", "-Werror", "-I/opt/rocm/include",
                       "benchmarks/runtime_gfx942/xgmi_peer_hsa.cpp", "-L/opt/rocm/lib",
                       "-Wl,-rpath,/opt/rocm/lib", "-lhsa-runtime64", "-o", str(owned / BINARIES["hsa"])], BUILD_SECONDS),
    ]


def trials(owned, devices):
    uids = [device[2] for device in devices]
    for ordinal, (backend, depth) in enumerate(ORDER, 1):
        env = H.environment(owned)
        command = [str(owned / BINARIES[backend])]
        controls = [str(CONTROLS["bytes"]), str(depth), str(CONTROLS["warmups"]), str(CONTROLS["samples"])]
        if backend == "kfd":
            command += [*uids, *controls, "--aggregate-peer-batch-hot-only"]
        else:
            env["HSA_XNACK"] = "0"
            env["HIP_VISIBLE_DEVICES" if backend == "hip" else "ROCR_VISIBLE_DEVICES"] = ",".join(str(d[0]) for d in devices)
            command += ["0", "1", *controls, *uids, "--persistent-hot"]
        yield f"{ordinal:02}-{backend}-depth{depth}", backend, depth, command, env


def observe_endpoints(rec, env, devices, label):
    errors = []
    for index, bdf, uid in devices:
        try:
            folder = rec.run(label + "-gpu" + str(index), H.observe_spec(label, index, bdf, uid), OBSERVE_SECONDS, env=env)
            need((folder / "stderr").read_bytes() == b"", "empty observer stderr")
            H.parse_endpoint((folder / "stdout").read_bytes(), index, bdf, uid)
        except BaseException as error:
            errors.append(error)
    if errors:
        raise errors[0]


def run_trials(rec, owned, devices, parser, identities, observe, results):
    for name, backend, depth, command, env in trials(owned, devices):
        identities()
        failure = None
        try:
            observe(name + "-before")
            folder = rec.run(name, command, TRIAL_SECONDS, env=env)
            need((folder / "stderr").read_bytes() == b"", "empty workload stderr")
            parsed = parser.parse_result((folder / "stdout").read_bytes(), backend=backend,
                                         unique_ids=[d[2] for d in devices], copy_bytes=CONTROLS["bytes"],
                                         depth=depth, warmups=CONTROLS["warmups"], samples=CONTROLS["samples"])
            results.append({"trial": name, "backend": backend, "depth": depth, "result": parsed})
        except BaseException as error:
            failure = error
        failure = B.settled_postflight(observe, name, failure)
        if failure is not None:
            raise failure
    need(len(results) == len(ORDER), "complete trial roster")


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
        need(value["commit"] == marker["commit"] and set(value["payload"]) == PAYLOAD, "bound source/payload")
        need(H.same_json(value["controls"], CONTROLS) and H.same_json(value["order"], ORDER), "fixed campaign")
        need(len(value["devices"]) == 2 and all(len({d[i] for d in value["devices"]}) == 2 for i in range(3)), "two distinct endpoints")
        for name, digest in value["payload"].items():
            need(sha(owned / name) == digest, "unchanged payload: " + name)
        need(value["payload"]["hot.py"] == HOT_SHA and value["payload"]["base.py"] == BASE_SHA
             and value["payload"]["results.py"] == PARSER_SHA, "pinned helpers/parser")
        return value

    def identities():
        need(bound() == binding, "unchanged binding")
        B.source_clean(source, binding)
        if binaries is not None:
            need({key: sha(owned / value) for key, value in BINARIES.items()} == binaries, "unchanged ELFs")
            need({key: sha(rec.output / "artifacts" / value) for key, value in BINARIES.items()} == binaries, "retained ELF copies")

    try:
        need({p.name for p in owned.iterdir()} == {"owner.json", "binding.json", "results", *PAYLOAD}, "initial payload closure")
        for path in owned.iterdir():
            if path.name != "results":
                need(stat.S_ISREG(path.lstat().st_mode), "ordinary payload")
        binding = bound()
        parser = load(owned / "results.py", PARSER_SHA, "hot_batch_results")
        source.mkdir()
        (owned / "tmp").mkdir()
        with tarfile.open(owned / "source.tar.gz", "r:gz") as archive:
            B.validate_members(archive.getmembers(), binding["source_files"])
            archive.extractall(source, filter="data")
        identities()
        B.write_json(rec.output / "source-before.json", B.inventory(source))
        rec.cwd = source
        for name, command, seconds in IDENTITIES + builds(owned):
            rec.run(name, command, seconds, env=env)
        (rec.output / "artifacts").mkdir()
        for name in BINARIES.values():
            shutil.copy2(owned / name, rec.output / "artifacts" / name)
        binaries = {key: sha(owned / value) for key, value in BINARIES.items()}
        B.write_json(rec.output / "binaries.json", binaries)
        run_trials(rec, owned, binding["devices"], parser, identities,
                   lambda label: observe_endpoints(rec, env, binding["devices"], label), results)
    except BaseException as error:
        failures.append(repr(error))
    finally:
        for name, command, seconds in IDENTITIES:
            try:
                folder = rec.run("after-" + name, command, seconds, env=env)
                for stream in ("stdout", "stderr"):
                    need(sha(folder / stream) == sha(rec.output / name / stream), "unchanged tools")
            except BaseException as error:
                failures.append(repr(error))
        try:
            identities()
            B.write_json(rec.output / "source-after.json", B.inventory(source))
            B.write_json(rec.output / "binaries-after.json", {key: sha(owned / value) for key, value in BINARIES.items()})
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
