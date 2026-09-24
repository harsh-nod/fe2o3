#!/usr/bin/env python3
"""Capture source-bracketed CPU qualification before any hot-batch native trial."""

import argparse
import hashlib
import importlib.util
from pathlib import Path
import shutil
import signal

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("hot_batch_qualification", HERE / "campaign.py")
C = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(C)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir()
    rec = C.B.Recorder(args.output / "commands", C.REPO)
    for number in C.B.MANAGED:
        signal.signal(number, C.B.interrupted)
    def inputs():
        return {name: hashlib.sha256((HERE / name).read_bytes()).hexdigest() for name in C.PROTOCOL}
    before = inputs()
    C.B.write_json(args.output / "inputs-before.json", before)
    (args.output / "protocol").mkdir()
    for name in C.PROTOCOL:
        shutil.copy2(HERE / name, args.output / "protocol" / name)
    environment = {"HOME": "/home/harsh", "PATH": "/usr/bin:/bin", "LC_ALL": "C"}
    try:
        scripts = [HERE / "test_campaign.py",
                   C.REPO / "benchmarks/runtime_gfx942/test_xgmi_backing_budget_campaign.py",
                   C.REPO / "benchmarks/runtime_gfx942/test_xgmi_peer_hot_results.py"]
        for script in scripts:
            rec.run(script.stem, ["/usr/bin/python3", "-I", "-B", str(script)], 180, env=environment)
    finally:
        after = inputs()
        C.B.write_json(args.output / "inputs-after.json", after)
        C.H.need(before == after, "unchanged qualification inputs")
        C.H.need(C.B.inventory(args.output / "protocol") == before, "exact frozen qualification scripts")


if __name__ == "__main__":
    main()
