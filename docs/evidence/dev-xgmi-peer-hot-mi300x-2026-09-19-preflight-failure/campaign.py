#!/usr/bin/env python3
"""Collect a signed-source, owned-directory hot peer-copy comparison."""

import sys

if not sys.flags.isolated:
    raise RuntimeError("use python3 -I -B")

import hashlib
import importlib.util
import json
import re
import secrets
import shlex
import shutil
import signal
import subprocess
import tarfile
import tempfile
from pathlib import Path

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
CPU = ROOT / "docs/evidence/dev-xgmi-peer-hot-controls-cpu-2026-09-19"
BASE_PATH = ROOT / "docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py"
BASE_SHA = "df740a80c83c94af02f7e7dfb1203dcf7c1eafbde0010e41219e4aac40b34bb7"
SIGNERS = (
    "/home/harsh/.codex-tmp/fe2o3-logical-mux-sdma-protocol-v2-20260918/allowed-signers"
)
SIGNERS_SHA = "ea67b34912b272ccbb46a4a83d8e00eb1a83e87832f089c1c134eed1786b560b"
BRANCH = "refs/heads/codex/r65-runtime-drain-versions"
SSH = [
    "-o",
    "BatchMode=yes",
    "-o",
    "ConnectTimeout=10",
    "-o",
    "ServerAliveInterval=10",
    "-o",
    "ServerAliveCountMax=3",
]
SOURCE_PATHS = [
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    ".cargo",
    "crates",
    "examples",
    "benchmarks/runtime_gfx942",
    "scripts/unsafe-source-baseline.json",
    "docs/runtime-primary-queue-release-v1.md",
]
STATIC = (
    "campaign.py",
    "native.py",
    "results.py",
    "test_campaign.py",
    "test_native.py",
    "test_results.py",
    "verify.py",
)
RECEIPT_KEYS = {
    "command",
    "cwd",
    "started_ns",
    "timeout_seconds",
    "pid",
    "exit",
    "error",
    "group_absent",
    "environment",
    "stdin_sha256",
    "finished_ns",
    "stdout_sha256",
    "stderr_sha256",
}
LOWER_SHA256 = re.compile(r"[0-9a-f]{64}", re.ASCII)


def load(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


if (
    BASE_PATH.is_symlink()
    or hashlib.sha256(BASE_PATH.read_bytes()).hexdigest() != BASE_SHA
):
    raise RuntimeError("authenticate owned-process recorder")
B = load(BASE_PATH, "hot_campaign_base")
N = load(HERE / "native.py", "hot_campaign_native")
B.PREFIX = N.B.PREFIX
need, sha = B.need, B.sha


def read(path):
    return json.loads(Path(path).read_bytes())


def control_bytes():
    source = BASE_PATH.read_bytes()
    need(hashlib.sha256(source).hexdigest() == BASE_SHA, "pinned ownership control")
    return (
        "import hashlib, sys\n"
        f"source = {source!r}\n"
        f"assert hashlib.sha256(source).hexdigest() == {BASE_SHA!r}\n"
        "scope = {'__name__': 'owned_control'}\n"
        "exec(compile(source, 'owned_control', 'exec'), scope)\n"
        f"scope['PREFIX'] = {B.PREFIX!r}\n"
        "scope['main']()\n"
    ).encode()


def control_spec(mode, marker):
    serialized = json.dumps(marker, sort_keys=True, separators=(",", ":"))
    return [
        "ssh",
        "-T",
        *SSH,
        "mi300x",
        shlex.join(["/usr/bin/python3", "-I", "-", mode, serialized]),
    ]


def source_archive_files(path):
    with tarfile.open(path, "r:gz") as archive:
        files = {}
        for member in archive.getmembers():
            if member.isdir():
                continue
            need(
                member.isfile() and member.name not in files,
                "ordinary unique source member",
            )
            need(
                not member.name.startswith("/")
                and all(part not in ("", ".", "..") for part in member.name.split("/")),
                "relative source member",
            )
            files[member.name] = hashlib.sha256(
                archive.extractfile(member).read()
            ).hexdigest()
        return files


def verify_receipt(folder):
    folder = Path(folder)
    need(folder.is_dir() and not folder.is_symlink(), "ordinary receipt directory")
    children = list(folder.iterdir())
    need(
        {child.name for child in children} == {"receipt.json", "stdout", "stderr"}
        and len(children) == 3
        and all(child.is_file() and not child.is_symlink() for child in children),
        "exact ordinary receipt files",
    )
    row = read(folder / "receipt.json")
    need(
        type(row) is dict and set(row) == RECEIPT_KEYS,
        "exact receipt keys",
    )
    need(
        type(row["command"]) is list
        and bool(row["command"])
        and all(type(item) is str and item for item in row["command"])
        and type(row["cwd"]) is str
        and bool(row["cwd"])
        and type(row["timeout_seconds"]) is int
        and row["timeout_seconds"] > 0,
        "typed receipt command",
    )
    need(
        type(row["exit"]) is int
        and row["exit"] == 0
        and row["error"] is None
        and type(row["group_absent"]) is bool
        and row["group_absent"]
        and type(row["pid"]) is int
        and row["pid"] > 0,
        "successful reaped command: " + str(folder),
    )
    need(
        type(row["started_ns"]) is int
        and row["started_ns"] > 0
        and type(row["finished_ns"]) is int
        and row["finished_ns"] >= row["started_ns"],
        "ordered receipt times",
    )
    environment = row["environment"]
    need(
        environment is None
        or (
            type(environment) is dict
            and all(
                type(key) is str and type(value) is str
                for key, value in environment.items()
            )
        ),
        "typed receipt environment",
    )
    stdin_sha = row["stdin_sha256"]
    need(
        stdin_sha is None
        or (type(stdin_sha) is str and LOWER_SHA256.fullmatch(stdin_sha) is not None),
        "typed receipt stdin digest",
    )
    for key, name in (("stdout_sha256", "stdout"), ("stderr_sha256", "stderr")):
        digest = row[key]
        need(
            type(digest) is str
            and LOWER_SHA256.fullmatch(digest) is not None
            and digest == sha(folder / name),
            "receipt output digest",
        )
    return row


def new_state():
    return {
        "schema": "fe2o3.xgmi-peer-hot-controller-state.v1",
        "create_attempted": False,
        "created": False,
        "uploaded": False,
        "native_attempted": False,
        "native_succeeded": False,
        "collected": False,
        "cleanup_attempted": False,
        "cleaned": False,
        "absence_attempted": False,
        "remote_absent": False,
        "local_payload_absent": False,
        "failure": None,
        "secondary_failures": [],
    }


def preserve_failure(state, primary, error, stage):
    detail = f"{type(error).__name__}: {error}"
    if primary is None:
        state["failure"] = {"stage": stage, "error": detail}
        return error
    state["secondary_failures"].append({"stage": stage, "error": detail})
    return primary


def settle_remote(step, check_absence, state, failure, stage_prefix):
    state["cleanup_attempted"] = True
    try:
        step("cleanup")
        state["cleaned"] = True
    except BaseException as error:
        failure = preserve_failure(state, failure, error, stage_prefix + "cleanup")
    state["absence_attempted"] = True
    try:
        check_absence(step)
        state["remote_absent"] = True
    except BaseException as error:
        failure = preserve_failure(state, failure, error, stage_prefix + "absence")
    return failure


def remote_lifecycle(step, collect_exact, check_absence, state):
    """Run one owned remote lifecycle, preserving the first failure."""

    failure = None
    state["create_attempted"] = True
    try:
        step("create")
        state["created"] = True
    except BaseException as error:
        failure = preserve_failure(state, failure, error, "create")

    if failure is None:
        try:
            step("upload")
            state["uploaded"] = True
        except BaseException as error:
            failure = preserve_failure(state, failure, error, "upload")

    if state["uploaded"]:
        state["native_attempted"] = True
        try:
            step("native")
            state["native_succeeded"] = True
        except BaseException as error:
            failure = preserve_failure(state, failure, error, "native")
        try:
            collect_exact(step)
            state["collected"] = True
        except BaseException as error:
            failure = preserve_failure(state, failure, error, "collection")
        if state["collected"]:
            failure = settle_remote(
                step, check_absence, state, failure, "post-collection-"
            )
    else:
        # Create may have taken effect even when its local receipt did not close.
        failure = settle_remote(step, check_absence, state, failure, "pre-native-")

    if failure is not None:
        raise failure


def path_absent(path):
    path = Path(path)
    return not path.exists() and not path.is_symlink()


def finalize_local(payload, state, failure):
    if payload is None:
        state["local_payload_absent"] = True
        return failure
    if not state["create_attempted"] or state["remote_absent"]:
        try:
            if not path_absent(payload):
                shutil.rmtree(payload)
            need(path_absent(payload), "owned local payload absent")
            state["local_payload_absent"] = True
        except BaseException as error:
            failure = preserve_failure(state, failure, error, "local-payload-cleanup")
    return failure


def run():
    need(
        not (HERE / "raw").exists() and not (HERE / "remote").exists(),
        "fresh packet; never overwrite an attempt",
    )
    for number in B.MANAGED:
        signal.signal(number, B.interrupted)
    state = new_state()
    payload = None
    commit = None
    failure = None
    try:
        commit = subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
        ).strip()
        need(
            subprocess.check_output(
                ["git", "status", "--porcelain=v1", "--untracked-files=all"], cwd=ROOT
            )
            == b"",
            "clean signed worktree",
        )
        need(sha(Path(SIGNERS)) == SIGNERS_SHA, "trusted signer list")
        for name in STATIC:
            relative = (HERE / name).relative_to(ROOT).as_posix()
            need(
                subprocess.check_output(
                    ["git", "show", commit + ":" + relative], cwd=ROOT
                )
                == (HERE / name).read_bytes(),
                "committed tooling: " + name,
            )
        rec = B.Recorder(HERE / "raw", ROOT)
        rec.run(
            "signature",
            [
                "git",
                "-c",
                "gpg.ssh.allowedSignersFile=" + SIGNERS,
                "verify-commit",
                commit,
            ],
            30,
        )
        rec.run("cpu", ["python3", "-I", "-B", str(CPU / "cpu.py"), "--live"], 120)
        rec.run(
            "native-tests",
            ["python3", "-I", "-B", str(HERE / "test_native.py")],
            60,
        )
        rec.run(
            "campaign-tests",
            ["python3", "-I", "-B", str(HERE / "test_campaign.py")],
            60,
        )
        rec.run(
            "parser-tests", ["python3", "-I", "-B", str(HERE / "test_results.py")], 60
        )
        for remote in ("origin", "upstream"):
            folder = rec.run("ref-" + remote, ["git", "ls-remote", remote, BRANCH], 60)
            need(
                (folder / "stdout").read_text() == commit + "\t" + BRANCH + "\n",
                "both published tips match signed source",
            )
        # The CPU packet is source-bound; only this exact signed tree is transported.
        cpu_source = read(CPU / "raw/source-before/stdout")["files"]
        payload = Path(
            tempfile.mkdtemp(prefix="fe2o3-peer-hot-payload-", dir=ROOT.parent)
        )
        B.write_json(HERE / "local-payload.json", {"path": str(payload)})
        source_tar = payload / "source.tar.gz"
        rec.run(
            "pack",
            [
                "git",
                "archive",
                "--format=tar.gz",
                "--output=" + str(source_tar),
                commit,
                "--",
                *SOURCE_PATHS,
            ],
            120,
        )
        need(
            source_archive_files(source_tar) == cpu_source,
            "signed archive equals CPU source",
        )
        # B.validate_members expects files only, so drop git archive's directory entries.
        normalized = payload / "source-files.tar.gz"
        with (
            tarfile.open(source_tar, "r:gz") as source,
            tarfile.open(normalized, "w:gz") as dest,
        ):
            for member in source.getmembers():
                if member.isfile():
                    dest.addfile(member, source.extractfile(member))
        normalized.replace(source_tar)
        for name in ("native.py", "results.py"):
            shutil.copyfile(HERE / name, payload / name)
        shutil.copyfile(BASE_PATH, payload / "base.py")
        binding = {
            "commit": commit,
            "source_files": cpu_source,
            "payload": {name: sha(payload / name) for name in N.PAYLOAD},
            "plan": N.PLAN,
        }
        B.write_json(payload / "binding.json", binding)
        B.write_json(HERE / "binding.json", binding)
        marker = {
            "path": B.PREFIX + secrets.token_hex(8),
            "commit": commit,
            "binding_sha256": sha(payload / "binding.json"),
        }
        B.write_json(HERE / "owner.json", marker)
        serialized = json.dumps(marker, sort_keys=True, separators=(",", ":"))

        def step(name):
            if name in ("create", "remote-inventory", "cleanup", "absence"):
                timeouts = {
                    "create": 45,
                    "remote-inventory": 120,
                    "cleanup": 120,
                    "absence": 120,
                }
                return rec.run(
                    name,
                    control_spec(name.removeprefix("remote-"), marker),
                    timeouts[name],
                    stdin=control_bytes(),
                )
            if name == "upload":
                return rec.run(
                    name,
                    [
                        "scp",
                        *SSH,
                        *(str(payload / item) for item in (*N.PAYLOAD, "binding.json")),
                        "mi300x:" + marker["path"] + "/",
                    ],
                    300,
                )
            if name == "native":
                return rec.run(
                    name,
                    [
                        "ssh",
                        "-T",
                        *SSH,
                        "mi300x",
                        shlex.join(
                            [
                                "/usr/bin/python3",
                                "-I",
                                "-B",
                                marker["path"] + "/native.py",
                                "run",
                                serialized,
                            ]
                        ),
                    ],
                    3000,
                )
            if name == "collect":
                return rec.run(
                    name,
                    [
                        "scp",
                        *SSH,
                        "-r",
                        "mi300x:" + marker["path"] + "/results",
                        str(HERE / "remote"),
                    ],
                    300,
                )
            raise ValueError("unknown controller step: " + name)

        def collect_exact(step):
            folder = step("remote-inventory")
            inventory = read(folder / "stdout")
            B.write_json(HERE / "remote-inventory.json", inventory)
            step("collect")
            need(
                B.inventory(HERE / "remote") == inventory,
                "complete byte-exact collection",
            )

        def check_absence(step):
            folder = step("absence")
            need(
                read(folder / "stdout")
                == {"path_absent": True, "processes_absent": True},
                "owned resources absent",
            )

        remote_lifecycle(step, collect_exact, check_absence, state)
    except BaseException as error:
        if state["failure"] is None:
            failure = preserve_failure(state, failure, error, "controller")
        else:
            failure = error
    finally:
        failure = finalize_local(payload, state, failure)
        if failure is not None:
            try:
                B.write_json(
                    HERE / "failure.json",
                    {
                        "error": f"{type(failure).__name__}: {failure}",
                        "primary": state["failure"],
                        "secondary_failures": state["secondary_failures"],
                    },
                )
            except BaseException as error:
                failure = preserve_failure(state, failure, error, "failure-summary")
        if failure is None:
            try:
                B.write_json(
                    HERE / "collection.json",
                    {
                        "source_commit": commit,
                        "cpu_seal_sha256": sha(CPU / "SHA256SUMS"),
                        "native_execution": True,
                        "performance_acceptance": False,
                        "formal_refinement": False,
                        "exclusive_reservation": False,
                        "remote_removed": state["remote_absent"],
                        "local_payload_removed": state["local_payload_absent"],
                    },
                )
            except BaseException as error:
                failure = preserve_failure(state, failure, error, "collection-summary")
        try:
            B.write_json(HERE / "state.json", state)
        except BaseException as error:
            failure = preserve_failure(state, failure, error, "state-summary")
    if failure is not None:
        raise failure
    print("collected; run verify.py --seal, then verify.py")


if __name__ == "__main__":
    run()
