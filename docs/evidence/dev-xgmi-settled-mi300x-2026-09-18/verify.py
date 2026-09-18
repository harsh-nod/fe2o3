#!/usr/bin/env python3
"""Offline acceptance of a bounded native diagnostic, never a parity verdict."""

import argparse
import importlib.util
import json
from pathlib import Path
import re
import shlex
import sys

sys.dont_write_bytecode = True
import native as N  # noqa: E402
import controller as C  # noqa: E402

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
ORIGINAL_ROOT = Path("/home/harsh/.codex-tmp/fe2o3-c4-completion-20260917")
ORIGINAL_HERE = ORIGINAL_ROOT / HERE.relative_to(ROOT)
PRIOR = (
    ROOT
    / "docs/evidence/dev-logical-mux-sdma-release-native-gpu1-2026-09-18/raw/native/collected"
)


def load_module(path, digest, name):
    N.need(N.sha(path) == digest, "pinned read-only parser")
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def endpoint(value, index, bdf, uid):
    protocol = load_module(
        PRIOR / "protocol.py",
        "237ae64cecea6d64c8722813c89e6660008f49bc5d334031165ded88aae3a5a1",
        "pinned_endpoint_protocol",
    )
    protocol.GPU, protocol.BDF, protocol.UID = index, bdf, uid
    protocol.endpoint(value)
    observer = load_module(
        protocol.OBSERVER_SOURCE, protocol.OBSERVER_SHA, "pinned_observer"
    )
    captures = iter([value["status"], value["pids"]])
    snapshots = iter(value["sysfs"])
    stamps = iter([value["started"], value["finished"]])
    for snapshot in value["sysfs"]:
        N.need(
            set(snapshot) == {"started", "finished", "path", "values", "errors"}
            and set(snapshot["values"]) == set(observer.METRICS),
            "exact raw sysfs keys",
        )
    for capture in (value["status"], value["pids"]):
        N.need(
            set(capture)
            == {"command", "started", "finished", "exit", "error", "stdout", "stderr"},
            "exact raw capture keys",
        )
    observer.capture_sysfs = lambda _root, _bdf: next(snapshots)
    observer.stamp = lambda: next(stamps)
    reconstructed = observer.observe(
        index,
        bdf,
        uid,
        Path("/opt/rocm/bin/rocm-smi"),
        run=lambda _command: next(captures),
    )
    reconstructed["index"] = 0
    N.need(
        reconstructed == value and reconstructed["endpoint_admitted"] is True,
        "raw endpoint admission replay",
    )


def unique(pairs):
    result = {}
    for key, value in pairs:
        N.need(key not in result, "duplicate JSON key")
        result[key] = value
    return result


def read(path):
    return json.loads(path.read_text(), object_pairs_hook=unique)


def receipt(folder, command=None, stdin=None):
    N.need(
        {path.name for path in folder.iterdir()}
        == {"receipt.json", "stdout", "stderr"},
        "exact receipt files",
    )
    row = read(folder / "receipt.json")
    N.need(
        set(row)
        == {
            "command",
            "cwd",
            "started_ns",
            "finished_ns",
            "timeout_seconds",
            "pid",
            "exit",
            "error",
            "group_absent",
            "environment",
            "stdin_sha256",
            "stdout_sha256",
            "stderr_sha256",
        },
        "exact receipt keys",
    )
    N.need(
        type(row["exit"]) is int
        and row["exit"] == 0
        and row["error"] is None
        and row["group_absent"] is True,
        "successful closed process",
    )
    N.need(type(row["pid"]) is int and row["pid"] > 0, "positive process identity")
    N.need(
        type(row["started_ns"]) is int
        and type(row["finished_ns"]) is int
        and 0 < row["started_ns"] <= row["finished_ns"],
        "receipt chronology",
    )
    N.need(
        row["stdout_sha256"] == N.sha(folder / "stdout")
        and row["stderr_sha256"] == N.sha(folder / "stderr"),
        "receipt output digests",
    )
    if command is not None:
        N.need(row["command"] == command, "exact receipt command")
    N.need(row["stdin_sha256"] == stdin, "exact receipt input")
    return row


def postflight_timing(rows, name):
    N.need(
        rows[name + "-settled-gpu1"]["started_ns"] - rows[name]["finished_ns"]
        >= 2 * 10**9,
        "settled postflight not earlier than two seconds after process closure",
    )
    N.need(
        rows[name + "-delayed-gpu1"]["started_ns"]
        - rows[name + "-settled-gpu2"]["finished_ns"]
        >= 20 * 10**9,
        "delayed postflight at least twenty seconds after settled endpoints complete",
    )


def verify():
    expected = {
        "native.py",
        "controller.py",
        "test_native.py",
        "verify.py",
        "README.md",
        ".gitattributes",
        "binding.json",
        "owner.json",
        "remote-inventory.json",
        "controller-state.json",
        "local",
        "remote",
    }
    N.need(
        {path.name for path in HERE.iterdir()} - {"SHA256SUMS"} == expected,
        "exact top-level archive membership",
    )
    binding, marker = read(HERE / "binding.json"), read(HERE / "owner.json")
    N.need(
        set(binding)
        == {
            "schema",
            "commit",
            "cpu_seal_sha256",
            "source_files",
            "payload",
            "local_tools",
            "devices",
            "bytes",
            "depths",
            "settled_delay_seconds",
            "delayed_delay_seconds",
            "delayed_anchor",
            "warmups",
            "samples",
            "performance_acceptance",
            "formal_refinement",
        }
        and binding["schema"] == "fe2o3.xgmi-settled-native-development.v1",
        "exact binding schema and keys",
    )
    owned = N.owned_path(marker, exists=False)
    N.need(
        marker["commit"] == binding["commit"]
        and marker["binding_sha256"] == N.sha(HERE / "binding.json"),
        "bound ownership identity",
    )
    N.need(
        binding["payload"]["native.py"] == N.sha(HERE / "native.py"),
        "bound executed runner",
    )
    N.need(
        binding["local_tools"]
        == {
            name: N.sha(HERE / name)
            for name in ("controller.py", "native.py", "test_native.py", "verify.py")
        },
        "unchanged local control and verification tools",
    )
    N.need(
        set(binding["payload"]) == {"native.py", "source.tar.gz"}
        and all(
            re.fullmatch(r"[0-9a-f]{64}", value)
            for value in binding["payload"].values()
        ),
        "payload roster",
    )
    N.need(
        binding["devices"] == [list(device) for device in N.DEVICES]
        and (
            binding["bytes"],
            binding["depths"],
            binding["warmups"],
            binding["samples"],
        )
        == (1048576, [1], 10, 30)
        and binding["settled_delay_seconds"] == 2
        and binding["delayed_delay_seconds"] == 20
        and binding["delayed_anchor"] == "settled-endpoints-complete"
        and binding["performance_acceptance"] is False
        and binding["formal_refinement"] is False,
        "bounded diagnostic controls",
    )
    cpu = ROOT / "docs/evidence/dev-xgmi-creation-root-cpu-2026-09-18"
    N.need(
        binding["cpu_seal_sha256"] == N.sha(cpu / "SHA256SUMS")
        and binding["source_files"] == read(cpu / "raw/source-before.log")["files"],
        "qualified CPU source cohort",
    )
    cpu_manifest = "".join(
        digest + "  " + name + "\n"
        for name, digest in N.inventory(cpu).items()
        if name != "SHA256SUMS"
    )
    N.need(
        (cpu / "SHA256SUMS").read_text() == cpu_manifest,
        "complete sibling CPU archive integrity",
    )
    state = read(HERE / "controller-state.json")
    N.need(
        all(
            state.get(key) is True
            for key in (
                "created",
                "native_success",
                "collected",
                "cleaned",
                "absence",
                "local_payload_absent",
            )
        )
        and state["failure"] is None
        and state["native_failure"] is None,
        "complete collection and cleanup",
    )
    remote_manifest = read(HERE / "remote-inventory.json")
    N.need(
        N.inventory(HERE / "remote") == remote_manifest, "collected inventory identity"
    )
    serialized = json.dumps(marker, sort_keys=True, separators=(",", ":"))
    local_names = [
        "calibration",
        "observer-tests",
        "cpu-verify",
        "signature",
        "source-clean",
        "create",
        "upload",
        "native",
        "remote-inventory",
        "collect",
        "cleanup",
        "absence",
    ]
    N.need(
        {path.name for path in (HERE / "local").iterdir()} == set(local_names),
        "exact local receipt roster",
    )
    last = 0
    local = {}
    local_bounds = {
        "signature": 30,
        "create": 45,
        "native": 2500,
        "remote-inventory": 120,
        "cleanup": 120,
        "absence": 120,
    }
    payload = Path(state["local_payload"])
    original_cpu = ORIGINAL_ROOT / "docs/evidence/dev-xgmi-creation-root-cpu-2026-09-18"
    local_commands = {
        "calibration": (["python3", "-B", str(ORIGINAL_HERE / "test_native.py")], 60),
        "observer-tests": (
            ["python3", "-B", "benchmarks/runtime_gfx942/test_copy_host_observe.py"],
            60,
        ),
        "cpu-verify": (
            ["python3", "-B", str(original_cpu / "verify.py"), "--live"],
            60,
        ),
        "source-clean": (
            [
                "git",
                "status",
                "--porcelain=v1",
                "--untracked-files=all",
                "--",
                "Cargo.toml",
                "Cargo.lock",
                "rust-toolchain.toml",
                ".cargo",
                "crates",
                "examples",
                "benchmarks/runtime_gfx942",
                "scripts/unsafe-source-baseline.json",
                "docs/runtime-primary-queue-release-v1.md",
            ],
            30,
        ),
        "upload": (
            [
                "scp",
                *C.SSH,
                *(
                    str(payload / name)
                    for name in ("native.py", "source.tar.gz", "binding.json")
                ),
                "mi300x:" + str(owned) + "/",
            ],
            300,
        ),
        "collect": (
            [
                "scp",
                "-r",
                *C.SSH,
                "mi300x:" + str(owned / "results"),
                str(ORIGINAL_HERE / "remote"),
            ],
            300,
        ),
    }
    for name in local_names:
        stdin = (
            binding["payload"]["native.py"]
            if name in ("create", "remote-inventory", "cleanup", "absence")
            else None
        )
        row = receipt(HERE / "local" / name, stdin=stdin)
        N.need(
            last <= row["started_ns"] and row["cwd"] == str(ORIGINAL_ROOT),
            "local chronology and directory",
        )
        last = row["finished_ns"]
        local[name] = row
        if name in local_bounds:
            N.need(
                row["timeout_seconds"] == local_bounds[name],
                "exact local control bound",
            )
        N.need(row["environment"] is None, "inherited local command environment")
        if name in local_commands:
            command, bound = local_commands[name]
            N.need(
                row["command"] == command and row["timeout_seconds"] == bound,
                "exact local qualification command",
            )
    for name, count in (("calibration", 12), ("observer-tests", 19)):
        N.need(
            re.fullmatch(
                r"\.{"
                + str(count)
                + r"}\n-+\nRan "
                + str(count)
                + r" tests in [0-9.]+s\n\nOK\n",
                (HERE / "local" / name / "stderr").read_text(),
            ),
            "complete CPU calibration outcomes",
        )
    N.need(
        local["signature"]["command"]
        == [
            "git",
            "-c",
            "gpg.ssh.allowedSignersFile=" + C.SIGNERS,
            "verify-commit",
            binding["commit"],
        ],
        "exact signed commit checked",
    )
    N.need(
        (HERE / "local/source-clean/stdout").read_bytes() == b"",
        "committed clean source",
    )
    N.need(
        local["native"]["command"]
        == [
            "ssh",
            "-T",
            *C.SSH,
            "mi300x",
            shlex.join(
                ["/usr/bin/python3", "-I", str(owned / "native.py"), "run", serialized]
            ),
        ],
        "exact owned runner launch",
    )
    for name, mode in (
        ("create", "create"),
        ("remote-inventory", "inventory"),
        ("cleanup", "cleanup"),
        ("absence", "absence"),
    ):
        N.need(
            local[name]["command"]
            == [
                "ssh",
                "-T",
                *C.SSH,
                "mi300x",
                shlex.join(["/usr/bin/python3", "-B", "-", mode, serialized]),
            ],
            "exact ownership control command",
        )
    N.need(
        read(HERE / "local/create/stdout") == marker
        and read(HERE / "local/remote-inventory/stdout") == remote_manifest
        and read(HERE / "local/cleanup/stdout") == {"removed": str(owned)}
        and read(HERE / "local/absence/stdout")
        == {"path_absent": True, "processes_absent": True},
        "ownership control outcomes",
    )
    remote = HERE / "remote"
    N.need(
        read(remote / "source-before.json")
        == binding["source_files"]
        == read(remote / "source-after.json"),
        "unchanged remote source",
    )
    binaries = read(remote / "binaries.json")
    N.need(
        set(binaries)
        == {
            "target/release/examples/gfx942-runtime-xgmi-peer-benchmark",
            "xgmi-peer-hip",
            "xgmi-peer-hsa",
        }
        and all(re.fullmatch(r"[0-9a-f]{64}", digest) for digest in binaries.values()),
        "three bound binary identities",
    )
    names = [
        "rustc",
        "cargo",
        "hipcc",
        "g++",
        "rocm",
        "build-kfd",
        "build-hip",
        "build-hsa",
    ]
    for depth in (1,):
        for backend in ("kfd", "hsa", "hip"):
            phase = backend + "-d" + str(depth)
            names.extend(
                phase + "-before-gpu" + str(index) for index, _, _ in N.DEVICES
            )
            names.append(phase)
            for suffix in ("settled", "delayed"):
                names.extend(
                    phase + "-" + suffix + "-gpu" + str(index)
                    for index, _, _ in N.DEVICES
                )
    extra = {
        "source-before.json",
        "source-after.json",
        "binaries.json",
        "validated-results.json",
        "finished.json",
    }
    N.need(
        {path.name for path in remote.iterdir()} == set(names) | extra,
        "exact remote evidence roster",
    )
    # Host clocks need not agree; ordering is checked within each clock domain.
    last = 0
    remote_rows = {}
    base_env = {
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
    build_commands = {
        "rustc": (["rustc", "-vV"], 30),
        "cargo": (["cargo", "-V"], 30),
        "hipcc": (["/opt/rocm/bin/hipcc", "--version"], 30),
        "g++": (["g++", "--version"], 30),
        "rocm": (["cat", "/opt/rocm/.info/version"], 30),
        "build-kfd": (
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
        ),
        "build-hip": (
            [
                "/opt/rocm/bin/hipcc",
                "-std=c++17",
                "-O3",
                "-Wall",
                "-Wextra",
                "-Werror",
                "benchmarks/runtime_gfx942/xgmi_peer_hip.cpp",
                "-o",
                str(owned / "xgmi-peer-hip"),
            ],
            180,
        ),
        "build-hsa": (
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
                str(owned / "xgmi-peer-hsa"),
            ],
            180,
        ),
    }
    for name in names:
        row = receipt(remote / name)
        N.need(
            last <= row["started_ns"] <= row["finished_ns"],
            "remote chronology",
        )
        N.need(row["cwd"] == str(owned / "source"), "remote source directory")
        last = row["finished_ns"]
        remote_rows[name] = row
        expected_env = dict(base_env)
        if name in ("hsa-d1", "hsa-d16", "hip-d1", "hip-d16"):
            expected_env["HSA_XNACK"] = "0"
            expected_env[
                "ROCR_VISIBLE_DEVICES"
                if name.startswith("hsa-")
                else "HIP_VISIBLE_DEVICES"
            ] = "1,2"
        N.need(
            row["environment"] == expected_env,
            "exact bounded build and workload environment",
        )
        if name in build_commands:
            command, bound = build_commands[name]
            N.need(
                row["command"] == command and row["timeout_seconds"] == bound,
                "exact version and build command",
            )
    results = []
    for depth in (1,):
        for backend in ("kfd", "hsa", "hip"):
            name = backend + "-d" + str(depth)
            row = remote_rows[name]
            uids = [device[2] for device in N.DEVICES]
            args = ["1048576", str(depth), "10", "30"]
            if backend == "kfd":
                command = [
                    str(
                        owned
                        / "target/release/examples/gfx942-runtime-xgmi-peer-benchmark"
                    ),
                    *uids,
                    *args,
                ]
            else:
                command = [
                    str(owned / ("xgmi-peer-" + backend)),
                    "0",
                    "1",
                    *args,
                    *uids,
                ]
            N.need(
                row["command"] == command and row["timeout_seconds"] == 120,
                "exact benchmark invocation",
            )
            env = row["environment"]
            masks = {
                key: env[key]
                for key in (
                    "HIP_VISIBLE_DEVICES",
                    "ROCR_VISIBLE_DEVICES",
                    "CUDA_VISIBLE_DEVICES",
                    "GPU_DEVICE_ORDINAL",
                )
                if key in env
            }
            expected_masks = (
                {}
                if backend == "kfd"
                else {
                    "ROCR_VISIBLE_DEVICES"
                    if backend == "hsa"
                    else "HIP_VISIBLE_DEVICES": "1,2"
                }
            )
            N.need(
                masks == expected_masks
                and (backend == "kfd" or env["HSA_XNACK"] == "0"),
                "exact physical identity masks",
            )
            N.need(
                (remote / name / "stderr").read_bytes() == b"",
                "no benchmark diagnostics",
            )
            results.extend(
                N.parse_results((remote / name / "stdout").read_bytes(), backend, depth)
            )
            for suffix in ("before", "settled", "delayed"):
                for index, bdf, uid in N.DEVICES:
                    guard_name = name + "-" + suffix + "-gpu" + str(index)
                    guard = remote_rows[guard_name]
                    N.need(guard["timeout_seconds"] == 100, "exact observer bound")
                    N.need(
                        guard["command"]
                        == [
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
                        "exact observer invocation",
                    )
                    lines = (remote / guard_name / "stdout").read_text().splitlines()
                    N.need(len(lines) == 2, "complete observer transcript")
                    observation, complete = [
                        json.loads(line, object_pairs_hook=unique) for line in lines
                    ]
                    endpoint(observation, index, bdf, uid)
                    N.need(
                        observation["schema"] == "fe2o3.copy-host-observation.v1"
                        and observation["record"] == "observation"
                        and (
                            observation["gpu_index"],
                            observation["pci_bdf"],
                            observation["unique_id"],
                        )
                        == (index, bdf, uid)
                        and observation["endpoint_admitted"] is True
                        and observation["selected_pids"] == []
                        and observation["reasons"] == [],
                        "observed free selected endpoint",
                    )
                    N.need(
                        complete
                        == {
                            "schema": "fe2o3.copy-host-observation.v1",
                            "record": "complete",
                            "observations": 1,
                            "refused": 0,
                            "all_endpoints_admitted": True,
                            "performance_accepted": False,
                        },
                        "observer completion",
                    )
            postflight_timing(remote_rows, name)
    N.need(
        results == read(remote / "validated-results.json") and len(results) == 4,
        "exact validated result roster",
    )
    N.need(
        read(remote / "finished.json")
        == {
            "exit": 0,
            "commit": binding["commit"],
            "native_execution": True,
            "performance_acceptance": False,
            "formal_refinement": False,
        },
        "bounded native completion",
    )
    summary = []
    for depth in (1,):
        rows = {
            row["backend"]: row
            for row in results
            if row["depth"] == str(depth)
            and row.get("measurement", "persistent-hot") == "persistent-hot"
        }
        for direction in ("forward", "reverse"):
            times = {
                backend: int(row[direction + "_p50_ns"])
                for backend, row in rows.items()
            }
            summary.append(
                {
                    "depth": depth,
                    "direction": direction,
                    "p50_ns": times,
                    "kfd_latency_over_hsa": times["kfd"] / times["hsa"],
                    "kfd_latency_over_hip": times["kfd"] / times["hip"],
                }
            )
    return {
        "commit": binding["commit"],
        "native_correctness": True,
        "cleanup_closed": True,
        "performance_acceptance": False,
        "formal_refinement": False,
        "diagnostic_timings": summary,
    }


def seal(create):
    manifest = N.inventory(HERE)
    manifest.pop("SHA256SUMS", None)
    text = "".join(digest + "  " + name + "\n" for name, digest in manifest.items())
    path = HERE / "SHA256SUMS"
    if create:
        with path.open("x") as target:
            target.write(text)
    else:
        N.need(path.read_text() == text, "complete archive seal")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--seal", action="store_true")
    mode.add_argument("--allow-unsealed", action="store_true")
    args = parser.parse_args()
    report = verify()
    if not args.allow_unsealed or (HERE / "SHA256SUMS").exists():
        seal(args.seal)
    print(json.dumps(report, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
