#!/usr/bin/env python3
"""Fail-closed lexer for the row-softmax Verus proof corpus."""

from __future__ import annotations

import dataclasses
import pathlib
import sys
import unicodedata


FORBIDDEN_IDENTIFIERS = {
    "admit",
    "assume",
    "assume_specification",
    "axiom",
    "external",
    "external_body",
    "external_fn_specification",
    "external_trait_specification",
    "external_type_specification",
    "trusted",
}

ALLOWED_UNINTERP_DECLARATION = [
    "pub",
    "uninterp",
    "spec",
    "fn",
    "exp_real_v1",
    "(",
    "value",
    ":",
    "real",
    ")",
    "->",
    "real",
    ";",
]


class ScanError(Exception):
    """A fail-closed source rejection."""


@dataclasses.dataclass(frozen=True)
class Token:
    value: str
    line: int
    column: int


def location(source: str, offset: int) -> tuple[int, int]:
    line = source.count("\n", 0, offset) + 1
    previous_newline = source.rfind("\n", 0, offset)
    return line, offset - previous_newline


def reject_format_and_control_separators(source: str) -> None:
    for offset, character in enumerate(source):
        if character in " \t\r\n":
            continue
        category = unicodedata.category(character)
        if category.startswith("C") or category.startswith("Z"):
            line, column = location(source, offset)
            raise ScanError(
                f"forbidden Unicode {category} U+{ord(character):04X} "
                f"at {line}:{column}"
            )


def identifier_start(character: str) -> bool:
    return character == "_" or character.isalpha() or unicodedata.category(character) == "Nl"


def identifier_continue(character: str) -> bool:
    category = unicodedata.category(character)
    return identifier_start(character) or character.isdigit() or category.startswith("M")


def raw_string_end(source: str, start: int) -> int | None:
    prefix_length = 0
    if source.startswith("br", start) or source.startswith("rb", start):
        prefix_length = 2
    elif source.startswith("r", start):
        prefix_length = 1
    else:
        return None
    cursor = start + prefix_length
    hashes = 0
    while cursor < len(source) and source[cursor] == "#":
        hashes += 1
        cursor += 1
    if cursor >= len(source) or source[cursor] != '"':
        return None
    closing = '"' + "#" * hashes
    end = source.find(closing, cursor + 1)
    if end < 0:
        line, column = location(source, start)
        raise ScanError(f"unterminated raw string at {line}:{column}")
    return end + len(closing)


def string_literal_end(source: str, start: int, prefix_length: int) -> int | None:
    cursor = start + prefix_length
    if cursor >= len(source) or source[cursor] != '"':
        return None
    cursor += 1
    escaped = False
    while cursor < len(source):
        character = source[cursor]
        if escaped:
            escaped = False
        elif character == "\\":
            escaped = True
        elif character == '"':
            return cursor + 1
        cursor += 1
    line, column = location(source, start)
    raise ScanError(f"unterminated string at {line}:{column}")


def character_literal_end(source: str, start: int, prefix_length: int) -> int | None:
    quote = start + prefix_length
    if quote >= len(source) or source[quote] != "'":
        return None
    cursor = quote + 1
    if cursor >= len(source) or source[cursor] in "'\r\n":
        return None
    if source[cursor] != "\\":
        cursor += 1
    else:
        cursor += 1
        if cursor >= len(source):
            return None
        escape = source[cursor]
        cursor += 1
        if escape == "x":
            digits = source[cursor : cursor + 2]
            if len(digits) != 2 or any(digit not in "0123456789abcdefABCDEF" for digit in digits):
                return None
            cursor += 2
        elif escape == "u":
            if cursor >= len(source) or source[cursor] != "{":
                return None
            closing = source.find("}", cursor + 1)
            if closing < 0:
                return None
            digits = source[cursor + 1 : closing].replace("_", "")
            if not 1 <= len(digits) <= 6 or any(
                digit not in "0123456789abcdefABCDEF" for digit in digits
            ):
                return None
            cursor = closing + 1
        elif escape not in "0nrt\\'\"":
            return None
    if cursor >= len(source) or source[cursor] != "'":
        return None
    return cursor + 1


def lex(source: str) -> list[Token]:
    reject_format_and_control_separators(source)
    tokens: list[Token] = []
    cursor = 0
    while cursor < len(source):
        character = source[cursor]
        if character in " \t\r\n":
            cursor += 1
            continue
        if source.startswith("//", cursor):
            newline = source.find("\n", cursor + 2)
            cursor = len(source) if newline < 0 else newline + 1
            continue
        if source.startswith("/*", cursor):
            comment_start = cursor
            cursor += 2
            depth = 1
            while cursor < len(source) and depth > 0:
                if source.startswith("/*", cursor):
                    depth += 1
                    cursor += 2
                elif source.startswith("*/", cursor):
                    depth -= 1
                    cursor += 2
                else:
                    cursor += 1
            if depth != 0:
                line, column = location(source, comment_start)
                raise ScanError(f"unterminated block comment at {line}:{column}")
            continue

        raw_end = raw_string_end(source, cursor)
        if raw_end is not None:
            cursor = raw_end
            continue
        literal_end = None
        for prefix in ("b\"", "c\"", '"'):
            if source.startswith(prefix, cursor):
                literal_end = string_literal_end(source, cursor, len(prefix) - 1)
                if literal_end is not None:
                    break
        if literal_end is None:
            for prefix in ("b'", "'"):
                if source.startswith(prefix, cursor):
                    literal_end = character_literal_end(source, cursor, len(prefix) - 1)
                    if literal_end is not None:
                        break
        if literal_end is not None:
            cursor = literal_end
            continue

        line, column = location(source, cursor)
        if identifier_start(character):
            end = cursor + 1
            while end < len(source) and identifier_continue(source[end]):
                end += 1
            identifier = unicodedata.normalize("NFKC", source[cursor:end])
            if not identifier.isascii():
                raise ScanError(f"non-ASCII code identifier {identifier!r} at {line}:{column}")
            tokens.append(Token(identifier, line, column))
            cursor = end
            continue
        if ord(character) > 0x7F:
            raise ScanError(
                f"non-ASCII code token U+{ord(character):04X} at {line}:{column}"
            )
        punctuator = next(
            (
                value
                for value in ("::", "->", "=>", "&&&", "==>")
                if source.startswith(value, cursor)
            ),
            character,
        )
        tokens.append(Token(punctuator, line, column))
        cursor += len(punctuator)
    return tokens


def validate(tokens: list[Token]) -> None:
    for token in tokens:
        if token.value in FORBIDDEN_IDENTIFIERS:
            raise ScanError(
                f"forbidden proof token {token.value!r} at {token.line}:{token.column}"
            )
    for index, token in enumerate(tokens):
        if token.value != "uninterp":
            continue
        start = index - 1
        values = [
            candidate.value
            for candidate in tokens[start : start + len(ALLOWED_UNINTERP_DECLARATION)]
        ]
        if start < 0 or values != ALLOWED_UNINTERP_DECLARATION:
            raise ScanError(
                f"unapproved uninterpreted declaration at {token.line}:{token.column}"
            )


def scan(path: pathlib.Path) -> None:
    try:
        source = path.read_text(encoding="utf-8", errors="strict")
    except (OSError, UnicodeError) as error:
        raise ScanError(f"cannot read UTF-8 proof source: {error}") from error
    validate(lex(source))


def main(arguments: list[str]) -> int:
    if not arguments:
        print(f"usage: {pathlib.Path(sys.argv[0]).name} PROOF [PROOF ...]", file=sys.stderr)
        return 2
    for argument in arguments:
        path = pathlib.Path(argument)
        try:
            scan(path)
        except ScanError as error:
            print(f"FAIL: {path}: {error}", file=sys.stderr)
            return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
