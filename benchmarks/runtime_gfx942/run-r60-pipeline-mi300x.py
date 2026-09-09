#!/usr/bin/env python3
"""Run the signed R60 qualifier and three guarded, matched MI300X triples."""

from __future__ import annotations

import argparse
import ctypes
import hashlib
import importlib.util
import json
import os
import pathlib
import re
import secrets
import shutil
import signal
import stat
import subprocess
import sys
import tarfile
import tempfile
import time
from typing import Any

sys.dont_write_bytecode = True
SYSTEM_PATH = "/usr/sbin:/usr/bin:/sbin:/bin"
CLEAN_ENV = {"LANG": "C", "LC_ALL": "C", "PATH": SYSTEM_PATH}
GPU_INDEX = 1
UNIQUE_ID = "0xab83d2ffef0d3cdf"
ORDERS = (("kfd", "hsa", "hip"), ("hsa", "hip", "kfd"), ("hip", "kfd", "hsa"))
PHASE_SECONDS = 180
MAX_LOG_BYTES = 8 << 20
MAX_ARCHIVE_BYTES = 1 << 30
BENCH_DIR = pathlib.Path("benchmarks/runtime_gfx942")
EXAMPLE = "gfx942-runtime-r60-ordinary-pipeline"
OUTPUT_SHA256 = "79fd0768604fe9de0ced87297f7d653343e998926b59e2cce7df5e38194c52b3"
QUALIFIER_PASS = (
    "PASS schema=fe2o3.runtime.gfx942-r60-ordinary-pipeline-qualification.v1 "
    "launches=64 publication_prefix=64 completion_order=contiguous "
    "execution_order=wait-for-prior concurrent_kernel_execution=false "
    "data_path_last=ResidentReused user_data_materializations_last=0 readbacks=1 "
    f"output_sha256={OUTPUT_SHA256} cleanup=complete\n"
)


class RunError(RuntimeError):
    pass


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def write_json(path: pathlib.Path, value: Any) -> None:
    with path.open("x", encoding="utf-8") as stream:
        json.dump(value, stream, sort_keys=True, allow_nan=False)
        stream.write("\n")


def load_module(path: pathlib.Path, name: str) -> Any:
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RunError(f"cannot import archived helper {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def fields(text: str, prefix: str) -> dict[str, str]:
    if not text.endswith("\n") or text.count("\n") != 1:
        raise RunError(f"{prefix} must be one newline-terminated record")
    tokens = text[:-1].split(" ")
    if tokens[0] != prefix:
        raise RunError(f"unexpected {prefix} record")
    result = {}
    for token in tokens[1:]:
        if "=" not in token:
            raise RunError(f"malformed {prefix} token")
        key, value = token.split("=", 1)
        if not key or not value or key in result:
            raise RunError(f"duplicate or empty {prefix} field")
        result[key] = value
    return result


def sealed_fields(text: str, prefix: str, names: tuple[str, ...]) -> dict[str, str]:
    result = fields(text, prefix)
    seal = f"{prefix}_sha256"
    if set(result) != set(names) | {seal}:
        raise RunError(f"unexpected {prefix} fields")
    body = prefix + " " + " ".join(f"{key}={result[key]}" for key in names)
    digest = hashlib.sha256((body + "\n").encode()).hexdigest()
    if result[seal] != digest or text != f"{body} {seal}={digest}\n":
        raise RunError(f"invalid or noncanonical {prefix} seal")
    return result


def integer(value: str, low: int, high: int, name: str) -> int:
    if re.fullmatch(r"0|[1-9][0-9]*", value) is None:
        raise RunError(f"invalid {name}")
    parsed = int(value)
    if not low <= parsed <= high:
        raise RunError(f"out-of-range {name}")
    return parsed


def validate_monitor(text: str, output: pathlib.Path, topology: dict[str, str],
                     retained_checker: Any) -> dict[str, str]:
    result = sealed_fields(text, "monitor", retained_checker.R26_MONITOR_SEALED_FIELDS)
    expected = {
        "schema": "fe2o3.r26-kfd-queue-monitor.v2", "status": "clean",
        "monitor": "selected-kfd-gpu-process-tree-census-v2",
        "schedule": "absolute-monotonic-raw-deadline-v1",
        "kfd_gpu_id": topology["kfd_gpu_id"], "observer_cpu": topology["observer_cpu"],
        "interval_us": "2000", "maximum_gap_us": "10000",
        "foreign_selected_queues": "0", "terminal_selected_queues": "0",
        "target_exit_code": "0", "target_reaped": "1", "process_group_absent": "1",
        "target_output_bytes": str(output.stat().st_size),
        "target_output_sha256": sha256_file(output),
    }
    if any(result[key] != value for key, value in expected.items()):
        raise RunError("monitor does not establish clean execution and exact output custody")
    pid = integer(result["root_pid"], 1, 1 << 30, "root PID")
    if result["process_group"] != str(pid):
        raise RunError("monitor process group does not match its root PID")
    integer(result["observations"], 3, 1 << 32, "monitor observations")
    integer(result["target_selected_queue_observations"], 1, 1 << 32, "queue observations")
    integer(result["observed_maximum_gap_us"], 1, 10000, "observed monitor gap")
    return result


def validate_topology(text: str, identity: dict[str, str], checker: Any) -> dict[str, str]:
    result = sealed_fields(text, "topology", checker.R26_TOPOLOGY_SEALED_FIELDS)
    expected = {
        "schema": "fe2o3.r26-host-topology.v1",
        "placement": "taskset-cpulist-then-numactl-physcpubind-membind-v1",
        "gpu_index": str(GPU_INDEX), "unique_id": UNIQUE_ID,
        "pci_bdf": identity["pci_bdf"], "numa_node": identity["pci_numa_node"],
        "kfd_node": identity["gpu_node_id"], "kfd_gpu_id": identity["gpu_guid"],
    }
    if any(result[key] != value for key, value in expected.items()):
        raise RunError("topology does not match the selected physical device")
    local = checker.r26_parse_id_list(result["device_local_cpu_list"], "local CPUs")
    allowed = checker.r26_parse_id_list(result["allowed_cpu_list"], "allowed CPUs")
    measurement = checker.r26_parse_id_list(result["measurement_cpu_list"], "measurement CPUs")
    memory = checker.r26_parse_id_list(result["allowed_mem_node_list"], "memory nodes")
    observer = integer(result["observer_cpu"], 0, 1048575, "observer CPU")
    numa = integer(result["numa_node"], 0, 1048575, "NUMA node")
    integer(result["kfd_gpu_id"], 1, (1 << 32) - 1, "KFD GPU ID")
    if not measurement or measurement - (local & allowed) or observer not in allowed or observer in measurement or numa not in memory:
        raise RunError("topology does not provide local CPU/memory placement and disjoint observer")
    return result


def validate_profile(data: bytes, checker: Any) -> None:
    if not data or len(data) > MAX_LOG_BYTES:
        raise RunError("qualification profile is empty or oversized")
    profile = json.loads(data, object_pairs_hook=checker.unique_object,
                         parse_constant=checker.reject_constant)
    if type(profile) is not dict or profile.get("schema") != "fe2o3-kfd-runtime-profile-v1" or not checker.exact(profile.get("schema_version"), 1):
        raise RunError("unexpected qualification profile schema")
    device = profile.get("device", {})
    target = device.get("target_profile")
    if target != "gfx942:xnack-" or not checker.exact(device.get("wave_width"), 64):
        raise RunError("qualification profile has the wrong device target")
    digest = hashlib.sha256(b"fe2o3.kfd-runtime-profile.device.v1\0")
    for value in (int(UNIQUE_ID, 16).to_bytes(8, "little"), target.encode(), (64).to_bytes(2, "little")):
        digest.update(len(value).to_bytes(8, "little"))
        digest.update(value)
    if device.get("identity") != digest.hexdigest():
        raise RunError("qualification profile has the wrong device identity")
    events = profile.get("events")
    coverage = profile.get("coverage", {})
    if type(events) is not list or not events or len(events) > 65536 or coverage.get("origin") != "observed" or coverage.get("complete_runtime_operation_history") is not True or not checker.exact(coverage.get("dropped_events"), 0) or not checker.exact(coverage.get("observed_events"), len(events)):
        raise RunError("qualification profile lacks complete operation history")
    published = []
    completed = 0
    for sequence, entry in enumerate(events):
        if type(entry) is not dict or not checker.exact(entry.get("sequence"), sequence) or entry.get("origin") != "observed":
            raise RunError("qualification profile has a missing or foreign sequence")
        event = entry.get("event", {})
        if type(event) is not dict:
            raise RunError("qualification profile contains a malformed event")
        kind = event.get("kind")
        dispatch = event.get("dispatch")
        if kind == "dispatch_published":
            if completed != 0 or len(published) == 64 or type(dispatch) is not str or re.fullmatch(r"[0-9a-f]{64}", dispatch) is None or dispatch == "0" * 64 or dispatch in published:
                raise RunError("qualification publication prefix is incomplete or duplicated")
            published.append(dispatch)
        elif kind == "dispatch_completed":
            if len(published) != 64 or completed >= 64 or published[completed] != dispatch:
                raise RunError("qualification completion precedes the full prefix or is out of order")
            if completed and not checker.exact(event.get("host_timing", {}).get("native_binding_ns"), 0):
                raise RunError("qualification successor rematerialized bindings")
            completed += 1
    if len(published) != 64 or completed != 64:
        raise RunError("qualification profile omits publications or completions")


def archive_members(archive: tarfile.TarFile) -> list[tarfile.TarInfo]:
    members = archive.getmembers()
    seen = set()
    size = 0
    if len(members) > 100000:
        raise RunError("source archive contains too many entries")
    for member in members:
        path = pathlib.PurePosixPath(member.name)
        if path.is_absolute() or ".." in path.parts or str(path) in ("", ".") or str(path) in seen or not (member.isfile() or member.isdir()):
            raise RunError("source archive has duplicate, unsafe, or nonregular entries")
        seen.add(str(path))
        size += member.size
        if size > MAX_ARCHIVE_BYTES:
            raise RunError("source archive exceeds the bounded size")
    return members


def tree_hashes(root: pathlib.Path) -> dict[str, str]:
    result = {}
    for path in sorted(root.rglob("*")):
        mode = path.lstat().st_mode
        if stat.S_ISREG(mode):
            result[str(path.relative_to(root))] = sha256_file(path)
        elif not stat.S_ISDIR(mode):
            raise RunError("source tree contains a nonregular entry")
    return result


def readonly_tree(root: pathlib.Path) -> None:
    for path in root.rglob("*"):
        mode = path.stat().st_mode
        path.chmod(0o500 if path.is_dir() or mode & 0o111 else 0o400)
    root.chmod(0o500)


def cleanup_tree(root: pathlib.Path) -> None:
    if root.exists():
        for directory, _, _ in os.walk(root):
            pathlib.Path(directory).chmod(0o700)
        shutil.rmtree(root)


def group_exists(group: int) -> bool:
    try:
        os.killpg(group, 0)
        return True
    except ProcessLookupError:
        return False


def enable_child_subreaper() -> None:
    # Adopt only orphan descendants so command-group cleanup can reap them.
    libc = ctypes.CDLL(None, use_errno=True)
    if libc.prctl(36, 1, 0, 0, 0) != 0:  # Linux PR_SET_CHILD_SUBREAPER.
        raise RunError(f"cannot enable child subreaper: errno={ctypes.get_errno()}")


def reap_owned_group(group: int) -> None:
    while True:
        try:
            pid, _ = os.waitpid(-group, os.WNOHANG)
        except ChildProcessError:
            return
        if pid == 0:
            return


def stop_owned_group(process: subprocess.Popen[bytes]) -> None:
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    # The retained GPU guard may need both of its five-second cleanup phases.
    deadline = time.monotonic() + 15
    while True:
        exited = process.poll() is not None
        if exited:
            reap_owned_group(process.pid)
        if exited and not group_exists(process.pid):
            return
        if time.monotonic() >= deadline:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.wait(timeout=5)
            for _ in range(100):
                reap_owned_group(process.pid)
                if not group_exists(process.pid):
                    return
                time.sleep(0.05)
            raise RunError("owned subprocess group remains after cleanup")
        time.sleep(0.05)


class Runner:
    def __init__(self, args: argparse.Namespace, stage: pathlib.Path):
        self.args = args
        self.stage = stage
        self.source = stage / "source"
        self.evidence = stage / "evidence"
        self.evidence.mkdir(mode=0o700)
        self.commands = []
        self.sequence = 0

    def run(self, argv: list[str | pathlib.Path], *, label: str, timeout: int = 60,
            env: dict[str, str] | None = None, cwd: pathlib.Path | None = None,
            output: pathlib.Path | None = None, limit: int = MAX_LOG_BYTES) -> str:
        command = [str(part) for part in argv]
        stdout_path = output or self.evidence / f"{self.sequence:03d}-{label}.stdout"
        stderr_path = self.evidence / f"{self.sequence:03d}-{label}.stderr"
        self.sequence += 1
        record = {"argv": command, "environment": env or CLEAN_ENV,
                  "cwd": str(cwd or self.stage), "timeout_seconds": timeout,
                  "stdout": str(stdout_path.relative_to(self.stage)),
                  "stderr": str(stderr_path.relative_to(self.stage))}
        self.commands.append(record)
        enable_child_subreaper()
        with stdout_path.open("xb") as stdout, stderr_path.open("xb") as stderr:
            process = subprocess.Popen(command, cwd=cwd or self.stage, env=env or CLEAN_ENV,
                                       stdout=stdout, stderr=stderr, start_new_session=True)
            record["process_group"] = process.pid
            try:
                deadline = time.monotonic() + timeout
                while process.poll() is None:
                    if time.monotonic() >= deadline:
                        raise RunError(f"{label} exceeded {timeout} seconds")
                    if stdout_path.stat().st_size > limit or stderr_path.stat().st_size > MAX_LOG_BYTES:
                        raise RunError(f"{label} exceeded its output limit")
                    time.sleep(0.05)
                record["returncode"] = process.returncode
                if process.returncode != 0:
                    detail = stderr_path.read_text(errors="replace")[-4000:]
                    raise RunError(f"{label} exited {process.returncode}: {detail.strip()}")
                if group_exists(process.pid):
                    raise RunError(f"{label} left a process group after its leader exited")
            finally:
                # The guard handles TERM by reaping its independently grouped target.
                if process.poll() is None or group_exists(process.pid):
                    stop_owned_group(process)
        if stdout_path.stat().st_size > limit or stderr_path.stat().st_size > MAX_LOG_BYTES:
            raise RunError(f"{label} exceeded its output limit")
        return "" if output else stdout_path.read_text(encoding="utf-8")

    def snapshot(self) -> None:
        repo = self.args.repo
        git = ["/usr/bin/git", "-C", repo]
        if self.run(git + ["status", "--porcelain=v1", "--untracked-files=all",
                           "--ignore-submodules=none"], label="source-status"):
            raise RunError("qualification requires a clean checkout")
        self.commit = self.run(git + ["rev-parse", "HEAD"], label="source-commit").strip()
        if re.fullmatch(r"[0-9a-f]{40}", self.commit) is None:
            raise RunError("source commit is not a full SHA-1 identity")
        signers = self.evidence / "allowed-signers"
        shutil.copyfile(self.args.allowed_signers, signers)
        signers.chmod(0o400)
        self.run(git + ["cat-file", "commit", self.commit], label="signed-commit",
                 output=self.evidence / "signed-commit.txt")
        commit_bytes = (self.evidence / "signed-commit.txt").read_bytes()
        if b"\ngpgsig -----BEGIN SSH SIGNATURE-----\n" not in commit_bytes.split(b"\n\n", 1)[0] + b"\n":
            raise RunError("source commit does not carry an SSH signature")
        self.run(git + ["-c", "gpg.format=ssh", "-c", "gpg.ssh.program=/usr/bin/ssh-keygen",
                        "-c", "gpg.minTrustLevel=fully", "-c", f"gpg.ssh.allowedSignersFile={signers}",
                        "verify-commit", self.commit], label="verify-signature")
        archive_path = self.evidence / "source.tar"
        self.run(git + ["archive", "--format=tar", self.commit], label="archive",
                 output=archive_path, limit=MAX_ARCHIVE_BYTES)
        archive_path.chmod(0o400)
        self.source.mkdir(mode=0o700)
        with tarfile.open(archive_path, "r:") as archive:
            members = archive_members(archive)
            archive.extractall(self.source, members=members)
        self.source_hashes = tree_hashes(self.source)
        runner_path = BENCH_DIR / pathlib.Path(__file__).name
        if self.source_hashes.get(str(runner_path)) != sha256_file(pathlib.Path(__file__)):
            raise RunError("invoked runner does not match the signed source archive")
        write_json(self.evidence / "source-files.json", self.source_hashes)
        self.snapshot_input_hashes = {}
        for name in ("source.tar", "signed-commit.txt", "allowed-signers", "source-files.json"):
            path = self.evidence / name
            path.chmod(0o400)
            self.snapshot_input_hashes[name] = sha256_file(path)
        readonly_tree(self.source)
        self.bench = self.source / BENCH_DIR
        self.checker = load_module(self.bench / "check-r60-pipeline.py", "r60_runner_checker")
        self.retained = load_module(self.bench / "check-parity.py", "r60_retained_checker")
        self.guard = self.bench / "r26-host-guard.py"
        self.identity_collector = self.bench / "r26-system-identity.py"

    def verify_source(self) -> None:
        if tree_hashes(self.source) != self.source_hashes:
            raise RunError("archived source changed during qualification")
        for name, digest in self.snapshot_input_hashes.items():
            if sha256_file(self.evidence / name) != digest:
                raise RunError(f"signed snapshot input changed: {name}")
        for tool, identity in self.tool_hashes.items():
            path = pathlib.Path(tool)
            if str(path.resolve(strict=True)) != identity["resolved"] or sha256_file(path) != identity["sha256"]:
                raise RunError(f"build or execution tool changed: {tool}")
        for backend, path in self.binaries.items():
            if sha256_file(path) != self.binary_hashes[backend]:
                raise RunError(f"{backend} binary changed during qualification")

    def build(self) -> None:
        rocm = self.args.rocm_path
        build_home = self.args.build_home
        for name in ("config", "config.toml"):
            if (build_home / ".cargo" / name).exists():
                raise RunError("ambient user Cargo configuration is not admitted")
        rust_path = build_home / ".cargo/bin"
        rust_env = dict(CLEAN_ENV, HOME=str(build_home), PATH=f"{rust_path}:{SYSTEM_PATH}",
                        CARGO_INCREMENTAL="0", CARGO_BUILD_JOBS="4",
                        CARGO_TARGET_DIR=str(self.stage / "target"))
        native_env = dict(CLEAN_ENV, ROCM_PATH=str(rocm), PATH=f"{rocm}/bin:{SYSTEM_PATH}")
        self.tool_hashes = {}
        tools = [rust_path / "cargo", rust_path / "rustc", rust_path / "rustup",
                 rocm / "bin/hipcc", pathlib.Path(sys.executable),
                 *(pathlib.Path("/usr/bin") / name for name in
                   ("git", "ssh-keygen", "python3", "g++", "numactl", "taskset", "timeout"))]
        for tool in tools:
            self.tool_hashes[str(tool)] = {"resolved": str(tool.resolve(strict=True)),
                                          "sha256": sha256_file(tool)}
        temporary = self.stage / "tmp"
        temporary.mkdir(mode=0o700)
        rust_env["TMPDIR"] = native_env["TMPDIR"] = str(temporary)
        for name in ("cargo", "rustc"):
            self.run([rust_path / name, "--version"], label=f"{name}-version",
                     env=rust_env, cwd=self.source)
            resolved = self.run([rust_path / "rustup", "which", name], label=f"resolved-{name}",
                                env=rust_env, cwd=self.source).strip()
            tool = pathlib.Path(resolved)
            if not tool.is_absolute():
                raise RunError("Rust toolchain resolution did not produce an absolute executable")
            self.tool_hashes[resolved] = {"resolved": str(tool.resolve(strict=True)),
                                         "sha256": sha256_file(tool)}
        self.run([rocm / "bin/hipcc", "--version"], label="hipcc-version", env=native_env)
        self.run(["/usr/bin/g++", "--version"], label="cxx-version")
        self.run([rust_path / "cargo", "build", "--offline", "--locked", "--release",
                  "-p", "fe2o3-runtime", "--features", "hardware-qualification",
                  "--example", EXAMPLE], label="build-kfd", timeout=1200,
                 env=rust_env, cwd=self.source)
        hip = self.stage / "r60-pipeline-hip"
        hsa = self.stage / "r60-pipeline-hsa"
        options = ["-O3", "-std=c++17", "-Wall", "-Wextra", "-Werror"]
        self.run([rocm / "bin/hipcc", *options, self.bench / "r60_pipeline_hip.cpp",
                  "-lcrypto", "-o", hip], label="build-hip", timeout=180, env=native_env)
        self.run(["/usr/bin/g++", *options, f"-I{rocm}/include",
                  self.bench / "r60_pipeline_hsa.cpp", f"-L{rocm}/lib",
                  f"-Wl,-rpath,{rocm}/lib", "-lhsa-runtime64", "-lcrypto", "-o", hsa],
                 label="build-hsa", timeout=180, env=native_env)
        self.binaries = {"kfd": self.stage / "target/release/examples" / EXAMPLE,
                         "hip": hip, "hsa": hsa}
        self.binary_hashes = {}
        for backend, path in self.binaries.items():
            if not path.is_file() or path.stat().st_size == 0:
                raise RunError(f"missing {backend} binary")
            path.chmod(0o500)
            self.binary_hashes[backend] = sha256_file(path)
        self.fixture = self.source / "crates/fe2o3-runtime/fixtures/trusted-gfx942-vecadd-v1/vecadd.hsaco"
        if sha256_file(self.fixture) != self.checker.HSACO_SHA256:
            raise RunError("archived HSACO does not match the qualified artifact")
        self.verify_source()

    def system_identity(self, edge: str) -> dict[str, str]:
        raw = self.run(["/usr/bin/python3", self.identity_collector, "--gpu-index", str(GPU_INDEX),
                        "--rocm-path", self.args.rocm_path, "--observation-edge", edge,
                        "--kfd-binary", self.binaries["kfd"], "--hsa-binary", self.binaries["hsa"],
                        "--hip-binary", self.binaries["hip"]], label=f"identity-{edge}", timeout=180)
        identity = fields(raw, "context")
        version = (self.args.rocm_path / ".info/version").read_text().strip()
        context = {"execution_environment": "env-i-lang-c-lc-all-c-path-usr-sbin-usr-bin-sbin-bin-v1",
                   "gpu_index": str(GPU_INDEX), "unique_id": UNIQUE_ID,
                   "uuid": f"GPU-{UNIQUE_ID[2:]}", "rocm_version": version,
                   **{f"{backend}_binary_sha256": digest for backend, digest in self.binary_hashes.items()}}
        self.retained.r26_validate_system_identity(identity, context)
        if identity["observation_edge"] != edge:
            raise RunError("system identity has the wrong observation edge")
        return identity

    def topology_record(self, label: str) -> str:
        return self.run(["/usr/bin/python3", self.guard, "topology", "--gpu-index", str(GPU_INDEX),
                         "--pci-bdf", self.identity_start["pci_bdf"], "--unique-id", UNIQUE_ID],
                        label=f"topology-{label}")

    def telemetry(self, label: str) -> None:
        raw = self.run([self.args.rocm_path / "bin/rocm-smi", "--showuse", "--showclocks", "--showpower"],
                       label=f"telemetry-{label}")
        selected = [line for line in raw.splitlines() if line.strip().startswith(f"GPU[{GPU_INDEX}]")]
        usage = []
        for line in selected:
            match = re.fullmatch(r"GPU\[1\]\s*:\s*GPU use \(%\):\s*([0-9]+)", line.strip())
            if match:
                usage.append(int(match.group(1)))
        text = "\n".join(selected).lower()
        if len(usage) != 1 or usage[0] > 5 or "power" not in text or not ("clock" in text or "sclk" in text):
            raise RunError("selected GPU is busy or clock/power telemetry is incomplete")

    def phase_command(self, backend: str, args: list[str | pathlib.Path]) -> list[str | pathlib.Path]:
        env = ["/usr/bin/env", "-i", "LANG=C", "LC_ALL=C", f"PATH={SYSTEM_PATH}"]
        if backend in ("hsa", "hip"):
            visibility = "ROCR_VISIBLE_DEVICES" if backend == "hsa" else "HIP_VISIBLE_DEVICES"
            env += ["HSA_XNACK=0", f"{visibility}={GPU_INDEX}"]
        return [*env, "/usr/bin/taskset", "--cpu-list", self.topology["measurement_cpu_list"],
                "/usr/bin/numactl", f"--physcpubind={self.topology['measurement_cpu_list']}",
                f"--membind={self.topology['numa_node']}", "/usr/bin/timeout", "--foreground",
                "--signal=TERM", "--kill-after=5s", f"{PHASE_SECONDS}s", self.binaries[backend], *args]

    def phase(self, label: str, backend: str, args: list[str | pathlib.Path]) -> pathlib.Path:
        self.verify_source()
        if self.topology_record(f"{label}-start") != self.topology_raw:
            raise RunError("host placement changed before a measured phase")
        self.telemetry(f"{label}-start")
        output = self.evidence / f"{label}.jsonl"
        raw = self.run(["/usr/bin/python3", self.guard, "monitor", "--gpu-id", self.topology["kfd_gpu_id"],
                        "--observer-cpu", self.topology["observer_cpu"], "--target-output", output,
                        "--", *self.phase_command(backend, args)], label=f"monitor-{label}",
                       timeout=PHASE_SECONDS + 30)
        validate_monitor(raw, output, self.topology, self.retained)
        self.telemetry(f"{label}-end")
        if self.topology_record(f"{label}-end") != self.topology_raw:
            raise RunError("host placement changed during a measured phase")
        return output

    def qualify_and_measure(self) -> None:
        self.identity_start = self.system_identity("start")
        self.topology_raw = self.topology_record("initial")
        self.topology = validate_topology(self.topology_raw, self.identity_start, self.retained)
        self.run(["/usr/bin/taskset", "--cpu-list", self.topology["measurement_cpu_list"],
                  "/usr/bin/numactl", f"--physcpubind={self.topology['measurement_cpu_list']}",
                  f"--membind={self.topology['numa_node']}", "/usr/bin/true"], label="placement-probe")
        for ordinal in range(2):
            profile = self.evidence / f"qualification-{ordinal}.profile.json"
            output = self.phase(f"qualification-{ordinal}", "kfd", [UNIQUE_ID, "qualify", profile])
            if output.read_text() != QUALIFIER_PASS:
                raise RunError("qualifier did not emit the exact completed PASS record")
            validate_profile(profile.read_bytes(), self.checker)
        self.set_id = secrets.token_hex(32)
        self.slots = []
        for slot, order in enumerate(ORDERS):
            run_id = hashlib.sha256(f"{self.set_id}:{slot}".encode()).hexdigest()
            paths = []
            for backend in order:
                argv = ([UNIQUE_ID, "benchmark", self.commit, run_id] if backend == "kfd" else
                        [self.fixture, "0", UNIQUE_ID, self.commit, run_id])
                paths.append(self.phase(f"slot-{slot}-{backend}", backend, argv))
            checked = [self.checker.load_log(path) for path in paths]
            if tuple(log["config"]["backend"] for log in checked) != order:
                raise RunError("observed backend order differs from the Latin square")
            for log in checked:
                if log["config"]["source_commit"] != self.commit or log["config"]["run_id"] != run_id or log["config"]["unique_id"] != UNIQUE_ID[2:]:
                    raise RunError("benchmark records do not match this runner invocation")
            comparison = self.checker.compare(checked)
            write_json(self.evidence / f"comparison-{slot}.json", comparison)
            self.slots.append({"slot": slot, "run_id": run_id, "order": list(order)})
        identity_end = self.system_identity("end")
        for field in self.retained.R26_EXACT_SYSTEM_IDENTITY_FIELDS:
            if field not in self.retained.R26_EDGE_VARIANT_SYSTEM_IDENTITY_FIELDS and self.identity_start[field] != identity_end[field]:
                raise RunError(f"system identity changed: {field}")
        self.verify_source()

    def publish(self) -> pathlib.Path:
        artifacts = self.evidence / "binaries"
        artifacts.mkdir(mode=0o700)
        for backend, path in self.binaries.items():
            shutil.copy2(path, artifacts / backend)
        write_json(self.evidence / "commands.json", self.commands)
        write_json(self.evidence / "provenance.json", {
            "schema": "fe2o3.r60-pipeline-run.v1", "source_commit": self.commit,
            "source_archive_sha256": sha256_file(self.evidence / "source.tar"),
            "source_files_sha256": sha256_file(self.evidence / "source-files.json"),
            "snapshot_input_sha256": self.snapshot_input_hashes,
            "tool_identity": self.tool_hashes,
            "binary_sha256": self.binary_hashes,
            "allowed_signers_sha256": sha256_file(self.evidence / "allowed-signers"),
            "gpu_index": GPU_INDEX, "unique_id": UNIQUE_ID,
            "pci_bdf": self.identity_start["pci_bdf"], "topology": self.topology,
            "counterbalance_set_id": self.set_id, "slots": self.slots,
            "qualification_runs": 2, "measured_phases": 9,
            "claim_scope": "one-device-64-ordered-vecadd-host-visible-batch",
            "guard": "selected-kfd-gpu-process-tree-census-v2", "census_retry_policy": "abort-set",
        })
        write_json(self.evidence / "sha256.json", tree_hashes(self.evidence))
        destination = self.args.output_dir / f"r60-pipeline-{self.set_id}"
        if destination.exists() or destination.is_symlink():
            raise RunError("evidence destination already exists")
        pending = pathlib.Path(tempfile.mkdtemp(prefix=".r60-publish.", dir=self.args.output_dir))
        try:
            shutil.copytree(self.evidence, pending, dirs_exist_ok=True)
            manifest = json.loads((pending / "sha256.json").read_text())
            actual = tree_hashes(pending)
            del actual["sha256.json"]
            if actual != manifest:
                raise RunError("copied evidence does not match its manifest")
            readonly_tree(pending)
            pending.rename(destination)
        finally:
            cleanup_tree(pending)
        return destination


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=pathlib.Path,
                        default=pathlib.Path(__file__).resolve().parents[2])
    parser.add_argument("--allowed-signers", type=pathlib.Path, required=True)
    parser.add_argument("--output-dir", type=pathlib.Path, required=True)
    parser.add_argument("--rocm-path", type=pathlib.Path, default=pathlib.Path("/opt/rocm"))
    parser.add_argument("--build-home", type=pathlib.Path, default=pathlib.Path.home())
    parser.add_argument("--staging-parent", type=pathlib.Path, default=pathlib.Path("/tmp"))
    args = parser.parse_args(argv)
    for name in ("repo", "allowed_signers", "output_dir", "rocm_path", "build_home", "staging_parent"):
        setattr(args, name, getattr(args, name).resolve(strict=True))
    if not args.allowed_signers.is_file() or not 0 < args.allowed_signers.stat().st_size <= 65536:
        raise RunError("allowed-signers must be a nonempty bounded trusted file")
    if any(not path.is_dir() for path in (args.repo, args.output_dir, args.rocm_path,
                                         args.build_home, args.staging_parent)):
        raise RunError("repository, output, toolchain, home, and staging paths must be directories")
    if args.output_dir == args.repo or args.repo in args.output_dir.parents:
        raise RunError("output directory must be outside the source checkout")
    if args.staging_parent == args.repo or args.repo in args.staging_parent.parents:
        raise RunError("staging directory must be outside the source checkout")
    return args


def interrupted(signum: int, _frame: Any) -> None:
    raise RunError(f"runner interrupted by signal {signum}")


def main(argv: list[str] | None = None) -> int:
    os.umask(0o077)
    stage = None
    destination = None
    completed = False
    try:
        args = parse_args(argv)
        for signum in (signal.SIGHUP, signal.SIGINT, signal.SIGQUIT, signal.SIGTERM):
            signal.signal(signum, interrupted)
        stage = pathlib.Path(tempfile.mkdtemp(prefix="fe2o3-r60.", dir=args.staging_parent))
        runner = Runner(args, stage)
        runner.snapshot()
        runner.build()
        runner.qualify_and_measure()
        destination = runner.publish()
        cleanup_tree(stage)
        stage = None
        print(f"PASS R60 qualification and matched evidence: {destination}")
        completed = True
        return 0
    except Exception as error:
        print(f"R60 runner rejected: {error}", file=sys.stderr)
        return 2
    finally:
        try:
            if stage is not None:
                cleanup_tree(stage)
        finally:
            if destination is not None and not completed:
                cleanup_tree(destination)


if __name__ == "__main__":
    sys.exit(main())
