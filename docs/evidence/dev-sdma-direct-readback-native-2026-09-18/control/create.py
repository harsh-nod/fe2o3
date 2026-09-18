#!/usr/bin/env python3
"""Create a fresh private marked directory; no payload upload or native work."""

import argparse
import json
from pathlib import Path
import re
import shlex

import controller as C
import remote_control as R

REMOTE = b"""
import json, os, sys, tempfile
from pathlib import Path
os.umask(0o077)
commit, payload = sys.argv[1:]
owned = Path(tempfile.mkdtemp(prefix="fe2o3-sdma-readback-fd1cf3dd-20260918.", dir="/tmp"))
assert owned.stat().st_uid == os.getuid() and owned.stat().st_mode & 0o777 == 0o700
marker = {"commit": commit, "path": str(owned), "payload_sha256": payload}
with (owned / "owner.json").open("x") as target:
    target.write(json.dumps(marker, indent=2) + "\\n")
print(json.dumps({"record": "fresh-owned-directory-created", **marker}, sort_keys=True))
"""


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    C.need(C.PAYLOAD == R.PAYLOAD == C.sha(C.BUNDLE / "payload.json"), "frozen payload")
    recorder = C.Recorder(args.output.resolve())
    row, folder = recorder.run(
        "create",
        [
            "ssh",
            "-T",
            *C.SSH_OPTIONS,
            "mi300x",
            shlex.join(["/usr/bin/python3", "-B", "-", R.COMMIT, R.PAYLOAD]),
        ],
        45,
        REMOTE,
    )
    C.need(C.passed(row), "creation completed")
    C.need(not (folder / "stderr.log").read_text().strip(), "no creation diagnostics")
    value = json.loads((folder / "stdout.log").read_text())
    C.need(
        value["record"] == "fresh-owned-directory-created"
        and value["commit"] == R.COMMIT
        and value["payload_sha256"] == R.PAYLOAD,
        "exact creation identity",
    )
    C.need(
        re.fullmatch(
            r"/tmp/fe2o3-sdma-readback-fd1cf3dd-20260918\.[A-Za-z0-9]{8}", value["path"]
        ),
        "fresh path shape",
    )
    print(json.dumps(value, sort_keys=True))


if __name__ == "__main__":
    main()
