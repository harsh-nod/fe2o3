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
AMDGPU_LANE = re.compile(r"AMDGPU Lane [^]]+\]")
SPACE = re.compile(r"[ \t]+")
S09_SOURCE = "crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src/main.rs"
SOURCE_PATH = re.compile(rf'(?:[^\s"(]*/)?{re.escape(S09_SOURCE)}')
VARIABLES = ("scale", "input_data", "input_len", "output_data", "output_len", "i")
EXPECTED_OBSERVATIONS = {
    "scale": "1.5",
    "input_data": "(*mut f32) 0x<ADDR>",
    "input_len": "1",
    "output_data": "(*mut f32) 0x<ADDR>",
    "output_len": "1",
    "i": "0",
}


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


def normalize_line(line: str) -> str:
    line = THREAD.sub("Thread <THREAD>", line)
    line = PROCESS_ID.sub("process <PID>", line)
    line = AMDGPU_LANE.sub("AMDGPU Lane <LANE>", line)
    line = ADDRESS.sub("0x<ADDR>", line)
    line = SOURCE_PATH.sub(f"$REPO/{S09_SOURCE}", line)
    return SPACE.sub(" ", line.strip())


def normalize_dwarf(text: str) -> str:
    lines = [normalize_line(line) for line in text.splitlines()]
    lines = [
        "$HSACO: file format elf64-amdgpu"
        if line.endswith(": file format elf64-amdgpu")
        else line
        for line in lines
    ]
    return "\n".join(line for line in lines if line) + "\n"


def normalize_rocgdb(text: str) -> str:
    lines = [normalize_line(line) for line in text.splitlines()]
    begin = [index for index, line in enumerate(lines) if line == "FE2O3_S09_BEGIN"]
    end = [index for index, line in enumerate(lines) if line == "FE2O3_S09_END"]
    if len(begin) != 1 or len(end) != 1 or begin[0] >= end[0]:
        raise CheckError("ROCgdb transcript must contain one ordered S09 marker interval")
    return "\n".join(line for line in lines[begin[0] : end[0] + 1] if line) + "\n"


def require_once(text: str, token: str, context: str) -> None:
    count = text.count(token)
    if count != 1:
        raise CheckError(f"{context} requires exactly one {token!r}; found {count}")


def check_dwarf(text: str) -> None:
    for token in (
        "DW_TAG_compile_unit",
        "DW_LANG_Rust",
        "DW_TAG_subprogram",
        'DW_AT_name (\"alpha\")',
        "DW_AT_decl_line (68)",
        S09_SOURCE,
    ):
        if token not in text:
            raise CheckError(f"DWARF is missing {token!r}")
    dies = re.split(r"(?=^0x<ADDR>: .*DW_TAG_)", text, flags=re.MULTILINE)
    for name in VARIABLES:
        require_once(text, f'DW_AT_name (\"{name}\")', "DWARF")
        matching = [
            die
            for die in dies
            if f'DW_AT_name (\"{name}\")' in die
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


def check_rocgdb(text: str) -> None:
    markers = (
        "FE2O3_S09_BEGIN",
        "FE2O3_S09_FUNCTION",
        "FE2O3_S09_ARGUMENTS",
        "FE2O3_S09_LOCAL",
        "FE2O3_S09_END",
    )
    positions = []
    for marker in markers:
        require_once(text, marker, "ROCgdb transcript")
        positions.append(text.index(marker))
    if positions != sorted(positions):
        raise CheckError("ROCgdb S09 markers are out of order")
    if not re.search(r"Breakpoint [^\n]*\balpha\b", text):
        raise CheckError("ROCgdb did not stop in alpha")
    for line in (69, 70):
        if f"main.rs:{line}" not in text and f"line {line}" not in text.lower():
            raise CheckError(f"ROCgdb did not report supported alpha source line {line}")
    argument_section = text[positions[2] : positions[3]]
    local_section = text[positions[3] : positions[4]]
    for name in VARIABLES:
        section = local_section if name == "i" else argument_section
        observations = re.findall(
            rf"(?m)^{re.escape(name)}\s*=\s*(\S.*)$", section
        )
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
            raise CheckError(
                f"ROCgdb observed unexpected {name!r} value: {observations[0]!r}"
            )
    for rejected in (
        "No symbol",
        "Cannot access memory",
        "not defined",
        "The program is not being run",
    ):
        if rejected.lower() in text.lower():
            raise CheckError(f"ROCgdb transcript contains failure marker {rejected!r}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    for name in ("normalize-dwarf", "normalize-rocgdb"):
        command = subparsers.add_parser(name)
        command.add_argument("--input", required=True, type=pathlib.Path)
        command.add_argument("--output", required=True, type=pathlib.Path)
    for name in ("check-dwarf", "check-rocgdb"):
        command = subparsers.add_parser(name)
        command.add_argument("--input", required=True, type=pathlib.Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    text = read_bounded(args.input)
    if args.command == "normalize-dwarf":
        write_new(args.output, normalize_dwarf(text))
    elif args.command == "normalize-rocgdb":
        write_new(args.output, normalize_rocgdb(text))
    elif args.command == "check-dwarf":
        check_dwarf(normalize_dwarf(text))
    elif args.command == "check-rocgdb":
        check_rocgdb(normalize_rocgdb(text))
    else:
        raise AssertionError(args.command)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except CheckError as error:
        print(f"s09-debug-check: {error}", file=sys.stderr)
        raise SystemExit(1)
