#!/usr/bin/env python3
"""Normalize and validate the closed S09 DWARF/ROCgdb evidence profiles."""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

MAX_INPUT_BYTES = 8 * 1024 * 1024
ADDRESS = re.compile(r"0x[0-9a-fA-F]+")
THREAD = re.compile(r"\bThread (?:0x[0-9a-fA-F]+|[0-9]+)", re.IGNORECASE)
PROCESS_ID = re.compile(r"\b(?:LWP|process) [0-9]+\b", re.IGNORECASE)
AMDGPU_LANE = re.compile(r"AMDGPU Lane (?!<LANE>)[^]]+\]")
AMDGPU_WAVE = re.compile(r"AMDGPU Wave \S+")
MEMORY_URI = re.compile(r"memory://[0-9]+#offset=0x[0-9a-fA-F]+&size=[0-9]+")
SPACE = re.compile(r"[ \t]+")
HEX_SHA256 = re.compile(r"[0-9a-f]{64}")
HEX_BUILD_ID = re.compile(r"[0-9a-f]{40,64}")
S09_SOURCE = "crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src/main.rs"
S09_DIRECTORY = "crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src"
SOURCE_CANDIDATE = re.compile(rf'(?:[^\s"()]+/)?{re.escape(S09_SOURCE)}')
POSIX_ABSOLUTE_PATH = re.compile(r'(?:^|[\s"\'(=])(/[^\s"\'()]*)', re.MULTILINE)
WINDOWS_ABSOLUTE_PATH = re.compile(r"(?:^|[\s\"'(=])(?:[A-Za-z]:[\\/]|\\\\)[^\s\"'()]*", re.MULTILINE)
RELATIVE_RUST_PATH = re.compile(r"(?:[A-Za-z0-9_.-]+/)+[A-Za-z0-9_.-]+\.rs")
VARIABLES = ("scale", "input_data", "input_len", "output_data", "output_len", "i")
EXPECTED_OBSERVATIONS = {
    "scale": "1.5",
    "input_data": "(*mut f32) 0x<ADDR>",
    "input_len": "1",
    "output_data": "(*mut f32) 0x<ADDR>",
    "output_len": "1",
    "i": "0",
}
HARDWARE_TEST = "gfx942_cov6_alpha_then_zeta_generated_safe_spi_with_fake_authenticator"


class CheckError(Exception):
    pass


def read_bounded(path: pathlib.Path) -> str:
    if not path.is_file() or path.is_symlink():
        raise CheckError(f"input must be a regular non-symlink file: {path}")
    size = path.stat().st_size
    if size == 0 or size > MAX_INPUT_BYTES:
        raise CheckError(f"input size must be within 1..{MAX_INPUT_BYTES} bytes: {path}")
    try:
        return path.read_text(encoding="utf-8")
    except UnicodeDecodeError as error:
        raise CheckError(f"input is not UTF-8: {path}") from error


def write_new(path: pathlib.Path, text: str) -> None:
    if not path.is_absolute() or path.exists() or path.is_symlink():
        raise CheckError(f"output must be a fresh absolute path: {path}")
    if path.parent.resolve() != path.parent:
        raise CheckError(f"output parent must be canonical: {path.parent}")
    with path.open("x", encoding="utf-8", newline="\n") as output:
        output.write(text)


def require_path_hygiene(text: str) -> None:
    candidates = SOURCE_CANDIDATE.findall(text)
    if not candidates:
        raise CheckError("evidence contains no canonical S09 source path")
    for candidate in candidates:
        if candidate != S09_SOURCE:
            raise CheckError("evidence contains an absolute or non-canonical S09 source path")
    if POSIX_ABSOLUTE_PATH.search(text) or WINDOWS_ABSOLUTE_PATH.search(text):
        raise CheckError("normalized evidence contains an absolute path")
    for candidate in RELATIVE_RUST_PATH.findall(text):
        if candidate != S09_SOURCE:
            raise CheckError(f"normalized evidence contains unallowlisted source path {candidate!r}")


def normalize_line(line: str) -> str:
    line = MEMORY_URI.sub("memory://<PID>#offset=0x<ADDR>&size=<SIZE>", line)
    line = THREAD.sub("Thread <THREAD>", line)
    line = PROCESS_ID.sub("process <PID>", line)
    line = AMDGPU_LANE.sub("AMDGPU Lane <LANE>", line)
    line = AMDGPU_WAVE.sub("AMDGPU Wave <WAVE>", line)
    line = re.sub(r"^(\*? )[0-9]+( +AMDGPU Wave)", r"\1<THREAD>\2", line)
    line = ADDRESS.sub("0x<ADDR>", line)
    if line.lstrip().startswith("Starting program: "):
        line = "Starting program: $HOST_EXECUTABLE"
    return SPACE.sub(" ", line.strip())


def normalize_dwarf(text: str) -> str:
    lines = [normalize_line(line) for line in text.splitlines()]
    lines = [
        "$HSACO: file format elf64-amdgpu"
        if line.endswith(": file format elf64-amdgpu")
        else line
        for line in lines
    ]
    normalized = "\n".join(line for line in lines if line) + "\n"
    require_path_hygiene(normalized)
    return normalized


def normalize_rocgdb(text: str) -> str:
    lines = [normalize_line(line) for line in text.splitlines()]
    begin = [index for index, line in enumerate(lines) if line == "FE2O3_S09_BEGIN"]
    end = [index for index, line in enumerate(lines) if line == "FE2O3_S09_END"]
    if len(begin) != 1 or len(end) != 1 or begin[0] >= end[0]:
        raise CheckError("ROCgdb transcript must contain one ordered S09 marker interval")
    normalized = "\n".join(line for line in lines[begin[0] : end[0] + 1] if line) + "\n"
    require_path_hygiene(normalized)
    return normalized


def require_once(text: str, token: str, context: str) -> None:
    count = text.count(token)
    if count != 1:
        raise CheckError(f"{context} requires exactly one {token!r}; found {count}")


def check_dwarf(text: str) -> None:
    for token in (
        "DW_TAG_compile_unit",
        'DW_AT_producer ("fe2o3 S09 alpha gfx942 O0 v1")',
        "DW_LANG_Rust",
        "DW_TAG_subprogram",
        'DW_AT_name ("alpha")',
        f'DW_AT_comp_dir ("{S09_DIRECTORY}")',
    ):
        require_once(text, token, "DWARF")
    for token in ("DW_AT_decl_line (68)", S09_SOURCE):
        if token not in text:
            raise CheckError(f"DWARF is missing {token!r}")
    dies = re.split(r"(?=^0x<ADDR>: .*DW_TAG_)", text, flags=re.MULTILINE)
    for name in VARIABLES:
        require_once(text, f'DW_AT_name ("{name}")', "DWARF")
        matching = [
            die
            for die in dies
            if f'DW_AT_name ("{name}")' in die
            and "DW_AT_location" in die
            and (
                "DW_TAG_formal_parameter" in die
                if name != "i"
                else "DW_TAG_variable" in die
            )
        ]
        if len(matching) != 1:
            raise CheckError(f"DWARF does not contain one located DIE for {name!r}")
    if "DW_AT_decl_line (70)" not in text:
        raise CheckError("DWARF does not bind local `i` to source line 70")
    for line in (68, 69, 70):
        if not re.search(rf"(?m)^0x<ADDR> +{line} +", text):
            raise CheckError(f"DWARF line table does not contain source line {line}")
    for rejected in ("DW_AT_APPLE_optimized", "DW_AT_GNU_locviews", "DW_TAG_structure_type"):
        if rejected in text:
            raise CheckError(f"bounded S09 DWARF contains unsupported construct {rejected!r}")


def artifact_facts(metadata: str, dwarf: str) -> str:
    check_dwarf(normalize_dwarf(dwarf))
    for token in (
        "Format: elf64-amdgpu",
        "Arch: amdgcn",
        ".name:           alpha",
        ".symbol:         alpha.kd",
        ".name:           zeta",
        ".symbol:         zeta.kd",
    ):
        require_once(metadata, token, "AMDGPU artifact metadata")
    targets = re.findall(
        r"(?m)^amdhsa\.target:\s+'(amdgcn-amd-amdhsa--gfx942:xnack-)'$", metadata
    )
    if targets != ["amdgcn-amd-amdhsa--gfx942:xnack-"]:
        raise CheckError("AMDGPU artifact metadata has no exact unique gfx942:xnack- target")
    return (
        "format=fe2o3-s09-artifact-facts-v1\n"
        "object_format=elf64-amdgpu\n"
        "arch=amdgcn\n"
        "target=gfx942:xnack-\n"
        "optimization=O0\n"
        "source_path=" + S09_SOURCE + "\n"
        "kernel=alpha:alpha.kd\n"
        "kernel=zeta:zeta.kd\n"
    )


def hardware_facts(text: str, sha256: str) -> tuple[str, str]:
    if not HEX_SHA256.fullmatch(sha256):
        raise CheckError("hardware executable SHA-256 must be 64 lowercase hex digits")
    for token in ("Class:", "ELF64", "Machine:", "Advanced Micro Devices X86-64"):
        if token not in text:
            raise CheckError(f"hardware executable identity is missing {token!r}")
    build_ids = re.findall(r"(?m)^\s*Build ID: ([0-9a-f]+)$", text)
    if len(build_ids) != 1 or not HEX_BUILD_ID.fullmatch(build_ids[0]):
        raise CheckError("hardware executable must contain one bounded GNU build ID")
    build_id = build_ids[0]
    facts = (
        "format=fe2o3-s09-hardware-facts-v1\n"
        "object_format=elf64-x86-64\n"
        f"sha256={sha256}\n"
        f"build_id={build_id}\n"
    )
    return facts, build_id


def check_rocgdb(text: str, hsaco_sha256: str, hardware_sha256: str, build_id: str) -> None:
    if not HEX_SHA256.fullmatch(hsaco_sha256) or not HEX_SHA256.fullmatch(hardware_sha256):
        raise CheckError("ROCgdb binding SHA-256 values must be lowercase hex")
    if not HEX_BUILD_ID.fullmatch(build_id):
        raise CheckError("ROCgdb hardware build ID is malformed")
    markers = (
        "FE2O3_S09_BEGIN",
        "FE2O3_S09_BINDING",
        "FE2O3_S09_KERNEL_LOAD",
        "FE2O3_S09_GPU_CONTEXT",
        "FE2O3_S09_FUNCTION",
        "FE2O3_S09_BP2_ARMED",
        "FE2O3_S09_BP2_STOP",
        "FE2O3_S09_ARGUMENTS",
        "FE2O3_S09_BP3_ARMED",
        "FE2O3_S09_BP3_STOP",
        "FE2O3_S09_LOCAL",
        "FE2O3_S09_RESUME",
        "FE2O3_S09_HARDWARE_PASS",
        "FE2O3_S09_END",
    )
    positions = []
    for marker in markers:
        require_once(text, marker, "ROCgdb transcript")
        positions.append(text.index(marker))
    if positions != sorted(positions):
        raise CheckError("ROCgdb S09 markers are out of order")

    for token in (
        f"hsaco_sha256 = {hsaco_sha256}",
        f"hardware_sha256 = {hardware_sha256}",
        f"hardware_build_id = {build_id}",
        "target = gfx942:xnack-",
        "optimization = O0",
    ):
        require_once(text, token, "ROCgdb binding")

    marker_position = dict(zip(markers, positions, strict=True))
    kernel_section = text[marker_position["FE2O3_S09_KERNEL_LOAD"] : marker_position["FE2O3_S09_GPU_CONTEXT"]]
    if 'Function "alpha" not defined.' not in kernel_section or "Breakpoint 1 (alpha) pending." not in kernel_section:
        raise CheckError("ROCgdb did not prove pending alpha resolved after kernel load")
    gpu_section = text[marker_position["FE2O3_S09_GPU_CONTEXT"] : marker_position["FE2O3_S09_FUNCTION"]]
    if not re.search(r"Switching to Thread <THREAD>, lane 0 \(AMDGPU Lane <LANE>\)", gpu_section):
        raise CheckError("ROCgdb did not select an AMDGPU lane")
    if not re.search(r'(?m)^\* <THREAD> AMDGPU Wave <WAVE> .*"alpha".*' + re.escape(S09_SOURCE) + r":68$", gpu_section):
        raise CheckError("ROCgdb thread inventory does not bind alpha to an AMDGPU wave")
    if "Yes memory://<PID>#offset=0x<ADDR>&size=<SIZE>" not in gpu_section:
        raise CheckError("ROCgdb did not report a loaded in-memory AMDGPU code object")
    if not re.search(
        r'Thread <THREAD> "alpha" hit Breakpoint 1, with lanes \[0-63\], alpha .* at '
        + re.escape(S09_SOURCE)
        + r":68",
        gpu_section,
    ):
        raise CheckError("ROCgdb did not hit the loaded alpha kernel breakpoint")

    function_section = text[marker_position["FE2O3_S09_FUNCTION"] : marker_position["FE2O3_S09_BP2_ARMED"]]
    if f"at {S09_SOURCE}:68" not in function_section:
        raise CheckError("ROCgdb alpha frame is not bound to canonical source line 68")

    bp2_hit = text[marker_position["FE2O3_S09_BP2_ARMED"] : marker_position["FE2O3_S09_BP2_STOP"]]
    if not re.search(
        r'Thread <THREAD> "alpha" hit Breakpoint 2, with lanes \[0-63\], alpha .* at '
        + re.escape(S09_SOURCE)
        + r":69",
        bp2_hit,
    ):
        raise CheckError("ROCgdb did not prove the exact BP2 line-69 stop")
    bp2_context = text[marker_position["FE2O3_S09_BP2_STOP"] : marker_position["FE2O3_S09_ARGUMENTS"]]
    if not re.search(
        r'(?m)^\* <THREAD> AMDGPU Wave <WAVE> .*"alpha".*'
        + re.escape(S09_SOURCE)
        + r":69$",
        bp2_context,
    ) or f"at {S09_SOURCE}:69" not in bp2_context:
        raise CheckError("ROCgdb BP2 observations are not bound to an AMDGPU wave at line 69")

    argument_section = text[marker_position["FE2O3_S09_ARGUMENTS"] : marker_position["FE2O3_S09_BP3_ARMED"]]
    bp3_hit = text[marker_position["FE2O3_S09_BP3_ARMED"] : marker_position["FE2O3_S09_BP3_STOP"]]
    if not re.search(
        r'Thread <THREAD> "alpha" hit Breakpoint 3, with lanes \[0-63\], alpha .* at '
        + re.escape(S09_SOURCE)
        + r":70",
        bp3_hit,
    ):
        raise CheckError("ROCgdb did not prove the exact BP3 line-70 stop")
    bp3_context = text[marker_position["FE2O3_S09_BP3_STOP"] : marker_position["FE2O3_S09_LOCAL"]]
    if not re.search(
        r'(?m)^\* <THREAD> AMDGPU Wave <WAVE> .*"alpha".*'
        + re.escape(S09_SOURCE)
        + r":70$",
        bp3_context,
    ) or f"at {S09_SOURCE}:70" not in bp3_context:
        raise CheckError("ROCgdb local observation is not bound to an AMDGPU wave at line 70")
    local_section = text[marker_position["FE2O3_S09_LOCAL"] : marker_position["FE2O3_S09_RESUME"]]
    for name in VARIABLES:
        section = local_section if name == "i" else argument_section
        observations = re.findall(rf"(?m)^{re.escape(name)}\s*=\s*(\S.*)$", section)
        if len(observations) != 1:
            raise CheckError(
                f"ROCgdb requires one successful {name!r} observation; found {len(observations)}"
            )
        value = observations[0].lower()
        for rejected in (
            "<optimized out>",
            "<unavailable>",
            "could not find the frame base",
            "error reading variable",
        ):
            if rejected in value:
                raise CheckError(f"ROCgdb could not inspect {name!r}: {observations[0]}")
        if observations[0] != EXPECTED_OBSERVATIONS[name]:
            raise CheckError(f"ROCgdb observed unexpected {name!r} value: {observations[0]!r}")

    hardware_section = text[marker_position["FE2O3_S09_RESUME"] : marker_position["FE2O3_S09_HARDWARE_PASS"]]
    if f"test {HARDWARE_TEST} ... ok" not in hardware_section:
        raise CheckError("ROCgdb transcript does not contain the exact hardware test pass")
    if "test result: ok. 1 passed; 0 failed;" not in hardware_section:
        raise CheckError("ROCgdb transcript does not contain a successful hardware result")
    if "exited normally" not in hardware_section:
        raise CheckError("ROCgdb inferior did not exit normally after hardware execution")

    for rejected in ("No symbol", "Cannot access memory", "The program is not being run"):
        if rejected.lower() in text.lower():
            raise CheckError(f"ROCgdb transcript contains failure marker {rejected!r}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    for name in ("normalize-dwarf", "normalize-rocgdb"):
        command = subparsers.add_parser(name)
        command.add_argument("--input", required=True, type=pathlib.Path)
        command.add_argument("--output", required=True, type=pathlib.Path)
    command = subparsers.add_parser("check-dwarf")
    command.add_argument("--input", required=True, type=pathlib.Path)
    command = subparsers.add_parser("check-rocgdb")
    command.add_argument("--input", required=True, type=pathlib.Path)
    command.add_argument("--hsaco-sha256", required=True)
    command.add_argument("--hardware-sha256", required=True)
    command.add_argument("--hardware-build-id", required=True)
    command = subparsers.add_parser("artifact-facts")
    command.add_argument("--metadata", required=True, type=pathlib.Path)
    command.add_argument("--dwarf", required=True, type=pathlib.Path)
    command.add_argument("--output", required=True, type=pathlib.Path)
    command = subparsers.add_parser("hardware-facts")
    command.add_argument("--input", required=True, type=pathlib.Path)
    command.add_argument("--sha256", required=True)
    command.add_argument("--output", required=True, type=pathlib.Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.command == "normalize-dwarf":
        write_new(args.output, normalize_dwarf(read_bounded(args.input)))
    elif args.command == "normalize-rocgdb":
        write_new(args.output, normalize_rocgdb(read_bounded(args.input)))
    elif args.command == "check-dwarf":
        check_dwarf(normalize_dwarf(read_bounded(args.input)))
    elif args.command == "check-rocgdb":
        check_rocgdb(
            normalize_rocgdb(read_bounded(args.input)),
            args.hsaco_sha256,
            args.hardware_sha256,
            args.hardware_build_id,
        )
    elif args.command == "artifact-facts":
        write_new(
            args.output,
            artifact_facts(read_bounded(args.metadata), read_bounded(args.dwarf)),
        )
    elif args.command == "hardware-facts":
        facts, _ = hardware_facts(read_bounded(args.input), args.sha256)
        write_new(args.output, facts)
    else:
        raise AssertionError(args.command)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except CheckError as error:
        print(f"s09-debug-check: {error}", file=sys.stderr)
        raise SystemExit(1)
