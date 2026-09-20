#!/usr/bin/env python3
"""Offline consistency replay; trust comes from the signed archive commit."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import ast
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shlex
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
SOURCE = "a1301779d5536723cbbb5693823ce7f652d91129"
HOT = ROOT / "docs/evidence/dev-xgmi-peer-hot-mi300x-2026-09-19/native.py"
SIGNERS = ROOT / "docs/evidence/dev-combined-sdma-release-native-gpu2-2026-09-18/raw/native/collected/allowed-signers"
EXECUTION_ROOT = "/home/harsh/.codex-tmp/fe2o3-c4-completion-20260917"
PREFIX = "/home/harsh/fe2o3-ordered-peer-smoke-20260920."
BINARY = "gfx942-runtime-xgmi-segments-smoke"
OBSERVER = "copy-host-observe.py"
SSH = ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "mi300x"]
DEVICES = [[1, "0000:26:00.0", "0xab83d2ffef0d3cdf"],
           [2, "0000:46:00.0", "0xd2e26fef80cf5c33"]]
ORDER = ["source-signature", "create", "upload", "payload-before", "preflight-gpu1",
         "preflight-gpu2", "native", "settled-gpu1", "settled-gpu2", "delayed-gpu1",
         "delayed-gpu2", "payload-after", "cleanup", "absence"]


def need(value, message):
    if not value:
        raise RuntimeError(message)


def sha(path):
    need(path.is_file() and not path.is_symlink(), "regular file: " + str(path))
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


need(sha(HOT) == "820ad87e74a1f9915c2eb7d2d7c7c6c1c451da4cecf29fcc381229324d98473b", "endpoint helper")
H = load(HOT, "ordered_peer_endpoint_replay")


def read(path):
    return H.parse_json(path.read_bytes())


def inventory(folder, excluded=()):
    result = {}
    for path in sorted(folder.rglob("*")):
        need(not path.is_symlink(), "no archive symlinks")
        name = path.relative_to(folder).as_posix()
        if path.is_file() and name not in excluded:
            result[name] = sha(path)
        else:
            need(path.is_dir() or name in excluded, "ordinary archive entry")
    return result


def records(folder, report, order, stage_json):
    need(type(report["records"]) is list and [r["label"] for r in report["records"]] == order, "exact command order")
    suffixes = ["stdout", "stderr"] + (["json"] if stage_json else [])
    expected = {name + "." + suffix for name in order for suffix in suffixes}
    need(set(report["files"]) == expected, "exact receipt file roster")
    need(inventory(folder, ["result.json"]) == report["files"], "receipt inventory")
    previous = 0
    for record in report["records"]:
        need(set(record) == {"label", "command", "exit", "started_unix_ns", "finished_unix_ns"}, "receipt schema")
        need(type(record["exit"]) is int and record["exit"] == 0, "successful command")
        need(type(record["started_unix_ns"]) is int and type(record["finished_unix_ns"]) is int,
             "integer timestamps")
        need(previous <= record["started_unix_ns"] <= record["finished_unix_ns"], "receipt chronology")
        previous = record["finished_unix_ns"]
        if stage_json:
            need(H.same_json(read(folder / (record["label"] + ".json")), record), "embedded receipt equality")
        if record["label"] != "source-signature":
            need((folder / (record["label"] + ".stderr")).read_bytes() == b"", "empty command stderr")
    return {r["label"]: r for r in report["records"]}


def verify(folder=HERE, sealed=True):
    if sealed:
        need(read(folder / "inventory.json") == inventory(folder, ["inventory.json"]), "archive seal")
    need(sha(SIGNERS) == "ea67b34912b272ccbb46a4a83d8e00eb1a83e87832f089c1c134eed1786b560b", "trusted signer list")
    signature = subprocess.run(["git", "-c", "gpg.ssh.allowedSignersFile=" + str(SIGNERS),
                                "verify-commit", SOURCE], cwd=ROOT, capture_output=True, check=True)
    raw = folder / "raw"
    report = read(raw / "result.json")
    need(set(report) == {"schema", "source_commit", "controller_sha256", "target", "profile", "features",
                         "binary_sha256", "observer_sha256", "devices", "remote_owned", "owned_cleanup",
                         "performance_accepted", "exclusive_reservation", "failure", "records", "files"}, "native report schema")
    fixed = {"schema": "fe2o3.ordered-peer-smoke.v1", "source_commit": SOURCE,
             "controller_sha256": "b699a9ee5dc82fce2a9d75b3998aa747718f2a4fb8e8e1e682df0f5c1f532d2b",
             "target": "x86_64-unknown-linux-musl",
             "profile": "dev opt-level=1 debug=0 debug-assertions=true overflow-checks=true incremental=0",
             "features": "all-features", "observer_sha256": H.OBSERVER_SHA, "devices": DEVICES,
             "owned_cleanup": True, "performance_accepted": False, "exclusive_reservation": False, "failure": None}
    need(H.same_json({k: report[k] for k in fixed}, fixed), "bounded native claims")
    need(sha(folder / "controller.py") == fixed["controller_sha256"], "archived controller")
    need(re.fullmatch(r"[0-9a-f]{64}", report["binary_sha256"]) is not None, "binary identity")
    owned = report["remote_owned"]
    need(re.fullmatch(re.escape(PREFIX) + r"[A-Za-z0-9]{8}", owned) is not None, "owned directory")
    rec = records(raw, report, ORDER, True)

    def output(label):
        return (raw / (label + ".stdout")).read_bytes()

    def remote(label, command):
        need(rec[label]["command"] == SSH + [shlex.join(command)], "exact remote command: " + label)

    need(output("source-signature") == signature.stdout
         and (raw / "source-signature.stderr").read_bytes() == signature.stderr, "signature diagnostic")
    need(rec["source-signature"]["command"] == ["git", "-c",
         "gpg.ssh.allowedSignersFile=/home/harsh/.codex-tmp/fe2o3-logical-mux-sdma-protocol-v2-20260918/allowed-signers",
         "verify-commit", SOURCE], "source signature command")
    remote("create", ["/usr/bin/mktemp", "-d", PREFIX + "XXXXXXXX"])
    need(output("create") == (owned + "\n").encode(), "created owned path")
    target = "/dev/shm/fe2o3-link-parser-build-20260920.2LJm2Q0Q/target"
    need(rec["upload"]["command"] == ["scp", "-q", "-o", "BatchMode=yes", "--",
         target + "/x86_64-unknown-linux-musl/debug/examples/" + BINARY,
         EXECUTION_ROOT + "/benchmarks/runtime_gfx942/" + OBSERVER, "mi300x:" + owned + "/"], "exact upload")
    need(output("upload") == b"", "empty upload output")
    expected_hashes = (report["binary_sha256"] + "  " + owned + "/" + BINARY + "\n"
                       + H.OBSERVER_SHA + "  " + owned + "/" + OBSERVER + "\n").encode()
    for label in ["payload-before", "payload-after"]:
        remote(label, ["/usr/bin/sha256sum", "--", owned + "/" + BINARY, owned + "/" + OBSERVER])
        need(output(label) == expected_hashes, "unchanged payload")
    previous_endpoint = 0
    for phase in ["preflight", "settled", "delayed"]:
        for index, bdf, uid in DEVICES:
            label = phase + "-gpu" + str(index)
            remote(label, ["/usr/bin/python3", "-I", "-B", owned + "/" + OBSERVER,
                           "--gpu-index", str(index), "--pci-bdf", bdf, "--unique-id", uid])
            observation = H.parse_endpoint(output(label), index, bdf, uid)
            need(previous_endpoint <= H.stamp(observation["started"]), "endpoint chronology")
            previous_endpoint = H.stamp(observation["finished"])
    tree = ast.parse((folder / "controller.py").read_text())

    def constant(name):
        values = [node.value.value for node in ast.walk(tree) if isinstance(node, ast.Assign)
                  and any(isinstance(t, ast.Name) and t.id == name for t in node.targets)
                  and isinstance(node.value, ast.Constant)]
        need(len(values) == 1 and type(values[0]) is str, "controller constant: " + name)
        return values[0]

    remote("native", ["/usr/bin/python3", "-I", "-B", "-c", constant("launcher"), owned,
           "/usr/bin/env", "-u", "HIP_VISIBLE_DEVICES", "-u", "ROCR_VISIBLE_DEVICES", "-u", "CUDA_VISIBLE_DEVICES",
           "-u", "GPU_DEVICE_ORDINAL", "TMPDIR=" + owned, "/usr/bin/timeout", "--signal=TERM", "--kill-after=5s",
           "180s", owned + "/" + BINARY, DEVICES[0][2], DEVICES[1][2]])
    lines = [f"direction={direction} segments={count} poll_flush={str(poll).lower()} correctness=passed"
             for direction in range(2) for count, poll in [(1, False), (65, False), (4096, False), (4, True)]]
    need(output("native") == ("\n".join(lines + ["native_shutdown=passed"]) + "\n").encode(), "exact native success roster")
    for later, earlier, seconds in [("settled-gpu1", "native", 2), ("delayed-gpu1", "settled-gpu2", 20)]:
        need(rec[later]["started_unix_ns"] - rec[earlier]["finished_unix_ns"] >= seconds * 10**9, "postflight delay")
    remote("cleanup", ["/usr/bin/python3", "-I", "-B", "-c", constant("code"), owned, BINARY, OBSERVER])
    remote("absence", ["/usr/bin/python3", "-I", "-B", "-c",
           "import pathlib,sys; p=pathlib.Path(sys.argv[1]); assert not p.exists() and not p.is_symlink(); print('owned_absence=passed')", owned])
    need(output("cleanup") == b"owned_cleanup=passed\n" and output("absence") == b"owned_absence=passed\n", "owned cleanup")

    need(sha(folder / "qualify.py") == "e8d2d05d79917121d659db090642757dbd3c21c4b402ab4d933e86f81dcb418d", "CPU replay controller")
    Q = load(folder / "qualify.py", "ordered_peer_cpu_replay")
    cpu = read(folder / "local/result.json")
    need(set(cpu) == {"schema", "source_commit", "cwd", "environment", "records", "binary_sha256",
                      "byte_identical_post_trial_build", "clean_target_rebuild", "files"}, "CPU report schema")
    claims = {"schema": "fe2o3.ordered-peer-cpu-replay.v1", "source_commit": SOURCE, "cwd": EXECUTION_ROOT,
              "environment": Q.ENV, "binary_sha256": report["binary_sha256"],
              "byte_identical_post_trial_build": True, "clean_target_rebuild": False}
    need(H.same_json({k: cpu[k] for k in claims}, claims), "bounded CPU claims")
    order = ["source-before", "rustc", "cargo", "build", "gnu-runtime", "musl-runtime", "r74-model", "clippy", "source-after"]
    local = records(folder / "local", cpu, order, False)
    need(rec["absence"]["finished_unix_ns"] < local["source-before"]["started_unix_ns"], "post-trial replay")
    paths = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", "crates"]
    test = ["cargo", "test", "--locked", "-q", "-p", "fe2o3-runtime", "--all-features"]
    commands = {"source-before": ["git", "diff", "--exit-code", SOURCE, "--", *paths],
                "source-after": ["git", "diff", "--exit-code", SOURCE, "--", *paths],
                "rustc": ["rustc", "-vV"], "cargo": ["cargo", "-V"], "build": Q.BUILD,
                "gnu-runtime": test + ["--lib"],
                "musl-runtime": test + ["--target", "x86_64-unknown-linux-musl", "--lib", "--example", BINARY],
                "r74-model": ["cargo", "test", "--locked", "-q", "-p", "fe2o3-runtime-model", "--lib", "r74_ordered_peer_copy"],
                "clippy": ["cargo", "clippy", "--locked", "-q", "-p", "fe2o3-runtime", "--all-features", "--lib", "--tests",
                           "--example", BINARY, "--", "-D", "warnings"]}
    for label, command in commands.items():
        need(local[label]["command"] == command, "exact CPU command: " + label)
    need((folder / "local/rustc.stdout").read_text().splitlines() == [
        "rustc 1.96.0-nightly (55e86c996 2026-04-02)", "binary: rustc",
        "commit-hash: 55e86c996809902e8bbad512cfb4d2c18be446d9", "commit-date: 2026-04-02",
        "host: x86_64-unknown-linux-gnu", "release: 1.96.0-nightly", "LLVM version: 22.1.2",
    ], "pinned compiler identity")
    need((folder / "local/cargo.stdout").read_bytes() == b"cargo 1.96.0-nightly (888f67534 2026-03-30)\n",
         "pinned Cargo identity")
    for label in ["source-before", "source-after", "build", "clippy"]:
        need((folder / "local" / (label + ".stdout")).read_bytes() == b"", "empty CPU output: " + label)
    for label, expected in [("gnu-runtime", [(1190, 20, 0)]),
                            ("musl-runtime", [(1190, 20, 0), (2, 0, 0)]), ("r74-model", [(3, 0, 798)])]:
        text = (folder / "local" / (label + ".stdout")).read_text()
        found = re.findall(r"^test result: ok\. (\d+) passed; 0 failed; (\d+) ignored; 0 measured; (\d+) filtered out; finished in [0-9.]+s$", text, re.M)
        need([tuple(map(int, row)) for row in found] == expected, "exact CPU test outcomes")
    return {"native_cases": 8, "endpoint_observations": 6, "owned_cleanup": True,
            "performance_accepted": False, "source_commit": SOURCE}


if __name__ == "__main__":
    need(sys.argv[1:] in ([], ["--seal"]), "usage: verify.py [--seal]")
    if sys.argv[1:] == ["--seal"]:
        verify(sealed=False)
        need(not (HERE / "inventory.json").exists(), "archive already sealed")
        (HERE / "inventory.json").write_text(json.dumps(inventory(HERE), indent=2) + "\n", encoding="ascii")
    print(json.dumps(verify(), sort_keys=True))
