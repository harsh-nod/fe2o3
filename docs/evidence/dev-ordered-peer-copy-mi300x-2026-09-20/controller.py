#!/usr/bin/env python3
"""One bounded shared-host correctness trial; no performance acceptance."""
import sys
if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import tempfile
import time

ROOT = Path("/home/harsh/.codex-tmp/fe2o3-c4-completion-20260917")
BINARY = Path("/dev/shm/fe2o3-link-parser-build-20260920.2LJm2Q0Q/target/x86_64-unknown-linux-musl/debug/examples/gfx942-runtime-xgmi-segments-smoke")
OBSERVER = ROOT / "benchmarks/runtime_gfx942/copy-host-observe.py"
HOT = ROOT / "docs/evidence/dev-xgmi-peer-hot-mi300x-2026-09-19/native.py"
PREFIX = "/home/harsh/fe2o3-ordered-peer-smoke-20260920."
DEVICES = [(1, "0000:26:00.0", "0xab83d2ffef0d3cdf"), (2, "0000:46:00.0", "0xd2e26fef80cf5c33")]
SSH = ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "mi300x"]

def sha(path):
    if path.is_symlink() or not path.is_file():
        raise RuntimeError("regular file required: " + str(path))
    return hashlib.sha256(path.read_bytes()).hexdigest()

if sha(HOT) != "820ad87e74a1f9915c2eb7d2d7c7c6c1c451da4cecf29fcc381229324d98473b":
    raise RuntimeError("hot helper digest")
spec = importlib.util.spec_from_file_location("ordered_peer_admission", HOT)
H = importlib.util.module_from_spec(spec)
spec.loader.exec_module(H)

if subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT):
    raise RuntimeError("source must be a clean committed tree")
commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
binary_sha, observer_sha = sha(BINARY), sha(OBSERVER)
if observer_sha != H.OBSERVER_SHA:
    raise RuntimeError("observer digest")
output = Path(tempfile.mkdtemp(prefix="fe2o3-ordered-peer-smoke-results-20260920.", dir="/home/harsh/.codex-tmp"))
print("results=" + str(output), flush=True)
records = []

def run(label, command, timeout=120):
    started = time.time_ns()
    cp = subprocess.run(command, cwd=ROOT, capture_output=True, timeout=timeout)
    (output / (label + ".stdout")).write_bytes(cp.stdout)
    (output / (label + ".stderr")).write_bytes(cp.stderr)
    record = {"label": label, "command": command, "exit": cp.returncode,
              "started_unix_ns": started, "finished_unix_ns": time.time_ns()}
    records.append(record)
    (output / (label + ".json")).write_text(json.dumps(record, indent=2) + "\n", encoding="ascii")
    if cp.returncode:
        raise RuntimeError(label + " exited " + str(cp.returncode))
    return cp.stdout

def remote(label, command, timeout=120):
    return run(label, SSH + [shlex.join(command)], timeout)

def observe(label, owned):
    for index, bdf, uid in DEVICES:
        data = remote(label + "-gpu" + str(index), ["/usr/bin/python3", "-I", "-B",
            owned + "/" + OBSERVER.name, "--gpu-index", str(index), "--pci-bdf", bdf, "--unique-id", uid])
        if (output / (label + "-gpu" + str(index) + ".stderr")).read_bytes():
            raise RuntimeError("observer stderr")
        H.parse_endpoint(data, index, bdf, uid)

owned = None
failure = None
cleanup = False
try:
    run("source-signature", ["git", "-c", "gpg.ssh.allowedSignersFile=/home/harsh/.codex-tmp/fe2o3-logical-mux-sdma-protocol-v2-20260918/allowed-signers", "verify-commit", commit])
    owned = remote("create", ["/usr/bin/mktemp", "-d", PREFIX + "XXXXXXXX"]).decode("ascii").strip()
    if re.fullmatch(re.escape(PREFIX) + r"[A-Za-z0-9]{8}", owned) is None:
        raise RuntimeError("unexpected owned directory")
    run("upload", ["scp", "-q", "-o", "BatchMode=yes", "--", str(BINARY), str(OBSERVER), "mi300x:" + owned + "/"])
    expected = (binary_sha + "  " + owned + "/" + BINARY.name + "\n"
                + observer_sha + "  " + owned + "/" + OBSERVER.name + "\n").encode("ascii")
    def identity(label):
        actual = remote(label, ["/usr/bin/sha256sum", "--", owned + "/" + BINARY.name, owned + "/" + OBSERVER.name])
        if actual != expected:
            raise RuntimeError("remote payload changed")
    identity("payload-before")
    observe("preflight", owned)
    launcher = "import os,resource,sys; resource.setrlimit(resource.RLIMIT_CORE,(0,0)); os.chdir(sys.argv[1]); os.execvpe(sys.argv[2],sys.argv[2:],os.environ)"
    data = remote("native", ["/usr/bin/python3", "-I", "-B", "-c", launcher, owned,
        "/usr/bin/env", "-u", "HIP_VISIBLE_DEVICES", "-u", "ROCR_VISIBLE_DEVICES",
        "-u", "CUDA_VISIBLE_DEVICES", "-u", "GPU_DEVICE_ORDINAL", "TMPDIR=" + owned,
        "/usr/bin/timeout", "--signal=TERM", "--kill-after=5s", "180s",
        owned + "/" + BINARY.name, DEVICES[0][2], DEVICES[1][2]], 210)
    expected_lines = [f"direction={direction} segments={count} poll_flush={str(poll).lower()} correctness=passed"
        for direction in range(2) for count, poll in [(1, False), (65, False), (4096, False), (4, True)]]
    expected_lines.append("native_shutdown=passed")
    if data.decode("ascii").splitlines() != expected_lines or (output / "native.stderr").read_bytes():
        raise RuntimeError("native smoke did not emit exact success roster")
    time.sleep(2)
    observe("settled", owned)
    time.sleep(20)
    observe("delayed", owned)
    identity("payload-after")
    if sha(BINARY) != binary_sha or subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT):
        raise RuntimeError("local source or binary changed")
except BaseException as error:
    failure = repr(error)
finally:
    if owned is not None and re.fullmatch(re.escape(PREFIX) + r"[A-Za-z0-9]{8}", owned):
        try:
            code = """import os,pathlib,re,stat,subprocess,sys
p=pathlib.Path(sys.argv[1]); names=sys.argv[2:]
s=p.lstat()
assert stat.S_ISDIR(s.st_mode) and s.st_uid==os.geteuid() and stat.S_IMODE(s.st_mode)==0o700
assert {f.name for f in p.iterdir()} <= set(names)
processes=subprocess.check_output(['/bin/ps','-eo','args='],text=True).splitlines()
assert not any(re.match('^'+re.escape(str(p/names[0]))+r'(?: |$)',line) for line in processes)
for f in p.iterdir():
 s=f.lstat(); assert stat.S_ISREG(s.st_mode) and s.st_uid==os.geteuid()
for f in p.iterdir(): f.unlink()
p.rmdir()
assert not p.exists() and not p.is_symlink()
print('owned_cleanup=passed')
"""
            remote("cleanup", ["/usr/bin/python3", "-I", "-B", "-c", code, owned, BINARY.name, OBSERVER.name])
            remote("absence", ["/usr/bin/python3", "-I", "-B", "-c",
                "import pathlib,sys; p=pathlib.Path(sys.argv[1]); assert not p.exists() and not p.is_symlink(); print('owned_absence=passed')", owned])
            cleanup = True
        except BaseException as error:
            failure = (failure or "") + "; cleanup: " + repr(error)
    report = {"schema": "fe2o3.ordered-peer-smoke.v1", "source_commit": commit,
        "controller_sha256": sha(Path(__file__)), "target": "x86_64-unknown-linux-musl",
        "profile": "dev opt-level=1 debug=0 debug-assertions=true overflow-checks=true incremental=0",
        "features": "all-features",
        "binary_sha256": binary_sha, "observer_sha256": observer_sha,
        "devices": DEVICES, "remote_owned": owned, "owned_cleanup": cleanup,
        "performance_accepted": False, "exclusive_reservation": False,
        "failure": failure, "records": records}
    report["files"] = {p.name: sha(p) for p in sorted(output.iterdir()) if p.is_file()}
    (output / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="ascii")
    print(json.dumps({"results": str(output), "failure": failure, "owned_cleanup": cleanup}), flush=True)
if failure or not cleanup:
    raise SystemExit(1)
