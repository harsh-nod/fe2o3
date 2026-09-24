#!/usr/bin/env python3
"""Export the qualified harness, run the frozen matrix, collect before cleanup."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import argparse
import hashlib
import json
from pathlib import Path
import secrets
import shlex
import shutil
import signal
from types import ModuleType

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
SSH = ["-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "-o", "ServerAliveInterval=10", "-o", "ServerAliveCountMax=3"]
PROTOCOL = ("prepare.py", "protocol.py", "native.py", "campaign.py", "test_protocol.py", "PROTOCOL.md")
RECORDER = "docs/evidence/dev-primary-envelope-late-selection-native-2026-09-18/raw/native/collected/run.py"
BOOTSTRAP = '''import hashlib, json, sys
from pathlib import Path
marker = json.loads(sys.argv[1])
root = Path(marker["path"])
def raw(name):
    path = root / name
    if path.is_symlink() or not path.is_file():
        raise RuntimeError("ordinary bootstrap input")
    return path.read_bytes()
binding_bytes = raw("binding.json")
if hashlib.sha256(binding_bytes).hexdigest() != marker["binding_sha256"]:
    raise RuntimeError("bootstrap binding")
binding = json.loads(binding_bytes)
native_bytes = raw("native.py")
if hashlib.sha256(native_bytes).hexdigest() != binding["payload"]["native.py"]:
    raise RuntimeError("bootstrap native identity")
sys.argv = [str(root / "native.py"), "run", sys.argv[1]]
exec(compile(native_bytes, sys.argv[0], "exec"), {"__name__": "__main__", "__file__": sys.argv[0]})
'''


def module(path, name):
    raw = path.read_bytes()
    value = ModuleType(name)
    value.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), value.__dict__)
    return value


def control_bytes(p, base):
    raw = base.read_bytes()
    p.need(hashlib.sha256(raw).hexdigest() == p.BASE_SHA, "exact captured control helper")
    return ("import hashlib\n" + f"raw = {raw!r}\n"
            + f"assert hashlib.sha256(raw).hexdigest() == {p.BASE_SHA!r}\n"
            + "scope = {'__name__': 'owned_native_matrix_control'}\n"
            + "exec(compile(raw, 'owned_native_matrix_control', 'exec'), scope)\n"
            + f"scope['PREFIX'] = {p.PREFIX!r}\n"
            + "scope['main']()\n").encode("ascii")


def validate_build(build, m, p, b):
    p.need(build.is_dir() and not build.is_symlink(), "ordinary build root")
    value = p.parse((build / "build.json").read_bytes())
    p.need(value["commit"] == p.COMMIT and value["cpu_source_inputs"] == 3956, "fixed qualified build")
    p.need(p.parse((build / "source.json").read_bytes()) == p.parse((build / "source-after.json").read_bytes())
           == value["source_files"] == b.inventory(build / "source"), "closed unchanged signed source")
    p.need(p.parse((build / "runner-before.json").read_bytes()) == p.parse((build / "runner-after.json").read_bytes())
           == {"sha256": p.sha(build / "prepare.py")} == {"sha256": p.sha(HERE / "prepare.py")}, "unchanged build runner")
    names = ["signature", "source-tree", "source-archive", "cpu-source", "cpu-roster", "rustc", "cargo",
             "build", "runtime-cpu", "after-rustc", "after-cargo"]
    p.need({path.name for path in (build / "commands").iterdir()} == set(names), "complete build commands")
    for name in names:
        folder = build / "commands" / name
        row = p.parse((folder / "receipt.json").read_bytes())
        p.need(row["exit"] == 0 and row["error"] is None and row["group_absent"] is True, "successful closed build command")
        for stream in ("stdout", "stderr"):
            p.need(p.sha(folder / stream) == row[stream + "_sha256"], "unchanged build output")
    target = Path(value["environment"]["CARGO_TARGET_DIR"])
    executable = m.executable((build / "commands/build/stdout").read_bytes(), target)
    p.need(p.sha(executable) == p.sha(build / "runtime-tests") == value["binary_sha256"], "qualified retained ELF")
    p.need(m.roster((build / "commands/runtime-cpu/stdout").read_text()) == value["runtime_roster"]
           == m.roster((build / "commands/cpu-roster/stdout").read_text()), "actual ELF exact CPU test roster")
    p.need({name for name, outcome in value["runtime_roster"].items() if outcome == "ignored"} == set(p.TESTS.values()),
           "all and only current ignored runtime tests")
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    p, m = module(HERE / "protocol.py", "native_matrix_protocol"), module(HERE / "prepare.py", "native_matrix_prepare")
    b = p.load_module(REPO / m.HELPER, p.BASE_SHA, "native_matrix_local_recorder")
    n = module(HERE / "native.py", "native_matrix_runner")
    for number in b.MANAGED:
        signal.signal(number, b.interrupted)
    build, output = args.build, args.output
    p.need(build.resolve() == build and output.resolve() == output and output.parent == build.parent and output.name == "campaign",
           "fresh sibling campaign path")
    output.mkdir()
    rec = b.Recorder(output / "commands", REPO)
    initial = {name: p.sha(HERE / name) for name in PROTOCOL}
    b.write_json(output / "protocol-before.json", initial)
    (output / "protocol").mkdir()
    for name in PROTOCOL:
        shutil.copy2(HERE / name, output / "protocol" / name)
    qualification = validate_build(build, m, p, b)
    rec.run("protocol-tests", ["/usr/bin/python3", "-I", "-B", str(HERE / "test_protocol.py")], 120, env=m.GIT_ENV)
    payload = output / "payload"
    payload.mkdir()
    sources = {"native.py": HERE / "native.py", "protocol.py": HERE / "protocol.py", "base.py": REPO / m.HELPER,
               "recorder.py": REPO / RECORDER, "observer.py": build / "source/benchmarks/runtime_gfx942/copy-host-observe.py",
               "topology.py": build / "source/benchmarks/runtime_gfx942/r26-host-guard.py", "runtime-tests": build / "runtime-tests"}
    for name, path in sources.items():
        shutil.copy2(path, payload / name)
    p.need(set(sources) == n.PAYLOAD and p.sha(payload / "base.py") == p.BASE_SHA
           and p.sha(payload / "recorder.py") == p.RECORDER_SHA and p.sha(payload / "observer.py") == p.OBSERVER_SHA
           and p.sha(payload / "topology.py") == n.TOPOLOGY_SHA, "exact complete native payload")
    binding = {"commit": p.COMMIT, "payload": b.inventory(payload), "order": p.CASE_NAMES, "outer_seconds": n.REMOTE_SECONDS,
               "protocol": initial, "build_sha256": p.sha(build / "build.json"), "source_files": qualification["source_files"],
               "device": [p.GPU, p.BDF, p.UID], "target": "x86_64-unknown-linux-musl", "features": "all-features",
               "build_environment": qualification["environment"]}
    b.write_json(output / "binding.json", binding)
    marker = {"path": p.PREFIX + secrets.token_hex(8), "commit": p.COMMIT, "binding_sha256": p.sha(output / "binding.json")}
    b.write_json(output / "owner.json", marker)
    serialized = json.dumps(marker, sort_keys=True, separators=(",", ":"))
    wire = control_bytes(p, payload / "base.py")
    (output / "control.py").write_bytes(wire)

    def control(name):
        return rec.run(name, ["ssh", "-T", *SSH, "mi300x", shlex.join(["/usr/bin/python3", "-I", "-B", "-", name, serialized])],
                       120, stdin=wire, env=m.GIT_ENV)

    attempted, collected, cleaned, failures = False, False, False, []
    try:
        p.need({name: p.sha(HERE / name) for name in PROTOCOL} == initial, "unchanged prelaunch protocol")
        control("create")
        rec.run("upload", ["scp", "-q", *SSH, "--", *(str(payload / name) for name in sorted(sources)),
                           str(output / "binding.json"), "mi300x:" + marker["path"] + "/"], 120, env=m.GIT_ENV)
        attempted = True
        remote = ["/usr/bin/timeout", "--signal=TERM", "--kill-after=15s", str(n.REMOTE_SECONDS) + "s",
                  "/usr/bin/python3", "-I", "-B", "-c", BOOTSTRAP, serialized]
        rec.run("native", ["ssh", "-T", *SSH, "mi300x", shlex.join(remote)], n.REMOTE_SECONDS + 120, env=m.GIT_ENV)
    except BaseException as error:
        failures.append(repr(error))
    finally:
        try:
            folder = control("inventory")
            expected = p.parse((folder / "stdout").read_bytes())
            rec.run("collect", ["scp", "-q", "-r", *SSH, "--", "mi300x:" + marker["path"] + "/results", str(output / "remote")],
                    180, env=m.GIT_ENV)
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
        else:
            failures.append("remote receipts retained; controller may still be live; recover exact owned path")
        try:
            p.need(b.inventory(payload) == binding["payload"], "retained payload continuity")
            validate_build(build, m, p, b)
            final = {name: p.sha(HERE / name) for name in PROTOCOL}
            b.write_json(output / "protocol-after.json", final)
            p.need(final == initial, "protocol continuity")
        except BaseException as error:
            failures.append("local identity: " + repr(error))
        b.write_json(output / "collection.json", {"failures": failures, "owned_cleanup": cleaned, "collected": collected,
                                                  "exclusive_reservation": False, "performance_acceptance": False})
    p.need(not failures and cleaned, "campaign did not qualify: " + repr(failures))


if __name__ == "__main__":
    main()
