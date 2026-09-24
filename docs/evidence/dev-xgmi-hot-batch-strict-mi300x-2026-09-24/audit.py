#!/usr/bin/env python3
"""Capture strict replay and hostile-record controls with unchanged inputs."""

import argparse
import importlib.util
from pathlib import Path
import signal

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("strict_hot_audit", HERE / "packet.py")
P = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(P)
V = P.configured("verify")


def inputs():
    names = (*V.C.PROTOCOL, "packet.py", "test_packet.py", "audit.py", "README.md", "artifacts.json")
    result = {name: V.sha(HERE / name) for name in names}
    for role in P.HELPERS | P.TESTS:
        path, _ = P.authenticated(role)
        result["prior/" + role + ".py"] = V.sha(path)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir()
    before = inputs()
    V.B.write_json(args.output / "inputs-before.json", before)
    recorder = V.B.Recorder(args.output / "commands", V.REPO)
    for number in V.B.MANAGED:
        signal.signal(number, V.B.interrupted)
    env = {"HOME": "/home/harsh", "PATH": "/usr/bin:/bin", "LC_ALL": "C"}
    try:
        for name, command in (
            ("verify", [str(HERE / "packet.py"), "verify"]),
            ("test_packet", [str(HERE / "test_packet.py")]),
            ("test_package", [str(P.PRIOR / "test_package.py")]),
        ):
            recorder.run(name, ["/usr/bin/python3", "-I", "-B", *command], 900, env=env)
    finally:
        after = inputs()
        V.B.write_json(args.output / "inputs-after.json", after)
        V.need(before == after, "unchanged strict packet audit inputs")


if __name__ == "__main__":
    main()
