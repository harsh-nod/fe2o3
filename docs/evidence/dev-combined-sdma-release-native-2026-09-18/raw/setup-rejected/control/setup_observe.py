#!/usr/bin/env python3
"""Record read-only absence and filesystem status after rejected setup."""

from pathlib import Path
import shlex

import controller as C


def main():
    recorder = C.Recorder(Path("/home/harsh/.codex-tmp/fe2o3-combined-sdma-setup-observation-20260918"))
    code = "import os,json; p='/tmp/fe2o3-combined-sdma-20260918.47b2755b'; assert not os.path.lexists(p); print(json.dumps({'owned':p,'absent':True,'runtime_uploaded':False,'runtime_executed':False},sort_keys=True))"
    row, _ = recorder.run(
        "failed-path-absence",
        ["ssh", "-T", *C.SSH_OPTIONS, "mi300x", shlex.join(["/usr/bin/python3", "-B", "-c", code])],
        45,
    )
    C.need(C.passed(row), "failed setup path absent")
    for name, option in (("filesystem-bytes", "-h"), ("filesystem-inodes", "-i")):
        row, _ = recorder.run(
            name,
            ["ssh", "-T", *C.SSH_OPTIONS, "mi300x", shlex.join(["/usr/bin/df", option, "/tmp", "/home/harsh", "/dev/shm"])],
            45,
        )
        C.need(C.passed(row), "complete read-only filesystem observation")


if __name__ == "__main__":
    main()
