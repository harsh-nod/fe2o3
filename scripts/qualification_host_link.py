#!/usr/bin/python3
"""Observe the genuine campaign's host links; never grants runtime authority."""

from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path, PurePosixPath
import re
import selectors
import shlex
import shutil
import stat
import struct
import subprocess
import sys
import tarfile
import time
import signal

from qualification_input_bundle import (
    DIR_FLAGS, HardeningError, canonical_json, fail, file_at,
    hash_fd, load_json, relative_path, bounded_names,
)

ROOT = Path("/run/qualification-host-link-v1")
RECORDS = Path("/run/qualification-host-link-records")
TARGET = Path("/run/application-target")
GCC = Path("/usr/bin/x86_64-linux-gnu-gcc-13")
GCC_LIB = Path("/usr/lib/gcc/x86_64-linux-gnu/13")
GCC_EXEC = Path("/usr/libexec/gcc/x86_64-linux-gnu/13")
SYSTEM = Path("/usr/lib/x86_64-linux-gnu")
TARGET_NAME = "x86_64-unknown-linux-gnu"
MAX_FILE = 512 * 1024 * 1024
MAX_TAR = MAX_FILE
MAX_ENTRIES = 4096
MAX_RESPONSE = 1024 * 1024
SCHEMA = "fe2o3.qualification.host-link-premises.v1"
CC_LINKS = {
    "/usr/bin/cc": "/etc/alternatives/cc",
    "/etc/alternatives/cc": "/usr/bin/gcc",
    "/usr/bin/gcc": "gcc-13",
    "/usr/bin/gcc-13": "x86_64-linux-gnu-gcc-13",
}
CAPTURE_FILES = {"argv.bin", "environment.bin", "cwd", "original.elf", "inputs.tar", "complete"}


def directory_profile(path: Path, uid: int, gid: int, mode: int) -> None:
    if path.resolve(strict=True) != path:
        fail("qualification directory is an alias")
    fd = os.open(path, DIR_FLAGS)
    try:
        info = os.fstat(fd)
        if (info.st_uid, info.st_gid, stat.S_IMODE(info.st_mode)) != (uid, gid, mode):
            fail("qualification directory ownership/mode differs")
    finally:
        os.close(fd)


def identity(path: Path) -> dict:
    resolved = path.resolve(strict=True)
    descriptor = os.open(resolved.parent, DIR_FLAGS)
    try:
        with file_at(descriptor, resolved.name, hardlinks=True) as (file, info):
            if info.st_size > MAX_FILE:
                fail("host link input exceeds byte bound")
            return {"resolved": str(resolved), "mode": stat.S_IMODE(info.st_mode),
                    "bytes": info.st_size, "sha256": hash_fd(file, info.st_size)}
    finally:
        os.close(descriptor)


def query(arguments: list[str], *, executable: Path) -> str:
    # Only the already selected trusted installed tools are queried, not tar inputs.
    process = subprocess.Popen(arguments, executable=executable, start_new_session=True,
                               env={"PATH": "/usr/bin:/bin", "LANG": "C", "LC_ALL": "C"},
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    output = [bytearray(), bytearray()]
    deadline = time.monotonic() + 10
    try:
        with selectors.DefaultSelector() as selector:
            for index, stream in enumerate((process.stdout, process.stderr)):
                os.set_blocking(stream.fileno(), False)
                selector.register(stream, selectors.EVENT_READ, index)
            while selector.get_map():
                if time.monotonic() >= deadline:
                    fail("host tool query timed out")
                for key, _ in selector.select(max(0, deadline - time.monotonic())):
                    chunk = os.read(key.fd, 65536)
                    if not chunk:
                        selector.unregister(key.fd)
                    else:
                        output[key.data].extend(chunk)
                        if sum(map(len, output)) > MAX_RESPONSE:
                            fail("host tool query exceeds output bound")
            if process.wait(timeout=max(0, deadline - time.monotonic())) != 0:
                fail("host tool query failed")
    finally:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.wait(timeout=5)
        process.stdout.close()
        process.stderr.close()
    return output[0].decode("utf-8")


def read_record(path: Path, limit: int) -> bytes:
    if path.parent.resolve(strict=True) != path.parent:
        fail("record parent is an alias")
    directory = os.open(path.parent, DIR_FLAGS)
    try:
        with file_at(directory, path.name) as (fd, info):
            if info.st_size > limit:
                fail("record exceeds byte bound")
            data = os.pread(fd, info.st_size + 1, 0)
            if len(data) != info.st_size:
                fail("record changed while read")
            return data
    finally:
        os.close(directory)


def measure(rust: Path) -> dict:
    if any(os.readlink(path) != target for path, target in CC_LINKS.items()):
        fail("cc alternatives chain differs")
    if (GCC_LIB / "specs").exists() or (GCC_LIB / "specs").is_symlink():
        fail("unreviewed GCC specs file")
    tool_paths = [ROOT / "real-cc", GCC_EXEC / "collect2", GCC_EXEC / "liblto_plugin.so",
                  GCC_EXEC / "lto-wrapper"]
    tools = {str(path): identity(path) for path in tool_paths}
    paths = [GCC_LIB / name for name in
             ("crtbegin.o", "crtbeginS.o", "crtbeginT.o", "crtend.o", "crtendS.o",
              "libgcc.a", "libgcc_eh.a", "libgcc_s.so")]
    paths += [SYSTEM / name for name in
              ("crt1.o", "Scrt1.o", "crti.o", "crtn.o", "libc.a", "libc_nonshared.a",
               "libutil.a", "librt.a", "libpthread.a", "libm.a", "libmvec.a", "libdl.a",
               "libc.so", "libc.so.6", "libm.so", "libm.so.6", "libmvec.so.1", "libgcc_s.so.1")]
    math = sorted(SYSTEM.glob("libm-[0-9]*.a"))
    if len(math) != 1:
        fail("missing or ambiguous versioned math archive")
    paths += math + [Path("/lib64/ld-linux-x86-64.so.2")]
    paths += [Path("/lib/x86_64-linux-gnu") / path.name for path in paths if path.parent == SYSTEM]
    target = rust / "lib/rustlib" / TARGET_NAME
    fd = os.open(target / "lib", DIR_FLAGS)
    try:
        paths += [target / "lib" / name for name in bounded_names(fd, MAX_ENTRIES)
                  if stat.S_ISREG(os.stat(name, dir_fd=fd, follow_symlinks=False).st_mode)]
    finally:
        os.close(fd)
    tools[str(target / "bin/gcc-ld/ld.lld")] = identity(target / "bin/gcc-ld/ld.lld")
    tools[str(target / "bin/rust-lld")] = identity(target / "bin/rust-lld")
    queries = {flag: query(["cc", flag], executable=ROOT / "real-cc")
               for flag in ("-dumpmachine", "-dumpfullversion", "-print-search-dirs", "-dumpspecs")}
    version = query([str(target / "bin/gcc-ld/ld.lld"), "--version"],
                    executable=target / "bin/gcc-ld/ld.lld")
    if not version.endswith(" (compatible with GNU linkers)\n"):
        fail("unrecognized LLD version response")
    return {"schema": SCHEMA, "cc_links": CC_LINKS, "rust_root": str(rust),
            "tools": tools, "queries": queries, "lld_version": version,
            "lld_reproducer_version": version.removesuffix(" (compatible with GNU linkers)\n") + "\n",
            "inputs": {str(path): identity(path) for path in paths},
            "scope": "observed-host-link-inputs-not-runtime-authority",
            "dynamic_loader_resolution_certified": False}


def prepare(rust: Path, proxy: Path, proxy_sha256: str, expected: str | None) -> None:
    if os.geteuid() != 0 or os.getegid() != 0:
        fail("host link setup requires real root")
    if ROOT.exists() or ROOT.is_symlink():
        fail("host link setup directory already exists")
    if rust.resolve(strict=True) != rust or any(c.isspace() for c in str(rust)):
        fail("selected Rust root is not canonical")
    if not re.fullmatch(r"[0-9a-f]{64}", proxy_sha256) or identity(proxy)["sha256"] != proxy_sha256:
        fail("qualification proxy differs from independent pin")
    ROOT.mkdir(mode=0o755)
    # Fresh copied tools are separate from the immutable external checkout/cache.
    for source, name in ((GCC, "real-cc"), (proxy, "proxy")):
        before = identity(source)
        shutil.copyfile(source, ROOT / name)
        (ROOT / name).chmod(0o555)
        if identity(ROOT / name)["sha256"] != before["sha256"] or identity(source) != before:
            fail("host link tool changed during setup")
        if name == "proxy" and before["sha256"] != proxy_sha256:
            fail("copied qualification proxy differs from independent pin")
    (ROOT / "gcc-ld").write_text(str(rust / "lib/rustlib" / TARGET_NAME / "bin/gcc-ld") + "\n")
    observed = measure(rust)
    raw = canonical_json(observed)
    digest = hashlib.sha256(raw).hexdigest()
    if expected is not None and (not re.fullmatch(r"[0-9a-f]{64}", expected) or expected != digest):
        fail("installed host link premises differ from independent pin")
    (ROOT / "premises.json").write_bytes(raw)
    (ROOT / "proxy.sha256").write_text(identity(ROOT / "proxy")["sha256"] + "\n")
    for path in ROOT.iterdir():
        if path.name not in ("real-cc", "proxy"):
            path.chmod(0o444)
    ROOT.chmod(0o555)
    print(f"FE2O3_HOST_LINK_PREMISES_V1={digest}")


def response_arguments(data: bytes) -> list[str]:
    if len(data) > MAX_RESPONSE or b"\0" in data:
        fail("invalid LLD response size/bytes")
    # LLVM's ELF reproducer writes GNU response quoting, not a shell command.
    lexer = shlex.shlex(data.decode("utf-8"), posix=True)
    lexer.whitespace_split = True
    lexer.commenters = ""
    args = list(lexer)
    if not args or len(args) > MAX_ENTRIES or args[:2] != ["--chroot", "."]:
        fail("unrecognized LLD reproduction response")
    if any(arg.startswith("@") or "--reproduce" in arg for arg in args):
        fail("nested response or reproduction control")
    return args


class BoundedTarReader:
    def __init__(self, stream):
        self.stream = stream

    def read(self, size):
        if size < 0 or size > MAX_RESPONSE:
            fail("oversized tar parser metadata read")
        return self.stream.read(size)

    def seek(self, *args):
        return self.stream.seek(*args)

    def tell(self):
        return self.stream.tell()


def read_reproducer(path: Path) -> tuple[dict[str, dict], bytes, bytes]:
    records = {}
    special = {}
    total = 0
    descriptor = os.open(path.parent, DIR_FLAGS)
    try:
        with file_at(descriptor, path.name) as (fd, info):
            if info.st_size > MAX_TAR:
                fail("LLD reproducer exceeds byte bound")
            if info.st_size < 1024 or os.pread(fd, 1024, info.st_size - 1024) != bytes(1024):
                fail("missing complete tar terminator")
            with os.fdopen(os.dup(fd), "rb") as stream, tarfile.open(fileobj=BoundedTarReader(stream), mode="r:") as archive:
                for member in archive:
                    if len(records) + len(special) >= MAX_ENTRIES:
                        fail("LLD reproducer exceeds entry bound")
                    name = relative_path(member.name)
                    if not name.startswith("inputs/") or not member.isfile() or member.linkname or member.issparse():
                        fail("unsupported LLD tar entry")
                    name = name[len("inputs/"):]
                    if "/" + name in records or name in special:
                        fail("duplicate LLD tar entry")
                    if member.size < 0 or member.size > MAX_FILE:
                        fail("LLD tar member exceeds bound")
                    total += member.size
                    if total > MAX_TAR:
                        fail("LLD tar expanded bytes exceed bound")
                    content = archive.extractfile(member)
                    if content is None:
                        fail("missing LLD tar member bytes")
                    digest = hashlib.sha256()
                    remaining = member.size
                    kept = bytearray()
                    is_special = name in ("response.txt", "version.txt")
                    if is_special and remaining > MAX_RESPONSE:
                        fail("LLD metadata exceeds byte bound")
                    while remaining:
                        chunk = content.read(min(65536, remaining))
                        if not chunk:
                            fail("truncated LLD tar member")
                        digest.update(chunk)
                        if is_special:
                            kept.extend(chunk)
                        remaining -= len(chunk)
                    if is_special:
                        special[name] = bytes(kept)
                    else:
                        records["/" + name] = {"bytes": member.size, "sha256": digest.hexdigest()}
                if info.st_size - archive.offset < 1024:
                    fail("missing complete tar terminator")
                stream.seek(archive.offset)
                while chunk := stream.read(65536):
                    if any(chunk):
                        fail("nonzero bytes after parsed tar members")
    finally:
        os.close(descriptor)
    if set(special) != {"response.txt", "version.txt"} or not records:
        fail("missing LLD reproduction metadata or inputs")
    return records, special["response.txt"], special["version.txt"]


def classify_inputs(records: dict, inputs: dict, generated: set[str]) -> dict:
    external = {}
    for logical, measured in records.items():
        if logical in inputs:
            expected = inputs[logical]
        elif logical in generated:
            continue
        else:
            fail(f"unmeasured link input: {logical}")
        if any(measured[key] != expected[key] for key in ("bytes", "sha256")):
            fail(f"changed external link input: {logical}")
        external[logical] = measured
    return external


def generated_inputs(argv: list[str], external: dict, output: Path) -> set[str]:
    generated = set()
    for arg in argv[1:]:
        candidates = [(arg, False)] if arg.startswith("/") else []
        if arg.startswith("-Wl,--version-script="):
            candidates.append((arg.split("=", 1)[1], True))
        for candidate, is_script in candidates:
            path = Path(candidate)
            if ".." in path.parts:
                fail("noncanonical generated input")
            if str(path) in external or candidate == str(output):
                continue
            if not is_script and path.suffix not in (".o", ".rlib", ".so"):
                continue
            # Rust deletes temporary inputs before postflight. Exact argv paths
            # remain observations, not external input authority.
            if path.is_relative_to(TARGET) or re.fullmatch(r"/tmp/rustc[A-Za-z0-9]+/[^/]+", candidate):
                if path.exists() and path.resolve(strict=True) != path:
                    fail("generated link input is a symlink/alias")
                generated.add(candidate)
    return generated


def audit_capture(directory: Path, premises: dict) -> dict:
    if read_record(directory / "complete", 64) != b"original-and-replay-byte-identical\n":
        fail("incomplete native link observation")
    raw = read_record(directory / "argv.bin", 262144)
    if not raw.endswith(b"\0") or len(raw) > 262144:
        fail("invalid observed argv")
    argv = [arg.decode("utf-8") for arg in raw[:-1].split(b"\0")]
    if len(argv) > MAX_ENTRIES or argv[0] != "cc" or argv.count("-o") != 1:
        fail("invalid observed output options")
    cwd = read_record(directory / "cwd", 4096).decode()
    if not cwd.startswith("/") or "\0" in cwd or ".." in Path(cwd).parts:
        fail("invalid observed working directory")
    environment = read_record(directory / "environment.bin", 131104).split(b"\0")
    if len(environment) != 3 or environment[2] or not environment[0].startswith(b"PATH=") or not environment[1].startswith(b"LD_LIBRARY_PATH="):
        fail("invalid observed tool environment")
    output = Path(argv[argv.index("-o") + 1])
    if not output.is_absolute() or ".." in output.parts or not output.resolve(strict=True).is_relative_to(TARGET):
        fail("observed output escaped target")
    if (directory / "original.elf").resolve(strict=True) != directory / "original.elf":
        fail("original ELF is an alias")
    original = identity(directory / "original.elf")
    if any(original[key] != identity(output)[key] for key in ("bytes", "sha256")):
        fail("final output differs from observed original")
    inputs, response, version = read_reproducer(directory / "inputs.tar")
    if version.decode() != premises["lld_reproducer_version"]:
        fail("reproducer LLD version differs")
    args = response_arguments(response)
    if args.count("-o") != 1 or args[args.index("-o") + 1] != output.name:
        fail("reproduction output role differs")
    # Temporary objects/scripts are admitted individually from the original cc
    # invocation, never by an unchecked lexical target/tmp prefix exemption.
    generated = generated_inputs(argv, premises["inputs"], output)
    external = classify_inputs(inputs, premises["inputs"], generated)
    crt_names = {"crt1.o", "Scrt1.o", "crti.o", "crtbegin.o", "crtbeginS.o", "crtbeginT.o", "crtend.o", "crtendS.o", "crtn.o"}
    crts = [PurePosixPath(arg).name for arg in args if not arg.startswith("-") and PurePosixPath(arg).name in crt_names]
    final = re.fullmatch(r"fe2o3_conditional_custodian_application-[0-9a-f]+", output.name) is not None
    if final:
        if crts != ["crt1.o", "crti.o", "crtbeginT.o", "crtend.o", "crtn.o"]:
            fail("final static CRT roster/order differs")
        if "-static" not in argv or "-no-pie" not in argv or "-shared" in argv:
            fail("final static link profile differs")
        with output.open("rb") as elf:
            header = elf.read(64)
        if len(header) != 64 or header[:7] != b"\x7fELF\x02\x01\x01" or struct.unpack_from("<HH", header, 16) != (2, 62):
            fail("final application is not x86-64 ET_EXEC")
        # The genuine application runner independently applies the existing full
        # sealed_static_application_identity_v1 policy before executing this ELF.
        required = {"libc.a", "libm.a", "libmvec.a", "libgcc.a", "libgcc_eh.a"}
        if not required.issubset(Path(path).name for path in external):
            fail("final link lacks required static archives/script expansion")
    return {"output": str(output), "elf": original["sha256"], "final_application": final,
            "cwd": cwd, "tool_environment": [value.decode() for value in environment[:2]],
            "external_inputs": external, "generated_count": len(generated),
            "generated_sha256": hashlib.sha256(canonical_json(sorted(generated))).hexdigest(),
            "original_argv_sha256": hashlib.sha256(raw).hexdigest(),
            "original_argv": argv if final else None, "response_sha256": hashlib.sha256(response).hexdigest(),
            "crt_order": crts}


def audit() -> None:
    if os.geteuid() != 0 or os.getegid() != 0:
        fail("host link postflight requires real root")
    directory_profile(ROOT, 0, 0, 0o555)
    directory_profile(RECORDS, 1000, 1000, 0o700)
    directory_profile(TARGET, 1000, 1000, 0o700)
    _, premises = load_json(ROOT / "premises.json")
    if measure(Path(premises["rust_root"])) != premises:
        fail("installed host link premises changed")
    if identity(GCC)["sha256"] + "\n" != read_record(ROOT / "proxy.sha256", 65).decode():
        fail("qualification proxy changed")
    directory = os.open(RECORDS, DIR_FLAGS)
    try:
        captures = [RECORDS / name for name in bounded_names(directory, 256)]
    finally:
        os.close(directory)
    if not captures or len(captures) > 256 or any(not p.is_dir() or p.is_symlink() for p in captures):
        fail("invalid link capture roster")
    reports = []
    expanded = encoded = 0
    for path in captures:
        if re.fullmatch(r"link-[1-9][0-9]*", path.name) is None:
            fail("invalid capture directory name")
        directory_profile(path, 1000, 1000, 0o700)
        fd = os.open(path, DIR_FLAGS)
        try:
            names = bounded_names(fd, len(CAPTURE_FILES))
            if set(names) != CAPTURE_FILES:
                fail("capture file roster differs")
            for name in names:
                info = os.stat(name, dir_fd=fd, follow_symlinks=False)
                if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1 or info.st_uid != 1000 or info.st_gid != 1000 or stat.S_IMODE(info.st_mode) != 0o600:
                    fail("invalid capture file type")
                expanded += info.st_size
                if expanded > 8 * 1024 * 1024 * 1024:
                    fail("aggregate capture byte bound exceeded")
        finally:
            os.close(fd)
        report = audit_capture(path, premises)
        encoded += len(canonical_json(report))
        if encoded > 16 * 1024 * 1024:
            fail("aggregate report byte bound exceeded")
        reports.append(report)
    if sum(report["final_application"] for report in reports) != 1:
        fail("expected exactly one final application link")
    print("FE2O3_HOST_LINK_OBSERVATION_V1=" + canonical_json({
        "schema": "fe2o3.qualification.host-link-observation.v1", "links": reports,
        "runtime_authority": False, "gpu_execution": False,
        "capture_scope": "output-equivalent-replay",
        "dynamic_loader_resolution_certified": False,
    }).decode().strip())


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    prepare_command = commands.add_parser("prepare")
    prepare_command.add_argument("rust", type=Path)
    prepare_command.add_argument("proxy", type=Path)
    prepare_command.add_argument("proxy_sha256")
    prepare_command.add_argument("--expected")
    commands.add_parser("audit")
    args = parser.parse_args()
    if args.command == "prepare":
        prepare(args.rust, args.proxy, args.proxy_sha256, args.expected)
    else:
        audit()


if __name__ == "__main__":
    try:
        main()
    except (HardeningError, OSError, ValueError, KeyError, IndexError, tarfile.TarError, subprocess.SubprocessError) as error:
        print(f"qualification host link: {error}", file=sys.stderr)
        sys.exit(1)
