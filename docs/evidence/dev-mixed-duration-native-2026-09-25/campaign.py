#!/usr/bin/env python3
"""Transfer the CPU-qualified ELF, run bounded native cases, collect and clean."""
import gzip
import hashlib
import json
from pathlib import Path
import runpy
import secrets
import shlex
import shutil
import signal
import sys
from types import ModuleType

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
PREVIOUS = HERE.parent / "dev-native-producer-mi300x-2026-09-24"
CPU = Path("/home/harsh/.codex-tmp/fe2o3-mixed-duration-20260925-Fi4yH01q/signed-cpu-v3")
SSH = ["-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "-o", "ServerAliveInterval=10", "-o", "ServerAliveCountMax=3"]
ENV = {"HOME": "/home/harsh", "PATH": "/usr/bin:/bin", "LC_ALL": "C"}


def load(path, name, digest=None):
    raw = path.read_bytes()
    if digest is not None and hashlib.sha256(raw).hexdigest() != digest:
        raise ValueError("pinned adapter: " + str(path))
    module = ModuleType(name)
    module.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), module.__dict__)
    return module


def main():
    if not sys.flags.isolated or not sys.flags.dont_write_bytecode or sys.flags.optimize:
        raise RuntimeError("use python3 -I -B")
    qualification = runpy.run_path(str(HERE.parent / "dev-mixed-duration-2026-09-25/verify.py"))["verify"](CPU)
    output = HERE / "raw"
    output.mkdir()
    payload = output / "payload"
    payload.mkdir()
    sources = {name: HERE / name for name in ("native.py", "protocol.py")}
    sources.update({"native-base.py": PREVIOUS / "native.py", "protocol-base.py": PREVIOUS / "protocol.py",
                    **{name: PREVIOUS / "raw/campaign1/payload" / name
                       for name in ("base.py", "recorder.py", "observer.py", "topology.py")},
                    "oracle.py": REPO / "crates/fe2o3-runtime/fixtures/trusted-gfx942-mixed-duration-v1/oracle.py"})
    for name, path in sources.items():
        shutil.copy2(path, payload / name)
    elf = json.loads((CPU / "musl-runtime-tests.json").read_text())
    with gzip.open(CPU / "musl-runtime-tests.gz", "rb") as source:
        raw = source.read(elf["bytes"] + 1)
    if len(raw) != elf["bytes"] or hashlib.sha256(raw).hexdigest() != elf["sha256"]:
        raise ValueError("CPU-qualified retained ELF")
    (payload / "runtime-tests").write_bytes(raw)
    (payload / "runtime-tests").chmod(0o700)
    del raw
    p = load(payload / "protocol.py", "mixed_duration_protocol")
    n = load(payload / "native.py", "mixed_duration_native").runner
    b = p.load_module(payload / "base.py", p.BASE_SHA, "mixed_duration_control")
    c = load(PREVIOUS / "campaign.py", "previous_campaign",
             "d4afb6869125903f2f7e654db943b67d95266700d0d763ee67ddaa93b15a71d3")
    for number in b.MANAGED:
        signal.signal(number, b.interrupted)
    rec = b.Recorder(output / "commands", REPO)
    rec.run("protocol-tests", ["/usr/bin/python3", "-I", "-B", str(HERE / "test_protocol.py"), str(payload)], 120, env=ENV)
    p.need(qualification["commit"] == p.COMMIT and p.sha(payload / "runtime-tests") == elf["sha256"], "signed tested source/ELF")
    binding = {"commit": p.COMMIT, "payload": b.inventory(payload), "order": p.CASE_NAMES,
               "outer_seconds": n.REMOTE_SECONDS, "cpu_qualification": qualification, "cpu_elf": elf,
               "device": [p.GPU, p.BDF, p.UID], "purpose": "correctness-and-observed-order-not-performance"}
    b.write_json(output / "binding.json", binding)
    marker = {"path": p.PREFIX + secrets.token_hex(8), "commit": p.COMMIT, "binding_sha256": p.sha(output / "binding.json")}
    b.write_json(output / "owner.json", marker)
    serialized = json.dumps(marker, sort_keys=True, separators=(",", ":"))
    wire = c.control_bytes(p, payload / "base.py")
    (output / "control.py").write_bytes(wire)

    def control(name):
        return rec.run(name, ["ssh", "-T", *SSH, "mi300x", shlex.join(["/usr/bin/python3", "-I", "-B", "-", name, serialized])],
                       120, stdin=wire, env=ENV)

    created, attempted, collected, cleaned, failures = False, False, False, False, []
    try:
        control("create")
        created = True
        rec.run("upload", ["scp", "-q", *SSH, "--", *(str(path) for path in sorted(payload.iterdir())),
                           str(output / "binding.json"), "mi300x:" + marker["path"] + "/"], 180, env=ENV)
        attempted = True
        remote = ["/usr/bin/timeout", "--signal=TERM", "--kill-after=15s", str(n.REMOTE_SECONDS) + "s",
                  "/usr/bin/python3", "-I", "-B", "-c", c.BOOTSTRAP, serialized]
        rec.run("native", ["ssh", "-T", *SSH, "mi300x", shlex.join(remote)], n.REMOTE_SECONDS + 120, env=ENV)
    except BaseException as error:
        failures.append(repr(error))
    finally:
        if created:
            try:
                folder = control("inventory")
                expected = p.parse((folder / "stdout").read_bytes())
                rec.run("collect", ["scp", "-q", "-r", *SSH, "--", "mi300x:" + marker["path"] + "/results", str(output / "remote")],
                        180, env=ENV)
                p.need(b.inventory(output / "remote") == expected, "byte-exact complete remote collection")
                b.write_json(output / "remote-inventory.json", expected)
                collected = True
            except BaseException as error:
                failures.append("collection: " + repr(error))
            if collected or not attempted:
                try:
                    control("cleanup")
                    control("absence")
                    cleaned = True
                except BaseException as error:
                    failures.append("cleanup: " + repr(error))
        p.need(b.inventory(payload) == binding["payload"], "retained payload unchanged")
        p.need(runpy.run_path(str(HERE.parent / "dev-mixed-duration-2026-09-25/verify.py"))["verify"](CPU) == qualification,
               "CPU evidence unchanged")
        b.write_json(output / "collection.json", {"failures": failures, "created": created, "attempted": attempted,
                                                  "owned_cleanup": cleaned, "collected": collected,
                                                  "exclusive_reservation": False, "performance_acceptance": False})
    p.need(not failures and cleaned and collected, "campaign not qualified: " + repr(failures))


if __name__ == "__main__":
    main()
