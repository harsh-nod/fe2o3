#!/usr/bin/env python3
"""Replay this CPU packet's hashes, source delta, receipts and reported results."""

import hashlib
import json
from pathlib import Path
import re
import subprocess


ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[2]
PRIOR = REPO / "docs/evidence/dev-topology-link-scratch-cpu-2026-09-24/raw/cpu1"
PRIOR_RUNNER = REPO / "docs/evidence/dev-topology-link-scratch-cpu-2026-09-24/runner.py"
PRIOR_RUNNER_SHA256 = "b1e15db084398e4e19cbb0f4f7a0588cf2594b89611d49248a1426243c339b4a"
OWNED = Path("/home/harsh/.codex-tmp/fe2o3-hot-batch-20260924-feYb12D3")
ALLOWED_DELTA = {"crates/fe2o3-runtime/examples/gfx942-runtime-xgmi-peer-benchmark.rs"} | {
    "benchmarks/runtime_gfx942/" + name for name in (
        "xgmi_peer_benchmark_common.hpp", "xgmi_peer_benchmark_common_test.cpp",
        "test_xgmi_peer_benchmark_common.py", "xgmi_peer_hip.cpp", "xgmi_peer_hsa.cpp",
        "xgmi_peer_hot_results.py", "test_xgmi_peer_hot_results.py",
        "xgmi_peer_hot_callbacks_test.cpp", "test_xgmi_peer_hot_callbacks.py",
    )
}
STAGES = (
    "rustc", "cargo", "g++", "hipcc", "native_benchmark_args",
    "xgmi_peer_benchmark_common", "xgmi_peer_hot_callbacks", "xgmi_peer_hot_results",
    "xgmi_peer_segments", "build-hip", "build-hsa", "gnu-default", "gnu-all",
    "musl-default", "musl-all", "format", "clippy", "after-rustc", "after-cargo",
    "after-g++", "after-hipcc",
)
PRIOR_STAGES = (
    "rustc", "cargo", "format", "gnu-topology", "gnu-currentness", "gnu-runtime",
    "musl-topology", "musl-currentness", "musl-runtime", "docs", "clippy",
    "after-rustc", "after-cargo",
)


def need(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read(path):
    return json.loads(path.read_text())


def bracket(folder):
    before = read(folder / "inputs-before.json")
    need(before == read(folder / "inputs-after.json"), "source bracket: " + str(folder))
    return before


def expected_commands(prior=False):
    commands = {"rustc": ["rustc", "-vV"], "cargo": ["cargo", "-V"]}
    if prior:
        cargo = ["cargo", "--locked"]
        packages = ["-p", "fe2o3-kfd", "-p", "fe2o3-runtime"]
        commands["format"] = ["cargo", "fmt"] + packages + ["--", "--check"]
        for label, target in (("gnu", []), ("musl", ["--target", "x86_64-unknown-linux-musl"])):
            for selection in ("topology", "currentness"):
                commands[label + "-" + selection] = cargo + ["test", "-p", "fe2o3-kfd", "--all-features", "--lib"] + target + [selection + "::", "--", "--test-threads=4"]
            commands[label + "-runtime"] = cargo + ["test", "-p", "fe2o3-runtime", "--all-features", "--lib"] + target + ["--", "--test-threads=4"]
        commands["docs"] = cargo + ["test"] + packages + ["--all-features", "--doc"]
        commands["clippy"] = cargo + ["clippy"] + packages + ["--all-features", "--all-targets", "--", "-D", "warnings"]
    else:
        commands.update({"g++": ["/usr/bin/g++", "--version"], "hipcc": ["/opt/rocm/bin/hipcc", "--version"]})
        prefix = "benchmarks/runtime_gfx942/"
        captured = "/home/harsh/.codex-tmp/fe2o3-hot-batch-20260924-feYb12D3/cpu2/"
        for name in STAGES[4:9]:
            commands[name] = ["/usr/bin/python3", "-I", "-B", prefix + "test_" + name + ".py"]
        commands["build-hip"] = ["/opt/rocm/bin/hipcc", "-std=c++17", "-O3", "-Wall", "-Wextra", "-Werror", "--offload-arch=gfx942", prefix + "xgmi_peer_hip.cpp", "-o", captured + "peer-hip"]
        commands["build-hsa"] = ["/usr/bin/g++", "-std=c++17", "-O3", "-Wall", "-Wextra", "-Werror", "-I/opt/rocm/include", prefix + "xgmi_peer_hsa.cpp", "-L/opt/rocm/lib", "-Wl,-rpath,/opt/rocm/lib", "-lhsa-runtime64", "-o", captured + "peer-hsa"]
        cargo = ["cargo", "--offline", "--locked"]
        example = ["-p", "fe2o3-runtime", "--example", "gfx942-runtime-xgmi-peer-benchmark"]
        for label, target in (("gnu", []), ("musl", ["--target", "x86_64-unknown-linux-musl"])):
            for feature, flags in (("default", []), ("all", ["--all-features"])):
                commands[label + "-" + feature] = cargo + ["test"] + example + target + flags + ["--", "--test-threads=2"]
        commands["format"] = ["cargo", "fmt", "-p", "fe2o3-runtime", "--", "--check"]
        commands["clippy"] = cargo + ["clippy", "-p", "fe2o3-runtime", "--all-features", "--example", "gfx942-runtime-xgmi-peer-benchmark", "--", "-D", "warnings"]
    for tool in ("rustc", "cargo") + (() if prior else ("g++", "hipcc")):
        commands["after-" + tool] = commands[tool]
    return commands


def receipts(folder, names, expected):
    need({path.name for path in folder.iterdir() if path.is_dir()} == set(names), "command roster")
    need(set(expected) == set(names), "expected command roster")
    end = 0
    for name in names:
        row = read(folder / name / "record.json")
        need(set(row) == {"command", "started_ns", "process_group", "group_absent", "finished_ns", "status"},
             "receipt field roster")
        need(type(row["status"]) is int and row["status"] == 0 and row["group_absent"] is True,
             "failed or unreaped command: " + name)
        need(type(row["process_group"]) is int and row["process_group"] > 0, "process group")
        need(end <= row["started_ns"] < row["finished_ns"], "command chronology")
        need(row["command"] == expected[name], "argv: " + name)
        end = row["finished_ns"]
    for tool in ("rustc", "cargo") + (("g++", "hipcc") if "hipcc" in names else ()):
        for stream in ("stdout.log", "stderr.log"):
            need((folder / tool / stream).read_bytes() == (folder / ("after-" + tool) / stream).read_bytes(),
                 "tool continuity: " + tool)


def main():
    manifest = read(ROOT / "artifacts.json")
    need(not any(path.is_symlink() for path in (ROOT / "raw").rglob("*")), "symlinked artifact")
    actual = {str(path.relative_to(ROOT)): digest(path) for path in (ROOT / "raw").rglob("*") if path.is_file()}
    need(actual == manifest, "artifact hashes or roster")
    raw = ROOT / "raw/cpu2"
    before = bracket(raw)
    need(before["runner"] == digest(ROOT / "runner.py"), "runner identity")
    paths = subprocess.check_output([
        "/usr/bin/git", "ls-files", "--cached", "--others", "--exclude-standard", "-z",
        "crates", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", "benchmarks/runtime_gfx942/",
    ], cwd=REPO, timeout=30, env={
        "PATH": "/usr/bin:/bin", "HOME": "/nonexistent", "LC_ALL": "C",
        "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null", "GIT_CONFIG_SYSTEM": "/dev/null",
        "GIT_NO_REPLACE_OBJECTS": "1",
    }).decode().split("\0")
    selected = {path for path in paths if path and (
        path.endswith((".rs", ".toml", ".lock", ".json", ".py", ".cpp", ".hpp")) or "/fixtures/" in path
    )}
    need(set(before["source"]) == selected, "source roster differs")
    for path, expected in before["source"].items():
        need(digest(REPO / path) == expected, "current source differs: " + path)
    prior = bracket(PRIOR)
    need(prior["runner"] == digest(PRIOR_RUNNER) == PRIOR_RUNNER_SHA256, "prior runner identity")
    delta = read(raw / "prior-qualification-delta.json")
    need(delta["prior_inputs_sha256"] == digest(PRIOR / "inputs-before.json"), "prior input identity")
    changed = sorted(path for path in set(prior["source"]) | set(before["source"])
                     if prior["source"].get(path) != before["source"].get(path))
    need(changed == delta["changed"] == delta["allowed"], "source delta")
    need(set(changed) == ALLOWED_DELTA, "unexpected source delta")
    receipts(raw, STAGES, expected_commands())
    receipts(PRIOR, PRIOR_STAGES, expected_commands(prior=True))
    for tool in ("rustc", "cargo"):
        for stream in ("stdout.log", "stderr.log"):
            need((raw / tool / stream).read_bytes() == (PRIOR / tool / stream).read_bytes(),
                 "prior Rust/Cargo identity differs")
    for label in ("gnu", "musl"):
        for suite, count, ignored in (("topology", 83, 0), ("currentness", 30, 0), ("runtime", 1365, 20)):
            text = (PRIOR / f"{label}-{suite}" / "stdout.log").read_text()
            need(f"test result: ok. {count} passed; 0 failed; {ignored} ignored;" in text, "prior result count")
        for feature in ("default", "all"):
            text = (raw / f"{label}-{feature}" / "stdout.log").read_text()
            count = 13 if feature == "default" else 14
            need(f"test result: ok. {count} passed; 0 failed; 0 ignored;" in text, "example results")
    for name, count in (("native_benchmark_args", 2), ("xgmi_peer_benchmark_common", 3),
                        ("xgmi_peer_hot_callbacks", 7), ("xgmi_peer_hot_results", 7)):
        text = (raw / name / "stderr.log").read_text()
        need(re.search(rf"Ran {count} tests? in [0-9.]+s\n\nOK\n\Z", text) is not None, "Python result: " + name)
    text = (raw / "xgmi_peer_segments" / "stderr.log").read_text()
    need(re.search(r"Ran 2 tests in [0-9.]+s\n\nOK \(skipped=1\)\n\Z", text) is not None,
         "ordered segment controls: one passed, optional Rust differential skipped")
    binaries = read(raw / "binaries.json")
    need(set(binaries) == {"peer-hip", "peer-hsa"}, "comparator ELF roster")
    for name, expected in binaries.items():
        need(type(expected) is str and re.fullmatch(r"[0-9a-f]{64}", expected) is not None, "ELF digest format")
        need((raw / name).is_file() and not (raw / name).is_symlink(), "ELF file type")
        need(digest(raw / name) == expected, "comparator ELF identity")
    cleanup = read(ROOT / "raw/cleanup.json")
    paths = [OWNED / "cpu2/target", OWNED / "cpu1/target", OWNED / "hsa-callbacks", OWNED / "hip-callbacks"]
    need(type(cleanup) is list and len(cleanup) == len(paths), "cleanup roster")
    for row, path in zip(cleanup, paths, strict=True):
        need(set(row) == {"path", "allocated_bytes", "absent"} and row["path"] == str(path), "cleanup path")
        need(type(row["allocated_bytes"]) is int and row["allocated_bytes"] >= 0, "cleanup size")
        need(row["absent"] is True and not path.exists() and not path.is_symlink(), "owned path remains")
    rejected = ROOT / "raw/rejected-cpu1"
    rejected_inputs = bracket(rejected)
    need(rejected_inputs["runner"] == before["runner"], "rejected runner identity")
    need(set(rejected_inputs["source"]) == selected, "rejected source roster")
    changed_since_rejection = {path for path in selected
                              if rejected_inputs["source"][path] != before["source"][path]}
    need(changed_since_rejection == {"crates/fe2o3-runtime/examples/gfx942-runtime-xgmi-peer-benchmark.rs"},
         "unexpected changes after rejection")
    need({path.name for path in rejected.iterdir() if path.is_dir()} == set(STAGES[:16]), "rejected command roster")
    failed = read(rejected / "format/record.json")
    need(failed["status"] == 1 and failed["group_absent"] is True, "rejected format outcome")
    need("aggregate_hot_only_admits_bounded_batches" in (rejected / "format/stdout.log").read_text(), "format rejection detail")
    print(f"hot batch CPU packet: {len(actual)} artifacts, {len(STAGES)} commands, source delta and prior qualification pass")


if __name__ == "__main__":
    main()
