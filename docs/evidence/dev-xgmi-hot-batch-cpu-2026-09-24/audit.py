#!/usr/bin/env python3
"""Record bounded final-packet replay and hostile-record tests."""

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import sys


ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[2]


def inputs():
    return {name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest() for name in (
        "runner.py", "collect.py", "verify.py", "test_verify.py", "audit.py", "artifacts.json", "README.md",
    )}


def main():
    os.chdir(REPO)
    output = ROOT / "validation"
    output.mkdir()
    before = inputs()
    (output / "inputs-before.json").write_text(json.dumps(before, indent=2) + "\n")
    spec = importlib.util.spec_from_file_location(
        "owner_check", REPO / "crates/fe2o3-runtime-model/verus/check-owner-lifecycle.py"
    )
    check = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = check
    spec.loader.exec_module(check)
    base = check.dependencies(REPO)["base"]
    for number in base.SIGNALS:
        signal.signal(number, base.interrupted)
    environment = {"HOME": "/home/harsh", "PATH": "/usr/bin:/bin", "LC_ALL": "C"}
    try:
        for name, script in (("replay", "verify.py"), ("mutations", "test_verify.py")):
            status, stdout, stderr = base.run_owned(
                ["/usr/bin/python3", "-I", "-B", str(ROOT / script)], 300, output / name, environment
            )
            print(stdout, end="")
            print(stderr, end="", file=sys.stderr)
            check.need(status == 0, "failed packet audit: " + name)
    finally:
        after = inputs()
        (output / "inputs-after.json").write_text(json.dumps(after, indent=2) + "\n")
        check.need(before == after, "packet audit inputs changed")


if __name__ == "__main__":
    main()
