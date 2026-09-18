#!/usr/bin/env python3
"""Classify this exact rejected prefix and cleanup; never qualify a campaign."""

import argparse
import copy
import json
import re
from pathlib import Path
import shlex
import sys

sys.dont_write_bytecode = True
import native as N  # noqa: E402
import controller as C  # noqa: E402
import verify as V  # noqa: E402

HERE = Path(__file__).resolve().parent


def commands(marker, state):
    owned, original, archive = Path(marker["path"]), V.ORIGINAL_ROOT, V.ORIGINAL_HERE
    payload = Path(state["local_payload"])
    serialized = json.dumps(marker, sort_keys=True, separators=(",", ":"))
    local = {
        "calibration": (["python3", "-B", str(archive / "test_native.py")], 60),
        "observer-tests": (
            ["python3", "-B", "benchmarks/runtime_gfx942/test_copy_host_observe.py"],
            60,
        ),
        "cpu-verify": (
            [
                "python3",
                "-B",
                str(
                    original
                    / "docs/evidence/dev-xgmi-creation-root-cpu-2026-09-18/verify.py"
                ),
                "--live",
            ],
            60,
        ),
        "signature": (
            [
                "git",
                "-c",
                "gpg.ssh.allowedSignersFile=" + C.SIGNERS,
                "verify-commit",
                marker["commit"],
            ],
            30,
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
                        str(owned / "native.py"),
                        "run",
                        serialized,
                    ]
                ),
            ],
            2500,
        ),
        "collect": (
            [
                "scp",
                "-r",
                *C.SSH,
                "mi300x:" + str(owned / "results"),
                str(archive / "remote"),
            ],
            300,
        ),
    }
    for name, mode, bound in (
        ("create", "create", 45),
        ("remote-inventory", "inventory", 120),
        ("cleanup", "cleanup", 120),
        ("absence", "absence", 120),
    ):
        local[name] = (
            [
                "ssh",
                "-T",
                *C.SSH,
                "mi300x",
                shlex.join(["/usr/bin/python3", "-B", "-", mode, serialized]),
            ],
            bound,
        )
    remote = {
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
        "kfd-d1": (
            [
                str(
                    owned / "target/release/examples/gfx942-runtime-xgmi-peer-benchmark"
                ),
                *(device[2] for device in N.DEVICES),
                "1048576",
                "1",
                "10",
                "30",
            ],
            120,
        ),
    }
    for suffix in ("before", "after", "delayed"):
        for index, bdf, uid in N.DEVICES:
            remote[f"kfd-d1-{suffix}-gpu{index}"] = (
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
            )
    return local, remote


def check():
    expected_top = {
        ".gitattributes",
        "README.md",
        "native.py",
        "controller.py",
        "verify.py",
        "test_native.py",
        "rejection.py",
        "binding.json",
        "owner.json",
        "remote-inventory.json",
        "controller-state.json",
        "local",
        "remote",
    }
    N.need(
        {path.name for path in HERE.iterdir()} - {"SHA256SUMS"} == expected_top,
        "exact rejected archive membership",
    )
    binding, marker = V.read(HERE / "binding.json"), V.read(HERE / "owner.json")
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
            "warmups",
            "samples",
            "performance_acceptance",
            "formal_refinement",
        }
        and binding["schema"] == "fe2o3.xgmi-creation-native-development.v1"
        and binding["devices"] == [list(device) for device in N.DEVICES]
        and (
            binding["bytes"],
            binding["depths"],
            binding["warmups"],
            binding["samples"],
        )
        == (1048576, [1, 16], 10, 30)
        and binding["performance_acceptance"] is False
        and binding["formal_refinement"] is False,
        "exact original binding policy",
    )
    N.owned_path(marker, exists=False)
    N.need(
        binding["commit"]
        == marker["commit"]
        == "1161876123a061ca0a772937795daed648b02d62",
        "exact rejected source",
    )
    N.need(
        N.sha(HERE / "binding.json") == marker["binding_sha256"], "ownership binding"
    )
    N.need(
        binding["local_tools"]
        == {
            name: N.sha(HERE / name)
            for name in ("controller.py", "native.py", "test_native.py", "verify.py")
        },
        "original tools unchanged",
    )
    N.need(
        binding["payload"]["native.py"] == N.sha(HERE / "native.py"),
        "executed runner identity",
    )
    cpu = HERE.parent / "dev-xgmi-creation-root-cpu-2026-09-18"
    N.need(
        N.sha(cpu / "SHA256SUMS") == binding["cpu_seal_sha256"]
        and V.read(cpu / "raw/source-before.log")["files"] == binding["source_files"],
        "CPU source binding",
    )
    expected_cpu = "".join(
        digest + "  " + name + "\n"
        for name, digest in N.inventory(cpu).items()
        if name != "SHA256SUMS"
    )
    N.need((cpu / "SHA256SUMS").read_text() == expected_cpu, "complete CPU seal")
    state = V.read(HERE / "controller-state.json")
    local_commands, remote_commands = commands(marker, state)
    remote_environment = {
        "HOME": "/home/harsh",
        "USER": "harsh",
        "PATH": "/home/harsh/.cargo/bin:/opt/rocm/bin:/usr/bin:/bin",
        "LANG": "C",
        "LC_ALL": "C",
        "CARGO_TARGET_DIR": marker["path"] + "/target",
        "CARGO_INCREMENTAL": "0",
        "CARGO_BUILD_JOBS": "2",
        "CARGO_TERM_COLOR": "never",
        "RUSTUP_TOOLCHAIN": "nightly-2026-04-03",
        "TMPDIR": marker["path"] + "/tmp",
    }
    N.need(
        state["native_success"] is False
        and state["failure"]
        == state["native_failure"]
        == "RuntimeError: native failed",
        "campaign remains rejected",
    )
    N.need(
        all(
            state[key] is True
            for key in (
                "created",
                "collected",
                "cleaned",
                "absence",
                "local_payload_absent",
            )
        ),
        "cleanup closure",
    )
    remote = HERE / "remote"
    binaries = V.read(remote / "binaries.json")
    N.need(
        set(binaries)
        == {
            "target/release/examples/gfx942-runtime-xgmi-peer-benchmark",
            "xgmi-peer-hip",
            "xgmi-peer-hsa",
        }
        and all(re.fullmatch(r"[0-9a-f]{64}", digest) for digest in binaries.values()),
        "three optimized binary identities",
    )
    files = N.inventory(remote)
    N.need(
        len(files) == 48 and files == V.read(HERE / "remote-inventory.json"),
        "complete rejected collection",
    )
    N.need(
        V.read(remote / "source-before.json")
        == binding["source_files"]
        == V.read(remote / "source-after.json"),
        "unchanged native source",
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
        "kfd-d1-before-gpu1",
        "kfd-d1-before-gpu2",
        "kfd-d1",
        "kfd-d1-after-gpu1",
        "kfd-d1-after-gpu2",
        "kfd-d1-delayed-gpu1",
        "kfd-d1-delayed-gpu2",
    ]
    N.need(
        {path.name for path in remote.iterdir()}
        == set(names) | {"source-before.json", "source-after.json", "binaries.json"},
        "no later phases or successful campaign record",
    )
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
        "exact local control roster",
    )
    for folder, roster in ((remote, names), (HERE / "local", local_names)):
        last = 0
        for name in roster:
            path = folder / name
            row = V.read(path / "receipt.json")
            N.need(
                {entry.name for entry in path.iterdir()}
                == {"receipt.json", "stdout", "stderr"},
                "exact receipt files",
            )
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
            expected_command, expected_bound = (
                remote_commands if folder == remote else local_commands
            )[name]
            expected_input = (
                binding["payload"]["native.py"]
                if folder != remote
                and name in ("create", "remote-inventory", "cleanup", "absence")
                else None
            )
            N.need(
                row["command"] == expected_command
                and row["timeout_seconds"] == expected_bound
                and row["cwd"]
                == (
                    marker["path"] + "/source"
                    if folder == remote
                    else str(V.ORIGINAL_ROOT)
                )
                and row["environment"]
                == (remote_environment if folder == remote else None)
                and row["stdin_sha256"] == expected_input,
                "exact phase command, environment and bound",
            )
            N.need(
                type(row["pid"]) is int
                and row["pid"] > 0
                and type(row["started_ns"]) is int
                and type(row["finished_ns"]) is int
                and row["started_ns"] > 0,
                "typed receipt identities and timestamps",
            )
            expected_exit = int(
                (folder == remote and name == "kfd-d1-after-gpu1")
                or (folder != remote and name == "native")
            )
            N.need(
                type(row["exit"]) is int
                and row["exit"] == expected_exit
                and row["error"] is None
                and row["group_absent"] is True,
                "exact phase outcome and process closure",
            )
            N.need(
                row["stdout_sha256"] == N.sha(path / "stdout")
                and row["stderr_sha256"] == N.sha(path / "stderr"),
                "raw receipt hashes",
            )
            N.need(
                last <= row["started_ns"] <= row["finished_ns"],
                "per-host ordered receipts",
            )
            last = row["finished_ns"]
    N.need(
        (HERE / "local/source-clean/stdout").read_bytes() == b"",
        "clean committed source",
    )
    for name, count in (("calibration", 10), ("observer-tests", 19)):
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
    parsed = N.parse_results((remote / "kfd-d1/stdout").read_bytes(), "kfd", 1)
    N.need((remote / "kfd-d1/stderr").read_bytes() == b"", "no KFD diagnostics")
    for suffix in ("before", "after", "delayed"):
        for index, bdf, uid in N.DEVICES:
            name = f"kfd-d1-{suffix}-gpu{index}"
            lines = (remote / name / "stdout").read_text().splitlines()
            N.need(len(lines) == 2, "complete endpoint transcript")
            observation, complete = [
                json.loads(line, object_pairs_hook=V.unique) for line in lines
            ]
            rejected = suffix == "after" and index == 1
            N.need(
                complete
                == {
                    "schema": "fe2o3.copy-host-observation.v1",
                    "record": "complete",
                    "observations": 1,
                    "refused": int(rejected),
                    "all_endpoints_admitted": not rejected,
                    "performance_accepted": False,
                },
                "exact endpoint completion",
            )
            if rejected:
                N.need(
                    observation["reasons"] == ["sysfs-before-busy"]
                    and observation["endpoint_admitted"] is False
                    and observation["selected_pids"] == [],
                    "sole immediate rejection reason",
                )
                N.need(
                    [
                        snapshot["values"]["gpu_busy_percent"]
                        for snapshot in observation["sysfs"]
                    ]
                    == ["9", "0", "0"],
                    "exact residual activity sequence",
                )
                N.need(
                    all(
                        snapshot["values"]["mem_info_vram_used"] == "298647552"
                        for snapshot in observation["sysfs"]
                    ),
                    "baseline VRAM throughout rejection",
                )
                quiet = copy.deepcopy(observation)
                quiet["sysfs"][0]["values"]["gpu_busy_percent"] = "0"
                quiet["reasons"], quiet["endpoint_admitted"] = [], True
                # Only the asserted 9% sample differs; replay every other raw field.
                V.endpoint(quiet, index, bdf, uid)
            else:
                V.endpoint(observation, index, bdf, uid)
    serialized = json.dumps(marker, sort_keys=True, separators=(",", ":"))
    for name in ("cleanup", "absence"):
        row = V.read(HERE / "local" / name / "receipt.json")
        N.need(
            row["command"]
            == [
                "ssh",
                "-T",
                *C.SSH,
                "mi300x",
                shlex.join(["/usr/bin/python3", "-B", "-", name, serialized]),
            ]
            and row["stdin_sha256"] == binding["payload"]["native.py"],
            "exact owned cleanup and absence commands",
        )
    N.need(
        V.read(HERE / "local/remote-inventory/stdout") == files
        and V.read(HERE / "local/cleanup/stdout") == {"removed": marker["path"]}
        and V.read(HERE / "local/absence/stdout")
        == {"path_absent": True, "processes_absent": True},
        "collected and cleaned exact owned run",
    )
    return {
        "campaign": "rejected",
        "native_qualification": False,
        "performance_acceptance": False,
        "formal_refinement": False,
        "valid_kfd_prefix_rows": len(parsed),
        "remote_files": len(files),
        "cleanup_closed": True,
        "reason": "first-postflight-gpu1-sysfs-busy-9-percent",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seal", action="store_true")
    parser.add_argument("--allow-unsealed", action="store_true")
    parser.add_argument("--calibrate", action="store_true")
    args = parser.parse_args()
    result = check()
    if args.calibrate:
        from unittest import mock

        original = V.read
        mutations = [
            ("controller-state.json", {"native_success": True}),
            ("remote/kfd-d1/receipt.json", {"exit": 1}),
            ("remote/kfd-d1-after-gpu1/receipt.json", {"exit": 0}),
            ("local/cleanup/receipt.json", {"stdin_sha256": "0" * 64}),
            ("local/absence/stdout", {"processes_absent": False}),
            ("remote/source-after.json", {"extra": "0" * 64}),
            ("local/native/receipt.json", {"command": ["wrong"]}),
            ("remote/build-kfd/receipt.json", {"environment": {"LANG": "wrong"}}),
            ("remote/kfd-d1-before-gpu1/receipt.json", {"timeout_seconds": 1}),
            ("remote/binaries.json", {"xgmi-peer-hip": "invalid"}),
        ]
        for name, changes in mutations:
            target = HERE / name

            def altered(path):
                value = original(path)
                return {**value, **changes} if path == target else value

            with mock.patch.object(V, "read", side_effect=altered):
                try:
                    check()
                except RuntimeError:
                    pass
                else:
                    raise RuntimeError("rejection calibration accepted " + name)
        result["rejection_mutation_calibrations"] = len(mutations)
    if args.seal or not args.allow_unsealed or (HERE / "SHA256SUMS").exists():
        V.seal(args.seal)
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
