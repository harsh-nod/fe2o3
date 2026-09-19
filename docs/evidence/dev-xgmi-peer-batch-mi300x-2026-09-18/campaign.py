#!/usr/bin/env python3
"""Run one bounded same-binary ordinary/aggregate peer-copy campaign."""

import sys

if not sys.flags.isolated:
    raise RuntimeError("run with python3 -I")

import argparse
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import resource
import secrets
import shlex
import shutil
import signal
import stat
import tarfile
import tempfile

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
PROTOCOL_SHA256 = "4471b3498aa314bc0f539db50f0375e3a44c1966045d5b82e887f24267620e95"
LIFECYCLE_SHA256 = "f6f79e527249b3c8d4ba02e95644114e930ba9979b6ee5ceb5df088723fb4aba"
LIFECYCLE_PATH = (
    ROOT
    / "docs/evidence/dev-topology-prechecked-comparison-mi300x-2026-09-18/campaign.py"
)


def authenticated_module(path, digest, name):
    if (
        path.is_symlink()
        or not path.is_file()
        or hashlib.sha256(path.read_bytes()).hexdigest() != digest
    ):
        raise RuntimeError("authenticate helper before import: " + str(path))
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


P = authenticated_module(HERE / "protocol.py", PROTOCOL_SHA256, "peer_batch_protocol")
B, need, read, sha = P.B, P.need, P.read, P.sha
INITIAL = [
    "calibration",
    "cpu",
    "signature-tooling",
    "signature-source",
    "ancestor",
    "source",
    "source-clean",
    "ref-origin",
    "ref-upstream",
    "pack",
]
LOCAL_ORDER = INITIAL + [
    "create",
    "upload",
    "native",
    "remote-inventory",
    "collect",
    "cleanup",
    "absence",
]


def lifecycle_module():
    return authenticated_module(
        LIFECYCLE_PATH, LIFECYCLE_SHA256, "peer_batch_controller_lifecycle"
    )


def committed_tools(commit):
    result = {}
    for name in P.STATIC_INPUTS:
        relative = (HERE / name).relative_to(ROOT).as_posix()
        value = P.T.git(ROOT, ["show", commit + ":" + relative])
        digest = hashlib.sha256(value).hexdigest()
        need(digest == sha(HERE / name), "tool equals signed commit: " + name)
        result[name] = digest
    return result


def control_bytes():
    source = P.helper_source("base.py").read_bytes()
    need(
        hashlib.sha256(source).hexdigest() == P.HELPERS["base.py"][1],
        "pinned ownership control",
    )
    return (
        "import hashlib, os, sys\n"
        f"source = {source!r}\n"
        f"assert hashlib.sha256(source).hexdigest() == {P.HELPERS['base.py'][1]!r}\n"
        "scope = {'__name__': 'owned_control'}\n"
        "exec(compile(source, 'pinned_owned_control', 'exec'), scope)\n"
        f"scope['PREFIX'] = {P.PREFIX!r}\n"
        "if sys.argv[1] == 'create':\n"
        "    space = os.statvfs('/home/harsh')\n"
        "    scope['need'](space.f_bavail * space.f_frsize >= 8 * 1024**3, 'eight GiB free')\n"
        "scope['main']()\n"
    ).encode()


def local_specs(payload, marker, *, execution_root=None):
    root = ROOT if execution_root is None else Path(execution_root)
    here = root / HERE.relative_to(ROOT)
    cpu = root / P.CPU_RELATIVE
    selector = root / P.T.SELECTOR.relative_to(ROOT)
    serialized = json.dumps(marker, sort_keys=True, separators=(",", ":"))
    result = {
        "calibration": (
            ["python3", "-I", "-B", str(here / "test_campaign.py")],
            120,
            None,
        ),
        "cpu": (
            ["python3", "-I", "-B", str(cpu / "verify.py"), "--live"],
            120,
            None,
        ),
        "signature-tooling": (
            [
                "git",
                "-c",
                "gpg.ssh.allowedSignersFile=" + P.SIGNERS,
                "verify-commit",
                marker["commit"],
            ],
            30,
            None,
        ),
        "signature-source": (
            [
                "git",
                "-c",
                "gpg.ssh.allowedSignersFile=" + P.SIGNERS,
                "verify-commit",
                P.SOURCE_COMMIT,
            ],
            30,
            None,
        ),
        "ancestor": (
            ["git", "merge-base", "--is-ancestor", P.SOURCE_COMMIT, marker["commit"]],
            30,
            None,
        ),
        "source": (["python3", "-I", str(selector)], 60, None),
        "source-clean": (
            [
                "git",
                "status",
                "--porcelain=v1",
                "--untracked-files=all",
                "--",
                *P.SOURCE_PATHS,
            ],
            30,
            None,
        ),
        "pack": (
            [
                "python3",
                "-I",
                str(here / "protocol.py"),
                str(payload / "source.tar.gz"),
            ],
            180,
            None,
        ),
    }
    for remote in ("origin", "upstream"):
        result["ref-" + remote] = (
            ["git", "ls-remote", remote, P.TOOLING_BRANCH],
            60,
            None,
        )
    for name, mode in (
        ("create", "create"),
        ("remote-inventory", "inventory"),
        ("cleanup", "cleanup"),
        ("absence", "absence"),
    ):
        result[name] = (
            [
                "ssh",
                "-T",
                *P.SSH,
                "mi300x",
                shlex.join(["/usr/bin/python3", "-I", "-", mode, serialized]),
            ],
            45 if mode == "create" else 120,
            control_bytes(),
        )
    result["upload"] = (
        [
            "scp",
            *P.SSH,
            *(str(payload / name) for name in (*P.PAYLOAD, "binding.json")),
            "mi300x:" + marker["path"] + "/",
        ],
        300,
        None,
    )
    result["native"] = (
        [
            "ssh",
            "-T",
            *P.SSH,
            "mi300x",
            shlex.join(
                [
                    "/usr/bin/python3",
                    "-I",
                    marker["path"] + "/campaign.py",
                    "remote",
                    serialized,
                ]
            ),
        ],
        P.native_timeout(),
        None,
    )
    result["collect"] = (
        [
            "scp",
            "-r",
            *P.SSH,
            "mi300x:" + marker["path"] + "/results",
            str(here / "remote"),
        ],
        300,
        None,
    )
    need(set(result) == set(LOCAL_ORDER), "complete local command plan")
    return result


def source_observation(source):
    files, modes = B.inventory(source), {}
    for name in files:
        mode = stat.S_IMODE((source / name).stat().st_mode)
        need(mode in (0o644, 0o755), "ordinary source permissions")
        modes[name] = "100755" if mode == 0o755 else "100644"
    return {"source_files": files, "source_modes": modes}


def checked_binding(owned, marker, *, local_cpu=False):
    need(sha(owned / "binding.json") == marker["binding_sha256"], "binding digest")
    binding = read(owned / "binding.json")
    P.validate_binding(binding, P.cpu_binding() if local_cpu else None)
    need(binding["tooling_commit"] == marker["commit"], "tooling marker identity")
    for name, digest in binding["payload"].items():
        need(sha(owned / name) == digest, "unchanged payload: " + name)
    return binding


def initial_closure(owned):
    files = {"owner.json", "binding.json", *P.PAYLOAD}
    need(
        {path.name for path in owned.iterdir()} == files | {"results"},
        "exact initial owned closure",
    )
    for name in files:
        need(stat.S_ISREG((owned / name).lstat().st_mode), "ordinary payload file")
    results = owned / "results"
    need(
        stat.S_ISDIR(results.lstat().st_mode) and not any(results.iterdir()),
        "new empty results directory",
    )


def execute_phase(rec, specs, selected, phase, results):
    label = phase[0]

    def run(name):
        command, seconds, cwd, env = specs[name]
        rec.cwd = cwd
        return rec.run(name, command, seconds, env=env)

    def observe(prefix):
        failures = []
        for device in selected:
            try:
                folder = run(prefix + "-gpu" + str(device[0]))
                need(
                    (folder / "stderr").read_bytes() == b"",
                    "empty observer stderr",
                )
                P.parse_endpoint((folder / "stdout").read_bytes(), device)
            except BaseException as error:
                failures.append(error)
        if failures:
            raise failures[0]

    failure = None
    try:
        observe(label + "-before")
        folder = run(label)
        need((folder / "stderr").read_bytes() == b"", "empty workload stderr")
        results[label] = P.parse_transcript(
            (folder / "stdout").read_bytes(), selected, phase
        )
    except BaseException as error:
        failure = error
    failure = B.settled_postflight(observe, label, failure)
    if failure is not None:
        raise failure


def run_remote(marker):
    owned = B.owned_path(marker)
    need(HERE == owned, "private remote runner identity")
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    rec = B.Recorder(owned / "results", owned)
    binding, failure, binary, results, secondary = None, None, None, {}, []

    def identities():
        checked_binding(owned, marker)
        observed = source_observation(owned / "source")
        need(
            observed
            == {
                key: binding["source"][key] for key in ("source_files", "source_modes")
            },
            "unchanged signed source",
        )
        if binary is not None:
            need(sha(owned / P.BINARY) == binary, "unchanged benchmark ELF")
        return observed

    try:
        initial_closure(owned)
        binding = checked_binding(owned, marker)
        (owned / "tmp").mkdir()
        source = owned / "source"
        source.mkdir()
        data = (owned / "source.tar.gz").read_bytes()
        P.T.validate_transport(
            data, binding["source"]["source_files"], binding["source"]["source_modes"]
        )
        with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
            archive.extractall(source, filter="data")
        B.write_json(rec.output / "source-before.json", identities())
        build_specs = P.build_specs(owned)
        need(
            [spec[0] for spec in build_specs]
            == ["rustc", "cargo", "kernel", "rocm", "build"],
            "exact toolchain-before-build order",
        )
        for name, command, seconds, cwd, env in build_specs[:2]:
            rec.cwd = cwd
            rec.run(name, command, seconds, env=env)
        P.validate_toolchain_receipts(rec.output, binding["cpu"]["toolchain"])
        for name, command, seconds, cwd, env in build_specs[2:]:
            rec.cwd = cwd
            rec.run(name, command, seconds, env=env)
        binary = sha(owned / P.BINARY)
        B.write_json(rec.output / "binaries.json", {P.BINARY: binary})
        specs = {
            name: (command, seconds, cwd, env)
            for name, command, seconds, cwd, env in P.remote_specs(
                owned, binding["devices"]
            )
        }
        for phase in P.PHASES:
            identities()
            execute_phase(rec, specs, binding["devices"], phase, results)
        need(
            set(results) == {phase[0] for phase in P.PHASES},
            "complete nine-process result roster",
        )
    except BaseException as error:
        failure = error
    finally:
        if binding is not None:
            try:
                B.write_json(rec.output / "source-after.json", identities())
            except BaseException as error:
                if failure is None:
                    failure = error
                else:
                    secondary.append(
                        {
                            "stage": "final-identities",
                            "error": f"{type(error).__name__}: {error}",
                        }
                    )
        try:
            B.write_json(rec.output / "parsed.json", results)
            B.write_json(
                rec.output / "finished.json",
                {
                    "tooling_commit": marker["commit"],
                    "source_commit": P.SOURCE_COMMIT,
                    "native_execution": failure is None,
                    "formal_refinement": False,
                    "performance_acceptance": False,
                    "hip_hsa_parity": False,
                    "failure": (
                        None
                        if failure is None
                        else f"{type(failure).__name__}: {failure}"
                    ),
                    "secondary_failures": secondary,
                },
            )
        except BaseException as error:
            if failure is None:
                failure = error
    if failure is not None:
        raise failure


def collect_and_clean(step, state):
    manifest = read(step("remote-inventory") / "stdout")
    need(manifest, "nonempty result manifest")
    B.write_json(HERE / "remote-inventory.json", manifest)
    step("collect")
    need(B.inventory(HERE / "remote") == manifest, "complete collection before cleanup")
    state["collected"] = True
    step("cleanup")
    state["cleaned"] = True
    step("absence")
    state["absence"] = True


def controlled_create_upload_run(step, state, lifecycle):
    original = lifecycle.collect_and_clean
    lifecycle.collect_and_clean = collect_and_clean
    try:
        lifecycle.create_upload_and_run(step, state)
    finally:
        lifecycle.collect_and_clean = original


def path_absent(path):
    path = Path(path)
    return not path.exists() and not path.is_symlink()


def run_local(indices):
    P.configured()
    selected, cpu = P.devices(indices), P.cpu_binding()
    need(selected == P.devices(), "campaign is fixed to physical GPUs 1 and 2")
    need(
        sha(P.SIGNERS) == P.SIGNERS_SHA,
        "authenticated signing trust",
    )
    tooling_commit = P.T.git(ROOT, ["rev-parse", "HEAD"]).decode().strip()
    tools = committed_tools(tooling_commit)
    rec = B.Recorder(HERE / "local", ROOT)
    dummy = {
        "path": P.PREFIX + "0" * 16,
        "commit": tooling_commit,
        "binding_sha256": "0" * 64,
    }
    initial = local_specs(Path("/unused"), dummy)
    for name in INITIAL[:-1]:
        command, seconds, stdin = initial[name]
        rec.run(name, command, seconds, stdin=stdin)
    need((HERE / "local/source-clean/stdout").read_bytes() == b"", "clean inputs")
    snapshot = read(HERE / "local/source/stdout")
    need(
        snapshot == {"base": tooling_commit, "files": P.cpu_source()},
        "live tooling tree files equal CPU source",
    )
    for remote in ("origin", "upstream"):
        need(
            (HERE / ("local/ref-" + remote) / "stdout").read_text()
            == tooling_commit + "\t" + P.TOOLING_BRANCH + "\n",
            "published signed tooling ref",
        )
    payload = Path(
        tempfile.mkdtemp(prefix=P.LOCAL_PREFIX, dir="/home/harsh/.codex-tmp")
    )
    state = {
        "create_attempted": False,
        "created": False,
        "native_attempted": False,
        "native_success": False,
        "collected": False,
        "cleaned": False,
        "absence": False,
        "local_payload_absent": False,
        "local_payload": str(payload),
        "failure": None,
        "native_failure": None,
        "secondary_failures": [],
    }
    failure, lifecycle = None, lifecycle_module()
    try:
        for name in ("protocol.py", "campaign.py"):
            shutil.copyfile(HERE / name, payload / name)
        for name, (_, digest) in P.HELPERS.items():
            source = P.helper_source(name)
            need(sha(source) == digest, "unchanged helper: " + name)
            shutil.copyfile(source, payload / name)
        commands = local_specs(payload, dummy)
        command, seconds, stdin = commands["pack"]
        folder = rec.run("pack", command, seconds, stdin=stdin)
        source = read(folder / "stdout")
        binding = {
            "schema": "fe2o3.xgmi-peer-batch-native.v1",
            "tooling_commit": tooling_commit,
            "source_commit": P.SOURCE_COMMIT,
            "cpu": {
                "relative": P.CPU_RELATIVE,
                "seal_sha256": P.CPU_SEAL_SHA256,
                "binding_sha256": sha(ROOT / P.CPU_RELATIVE / "binding.json"),
                "source_snapshot_sha256": cpu["source_snapshot_sha256"],
                "qualified_source_base": cpu["source_base"],
                "toolchain": P.toolchain_binding(cpu["toolchain"]),
            },
            "tools": tools,
            "helpers": P.helper_identities(),
            "devices": selected,
            "plan": P.PLAN,
            "source": source,
            "payload": {name: sha(payload / name) for name in P.PAYLOAD},
        }
        P.validate_binding(binding, cpu)
        B.write_json(payload / "binding.json", binding)
        B.write_json(HERE / "binding.json", binding)
        marker = {
            "path": P.PREFIX + secrets.token_hex(8),
            "commit": tooling_commit,
            "binding_sha256": sha(payload / "binding.json"),
        }
        B.write_json(HERE / "owner.json", marker)
        commands = local_specs(payload, marker)

        def step(name):
            command, seconds, stdin = commands[name]
            return rec.run(name, command, seconds, stdin=stdin)

        controlled_create_upload_run(step, state, lifecycle)
    except BaseException as error:
        failure = error
        state["failure"] = f"{type(error).__name__}: {error}"
    finally:
        try:
            if state["absence"] or not state["create_attempted"]:
                shutil.rmtree(payload)
                state["local_payload_absent"] = path_absent(payload)
        except BaseException as error:
            failure = lifecycle.preserve_failure(
                state, failure, error, "local-payload-cleanup"
            )
        try:
            B.write_json(HERE / "controller-state.json", state)
        except BaseException as error:
            failure = lifecycle.preserve_failure(
                state, failure, error, "state-finalization"
            )
    if failure is not None:
        raise failure


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("run", "remote"))
    parser.add_argument("arguments", nargs="+")
    args = parser.parse_args()
    os.umask(0o077)
    for number in B.MANAGED:
        signal.signal(number, B.interrupted)
    if args.mode == "run":
        run_local([int(value) for value in args.arguments])
    else:
        need(len(args.arguments) == 1, "one ownership marker")
        run_remote(P.T.parse_json(args.arguments[0]))


if __name__ == "__main__":
    main()
