#!/usr/bin/python3.12
"""Reproducible gfx942 compiler-evidence controller.

This is a measurement controller, not a compiler-causality authenticator.  Its
tool configuration is repository-pinned and host-specific by design.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
import re
import resource
import shutil
import stat
import sys
import tempfile
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from compiler_evidence_hardening import (
    CommandLimits,
    HardeningError,
    RetainedClosure,
    RetainedFile,
    SealedExecutable,
    SnapshotClosure,
    Supervisor,
    adversarial_self_test,
    capture_snapshot,
    capture_retained_closure,
    compare_labeled_manifests,
)


SCHEMA = "fe2o3-gfx942-compiler-tool-manifest-v1"
GOLDEN_SCHEMA = "fe2o3-gfx942-alpha-zeta-compiler-evidence-v1"
EXPECTED_PLATFORM = "mi300x-rocm-7.2.4-ubuntu-24.04-x86_64"
EXPECTED_RUST_TOOLCHAIN = "nightly-2026-04-03"
EXPECTED_CARGO_RELEASE = "1.96.0-nightly"
EXPECTED_CARGO_COMMIT = "888f675344eb1cf2308fd53183e667bdd2c58e51"
EXPECTED_RUSTC_RELEASE = "1.96.0-nightly"
EXPECTED_RUSTC_COMMIT = "55e86c996809902e8bbad512cfb4d2c18be446d9"
EXPECTED_LLVM_BUILD = "7.2.4"
EXPECTED_LLVM_PACKAGE = "22.0.0git"
MAX_CONFIG_BYTES = 256 * 1024
MAX_TOOL_BYTES = 256 * 1024 * 1024
MAX_VERSION_BYTES = 64 * 1024
MAX_RUNTIME_FILES = 256
MAX_RUNTIME_BYTES = 512 * 1024 * 1024
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
TOOL_NAMES = (
    "shell",
    "python",
    "git",
    "cmake",
    "ctest",
    "ninja",
    "cxx",
    "lld",
    "llvm_ar",
    "cargo",
    "rustc",
    "ldd",
    "getconf",
    "basename",
    "cat",
    "chmod",
    "cp",
    "dirname",
    "env",
    "ln",
    "mkdir",
    "mv",
    "pwd",
    "rm",
    "sha256sum",
    "stat",
    "touch",
    "uname",
)
EXPECTED_ALIASES = {
    "ar": "llvm_ar",
    "bash": "shell",
    "basename": "basename",
    "c++": "cxx",
    "cargo": "cargo",
    "cat": "cat",
    "cc": "cxx",
    "chmod": "chmod",
    "clang": "cxx",
    "clang++": "cxx",
    "cmake": "cmake",
    "cp": "cp",
    "ctest": "ctest",
    "dirname": "dirname",
    "env": "env",
    "git": "git",
    "getconf": "getconf",
    "ld": "lld",
    "ld.lld": "lld",
    "ln": "ln",
    "lld": "lld",
    "llvm-ar": "llvm_ar",
    "llvm-ranlib": "llvm_ar",
    "mkdir": "mkdir",
    "mv": "mv",
    "ninja": "ninja",
    "python3": "python",
    "pwd": "pwd",
    "ranlib": "llvm_ar",
    "rm": "rm",
    "rustc": "rustc",
    "sh": "shell",
    "sha256sum": "sha256sum",
    "stat": "stat",
    "touch": "touch",
    "uname": "uname",
}


class EvidenceError(RuntimeError):
    pass


def canonical_json(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(file: Any, size: int) -> str:
    if size < 1 or size > MAX_TOOL_BYTES:
        raise EvidenceError("tool size is outside the configured bound")
    digest = hashlib.sha256()
    offset = 0
    while offset < size:
        chunk = os.pread(file.fileno(), min(1024 * 1024, size - offset), offset)
        if not chunk:
            raise EvidenceError("tool became truncated while hashing")
        digest.update(chunk)
        offset += len(chunk)
    return digest.hexdigest()


def exact_stat(value: os.stat_result) -> tuple[int, int, int, int, int, int, int]:
    return (
        value.st_dev,
        value.st_ino,
        value.st_mode,
        value.st_uid,
        value.st_gid,
        value.st_size,
        value.st_ctime_ns,
    )


def stat_record(value: os.stat_result) -> dict[str, int]:
    return {
        "device": value.st_dev,
        "inode": value.st_ino,
        "mode": value.st_mode & 0o7777,
        "uid": value.st_uid,
        "gid": value.st_gid,
        "bytes": value.st_size,
        "mtime_ns": value.st_mtime_ns,
        "ctime_ns": value.st_ctime_ns,
    }


def require_exact_keys(value: dict[str, Any], expected: set[str], label: str) -> None:
    if set(value) != expected:
        raise EvidenceError(f"{label} fields are not exact")


def read_bounded_json(path: Path) -> tuple[dict[str, Any], bytes]:
    if path.resolve(strict=True) != path or path.is_symlink():
        raise EvidenceError(f"configuration path is not canonical: {path}")
    data = path.read_bytes()
    if not data or len(data) > MAX_CONFIG_BYTES:
        raise EvidenceError(f"configuration size is invalid: {path}")
    try:
        value = json.loads(data)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"configuration is not canonical JSON: {path}") from error
    if not isinstance(value, dict):
        raise EvidenceError(f"configuration root is not an object: {path}")
    return value, data


def validate_manifest_document(manifest: dict[str, Any]) -> None:
    require_exact_keys(
        manifest,
        {
            "schema",
            "platform",
            "rust_toolchain",
            "cargo_release",
            "cargo_commit",
            "rustc_release",
            "rustc_commit",
            "llvm_package_version",
            "llvm_build_identity",
            "registry_source",
            "runtime_manifest_sha256",
            "tools",
            "path_aliases",
        },
        "tool manifest",
    )
    expected_scalars = {
        "schema": SCHEMA,
        "platform": EXPECTED_PLATFORM,
        "rust_toolchain": EXPECTED_RUST_TOOLCHAIN,
        "cargo_release": EXPECTED_CARGO_RELEASE,
        "cargo_commit": EXPECTED_CARGO_COMMIT,
        "rustc_release": EXPECTED_RUSTC_RELEASE,
        "rustc_commit": EXPECTED_RUSTC_COMMIT,
        "llvm_package_version": EXPECTED_LLVM_PACKAGE,
        "llvm_build_identity": EXPECTED_LLVM_BUILD,
    }
    for field, expected in expected_scalars.items():
        if manifest.get(field) != expected:
            raise EvidenceError(f"tool manifest {field} is not the required identity")
    if not SHA256_RE.fullmatch(str(manifest["runtime_manifest_sha256"])):
        raise EvidenceError("runtime closure identity is not SHA-256")
    registry = Path(str(manifest["registry_source"]))
    if not registry.is_absolute() or os.fspath(registry) != os.fspath(registry.resolve(strict=False)):
        raise EvidenceError("registry source path is not absolute and canonical")
    if manifest["path_aliases"] != EXPECTED_ALIASES:
        raise EvidenceError("allowlisted PATH aliases changed")
    tools = manifest["tools"]
    if not isinstance(tools, list) or [tool.get("name") for tool in tools] != list(TOOL_NAMES):
        raise EvidenceError("tool ordering or closure changed")
    for tool in tools:
        if not isinstance(tool, dict):
            raise EvidenceError("tool entry is not an object")
        require_exact_keys(
            tool,
            {
                "name",
                "path",
                "sha256",
                "bytes",
                "uid",
                "gid",
                "mode",
                "version_args",
                "version_output_sha256",
            },
            f"tool {tool.get('name')}",
        )
        path = Path(str(tool["path"]))
        if not path.is_absolute() or os.fspath(path) != os.fspath(path.resolve(strict=False)):
            raise EvidenceError(f"tool {tool['name']} path is not absolute and canonical")
        if not SHA256_RE.fullmatch(str(tool["sha256"])) or not SHA256_RE.fullmatch(
            str(tool["version_output_sha256"])
        ):
            raise EvidenceError(f"tool {tool['name']} identity is not SHA-256")
        if (
            not isinstance(tool["bytes"], int)
            or not 1 <= tool["bytes"] <= MAX_TOOL_BYTES
            or not isinstance(tool["uid"], int)
            or not isinstance(tool["gid"], int)
            or tool["mode"] != 0o755
            or not isinstance(tool["version_args"], list)
            or not tool["version_args"]
            or not all(isinstance(arg, str) and arg and "\0" not in arg for arg in tool["version_args"])
        ):
            raise EvidenceError(f"tool {tool['name']} metadata is invalid")


def validate_golden(golden: dict[str, Any], manifest_path: Path, manifest_bytes: bytes) -> None:
    if golden.get("schema") != GOLDEN_SCHEMA:
        raise EvidenceError("compiler-evidence golden schema changed")
    expected_relative = "tests/fixtures/compiler-evidence/gfx942-mi300x-tools.json"
    if golden.get("tool_manifest_path") != expected_relative:
        raise EvidenceError("compiler-evidence tool manifest path changed")
    if golden.get("tool_manifest_sha256") != sha256_bytes(manifest_bytes):
        raise EvidenceError("compiler-evidence tool manifest was substituted")
    if manifest_path.as_posix().split("/tests/fixtures/compiler-evidence/")[-1] != "gfx942-mi300x-tools.json":
        raise EvidenceError("unexpected tool manifest location")


@dataclass
class PinnedTool:
    record: dict[str, Any]
    file: Any
    identity: tuple[int, int, int, int, int, int, int]
    retained: RetainedFile
    executable: SealedExecutable

    @property
    def name(self) -> str:
        return str(self.record["name"])

    @property
    def path(self) -> Path:
        return Path(str(self.record["path"]))

    def revalidate(self) -> None:
        named = os.stat(self.path, follow_symlinks=False)
        opened = os.fstat(self.file.fileno())
        if exact_stat(named) != self.identity or exact_stat(opened) != self.identity:
            raise EvidenceError(f"pinned tool changed: {self.name}")
        if sha256_file(self.file, opened.st_size) != self.record["sha256"]:
            raise EvidenceError(f"pinned tool bytes changed: {self.name}")
        try:
            self.executable.revalidate()
        except HardeningError as error:
            raise EvidenceError(str(error)) from error

    def revalidate_identity(self) -> None:
        named = os.stat(self.path, follow_symlinks=False)
        opened = os.fstat(self.file.fileno())
        if exact_stat(named) != self.identity or exact_stat(opened) != self.identity:
            raise EvidenceError(f"pinned tool changed: {self.name}")
        try:
            self.executable.revalidate_identity()
        except HardeningError as error:
            raise EvidenceError(str(error)) from error

    def close(self) -> None:
        self.executable.close()
        self.retained.close()
        self.file.close()


def pin_tools(manifest: dict[str, Any]) -> dict[str, PinnedTool]:
    pinned: dict[str, PinnedTool] = {}
    try:
        for record in manifest["tools"]:
            path = Path(record["path"])
            if path.resolve(strict=True) != path or path.is_symlink():
                raise EvidenceError(f"tool path is not exact and canonical: {path}")
            named = os.stat(path, follow_symlinks=False)
            if not stat.S_ISREG(named.st_mode) or named.st_mode & 0o7777 != record["mode"]:
                raise EvidenceError(f"tool type or mode changed: {record['name']}")
            if (named.st_uid, named.st_gid, named.st_size) != (
                record["uid"],
                record["gid"],
                record["bytes"],
            ):
                raise EvidenceError(f"tool owner or size changed: {record['name']}")
            retained = RetainedFile.open(
                f"configured-tool:{record['name']}", path, require_executable=True
            )
            executable = SealedExecutable.from_retained(retained)
            file = open(path, "rb", buffering=0)
            opened = os.fstat(file.fileno())
            if exact_stat(named) != exact_stat(opened):
                executable.close()
                retained.close()
                file.close()
                raise EvidenceError(f"tool changed while opening: {record['name']}")
            tool = PinnedTool(record, file, exact_stat(opened), retained, executable)
            tool.revalidate()
            pinned[tool.name] = tool
    except BaseException:
        for tool in pinned.values():
            tool.close()
        raise
    return pinned


def create_allowlisted_path(directory: Path, manifest: dict[str, Any], tools: dict[str, PinnedTool]) -> None:
    directory.mkdir(mode=0o700)
    for alias, name in manifest["path_aliases"].items():
        target = tools[name].executable.proc_path
        os.symlink(target, directory / alias)
    actual = {entry.name: os.readlink(entry) for entry in directory.iterdir()}
    expected = {
        alias: tools[name].executable.proc_path
        for alias, name in manifest["path_aliases"].items()
    }
    if actual != expected:
        raise EvidenceError("allowlisted PATH materialization changed")


def clean_environment(run: Path, path_dir: Path, manifest: dict[str, Any]) -> dict[str, str]:
    home = run / "home"
    temp = run / "tmp"
    home.mkdir(mode=0o700)
    temp.mkdir(mode=0o700)
    target = run / "cargo-target"
    target.mkdir(mode=0o700)
    if any(target.iterdir()):
        raise EvidenceError("CARGO_TARGET_DIR was not empty at run start")
    return {
        "AR": os.fspath(path_dir / "ar"),
        "CARGO_HOME": os.fspath(run / "cargo-home"),
        "CARGO_NET_OFFLINE": "true",
        "CARGO_TARGET_DIR": os.fspath(target),
        "CC": os.fspath(path_dir / "cc"),
        "CXX": os.fspath(path_dir / "c++"),
        "HOME": os.fspath(home),
        "LANG": "C",
        "LC_ALL": "C",
        "LD": os.fspath(path_dir / "ld.lld"),
        "LD_LIBRARY_PATH": os.fspath(run / "rust-sysroot/lib"),
        "PATH": os.fspath(path_dir),
        "RANLIB": os.fspath(path_dir / "ranlib"),
        "RUSTC": os.fspath(path_dir / "rustc"),
        "RUSTFLAGS": f"--sysroot={run / 'rust-sysroot'}",
        "RUSTUP_TOOLCHAIN": EXPECTED_RUST_TOOLCHAIN,
        "SOURCE_DATE_EPOCH": "0",
        "TERM": "dumb",
        "TMPDIR": os.fspath(temp),
        "TZ": "UTC",
    }


def run_command(
    arguments: list[str],
    cwd: Path,
    environment: dict[str, str],
    tools: dict[str, PinnedTool],
    supervisor: Supervisor,
    *,
    capture: bool = False,
    executable: SealedExecutable | None = None,
    extra_inherited_fds: tuple[int, ...] = (),
    limits: CommandLimits = CommandLimits(),
) -> Any:
    for tool in tools.values():
        tool.revalidate_identity()
    selected = executable
    command = list(arguments)
    if selected is None:
        first = Path(arguments[0])
        matched = next(
            (
                tool
                for tool in tools.values()
                if first == tool.path
                or os.fspath(first) == tool.executable.proc_path
                or os.fspath(first) == f"/proc/self/fd/{tool.executable.fd}"
            ),
            None,
        )
        if matched is None:
            raise EvidenceError(f"command executable is not retained: {arguments[0]}")
        if matched.name == "ldd":
            selected = tools["shell"].executable
            command = ["bash", matched.executable.proc_path, *arguments[1:]]
        else:
            selected = matched.executable
    inherited = [tool.executable.fd for tool in tools.values()]
    inherited.extend(extra_inherited_fds)
    try:
        completed = supervisor.run(
            selected,
            command,
            cwd,
            environment,
            limits=limits,
            inherited_fds=inherited,
        )
    except HardeningError as error:
        raise EvidenceError(str(error)) from error
    for tool in tools.values():
        tool.revalidate_identity()
    if completed.returncode != 0:
        detail = b""
        if capture:
            detail = completed.stdout + completed.stderr
        raise EvidenceError(
            f"command failed ({completed.returncode}): {' '.join(arguments)}\n"
            + detail[-8192:].decode("utf-8", "replace")
        )
    return completed


def version_and_runtime_manifest(
    manifest: dict[str, Any],
    tools: dict[str, PinnedTool],
    path_dir: Path,
    rust_library_path: Path,
    supervisor: Supervisor,
) -> tuple[dict[str, Any], RetainedClosure]:
    environment = {
        "LANG": "C",
        "LC_ALL": "C",
        "LD_LIBRARY_PATH": os.fspath(rust_library_path),
        "PATH": os.fspath(path_dir),
        "TZ": "UTC",
    }
    observed_tools: list[dict[str, Any]] = []
    runtime_paths: set[Path] = set()
    ldd = tools["ldd"].path
    for name in TOOL_NAMES:
        tool = tools[name]
        completed = run_command(
            [os.fspath(tool.path), *tool.record["version_args"]],
            Path("/"),
            environment,
            tools,
            supervisor,
            capture=True,
        )
        output = (completed.stdout or b"") + (completed.stderr or b"")
        if not output or len(output) > MAX_VERSION_BYTES:
            raise EvidenceError(f"version output is invalid: {name}")
        output_digest = sha256_bytes(output)
        if output_digest != tool.record["version_output_sha256"]:
            raise EvidenceError(
                f"version output changed: {name}: expected "
                f"{tool.record['version_output_sha256']}, observed {output_digest}; "
                f"output={output.decode('utf-8', 'replace')!r}"
            )
        observed_tools.append(
            {
                "name": name,
                "path": os.fspath(tool.path),
                "sha256": tool.record["sha256"],
                "bytes": tool.record["bytes"],
                "uid": tool.record["uid"],
                "gid": tool.record["gid"],
                "mode": tool.record["mode"],
                "version_args": tool.record["version_args"],
                "version_output": output.decode("utf-8", "strict"),
                "version_output_sha256": output_digest,
                "stat": stat_record(os.fstat(tool.file.fileno())),
            }
        )
        data = os.pread(tool.file.fileno(), 128, 0)
        if data.startswith(b"\x7fELF"):
            closure = run_command(
                [os.fspath(ldd), os.fspath(tool.path)],
                Path("/"),
                environment,
                tools,
                supervisor,
                capture=True,
            )
            for line in (closure.stdout or b"").decode("utf-8", "strict").splitlines():
                match = re.search(r"=>\s+(/\S+)", line)
                direct = re.match(r"\s*(/\S+)\s+\(", line)
                candidate = match.group(1) if match else direct.group(1) if direct else None
                if candidate is not None:
                    runtime_paths.add(Path(candidate).resolve(strict=True))
        elif data.startswith(b"#!"):
            interpreter = data.splitlines()[0][2:].decode("ascii").split()[0]
            runtime_paths.add(Path(interpreter).resolve(strict=True))
        else:
            raise EvidenceError(f"tool is neither ELF nor a script: {name}")
    if not runtime_paths or len(runtime_paths) > MAX_RUNTIME_FILES:
        raise EvidenceError("runtime closure file count is invalid")
    runtime: list[dict[str, Any]] = []
    total = 0
    for path in sorted(runtime_paths, key=os.fspath):
        if path.is_symlink() or path.resolve(strict=True) != path:
            raise EvidenceError(f"runtime path is not canonical: {path}")
        metadata = os.stat(path, follow_symlinks=False)
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_size < 1:
            raise EvidenceError(f"runtime closure member is invalid: {path}")
        total += metadata.st_size
        if total > MAX_RUNTIME_BYTES:
            raise EvidenceError("runtime closure exceeds byte bound")
        with open(path, "rb", buffering=0) as file:
            digest = sha256_file(file, metadata.st_size)
        runtime.append(
            {
                "path": os.fspath(path),
                "sha256": digest,
                "bytes": metadata.st_size,
                "uid": metadata.st_uid,
                "gid": metadata.st_gid,
                "mode": metadata.st_mode & 0o7777,
                "device": metadata.st_dev,
                "inode": metadata.st_ino,
                "mtime_ns": metadata.st_mtime_ns,
                "ctime_ns": metadata.st_ctime_ns,
            }
        )
    runtime_digest = sha256_bytes(canonical_json(runtime))
    if runtime_digest != manifest["runtime_manifest_sha256"]:
        raise EvidenceError(
            "pinned runtime closure changed: "
            f"expected {manifest['runtime_manifest_sha256']}, observed {runtime_digest}"
        )
    observed = {
        "schema": "fe2o3-observed-gfx942-tool-runtime-manifest-v1",
        "configured_manifest_sha256": sha256_bytes(canonical_json(manifest)),
        "runtime_manifest_sha256": runtime_digest,
        "tools": observed_tools,
        "runtime": runtime,
    }
    retained_runtime = capture_retained_closure(
        "configured-tool-runtime",
        [(f"runtime:{path.as_posix()}", path) for path in sorted(runtime_paths, key=os.fspath)],
        {"runtime_manifest_sha256": runtime_digest},
    )
    return observed, retained_runtime


def require_absent_output(path: Path, label: str) -> None:
    if not path.is_absolute() or path.exists() or path.is_symlink():
        raise EvidenceError(f"{label} must be an absent absolute path")
    parent = path.parent.resolve(strict=True)
    if parent / path.name != path:
        raise EvidenceError(f"{label} parent or spelling is not canonical")


def git_clean(
    repo: Path,
    environment: dict[str, str],
    tools: dict[str, PinnedTool],
    supervisor: Supervisor,
) -> tuple[str, str, list[str]]:
    git = os.fspath(tools["git"].path)
    status = run_command(
        [git, "-c", "core.hooksPath=/dev/null", "status", "--porcelain=v1", "--untracked-files=all"],
        repo,
        environment,
        tools,
        supervisor,
        capture=True,
    ).stdout
    if status:
        raise EvidenceError("compiler evidence requires a clean committed tree")
    commit = run_command(
        [git, "rev-parse", "HEAD"], repo, environment, tools, supervisor, capture=True
    ).stdout.decode("ascii").strip()
    tree = run_command(
        [git, "rev-parse", "HEAD^{tree}"], repo, environment, tools, supervisor, capture=True
    ).stdout.decode("ascii").strip()
    listed = run_command(
        [git, "ls-files", "-z"], repo, environment, tools, supervisor, capture=True
    ).stdout
    if not re.fullmatch(r"[0-9a-f]{40}", commit) or not re.fullmatch(r"[0-9a-f]{40}", tree):
        raise EvidenceError("source commit identity is invalid")
    paths = [item.decode("utf-8", "strict") for item in listed.split(b"\0") if item]
    if not paths or len(paths) != len(set(paths)):
        raise EvidenceError("tracked source list is empty or duplicated")
    return commit, tree, paths


def measure_generated_executable(path: Path, parent: Path, label: str) -> tuple[Any, dict[str, Any]]:
    if path.is_symlink() or path.resolve(strict=True) != path:
        raise EvidenceError(f"{label} path is not absolute and canonical")
    if parent.resolve(strict=True) not in path.parents:
        raise EvidenceError(f"{label} escaped its isolated run root")
    named = os.stat(path, follow_symlinks=False)
    if not stat.S_ISREG(named.st_mode) or not named.st_mode & 0o111:
        raise EvidenceError(f"{label} is not an executable regular file")
    file = open(path, "rb", buffering=0)
    opened = os.fstat(file.fileno())
    if exact_stat(named) != exact_stat(opened):
        file.close()
        raise EvidenceError(f"{label} changed while it was opened")
    return file, {
        "label": label,
        "path": os.fspath(path),
        "sha256": sha256_file(file, opened.st_size),
        "stat": stat_record(opened),
    }


def revalidate_generated_executable(file: Any, record: dict[str, Any]) -> None:
    path = Path(record["path"])
    named = os.stat(path, follow_symlinks=False)
    opened = os.fstat(file.fileno())
    if stat_record(named) != record["stat"] or stat_record(opened) != record["stat"]:
        raise EvidenceError(f"generated executable stat identity changed: {record['label']}")
    if sha256_file(file, opened.st_size) != record["sha256"]:
        raise EvidenceError(f"generated executable content changed: {record['label']}")


def reject_cross_run_reuse(first: dict[str, Any], second: dict[str, Any]) -> None:
    first_records = first.get("executables")
    second_records = second.get("executables")
    if (
        not isinstance(first_records, list)
        or not isinstance(second_records, list)
        or len(first_records) != len(second_records)
        or not first_records
    ):
        raise EvidenceError("cross-run executable closures are not comparable")
    for left, right in zip(first_records, second_records, strict=True):
        left_stat = left["stat"]
        right_stat = right["stat"]
        if left["path"] == right["path"] or (
            left_stat["device"] == right_stat["device"]
            and left_stat["inode"] == right_stat["inode"]
        ):
            raise EvidenceError("run B reused an executable from run A")


def registry_paths(lock_path: Path, registry_root: Path) -> list[str]:
    lock = tomllib.loads(lock_path.read_text("utf-8"))
    packages = lock.get("package")
    if not isinstance(packages, list):
        raise EvidenceError("Cargo.lock has no package closure")
    paths: list[str] = []
    for package in packages:
        source = package.get("source")
        if not isinstance(source, str) or not source.startswith("registry+"):
            continue
        name = package.get("name")
        version = package.get("version")
        if not isinstance(name, str) or not isinstance(version, str):
            raise EvidenceError("Cargo.lock registry package identity is invalid")
        package_root = registry_root / f"{name}-{version}"
        if not package_root.exists():
            continue
        if package_root.resolve(strict=True) != package_root or package_root.is_symlink():
            raise EvidenceError(f"registry package is not canonical: {name}-{version}")
        members = sorted(path for path in package_root.rglob("*") if path.is_file())
        if not members:
            raise EvidenceError(f"registry package closure is incomplete: {name}-{version}")
        for member in members:
            if member.is_symlink() or member.resolve(strict=True) != member:
                raise EvidenceError(f"registry package member is not canonical: {member}")
            paths.append(member.relative_to(registry_root).as_posix())
    if not paths:
        raise EvidenceError("Cargo.lock selected no registry package sources")
    return sorted(set(paths))


def materialize_vendor_checksums(lock_path: Path, vendor: Path, index: int) -> RetainedClosure:
    packages = tomllib.loads(lock_path.read_text("utf-8")).get("package")
    if not isinstance(packages, list):
        raise EvidenceError("Cargo.lock has no package closure")
    generated: list[tuple[str, Path]] = []
    for package in packages:
        source = package.get("source")
        if not isinstance(source, str) or not source.startswith("registry+"):
            continue
        name = package.get("name")
        version = package.get("version")
        checksum = package.get("checksum")
        if (
            not isinstance(name, str)
            or not isinstance(version, str)
            or not isinstance(checksum, str)
            or not SHA256_RE.fullmatch(checksum)
        ):
            raise EvidenceError("Cargo.lock registry checksum identity is invalid")
        package_root = vendor / f"{name}-{version}"
        if not package_root.exists():
            continue
        files: dict[str, str] = {}
        for member in sorted(path for path in package_root.rglob("*") if path.is_file()):
            relative = member.relative_to(package_root).as_posix()
            if relative == ".cargo-checksum.json":
                raise EvidenceError("vendor checksum input was preexisting")
            files[relative] = sha256_bytes(member.read_bytes())
        package_root.chmod(0o755)
        checksum_path = package_root / ".cargo-checksum.json"
        checksum_path.write_bytes(canonical_json({"files": files, "package": checksum}))
        checksum_path.chmod(0o444)
        package_root.chmod(0o555)
        generated.append((f"vendor-checksum:{name}-{version}", checksum_path))
    if not generated:
        raise EvidenceError("no vendor checksum manifests were generated")
    return capture_retained_closure(
        f"run-{index}-vendor-generated-inputs",
        generated,
        {"cargo_lock_sha256": sha256_bytes(lock_path.read_bytes())},
    )


def prepare_run_closures(
    index: int,
    repo: Path,
    run_root: Path,
    evidence_root: Path,
    tracked_paths: list[str],
    commit: str,
    tree: str,
    manifest: dict[str, Any],
) -> tuple[
    Path,
    SnapshotClosure,
    SnapshotClosure,
    SnapshotClosure,
    RetainedClosure,
    RetainedFile,
]:
    run = run_root / f"run-{index}"
    if run.exists() or run.is_symlink():
        raise EvidenceError(f"run {index} work root was not absent")
    run.mkdir(mode=0o700)
    source = capture_snapshot(
        f"run-{index}-repository",
        repo,
        run / "source",
        tracked_paths,
        {"git_commit": commit, "git_tree": tree},
    )
    cargo_home = run / "cargo-home"
    cargo_home.mkdir(mode=0o700)
    registry_root = Path(str(manifest["registry_source"]))
    registry = capture_snapshot(
        f"run-{index}-registry",
        registry_root,
        cargo_home / "vendor",
        registry_paths(source.root / "Cargo.lock", registry_root),
        {
            "cargo_lock_sha256": sha256_bytes((source.root / "Cargo.lock").read_bytes()),
            "registry_source": os.fspath(registry_root),
        },
    )
    vendor_generated = materialize_vendor_checksums(
        source.root / "Cargo.lock", cargo_home / "vendor", index
    )
    rust_root = Path(
        "/home/harsh/.rustup/toolchains/nightly-2026-04-03-x86_64-unknown-linux-gnu"
    )
    if rust_root.resolve(strict=True) != rust_root:
        raise EvidenceError("nightly-2026-04-03 sysroot is not canonical")
    rust_paths = sorted(
        member.relative_to(rust_root).as_posix()
        for member in rust_root.rglob("*")
        if member.is_file()
    )
    rust = capture_snapshot(
        f"run-{index}-rust-sysroot",
        rust_root,
        run / "rust-sysroot",
        rust_paths,
        {
            "rust_toolchain": EXPECTED_RUST_TOOLCHAIN,
            "rustc_sha256": next(
                tool["sha256"] for tool in manifest["tools"] if tool["name"] == "rustc"
            ),
        },
    )
    cargo_config = cargo_home / "config.toml"
    cargo_config.write_text(
        "[net]\noffline = true\n\n"
        "[source.crates-io]\nreplace-with = \"vendored-sources\"\n\n"
        "[source.vendored-sources]\n"
        f'directory = "{cargo_home / "vendor"}"\n',
        encoding="ascii",
    )
    cargo_config.chmod(0o444)
    config = RetainedFile.open(
        f"run-{index}-cargo-config", cargo_config, require_read_only=True
    )
    output_dir = evidence_root / f"run-{index}"
    output_dir.mkdir(mode=0o700)
    (output_dir / "repository-source-manifest.json").write_bytes(
        canonical_json(source.manifest)
    )
    (output_dir / "cargo-registry-manifest.json").write_bytes(
        canonical_json(registry.manifest)
    )
    (output_dir / "rust-sysroot-manifest.json").write_bytes(
        canonical_json(rust.manifest)
    )
    (output_dir / "cargo-vendor-generated-manifest.json").write_bytes(
        canonical_json(vendor_generated.manifest)
    )
    return run, source, registry, rust, vendor_generated, config


def build_provider_members() -> list[tuple[str, Path]]:
    roots = (
        ("llvm-include", Path("/opt/rocm-7.2.4/lib/llvm/include")),
        ("llvm-cmake", Path("/opt/rocm-7.2.4/lib/llvm/lib/cmake")),
        ("clang-resource", Path("/opt/rocm-7.2.4/lib/llvm/lib/clang/22/include")),
        (
            "device-bitcode",
            Path("/opt/rocm-7.2.4/lib/llvm/lib/clang/22/lib/amdgcn/bitcode"),
        ),
        ("rocm-info", Path("/opt/rocm-7.2.4/.info")),
    )
    members: list[tuple[str, Path]] = []
    for prefix, root in roots:
        if root.resolve(strict=True) != root or root.is_symlink():
            raise EvidenceError(f"build-provider root is not canonical: {root}")
        for member in sorted(root.rglob("*")):
            if member.is_file() and not member.is_symlink():
                members.append((f"{prefix}:{member.relative_to(root).as_posix()}", member))
    library_root = Path("/opt/rocm-7.2.4/lib/llvm/lib")
    for member in sorted(library_root.iterdir()):
        if member.is_file() and not member.is_symlink():
            members.append((f"llvm-library:{member.name}", member))
    return members


def build_and_generate(
    index: int,
    repo: Path,
    run: Path,
    evidence_root: Path,
    manifest: dict[str, Any],
    tools: dict[str, PinnedTool],
    golden: dict[str, Any],
    supervisor: Supervisor,
    *,
    observe_candidate: bool,
) -> tuple[Path, dict[str, Any]]:
    if run != run.parent / f"run-{index}" or not run.is_dir():
        raise EvidenceError("run root does not match its independent index")
    path_dir = run / "tool-path"
    create_allowlisted_path(path_dir, manifest, tools)
    environment = clean_environment(run, path_dir, manifest)
    worker_build = run / "worker-build"
    if worker_build.exists():
        raise EvidenceError("Worker build directory was not absent at run start")
    cmake = os.fspath(tools["cmake"].path)
    compiler_launcher = ";".join(
        (
            tools["python"].executable.proc_path,
            os.fspath(repo / "scripts/retained_tool_launcher.py"),
            "--expected",
            os.fspath(tools["cxx"].path),
            "--retained",
            tools["cxx"].executable.proc_path,
            "--sha256",
            tools["cxx"].record["sha256"],
        )
    )
    run_command(
        [
            cmake,
            "-S",
            os.fspath(repo / "tools/fe2o3-llvm-link-worker"),
            "-B",
            os.fspath(worker_build),
            "-G",
            "Ninja",
            "-DLLVM_DIR=/opt/rocm-7.2.4/lib/llvm/lib/cmake/llvm",
            "-DLLD_DIR=/opt/rocm-7.2.4/lib/llvm/lib/cmake/lld",
            f"-DCMAKE_MAKE_PROGRAM={tools['ninja'].executable.proc_path}",
            f"-DCMAKE_CXX_COMPILER={tools['cxx'].path}",
            "-DCMAKE_CXX_COMPILER_ARG1=--driver-mode=g++",
            f"-DCMAKE_CXX_COMPILER_LAUNCHER={compiler_launcher}",
            f"-DCMAKE_CXX_LINKER_LAUNCHER={compiler_launcher}",
            f"-DCMAKE_LINKER={tools['lld'].executable.proc_path}",
            f"-DCMAKE_AR={tools['llvm_ar'].executable.proc_path}",
            "-DFE2O3_PINNED_LLVM_VERSION=22.0.0git",
            "-DFE2O3_LLVM_BUILD_ID_FILE=/opt/rocm-7.2.4/.info/version",
            "-DFE2O3_EXPECTED_LLVM_BUILD_ID=7.2.4",
            "-DFE2O3_GFX942_DEVICE_LIB_DIR="
            "/opt/rocm-7.2.4/lib/llvm/lib/clang/22/lib/amdgcn/bitcode",
            "-DBUILD_TESTING=ON",
            "-DCMAKE_BUILD_TYPE=Release",
        ],
        repo,
        environment,
        tools,
        supervisor,
    )
    run_command(
        [cmake, "--build", os.fspath(worker_build), "--parallel", "8"],
        repo,
        environment,
        tools,
        supervisor,
    )
    worker = worker_build / "fe2o3-llvm-link-worker"
    generated: list[tuple[Any, dict[str, Any]]] = []
    worker_file, worker_record = measure_generated_executable(
        worker, worker_build, f"run-{index} Worker"
    )
    generated.append((worker_file, worker_record))
    if not observe_candidate and worker_record["sha256"] != golden["worker_executable_sha256"]:
        raise EvidenceError(f"run {index} Worker executable digest changed")
    ctest_listing = run_command(
        [os.fspath(tools["ctest"].path), "--test-dir", os.fspath(worker_build), "--show-only=json-v1"],
        repo,
        environment,
        tools,
        supervisor,
        capture=True,
    )
    listing = json.loads((ctest_listing.stdout or b"").decode("utf-8"))
    for test in listing.get("tests", []):
        command = test.get("command", [])
        if not command:
            raise EvidenceError("CTest listed a test without an executable")
        executable = Path(command[0])
        file, record = measure_generated_executable(
            executable, worker_build, f"run-{index} native test {test.get('name')}"
        )
        generated.append((file, record))
    run_command(
        [os.fspath(tools["ctest"].path), "--test-dir", os.fspath(worker_build), "--output-on-failure"],
        repo,
        environment,
        tools,
        supervisor,
    )
    for file, record in generated:
        revalidate_generated_executable(file, record)
    build_identity = (worker_build / "fe2o3-worker-build-id.txt").read_text("ascii").strip()
    if not observe_candidate and build_identity != golden["worker_build_identity"]:
        raise EvidenceError(f"run {index} Worker build identity changed")
    output_dir = evidence_root / f"run-{index}"
    if not output_dir.is_dir():
        raise EvidenceError("run evidence root was not prepared independently")
    output = output_dir / "alpha-zeta-cov6.hsaco"
    generation_environment = dict(environment)
    generation_environment.update(
        {
            "FE2O3_GFX942_ALPHA_ZETA_OUTPUT": os.fspath(output),
            "FE2O3_LLVM_BUILD_ID": EXPECTED_LLVM_BUILD,
            "FE2O3_LLVM_LINK_WORKER": os.fspath(worker),
            "FE2O3_LLVM_LINK_WORKER_BUILD_ID": build_identity,
        }
    )
    build_test = run_command(
        [
            os.fspath(tools["cargo"].path),
            "test",
            "--offline",
            "--locked",
            "-p",
            "rustc-codegen-fe2o3",
            "--test",
            "kernel_ir_codegen",
            "--no-run",
            "--message-format=json",
        ],
        repo,
        environment,
        tools,
        supervisor,
        capture=True,
    )
    test_executables: list[Path] = []
    for line in (build_test.stdout or b"").splitlines():
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        target = message.get("target", {})
        executable = message.get("executable")
        if target.get("name") == "kernel_ir_codegen" and target.get("kind") == ["test"] and executable:
            test_executables.append(Path(executable))
    if len(test_executables) != 1:
        raise EvidenceError(f"run {index} did not produce exactly one integration-test executable")
    test_file, test_record = measure_generated_executable(
        test_executables[0], run / "cargo-target", f"run-{index} Rust integration test"
    )
    generated.append((test_file, test_record))
    retained_worker = RetainedFile.open(
        f"run-{index}-worker-exec", worker, require_executable=True
    )
    sealed_worker = SealedExecutable.from_retained(retained_worker)
    retained_test = RetainedFile.open(
        f"run-{index}-rust-integration-test",
        test_executables[0],
        require_executable=True,
    )
    sealed_test = SealedExecutable.from_retained(retained_test)
    generation_environment["FE2O3_LLVM_LINK_WORKER"] = sealed_worker.proc_path
    try:
        run_command(
            [
                os.fspath(test_executables[0]),
                golden["generator_test"],
                "--ignored",
                "--exact",
                "--nocapture",
            ],
            repo,
            generation_environment,
            tools,
            supervisor,
            executable=sealed_test,
            extra_inherited_fds=(sealed_worker.fd,),
        )
        retained_worker.revalidate()
        retained_test.revalidate()
    finally:
        sealed_test.close()
        retained_test.close()
        sealed_worker.close()
        retained_worker.close()
    for file, record in generated:
        revalidate_generated_executable(file, record)
    data = output.read_bytes()
    if len(data) != golden["hsaco_bytes"] or sha256_bytes(data) != golden["hsaco_sha256"]:
        raise EvidenceError(f"run {index} HSACO identity changed")
    executable_manifest = {
        "schema": "fe2o3-gfx942-run-executable-manifest-v1",
        "run": index,
        "executables": [record for _, record in generated],
    }
    (output_dir / "executables.json").write_bytes(canonical_json(executable_manifest))
    for file, _ in generated:
        file.close()
    return output, executable_manifest


def controller(run_root: Path, evidence_root: Path, *, observe_candidate: bool) -> None:
    require_absent_output(run_root, "run root")
    require_absent_output(evidence_root, "evidence root")
    script = Path(__file__)
    if script.resolve(strict=True) != script or script.is_symlink():
        raise EvidenceError("controller must execute by its canonical repository path")
    repo = script.parent.parent
    manifest_path = repo / "tests/fixtures/compiler-evidence/gfx942-mi300x-tools.json"
    golden_path = repo / "tests/fixtures/compiler-evidence/gfx942-alpha-zeta-cov6.json"
    manifest, manifest_bytes = read_bounded_json(manifest_path)
    golden, _ = read_bounded_json(golden_path)
    validate_manifest_document(manifest)
    validate_golden(golden, manifest_path, manifest_bytes)
    soft_files, hard_files = resource.getrlimit(resource.RLIMIT_NOFILE)
    if hard_files < 131072:
        raise EvidenceError("compiler evidence requires an RLIMIT_NOFILE hard limit of 131072")
    if soft_files < 131072:
        resource.setrlimit(resource.RLIMIT_NOFILE, (131072, hard_files))
    tools = pin_tools(manifest)
    closures: list[SnapshotClosure] = []
    retained_closures: list[RetainedClosure] = []
    generated_inputs: list[RetainedFile] = []
    try:
        supervisor = Supervisor()
        run_root.mkdir(mode=0o700)
        evidence_root.mkdir(mode=0o700)
        bootstrap_path = run_root / "bootstrap-tool-path"
        create_allowlisted_path(bootstrap_path, manifest, tools)
        bootstrap_environment = {
            "LANG": "C",
            "LC_ALL": "C",
            "PATH": os.fspath(bootstrap_path),
            "TZ": "UTC",
        }
        commit, tree, tracked_paths = git_clean(
            repo, bootstrap_environment, tools, supervisor
        )
        prepared = []
        for index in (1, 2):
            run, source, registry, rust, vendor_generated, cargo_config = prepare_run_closures(
                index,
                repo,
                run_root,
                evidence_root,
                tracked_paths,
                commit,
                tree,
                manifest,
            )
            prepared.append((run, source))
            closures.extend((source, registry, rust))
            retained_closures.append(vendor_generated)
            supervisor.guards.extend(vendor_generated.files)
            generated_inputs.append(cargo_config)
        compare_labeled_manifests(closures[0].manifest, closures[3].manifest)
        compare_labeled_manifests(closures[1].manifest, closures[4].manifest)
        compare_labeled_manifests(closures[2].manifest, closures[5].manifest)
        compare_labeled_manifests(
            retained_closures[0].manifest, retained_closures[1].manifest
        )
        provider_members = build_provider_members()
        for index in (1, 2):
            provider = capture_retained_closure(
                f"run-{index}-llvm-rocm-provider",
                provider_members,
                {
                    "llvm_package_version": EXPECTED_LLVM_PACKAGE,
                    "llvm_build_identity": EXPECTED_LLVM_BUILD,
                    "rocm_root": "/opt/rocm-7.2.4",
                },
            )
            retained_closures.append(provider)
            supervisor.guards.extend(provider.files)
            (evidence_root / f"run-{index}/llvm-rocm-provider-manifest.json").write_bytes(
                canonical_json(provider.manifest)
            )
        compare_labeled_manifests(
            retained_closures[2].manifest, retained_closures[3].manifest
        )
        for closure in closures:
            closure.revalidate()
        observed, runtime_closure = version_and_runtime_manifest(
            manifest,
            tools,
            bootstrap_path,
            run_root / "run-1/rust-sysroot/lib",
            supervisor,
        )
        retained_closures.append(runtime_closure)
        supervisor.guards.extend(runtime_closure.files)
        (evidence_root / "tool-runtime-manifest.json").write_bytes(canonical_json(observed))
        first, first_executables = build_and_generate(
            1,
            prepared[0][1].root,
            prepared[0][0],
            evidence_root,
            manifest,
            tools,
            golden,
            supervisor,
            observe_candidate=observe_candidate,
        )
        for closure in closures:
            closure.revalidate()
        git_clean(repo, bootstrap_environment, tools, supervisor)
        second, second_executables = build_and_generate(
            2,
            prepared[1][1].root,
            prepared[1][0],
            evidence_root,
            manifest,
            tools,
            golden,
            supervisor,
            observe_candidate=observe_candidate,
        )
        for closure in closures:
            closure.revalidate()
        git_clean(repo, bootstrap_environment, tools, supervisor)
        if first.read_bytes() != second.read_bytes():
            raise EvidenceError("independent Worker/build/target runs were not byte-identical")
        reject_cross_run_reuse(first_executables, second_executables)
        for closure in closures:
            closure.revalidate()
        for closure in retained_closures:
            closure.revalidate()
        for tool in tools.values():
            tool.revalidate()
        summary = {
            "schema": "fe2o3-gfx942-two-run-compiler-evidence-summary-v1",
            "source_commit": commit,
            "source_tree": tree,
            "tool_manifest_sha256": sha256_bytes(manifest_bytes),
            "runtime_manifest_sha256": manifest["runtime_manifest_sha256"],
            "run_1": {
                "worker_build": os.fspath(run_root / "run-1/worker-build"),
                "cargo_target": os.fspath(run_root / "run-1/cargo-target"),
                "hsaco_sha256": sha256_bytes(first.read_bytes()),
            },
            "run_2": {
                "worker_build": os.fspath(run_root / "run-2/worker-build"),
                "cargo_target": os.fspath(run_root / "run-2/cargo-target"),
                "hsaco_sha256": sha256_bytes(second.read_bytes()),
            },
            "claim": "exact-artifact-observation-only",
            "compiler_causality_authenticated": False,
            "compiler_receipt_issued": False,
            "transition_candidate_observation": observe_candidate,
        }
        (evidence_root / "summary.json").write_bytes(canonical_json(summary))
        print(f"source commit: {commit}")
        print(f"artifact SHA-256: {golden['hsaco_sha256']}")
        print("independent pinned-tool Worker V2 compiler evidence: PASS")
    finally:
        for generated in generated_inputs:
            generated.close()
        for closure in closures:
            closure.close()
        for closure in retained_closures:
            closure.close()
        for tool in tools.values():
            tool.close()


def self_test(repo: Path) -> None:
    manifest_path = repo / "tests/fixtures/compiler-evidence/gfx942-mi300x-tools.json"
    golden_path = repo / "tests/fixtures/compiler-evidence/gfx942-alpha-zeta-cov6.json"
    manifest, manifest_bytes = read_bounded_json(manifest_path)
    golden, _ = read_bounded_json(golden_path)
    validate_manifest_document(manifest)
    validate_golden(golden, manifest_path, manifest_bytes)
    for label, mutate in (
        ("cargo path", lambda value: value["tools"][9].__setitem__("path", value["tools"][10]["path"])),
        ("rustc digest", lambda value: value["tools"][10].__setitem__("sha256", "0" * 64)),
        ("cargo version", lambda value: value.__setitem__("cargo_commit", "0" * 40)),
        ("runtime closure", lambda value: value.__setitem__("runtime_manifest_sha256", "0" * 64)),
        ("PATH alias", lambda value: value["path_aliases"].__setitem__("cargo", "rustc")),
    ):
        changed = copy.deepcopy(manifest)
        mutate(changed)
        try:
            validate_manifest_document(changed)
            # Shape-valid content substitutions are rejected by the golden digest.
            validate_golden(golden, manifest_path, canonical_json(changed))
        except EvidenceError:
            pass
        else:
            raise AssertionError(f"{label} substitution was accepted")
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        for name in ("run", "evidence"):
            path = root / name
            path.mkdir()
            try:
                require_absent_output(path, name)
            except EvidenceError:
                pass
            else:
                raise AssertionError(f"preexisting {name} directory was accepted")
        run_a = root / "fresh-a"
        run_b = root / "fresh-b"
        run_a.mkdir()
        run_b.mkdir()
        (run_a / "cargo-target").mkdir()
        (run_b / "cargo-target").mkdir()
        (run_a / "worker-build").mkdir()
        (run_b / "worker-build").mkdir()
        if (
            (run_a / "cargo-target").resolve() == (run_b / "cargo-target").resolve()
            or (run_a / "worker-build").resolve() == (run_b / "worker-build").resolve()
        ):
            raise AssertionError("cross-run build-root reuse was accepted")
        reused = {
            "executables": [
                {"path": "/run-a/test", "stat": {"device": 1, "inode": 2}}
            ]
        }
        substituted = {
            "executables": [
                {"path": "/run-b/test", "stat": {"device": 1, "inode": 2}}
            ]
        }
        try:
            reject_cross_run_reuse(reused, substituted)
        except EvidenceError:
            pass
        else:
            raise AssertionError("cross-run executable reuse was accepted")
        tools = pin_tools(manifest)
        try:
            allowlist = root / "allowlist"
            shadow = root / "shadow"
            shadow.mkdir()
            (shadow / "cargo").write_text("substituted", encoding="ascii")
            create_allowlisted_path(allowlist, manifest, tools)
            selected = shutil.which("cargo", path=os.fspath(allowlist))
            if selected is None or os.readlink(selected) != tools["cargo"].executable.proc_path:
                raise AssertionError("allowlisted PATH resolved a shadow Cargo")
            if str(shadow) in str(allowlist):
                raise AssertionError("ambient PATH entered the allowlist")
        finally:
            for tool in tools.values():
                tool.close()
    adversarial_self_test()
    print("gfx942 compiler-evidence controller mutation tests: PASS")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--observe-transition-candidate", action="store_true")
    parser.add_argument("run_root", nargs="?")
    parser.add_argument("evidence_root", nargs="?")
    args = parser.parse_args()
    script = Path(__file__).resolve(strict=True)
    repo = script.parent.parent
    try:
        if args.self_test:
            if (
                args.run_root is not None
                or args.evidence_root is not None
                or args.observe_transition_candidate
            ):
                raise EvidenceError("--self-test accepts no output paths")
            self_test(repo)
        else:
            if args.run_root is None or args.evidence_root is None:
                raise EvidenceError("RUN_ROOT and EVIDENCE_ROOT are required")
            controller(
                Path(args.run_root),
                Path(args.evidence_root),
                observe_candidate=args.observe_transition_candidate,
            )
    except (EvidenceError, HardeningError, OSError, UnicodeError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
