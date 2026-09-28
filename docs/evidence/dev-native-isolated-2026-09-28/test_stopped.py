#!/usr/bin/env python3
"""Calibrate stopped-run replay against a private copy of the actual failed records."""
import argparse
import hashlib
import json
from pathlib import Path
import runpy
import shutil
import tempfile
from types import SimpleNamespace

HERE = Path(__file__).resolve().parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("raw", type=Path)
    args = parser.parse_args()
    v = SimpleNamespace(**runpy.run_path(str(HERE / "verify_stopped.py")))
    s = SimpleNamespace(**runpy.run_path(str(HERE / "stage.py")))
    path, digest = s.PINS["base.py"]
    b = s.module(path, digest)
    with tempfile.TemporaryDirectory(prefix="fe2o3-stopped-calibration-") as temp:
        root = Path(temp)
        for name in ("source.tar", "source.commit", "source.sha256", "build-remote.status", "build-inventory.json"):
            shutil.copy2(args.raw / name, root / name)
        for name in ("build-results", "campaign1"):
            shutil.copytree(args.raw / name, root / name)
        campaign = root / "campaign1"
        assert v.verify(campaign)["qualified"] is False
        rejected = 0

        def alter(relative, change, stream=False):
            nonlocal rejected
            path = campaign / relative
            original = path.read_bytes()
            inventory = (campaign / "remote-inventory.json").read_bytes()
            receipt_path = path.parent / ("record.json" if relative.startswith("remote/") else "receipt.json")
            receipt = receipt_path.read_bytes() if stream else None
            try:
                if stream:
                    path.write_bytes(change(original))
                    row = json.loads(receipt)
                    key = "stdout_sha256" if path.name.startswith("stdout") else "stderr_sha256"
                    row[key] = hashlib.sha256(path.read_bytes()).hexdigest()
                    receipt_path.write_text(json.dumps(row))
                else:
                    row = json.loads(original)
                    change(row)
                    path.write_text(json.dumps(row))
                (campaign / "remote-inventory.json").write_text(json.dumps(b.inventory(campaign / "remote")))
                try:
                    v.verify(campaign)
                except ValueError:
                    rejected += 1
                else:
                    raise AssertionError("altered stopped record accepted: " + relative)
            finally:
                path.write_bytes(original)
                if receipt is not None:
                    receipt_path.write_bytes(receipt)
                (campaign / "remote-inventory.json").write_bytes(inventory)

        alter("remote/finished.json", lambda row: row.__setitem__("complete_matrix", True))
        alter("remote/07-depth-test/record.json", lambda row: row.__setitem__("status", 0))
        alter("remote/01-profiles-short-test/stdout.log", lambda raw: raw.replace(b"observed_hex=eda5", b"observed_hex=eda4"), True)
        alter("remote/07-depth-delayed/record.json", lambda row: row.__setitem__("group_absent", False))
        alter("commands/absence/stdout", lambda raw: raw.replace(b"true", b"1", 1), True)
        alter("remote/07-depth-test/stderr.log", lambda raw: raw.replace(b":261:75:", b":262:75:"), True)
        alter("remote/06-backpressure-test/record.json", lambda row: row.__setitem__("status", 101))
        assert rejected == 7 and v.verify(campaign)["qualified"] is False
        print(json.dumps({"positive_failure_replays": 2, "resealed_negative_controls": rejected,
                          "native_qualification": False}, sort_keys=True))


if __name__ == "__main__":
    main()
