#!/usr/bin/env python3
"""Bounded same-binary XGMI attribution experiment; never parity acceptance."""

import sys

if not sys.flags.isolated:
    raise RuntimeError("run with python3 -I")

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import resource
import secrets
import shlex
import shutil
import signal
import subprocess
import tarfile
import tempfile

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
QUALIFIED_SOURCE_COMMIT = "5543ac32e9030a35b68b40d0d42795abcd691933"
CPU = ROOT / "docs/evidence/dev-xgmi-host-attribution-cpu-2026-09-18"
CPU_SEAL = "c298265faac9113b04fee5b4c3959f54327ef3470e461af0ab053a152b4b418f"
SELECTOR = ROOT / "docs/evidence/dev-sdma-direct-readback-cpu-2026-09-18/source.py"
SELECTOR_SHA = "a7a8b8e73aa1a0817e13f9e575369403ece42f4eb702fd1ec04d9bc57fcff953"
BASE_SHA = "df740a80c83c94af02f7e7dfb1203dcf7c1eafbde0010e41219e4aac40b34bb7"
BASE_PATH = HERE / "base.py"
if not BASE_PATH.exists():
    BASE_PATH = ROOT / "docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py"
PREFIX = "/home/harsh/fe2o3-xgmi-attribution-20260918."
SIGNERS = (
    "/home/harsh/.codex-tmp/fe2o3-logical-mux-sdma-protocol-v2-20260918/allowed-signers"
)
SIGNERS_SHA = "ea67b34912b272ccbb46a4a83d8e00eb1a83e87832f089c1c134eed1786b560b"
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
PHASES = [["off1", False], ["on1", True], ["on2", True], ["off2", False]]
BINARY = "target/release/examples/gfx942-runtime-xgmi-peer-benchmark"
TOOLS = ("campaign.py", "test_campaign.py", "verify.py")
STATIC_INPUTS = (*TOOLS, ".gitattributes")
PAYLOAD = ("campaign.py", "base.py", "source.tar.gz")
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
DEVICE_ROSTER = [
    (0, "0000:05:00.0", "0x6ced1647a296545c"),
    (1, "0000:26:00.0", "0xab83d2ffef0d3cdf"),
    (2, "0000:46:00.0", "0xd2e26fef80cf5c33"),
    (3, "0000:65:00.0", "0x189ca857de1ef2fb"),
    (4, "0000:85:00.0", "0x54f88318ca05093d"),
    (5, "0000:a6:00.0", "0xb7baafd0fb173d8e"),
    (6, "0000:c6:00.0", "0x10a254ce4987e716"),
    (7, "0000:e5:00.0", "0x53691ef168a0147d"),
]


def need(value, message):
    if not value:
        raise RuntimeError(message)


def sha(path):
    path = Path(path)
    need(path.is_file() and not path.is_symlink(), "ordinary file: " + str(path))
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_pinned(path, digest, name):
    need(sha(path) == digest, "pinned helper: " + str(path))
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


B = load_pinned(BASE_PATH, BASE_SHA, "xgmi_owned_runner")
B.PREFIX = PREFIX


def pairs(items):
    result = {}
    for key, value in items:
        need(key not in result, "duplicate key")
        result[key] = value
    return result


def parse_json(text):
    return json.loads(
        text,
        object_pairs_hook=pairs,
        parse_constant=lambda _: need(False, "nonfinite JSON"),
    )


def read(path):
    sha(path)
    return parse_json(Path(path).read_text())


def same_json(actual, expected):
    return json.dumps(actual, sort_keys=True, allow_nan=False) == json.dumps(
        expected, sort_keys=True, allow_nan=False
    )


def cpu_integrity():
    need(sha(CPU / "SHA256SUMS") == CPU_SEAL, "pinned CPU seal")
    manifest = "".join(
        digest + "  " + name + "\n"
        for name, digest in B.inventory(CPU).items()
        if name != "SHA256SUMS"
    )
    need(
        (CPU / "SHA256SUMS").read_text() == manifest,
        "CPU helpers authenticated before execution",
    )


def committed_tools(commit):
    need(re.fullmatch(r"[0-9a-f]{40}", commit), "canonical containing commit")
    result = {}
    for name in STATIC_INPUTS:
        relative = (HERE / name).relative_to(ROOT).as_posix()
        data = subprocess.check_output(
            ["git", "show", commit + ":" + relative], cwd=ROOT
        )
        digest = hashlib.sha256(data).hexdigest()
        need(
            digest == sha(HERE / name),
            "launch input equals signed commit tree: " + name,
        )
        result[name] = digest
    return result


def devices(indices):
    need(
        len(indices) == 2
        and all(type(i) is int and 0 <= i < 8 for i in indices)
        and indices[0] != indices[1],
        "two distinct known physical GPUs",
    )
    return [list(DEVICE_ROSTER[i]) for i in indices]


def control_bytes():
    source = BASE_PATH.read_bytes()
    need(hashlib.sha256(source).hexdigest() == BASE_SHA, "control helper")
    return (
        "import hashlib\n"
        f"source = {source!r}\n"
        f"assert hashlib.sha256(source).hexdigest() == {BASE_SHA!r}\n"
        "scope = {'__name__': 'owned_control'}\n"
        "exec(compile(source, 'pinned_owned_control', 'exec'), scope)\n"
        f"scope['PREFIX'] = {PREFIX!r}\n"
        "scope['main']()\n"
    ).encode()


def environment(owned):
    return {
        "HOME": "/home/harsh",
        "USER": "harsh",
        "PATH": "/home/harsh/.cargo/bin:/opt/rocm/bin:/usr/bin:/bin",
        "LANG": "C",
        "LC_ALL": "C",
        "CARGO_TARGET_DIR": str(owned / "target"),
        "CARGO_INCREMENTAL": "0",
        "CARGO_BUILD_JOBS": "2",
        "CARGO_TERM_COLOR": "never",
        "RUSTUP_TOOLCHAIN": "nightly-2026-04-03",
        "TMPDIR": str(owned / "tmp"),
    }


def builds():
    return [
        ("rustc", ["rustc", "-vV"], 30),
        ("cargo", ["cargo", "-V"], 30),
        ("kernel", ["uname", "-a"], 30),
        ("rocm", ["cat", "/opt/rocm/.info/version"], 30),
        (
            "build",
            [
                "cargo",
                "build",
                "--frozen",
                "--release",
                "-p",
                "fe2o3-runtime",
                "--features",
                "hardware-diagnostic",
                "--example",
                "gfx942-runtime-xgmi-peer-benchmark",
            ],
            1200,
        ),
    ]


def observer_command(device):
    index, bdf, uid = device
    return [
        "/usr/bin/python3",
        "-I",
        "benchmarks/runtime_gfx942/copy-host-observe.py",
        "--gpu-index",
        str(index),
        "--pci-bdf",
        bdf,
        "--unique-id",
        uid,
    ]


def workload(owned, selected, enabled):
    return [
        str(owned / BINARY),
        *(d[2] for d in selected),
        "1048576",
        "1",
        "10",
        "30",
        *(["--diagnose-xgmi"] if enabled else []),
    ]


def remote_commands(owned, selected):
    result = builds()
    for name, enabled in PHASES:
        result.extend(
            (name + "-before-gpu" + str(d[0]), observer_command(d), 100)
            for d in selected
        )
        result.append((name, workload(owned, selected, enabled), 120))
        for suffix in ("settled", "delayed"):
            result.extend(
                (name + "-" + suffix + "-gpu" + str(d[0]), observer_command(d), 100)
                for d in selected
            )
    return result


def kv(line):
    items = [part.split("=") for part in line.split(" ")]
    need(all(len(item) == 2 and all(item) for item in items), "canonical key-value row")
    return pairs(items)


def integer(value, positive=False):
    need(
        isinstance(value, str) and re.fullmatch(r"0|[1-9][0-9]{0,19}", value),
        "bounded canonical decimal",
    )
    result = int(value)
    need(result <= (1 << 64) - 1 and (not positive or result > 0), "u64 duration")
    return result


def parse_transcript(data, selected, enabled):
    need(
        isinstance(data, bytes)
        and data.endswith(b"\n")
        and b"\r" not in data
        and b"\0" not in data,
        "complete ASCII transcript",
    )
    lines = data.decode("ascii").splitlines()
    need(2 <= len(lines) <= 40002, "bounded transcript roster")
    records, aggregates = lines[:-2], []
    need(
        (324 <= len(records) <= 40000) if enabled else not records,
        "exact diagnostic mode",
    )
    metrics = {
        d + s
        for d in ("forward", "reverse")
        for s in ("_p50_ns", "_p95_ns", "_p50_GBps")
    }
    for line, measurement in zip(lines[-2:], ("remap-per-round", "persistent-hot")):
        row = kv(line)
        fixed = B.expected_fields("kfd", 1, measurement)
        fixed["unique_ids"] = ",".join(d[2][2:] for d in selected)
        if enabled:
            fixed["diagnostic"] = "xgmi-host-stages-v1"
        need(
            set(row) == set(fixed) | metrics
            and all(row[k] == v for k, v in fixed.items()),
            "exact aggregate mode, identity and controls",
        )
        for direction in ("forward", "reverse"):
            median, upper = (
                integer(row[direction + suffix], True)
                for suffix in ("_p50_ns", "_p95_ns")
            )
            bandwidth = row[direction + "_p50_GBps"]
            need(
                upper >= median
                and re.fullmatch(r"(?:0|[1-9][0-9]*)\.[0-9]{3}", bandwidth),
                "latency order and bandwidth shape",
            )
            need(
                abs(float(bandwidth) - 1048576 / median) <= 0.00050001,
                "consistent rounded bandwidth",
            )
        aggregates.append(row)
    observations, expected_id, open_call = [], 7, False
    duration_keys = {
        "opening_currentness_ns",
        "preparation_ns",
        "native_call_ns",
        "closing_currentness_ns",
        "total_ns",
    }
    keys = duration_keys | {
        "schema",
        "backend",
        "ordinal",
        "backend_submission",
        "source_uid",
        "destination_uid",
        "call",
        "authority",
        "teardown",
    }
    for ordinal, line in enumerate(records):
        row = kv(line)
        need(
            set(row) == keys
            and row["schema"] == "fe2o3.xgmi-host-attribution.v1"
            and row["backend"] == "kfd"
            and row["authority"] == "none"
            and row["teardown"] == "explicit",
            "exact diagnostic schema",
        )
        need(
            integer(row["ordinal"]) == ordinal
            and integer(row["backend_submission"], True) == expected_id
            and expected_id <= 168,
            "exact call ordinal and submission ID",
        )
        direction = (expected_id - 7) % 2
        need(
            (row["source_uid"], row["destination_uid"])
            == (selected[direction][2][2:], selected[1 - direction][2][2:]),
            "directional identity",
        )
        call = row["call"]
        need(
            call in (("pending", "completed") if open_call else ("submit",)),
            "submit/pending/completed sequence",
        )
        numeric = {key: integer(row[key]) for key in duration_keys - {"preparation_ns"}}
        if call == "submit":
            numeric["preparation_ns"] = integer(row["preparation_ns"])
        else:
            need(
                row["preparation_ns"] == "not-applicable",
                "poll has no preparation stage",
            )
        stages = sum(value for key, value in numeric.items() if key != "total_ns")
        need(stages <= numeric["total_ns"], "stage sum within call total")
        observations.append(
            {**row, **numeric, "ordinal": ordinal, "backend_submission": expected_id}
        )
        if call == "completed":
            expected_id += 1
            open_call = False
        else:
            open_call = True
    if enabled:
        need(expected_id == 169 and not open_call, "complete 162-submission roster")
    return {"aggregates": aggregates, "observations": observations}


def population(submission):
    need(type(submission) is int and 7 <= submission <= 168, "population ID")
    if submission <= 26:
        phase = "remap-warmup"
    elif submission <= 86:
        phase = "remap-sample"
    elif submission <= 88:
        phase = "prime"
    elif submission <= 108:
        phase = "hot-warmup"
    else:
        phase = "hot-sample"
    return phase, "forward" if submission % 2 else "reverse"


def statistics(values):
    values = sorted(values)
    if not values:
        return {"count": 0}
    if len(values) == 1:
        return {"count": 1, "value": values[0]}
    return {
        "count": len(values),
        "p50": values[(len(values) + 1) // 2 - 1],
        "p95": values[(95 * len(values) + 99) // 100 - 1],
        "sum": sum(values),
    }


def summarize(parsed):
    buckets = {}
    calls = {}
    for row in parsed["observations"]:
        identity = row["backend_submission"]
        calls.setdefault(identity, []).append(row)
    for identity, rows in calls.items():
        phase, direction = population(identity)
        bucket = buckets.setdefault(
            phase + "/" + direction, {"calls": {}, "submissions": {}}
        )
        for row in rows:
            call = bucket["calls"].setdefault(row["call"], {})
            for key in (
                "opening_currentness_ns",
                "preparation_ns",
                "native_call_ns",
                "closing_currentness_ns",
                "total_ns",
            ):
                if type(row[key]) is int:
                    call.setdefault(key, []).append(row[key])
        pending = [row for row in rows if row["call"] == "pending"]
        totals = {
            "pending_count": len(pending),
            "pending_total_ns": sum(row["total_ns"] for row in pending),
            "submit_total_ns": rows[0]["total_ns"],
            "completed_total_ns": rows[-1]["total_ns"],
            "all_calls_total_ns": sum(row["total_ns"] for row in rows),
            "currentness_total_ns": sum(
                row["opening_currentness_ns"] + row["closing_currentness_ns"]
                for row in rows
            ),
            "native_host_total_ns": sum(row["native_call_ns"] for row in rows),
        }
        for key, value in totals.items():
            bucket["submissions"].setdefault(key, []).append(value)
    for bucket in buckets.values():
        bucket["submissions"] = {
            k: statistics(v) for k, v in bucket["submissions"].items()
        }
        bucket["calls"] = {
            call: {k: statistics(v) for k, v in metrics.items()}
            for call, metrics in bucket["calls"].items()
        }
    return buckets


def checked_binding(owned, marker):
    need(sha(owned / "binding.json") == marker["binding_sha256"], "binding identity")
    binding = read(owned / "binding.json")
    need(binding["commit"] == marker["commit"], "signed source identity")
    need(
        binding["devices"] == devices([d[0] for d in binding["devices"]]),
        "known exact endpoint pair",
    )
    need(
        same_json(
            binding["plan"],
            {
                "order": PHASES,
                "bytes": 1048576,
                "depth": 1,
                "warmups": 10,
                "samples": 30,
                "settled_seconds": 2,
                "delayed_seconds": 20,
            },
        ),
        "frozen experiment plan",
    )
    need(set(binding["payload"]) == set(PAYLOAD), "exact payload roster")
    for name, digest in binding["payload"].items():
        need(sha(owned / name) == digest, "payload digest")
    return binding


def run_remote(marker):
    owned = B.owned_path(marker)
    binding = checked_binding(owned, marker)
    need(HERE == owned, "executing bound private runner")
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    rec = B.Recorder(owned / "results", owned)
    source = owned / "source"
    source.mkdir()
    (owned / "tmp").mkdir()
    with tarfile.open(owned / "source.tar.gz", "r:gz") as archive:
        B.validate_members(archive.getmembers(), binding["source_files"])
        archive.extractall(source, filter="data")
    B.source_clean(source, binding)
    B.write_json(rec.output / "source-before.json", B.inventory(source))
    rec.cwd = source
    env, selected = environment(owned), binding["devices"]
    for name, command, seconds in builds():
        rec.run(name, command, seconds, env=env)
    binary_sha = sha(owned / BINARY)
    B.write_json(rec.output / "binary.json", {BINARY: binary_sha})

    def identities():
        checked_binding(owned, marker)
        B.source_clean(source, binding)
        need(sha(owned / BINARY) == binary_sha, "same unchanged binary")

    def observe(label):
        failures = []
        for device in selected:
            try:
                rec.run(
                    label + "-gpu" + str(device[0]),
                    observer_command(device),
                    100,
                    env=env,
                )
            except BaseException as error:
                failures.append(error)
        if failures:
            raise failures[0]

    results, failure = {}, None
    try:
        for name, enabled in PHASES:
            identities()
            observe(name + "-before")
            failure = None
            try:
                folder = rec.run(name, workload(owned, selected, enabled), 120, env=env)
                need((folder / "stderr").read_bytes() == b"", "empty workload stderr")
                results[name] = parse_transcript(
                    (folder / "stdout").read_bytes(), selected, enabled
                )
            except BaseException as error:
                failure = error
            failure = B.settled_postflight(observe, name, failure)
            if failure is not None:
                raise failure
    except BaseException as error:
        failure = error
    finally:
        try:
            identities()
            B.write_json(rec.output / "source-after.json", B.inventory(source))
        except BaseException as error:
            if failure is None:
                failure = error
    if failure is not None:
        raise failure
    B.write_json(rec.output / "parsed.json", results)
    B.write_json(
        rec.output / "finished.json",
        {
            "commit": binding["commit"],
            "native_execution": True,
            "formal_refinement": False,
            "performance_acceptance": False,
        },
    )


def local_commands(payload, marker):
    serialized = json.dumps(marker, sort_keys=True, separators=(",", ":"))
    result = [
        ("calibration", ["python3", "-I", str(HERE / "test_campaign.py")], 60, None),
        (
            "observer-tests",
            ["python3", "-B", "benchmarks/runtime_gfx942/test_copy_host_observe.py"],
            60,
            None,
        ),
        ("cpu-verify", ["python3", "-I", str(CPU / "accept.py"), "--live"], 60, None),
        (
            "signature",
            [
                "git",
                "-c",
                "gpg.ssh.allowedSignersFile=" + SIGNERS,
                "verify-commit",
                marker["commit"],
            ],
            30,
            None,
        ),
        ("source", ["python3", "-I", str(SELECTOR)], 60, None),
        (
            "source-ancestor",
            [
                "git",
                "merge-base",
                "--is-ancestor",
                QUALIFIED_SOURCE_COMMIT,
                marker["commit"],
            ],
            30,
            None,
        ),
        (
            "source-clean",
            [
                "git",
                "status",
                "--porcelain=v1",
                "--untracked-files=all",
                "--",
                *SOURCE_PATHS,
            ],
            30,
            None,
        ),
    ]
    for name, mode in (
        ("create", "create"),
        ("remote-inventory", "inventory"),
        ("cleanup", "cleanup"),
        ("absence", "absence"),
    ):
        result.append(
            (
                name,
                [
                    "ssh",
                    "-T",
                    *SSH,
                    "mi300x",
                    shlex.join(["/usr/bin/python3", "-I", "-", mode, serialized]),
                ],
                45 if mode == "create" else 120,
                control_bytes(),
            )
        )
    result.extend(
        [
            (
                "upload",
                [
                    "scp",
                    *SSH,
                    *(str(payload / name) for name in (*PAYLOAD, "binding.json")),
                    "mi300x:" + marker["path"] + "/",
                ],
                300,
                None,
            ),
            (
                "native",
                [
                    "ssh",
                    "-T",
                    *SSH,
                    "mi300x",
                    shlex.join(
                        [
                            "/usr/bin/python3",
                            "-I",
                            marker["path"] + "/campaign.py",
                            "remote",
                            json.dumps(marker, sort_keys=True, separators=(",", ":")),
                        ]
                    ),
                ],
                5000,
                None,
            ),
            (
                "collect",
                [
                    "scp",
                    "-r",
                    *SSH,
                    "mi300x:" + marker["path"] + "/results",
                    str(HERE / "remote"),
                ],
                300,
                None,
            ),
        ]
    )
    return {name: (command, seconds, stdin) for name, command, seconds, stdin in result}


def run_local(indices):
    selected = devices(indices)
    cpu_integrity()
    need(
        sha(SELECTOR) == SELECTOR_SHA
        and sha(CPU / "SHA256SUMS") == CPU_SEAL
        and sha(SIGNERS) == SIGNERS_SHA,
        "pinned source selector, CPU archive and signer",
    )
    commit = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
    ).strip()
    need(re.fullmatch(r"[0-9a-f]{40}", commit), "canonical launch commit")
    tool_hashes = committed_tools(commit)
    rec = B.Recorder(HERE / "local", ROOT)
    dummy = {"path": PREFIX + "0" * 16, "commit": commit, "binding_sha256": "0" * 64}
    initial = local_commands(Path("/unused"), dummy)
    for name in (
        "calibration",
        "observer-tests",
        "cpu-verify",
        "signature",
        "source",
        "source-ancestor",
        "source-clean",
    ):
        command, seconds, stdin = initial[name]
        rec.run(name, command, seconds, stdin=stdin)
    snapshot = read(HERE / "local/source/stdout")
    need(
        snapshot["base"] == commit
        and snapshot["files"] == read(CPU / "raw/source-before/stdout")["files"],
        "exact CPU-qualified source",
    )
    need(
        (HERE / "local/source-clean/stdout").read_bytes() == b"",
        "committed clean source",
    )
    payload = Path(
        tempfile.mkdtemp(
            prefix="fe2o3-xgmi-attribution-20260918.", dir="/home/harsh/.codex-tmp"
        )
    )
    state = {
        "created": False,
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
        shutil.copyfile(HERE / "campaign.py", payload / "campaign.py")
        shutil.copyfile(BASE_PATH, payload / "base.py")
        with tarfile.open(payload / "source.tar.gz", "w:gz") as archive:
            for name, digest in snapshot["files"].items():
                need(sha(ROOT / name) == digest, "unchanged source before packing")
                archive.add(ROOT / name, arcname=name, recursive=False)
        binding = {
            "schema": "fe2o3.xgmi-host-attribution-experiment.v1",
            "commit": commit,
            "cpu_seal_sha256": CPU_SEAL,
            "source_files": snapshot["files"],
            "payload": {name: sha(payload / name) for name in PAYLOAD},
            "local_tools": tool_hashes,
            "devices": selected,
            "plan": {
                "order": PHASES,
                "bytes": 1048576,
                "depth": 1,
                "warmups": 10,
                "samples": 30,
                "settled_seconds": 2,
                "delayed_seconds": 20,
            },
        }
        B.write_json(payload / "binding.json", binding)
        B.write_json(HERE / "binding.json", binding)
        marker = {
            "commit": commit,
            "path": PREFIX + secrets.token_hex(8),
            "binding_sha256": sha(payload / "binding.json"),
        }
        B.write_json(HERE / "owner.json", marker)
        commands = local_commands(payload, marker)

        def step(name):
            command, seconds, stdin = commands[name]
            return rec.run(name, command, seconds, stdin=stdin)

        step("create")
        state["created"] = True
        step("upload")
        run_and_settle(step, state)
    except BaseException as error:
        failure = error
        state["failure"] = f"{type(error).__name__}: {error}"
    finally:
        failure = finalize_local(payload, state, failure)
    if failure is not None:
        raise failure


def preserve_failure(state, primary, error, stage):
    detail = f"{type(error).__name__}: {error}"
    if primary is None:
        state["failure"] = detail
        return error
    state["secondary_failures"].append({"stage": stage, "error": detail})
    return primary


def run_and_settle(step, state):
    primary = None
    try:
        step("native")
        state["native_success"] = True
    except BaseException as error:
        primary = error
        state["native_failure"] = f"{type(error).__name__}: {error}"
    try:
        collect_and_clean(step, state)
    except BaseException as error:
        primary = preserve_failure(state, primary, error, "remote-settlement")
    if primary is not None:
        raise primary


def finalize_local(payload, state, primary):
    try:
        if state["absence"]:
            shutil.rmtree(payload)
            state["local_payload_absent"] = not payload.exists()
    except BaseException as error:
        primary = preserve_failure(state, primary, error, "local-payload-cleanup")
    try:
        B.write_json(HERE / "controller-state.json", state)
    except BaseException as error:
        primary = preserve_failure(state, primary, error, "state-finalization")
        try:
            print(
                json.dumps({"controller_finalization_failed": state}), file=sys.stderr
            )
        except BaseException:
            pass  # Failed diagnostic output must not replace the primary failure.
    return primary


def collect_and_clean(step, state):
    manifest = read(step("remote-inventory") / "stdout")
    need(manifest, "nonempty collection manifest")
    B.write_json(HERE / "remote-inventory.json", manifest)
    step("collect")
    need(B.inventory(HERE / "remote") == manifest, "complete collection before cleanup")
    state["collected"] = True
    step("cleanup")
    state["cleaned"] = True
    step("absence")
    state["absence"] = True


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
        run_remote(parse_json(args.arguments[0]))


if __name__ == "__main__":
    main()
