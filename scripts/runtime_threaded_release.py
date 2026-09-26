#!/usr/bin/env python3
"""Qualify the actual threaded VecAdd release ELF, without running GPU work."""

from __future__ import annotations

import argparse
import hashlib
import importlib.abc
import importlib.util
import json
import os
from pathlib import Path
import shutil
import shlex
import signal
import stat
import struct
import sys
import tempfile
import tomllib

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parent.parent
EXAMPLE = "gfx942-runtime-vecadd-benchmark"
MUSL = "x86_64-unknown-linux-musl"
GNU = "x86_64-unknown-linux-gnu"
USAGE = (f'Error: "usage: {EXAMPLE} UNIQUE_ID_OR_AUTO WARMUPS SAMPLES '
         'LAUNCHES_PER_SAMPLE [PROFILE_SCOPE_HEX PROFILE_OUTPUT]"\n')
THREAD_PATHS = (
    "write_disjoint_source_partitions",
    "verify_disjoint_source_partitions",
    "write_disjoint_repeated_byte_partitions",
)
PINS = {
    "benchmarks/runtime_gfx942/run-r61-owner-mi300x.py": "52e56850ffbefaae2d168629b16fd40bc10549c524b38c5ac22388deccf21a5d",
    "benchmarks/runtime_gfx942/run-r60-pipeline-mi300x.py": "682a475bb91a5243c6d142d4c3baf02da161543c4d2a8dcd3dfd1adc3478921b",
    "crates/fe2o3-runtime-model/verus/check-journal-issuance.py": "36d5e0641300bf2568884ec4c440c668477a4b24a07ee95273a91eab34659480",
    "scripts/runtime_pure_rust_audit.py": "268315ca3ac16585d179b6055381827f41287c8ababae970cbeecec0073cec64",
    "scripts/runtime-pure-rust-policy.json": "0a10281676efd73ca55eaa1e04a5087f5bf94b2a403cb02b1fff85d6ae97c72f",
}

def capture_pinned(path: Path, expected: str) -> bytes:
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(descriptor, "rb") as stream:
        before = os.fstat(stream.fileno())
        if not stat.S_ISREG(before.st_mode) or before.st_size > 4 << 20:
            raise ValueError(f"helper/policy is not a bounded ordinary file: {path}")
        data = stream.read((4 << 20) + 1)
        after = os.fstat(stream.fileno())
    stable = lambda value: (value.st_dev, value.st_ino, value.st_mode, value.st_size,
                            value.st_mtime_ns, value.st_ctime_ns)
    if (stable(before) != stable(after) or len(data) != before.st_size
            or hashlib.sha256(data).hexdigest() != expected):
        raise ValueError(f"release qualification helper/policy differs from pin: {path}")
    return data


CAPTURED = {ROOT / name: capture_pinned(ROOT / name, digest) for name, digest in PINS.items()}
ORIGINAL_SPEC = importlib.util.spec_from_file_location


class CapturedLoader(importlib.abc.Loader):
    def __init__(self, path: Path):
        self.path = path

    def create_module(self, _spec):
        return None

    def exec_module(self, module):
        exec(compile(CAPTURED[self.path], str(self.path), "exec"), module.__dict__)


def captured_spec(name, path, *args, **kwargs):
    path = Path(path).absolute()
    if path in CAPTURED:
        return ORIGINAL_SPEC(name, path, loader=CapturedLoader(path))
    return ORIGINAL_SPEC(name, path, *args, **kwargs)


def load(name: str, path: Path):
    spec = captured_spec(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    # R61 explicitly loads R60 with spec_from_file_location; its transitive
    # import must execute the same captured bytes, not reopen a mutable path.
    previous = importlib.util.spec_from_file_location
    try:
        importlib.util.spec_from_file_location = captured_spec
        spec.loader.exec_module(module)
    finally:
        importlib.util.spec_from_file_location = previous
    return module


OWNER_PATH = ROOT / "benchmarks/runtime_gfx942/run-r61-owner-mi300x.py"
OWNER = load("threaded_release_owner", OWNER_PATH)
BASE = OWNER.base
AUDIT_PATH = ROOT / "scripts/runtime_pure_rust_audit.py"
AUDIT = load("threaded_release_audit", AUDIT_PATH)
OWNED_PATH = ROOT / "crates/fe2o3-runtime-model/verus/check-journal-issuance.py"
OWNED = load("threaded_release_process", OWNED_PATH)
Error = BASE.RunError


class CapturedPolicy:
    def read_bytes(self):
        return CAPTURED[ROOT / "scripts/runtime-pure-rust-policy.json"]

    def __str__(self):
        return "captured runtime-pure-rust-policy.json"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise Error(message)


def identity(path: Path) -> dict:
    return {"resolved": str(path.resolve(strict=True)), "sha256": BASE.sha256_file(path)}


def identities(paths) -> dict:
    return {str(path): identity(path) for path in sorted(set(paths))}


def reject_cargo_config(root: Path, home: Path) -> None:
    for directory in {home, root, *root.parents}:
        for name in ("config", "config.toml"):
            path = directory / ".cargo" / name
            require(not path.exists() and not path.is_symlink(),
                    f"ambient Cargo configuration is not admitted: {path}")


def validate_usage(status: int, stdout: str, stderr: str) -> None:
    require((status, stdout, stderr) == (1, "", USAGE),
            "missing enabled benchmark usage path (disabled stubs do not qualify)")


def validate_thread_paths(symbols: str) -> list[str]:
    defined = {}
    for line in symbols.splitlines():
        fields = line.split()
        if len(fields) == 4 and fields[1] in ("t", "T"):
            if int(fields[2], 16) > 0 and int(fields[3], 16) > 0:
                defined[fields[0]] = (int(fields[2], 16), int(fields[3], 16))
    selected = []
    for anchor in THREAD_PATHS:
        candidates = [name for name in defined if anchor in name
                      and name.startswith("_RINvNtNt") and "9fe2o3_kfd13shared_memory" in name
                      and "drop_in_place" not in name and "call_once6vtable" not in name]
        roles = {
            "worker": [name for name in candidates if "3std3sys9backtrace28___rust_begin_short_backtrace" in name
                       and "spawn_unchecked" not in name],
            "scope": [name for name in candidates if "3std6thread6scoped5scope" in name],
            "spawn": [name for name in candidates if "3std6thread9lifecycle15spawn_unchecked" in name
                      and "backtrace" not in name],
        }
        for role, matches in roles.items():
            require(len(matches) == 1, f"missing or ambiguous executable {role} path: {anchor}")
            selected.append(matches[0])
    require(len({defined[name][0] for name in selected}) == len(selected),
            "thread path text addresses are aliased")
    require(any("pthread_create" in line.split()[0] for line in symbols.splitlines() if line.split()),
            "missing native thread startup")
    return selected


def validate_static_layout(data: bytes) -> None:
    require(data[:7] == b"\x7fELF\x02\x01\x01", "not little-endian ELF64")
    header = struct.unpack_from("<HHIQQQIHHHHHH", data, 16)
    kind, machine, version, entry, offset = header[:5]
    require((kind, machine, version, header[7], header[8]) == (3, 62, 1, 64, 56),
            "not an x86-64 static PIE header")
    require(0 < header[9] <= 4096, "invalid program header count")
    programs = [struct.unpack_from("<IIQQQQQQ", data, offset + index * 56)
                for index in range(header[9])]
    require(sum(p[0] == 2 for p in programs) == 1, "missing unique DYNAMIC segment")
    require(sum(p[0] == 0x6474e552 for p in programs) == 1, "missing unique GNU_RELRO")
    stacks = [p for p in programs if p[0] == 0x6474e551]
    require(len(stacks) == 1 and stacks[0][1] == 6, "missing non-executable GNU_STACK")
    loads = [p for p in programs if p[0] == 1]
    require(not any(p[1] & 3 == 3 for p in loads), "writable executable LOAD")
    require(any(p[1] & 1 and p[3] <= entry < p[3] + p[5] for p in loads),
            "entry point is outside executable file bytes")
    require(not any(p[0] == 3 for p in programs), "static PIE has an interpreter")


def validate_link_inputs(link: str, mapping: str, libraries: dict, target: Path, temporary: Path) -> dict:
    fields = shlex.split(link)
    crt_names = ("rcrt1.o", "crti.o", "crtbeginS.o", "crtendS.o", "crtn.o")
    crts = [Path(value) for value in fields if Path(value).name in crt_names]
    require(tuple(path.name for path in crts) == crt_names, "static CRT roster/order differs")
    require(all(str(path) in libraries for path in crts), "CRT was not measured before build")
    inputs = [Path(line[5:]) for line in mapping.splitlines() if line.startswith("LOAD ")]
    require(bool(inputs), "link map contains no resolved inputs")
    external = {}
    for path in inputs:
        require(path.is_absolute(), f"nonabsolute link input: {path}")
        if path.is_relative_to(target) or path.is_relative_to(temporary):
            continue
        require(str(path) in libraries, f"unmeasured external link input: {path}")
        external[str(path)] = identity(path)
        require(external[str(path)] == libraries[str(path)], f"changed link input: {path}")
    require({"libc.a", "libunwind.a"}.issubset(Path(name).name for name in external),
            "missing resolved musl/unwind archive evidence")
    return external


def validate_gnu_rejection(violations: list[str]) -> None:
    require(violations == ["prohibited dynamic symbol: dlsym (exact)"],
            f"GNU candidate differs from the pinned dlsym rejection: {violations}")


class Qualifier:
    def __init__(self, output: Path, home: Path):
        self.output = output
        self.home = home
        self.rust = home / ".cargo/bin"
        self.target = output / "target"
        temporary = output / "tmp"
        temporary.mkdir()
        self.env = dict(BASE.CLEAN_ENV, HOME=str(home), PATH=f"{self.rust}:{BASE.SYSTEM_PATH}",
                        CARGO_INCREMENTAL="0", CARGO_BUILD_JOBS="1",
                        CARGO_TARGET_DIR=str(self.target), TMPDIR=str(temporary))
        self.groups_absent = True

    def run(self, label: str, command, *, timeout: int = 1200, expected: int = 0):
        self.groups_absent = False
        status, stdout, stderr = OWNED.run_owned(
            [str(value) for value in command], timeout, self.output / label, self.env)
        self.groups_absent = True
        require(len(stdout.encode()) <= 32 << 20 and len(stderr.encode()) <= 32 << 20,
                f"{label}: output exceeds accepted evidence bound")
        require(status == expected, f"{label}: expected exit {expected}, observed {status}")
        return stdout, stderr

    def source_identity(self) -> dict:
        # Git selects the complete tracked crate trees, including embedded fixtures.
        listing, _ = self.run(f"source-list-{len(list(self.output.glob('source-list-*')))}",
                             ["/usr/bin/git", "ls-files", "-z", "--", "crates", "Cargo.toml",
                              "Cargo.lock", "rust-toolchain.toml"])
        paths = [ROOT / name for name in listing.split("\0") if name]
        paths.extend((Path(__file__), AUDIT_PATH, OWNER_PATH, OWNED_PATH,
                      ROOT / "benchmarks/runtime_gfx942/run-r60-pipeline-mi300x.py",
                      ROOT / "scripts/runtime-pure-rust-policy.json",
                      ROOT / "scripts/ci-local.sh",
                      ROOT / "scripts/tests/ci-local-test-gate.sh",
                      ROOT / ".github/workflows/ci.yml",
                      ROOT / "scripts/tests/runtime_threaded_release.py"))
        untracked, _ = self.run(f"untracked-{len(list(self.output.glob('untracked-*')))}",
                               ["/usr/bin/git", "ls-files", "--others", "--exclude-standard", "--", "crates"])
        require(not untracked, "untracked crate inputs are not admitted")
        return identities(paths)

    def tool_identity(self) -> dict:
        tools = [Path("/usr/bin") / name for name in
                 ("cc", "readelf", "nm", "objdump", "git")]
        tools.append(Path(sys.executable))
        tools.extend(self.rust / name for name in ("cargo", "rustc", "rustup"))
        for name in ("ld.bfd", "collect2", "cc1"):
            value, _ = self.run(f"resolve-{name}", ["/usr/bin/cc", f"-print-prog-name={name}"])
            resolved = shutil.which(value.strip(), path=BASE.SYSTEM_PATH)
            require(resolved is not None, f"missing compiler tool: {name}")
            tools.append(Path(resolved))
        for name in ("cargo", "rustc"):
            value, _ = self.run(f"resolve-{name}", [self.rust / "rustup", "which", name])
            tools.append(Path(value.strip()))
            self.run(f"version-{name}", [self.rust / name, "-Vv"])
        return identities(tools)

    def target_libraries(self) -> dict:
        result = {}
        for target in (GNU, MUSL):
            value, _ = self.run(f"libdir-{target}",
                               [self.rust / "rustc", "--print", "target-libdir", "--target", target])
            directory = Path(value.strip())
            entries = sorted(path for path in directory.rglob("*") if path.is_file())
            require(any(path.name.startswith("libstd-") for path in entries), "missing target std")
            if target == MUSL:
                require(any(path.name == "rcrt1.o" for path in entries), "missing static PIE CRT")
            result.update(identities(entries))
        return result

    def build(self, target: str, policy: dict, libraries: dict) -> dict:
        directory = self.output / target
        directory.mkdir()
        arguments = ["--offline", "--locked", "--no-default-features",
                     "--features", "fe2o3-runtime/hardware-qualification"]
        metadata, _ = self.run(f"metadata-{target}",
                              [self.rust / "cargo", "metadata", *arguments,
                               "--filter-platform", target, "--format-version", "1"])
        violations, closure = AUDIT.audit_metadata(json.loads(metadata), ("fe2o3-runtime",), policy)
        require(not violations, f"Cargo closure rejected: {violations}")
        link_map = directory / "link.map"
        link, _ = self.run(f"build-{target}",
                          [self.rust / "cargo", "rustc", "--release", *arguments, "--target", target,
                           "-p", "fe2o3-runtime", "--example", EXAMPLE, "--", "--print=link-args",
                           "-C", "linker=/usr/bin/cc", "-C", "link-arg=-fuse-ld=bfd",
                           "-C", f"link-arg=-Wl,-Map,{link_map}"])
        binary = directory / EXAMPLE
        built = self.target / target / "release/examples" / EXAMPLE
        require(built.is_file() and not built.is_symlink(), "missing regular build output")
        shutil.copyfile(built, binary)
        binary.chmod(0o500)
        before = identity(binary)
        violations, elf = AUDIT.audit_elf(binary, policy)
        symbols, _ = self.run(f"symbols-{target}",
                              ["/usr/bin/nm", "--format=posix", "--no-demangle", binary])
        anchors = validate_thread_paths(symbols)
        for index, name in enumerate(anchors):
            disassembly, _ = self.run(f"thread-code-{target}-{index}",
                ["/usr/bin/objdump", f"--disassemble={name}", binary])
            require(f"<{name}>:" in disassembly, "retained thread symbol has no disassembly")
        headers, _ = self.run(f"headers-{target}",
                              ["/usr/bin/readelf", "--program-headers", "--dynamic", "--wide", binary])
        if target == MUSL:
            require(not violations, f"musl ELF rejected: {violations}")
            OWNER.validate_link_command(link)
            OWNER.validate_static_headers(headers)
            OWNER.validate_static_symbols(symbols, policy)
            validate_static_layout(binary.read_bytes())
            inputs = validate_link_inputs(link, link_map.read_text(), libraries,
                                          self.target, self.output / "tmp")
            BASE.write_json(directory / "external-link-inputs.json", inputs)
            require(b"__pthread_get_minstack" not in binary.read_bytes(), "GNU stack lookup retained")
        else:
            validate_gnu_rejection(violations)
        stdout, stderr = self.run(f"usage-{target}", [binary], timeout=10, expected=1)
        validate_usage(1, stdout, stderr)
        require(identity(binary) == before, "binary changed during audit")
        return {"target": target, "accepted": target == MUSL, "elf": elf,
                "violations": violations, "cargo_closure": closure,
                "enabled_usage": True, "retained_thread_paths": list(THREAD_PATHS)}

    def compiled_negatives(self, policy: dict) -> None:
        source = self.output / "prohibited.c"
        source.write_text('#include <dlfcn.h>\nint main(int n, char **v) {\n'
                          '  return dlsym((void *)0, v[n - 1]) == 0;\n}\n')
        binary = self.output / "prohibited-dynamic"
        self.run("compile-prohibited", ["/usr/bin/cc", "-O2", source, "-ldl", "-o", binary])
        violations, _ = AUDIT.audit_elf(binary, policy)
        validate_gnu_rejection(violations)
        # The unchanged full-symbol guard must also reject a linked static symbol,
        # even when the dynamic-only audit has nothing to inspect.
        rust = self.output / "prohibited.rs"
        rust.write_text('unsafe extern "C" { fn dlsym(h: *mut u8, n: *const u8) -> *mut u8; }\n'
                        'fn main() { unsafe { std::hint::black_box(dlsym(std::ptr::null_mut(), '
                        'c"deliberately_prohibited".as_ptr().cast())); } }\n')
        static = self.output / "prohibited-static"
        self.run("compile-prohibited-static", [self.rust / "rustc", rust, "--edition=2024",
                  "--target", MUSL, "-C", "linker=/usr/bin/cc", "-C", "link-arg=-fuse-ld=bfd", "-o", static])
        violations, _ = AUDIT.audit_elf(static, policy)
        require(not violations, f"static negative already rejected by dynamic audit: {violations}")
        symbols, _ = self.run("symbols-prohibited-static", ["/usr/bin/nm", "--format=posix", static])
        try:
            OWNER.validate_static_symbols(symbols, policy)
        except Error as error:
            require(str(error) in ("prohibited full symbol: dlsym", "prohibited full symbol: __dlsym"),
                    f"static negative failed for the wrong reason: {error}")
        else:
            raise Error("compiled static prohibited symbol was accepted")

    def qualify(self) -> dict:
        reject_cargo_config(ROOT, self.home)
        channel = tomllib.loads((ROOT / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
        active, _ = self.run("active-toolchain", [self.rust / "rustup", "show", "active-toolchain"])
        require(active.startswith(channel + "-"), "active Rust toolchain differs from repository pin")
        sources = self.source_identity()
        tools = self.tool_identity()
        libraries = self.target_libraries()
        BASE.write_json(self.output / "sources.json", sources)
        BASE.write_json(self.output / "tools.json", tools)
        BASE.write_json(self.output / "target-libraries.json", libraries)
        policy = AUDIT.load_policy(CapturedPolicy())
        self.compiled_negatives(policy)
        builds = [self.build(target, policy, libraries) for target in (GNU, MUSL)]
        require(self.source_identity() == sources, "source changed during qualification")
        require(identities(map(Path, tools)) == tools, "selected tools changed during qualification")
        require(identities(map(Path, libraries)) == libraries, "target std/CRT changed during qualification")
        return {"schema": "fe2o3.runtime.threaded-release.v1", "status": "passed",
                "scope": "feature-enabled-vecadd-release-ELF-only", "toolchain": channel,
                "builds": builds, "compiled_negatives": 2,
                "native_execution": False, "performance_qualified": False,
                "source_tool_std_crt_rechecked": True}


def main() -> int:
    for signum in OWNED.SIGNALS:
        signal.signal(signum, OWNED.interrupted)
    signal.pthread_sigmask(signal.SIG_UNBLOCK, OWNED.SIGNALS)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--build-home", type=Path, default=Path.home())
    arguments = parser.parse_args()
    output_root = arguments.output_root.resolve()
    output_root.mkdir(parents=True, exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix="threaded-release-", dir=output_root))
    print(f"threaded release evidence: {output}", flush=True)
    os.chdir(ROOT)
    qualifier = Qualifier(output, arguments.build_home.resolve())
    try:
        result = qualifier.qualify()
    except Exception as error:
        BASE.write_json(output / "result.json", {"status": "failed", "error": str(error)})
        print(f"threaded release qualification failed: {error}", file=sys.stderr)
        return 1
    else:
        BASE.write_json(output / "result.json", result)
        print("threaded release ELF qualification passed; native/performance not qualified")
        return 0
    finally:
        if qualifier.groups_absent and qualifier.target.exists():
            shutil.rmtree(qualifier.target)


if __name__ == "__main__":
    raise SystemExit(main())
