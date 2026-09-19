#!/usr/bin/env python3
"""Bounded two-source XGMI experiment, without performance acceptance."""

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
PROTOCOL_SHA = "03165f5657fce095b656cb1225c7f569fcc71073fb5c175e9df6790d5d7b19e5"
protocol = HERE / "protocol.py"
if (
    protocol.is_symlink()
    or hashlib.sha256(protocol.read_bytes()).hexdigest() != PROTOCOL_SHA
):
    raise RuntimeError("authenticate comparison protocol before import")
spec = importlib.util.spec_from_file_location("bound_pair_protocol", protocol)
P = importlib.util.module_from_spec(spec)
spec.loader.exec_module(P)
B, need, read, sha = P.B, P.need, P.read, P.sha
PARSER_TEST_SHA = "98066fb8907a9bb6159db22a67a7008d8b28db2834250da00565576d9f0317d2"
OLD_VERIFIER_SHA = "5ead3694f667380968b9dffde3813607fa4a6774beb3aff49ca0bce5021dd1df"
BRANCH = "refs/heads/codex/r65-runtime-drain-versions"
INITIAL = [
    "calibration",
    "parser-calibration",
    "observer-tests",
    "cpu-baseline",
    "cpu-candidate",
    "signature-tooling",
    "signature-baseline",
    "signature-candidate",
    "ancestor-baseline",
    "ancestor-candidate",
    "source",
    "source-clean",
    "ref-origin",
    "ref-upstream",
]
LOCAL_ORDER = INITIAL + [
    "pack-baseline",
    "pack-candidate",
    "create",
    "upload",
    "native",
    "remote-inventory",
    "collect",
    "cleanup",
    "absence",
]


def committed_tools(commit):
    result = {}
    for name in P.STATIC_INPUTS:
        path = (HERE / name).relative_to(ROOT).as_posix()
        digest = hashlib.sha256(P.git(ROOT, ["show", commit + ":" + path])).hexdigest()
        need(
            digest == sha(HERE / name), "tool equals signed containing commit: " + name
        )
        result[name] = digest
    return result


def control_bytes():
    source = P.C.BASE_PATH.read_bytes()
    need(hashlib.sha256(source).hexdigest() == P.BASE_SHA, "pinned owner control")
    return (
        "import hashlib, os, sys\n"
        f"source = {source!r}\n"
        f"assert hashlib.sha256(source).hexdigest() == {P.BASE_SHA!r}\n"
        "scope = {'__name__': 'owned_control'}\n"
        "exec(compile(source, 'pinned_owned_control', 'exec'), scope)\n"
        f"scope['PREFIX'] = {P.PREFIX!r}\n"
        "if sys.argv[1] == 'create':\n"
        "    space = os.statvfs('/home/harsh')\n"
        "    scope['need'](space.f_bavail * space.f_frsize >= 8 * 1024**3, 'eight GiB for two builds')\n"
        "scope['main']()\n"
    ).encode()


def local_specs(payload, marker, *, execution_root=None):
    root = ROOT if execution_root is None else execution_root
    here = root / HERE.relative_to(ROOT)
    serialized = json.dumps(marker, sort_keys=True, separators=(",", ":"))
    result = {
        "calibration": (["python3", "-I", str(here / "test_campaign.py")], 120, None),
        "parser-calibration": (
            ["python3", "-I", str(root / (P.PRIOR + "test_campaign.py"))],
            60,
            None,
        ),
        "observer-tests": (
            ["python3", "-B", "benchmarks/runtime_gfx942/test_copy_host_observe.py"],
            60,
            None,
        ),
        "source": (
            ["python3", "-I", str(root / P.SELECTOR.relative_to(ROOT))],
            60,
            None,
        ),
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
    }
    for cohort, value in P.COHORTS.items():
        result["cpu-" + cohort] = (
            [
                "python3",
                "-I",
                str(root / value["cpu"] / "verify.py"),
                *(["--live"] if cohort == "candidate" else []),
            ],
            60,
            None,
        )
        result["ancestor-" + cohort] = (
            ["git", "merge-base", "--is-ancestor", value["commit"], marker["commit"]],
            30,
            None,
        )
        result["pack-" + cohort] = (
            [
                "python3",
                "-I",
                str(here / "protocol.py"),
                cohort,
                str(payload / ("source-" + cohort + ".tar.gz")),
            ],
            180,
            None,
        )
    for name, commit in {
        "tooling": marker["commit"],
        **{n: s["commit"] for n, s in P.COHORTS.items()},
    }.items():
        result["signature-" + name] = (
            [
                "git",
                "-c",
                "gpg.ssh.allowedSignersFile=" + P.SIGNERS,
                "verify-commit",
                commit,
            ],
            30,
            None,
        )
    for remote in ("origin", "upstream"):
        result["ref-" + remote] = (["git", "ls-remote", remote, BRANCH], 60, None)
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
            *(str(payload / n) for n in (*P.PAYLOAD, "binding.json")),
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
    files = B.inventory(source)
    modes = {}
    for name in files:
        mode = stat.S_IMODE((source / name).stat().st_mode)
        need(mode in (0o644, 0o755), "unchanged ordinary source permissions")
        modes[name] = "100755" if mode == 0o755 else "100644"
    return {"source_files": files, "source_modes": modes}


def checked_binding(owned, marker):
    need(sha(owned / "binding.json") == marker["binding_sha256"], "binding digest")
    binding = read(owned / "binding.json")
    P.validate_binding(binding)
    need(binding["commit"] == marker["commit"], "containing commit identity")
    for name, digest in binding["payload"].items():
        need(sha(owned / name) == digest, "unchanged payload")
    return binding


def initial_closure(owned):
    files = {"owner.json", "binding.json", *P.PAYLOAD}
    need(
        {p.name for p in owned.iterdir()} == files | {"results"},
        "exact initial owned closure",
    )
    for name in files:
        need(stat.S_ISREG((owned / name).lstat().st_mode), "ordinary initial payload")
    results = owned / "results"
    need(
        stat.S_ISDIR(results.lstat().st_mode) and not any(results.iterdir()),
        "new empty results directory",
    )


def execute_phase(rec, specs, selected, phase, results):
    label, _, enabled = phase

    def run(name):
        command, seconds, cwd, env = specs[name]
        rec.cwd = cwd
        return rec.run(name, command, seconds, env=env)

    def observe(prefix):
        failures = []
        for device in selected:
            try:
                run(prefix + "-gpu" + str(device[0]))
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
            (folder / "stdout").read_bytes(), selected, enabled
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
    binding, failure, binaries, results, secondary = None, None, {}, {}, []

    def sources():
        observed = {}
        for cohort, expected in binding["cohorts"].items():
            value = source_observation(owned / ("source-" + cohort))
            need(
                value == {k: expected[k] for k in ("source_files", "source_modes")},
                "exact unchanged cohort source",
            )
            observed[cohort] = value
        return observed

    def identities():
        checked_binding(owned, marker)
        observed = sources()
        for name, digest in binaries.items():
            need(sha(owned / P.binary(name)) == digest, "unchanged cohort binary")
        return observed

    try:
        initial_closure(owned)
        binding = checked_binding(owned, marker)
        (owned / "tmp").mkdir()
        for cohort, expected in binding["cohorts"].items():
            source = owned / ("source-" + cohort)
            source.mkdir()
            data = (owned / ("source-" + cohort + ".tar.gz")).read_bytes()
            P.validate_transport(
                data, expected["source_files"], expected["source_modes"]
            )
            with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
                archive.extractall(source, filter="data")
        B.write_json(rec.output / "source-before.json", sources())
        for name, command, seconds, cwd, env in P.build_specs(owned):
            rec.cwd = cwd
            rec.run(name, command, seconds, env=env)
        binaries = {name: sha(owned / P.binary(name)) for name in P.COHORTS}
        B.write_json(rec.output / "binaries.json", binaries)
        specs = {
            name: (command, seconds, cwd, env)
            for name, command, seconds, cwd, env in P.remote_specs(
                owned, binding["devices"]
            )
        }
        for phase in P.PHASES:
            identities()
            execute_phase(rec, specs, binding["devices"], phase, results)
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
                    "commit": marker["commit"],
                    "native_execution": failure is None,
                    "formal_refinement": False,
                    "performance_acceptance": False,
                    "failure": None
                    if failure is None
                    else f"{type(failure).__name__}: {failure}",
                    "secondary_failures": secondary,
                },
            )
        except BaseException as error:
            if failure is None:
                failure = error
    if failure is not None:
        raise failure


def preserve_failure(state, primary, error, stage):
    detail = f"{type(error).__name__}: {error}"
    if primary is None:
        state["failure"] = detail
        return error
    state["secondary_failures"].append({"stage": stage, "error": detail})
    return primary


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


def run_and_settle(step, state):
    failure = None
    try:
        step("native")
        state["native_success"] = True
    except BaseException as error:
        failure = error
        state["native_failure"] = f"{type(error).__name__}: {error}"
    try:
        collect_and_clean(step, state)
    except BaseException as error:
        failure = preserve_failure(state, failure, error, "remote-settlement")
    if failure is not None:
        raise failure


def create_upload_and_run(step, state):
    failure = None
    try:
        state["create_attempted"] = True
        step("create")
        state["created"] = True
        step("upload")
        state["native_attempted"] = True
        run_and_settle(step, state)
    except BaseException as error:
        failure = error
    finally:
        if state["create_attempted"] and not state["native_attempted"]:
            # Exact-marker cleanup also covers interruption after remote create
            # but before its local success assignment. No workload was attempted.
            for action, flag in (("cleanup", "cleaned"), ("absence", "absence")):
                try:
                    step(action)
                    state[flag] = True
                except BaseException as error:
                    failure = preserve_failure(
                        state, failure, error, "pre-native-" + action
                    )
    if failure is not None:
        raise failure


def finalize_local(payload, state, failure):
    try:
        if state["absence"] or not state["create_attempted"]:
            shutil.rmtree(payload)
            state["local_payload_absent"] = not payload.exists()
    except BaseException as error:
        failure = preserve_failure(state, failure, error, "local-payload-cleanup")
    try:
        B.write_json(HERE / "controller-state.json", state)
    except BaseException as error:
        failure = preserve_failure(state, failure, error, "state-finalization")
        try:
            print(
                json.dumps({"controller_finalization_failed": state}), file=sys.stderr
            )
        except BaseException:
            pass
    return failure


def run_local(indices):
    selected = P.devices(indices)
    for cohort in P.COHORTS:
        P.cpu_source(cohort)
    need(
        sha(P.SELECTOR) == P.SELECTOR_SHA and sha(P.SIGNERS) == P.SIGNERS_SHA,
        "source selector and signing trust",
    )
    need(
        sha(ROOT / (P.PRIOR + "test_campaign.py")) == PARSER_TEST_SHA
        and sha(ROOT / (P.PRIOR + "verify.py")) == OLD_VERIFIER_SHA,
        "authenticated parser calibration closure",
    )
    commit = P.git(ROOT, ["rev-parse", "HEAD"]).decode().strip()
    tool_hashes = committed_tools(commit)
    rec = B.Recorder(HERE / "local", ROOT)
    dummy = {"path": P.PREFIX + "0" * 16, "commit": commit, "binding_sha256": "0" * 64}
    initial = local_specs(Path("/unused"), dummy)
    for name in INITIAL:
        command, seconds, stdin = initial[name]
        rec.run(name, command, seconds, stdin=stdin)
    need(
        (HERE / "local/source-clean/stdout").read_bytes() == b"",
        "clean committed compiler inputs",
    )
    snapshot = read(HERE / "local/source/stdout")
    need(
        snapshot == {"base": commit, "files": P.cpu_source("candidate")},
        "current selected source equals signed candidate",
    )
    for remote in ("origin", "upstream"):
        need(
            (HERE / ("local/ref-" + remote) / "stdout").read_text()
            == commit + "\t" + BRANCH + "\n",
            "both published tooling refs",
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
    failure = None
    try:
        for name, source in {
            "protocol.py": HERE / "protocol.py",
            "campaign.py": HERE / "campaign.py",
            "attribution.py": P.ATTRIBUTION_PATH,
            "base.py": P.C.BASE_PATH,
        }.items():
            shutil.copyfile(source, payload / name)
        commands = local_specs(payload, dummy)
        cohorts = {}
        for cohort in P.COHORTS:
            command, seconds, stdin = commands["pack-" + cohort]
            folder = rec.run("pack-" + cohort, command, seconds, stdin=stdin)
            cohorts[cohort] = read(folder / "stdout")
        binding = {
            "schema": "fe2o3.xgmi-pair-source-comparison.v1",
            "commit": commit,
            "tools": tool_hashes,
            "helpers": {"attribution.py": P.ATTRIBUTION_SHA, "base.py": P.BASE_SHA},
            "devices": selected,
            "plan": P.PLAN,
            "cohorts": cohorts,
            "source_difference": P.source_difference(cohorts),
            "payload": {name: sha(payload / name) for name in P.PAYLOAD},
        }
        P.validate_binding(binding)
        B.write_json(payload / "binding.json", binding)
        B.write_json(HERE / "binding.json", binding)
        marker = {
            "commit": commit,
            "path": P.PREFIX + secrets.token_hex(8),
            "binding_sha256": sha(payload / "binding.json"),
        }
        B.write_json(HERE / "owner.json", marker)
        commands = local_specs(payload, marker)

        def step(name):
            command, seconds, stdin = commands[name]
            return rec.run(name, command, seconds, stdin=stdin)

        create_upload_and_run(step, state)
    except BaseException as error:
        failure = error
        state["failure"] = f"{type(error).__name__}: {error}"
    finally:
        failure = finalize_local(payload, state, failure)
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
        run_remote(P.parse_json(args.arguments[0]))


if __name__ == "__main__":
    main()
