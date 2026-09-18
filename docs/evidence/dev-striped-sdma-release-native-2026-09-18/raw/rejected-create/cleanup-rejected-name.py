#!/usr/bin/env python3
"""Remove only the marker-only directory whose generated name failed admission."""

import json
from pathlib import Path
import shlex
import sys

sys.path.insert(0, "/home/harsh/.codex-tmp/fe2o3-striped-sdma-control-v2-20260918")
import controller as C

OWNED = "/tmp/fe2o3-striped-sdma-20260918.w_ypnqtg"
REMOTE = b"""
import json, os
from pathlib import Path
os.chdir('/tmp')
p=Path('/tmp/fe2o3-striped-sdma-20260918.w_ypnqtg')
assert p.is_dir() and not p.is_symlink() and p.resolve()==p
assert p.stat().st_uid==os.getuid() and p.stat().st_mode & 0o777==0o700
assert {x.name for x in p.iterdir()}=={'owner.json'}
owner=p/'owner.json'
assert owner.is_file() and not owner.is_symlink() and owner.stat().st_uid==os.getuid()
marker=json.loads(owner.read_text())
assert marker=={'commit':'602fda830307f9818cbff5d57d4be68a897e75b7','path':str(p),'payload_sha256':'503683e379673cebf6711e99f82dcccf9a39f1c4c8d3b787c6f940a375c164c2'}
owner.unlink()
p.rmdir()
assert not os.path.lexists(p)
print(json.dumps({'removed_marker_only':marker,'runtime_uploaded':False,'runtime_executed':False}))
"""
recorder = C.Recorder(Path(__file__).resolve().parent / "cleanup")
row, _ = recorder.run("exact-marker-only-cleanup", ["ssh", "-T", *C.SSH_OPTIONS, "mi300x", "/usr/bin/python3 -B -"], 45, REMOTE)
C.need(C.passed(row), "marker-only cleanup completed")
code = "import os,json; p=" + repr(OWNED) + "; assert not os.path.lexists(p); print(json.dumps({'owned':p,'absent':True}))"
row, _ = recorder.run("independent-path-absence", ["ssh", "-T", *C.SSH_OPTIONS, "mi300x", shlex.join(["/usr/bin/python3", "-B", "-c", code])], 45)
C.need(C.passed(row), "separate path absence completed")
