#!/usr/bin/env python3
"""Owned, fail-closed native retained-series campaign; no parity acceptance."""

import argparse
import base64
import hashlib
import importlib.util
import os
from pathlib import Path
import re
import resource
import shutil
import signal
import stat
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
HOT = "docs/evidence/dev-xgmi-peer-hot-mi300x-2026-09-19/native.py"
HOT_SHA256 = "820ad87e74a1f9915c2eb7d2d7c7c6c1c451da4cecf29fcc381229324d98473b"
SOURCE_ROOTS = ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", "crates",
                "examples", "benchmarks/runtime_gfx942", HOT,
                "docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py",
                "docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/.gitattributes")
CONTROLS = {"copy_bytes": 1048576, "warmups": 2, "samples": 10}


def load_helpers():
    if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
        raise RuntimeError("use python3 -I -B")
    path = ROOT / HOT
    if path.is_symlink() or hashlib.sha256(path.read_bytes()).hexdigest() != HOT_SHA256:
        raise ValueError("reviewed admission helper changed")
    spec = importlib.util.spec_from_file_location("series_owned_admission", path)
    hot = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(hot)
    sys.path.insert(0, str(HERE))
    import xgmi_peer_series_campaign as planner
    import xgmi_peer_series_observations as observations
    return hot, planner, observations


def fresh_output(path):
    if not path.is_absolute() or path.resolve() != path or path.exists() or path.is_symlink():
        raise ValueError("output must be a fresh canonical absolute path")
    if not path.parent.is_dir() or path.is_relative_to(ROOT):
        raise ValueError("output parent must exist outside the source tree")
    path.mkdir(mode=0o700)
    return path


def build_specs(binaries):
    return [
        ("build-kfd", ["cargo", "build", "--frozen", "--release", "-p", "fe2o3-kfd",
                       "--features", "live-validation", "--example", "kfd-sdma-xgmi-peer-benchmark"], 1200),
        ("build-hip", ["/opt/rocm/bin/hipcc", "-std=c++17", "-O3", "-Wall", "-Wextra",
                       "-Werror", "--offload-arch=gfx942",
                       "benchmarks/runtime_gfx942/xgmi_peer_hip.cpp", "-o", str(binaries / "hip-series")], 180),
        ("build-hsa", ["g++", "-std=c++17", "-O3", "-Wall", "-Wextra", "-Werror",
                       "-I/opt/rocm/include", "benchmarks/runtime_gfx942/xgmi_peer_hsa.cpp",
                       "-L/opt/rocm/lib", "-Wl,-rpath,/opt/rocm/lib", "-lhsa-runtime64",
                       "-o", str(binaries / "hsa-series")], 180),
    ]


def query_specs(binaries, devices, environment, clear_environment):
    result = []
    for backend in ("kfd", "hsa", "hip"):
        env = {key: value for key, value in environment.items() if key not in clear_environment}
        env["HSA_XNACK"] = "0"
        command = [str(binaries / (backend + "-series")), "--inspect-peer-pair"]
        if backend == "kfd":
            command += ["0x" + device["unique_id"] for device in devices]
        else:
            env["HIP_VISIBLE_DEVICES" if backend == "hip" else "ROCR_VISIBLE_DEVICES"] = ",".join(
                str(device["physical_index"]) for device in devices)
        result.append((backend, command, env))
    return result


def source_snapshot(rec, label, commit, environment, hot):
    head = rec.run(label + "-head", ["/usr/bin/git", "--no-replace-objects", "-c", "gc.auto=0", "rev-parse", "HEAD"],
                   30, env=environment)
    hot.need((head / "stdout").read_bytes() == (commit + "\n").encode("ascii"), "approved source commit")
    tree = rec.run(label + "-tree", ["/usr/bin/git", "--no-replace-objects", "-c", "gc.auto=0", "ls-tree", "-rz",
                    "--full-tree", commit, "--", *SOURCE_ROOTS], 30, env=environment)
    manifest = {}
    for row in (tree / "stdout").read_bytes().split(b"\0")[:-1]:
        metadata, name = row.split(b"\t", 1)
        mode, kind, oid = metadata.split(b" ")
        relative = name.decode("ascii")
        hot.need(mode in (b"100644", b"100755") and kind == b"blob" and
                 relative not in manifest and all(part not in ("", ".", "..") for part in relative.split("/")),
                 "ordinary unique signed source entry")
        path = ROOT / relative
        hot.need(path.is_file() and not path.is_symlink(), "present ordinary source: " + relative)
        data = path.read_bytes()
        observed = hashlib.sha1(b"blob " + str(len(data)).encode("ascii") + b"\0" + data).hexdigest()
        hot.need(observed == oid.decode("ascii"), "source blob differs: " + relative)
        manifest[relative] = hashlib.sha256(data).hexdigest()
    actual = set()
    for relative in SOURCE_ROOTS:
        path = ROOT / relative
        if relative == ".cargo" and not any(name == relative or name.startswith(relative + "/") for name in manifest):
            hot.need(not path.exists() and not path.is_symlink(), "unsigned optional source root")
            continue
        hot.need(not path.is_symlink(), "source root symlink")
        for member in path.rglob("*") if path.is_dir() else (path,):
            hot.need(not member.is_symlink(), "source symlink")
            if not member.is_dir():
                hot.need(member.is_file(), "ordinary source member")
                actual.add(member.relative_to(ROOT).as_posix())
    hot.need(manifest and actual == set(manifest), "exact build/helper source closure")
    for folder in (head, tree):
        hot.need(not (folder / "stderr").read_bytes(), "empty Git stderr")
    return manifest


def file_identity(path):
    resolved = path.resolve(strict=True)
    info = resolved.stat()
    if not stat.S_ISREG(info.st_mode) or not info.st_mode & 0o111:
        raise ValueError("ordinary executable required: " + str(path))
    with resolved.open("rb") as stream:
        return {"invocation": str(path), "resolved": str(resolved),
                "sha256": hashlib.file_digest(stream, "sha256").hexdigest()}


def tool_snapshot(rec, label, environment, hot):
    tools = {}
    for name in ("git", "rustup", "cargo", "rustc", "g++", "hipcc", "python3", "timeout"):
        path = shutil.which(name, path=environment["PATH"])
        hot.need(path is not None, "tool available: " + name)
        tools[name] = file_identity(Path(path))
    for name in ("cargo", "rustc"):
        folder = rec.run(label + "-resolve-" + name,
                         ["rustup", "which", "--toolchain", environment["RUSTUP_TOOLCHAIN"], name],
                         30, env=environment)
        hot.need(not (folder / "stderr").read_bytes(), "empty tool resolution stderr")
        path = Path((folder / "stdout").read_text("ascii").strip())
        hot.need(path.is_absolute(), "absolute toolchain path")
        tools["toolchain-" + name] = file_identity(path)
    for name, command, seconds in hot.IDENTITY_COMMANDS:
        folder = rec.run(label + "-" + name, command, seconds, env=environment)
        tools["version-" + name] = {stream: hot.sha(folder / stream) for stream in ("stdout", "stderr")}
    return tools


class FreshRecorder:
    """Bind raw files at the return boundary of each current owned invocation."""

    def __init__(self, output, cwd, hot):
        self.inner = hot.B.Recorder(output, cwd)
        self.output, self.hot = self.inner.output, hot
        self.attempts, self.pins = [], {}

    def run(self, name, command, seconds, **kwargs):
        self.hot.need(name not in self.attempts, "fresh stage name")
        self.attempts.append(name)
        try:
            return self.inner.run(name, command, seconds, **kwargs)
        finally:
            folder = self.output / name
            if all((folder / stream).is_file() for stream in ("receipt.json", "stdout", "stderr")):
                self.pins[name] = {stream: self.hot.sha(folder / stream)
                                   for stream in ("receipt.json", "stdout", "stderr")}


def fresh_census(rec, hot, namespace):
    """Replay only this recorder's fresh receipts; never re-probe their old PIDs."""
    hot.need(os.readlink("/proc/self/ns/pid") == namespace, "unchanged PID namespace")
    hot.need({folder.name for folder in rec.output.iterdir()} == set(rec.attempts) == set(rec.pins),
             "exact freshly attempted and closed stage roster")
    rows, pids = [], set()
    for folder in sorted(rec.output.iterdir()):
        hot.need(folder.is_dir() and not folder.is_symlink(), "ordinary command directory")
        hot.need({path.name for path in folder.iterdir()} == {"receipt.json", "stdout", "stderr"},
                 "complete owned command artifact roster")
        hot.need({stream: hot.sha(folder / stream) for stream in ("receipt.json", "stdout", "stderr")} ==
                 rec.pins[folder.name], "raw artifacts unchanged since owned invocation closed")
        receipt = hot.parse_json((folder / "receipt.json").read_bytes())
        pid = receipt["pid"]
        hot.need(type(pid) is int and pid > 0 and pid not in pids and receipt["group_absent"] is True,
                 "fresh unique closed process-group receipt")
        hot.need(receipt["stdout_sha256"] == hot.sha(folder / "stdout") and
                 receipt["stderr_sha256"] == hot.sha(folder / "stderr"), "unchanged command streams")
        pids.add(pid)
        rows.append({"name": folder.name, "pid": pid, "receipt_sha256": hot.sha(folder / "receipt.json")})
    return {"scope": "fresh-recorder-closed-groups-no-historical-pid-probes",
            "namespace": namespace, "attempt_order": rec.attempts,
            "records": rows, "raw_sha256": rec.pins}


def validate_trial(planner, plan, spec, record):
    options = {}
    if spec["backend"] == "kfd":
        options = {"kfd_gpu_ids": plan["kfd_gpu_ids"], "kfd_engines": plan["kfd_engines"]}
    if record["stderr"]:
        raise ValueError("nonempty workload stderr")
    return planner.parse_result(record["stdout"], backend=spec["backend"],
        unique_ids=["0x" + uid for uid in plan["unique_ids"]], copy_bytes=plan["copy_bytes"], depth=spec["depth"],
        warmups=plan["warmups"], samples=plan["samples"], **options)


def run_campaign(args, hot, planner, observations):
    output = fresh_output(args.output)
    work, binaries = output / "work", output / "binaries"
    work.mkdir(mode=0o700)
    binaries.mkdir(mode=0o700)
    (work / "tmp").mkdir(mode=0o700)
    environment = hot.environment(work)
    rec = FreshRecorder(output / "commands", ROOT, hot)
    namespace = os.readlink("/proc/self/ns/pid")
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    previous = {sig: signal.getsignal(sig) for sig in hot.B.MANAGED}
    for sig in hot.B.MANAGED:
        signal.signal(sig, hot.B.interrupted)
    failures, records = [], []
    source_before = tools_before = host_before = binaries_before = admission = incarnation = plan = None
    host_after = None

    def save(name, value):
        hot.B.write_json(output / (name + ".json"), value)

    def binary_snapshot():
        return {backend: hot.sha(binaries / name) for backend, name in planner.BINARIES.items()}

    def observe(label):
        raw, rows, errors = [], [], []
        for device in args.devices:
            index, bdf, uid = device["physical_index"], device["pci_bdf"], "0x" + device["unique_id"]
            try:
                folder = rec.run(label + "-gpu" + str(index), hot.observe_spec(label, index, bdf, uid),
                                 100, env=environment)
                hot.need(not (folder / "stderr").read_bytes(), "empty physical observer stderr")
                data = (folder / "stdout").read_bytes()
                parsed = hot.parse_endpoint(data, index, bdf, uid)
                raw.append(hashlib.sha256(data).hexdigest())
                rows.append({"physical_index": parsed["gpu_index"], "pci_bdf": parsed["pci_bdf"],
                             "unique_id": parsed["unique_id"][2:]})
            except BaseException as error:
                errors.append(error)
        if errors:
            raise errors[0]
        return rows, observations.digest(raw)

    def query(label, physical):
        raw = {}
        for backend, command, env in query_specs(binaries, args.devices, environment, planner._CLEAR_ENVIRONMENT):
            folder = rec.run(label + "-" + backend, command, 60, env=env)
            hot.need(not (folder / "stderr").read_bytes(), "empty API query stderr")
            raw[backend] = (folder / "stdout").read_bytes()
        return observations.join_admission(physical[0], raw, inventory_sha256=physical[1])

    def host(label, edge):
        command = ["/usr/bin/python3", "-I", "-B", str(HERE / "xgmi_peer_series_host.py"),
                   "--gpu-index", str(args.devices[0]["physical_index"]), "--observation-edge", edge]
        for backend, name in planner.BINARIES.items():
            command += ["--" + backend + "-binary", str(binaries / name)]
        folder = rec.run(label, command, 180, env=environment)
        hot.need(not (folder / "stderr").read_bytes(), "empty host identity stderr")
        value = hot.parse_json((folder / "stdout").read_bytes())
        hot.need(value["schema"] == "fe2o3.xgmi-peer-series-host-observation.v1", "host observation schema")
        planner.validate_environment(value["environment"])
        hot.need(value["context"]["unique_id"] == "0x" + args.devices[0]["unique_id"] and
                 value["context"]["pci_bdf"] == args.devices[0]["pci_bdf"], "host collector physical join")
        for backend, digest in binary_snapshot().items():
            hot.need(value["context"][backend + "_binary_sha256"] == digest, "host collector ELF join")
        return value

    def same_admission(current, current_incarnation):
        hot.need(current_incarnation == incarnation, "unchanged topology incarnation")
        hot.need({key: value for key, value in current.items() if key != "evidence_sha256"} ==
                 {key: value for key, value in admission.items() if key != "evidence_sha256"},
                 "unchanged independently observed pair/routes/visibility")

    try:
        source_before = source_snapshot(rec, "source-before", args.commit, environment, hot)
        save("source-before", source_before)
        tools_before = tool_snapshot(rec, "tools-before", environment, hot)
        save("tools-before", tools_before)
        for name, command, seconds in build_specs(binaries):
            rec.run(name, command, seconds, env=environment)
        shutil.copy2(work / "target/release/examples/kfd-sdma-xgmi-peer-benchmark", binaries / "kfd-series")
        binaries_before = binary_snapshot()
        save("binaries-before", binaries_before)
        host_before = host("host-before", "start")
        save("host-before", host_before)
        admission, incarnation = query("admission", observe("admission-before"))
        hot.need(incarnation["boot_id"] == host_before["context"]["boot_id"], "host/topology boot join")
        save("admission", admission)
        save("incarnation", incarnation)
        inputs = {"physical_indices": [row["physical_index"] for row in args.devices],
                  "unique_ids": [row["unique_id"] for row in args.devices], "admission": admission,
                  "environment_before": host_before["environment"], **CONTROLS}
        plan = planner.trial_specs(**inputs)
        save("plan", plan)
        for spec in plan["trials"]:
            name = spec["name"]
            failure = None
            try:
                hot.need(binary_snapshot() == binaries_before, "unchanged trial ELFs")
                same_admission(*query(name + "-query", observe(name + "-query-before")))
                observe(name + "-before")
                env = dict(environment)
                for variable in spec["clear_environment"]:
                    env.pop(variable, None)
                env.update(spec["environment_overrides"])
                folder = rec.run(name, [str(binaries / spec["binary"]), *spec["arguments"]], 180, env=env)
                record = {**spec, "returncode": 0, "stdout": (folder / "stdout").read_bytes(),
                          "stderr": (folder / "stderr").read_bytes(),
                          "execution_receipt_sha256": hot.sha(folder / "receipt.json")}
                # Reject a malformed successful producer before any subsequent workload.
                validate_trial(planner, plan, spec, record)
                records.append(record)
            except BaseException as error:
                failure = error
            failure = hot.B.settled_postflight(observe, name, failure)
            if failure is not None:
                raise failure
    except BaseException as error:
        failures.append(repr(error))
    finally:
        try:
            try:
                if binaries_before is None:
                    save("admission-after-status", {"performed": False, "status": "not-performed",
                        "reason": "native-binaries-not-built", "native_acceptance": False})
                    raise RuntimeError("not performed: native binaries were not built")
                same_admission(*query("admission-after", observe("admission-after-before")))
            except BaseException as error:
                failures.append("admission-after: " + repr(error))
            failure = hot.B.settled_postflight(observe, "campaign-close", None)
            if failure is not None:
                failures.append("closing idle observation: " + repr(failure))
            for label, action in (
                ("host-after", lambda: host("host-after", "end")),
                ("tools-after", lambda: tool_snapshot(rec, "tools-after", environment, hot)),
                ("source-after", lambda: source_snapshot(rec, "source-after", args.commit, environment, hot)),
                ("binaries-after", binary_snapshot),
            ):
                try:
                    value = action()
                    save(label, value)
                    expected = {"tools-after": tools_before, "source-after": source_before,
                                "binaries-after": binaries_before}.get(label)
                    if label == "host-after":
                        host_after = value
                        hot.need(host_before is not None and value["continuity"] == host_before["continuity"],
                                 "unchanged independently collected host/loader identity")
                    else:
                        hot.need(expected is not None and value == expected, "closing identity: " + label)
                except BaseException as error:
                    failures.append(label + ": " + repr(error))
            try:
                save("fresh-census", fresh_census(rec, hot, namespace))
            except BaseException as error:
                failures.append("fresh census: " + repr(error))
            try:
                serialized = [{**{key: value for key, value in row.items() if key not in ("stdout", "stderr")},
                               **{stream + "_base64": base64.b64encode(row[stream]).decode("ascii")
                                  for stream in ("stdout", "stderr")}} for row in records]
                save("records", serialized)
                if not failures:
                    save("replay", planner.replay_campaign(records, environment_after=host_after["environment"],
                                                           **inputs))
            except BaseException as error:
                failures.append("replay: " + repr(error))
            try:
                # No old PID probing: only the synchronous recorder may terminate its fresh children.
                # A missing closure receipt keeps scratch for inspection instead of deleting it.
                fresh_census(rec, hot, namespace)
                shutil.rmtree(work)
            except BaseException as error:
                failures.append("private scratch cleanup: " + repr(error))
            save("finished", {"commit": args.commit, "failures": failures, "native_execution": not failures,
                              "exclusive_reservation": False, "performance_acceptance": False,
                              "formal_refinement": False, "engine_matching": False})
        finally:
            for sig, handler in previous.items():
                signal.signal(sig, handler)
    hot.need(not failures, "native campaign failed: " + repr(failures))


def parse_arguments(arguments=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--campaign", required=True, action="store_true")
    parser.add_argument("--commit", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--device", nargs=3, metavar=("PHYSICAL_INDEX", "UNIQUE_ID", "PCI_BDF"),
                        required=True, action="append")
    args = parser.parse_args(arguments)
    if not re.fullmatch(r"[0-9a-f]{40}", args.commit) or len(args.device) != 2:
        parser.error("full approved commit and exactly two --device selections required")
    args.devices = []
    for index, uid, bdf in args.device:
        if not re.fullmatch(r"0|[1-9][0-9]*", index) or int(index) >= 1 << 31 or \
                not re.fullmatch(r"[0-9a-f]{16}", uid) or int(uid, 16) == 0 or \
                not re.fullmatch(r"[0-9a-f]{4}:[0-9a-f]{2}:[0-1][0-9a-f]\.[0-7]", bdf):
            parser.error("canonical physical index, nonzero 16-digit UID, and PCI BDF required")
        args.devices.append({"physical_index": int(index), "unique_id": uid, "pci_bdf": bdf})
    for key in args.devices[0]:
        if args.devices[0][key] == args.devices[1][key]:
            parser.error("selected physical endpoints must be distinct")
    return args


if __name__ == "__main__":
    run_campaign(parse_arguments(), *load_helpers())
