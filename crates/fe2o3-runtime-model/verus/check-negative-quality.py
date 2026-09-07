#!/usr/bin/env python3
"""Reject name-only expected-negative contradictions in runtime Verus sources."""

from __future__ import annotations

import re
import sys
from pathlib import Path


class QualityError(Exception):
    pass


def raw_string_end(source: str, start: int) -> int | None:
    cursor = start
    if source.startswith("br", cursor):
        cursor += 2
    elif source.startswith("r", cursor):
        cursor += 1
    else:
        return None
    hashes = 0
    while cursor < len(source) and source[cursor] == "#":
        hashes += 1
        cursor += 1
    if cursor >= len(source) or source[cursor] != '"':
        return None
    terminator = '"' + "#" * hashes
    end = source.find(terminator, cursor + 1)
    if end < 0:
        raise QualityError("unterminated raw string")
    return end + len(terminator)


def quoted_end(source: str, start: int, quote: str) -> int:
    cursor = start + 1
    while cursor < len(source):
        if source[cursor] == "\\":
            cursor += 2
        elif source[cursor] == quote:
            return cursor + 1
        else:
            cursor += 1
    raise QualityError("unterminated quoted literal")


def lifetime_end(source: str, start: int) -> int | None:
    cursor = start + 1
    if cursor >= len(source) or not (source[cursor].isalpha() or source[cursor] == "_"):
        return None
    cursor += 1
    while cursor < len(source) and (source[cursor].isalnum() or source[cursor] == "_"):
        cursor += 1
    if cursor < len(source) and source[cursor] == "'":
        return None
    return cursor


def code_only(source: str) -> str:
    result: list[str] = []
    cursor = 0
    while cursor < len(source):
        if source.startswith("//", cursor):
            end = source.find("\n", cursor + 2)
            cursor = len(source) if end < 0 else end
            result.append(" ")
        elif source.startswith("/*", cursor):
            depth = 1
            cursor += 2
            while cursor < len(source) and depth:
                if source.startswith("/*", cursor):
                    depth += 1
                    cursor += 2
                elif source.startswith("*/", cursor):
                    depth -= 1
                    cursor += 2
                else:
                    cursor += 1
            if depth:
                raise QualityError("unterminated block comment")
            result.append(" ")
        else:
            raw_end = raw_string_end(source, cursor)
            if raw_end is not None:
                cursor = raw_end
                result.append(" ")
            elif source.startswith('b"', cursor):
                cursor = quoted_end(source, cursor + 1, '"')
                result.append(" ")
            elif source[cursor] == '"':
                cursor = quoted_end(source, cursor, '"')
                result.append(" ")
            elif source.startswith("b'", cursor):
                cursor = quoted_end(source, cursor + 1, "'")
                result.append(" ")
            elif source[cursor] == "'":
                lifetime = lifetime_end(source, cursor)
                if lifetime is None:
                    cursor = quoted_end(source, cursor, "'")
                    result.append(" ")
                else:
                    result.append(source[cursor:lifetime])
                    cursor = lifetime
            else:
                result.append(source[cursor])
                cursor += 1
    return "".join(result)


LITERAL_TRUE_SPEC = re.compile(
    r"\b(?:pub\s+)?(?:open\s+)?spec\s+fn\s+"
    r"(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*"
    r"\(\s*\)\s*->\s*bool\s*\{\s*true\s*\}",
    re.DOTALL,
)


def scan_source(source: str) -> None:
    code = code_only(source)
    for match in LITERAL_TRUE_SPEC.finditer(code):
        name = match.group("name")
        direct_negation = re.compile(rf"\bensures\s*!\s*{re.escape(name)}\s*\(\s*\)")
        if direct_negation.search(code):
            raise QualityError(
                f"zero-argument literal-true predicate '{name}' is directly negated"
            )


def scan_path(path: Path) -> None:
    try:
        source = path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        raise QualityError(str(error)) from error
    scan_source(source)


def runtime_negative_sources(directory: Path) -> list[Path]:
    if not directory.is_dir():
        raise QualityError(f"negative source directory is unavailable: {directory}")
    entries = sorted(directory.iterdir())
    unexpected = [
        path
        for path in entries
        if path.is_symlink() or not path.is_file() or path.suffix != ".rs"
    ]
    if unexpected:
        raise QualityError(f"unexpected negative source entry: {unexpected[0]}")
    sources = entries
    if not sources:
        raise QualityError(f"negative source directory is empty: {directory}")
    return sources


def self_test(reject_fixture: Path, accept_fixture: Path) -> None:
    try:
        scan_path(reject_fixture)
    except QualityError as error:
        if "zero-argument literal-true predicate" not in str(error):
            raise QualityError(f"reject fixture failed unexpectedly: {error}") from error
    else:
        raise QualityError("reject fixture unexpectedly passed")
    scan_path(accept_fixture)


def usage() -> int:
    print(
        f"usage: {sys.argv[0]} DIRECTORY\n"
        f"       {sys.argv[0]} --self-test REJECT_FIXTURE ACCEPT_FIXTURE",
        file=sys.stderr,
    )
    return 2


def main() -> int:
    try:
        if len(sys.argv) == 4 and sys.argv[1] == "--self-test":
            self_test(Path(sys.argv[2]), Path(sys.argv[3]))
            print("PASS: runtime expected-negative quality self-test")
            return 0
        if len(sys.argv) != 2 or sys.argv[1] == "--self-test":
            return usage()
        sources = runtime_negative_sources(Path(sys.argv[1]))
        for path in sources:
            try:
                scan_path(path)
            except QualityError as error:
                raise QualityError(f"{path}: {error}") from error
        print(f"PASS: runtime expected-negative quality checked {len(sources)} files")
        return 0
    except QualityError as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
