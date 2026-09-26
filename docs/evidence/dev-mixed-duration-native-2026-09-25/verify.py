#!/usr/bin/env python3
"""Authenticate the interrupted upload packet; no native test was launched."""
import gzip
import hashlib
import json
from pathlib import Path
import runpy
import subprocess
import sys

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
RUNNER_COMMIT = "15355be3943f4f12ae4bae118f2de167874dc03f"


def need(value, message):
    if not value:
        raise ValueError(message)


def read(path):
    return json.loads(path.read_text())


def sha(path):
    need(path.is_file() and not path.is_symlink(), "ordinary artifact")
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify(cpu):
    qualification = runpy.run_path(str(HERE.parent / "dev-mixed-duration-2026-09-25/verify.py"))["verify"](cpu)
    env = dict(HOME="/home/harsh", PATH="/usr/bin:/bin", LC_ALL="C", GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL="/dev/null")
    git = ["/usr/bin/git", "--no-replace-objects"]
    subprocess.run([*git, "-c", "gpg.format=ssh", "-c", "gpg.ssh.program=/usr/bin/ssh-keygen", "-c",
                    "gpg.ssh.allowedSignersFile=" + str((cpu / "allowed-signers").resolve()), "verify-commit", RUNNER_COMMIT],
                   cwd=REPO, env=env, check=True, capture_output=True, timeout=30)
    root = HERE / "raw"
    binding, marker = read(root / "binding.json"), read(root / "owner.json")
    need(sha(root / "binding.json") == marker["binding_sha256"], "bound exact owner marker")
    need(binding["commit"] == marker["commit"] == qualification["commit"], "CPU source binding")
    need(binding["cpu_qualification"] == qualification, "complete CPU qualification join")
    elf = read(cpu / "musl-runtime-tests.json")
    need(binding["cpu_elf"] == elf, "same qualified executable")
    actual = {}
    for path in sorted((root / "payload").iterdir()):
        name = path.name
        if name == "runtime-tests.gz":
            with gzip.open(path, "rb") as source:
                raw = source.read(elf["bytes"] + 1)
            need(len(raw) == elf["bytes"] and raw.startswith(b"\x7fELF"), "bounded retained ELF")
            name = "runtime-tests"
            digest = hashlib.sha256(raw).hexdigest()
        else:
            digest = sha(path)
        need(name not in actual, "unique payload entry")
        actual[name] = digest
    need(actual == binding["payload"] and actual["runtime-tests"] == elf["sha256"], "complete staged payload")
    for name in ("native.py", "protocol.py", "campaign.py", "test_protocol.py"):
        relative = str(HERE.relative_to(REPO) / name)
        signed = subprocess.check_output([*git, "show", RUNNER_COMMIT + ":" + relative], cwd=REPO, env=env, timeout=30)
        need(sha(HERE / name) == hashlib.sha256(signed).hexdigest(), "signed unchanged campaign source: " + name)
        if name in actual:
            need(actual[name] == hashlib.sha256(signed).hexdigest(), "signed staged adapter")
    commands = {path.parent.name: read(path) for path in (root / "commands").glob("*/receipt.json")}
    need(set(commands) == {"protocol-tests", "create", "upload", "inventory", "cleanup"}, "no native launch or absence command")
    for name, row in commands.items():
        need(row["group_absent"] is True and type(row["pid"]) is int and row["pid"] > 1, "recorded local group closure")
        for stream in ("stdout", "stderr"):
            need(sha(root / "commands" / name / stream) == row[stream + "_sha256"], "unchanged command output")
    for name in ("protocol-tests", "create", "cleanup"):
        need(commands[name]["exit"] == 0 and commands[name]["error"] is None, "successful recorded command: " + name)
    need(commands["upload"]["exit"] == 1 and commands["upload"]["error"] == "RuntimeError: interrupted by signal 15",
         "interrupted upload, not a native failure")
    summary = read(root / "collection.json")
    need(summary["created"] is True and summary["attempted"] is False and summary["collected"] is False,
         "preserve original stopped campaign classification")
    need(summary["owned_cleanup"] is False and summary["performance_acceptance"] is False
         and summary["exclusive_reservation"] is False and len(summary["failures"]) == 3, "preserve original controller failures")
    need(read(root / "commands/create/stdout") == marker, "exact created marker")
    need(read(root / "commands/cleanup/stdout") == {"removed": marker["path"]}, "successful cleanup command output")
    need(not (root / "remote").exists(), "no fabricated native collection")
    return dict(cpu_commit=qualification["commit"], runner_commit=RUNNER_COMMIT, state="upload-interrupted",
                native_launched=False, native_passed=0, cleanup_command_succeeded=True,
                independent_remote_absence_verified=False, original_collection_summary_preserved=True,
                exclusive_reservation=False, performance_acceptance=False, full_parity=False)


if __name__ == "__main__":
    need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
    cpu = Path(sys.argv[1]) if len(sys.argv) == 2 else HERE.parent / "dev-mixed-duration-2026-09-25/retained/signed-cpu-v3"
    print(json.dumps(verify(cpu), sort_keys=True))
