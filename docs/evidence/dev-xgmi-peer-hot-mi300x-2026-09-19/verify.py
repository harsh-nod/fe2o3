#!/usr/bin/env python3
"""Replay exact hot-copy commands, endpoint checks, results, and collection."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import argparse
import hashlib
import importlib.util
import io
import json
import re
import shlex
import subprocess
import tarfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
SIGNERS = (
    "/home/harsh/.codex-tmp/fe2o3-logical-mux-sdma-protocol-v2-20260918/allowed-signers"
)
SIGNERS_SHA = "ea67b34912b272ccbb46a4a83d8e00eb1a83e87832f089c1c134eed1786b560b"


def need(value, message):
    if not value:
        raise RuntimeError(message)


def load(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def read(path):
    return json.loads(path.read_bytes())


def verify():
    binding = read(HERE / "binding.json")
    commit = binding["commit"]
    need(re.fullmatch(r"[0-9a-f]{40}", commit), "source commit")
    need(
        not Path(SIGNERS).is_symlink()
        and hashlib.sha256(Path(SIGNERS).read_bytes()).hexdigest() == SIGNERS_SHA,
        "trusted signer list",
    )
    subprocess.run(
        ["git", "-c", "gpg.ssh.allowedSignersFile=" + SIGNERS, "verify-commit", commit],
        cwd=ROOT,
        check=True,
        capture_output=True,
    )
    static = (
        "campaign.py",
        "native.py",
        "results.py",
        "test_results.py",
        "test_campaign.py",
        "test_native.py",
        "verify.py",
    )
    for name in static:
        path = HERE / name
        need(
            not path.is_symlink()
            and subprocess.check_output(
                ["git", "show", commit + ":" + path.relative_to(ROOT).as_posix()],
                cwd=ROOT,
            )
            == path.read_bytes(),
            "tooling equals signed commit: " + name,
        )
    C = load(HERE / "campaign.py", "verify_hot_campaign")
    N, B = C.N, C.B
    R = load(HERE / "results.py", "verify_hot_results")
    need(
        C.SIGNERS == SIGNERS and C.SIGNERS_SHA == SIGNERS_SHA, "shared signer contract"
    )
    cpu_prefix = C.CPU.relative_to(ROOT).as_posix() + "/"
    cpu_tar = subprocess.check_output(
        ["git", "archive", "--format=tar", commit, "--", cpu_prefix], cwd=ROOT
    )
    with tarfile.open(fileobj=io.BytesIO(cpu_tar), mode="r:") as archive:
        committed_cpu = {
            member.name.removeprefix(cpu_prefix): hashlib.sha256(
                archive.extractfile(member).read()
            ).hexdigest()
            for member in archive.getmembers()
            if member.isfile()
        }
    need(
        committed_cpu and B.inventory(C.CPU) == committed_cpu,
        "authenticate entire CPU archive before import",
    )
    Q = load(C.CPU / "cpu.py", "verify_hot_cpu")
    Q.verify()
    cpu = read(C.CPU / "binding.json")
    need(
        set(binding) == {"commit", "source_files", "payload", "plan"}
        and binding["source_files"] == cpu["source_files"]
        and binding["plan"] == N.PLAN,
        "CPU-bound source and exact plan",
    )
    need(set(binding["payload"]) == set(N.PAYLOAD), "payload roster")
    for name in ("native.py", "results.py"):
        need(
            binding["payload"][name] == C.sha(HERE / name),
            "transported tooling: " + name,
        )
    need(
        binding["payload"]["base.py"] == C.BASE_SHA
        and re.fullmatch(r"[0-9a-f]{64}", binding["payload"]["source.tar.gz"]),
        "transported helpers and source archive",
    )
    signed_tar = subprocess.check_output(
        ["git", "archive", "--format=tar", commit, "--", *C.SOURCE_PATHS], cwd=ROOT
    )
    with tarfile.open(fileobj=io.BytesIO(signed_tar), mode="r:") as archive:
        signed_files = {
            member.name: hashlib.sha256(archive.extractfile(member).read()).hexdigest()
            for member in archive.getmembers()
            if member.isfile()
        }
    need(signed_files == binding["source_files"], "complete source equals signed tree")

    marker = read(HERE / "owner.json")
    owned = B.owned_path(marker, exists=False)
    need(
        marker["commit"] == commit
        and marker["binding_sha256"] == C.sha(HERE / "binding.json"),
        "ownership binds source and payload",
    )
    collection = read(HERE / "collection.json")
    need(
        collection
        == {
            "source_commit": commit,
            "cpu_seal_sha256": C.sha(C.CPU / "SHA256SUMS"),
            "native_execution": True,
            "performance_acceptance": False,
            "formal_refinement": False,
            "exclusive_reservation": False,
            "remote_removed": True,
            "local_payload_removed": True,
        },
        "bounded collection claims",
    )
    need(
        B.inventory(HERE / "remote") == read(HERE / "remote-inventory.json"),
        "complete remote byte collection",
    )
    need(
        read(HERE / "raw/remote-inventory/stdout")
        == read(HERE / "remote-inventory.json"),
        "inventory is the recorded remote inventory",
    )

    local = {}
    for folder in sorted((HERE / "raw").iterdir()):
        need(folder.is_dir(), "local receipt folder")
        local[folder.name] = C.verify_receipt(folder)
    order = [
        "signature",
        "cpu",
        "native-tests",
        "campaign-tests",
        "parser-tests",
        "ref-origin",
        "ref-upstream",
        "pack",
        "create",
        "upload",
        "native",
        "remote-inventory",
        "collect",
        "cleanup",
        "absence",
    ]
    need(set(local) == set(order), "exact local command roster")
    previous = 0
    execution_root = Path(local["signature"]["cwd"])
    payload = Path(read(HERE / "local-payload.json")["path"])
    need(
        payload.parent == execution_root.parent
        and re.fullmatch(r"fe2o3-peer-hot-payload-[a-z0-9_]+", payload.name),
        "owned local payload",
    )
    here_at_run = execution_root / HERE.relative_to(ROOT)
    cpu_at_run = execution_root / C.CPU.relative_to(ROOT)
    serialized = json.dumps(marker, sort_keys=True, separators=(",", ":"))
    commands = {
        "signature": (
            [
                "git",
                "-c",
                "gpg.ssh.allowedSignersFile=" + C.SIGNERS,
                "verify-commit",
                commit,
            ],
            30,
        ),
        "cpu": (["python3", "-I", "-B", str(cpu_at_run / "cpu.py"), "--live"], 120),
        "native-tests": (
            ["python3", "-I", "-B", str(here_at_run / "test_native.py")],
            60,
        ),
        "campaign-tests": (
            ["python3", "-I", "-B", str(here_at_run / "test_campaign.py")],
            60,
        ),
        "parser-tests": (
            ["python3", "-I", "-B", str(here_at_run / "test_results.py")],
            60,
        ),
        "pack": (
            [
                "git",
                "archive",
                "--format=tar.gz",
                "--output=" + str(payload / "source.tar.gz"),
                commit,
                "--",
                *C.SOURCE_PATHS,
            ],
            120,
        ),
        "upload": (
            [
                "scp",
                *C.SSH,
                *(str(payload / name) for name in (*N.PAYLOAD, "binding.json")),
                "mi300x:" + marker["path"] + "/",
            ],
            300,
        ),
        "native": (
            [
                "ssh",
                "-T",
                *C.SSH,
                "mi300x",
                shlex.join(
                    [
                        "/usr/bin/python3",
                        "-I",
                        "-B",
                        str(owned / "native.py"),
                        "run",
                        serialized,
                    ]
                ),
            ],
            3000,
        ),
        "collect": (
            [
                "scp",
                *C.SSH,
                "-r",
                "mi300x:" + str(owned / "results"),
                str(here_at_run / "remote"),
            ],
            300,
        ),
    }
    for remote in ("origin", "upstream"):
        commands["ref-" + remote] = (["git", "ls-remote", remote, C.BRANCH], 60)
        need(
            (HERE / "raw" / ("ref-" + remote) / "stdout").read_text()
            == commit + "\t" + C.BRANCH + "\n",
            "published source identity",
        )
    for stage, mode in (
        ("create", "create"),
        ("remote-inventory", "inventory"),
        ("cleanup", "cleanup"),
        ("absence", "absence"),
    ):
        commands[stage] = (
            C.control_spec(mode, marker),
            45 if mode == "create" else 120,
        )
    for stage in order:
        row = local[stage]
        need(
            row["command"] == commands[stage][0]
            and row["timeout_seconds"] == commands[stage][1],
            "exact local command: " + stage,
        )
        need(
            row["cwd"] == str(execution_root)
            and row["environment"] is None
            and row["started_ns"] >= previous,
            "ordered local execution",
        )
        expected_stdin = (
            hashlib.sha256(C.control_bytes()).hexdigest()
            if stage in ("create", "remote-inventory", "cleanup", "absence")
            else None
        )
        need(row["stdin_sha256"] == expected_stdin, "bound ownership script")
        previous = row["finished_ns"]
    need(read(HERE / "raw/create/stdout") == marker, "created exact owned directory")
    need(
        read(HERE / "raw/cleanup/stdout") == {"removed": str(owned)},
        "removed exact owned directory",
    )
    need(
        read(HERE / "raw/absence/stdout")
        == {"path_absent": True, "processes_absent": True},
        "recorded remote absence",
    )
    need(not payload.exists() and not payload.is_symlink(), "local transport absent")

    remote = HERE / "remote"
    for phase in ("before", "after"):
        need(
            read(remote / ("source-" + phase + ".json")) == binding["source_files"],
            "native source " + phase,
        )
    binaries = read(remote / "binaries.json")
    need(
        set(binaries) == set(N.BINARIES.values())
        and all(re.fullmatch(r"[0-9a-f]{64}", digest) for digest in binaries.values()),
        "exact three ELF identities",
    )
    need(read(remote / "binaries-after.json") == binaries, "unchanged ELFs")
    expected_stages = []
    previous = 0
    env = N.environment(owned)

    def check(stage, command, seconds, environment):
        nonlocal previous
        folder = remote / stage
        row = C.verify_receipt(folder)
        need(
            row["command"] == command
            and row["timeout_seconds"] == seconds
            and row["cwd"] == str(owned / "source")
            and row["environment"] == environment
            and row["stdin_sha256"] is None,
            "exact native command: " + stage,
        )
        need(row["started_ns"] >= previous, "ordered native execution")
        previous = row["finished_ns"]
        expected_stages.append(stage)
        return row

    for stage, command, seconds in N.build_specs(owned):
        check(stage, command, seconds, env)
    for tool in ("rustc", "cargo"):
        need(
            C.sha(remote / tool / "stdout")
            == cpu["toolchain"][tool + "_stdout_sha256"],
            "CPU/native toolchain identity",
        )
    parsed = []
    for name, backend, command, trial_env in N.trial_specs(owned):
        for index, bdf, uid in N.DEVICES:
            label = name + "-before"
            stage = label + "-gpu" + str(index)
            check(stage, N.observe_spec(label, index, bdf, uid), 100, env)
            need(
                (remote / stage / "stderr").read_bytes() == b"", "empty observer stderr"
            )
            N.parse_endpoint((remote / stage / "stdout").read_bytes(), index, bdf, uid)
        trial = check(name, command, 120, trial_env)
        need((remote / name / "stderr").read_bytes() == b"", "no benchmark diagnostics")
        parsed.append(
            {
                "trial": name,
                "result": R.parse_result(
                    (remote / name / "stdout").read_bytes(),
                    backend,
                    [d[2] for d in N.DEVICES],
                ),
            }
        )
        anchor = trial["finished_ns"]
        for suffix, delay in (("-settled", 2), ("-delayed", 20)):
            for position, (index, bdf, uid) in enumerate(N.DEVICES):
                label = name + suffix
                stage = label + "-gpu" + str(index)
                row = check(stage, N.observe_spec(label, index, bdf, uid), 100, env)
                need(
                    (remote / stage / "stderr").read_bytes() == b"",
                    "empty observer stderr",
                )
                if position == 0:
                    need(
                        row["started_ns"] >= anchor + delay * 10**9,
                        "declared postflight delay",
                    )
                N.parse_endpoint(
                    (remote / stage / "stdout").read_bytes(), index, bdf, uid
                )
            anchor = previous
    for stage, command, seconds in N.final_specs(owned):
        check(stage, command, seconds, env)
        for stream in ("stdout", "stderr"):
            need(
                (remote / stage / stream).read_bytes()
                == (remote / stage.removeprefix("after-") / stream).read_bytes(),
                "unchanged native toolchain " + stream,
            )
    need(
        {p.name for p in remote.iterdir() if p.is_dir()} == set(expected_stages),
        "exact native stage roster",
    )
    need(
        read(remote / "validated-results.json") == parsed,
        "replayed all six result rows",
    )
    finished = read(remote / "finished.json")
    need(
        finished
        == {
            "exit": 0,
            "commit": commit,
            "native_execution": True,
            "performance_acceptance": False,
            "formal_refinement": False,
        },
        "bounded native completion claims",
    )
    expected_state = C.new_state()
    for key, value in expected_state.items():
        if type(value) is bool:
            expected_state[key] = True
    need(
        N.same_json(read(HERE / "state.json"), expected_state),
        "successful terminal lifecycle state",
    )
    expected_files = set(static) | {
        "README.md",
        "binding.json",
        "owner.json",
        "collection.json",
        "state.json",
        "local-payload.json",
        "remote-inventory.json",
    }
    expected_dirs = {"raw", "remote"}
    for prefix, stages in (("raw", order), ("remote", expected_stages)):
        for stage in stages:
            expected_dirs.add(prefix + "/" + stage)
            expected_files.update(
                prefix + "/" + stage + "/" + name
                for name in ("receipt.json", "stdout", "stderr")
            )
    expected_files.update(
        "remote/" + name
        for name in (
            "source-before.json",
            "source-after.json",
            "binaries.json",
            "binaries-after.json",
            "validated-results.json",
            "finished.json",
        )
    )
    observed_files = set(B.inventory(HERE)) - {"SHA256SUMS"}
    observed_dirs = {
        p.relative_to(HERE).as_posix() for p in HERE.rglob("*") if p.is_dir()
    }
    need(
        observed_files == expected_files and observed_dirs == expected_dirs,
        "exact complete packet closure",
    )
    return {
        "source_commit": commit,
        "native_commands": len(expected_stages),
        "trials": parsed,
        "performance_acceptance": False,
        "formal_refinement": False,
    }


def seal(create=False):
    B = load(HERE / "campaign.py", "seal_hot_campaign").B
    inventory = B.inventory(HERE)
    inventory.pop("SHA256SUMS", None)
    contents = "".join(
        digest + "  " + name + "\n" for name, digest in inventory.items()
    )
    if create:
        with (HERE / "SHA256SUMS").open("x") as target:
            target.write(contents)
    need((HERE / "SHA256SUMS").read_text() == contents, "complete packet seal")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seal", action="store_true")
    args = parser.parse_args()
    result = verify()
    seal(args.seal)
    print(json.dumps(result, indent=2, sort_keys=True))
