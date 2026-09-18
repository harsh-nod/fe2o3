#!/usr/bin/env python3
"""One bounded, owned-directory XGMI diagnostic; never a GPU reservation."""

import hashlib
import json
import os
from pathlib import Path
import re
import resource
import shutil
import signal
import stat
import subprocess
import sys
import tarfile
import time

MANAGED = (signal.SIGHUP, signal.SIGINT, signal.SIGTERM)
PREFIX = "/home/harsh/fe2o3-xgmi-creation-20260918."
DEVICES = [
    (1, "0000:26:00.0", "0xab83d2ffef0d3cdf"),
    (2, "0000:46:00.0", "0xd2e26fef80cf5c33"),
]


def need(value, message):
    if not value:
        raise RuntimeError(message)


def sha(path):
    with Path(path).open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def write_json(path, value):
    with Path(path).open("x") as target:
        json.dump(value, target, indent=2, sort_keys=True)
        target.write("\n")


def group_exists(pid):
    try:
        os.killpg(pid, 0)
        return True
    except ProcessLookupError:
        return False


def stop(process):
    for sig in (signal.SIGTERM, signal.SIGKILL):
        if group_exists(process.pid):
            os.killpg(process.pid, sig)
        end = time.monotonic() + 5
        while group_exists(process.pid) and time.monotonic() < end:
            process.poll()
            time.sleep(0.05)
    process.wait(timeout=5)
    need(not group_exists(process.pid), "owned process group remains")


def interrupted(number, _frame):
    for managed in MANAGED:
        signal.signal(managed, signal.SIG_IGN)
    raise RuntimeError(f"interrupted by signal {number}")


class Recorder:
    def __init__(self, output, cwd):
        self.output, self.cwd = Path(output), Path(cwd)
        self.output.mkdir()

    def run(self, name, command, seconds, *, stdin=None, env=None):
        folder = self.output / name
        folder.mkdir()
        row = {
            "command": command,
            "cwd": str(self.cwd),
            "started_ns": time.time_ns(),
            "timeout_seconds": seconds,
            "pid": None,
            "exit": None,
            "error": None,
            "group_absent": False,
            "environment": env,
            "stdin_sha256": hashlib.sha256(stdin).hexdigest()
            if stdin is not None
            else None,
        }
        process = None
        try:
            with (
                (folder / "stdout").open("xb") as out,
                (folder / "stderr").open("xb") as err,
            ):
                blocked = signal.pthread_sigmask(signal.SIG_BLOCK, MANAGED)
                try:
                    process = subprocess.Popen(
                        command,
                        cwd=self.cwd,
                        env=env,
                        start_new_session=True,
                        stdin=subprocess.PIPE
                        if stdin is not None
                        else subprocess.DEVNULL,
                        stdout=out,
                        stderr=err,
                        preexec_fn=lambda: signal.pthread_sigmask(
                            signal.SIG_SETMASK, blocked
                        ),
                    )
                    row["pid"] = process.pid
                finally:
                    signal.pthread_sigmask(signal.SIG_SETMASK, blocked)
                process.communicate(input=stdin, timeout=seconds)
                row["exit"] = process.returncode
                need(not group_exists(process.pid), "owned descendants remain")
                row["group_absent"] = True
        except BaseException as error:
            row["error"] = f"{type(error).__name__}: {error}"
            if process is not None:
                try:
                    stop(process)
                    row["group_absent"] = True
                except BaseException as cleanup:
                    row["error"] += f"; cleanup: {cleanup}"
                row["exit"] = process.returncode
        row["finished_ns"] = time.time_ns()
        row["stdout_sha256"], row["stderr_sha256"] = (
            sha(folder / "stdout"),
            sha(folder / "stderr"),
        )
        write_json(folder / "receipt.json", row)
        print(
            json.dumps(
                {
                    "phase": name,
                    "exit": row["exit"],
                    "error": row["error"],
                    "group_absent": row["group_absent"],
                }
            ),
            flush=True,
        )
        need(
            row["exit"] == 0 and row["error"] is None and row["group_absent"],
            name + " failed",
        )
        return folder


def owned_path(marker, exists=True):
    need(
        set(marker) == {"path", "commit", "binding_sha256"},
        "exact ownership marker keys",
    )
    path = Path(marker["path"])
    need(
        re.fullmatch(re.escape(PREFIX) + r"[0-9a-f]{16}", str(path)), "owned path shape"
    )
    need(re.fullmatch(r"[0-9a-f]{40}", marker["commit"]), "commit identity")
    need(re.fullmatch(r"[0-9a-f]{64}", marker["binding_sha256"]), "binding identity")
    if exists:
        info = path.lstat()
        need(
            stat.S_ISDIR(info.st_mode)
            and info.st_uid == os.getuid()
            and stat.S_IMODE(info.st_mode) == 0o700
            and path.resolve() == path,
            "private owned directory",
        )
        need(
            json.loads((path / "owner.json").read_text()) == marker,
            "exact ownership marker",
        )
    return path


def inventory(root):
    files = {}
    for path in sorted(root.rglob("*")):
        need(not path.is_symlink(), "no source symlinks")
        if path.is_dir():
            continue
        need(path.is_file(), "ordinary source files")
        files[path.relative_to(root).as_posix()] = sha(path)
    return files


def validate_members(members, expected):
    names = []
    for member in members:
        name = member.name
        need(
            member.isfile()
            and not name.startswith("/")
            and all(part not in ("", ".", "..") for part in name.split("/")),
            "ordinary relative archive member",
        )
        names.append(name)
    need(
        len(names) == len(set(names)) and set(names) == set(expected),
        "exact archive roster",
    )


def source_clean(source, binding):
    need(
        inventory(source) == binding["source_files"], "exact unchanged source inventory"
    )


def payload_clean(owned, marker):
    need(sha(owned / "binding.json") == marker["binding_sha256"], "binding digest")
    binding = json.loads((owned / "binding.json").read_text())
    need(binding["commit"] == marker["commit"], "bound commit")
    for name, digest in binding["payload"].items():
        need(
            name in ("native.py", "source.tar.gz") and sha(owned / name) == digest,
            "payload identity",
        )
    need(set(binding["payload"]) == {"native.py", "source.tar.gz"}, "payload roster")
    return binding


def expected_fields(backend, depth, measurement=None):
    fields = {
        "backend": backend,
        "schema": "fe2o3.xgmi-peer-benchmark.v1",
        "unique_ids": ",".join(device[2][2:] for device in DEVICES),
        "bytes": "1048576",
        "depth": str(depth),
        "warmups": "10",
        "samples": "30",
    }
    if backend == "kfd":
        fields.update(
            {
                "surface": "runtime-facade",
                "target": "gfx942:xnack-",
                "queue_depth": str(depth),
                "batch_size": str(depth),
                "direction": "forward-then-reverse",
                "outstanding_depth": str(depth),
                "engine_parallelism": "ordered-single-sdma",
                "measurement": measurement,
                "peer_access": "topology-xgmi",
                "doorbells_per_batch": "1",
                "progress": "explicit-flush-then-wait",
                "background_progress": "false",
                "forward_engine": "topology-selected",
                "reverse_engine": "topology-selected",
                "canaries": "pass",
                "teardown": "explicit",
                "timing": "facade-enqueue-flush-through-observed-completion",
            }
        )
        fields["mapping_lifetime"] = (
            "host-access-between-rounds"
            if measurement == "remap-per-round"
            else "persistent-no-host-access-between-timed-rounds"
        )
        fields["prime_batches"] = "0" if measurement == "remap-per-round" else "1"
    elif backend == "hsa":
        fields.update({"gpu_indices": "0,1", "xnack": "disabled"})
    elif backend == "hip":
        fields.update({"devices": "0,1", "peer_access": "enabled"})
    else:
        raise ValueError("unknown backend")
    return fields


def parse_results(data, backend, depth):
    need(
        isinstance(data, bytes)
        and data.endswith(b"\n")
        and b"\r" not in data
        and b"\0" not in data,
        "complete ordinary result transcript",
    )
    lines = data.decode("ascii").splitlines()
    measurements = ["remap-per-round", "persistent-hot"] if backend == "kfd" else [None]
    need(len(lines) == len(measurements), "exact result row count")
    metrics = {
        direction + suffix
        for direction in ("forward", "reverse")
        for suffix in ("_p50_ns", "_p95_ns", "_p50_GBps")
    }
    rows = []
    for line, measurement in zip(lines, measurements):
        pairs = [field.split("=") for field in line.split(" ")]
        need(
            all(len(pair) == 2 and all(pair) for pair in pairs),
            "result key-value fields",
        )
        row = dict(pairs)
        need(len(row) == len(pairs), "no duplicate result keys")
        fixed = expected_fields(backend, depth, measurement)
        need(
            set(row)
            == set(fixed) | metrics | ({"targets"} if backend != "kfd" else set()),
            "exact result keys",
        )
        need(
            all(row[key] == value for key, value in fixed.items()),
            "exact result controls and identities",
        )
        if backend != "kfd":
            targets = row["targets"].split(",")
            need(
                len(targets) == 2
                and all(
                    re.fullmatch(r"gfx942(?::sramecc[+-])?(?::xnack-)?", target)
                    for target in targets
                ),
                "gfx942 targets",
            )
            if backend == "hip":
                need(
                    all(target.endswith(":xnack-") for target in targets),
                    "HIP xnack disabled",
                )
        for direction in ("forward", "reverse"):
            median, upper, bandwidth = (
                row[direction + suffix]
                for suffix in ("_p50_ns", "_p95_ns", "_p50_GBps")
            )
            need(
                re.fullmatch(r"[1-9][0-9]{0,19}", median)
                and re.fullmatch(r"[1-9][0-9]{0,19}", upper),
                "positive bounded latency",
            )
            need(int(upper) >= int(median), "ordered latency quantiles")
            need(
                re.fullmatch(r"(?:0|[1-9][0-9]*)\.[0-9]{3}", bandwidth),
                "finite rounded bandwidth",
            )
            need(
                abs(float(bandwidth) - 1048576 * depth / int(median)) <= 0.00050001,
                "bandwidth agrees with latency",
            )
        rows.append(row)
    return rows


def run_native(marker):
    owned = owned_path(marker)
    binding = payload_clean(owned, marker)
    need(
        Path(__file__).resolve() == owned / "native.py"
        and sha(__file__) == binding["payload"]["native.py"],
        "executing the bound owned runner",
    )
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    rec = Recorder(owned / "results", owned)
    source = owned / "source"
    source.mkdir()
    with tarfile.open(owned / "source.tar.gz", "r:gz") as archive:
        validate_members(archive.getmembers(), binding["source_files"])
        archive.extractall(source, filter="data")
    source_clean(source, binding)
    write_json(rec.output / "source-before.json", inventory(source))
    (owned / "tmp").mkdir()
    env = {
        "HOME": str(Path.home()),
        "USER": os.environ.get("USER", "harsh"),
        "PATH": str(Path.home() / ".cargo/bin") + ":/opt/rocm/bin:/usr/bin:/bin",
        "LANG": "C",
        "LC_ALL": "C",
        "CARGO_TARGET_DIR": str(owned / "target"),
        "CARGO_INCREMENTAL": "0",
        "CARGO_BUILD_JOBS": "2",
        "CARGO_TERM_COLOR": "never",
        "RUSTUP_TOOLCHAIN": "nightly-2026-04-03",
        "TMPDIR": str(owned / "tmp"),
    }
    rec.cwd = source
    for name, command in [
        ("rustc", ["rustc", "-vV"]),
        ("cargo", ["cargo", "-V"]),
        ("hipcc", ["/opt/rocm/bin/hipcc", "--version"]),
        ("g++", ["g++", "--version"]),
        ("rocm", ["cat", "/opt/rocm/.info/version"]),
    ]:
        rec.run(name, command, 30, env=env)
    rec.run(
        "build-kfd",
        [
            "cargo",
            "build",
            "--frozen",
            "--release",
            "-p",
            "fe2o3-runtime",
            "--example",
            "gfx942-runtime-xgmi-peer-benchmark",
        ],
        1200,
        env=env,
    )
    hip, hsa = owned / "xgmi-peer-hip", owned / "xgmi-peer-hsa"
    rec.run(
        "build-hip",
        [
            "/opt/rocm/bin/hipcc",
            "-std=c++17",
            "-O3",
            "-Wall",
            "-Wextra",
            "-Werror",
            "benchmarks/runtime_gfx942/xgmi_peer_hip.cpp",
            "-o",
            str(hip),
        ],
        180,
        env=env,
    )
    rec.run(
        "build-hsa",
        [
            "g++",
            "-std=c++17",
            "-O3",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-I/opt/rocm/include",
            "benchmarks/runtime_gfx942/xgmi_peer_hsa.cpp",
            "-L/opt/rocm/lib",
            "-Wl,-rpath,/opt/rocm/lib",
            "-lhsa-runtime64",
            "-o",
            str(hsa),
        ],
        180,
        env=env,
    )
    kfd = owned / "target/release/examples/gfx942-runtime-xgmi-peer-benchmark"
    binaries = {str(path.relative_to(owned)): sha(path) for path in (kfd, hip, hsa)}
    write_json(rec.output / "binaries.json", binaries)

    def check_identities():
        payload_clean(owned, marker)
        source_clean(source, binding)
        need(
            all(sha(owned / name) == digest for name, digest in binaries.items()),
            "unchanged binaries",
        )

    def observe(label):
        failures = []
        for index, bdf, uid in DEVICES:
            try:
                rec.run(
                    label + "-gpu" + str(index),
                    [
                        "/usr/bin/python3",
                        "-I",
                        "benchmarks/runtime_gfx942/copy-host-observe.py",
                        "--gpu-index",
                        str(index),
                        "--pci-bdf",
                        bdf,
                        "--unique-id",
                        uid,
                    ],
                    100,
                    env=env,
                )
            except BaseException as error:
                failures.append(error)
        if failures:
            raise failures[0]

    results = []
    try:
        for depth in (1, 16):
            arguments = ["1048576", str(depth), "10", "30"]
            uids = [device[2] for device in DEVICES]
            for backend in ("kfd", "hsa", "hip"):
                name = backend + "-d" + str(depth)
                check_identities()
                observe(name + "-before")
                command = [str(kfd), *uids, *arguments]
                phase_env = dict(env)
                if backend != "kfd":
                    phase_env["HSA_XNACK"] = "0"
                    phase_env[
                        "ROCR_VISIBLE_DEVICES"
                        if backend == "hsa"
                        else "HIP_VISIBLE_DEVICES"
                    ] = "1,2"
                    command = [
                        str(hsa if backend == "hsa" else hip),
                        "0",
                        "1",
                        *arguments,
                        *uids,
                    ]
                failure = None
                try:
                    folder = rec.run(name, command, 120, env=phase_env)
                    need(
                        (folder / "stderr").read_bytes() == b"",
                        "no benchmark diagnostics",
                    )
                    results.extend(
                        parse_results((folder / "stdout").read_bytes(), backend, depth)
                    )
                except BaseException as error:
                    failure = error
                for suffix in ("-after", "-delayed"):
                    if suffix == "-delayed":
                        time.sleep(20)
                    try:
                        observe(name + suffix)
                    except BaseException as error:
                        if failure is None:
                            failure = error
                if failure is not None:
                    raise failure
    finally:
        check_identities()
        write_json(rec.output / "source-after.json", inventory(source))
    need(len(results) == 8, "complete eight-row result roster")
    write_json(rec.output / "validated-results.json", results)
    write_json(
        rec.output / "finished.json",
        {
            "exit": 0,
            "commit": binding["commit"],
            "native_execution": True,
            "performance_acceptance": False,
            "formal_refinement": False,
        },
    )


def assert_no_owned_processes(path):
    found = []
    needle = str(path).encode()
    for entry in Path("/proc").iterdir():
        if not entry.name.isdecimal() or int(entry.name) in (os.getpid(), os.getppid()):
            continue
        try:
            command = (entry / "cmdline").read_bytes()
            cwd = (entry / "cwd").resolve(strict=True)
        except (FileNotFoundError, PermissionError, ProcessLookupError):
            continue
        if needle in command or cwd == path or path in cwd.parents:
            found.append(int(entry.name))
    need(not found, f"owned processes still present: {found}")


def main():
    mode, serialized = sys.argv[1:]
    marker = json.loads(serialized)
    os.umask(0o077)
    for number in MANAGED:
        signal.signal(number, interrupted)
    if mode == "create":
        owned = owned_path(marker, exists=False)
        space = os.statvfs(owned.parent)
        need(
            space.f_bavail * space.f_frsize >= 4 * 1024**3,
            "four GiB of build space required",
        )
        owned.mkdir(mode=0o700)
        write_json(owned / "owner.json", marker)
        print(json.dumps(marker, sort_keys=True))
    elif mode == "run":
        run_native(marker)
    elif mode == "inventory":
        owned = owned_path(marker)
        assert_no_owned_processes(owned)
        print(json.dumps(inventory(owned / "results"), sort_keys=True))
    elif mode == "cleanup":
        owned = owned_path(marker)
        assert_no_owned_processes(owned)
        for receipt in (owned / "results").glob("*/receipt.json"):
            row = json.loads(receipt.read_text())
            if row["pid"] is not None:
                need(
                    not group_exists(row["pid"]),
                    "recorded remote process group remains",
                )
        shutil.rmtree(owned)
        need(not owned.exists(), "owned directory removed")
        print(json.dumps({"removed": str(owned)}))
    elif mode == "absence":
        owned = owned_path(marker, exists=False)
        need(not owned.exists() and not owned.is_symlink(), "owned path absent")
        assert_no_owned_processes(owned)
        print(json.dumps({"path_absent": True, "processes_absent": True}))
    else:
        raise ValueError("unknown mode")


if __name__ == "__main__":
    main()
