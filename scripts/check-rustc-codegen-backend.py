#!/usr/bin/env python3
"""Build and check a Cargo-reported backend; this is not launch authority."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import struct
import subprocess
import sys
import tempfile


PACKAGE = "rustc-codegen-fe2o3"
TARGET = "rustc_codegen_fe2o3"
# Match cargo-fe2o3/src/rustc_wrapper/pinned_codegen_backend.rs, not the
# separate executable limit. Debug LLVM-backed images can exceed 512 MiB.
MAX_BACKEND_BYTES = 1024 * 1024 * 1024
MAX_RECEIPT_BYTES = 32 * 1024 * 1024
MAX_RECORD_BYTES = 2 * 1024 * 1024
LOAD_TIMEOUT_SECONDS = 60


class CheckError(Exception):
    pass


def require(condition, message):
    if not condition:
        raise CheckError(message)


def package_identity(metadata, repo):
    packages = [p for p in metadata["packages"] if p["name"] == PACKAGE]
    require(len(packages) == 1, "expected one backend package")
    package = packages[0]
    require(Path(package["manifest_path"]).resolve() == repo / "crates" / PACKAGE / "Cargo.toml",
            "backend manifest is outside the selected workspace")
    targets = [t for t in package["targets"] if t["name"] == TARGET and "dylib" in t["kind"]]
    require(len(targets) == 1, "expected one backend dylib target")
    target = targets[0]
    require(Path(target["src_path"]).resolve() == repo / "crates" / PACKAGE / "src/lib.rs",
            "backend source is outside the selected workspace")
    target_dir = Path(metadata["target_directory"])
    require(target_dir.is_absolute(), "Cargo target directory must be absolute")
    return package["id"], target, target_dir.resolve()


def cargo_artifact(receipt, identity):
    package, target, target_dir = identity
    require(receipt.stat().st_size <= MAX_RECEIPT_BYTES, "Cargo receipt exceeds size limit")
    candidates, finished = [], False
    with receipt.open("rb") as stream:
        consumed = 0
        while line := stream.readline(MAX_RECORD_BYTES + 1):
            consumed += len(line)
            require(len(line) <= MAX_RECORD_BYTES and consumed <= MAX_RECEIPT_BYTES,
                    "Cargo receipt exceeds size limit")
            require(not finished, "Cargo emitted records after build-finished")
            record = json.loads(line)
            if record.get("reason") == "build-finished":
                require(record.get("success") is True, "Cargo reported an unsuccessful build")
                finished = True
            elif record.get("reason") == "compiler-artifact":
                actual = record.get("target", {})
                if actual.get("name") != TARGET:
                    continue
                require(record.get("package_id") == package and
                        all(actual.get(k) == target.get(k) for k in
                            ("name", "kind", "crate_types", "src_path")),
                        "Cargo backend package/source/target mismatch")
                profile = record.get("profile", {})
                require(profile.get("test") is False and profile.get("debuginfo") == 1 and
                        profile.get("opt_level") == "0" and
                        profile.get("debug_assertions") is True and
                        profile.get("overflow_checks") is True,
                        "Cargo backend is not the limited-debug development build")
                paths = [Path(p) for p in record.get("filenames", []) if p.endswith(".so")]
                require(len(paths) == 1, "expected one backend shared object")
                path = paths[0]
                require(path.is_absolute() and path == path.resolve() and
                        path.is_relative_to(target_dir) and path.name == f"lib{TARGET}.so",
                        "Cargo backend path is not canonical within the target directory")
                candidates.append(path)
    require(finished and len(candidates) == 1, "expected one completed backend artifact")
    return candidates[0]


def identity(value):
    return (value.st_dev, value.st_ino, value.st_mode, value.st_size,
            value.st_mtime_ns, value.st_ctime_ns)


def current(fd, path, expected):
    require(identity(os.fstat(fd)) == expected and identity(path.lstat()) == expected and
            path == path.resolve(), "backend changed during inspection or loading")


def digest(fd, size):
    result = hashlib.sha256()
    offset = 0
    while offset < size:
        block = os.pread(fd, min(1024 * 1024, size - offset), offset)
        require(bool(block), "backend truncated during hashing")
        result.update(block)
        offset += len(block)
    require(not os.pread(fd, 1, size), "backend grew during hashing")
    return result.hexdigest()


def check_elf(fd, size):
    header = os.pread(fd, 64, 0)
    require(len(header) == 64 and header[:7] == b"\x7fELF\x02\x01\x01",
            "backend must be a little-endian ELF64 object")
    kind, _, version, _, phoff, _, _, ehsize, phsize, phcount = struct.unpack_from("<HHIQQQIHHH", header, 16)
    require(kind == 3 and version == 1 and ehsize == 64 and phsize == 56 and
            phcount > 0 and phoff >= 64 and phoff + phcount * phsize <= size,
            "backend must be ET_DYN with bounded ELF64 program headers")


def load_probe(fd, rustc, repo, environment, scratch):
    source = scratch / "load_probe.rs"
    source.write_text("#![no_std]\npub fn fe2o3_backend_load_probe() {}\n", encoding="ascii")
    probe_env = environment.copy()
    for name in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"):
        probe_env.pop(name, None)
    log = scratch / "load.log"
    with log.open("wb") as output:
        # The dot is required: rustc otherwise treats this selector as a backend name.
        result = subprocess.run(
            ["timeout", "--signal=TERM", "--kill-after=5s", f"{LOAD_TIMEOUT_SECONDS}s",
             rustc, "--crate-name", "fe2o3_backend_load_probe", "--crate-type", "lib",
             "--emit=metadata", f"-Zcodegen-backend=/proc/./self/fd/{fd}",
             "--out-dir", str(scratch), str(source)],
            cwd=repo, env=probe_env, pass_fds=(fd,), stdout=output, stderr=subprocess.STDOUT,
            check=False,
        )
    if result.returncode:
        with log.open("rb") as output:
            print(output.read(16384).decode("utf-8", errors="replace"), file=sys.stderr)
        raise CheckError(f"rustc backend load probe failed with status {result.returncode}")


def inspect_backend(path, rustc, repo, environment, scratch):
    fd = os.open(path, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        info = os.fstat(fd)
        require(stat.S_ISREG(info.st_mode) and 0 < info.st_size <= MAX_BACKEND_BYTES,
                "backend must be a nonempty regular file within the 1 GiB limit")
        expected = identity(info)
        current(fd, path, expected)
        check_elf(fd, info.st_size)
        before = digest(fd, info.st_size)
        current(fd, path, expected)
        load_probe(fd, rustc, repo, environment, scratch)
        current(fd, path, expected)
        require(digest(fd, info.st_size) == before, "backend contents changed during loading")
        current(fd, path, expected)
        return {"path": str(path), "bytes": info.st_size, "limitBytes": MAX_BACKEND_BYTES,
                "headroomBytes": MAX_BACKEND_BYTES - info.st_size, "sha256": before,
                "metadataLoad": "passed"}
    finally:
        os.close(fd)


def build_and_check(repo, all_features):
    environment = os.environ.copy()
    rustc = shutil.which(environment.get("RUSTC", "rustc"))
    require(rustc is not None, "selected rustc is unavailable")
    # Preserve the rustup proxy spelling and cwd; do not resolve it into `rustup`.
    environment.update(RUSTC=os.path.abspath(rustc), RUSTC_WRAPPER="", RUSTC_WORKSPACE_WRAPPER="",
                       CARGO_PROFILE_DEV_DEBUG="1", CARGO_INCREMENTAL="0")
    with tempfile.TemporaryDirectory(prefix="fe2o3-backend-check-") as directory:
        scratch = Path(directory)
        metadata_path = scratch / "metadata.json"
        with metadata_path.open("wb") as output:
            subprocess.run(["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"],
                           cwd=repo, env=environment, stdout=output, check=True)
        require(metadata_path.stat().st_size <= MAX_RECEIPT_BYTES, "Cargo metadata exceeds size limit")
        metadata = json.loads(metadata_path.read_bytes())
        selected = package_identity(metadata, repo)
        receipt = scratch / "artifacts.jsonl"
        # Keep both binary targets in the package build, including all-features;
        # only the metadata-bound library record is admitted as the backend.
        command = ["cargo", "build", "--locked", "-p", PACKAGE, "--message-format=json-render-diagnostics"]
        if all_features:
            command.append("--all-features")
        with receipt.open("wb") as output:
            subprocess.run(command, cwd=repo, env=environment, stdout=output, check=True)
        artifact = cargo_artifact(receipt, selected)
        result = inspect_backend(artifact, environment["RUSTC"], repo, environment, scratch)
        result.update(packageId=selected[0], allFeatures=all_features, rustc=environment["RUSTC"])
        print(json.dumps(result, sort_keys=True))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--all-features", action="store_true")
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[1]
    try:
        build_and_check(repo, args.all_features)
    except (CheckError, OSError, ValueError, KeyError, TypeError, subprocess.CalledProcessError) as error:
        print(f"backend check: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
