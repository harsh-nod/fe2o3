#!/usr/bin/env python3
"""Run the six exact correctness cells once, collect completely, then remove owned files."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import runpy
import secrets
import shlex
import signal
import sys
from types import SimpleNamespace

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
ENV = {"HOME": "/home/harsh", "PATH": "/usr/bin:/bin", "LANG": "C", "LC_ALL": "C"}


def main():
    if not sys.flags.isolated or not sys.flags.dont_write_bytecode or sys.flags.optimize:
        raise ValueError("use python3 -I -B")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    s = SimpleNamespace(**runpy.run_path(str(HERE / "stage.py")))
    build, output = args.build, args.output
    if build.resolve() != build or output.resolve() != output or output.parent != build or output.name != "campaign1":
        raise ValueError("fresh exact campaign path")
    output.mkdir()
    initial = s.stage(output / "protocol")
    p = s.module(output / "protocol/protocol.py", initial["protocol.py"])
    n = s.module(output / "protocol/native.py", initial["native.py"]).runner
    b = s.module(output / "protocol/base.py", p.BASE_SHA)
    c = s.module(output / "protocol/campaign-base.py", s.PINS["campaign-base.py"][1])
    v = s.module(output / "protocol/verify_build.py", initial["verify_build.py"])
    v.REPO = REPO
    qualified = v.verify(build)
    p.need(set(p.TESTS.values()) <= set(qualified["ignored"]), "selected exact ignored ELF tests")
    p.need(p.FILTERED == qualified["filtered"], "current ELF filtered count")
    b.write_json(output / "protocol-before.json", initial)
    b.write_json(output / "build-binding.json", qualified)
    for number in b.MANAGED:
        signal.signal(number, b.interrupted)
    rec = b.Recorder(output / "commands", REPO)
    rec.run("protocol-tests", ["/usr/bin/python3", "-I", "-B", str(HERE / "test_protocol.py")], 180, env=ENV)
    payload = output / "payload"
    payload.mkdir()
    for name in sorted(n.PAYLOAD - {"runtime-tests"}):
        (payload / name).write_bytes(s.capture(output / "protocol" / name, initial[name]))
    with gzip.open(build / "build-results/runtime-tests.gz", "rb") as source:
        binary = source.read(v.ELF_BYTES + 1)
    p.need(len(binary) == v.ELF_BYTES and hashlib.sha256(binary).hexdigest() == v.ELF_SHA, "exact tested ELF")
    (payload / "runtime-tests").write_bytes(binary)
    (payload / "runtime-tests").chmod(0o700)
    binding = {"commit": p.COMMIT, "payload": b.inventory(payload), "order": p.CASE_NAMES,
               "outer_seconds": n.REMOTE_SECONDS, "protocol": initial,
               "build_sha256": p.sha(output / "build-binding.json"),
               "source_files": p.parse((build / "build-inventory.json").read_bytes())["source"],
               "device": [p.GPU, p.BDF, p.UID], "target": qualified["target"], "features": qualified["features"],
               "build_environment": qualified["environment"]}
    b.write_json(output / "binding.json", binding)
    marker = {"path": p.PREFIX + secrets.token_hex(8), "commit": p.COMMIT,
              "binding_sha256": p.sha(output / "binding.json")}
    b.write_json(output / "owner.json", marker)
    serialized = json.dumps(marker, sort_keys=True, separators=(",", ":"))
    wire = c.control_bytes(p, payload / "base.py")
    (output / "control.py").write_bytes(wire)

    def control(name):
        return rec.run(name, ["ssh", "-T", *c.SSH, "mi300x",
                             shlex.join(["/usr/bin/python3", "-I", "-B", "-", name, serialized])],
                       120, stdin=wire, env=ENV)

    attempted, collected, cleaned, failures = False, False, False, []
    try:
        p.need({name: p.sha(HERE / name) for name in s.FILES} == {name: initial[name] for name in s.FILES},
               "unchanged prelaunch adapters")
        control("create")
        rec.run("upload", ["scp", "-q", *c.SSH, "--", *(str(payload / name) for name in sorted(n.PAYLOAD)),
                           str(output / "binding.json"), "mi300x:" + marker["path"] + "/"], 120, env=ENV)
        attempted = True
        remote = ["/usr/bin/timeout", "--signal=TERM", "--kill-after=15s", str(n.REMOTE_SECONDS) + "s",
                  "/usr/bin/python3", "-I", "-B", "-c", c.BOOTSTRAP, serialized]
        rec.run("native", ["ssh", "-T", *c.SSH, "mi300x", shlex.join(remote)], n.REMOTE_SECONDS + 120, env=ENV)
    except BaseException as error:
        failures.append(repr(error))
    finally:
        try:
            folder = control("inventory")
            expected = p.parse((folder / "stdout").read_bytes())
            rec.run("collect", ["scp", "-q", "-r", *c.SSH, "--", "mi300x:" + marker["path"] + "/results",
                                str(output / "remote")], 180, env=ENV)
            p.need(b.inventory(output / "remote") == expected, "complete byte-exact collection")
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
            failures.append("retain exact owned remote path for recovery; controller may still be live")
        try:
            p.need(b.inventory(payload) == binding["payload"], "payload continuity")
            p.need(v.verify(build) == qualified, "build/CPU continuity")
            final = b.inventory(output / "protocol")
            b.write_json(output / "protocol-after.json", final)
            p.need(final == initial, "captured helper continuity")
            p.need({name: p.sha(HERE / name) for name in s.FILES} == {name: initial[name] for name in s.FILES},
                   "live adapter continuity")
        except BaseException as error:
            failures.append("local identity: " + repr(error))
        b.write_json(output / "collection.json", {"failures": failures, "owned_cleanup": cleaned, "collected": collected,
                                                  "exclusive_reservation": False, "performance_acceptance": False})
    p.need(not failures and cleaned, "campaign did not qualify: " + repr(failures))


if __name__ == "__main__":
    main()
