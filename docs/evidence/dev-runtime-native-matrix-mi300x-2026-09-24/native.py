#!/usr/bin/env python3
"""Run one fixed single-GPU matrix; never retry a refused or failed cell."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import hashlib
import json
import os
from pathlib import Path
import resource
import shutil
import signal
import subprocess
import time
from types import ModuleType, SimpleNamespace

HERE = Path(__file__).resolve().parent
PAYLOAD = {"native.py", "protocol.py", "recorder.py", "base.py", "observer.py", "topology.py", "runtime-tests"}
TOPOLOGY_SHA = "f16f8415badc3b44b72b840fe1c3191afc8dea4b577548e45b020f9b99768714"
OBSERVE_SECONDS, TEST_SECONDS = 100, 200
REMOTE_SECONDS = 12000
CASE_START = 6
EXPECTED_CASES = 16


def selected_cases(p):
    cases = p.CASE_NAMES[CASE_START:]
    p.need(len(cases) == EXPECTED_CASES, "exact unexecuted suffix")
    return cases


def load_protocol(root, binding):
    path = root / "protocol.py"
    if path.is_symlink() or not path.is_file():
        raise RuntimeError("ordinary protocol")
    raw = path.read_bytes()
    if hashlib.sha256(raw).hexdigest() != binding["payload"]["protocol.py"]:
        raise RuntimeError("protocol identity")
    value = ModuleType("native_matrix_protocol")
    value.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), value.__dict__)
    return value


def recorder(p, root):
    previous = sys.modules.get("protocol")
    sys.modules["protocol"] = p
    try:
        r = p.load_module(root / "recorder.py", p.RECORDER_SHA, "native_matrix_recorder")
    finally:
        if previous is None:
            del sys.modules["protocol"]
        else:
            sys.modules["protocol"] = previous
    rec = r.Recorder(root)
    rec.spawn_failures = []
    spawn = subprocess.Popen
    ordinal = 0

    def recorded_spawn(*args, **kwargs):
        nonlocal ordinal
        p.need(r.ACTIVE is None or not r.group_exists(r.ACTIVE.pid), "prior active group must close before another spawn")
        child = spawn(*args, **kwargs)
        r.ACTIVE = child
        try:
            ordinal += 1
            folder = rec.output / f"active-{ordinal:03}"
            folder.mkdir()
            r.write(folder / "receipt.json", {"pid": child.pid, "command": args[0], "started": r.stamp(),
                                               "purpose": "record group before unblocking managed signals"})
        except BaseException as error:
            rec.spawn_failures.append({"pid": child.pid, "error": repr(error)})
            try:
                r.stop_owned(child)
            except BaseException as cleanup:
                rec.spawn_failures.append({"pid": child.pid, "cleanup": repr(cleanup)})
            # Return custody even after persistence failure so Recorder can reap,
            # establish T0, and let the caller perform both fixed postflights.
        return child

    r.subprocess = SimpleNamespace(Popen=recorded_spawn)
    return r, rec


def topology(text, p):
    line, = text.splitlines()
    fields = p.pairs([part.split("=", 1) for part in line.removeprefix("topology ").split()])
    required = {"schema", "placement", "gpu_index", "pci_bdf", "unique_id", "numa_node", "device_local_cpu_list",
                "allowed_cpu_list", "allowed_mem_node_list", "measurement_cpu_list", "observer_cpu", "kfd_node",
                "kfd_gpu_id", "topology_sha256"}
    p.need(line.startswith("topology ") and set(fields) == required, "exact topology fields")
    p.need(hashlib.sha256((line.rsplit(" topology_sha256=", 1)[0] + "\n").encode()).hexdigest() == fields["topology_sha256"],
           "topology self digest")
    for name, value in {"schema": "fe2o3.r26-host-topology.v1", "gpu_index": str(p.GPU), "pci_bdf": p.BDF,
                        "unique_id": p.UID, "numa_node": "0", "device_local_cpu_list": "0-47",
                        "allowed_cpu_list": "0-95", "allowed_mem_node_list": "0-1", "measurement_cpu_list": "0-47",
                        "observer_cpu": "95", "placement": "taskset-cpulist-then-numactl-physcpubind-membind-v1"}.items():
        p.need(fields[name] == value, "fixed GPU-local topology: " + name)
    p.need(fields["kfd_node"].isdigit() and fields["kfd_gpu_id"].isdigit() and int(fields["kfd_gpu_id"]) > 0, "KFD topology identity")


def placement(text, p):
    expected = ["policy: bind", "preferred node: 0", "physcpubind: " + " ".join(str(cpu) for cpu in range(48)),
                "cpubind: 0", "nodebind: 0", "membind: 0", "preferred: 0"]
    p.need([line.rstrip() for line in text.splitlines()] == expected, "effective NUMA/CPU placement")


def resources(root, p):
    global_memory = Path("/proc/meminfo").read_text()
    node_memory = Path("/sys/devices/system/node/node0/meminfo").read_text()
    def kb(text, name):
        lines = [line for line in text.splitlines() if name + ":" in line]
        p.need(len(lines) == 1 and lines[0].endswith(" kB"), "memory observation")
        return int(lines[0].split()[-2]) * 1024
    p.need(kb(global_memory, "MemAvailable") >= 4 * 1024**3 and kb(node_memory, "MemFree") >= 4 * 1024**3,
           "four GiB global and selected NUMA free headroom")
    disk = shutil.disk_usage(root)
    p.need(disk.free >= 1024**3, "one GiB remote free disk")
    return {"global": global_memory, "node0": node_memory, "disk_free": disk.free}


def run_cases(p, r, rec, root, observer, identities, cases):
    def observe(label, t0=None, offset=None):
        rec.environment = p.environment(root)
        row, stdout, stderr = rec.run(label, ["/usr/bin/python3", "-I", "-B", str(root / "observer.py"),
                                            "--gpu-index", str(p.GPU), "--pci-bdf", p.BDF, "--unique-id", p.UID], OBSERVE_SECONDS)
        p.need(r.passed(row) and stderr.read_bytes() == b"", "successful strict observer command")
        value = p.endpoint(stdout.read_bytes(), observer, t0=t0, offset=offset)
        p.need(row["spawned"]["monotonic_ns"] <= p.stamp(value["started"]) <= p.stamp(value["finished"])
               <= row["t0"]["monotonic_ns"], "observer enclosed by command")
        return value

    for ordinal, (case, _) in enumerate(p.CASE_NAMES, 1):
        name = f"{ordinal:02}-{case}"
        p.need(not rec.spawn_failures, "no prior unrecorded spawn before case")
        identities()
        r.write(rec.output / (name + "-resources.json"), resources(root, p))
        before = observe(name + "-before")
        p.need(not rec.spawn_failures, "preflight spawn durably recorded before native launch")
        rec.environment = p.environment(root, case)
        row, stdout, stderr = rec.run(name + "-test", p.command(root, case), TEST_SECONDS, p.stamp(before["finished"]))
        item = {"case": case, "ordinal": ordinal, "failures": [], "transcript": None, "post_observations": {}}
        cases.append(item)
        p.need(row["group_absent"] and row["t0"] is not None, "closed test group before observations")
        t0 = row["t0"]["monotonic_ns"]
        for label, offset in (("immediate", 0), ("delayed", 20)):
            try:
                delay = (t0 + offset * 10**9 - time.monotonic_ns()) / 10**9
                if delay > 0:
                    time.sleep(delay)
                observe(name + "-" + label, t0, offset)
                item["post_observations"][label] = "strict_pass"
            except BaseException as error:
                item["post_observations"][label] = "refused"
                item["failures"].append(label + ": " + repr(error))
        try:
            p.need(r.passed(row), "native command passed")
            p.need(not rec.spawn_failures, "all spawned groups durably recorded")
            item["transcript"] = p.transcript(case, stdout.read_text(), stderr.read_text())
        except BaseException as error:
            item["failures"].insert(0, "test: " + repr(error))
        p.need(not item["failures"], "case rejected; no subsequent case: " + repr(item))


def main():
    mode, raw_marker = sys.argv[1:]
    if mode != "run":
        raise RuntimeError("native mode")
    marker = json.loads(raw_marker)
    raw = (HERE / "binding.json").read_bytes()
    if hashlib.sha256(raw).hexdigest() != marker["binding_sha256"]:
        raise RuntimeError("bound runner")
    binding = json.loads(raw)
    p = load_protocol(HERE, binding)
    p.CASE_NAMES = selected_cases(p)
    b = p.load_module(HERE / "base.py", p.BASE_SHA, "native_matrix_ownership")
    b.PREFIX = p.PREFIX
    root = b.owned_path(marker)
    p.need(HERE == root and marker["commit"] == p.COMMIT, "private fixed-source runner")
    r, rec = recorder(p, root)
    (rec.output / "outer").mkdir()
    r.write(rec.output / "outer/receipt.json", {"pid": os.getpgrp(), "controller_pid": os.getpid(),
                                                "purpose": "remote timeout process group"})
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    for number in r.MANAGED:
        signal.signal(number, r.interrupted)
    failures, cases = [], []

    def identities():
        p.need(p.sha(root / "binding.json") == marker["binding_sha256"], "binding continuity")
        p.need(binding["commit"] == p.COMMIT and binding["order"] == [list(row) for row in p.CASE_NAMES]
               and set(binding["payload"]) == PAYLOAD and type(binding["outer_seconds"]) is int
               and binding["outer_seconds"] == REMOTE_SECONDS, "fixed matrix binding")
        p.need(binding["payload"]["base.py"] == p.BASE_SHA and binding["payload"]["recorder.py"] == p.RECORDER_SHA
               and binding["payload"]["observer.py"] == p.OBSERVER_SHA and binding["payload"]["topology.py"] == TOPOLOGY_SHA,
               "pinned helper closure")
        for name, digest in binding["payload"].items():
            p.need(p.sha(root / name) == digest, "payload continuity: " + name)

    try:
        p.need({path.name for path in root.iterdir()} == {"owner.json", "binding.json", "results", *PAYLOAD}, "initial payload closure")
        identities()
        (root / "tmp").mkdir()
        (rec.output / "artifacts").mkdir()
        for name in PAYLOAD:
            shutil.copy2(root / name, rec.output / "artifacts" / name)
        observer = p.load_module(root / "observer.py", p.OBSERVER_SHA, "native_matrix_observer")
        rec.environment = p.environment(root)
        row, stdout, stderr = rec.run("topology", ["/usr/bin/python3", "-I", "-B", str(root / "topology.py"), "topology",
                                                "--gpu-index", str(p.GPU), "--pci-bdf", p.BDF, "--unique-id", p.UID], 100)
        p.need(r.passed(row) and stderr.read_bytes() == b"", "topology command")
        topology(stdout.read_text(), p)
        row, stdout, stderr = rec.run("placement", ["/usr/bin/numactl", "--physcpubind=0-47", "--membind=0",
                                                 "/usr/bin/numactl", "--show"], 30)
        p.need(r.passed(row) and stderr.read_bytes() == b"", "placement command")
        placement(stdout.read_text(), p)
        run_cases(p, r, rec, root, observer, identities, cases)
    except BaseException as error:
        failures.append(repr(error))
    finally:
        if r.ACTIVE is not None:
            try:
                r.stop_owned(r.ACTIVE)
            except BaseException as error:
                failures.append("active cleanup: " + repr(error))
        try:
            identities()
            p.need(not rec.spawn_failures, "all process groups recorded")
            p.need(b.inventory(rec.output / "artifacts") == binding["payload"], "retained exact payload including ELF")
        except BaseException as error:
            failures.append("closing identity: " + repr(error))
        r.write(rec.output / "finished.json", {"commit": p.COMMIT, "cases": cases, "failures": failures,
                                              "spawn_failures": rec.spawn_failures,
                                              "complete_matrix": not failures and len(cases) == EXPECTED_CASES,
                                              "exclusive_reservation": False, "performance_acceptance": False,
                                              "formal_refinement": False})
    p.need(not failures and len(cases) == EXPECTED_CASES, "native matrix did not qualify: " + repr(failures))


if __name__ == "__main__":
    main()
