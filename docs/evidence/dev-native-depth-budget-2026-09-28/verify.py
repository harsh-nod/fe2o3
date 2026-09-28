#!/usr/bin/env python3
"""Replay seven isolated native cells, endpoint windows, ownership and build/ELF joins."""
import argparse
import hashlib
import json
from pathlib import Path
import runpy
import shlex
import sys
from types import SimpleNamespace

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
ORIGINAL_REPO = Path("/home/harsh/.codex-tmp/fe2o3-r61-execution")
ORIGINAL_HERE = ORIGINAL_REPO / "docs/evidence/dev-native-depth-budget-2026-09-28"
ORIGINAL = Path("/home/harsh/.codex-tmp/fe2o3-native-depth-budget-20260928-UnAv0P2r/campaign1")
ENV = {"HOME": "/home/harsh", "PATH": "/usr/bin:/bin", "LANG": "C", "LC_ALL": "C"}


def helpers(folder):
    s = SimpleNamespace(**runpy.run_path(str(HERE / "stage.py")))
    expected = {name: hashlib.sha256(s.capture(HERE / name)).hexdigest() for name in s.FILES}
    expected.update({name: digest for name, (_, digest) in s.PINS.items()})
    if folder.is_symlink() or not folder.is_dir() or {path.name for path in folder.iterdir()} != set(expected):
        raise ValueError("complete ordinary retained helper closure")
    for name, digest in expected.items():
        s.capture(folder / name, digest)
    p = s.module(folder / "protocol.py", expected["protocol.py"])
    n = s.module(folder / "native.py", expected["native.py"]).runner
    b = s.module(folder / "base.py", p.BASE_SHA)
    c = s.module(folder / "campaign-base.py", s.PINS["campaign-base.py"][1])
    v = s.module(folder / "verify-base.py", s.PINS["verify-base.py"][1],
                 (b'p.environment(root, case), n.TEST_SECONDS)', b'p.environment(root, case), p.test_seconds(case))'))
    return s, p, n, b, c, v


def verify(campaign):
    s, p, n, b, c, v = helpers(campaign / "protocol")
    initial = v.read(campaign / "protocol-before.json")
    p.need(initial == v.read(campaign / "protocol-after.json") == b.inventory(campaign / "protocol"), "helper bracket")
    p.need(set(initial) == set(s.FILES) | set(s.PINS), "complete helper closure")
    for name, (_, digest) in s.PINS.items():
        p.need(initial[name] == digest, "inherited helper pin")
    for name in s.FILES:
        p.need(v.sha(HERE / name) == initial[name], "retained current adapter")
    build = s.module(campaign / "protocol/verify_build.py", initial["verify_build.py"])
    build.REPO = REPO
    qualified = build.verify(campaign.parent)
    p.need(qualified == v.read(campaign / "build-binding.json"), "build qualification join")
    p.need(p.FILTERED == qualified["filtered"] and set(p.TESTS.values()) <= set(qualified["ignored"]), "exact ELF roster join")
    binding, marker = v.verify_native(campaign, p, n, b, False)
    p.need(binding["build_sha256"] == v.sha(campaign / "build-binding.json")
           and binding["source_files"] == v.read(campaign.parent / "build-inventory.json")["source"]
           and binding["build_environment"] == qualified["environment"]
           and binding["payload"]["runtime-tests"] == qualified["elf_sha256"], "signed source/CPU/uploaded/retained ELF")
    p.need(binding["protocol"] == initial and binding["device"] == [p.GPU, p.BDF, p.UID]
           and binding["target"] == qualified["target"] and binding["features"] == qualified["features"], "bound build profile")
    for name in n.PAYLOAD - {"runtime-tests"}:
        p.need(binding["payload"][name] == initial[name], "complete helper payload join")
    serialized = json.dumps(marker, sort_keys=True, separators=(",", ":"))
    wire = (campaign / "control.py").read_bytes()
    p.need(wire == c.control_bytes(p, campaign / "payload/base.py"), "exact cleanup helper")
    remote = ["/usr/bin/timeout", "--signal=TERM", "--kill-after=15s", str(n.REMOTE_SECONDS) + "s",
              "/usr/bin/python3", "-I", "-B", "-c", c.BOOTSTRAP, serialized]
    def control(name):
        return ["ssh", "-T", *c.SSH, "mi300x", shlex.join(["/usr/bin/python3", "-I", "-B", "-", name, serialized])]
    specs = [("protocol-tests", ["/usr/bin/python3", "-I", "-B", str(ORIGINAL_HERE / "test_protocol.py")], 180, None),
             ("create", control("create"), 120, wire),
             ("upload", ["scp", "-q", *c.SSH, "--", *(str(ORIGINAL / "payload" / name) for name in sorted(n.PAYLOAD)),
                         str(ORIGINAL / "binding.json"), "mi300x:" + marker["path"] + "/"], 120, None),
             ("native", ["ssh", "-T", *c.SSH, "mi300x", shlex.join(remote)], n.REMOTE_SECONDS + 120, None),
             ("inventory", control("inventory"), 120, wire),
             ("collect", ["scp", "-q", "-r", *c.SSH, "--", "mi300x:" + marker["path"] + "/results",
                          str(ORIGINAL / "remote")], 180, None),
             ("cleanup", control("cleanup"), 120, wire), ("absence", control("absence"), 120, wire)]
    p.need({path.name for path in (campaign / "commands").iterdir()} == {row[0] for row in specs}, "exact local commands")
    previous = 0
    for name, argv, bound, stdin in specs:
        row = v.command(campaign / "commands" / name, argv, ORIGINAL_REPO, ENV, bound, stdin)
        p.need(previous <= row["started_ns"], "collect before cleanup before absence")
        previous = row["finished_ns"]
    p.need(v.read(campaign / "commands/create/stdout") == marker, "exact owned creation")
    p.need(v.read(campaign / "commands/inventory/stdout") == v.read(campaign / "remote-inventory.json"), "collection inventory")
    p.need(v.read(campaign / "commands/cleanup/stdout") == {"removed": marker["path"]}, "owned cleanup")
    p.need(p.same(v.read(campaign / "commands/absence/stdout"), {"path_absent": True, "processes_absent": True}), "independent absence")
    p.need(p.same(v.read(campaign / "collection.json"), {"failures": [], "owned_cleanup": True, "collected": True,
                                                     "exclusive_reservation": False, "performance_acceptance": False}), "successful collection")
    return {"harness_passes": 7, "strict_endpoints": 21, "remote_commands": 30, "cpu_passes": qualified["cpu_passes"],
            "elf_sha256": qualified["elf_sha256"], "owned_cleanup": True, "formal_refinement": False,
            "performance_acceptance": False, "exclusive_reservation": False}


if __name__ == "__main__":
    if not sys.flags.isolated or not sys.flags.dont_write_bytecode or sys.flags.optimize:
        raise ValueError("use python3 -I -B")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("campaign", type=Path)
    print(json.dumps(verify(parser.parse_args().campaign), sort_keys=True))
