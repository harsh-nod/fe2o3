#!/usr/bin/env python3
"""Recover only native1's exact owned receipts; never restart GPU workloads."""

import argparse
import importlib.util
import json
from pathlib import Path
import shlex
import signal

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("hot_batch_recovery", HERE / "campaign.py")
C = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(C)
OWNED = Path("/home/harsh/.codex-tmp/fe2o3-hot-batch-20260924-feYb12D3/native1")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    C.H.need(args.output.parent.resolve() == OWNED and args.output.name.startswith("recovery"), "owned recovery output")
    args.output.mkdir()
    rec = C.B.Recorder(args.output / "commands", C.REPO)
    for number in C.B.MANAGED:
        signal.signal(number, C.B.interrupted)
    marker = C.H.parse_json((OWNED / "owner.json").read_bytes())
    C.B.owned_path(marker, exists=False)
    C.H.need(marker["commit"] == C.SOURCE_COMMIT and marker["binding_sha256"] == C.H.sha(OWNED / "binding.json"), "original recovery binding")
    serialized = json.dumps(marker, sort_keys=True, separators=(",", ":"))
    def control(name):
        return rec.run(name, ["ssh", "-T", *C.C.SSH, "mi300x", shlex.join(["/usr/bin/python3", "-I", "-B", "-", name, serialized])],
                       120, stdin=C.C.control_bytes(C.N.PREFIX))
    collected, cleaned, failures = False, False, []
    try:
        # Inventory refuses while any process is still using this owned directory.
        folder = control("inventory")
        expected = C.H.parse_json((folder / "stdout").read_bytes())
        rec.run("collect", ["scp", "-q", "-r", *C.C.SSH, "--", "mi300x:" + marker["path"] + "/results", str(args.output / "remote")], 120)
        C.H.need(C.B.inventory(args.output / "remote") == expected, "byte-exact recovered inventory")
        C.B.write_json(args.output / "remote-inventory.json", expected)
        collected = True
    except BaseException as error:
        failures.append(repr(error))
    finally:
        cleaned, errors = C.C.settle_remote(control, collected=collected, native_attempted=True)
        failures.extend(errors)
        C.B.write_json(args.output / "result.json", {"collected": collected, "owned_cleanup": cleaned,
                       "failures": failures, "campaign_accepted": False, "performance_acceptance": False})
    C.H.need(collected and cleaned and not failures, "recovery incomplete; original owned receipts retained")


if __name__ == "__main__":
    main()
