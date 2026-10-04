#!/usr/bin/env python3
"""Fail-closed lexical audit for the FlashAttention Verus evidence."""

from __future__ import annotations

import re
import sys
import unicodedata
from pathlib import Path

FORBIDDEN_IDENTIFIERS = {
    "admit",
    "assume",
    "axiom",
    "external_body",
    "include",
    "include_bytes",
    "include_str",
    "macro_rules",
    "mod",
}


class ScanError(Exception):
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
        raise ScanError("unterminated raw string")
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
    raise ScanError("unterminated quoted literal")


def code_only(source: str) -> str:
    result: list[str] = []
    cursor = 0
    while cursor < len(source):
        if source.startswith("//", cursor):
            end = source.find("\n", cursor + 2)
            cursor = len(source) if end < 0 else end
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
                raise ScanError("unterminated block comment")
        else:
            raw_end = raw_string_end(source, cursor)
            if raw_end is not None:
                cursor = raw_end
            elif source.startswith('b"', cursor):
                cursor = quoted_end(source, cursor + 1, '"')
            elif source[cursor] == '"':
                cursor = quoted_end(source, cursor, '"')
            elif source.startswith("b'", cursor):
                cursor = quoted_end(source, cursor + 1, "'")
            elif source[cursor] == "'":
                cursor = quoted_end(source, cursor, "'")
            else:
                result.append(source[cursor])
                cursor += 1
    return "".join(result)


def scan(path: Path) -> None:
    source = path.read_text(encoding="utf-8")
    normalized = unicodedata.normalize("NFKC", source)
    if normalized != source:
        raise ScanError("source changes under Unicode NFKC normalization")
    for character in source:
        category = unicodedata.category(character)
        if category.startswith("C") and character in {"\u200e", "\u200f", "\u202a", "\u202e"}:
            raise ScanError(f"forbidden Unicode category {category}")
        if category.startswith("Z") and character not in {" ", "\n"}:
            raise ScanError(f"forbidden Unicode category {category}")

    code = code_only(source)
    identifiers = re.findall(r"[A-Za-z_][A-Za-z0-9_]*", code)
    forbidden = sorted(set(identifiers) & FORBIDDEN_IDENTIFIERS)
    if forbidden:
        raise ScanError(f"forbidden proof identifier '{forbidden[0]}'")
    if re.search(r"#\s*!?\s*\[\s*(?:cfg|cfg_attr)\b", code):
        raise ScanError("conditional proof source is forbidden")

    uninterpreted = [name for name in identifiers if name == "uninterp"]
    allowed = re.findall(
        r"pub\s+uninterp\s+spec\s+fn\s+exp_weight_v1\s*\(", code
    )
    if path.name == "flash_attention_v1.rs":
        if len(uninterpreted) != 1 or len(allowed) != 1:
            raise ScanError("exactly one named exponential abstraction is required")
    elif uninterpreted:
        raise ScanError("unexpected uninterpreted function in mutation source")


def main() -> int:
    if len(sys.argv) < 2:
        print(f"usage: {sys.argv[0]} SOURCE...", file=sys.stderr)
        return 2
    for argument in sys.argv[1:]:
        path = Path(argument)
        try:
            scan(path)
        except (OSError, UnicodeError, ScanError) as error:
            print(f"FAIL: {path}: {error}", file=sys.stderr)
            return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
