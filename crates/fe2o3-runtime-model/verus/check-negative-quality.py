#!/usr/bin/env python3
"""Audit runtime expected-negative source quality and runner inventory."""

from __future__ import annotations

import hashlib
import os
import re
import sys
import tempfile
from collections import Counter
from pathlib import Path


EXPECTED_RUNTIME_NEGATIVE_COUNT = 650
EXPECTED_CHECK_NEGATIVE_FUNCTION_SHA256 = (
    "030b914bc4d005c3363972f5469591b6f4954872785932c66d7cfed94e779636"
)
EXPECTED_RUNNER_FUNCTION_SHA256 = {
    "read_pin": "dd0f063d2e13126778cfec4fc6bd9a89af38a0b55ed45491e59eff830a3448aa",
    "check_digest": "1b1af7d88401a6baa244c63b3edd41fd15b07814e02f3d1bfb4b1841c33819e0",
    "check_sources": "7e6a646b36ae62e00d5c7e8f4f008ed659f2254083cabe7c7b16ea1123a3559e",
    "run_verus": "a22ae6cb1d34ec0ea6b402ecf96008773cefcc620e511aae7bc3d6e495b5cf66",
    "check_positive": "2f353f8881c8def07ede3a251bb64de9148b92c802eda65b768a64172cd65d92",
    "seal_authority": "3e49725d7555f6816455cac2080aae192a8ad6210f62f5be5d8f19ac395d227a",
}
EXPECTED_PRESEAL_AUTHORITY_ASSIGNMENT_COUNT = 1428
EXPECTED_PRESEAL_AUTHORITY_ASSIGNMENTS_SHA256 = (
    "bad65467db7f59ddfb759f2353cb94aea4519d75bda58fce1d4e00b78467e85f"
)
EXPECTED_RUNNER_SHA256 = (
    "819b98162a9b71284df16eea1894c8261269fea3e1141df408dd699917829cd2"
)
HEX_SHA256 = re.compile(r"[0-9a-f]{64}")
IDENTIFIER = r"(?:r#)?[A-Za-z_][A-Za-z0-9_]*"
TOKEN = re.compile(rf"{IDENTIFIER}|->|==>|::|!=|==|<=|>=|&&|\|\||[^\s]")
SOURCE_ASSIGNMENT = re.compile(
    r"^[ \t]*(negative_[A-Za-z0-9_]+)[ \t]*=[ \t]*"
    r'"\$script_dir/negative/([A-Za-z0-9_.-]+\.rs)"[ \t]*$',
    re.MULTILINE,
)
PIN_ASSIGNMENT = re.compile(
    r"^[ \t]*(expected_negative_[A-Za-z0-9_]+)[ \t]*=[ \t]*"
    r'\$\([ \t]*read_pin[ \t]+"\$pin_dir/([A-Z0-9_]+)"[ \t]*\)[ \t]*$',
    re.MULTILINE,
)
DIGEST_CHECK = re.compile(
    r'^[ \t]*check_digest[ \t]+"\$(expected_negative_[A-Za-z0-9_]+)"[ \t]+'
    r'"\$(negative_[A-Za-z0-9_]+)"[ \t]*$',
    re.MULTILINE,
)
NEGATIVE_CALL = re.compile(
    r'^[ \t]*check_negative[ \t]+"\$(negative_[A-Za-z0-9_]+)"[ \t]+'
    r"([A-Za-z_][A-Za-z0-9_]*)[ \t]+([A-Za-z0-9_-]+)[ \t]*$",
    re.MULTILINE,
)
TRANSCRIPT_COUNT = re.compile(
    r"^[ \t]*transcript[ \t]*=[ \t]*'[^'\n]*"
    r"\bexpected_negative_files=([0-9]+)\b[^'\n]*'[ \t]*$",
    re.MULTILINE,
)
AUXILIARY_NEGATIVE_PINS = {
    "expected_negative_quality_checker",
    "expected_negative_quality_reject_fixture",
    "expected_negative_quality_accept_fixture",
}
RUNNER_ENVIRONMENT_BLOCK = (
    "PATH=/usr/bin:/bin",
    "\\readonly PATH IFS",
    "\\export PATH",
)
RUNNER_ROOT_BINDING_BLOCK = (
    'script_dir=$(CDPATH=\'\' cd -- "$(dirname -- "$0")" && pwd)',
    "repo_root=$(CDPATH='' cd -- \"$script_dir/../../..\" && pwd)",
    "\\readonly script_dir repo_root",
)
RUNNER_TOOL_BINDING_BLOCK = (
    'closure_manifest="$pin_dir/VERUS_CLOSURE_MANIFEST"',
    'closure_checker="$repo_root/examples/row_softmax_v1/verify-verus-closure.sh"',
    'source_checker="$repo_root/examples/wave64_collectives_v1/check-proof-source.py"',
    'negative_quality_checker="$script_dir/check-negative-quality.py"',
    'negative_quality_reject_fixture="$script_dir/tests/fixtures/negative-quality-direct-literal.rs"',
    'negative_quality_accept_fixture="$script_dir/tests/fixtures/negative-quality-adverse-input.rs"',
    "\\readonly closure_manifest closure_checker source_checker negative_quality_checker negative_quality_reject_fixture negative_quality_accept_fixture",
)
RUNNER_NEGATIVE_QUALITY_INVOCATION_BLOCK = (
    '/usr/bin/env -i PATH=/usr/bin:/bin /usr/bin/python3 -I "$negative_quality_checker" --self-test \\',
    '    "$negative_quality_reject_fixture" \\',
    '    "$negative_quality_accept_fixture"',
    '/usr/bin/env -i PATH=/usr/bin:/bin /usr/bin/python3 -I "$negative_quality_checker" "$script_dir/negative" "$script_dir/verify-verus.sh"',
)
RUNNER_NEGATIVE_QUALITY_AUDIT = RUNNER_NEGATIVE_QUALITY_INVOCATION_BLOCK[-1]
RUNNER_SOURCE_CHECKER_INVOCATION = (
    '/usr/bin/env -i PATH=/usr/bin:/bin /usr/bin/python3 -I "$source_checker" \\'
)
RUNNER_TOOL_DIGEST_LINES = (
    '    check_digest "$expected_closure" "$closure_manifest"',
    "    check_digest 'c0f5f201dca9ea6b3fa953884cdfaca8ca38413ad2a9de7700b3aaeb3a610d0c' \"$closure_checker\"",
    '    check_digest "$expected_source_checker" "$source_checker"',
    '    check_digest "$expected_negative_quality_checker" "$negative_quality_checker"',
    '    check_digest "$expected_negative_quality_reject_fixture" "$negative_quality_reject_fixture"',
    '    check_digest "$expected_negative_quality_accept_fixture" "$negative_quality_accept_fixture"',
)
RUNNER_TOOL_NAMES = (
    "closure_manifest",
    "closure_checker",
    "source_checker",
    "negative_quality_checker",
    "negative_quality_reject_fixture",
    "negative_quality_accept_fixture",
)
RUNNER_FUNCTION_NAMES = (
    "read_pin",
    "check_digest",
    "check_sources",
    "run_verus",
    "check_positive",
    "check_negative",
    "seal_authority",
)
RUNNER_LATE_ASSIGNMENT_LINES = (
    '    */*) [ -x "$verus_bin" ] && verus_path=$verus_bin || verus_path= ;;',
    '    *) verus_path=$(command -v "$verus_bin" 2>/dev/null || true) ;;',
    'verus_path=$(/usr/bin/readlink -f "$verus_path")',
    'verus_root=$(CDPATH=\'\' cd -- "$(dirname -- "$verus_path")" && pwd)',
    "runner_home=${HOME:-/nonexistent}",
    'runner_path="$runner_home/.cargo/bin:/usr/local/bin:/usr/bin:/bin"',
    'runner_rustup_home=${RUSTUP_HOME:-"$runner_home/.rustup"}',
    'runner_cargo_home=${CARGO_HOME:-"$runner_home/.cargo"}',
    "timeout_seconds=${VERUS_TIMEOUT_SECONDS:-120}",
    'tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-runtime-model-verus.XXXXXX")',
)
RUNNER_LATE_READONLY_LINES = (
    "\\readonly verus_path verus_root",
    "\\readonly runner_home runner_path runner_rustup_home runner_cargo_home",
    "\\readonly timeout_seconds",
    "\\readonly tmp_dir",
)
RUNNER_LATE_BINDING_BLOCKS = (
    (
        'verus_root=$(CDPATH=\'\' cd -- "$(dirname -- "$verus_path")" && pwd)',
        "\\readonly verus_path verus_root",
        '"$closure_checker" "$verus_root" "$closure_manifest"',
    ),
    (
        "runner_home=${HOME:-/nonexistent}",
        'runner_path="$runner_home/.cargo/bin:/usr/local/bin:/usr/bin:/bin"',
        'runner_rustup_home=${RUSTUP_HOME:-"$runner_home/.rustup"}',
        'runner_cargo_home=${CARGO_HOME:-"$runner_home/.cargo"}',
        "\\readonly runner_home runner_path runner_rustup_home runner_cargo_home",
    ),
    (
        "timeout_seconds=${VERUS_TIMEOUT_SECONDS:-120}",
        "\\readonly timeout_seconds",
    ),
    (
        'tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-runtime-model-verus.XXXXXX")',
        "\\readonly tmp_dir",
    ),
)
RUNNER_LATE_NAMES = (
    "verus_path",
    "verus_root",
    "runner_home",
    "runner_path",
    "runner_rustup_home",
    "runner_cargo_home",
    "timeout_seconds",
    "tmp_dir",
)


class QualityError(Exception):
    def __init__(self, message: str, *, code: str = "quality.unclassified") -> None:
        super().__init__(message)
        self.code = code


def raw_string_end(source: str, start: int) -> int | None:
    cursor = start
    if source.startswith("br", cursor):
        cursor += 2
    elif source.startswith("r", cursor):
        cursor += 1
    else:
        return None
    while cursor < len(source) and source[cursor] == "#":
        cursor += 1
    if cursor >= len(source) or source[cursor] != '"':
        return None
    prefix_length = 2 if source.startswith("br", start) else 1
    hashes = cursor - start - prefix_length
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


def matching_token(
    tokens: list[str], start: int, opener: str, closer: str
) -> int | None:
    depth = 0
    for cursor in range(start, len(tokens)):
        if tokens[cursor] == opener:
            depth += 1
        elif tokens[cursor] == closer:
            depth -= 1
            if depth == 0:
                return cursor
    return None


def literal_body(tokens: list[str]) -> str | None:
    body = tokens
    while len(body) >= 2 and body[0] in {"(", "{"}:
        closer = ")" if body[0] == "(" else "}"
        end = matching_token(body, 0, body[0], closer)
        if end != len(body) - 1:
            break
        body = body[1:-1]
    if len(body) == 1 and body[0] in {"true", "false"}:
        return body[0]
    return None


def direct_bool_type_end(tokens: list[str], start: int) -> int | None:
    if start >= len(tokens):
        return None
    if tokens[start] == "(":
        inner_end = direct_bool_type_end(tokens, start + 1)
        if inner_end is None or inner_end >= len(tokens) or tokens[inner_end] != ")":
            return None
        return inner_end + 1
    for spelling in (
        ["bool"],
        ["core", "::", "primitive", "::", "bool"],
        ["std", "::", "primitive", "::", "bool"],
        ["::", "core", "::", "primitive", "::", "bool"],
        ["::", "std", "::", "primitive", "::", "bool"],
    ):
        if tokens[start : start + len(spelling)] == spelling:
            return start + len(spelling)
    return None


def literal_bool_specs(source: str) -> list[tuple[str, str]]:
    code = code_only(source)
    if any(not character.isascii() for character in code):
        raise QualityError("non-ASCII Rust code is outside the audited source grammar")
    tokens = TOKEN.findall(code)
    found: list[tuple[str, str]] = []
    cursor = 0
    while cursor < len(tokens):
        if tokens[cursor] != "spec":
            cursor += 1
            continue
        scan = cursor + 1
        if scan < len(tokens) and tokens[scan] == "(":
            annotated = matching_token(tokens, scan, "(", ")")
            if annotated is None:
                raise QualityError("unterminated spec annotation")
            scan = annotated + 1
        if scan >= len(tokens) or tokens[scan] != "fn":
            cursor += 1
            continue
        scan += 1
        if scan >= len(tokens) or not re.fullmatch(IDENTIFIER, tokens[scan]):
            cursor += 1
            continue
        name = tokens[scan]
        scan += 1
        if scan < len(tokens) and tokens[scan] == "<":
            generic_end = matching_token(tokens, scan, "<", ">")
            if generic_end is None:
                raise QualityError(
                    f"unterminated generic parameters for spec fn '{name}'"
                )
            scan = generic_end + 1
        if scan >= len(tokens) or tokens[scan] != "(":
            cursor += 1
            continue
        arguments_end = matching_token(tokens, scan, "(", ")")
        if arguments_end is None:
            raise QualityError(f"unterminated arguments for spec fn '{name}'")
        if arguments_end != scan + 1:
            cursor = arguments_end + 1
            continue
        scan = arguments_end + 1
        if scan >= len(tokens) or tokens[scan] != "->":
            cursor = scan
            continue
        return_end = direct_bool_type_end(tokens, scan + 1)
        if return_end is None:
            cursor = scan + 1
            continue
        scan = return_end
        while scan < len(tokens) and tokens[scan] not in {"{", ";"}:
            scan += 1
        if scan >= len(tokens) or tokens[scan] != "{":
            cursor = scan + 1
            continue
        body_end = matching_token(tokens, scan, "{", "}")
        if body_end is None:
            raise QualityError(f"unterminated body for spec fn '{name}'")
        literal = literal_body(tokens[scan + 1 : body_end])
        if literal is not None:
            found.append((name, literal))
        cursor = body_end + 1
    return found


def scan_source(source: str) -> None:
    literals = literal_bool_specs(source)
    if literals:
        rendered = ", ".join(f"{name}={literal}" for name, literal in literals)
        raise QualityError(f"zero-argument literal-bool spec constant(s): {rendered}")


def scan_path(path: Path) -> None:
    try:
        source = path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        raise QualityError(str(error)) from error
    scan_source(source)


def runtime_negative_sources(directory: Path) -> list[Path]:
    if directory.is_symlink() or not directory.is_dir():
        raise QualityError(
            f"negative source directory is unavailable: {directory}",
            code="source.directory_unavailable",
        )
    entries = sorted(directory.iterdir())
    unexpected = [
        path
        for path in entries
        if path.is_symlink() or not path.is_file() or path.suffix != ".rs"
    ]
    if unexpected:
        raise QualityError(
            f"unexpected negative source entry: {unexpected[0]}",
            code="source.unexpected_entry",
        )
    if not entries:
        raise QualityError(
            f"negative source directory is empty: {directory}",
            code="source.empty_directory",
        )
    return entries


def require_unique(stage: str, values: list[str]) -> None:
    duplicates = sorted(value for value, count in Counter(values).items() if count != 1)
    if duplicates:
        raise QualityError(
            f"duplicate {stage}: {duplicates[0]}",
            code=f"roster.duplicate.{stage.replace(' ', '_')}",
        )


def shell_line_code(line: str) -> str:
    single_quoted = False
    double_quoted = False
    escaped = False
    for cursor, character in enumerate(line):
        if escaped:
            escaped = False
            continue
        if character == "\\" and not single_quoted:
            escaped = True
            continue
        if character == "'" and not double_quoted:
            single_quoted = not single_quoted
            continue
        if character == '"' and not single_quoted:
            double_quoted = not double_quoted
            continue
        if (
            character == "#"
            and not single_quoted
            and not double_quoted
            and (cursor == 0 or line[cursor - 1].isspace())
        ):
            return line[:cursor].rstrip()
    if single_quoted or double_quoted:
        raise QualityError(
            "unterminated shell quote in verification runner",
            code="runner.grammar.unterminated_quote",
        )
    return line.rstrip()


def shell_unquoted_projection(line: str) -> str:
    projection: list[str] = []
    single_quoted = False
    double_quoted = False
    escaped = False
    for character in line:
        if escaped:
            projection.append(" ")
            escaped = False
        elif character == "\\" and not single_quoted:
            projection.append(" ")
            escaped = True
        elif character == "'" and not double_quoted:
            single_quoted = not single_quoted
            projection.append(" ")
        elif character == '"' and not single_quoted:
            double_quoted = not double_quoted
            projection.append(" ")
        elif single_quoted or double_quoted:
            projection.append(" ")
        else:
            projection.append(character)
    return "".join(projection)


def shell_active_projection(line: str) -> str:
    projection: list[str] = []
    single_quoted = False
    double_quoted = False
    escaped = False
    for character in line:
        if escaped:
            projection.append(" ")
            escaped = False
        elif character == "\\" and not single_quoted:
            projection.append(" ")
            escaped = True
        elif character == "'" and not double_quoted:
            single_quoted = not single_quoted
            projection.append(" ")
        elif character == '"' and not single_quoted:
            double_quoted = not double_quoted
            projection.append(character)
        elif single_quoted:
            projection.append(" ")
        else:
            projection.append(character)
    return "".join(projection)


def shell_dequoted_projection(line: str) -> str:
    projection: list[str] = []
    single_quoted = False
    double_quoted = False
    escaped = False
    for character in line:
        if escaped:
            projection.append(character)
            escaped = False
        elif character == "\\" and not single_quoted:
            escaped = True
        elif character == "'" and not double_quoted:
            single_quoted = not single_quoted
        elif character == '"' and not single_quoted:
            double_quoted = not double_quoted
        else:
            projection.append(character)
    return "".join(projection)


def normalized_runner_source(runner_source: str) -> str:
    return "\n".join(shell_line_code(line) for line in runner_source.splitlines())


def reject_token_split_continuations(runner_source: str) -> None:
    cursor = 0
    single_quoted = False
    double_quoted = False
    comment = False
    while cursor < len(runner_source):
        character = runner_source[cursor]
        if comment:
            if character == "\\" and cursor + 1 < len(runner_source):
                if runner_source[cursor + 1] == "\n":
                    cursor += 2
                    continue
                if runner_source[cursor + 1 : cursor + 3] == "\r\n":
                    cursor += 3
                    continue
            if character == "\n":
                comment = False
            cursor += 1
            continue
        if character == "'" and not double_quoted:
            single_quoted = not single_quoted
            cursor += 1
            continue
        if character == '"' and not single_quoted:
            double_quoted = not double_quoted
            cursor += 1
            continue
        if (
            character == "#"
            and not single_quoted
            and not double_quoted
            and (cursor == 0 or runner_source[cursor - 1].isspace())
        ):
            comment = True
            cursor += 1
            continue
        if character == "\\" and not single_quoted:
            newline_length = 0
            if cursor + 1 < len(runner_source) and runner_source[cursor + 1] == "\n":
                newline_length = 1
            elif runner_source[cursor + 1 : cursor + 3] == "\r\n":
                newline_length = 2
            if newline_length:
                previous = runner_source[cursor - 1] if cursor else "\n"
                if not previous.isspace():
                    line = runner_source.count("\n", 0, cursor) + 1
                    raise QualityError(
                        f"token-splitting shell continuation at runner line {line}",
                        code="runner.grammar.token_split",
                    )
                cursor += 1 + newline_length
                continue
            cursor += 2
            continue
        cursor += 1


def join_active_continuations(runner_source: str) -> str:
    result: list[str] = []
    cursor = 0
    single_quoted = False
    double_quoted = False
    comment = False
    while cursor < len(runner_source):
        character = runner_source[cursor]
        if comment:
            if character == "\\" and runner_source[cursor + 1 : cursor + 2] == "\n":
                cursor += 2
                continue
            if character == "\n":
                comment = False
            result.append(character)
            cursor += 1
            continue
        if character == "'" and not double_quoted:
            single_quoted = not single_quoted
            result.append(character)
            cursor += 1
            continue
        if character == '"' and not single_quoted:
            double_quoted = not double_quoted
            result.append(character)
            cursor += 1
            continue
        if (
            character == "#"
            and not single_quoted
            and not double_quoted
            and (cursor == 0 or runner_source[cursor - 1].isspace())
        ):
            comment = True
            result.append(character)
            cursor += 1
            continue
        if character == "\\" and not single_quoted:
            if runner_source[cursor + 1 : cursor + 2] == "\n":
                cursor += 2
                continue
            if runner_source[cursor + 1 : cursor + 3] == "\r\n":
                cursor += 3
                continue
        result.append(character)
        cursor += 1
    return "".join(result)


def reject_continuation_split_functions(runner_source: str) -> None:
    logical_source = join_active_continuations(runner_source)
    for line_number, original_line in enumerate(logical_source.splitlines(), start=1):
        line = shell_line_code(original_line)
        projection = shell_dequoted_projection(line)
        function_header = re.fullmatch(
            r"[ \t]*([A-Za-z_][A-Za-z0-9_]*)[ \t]*\([ \t]*\)[ \t]*\{[ \t]*",
            projection,
        )
        canonical = (
            function_header is not None
            and function_header.group(1) in RUNNER_FUNCTION_NAMES
            and line == f"{function_header.group(1)}() {{"
        )
        if re.search(r"\([ \t]*\)", projection) and not canonical:
            raise QualityError(
                "continuation-split shell function is outside the audited runner "
                f"grammar at logical line {line_number}",
                code="runner.grammar.continuation_split_function",
            )


def reject_unparsed_runner_forms(runner_source: str) -> None:
    for line_number, original_line in enumerate(runner_source.splitlines(), start=1):
        line = shell_line_code(original_line)
        if not line.strip():
            continue
        unquoted_projection = shell_unquoted_projection(line)
        if "<<" in unquoted_projection:
            raise QualityError(
                f"shell here-document/string is outside the audited runner grammar "
                f"at line {line_number}",
                code="runner.grammar.heredoc",
            )
        if re.search(
            r"^[ \t]*(?:export[ \t]+)?['\"](?:expected_)?negative_[A-Za-z0-9_]+[ \t]*=",
            line,
        ):
            raise QualityError(
                f"quoted negative assignment is outside the audited runner grammar "
                f"at line {line_number}",
                code="runner.grammar.quoted_negative_assignment",
            )
        if re.search(
            r"^[ \t]*(['\"])(?:check_negative|check_digest)\1(?:[ \t]|$)", line
        ):
            raise QualityError(
                f"quoted negative command is outside the audited runner grammar "
                f"at line {line_number}",
                code="runner.grammar.quoted_negative_command",
            )
        literal_assignment = re.fullmatch(
            r"[ \t]*([A-Za-z_][A-Za-z0-9_]*)='[^']*'[ \t]*", line
        )
        if literal_assignment is not None and not re.fullmatch(
            r"(?:expected_)?negative_[A-Za-z0-9_]+", literal_assignment.group(1)
        ):
            continue
        projection = shell_dequoted_projection(line)
        if re.search(
            r"(?:^|[ \t;|&()])(?:alias|unalias|eval)(?=[ \t;]|$)",
            projection,
        ):
            raise QualityError(
                f"shell aliasing or eval is outside the audited runner grammar "
                f"at line {line_number}",
                code="runner.grammar.alias_or_eval",
            )
        function_header = re.fullmatch(
            r"[ \t]*([A-Za-z_][A-Za-z0-9_]*)[ \t]*\([ \t]*\)[ \t]*\{[ \t]*",
            projection,
        )
        canonical_function_header = (
            function_header is not None
            and function_header.group(1) in RUNNER_FUNCTION_NAMES
            and line == f"{function_header.group(1)}() {{"
        )
        if re.search(r"\([ \t]*\)", projection) and not canonical_function_header:
            raise QualityError(
                f"unparsed shell function is outside the audited runner grammar "
                f"at line {line_number}",
                code="runner.grammar.unparsed_function",
            )
        source_assignment = SOURCE_ASSIGNMENT.fullmatch(line)
        pin_assignment = PIN_ASSIGNMENT.fullmatch(line)
        digest_check = DIGEST_CHECK.fullmatch(line)
        negative_call = NEGATIVE_CALL.fullmatch(line)
        if re.search(r"\bnegative_(?!quality_)[A-Za-z0-9_]*[ \t]*=", projection):
            if source_assignment is None:
                raise QualityError(
                    f"unparsed negative source assignment at runner line {line_number}",
                    code="runner.grammar.unparsed_source_assignment",
                )
        if re.search(
            r"\bexpected_negative_(?!quality_)[A-Za-z0-9_]*[ \t]*=", projection
        ):
            if pin_assignment is None:
                raise QualityError(
                    f"unparsed negative pin assignment at runner line {line_number}",
                    code="runner.grammar.unparsed_pin_assignment",
                )
        if re.search(r"\bcheck_digest\b", projection) and "negative_" in line:
            if digest_check is None:
                raise QualityError(
                    f"unparsed negative digest check at runner line {line_number}",
                    code="runner.grammar.unparsed_digest_check",
                )
        if re.search(r"\bcheck_negative\b", projection):
            if re.fullmatch(
                r"[ \t]*check_negative[ \t]*\([ \t]*\)[ \t]*\{[ \t]*", line
            ):
                continue
            if negative_call is None:
                raise QualityError(
                    f"unparsed negative verification call at runner line {line_number}",
                    code="runner.grammar.unparsed_verification_call",
                )


def runner_function_source(runner_source: str, name: str) -> str:
    lines = runner_source.splitlines(keepends=True)
    header = re.compile(rf"[ \t]*{re.escape(name)}[ \t]*\([ \t]*\)[ \t]*\{{[ \t]*")
    starts = [
        index
        for index, line in enumerate(lines)
        if header.fullmatch(shell_line_code(line.rstrip("\r\n")))
    ]
    if len(starts) != 1:
        raise QualityError(
            f"runner must contain exactly one {name} definition",
            code=f"runner.function.{name}.count",
        )
    for end in range(starts[0] + 1, len(lines)):
        if shell_line_code(lines[end].rstrip("\r\n")).strip() == "}":
            return "".join(lines[starts[0] : end + 1])
    raise QualityError(
        f"unterminated {name} definition",
        code=f"runner.function.{name}.unterminated",
    )


def check_negative_function_source(runner_source: str) -> str:
    return runner_function_source(runner_source, "check_negative")


def exact_block_start(lines: list[str], block: tuple[str, ...], label: str) -> int:
    starts = [
        index
        for index in range(len(lines) - len(block) + 1)
        if tuple(lines[index : index + len(block)]) == block
    ]
    if len(starts) != 1:
        raise QualityError(
            f"runner must contain exactly one canonical {label} block",
            code=f"runner.block.{label.replace('-', '_').replace(' ', '_')}.count",
        )
    return starts[0]


def preseal_authority_assignments(
    runner_source: str,
) -> tuple[str, set[str], set[int], int]:
    lines = [shell_line_code(line).rstrip() for line in runner_source.splitlines()]
    seal_calls = [index for index, line in enumerate(lines) if line == "seal_authority"]
    if len(seal_calls) != 1:
        raise QualityError(
            "runner must call seal_authority exactly once",
            code="runner.preseal.call_count",
        )
    seal_call = seal_calls[0]
    assignments: list[str] = []
    names: list[str] = []
    indices: set[int] = set()
    assignment = re.compile(r"^[ \t]*([A-Za-z_][A-Za-z0-9_]*)=")
    for index, line in enumerate(lines[:seal_call]):
        match = assignment.match(line)
        if match is None:
            continue
        name = match.group(1)
        if not (
            name.startswith("expected_")
            or name.startswith("negative_")
            or name.endswith("_proof")
            or name == "verus_bin"
        ):
            continue
        assignments.append(line)
        names.append(name)
        indices.add(index)
    require_unique("pre-seal authority assignment", names)
    return "\n".join(assignments) + "\n", set(names), indices, seal_call


def audit_preseal_authority(
    runner_source: str, expected_count: int, expected_sha256: str
) -> None:
    source, names, assignment_lines, seal_call = preseal_authority_assignments(
        runner_source
    )
    if len(names) != expected_count:
        raise QualityError(
            f"pre-seal authority assignment count is {len(names)}, expected {expected_count}",
            code="runner.preseal.assignment_count",
        )
    if hashlib.sha256(source.encode()).hexdigest() != expected_sha256:
        raise QualityError(
            "pre-seal authority assignments do not match their audit",
            code="runner.preseal.assignment_digest",
        )

    lines = [shell_line_code(line).rstrip() for line in runner_source.splitlines()]
    seal_source = runner_function_source(runner_source, "seal_authority").splitlines()
    seal_starts = [
        index
        for index in range(len(lines) - len(seal_source) + 1)
        if lines[index : index + len(seal_source)] == seal_source
    ]
    if len(seal_starts) != 1 or seal_call != seal_starts[0] + len(seal_source):
        raise QualityError(
            "seal_authority must be called immediately after its definition",
            code="runner.preseal.call_adjacency",
        )
    source_checks = [
        index for index, line in enumerate(lines) if line == "check_sources"
    ]
    if len(source_checks) != 3 or seal_call >= source_checks[0]:
        raise QualityError(
            "seal_authority must run before the first source check",
            code="runner.preseal.before_source_check",
        )

    readonly_tool_line = RUNNER_TOOL_BINDING_BLOCK[-1]
    for index, line in enumerate(lines):
        projection = shell_dequoted_projection(line)
        assignment = re.search(
            r"(?<![A-Za-z0-9_])([A-Za-z_][A-Za-z0-9_]*)[ \t]*(?:\+?=)",
            projection,
        )
        parameter_assignment = re.search(
            r"\$\{([A-Za-z_][A-Za-z0-9_]*)(?::?=)", projection
        )
        if (
            assignment is not None
            and assignment.group(1) in names
            and index not in assignment_lines
        ) or (
            parameter_assignment is not None and parameter_assignment.group(1) in names
        ):
            raise QualityError(
                f"unparsed sealed-authority assignment at runner line {index + 1}",
                code="runner.preseal.unparsed_assignment",
            )
        mutator = re.search(
            r"(?:^|[ \t;|&()])"
            r"(?:read|unset|export|readonly|declare|typeset|local|for|printf)\b",
            projection,
        )
        referenced_names = set(re.findall(IDENTIFIER, projection)) & names
        if (
            mutator is not None
            and referenced_names
            and line != readonly_tool_line
            and not (seal_starts[0] <= index < seal_call)
        ):
            raise QualityError(
                f"unparsed sealed-authority mutator at runner line {index + 1}",
                code="runner.preseal.unparsed_mutator",
            )


def audit_runner_root_bindings(runner_source: str) -> None:
    lines = [shell_line_code(line).rstrip() for line in runner_source.splitlines()]
    environment_start = exact_block_start(
        lines, RUNNER_ENVIRONMENT_BLOCK, "environment"
    )
    root_start = exact_block_start(lines, RUNNER_ROOT_BINDING_BLOCK, "root binding")
    tool_start = exact_block_start(lines, RUNNER_TOOL_BINDING_BLOCK, "tool binding")
    invocation_start = exact_block_start(
        lines,
        RUNNER_NEGATIVE_QUALITY_INVOCATION_BLOCK,
        "negative-quality invocation",
    )
    if not environment_start < root_start < tool_start < invocation_start:
        raise QualityError(
            "runner environment, root, tool, and invocation blocks are out of order",
            code="runner.root.block_order",
        )

    allowed_tool_lines = set(
        range(tool_start, tool_start + len(RUNNER_TOOL_BINDING_BLOCK))
    )
    allowed_tool_lines.update(
        range(
            invocation_start,
            invocation_start + len(RUNNER_NEGATIVE_QUALITY_INVOCATION_BLOCK),
        )
    )
    quality_audits = [
        index
        for index, line in enumerate(lines)
        if line == RUNNER_NEGATIVE_QUALITY_AUDIT
    ]
    if len(quality_audits) != 2:
        raise QualityError(
            "runner must contain exactly two canonical negative-quality audits",
            code="runner.quality_audit.count",
        )
    allowed_tool_lines.update(quality_audits)
    for digest_line in RUNNER_TOOL_DIGEST_LINES:
        matches = [index for index, line in enumerate(lines) if line == digest_line]
        if len(matches) != 1:
            raise QualityError(
                "runner must contain exactly one canonical tool digest check",
                code="runner.tool_digest.count",
            )
        allowed_tool_lines.add(matches[0])

    source_invocations = [
        index
        for index, line in enumerate(lines)
        if line == RUNNER_SOURCE_CHECKER_INVOCATION
    ]
    if len(source_invocations) != 1:
        raise QualityError(
            "runner must contain exactly one canonical source-checker invocation",
            code="runner.source_checker.canonical_invocation_count",
        )
    allowed_tool_lines.add(source_invocations[0])

    closure_invocations = [
        index
        for index, line in enumerate(lines)
        if line == '"$closure_checker" "$verus_root" "$closure_manifest"'
    ]
    if len(closure_invocations) != 2:
        raise QualityError(
            "runner must contain exactly two canonical closure-checker invocations",
            code="runner.closure_checker.invocation_count",
        )
    allowed_tool_lines.update(closure_invocations)

    tool_name = re.compile(rf"(?<![A-Za-z0-9_])(?:{'|'.join(RUNNER_TOOL_NAMES)})\b")
    for index, line in enumerate(lines):
        if index in allowed_tool_lines:
            continue
        if tool_name.search(shell_dequoted_projection(line)):
            raise QualityError(
                f"unparsed authenticated-tool wiring at runner line {index + 1}",
                code="runner.tool_wiring.unparsed",
            )

    root_assignment = re.compile(
        r"(?<![A-Za-z0-9_])(?:script_dir|repo_root|PATH|IFS)[ \t]*(?:\+?=)"
    )
    allowed_root_assignments = {environment_start, root_start, root_start + 1}
    for index, line in enumerate(lines):
        if (
            index in allowed_root_assignments
            or index in allowed_tool_lines
            or line == '        "PATH=$runner_path" \\'
        ):
            continue
        if root_assignment.search(shell_dequoted_projection(line)):
            raise QualityError(
                f"unparsed runner-root assignment at runner line {index + 1}",
                code="runner.root.unparsed_assignment",
            )

    late_assignment_lines: set[int] = set()
    for assignment_line in RUNNER_LATE_ASSIGNMENT_LINES:
        matches = [index for index, line in enumerate(lines) if line == assignment_line]
        if len(matches) != 1:
            raise QualityError(
                "runner must contain exactly one canonical late-authority assignment",
                code="runner.late_authority.assignment_count",
            )
        late_assignment_lines.add(matches[0])
    late_readonly_lines: list[int] = []
    for readonly_line in RUNNER_LATE_READONLY_LINES:
        matches = [index for index, line in enumerate(lines) if line == readonly_line]
        if len(matches) != 1:
            raise QualityError(
                "runner must contain exactly one canonical late-authority seal",
                code="runner.late_authority.seal_count",
            )
        late_readonly_lines.append(matches[0])
    if late_readonly_lines != sorted(late_readonly_lines):
        raise QualityError(
            "runner late-authority seals are out of order",
            code="runner.late_authority.seal_order",
        )
    late_block_starts = [
        exact_block_start(lines, block, "late-authority binding")
        for block in RUNNER_LATE_BINDING_BLOCKS
    ]
    if late_block_starts != sorted(late_block_starts):
        raise QualityError(
            "runner late-authority binding blocks are out of order",
            code="runner.late_authority.block_order",
        )
    late_assignment = re.compile(
        rf"(?<![A-Za-z0-9_])(?:{'|'.join(RUNNER_LATE_NAMES)})[ \t]*(?:\+?=)"
    )
    for index, line in enumerate(lines):
        if index in late_assignment_lines or line == '        "PATH=$runner_path" \\':
            continue
        if late_assignment.search(shell_dequoted_projection(line)):
            raise QualityError(
                f"unparsed late-authority assignment at runner line {index + 1}",
                code="runner.late_authority.unparsed_assignment",
            )

    seal_calls = [index for index, line in enumerate(lines) if line == "seal_authority"]
    if len(seal_calls) != 1 or seal_calls[0] >= source_invocations[0]:
        raise QualityError(
            "runner must call seal_authority exactly once before checking",
            code="runner.preseal.call_before_check",
        )


def source_checker_arguments(runner_source: str) -> list[str]:
    lines = runner_source.splitlines()
    start_pattern = re.compile(
        r"^[ \t]*/usr/bin/env -i PATH=/usr/bin:/bin /usr/bin/python3 -I "
        r'"\$source_checker"[ \t]+\\[ \t]*$'
    )
    starts = [
        index
        for index, line in enumerate(lines)
        if start_pattern.fullmatch(shell_line_code(line))
    ]
    for index, line in enumerate(lines):
        code = shell_line_code(line)
        active = shell_active_projection(code)
        source_checker_expansion = re.search(
            r"\$(?:source_checker\b|\{source_checker\})", active
        )
        source_checker_digest = re.fullmatch(
            r'[ \t]*check_digest[ \t]+"\$expected_source_checker"[ \t]+'
            r'"\$source_checker"[ \t]*',
            code,
        )
        if (
            source_checker_expansion
            and not start_pattern.fullmatch(code)
            and source_checker_digest is None
        ):
            raise QualityError(
                f"unparsed source-checker invocation at runner line {index + 1}",
                code="runner.source_checker.unparsed_invocation",
            )
    if len(starts) != 1:
        raise QualityError(
            "runner must contain exactly one source-checker invocation",
            code="runner.source_checker.invocation_count",
        )
    arguments: list[str] = []
    cursor = starts[0] + 1
    while cursor < len(lines):
        line = shell_line_code(lines[cursor])
        match = re.fullmatch(
            r'[ \t]*"\$([A-Za-z_][A-Za-z0-9_]*)"(?:[ \t]+\\)?[ \t]*', line
        )
        if match is None:
            raise QualityError(
                f"unparsed source-checker argument at runner line {cursor + 1}",
                code="runner.source_checker.unparsed_argument",
            )
        variable = match.group(1)
        if variable.startswith("negative_"):
            arguments.append(variable)
        continued = re.search(r"\\[ \t]*$", line) is not None
        if not continued:
            break
        cursor += 1
    return arguments


def read_runner(path: Path) -> str:
    if path.is_symlink() or not path.is_file():
        raise QualityError(
            f"verification runner is unavailable: {path}",
            code="runner.unavailable",
        )
    try:
        return path.read_bytes().decode("utf-8")
    except (OSError, UnicodeError) as error:
        raise QualityError(str(error)) from error


def audit_negative_rosters(
    sources: list[Path],
    directory: Path,
    runner: Path,
    runner_source: str,
    expected_count: int,
) -> list[Path]:
    normalized_runner = normalized_runner_source(runner_source)
    assignments = SOURCE_ASSIGNMENT.findall(normalized_runner)
    variables = [variable for variable, _ in assignments]
    filenames = [filename for _, filename in assignments]
    require_unique("negative source assignment", variables)
    require_unique("negative source filename assignment", filenames)
    disk_names = [path.name for path in sources]
    if len(sources) != expected_count:
        raise QualityError(
            f"negative source count is {len(sources)}, expected {expected_count}",
            code="roster.source_count",
        )
    if set(filenames) != set(disk_names):
        missing = sorted(set(disk_names) - set(filenames))
        extra = sorted(set(filenames) - set(disk_names))
        detail = (missing or extra)[0]
        raise QualityError(
            f"negative source assignment inventory mismatch: {detail}",
            code="roster.source_assignment_mismatch",
        )

    variable_set = set(variables)
    expected_variables = {f"expected_{variable}" for variable in variables}
    pin_assignments = PIN_ASSIGNMENT.findall(normalized_runner)
    pin_variables = [variable for variable, _ in pin_assignments]
    pin_names = [
        pin for variable, pin in pin_assignments if variable in expected_variables
    ]
    relevant_pin_variables = [
        variable for variable in pin_variables if variable in expected_variables
    ]
    unexpected_pin_variables = (
        set(pin_variables) - expected_variables - AUXILIARY_NEGATIVE_PINS
    )
    if unexpected_pin_variables:
        raise QualityError(
            f"unexpected negative pin assignment: {sorted(unexpected_pin_variables)[0]}",
            code="roster.unexpected_pin_assignment",
        )
    require_unique("negative pin assignment", relevant_pin_variables)
    require_unique("negative pin file", pin_names)
    if set(relevant_pin_variables) != expected_variables:
        detail = sorted(
            expected_variables.symmetric_difference(relevant_pin_variables)
        )[0]
        raise QualityError(
            f"negative pin assignment inventory mismatch: {detail}",
            code="roster.pin_assignment_mismatch",
        )
    pin_by_variable = {
        variable: pin
        for variable, pin in pin_assignments
        if variable in expected_variables
    }

    digest_checks = [
        pair
        for pair in DIGEST_CHECK.findall(normalized_runner)
        if pair[1] in variable_set
    ]
    require_unique("negative digest check", [variable for _, variable in digest_checks])
    expected_digest_checks = {
        (f"expected_{variable}", variable) for variable in variables
    }
    if set(digest_checks) != expected_digest_checks:
        difference = sorted(expected_digest_checks.symmetric_difference(digest_checks))[
            0
        ]
        raise QualityError(
            f"negative digest-check inventory mismatch: {difference}",
            code="roster.digest_check_mismatch",
        )

    checked_arguments = source_checker_arguments(runner_source)
    require_unique("negative source-checker argument", checked_arguments)
    if set(checked_arguments) != variable_set:
        detail = sorted(variable_set.symmetric_difference(checked_arguments))[0]
        raise QualityError(
            f"negative source-checker inventory mismatch: {detail}",
            code="roster.source_checker_mismatch",
        )

    calls = NEGATIVE_CALL.findall(normalized_runner)
    called_variables = [variable for variable, _, _ in calls]
    labels = [label for _, _, label in calls]
    require_unique("negative verification call", called_variables)
    require_unique("negative verification label", labels)
    if set(called_variables) != variable_set:
        detail = sorted(variable_set.symmetric_difference(called_variables))[0]
        raise QualityError(
            f"negative verification-call inventory mismatch: {detail}",
            code="roster.verification_call_mismatch",
        )

    counts = [int(value) for value in TRANSCRIPT_COUNT.findall(normalized_runner)]
    if counts != [expected_count]:
        raise QualityError(
            f"runner must contain one expected-negative count equal to {expected_count}",
            code="roster.transcript_count",
        )

    assignment_by_variable = dict(assignments)
    pin_root = runner.parent / "pins"
    for variable in variables:
        expected_variable = f"expected_{variable}"
        pin_path = pin_root / pin_by_variable[expected_variable]
        if pin_path.is_symlink() or not pin_path.is_file():
            raise QualityError(
                f"negative source pin is unavailable: {pin_path}",
                code="roster.pin_unavailable",
            )
        try:
            pin_lines = pin_path.read_text(encoding="utf-8").splitlines()
        except (OSError, UnicodeError) as error:
            raise QualityError(str(error)) from error
        if len(pin_lines) != 1 or HEX_SHA256.fullmatch(pin_lines[0]) is None:
            raise QualityError(
                f"invalid negative source pin: {pin_path}",
                code="roster.pin_invalid",
            )
        source_path = directory / assignment_by_variable[variable]
        digest = hashlib.sha256(source_path.read_bytes()).hexdigest()
        if digest != pin_lines[0]:
            raise QualityError(
                f"negative source digest mismatch: {source_path}",
                code="roster.source_digest_mismatch",
            )
    return sources


def audit_inventory(
    directory: Path,
    runner: Path,
    expected_count: int = EXPECTED_RUNTIME_NEGATIVE_COUNT,
    expected_check_negative_sha256: str = EXPECTED_CHECK_NEGATIVE_FUNCTION_SHA256,
    expected_runner_function_sha256: dict[str, str] = EXPECTED_RUNNER_FUNCTION_SHA256,
    expected_preseal_authority_count: int = EXPECTED_PRESEAL_AUTHORITY_ASSIGNMENT_COUNT,
    expected_preseal_authority_sha256: str = EXPECTED_PRESEAL_AUTHORITY_ASSIGNMENTS_SHA256,
    expected_runner_sha256: str = EXPECTED_RUNNER_SHA256,
) -> list[Path]:
    sources = runtime_negative_sources(directory)
    runner_source = read_runner(runner)
    if hashlib.sha256(runner_source.encode()).hexdigest() != expected_runner_sha256:
        raise QualityError(
            "verification runner does not match its complete-source audit",
            code="runner.raw_digest",
        )
    reject_token_split_continuations(runner_source)
    reject_continuation_split_functions(runner_source)
    reject_unparsed_runner_forms(runner_source)
    audit_runner_root_bindings(runner_source)
    audit_preseal_authority(
        runner_source,
        expected_preseal_authority_count,
        expected_preseal_authority_sha256,
    )
    function_digest = hashlib.sha256(
        check_negative_function_source(runner_source).encode()
    ).hexdigest()
    if function_digest != expected_check_negative_sha256:
        raise QualityError(
            "check_negative definition does not match its audited body",
            code="runner.function.check_negative.digest",
        )
    for name, expected_digest in expected_runner_function_sha256.items():
        function_digest = hashlib.sha256(
            runner_function_source(runner_source, name).encode()
        ).hexdigest()
        if function_digest != expected_digest:
            raise QualityError(
                f"{name} definition does not match its audited body",
                code=f"runner.function.{name}.digest",
            )
    return audit_negative_rosters(
        sources, directory, runner, runner_source, expected_count
    )


def expect_inventory_rejection(
    label: str,
    expected_error_code: str,
    build_case,
    *,
    reauth_preseal: bool = False,
    reauth_functions: tuple[str, ...] = (),
) -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-negative-quality-") as temp:
        root = Path(temp)
        directory, runner = build_inventory_fixture(root)
        expected_function = hashlib.sha256(
            check_negative_function_source(read_runner(runner)).encode()
        ).hexdigest()
        expected_runner_functions = {
            name: hashlib.sha256(
                runner_function_source(read_runner(runner), name).encode()
            ).hexdigest()
            for name in EXPECTED_RUNNER_FUNCTION_SHA256
        }
        expected_runner_sha256 = hashlib.sha256(
            read_runner(runner).encode()
        ).hexdigest()
        authority_source, authority_names, _, _ = preseal_authority_assignments(
            read_runner(runner)
        )
        build_case(directory, runner)
        if label != "complete runner raw-byte substitution":
            expected_runner_sha256 = hashlib.sha256(
                read_runner(runner).encode()
            ).hexdigest()
        if reauth_preseal:
            authority_source, authority_names, _, _ = preseal_authority_assignments(
                read_runner(runner)
            )
        for name in reauth_functions:
            expected_runner_functions[name] = hashlib.sha256(
                runner_function_source(read_runner(runner), name).encode()
            ).hexdigest()
        try:
            audit_inventory(
                directory,
                runner,
                expected_count=2,
                expected_check_negative_sha256=expected_function,
                expected_runner_function_sha256=expected_runner_functions,
                expected_preseal_authority_count=len(authority_names),
                expected_preseal_authority_sha256=hashlib.sha256(
                    authority_source.encode()
                ).hexdigest(),
                expected_runner_sha256=expected_runner_sha256,
            )
        except QualityError as error:
            if error.code == expected_error_code:
                return
            raise QualityError(
                f"inventory self-test {label} reached {error.code}, "
                f"expected {expected_error_code}: {error}",
                code="self_test.wrong_rejection",
            ) from error
        raise QualityError(
            f"inventory self-test unexpectedly accepted {label}",
            code="self_test.unexpected_acceptance",
        )


def expect_roster_rejection(label: str, expected_error_code: str, build_case) -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-negative-quality-") as temp:
        directory, runner = build_inventory_fixture(Path(temp))
        build_case(directory, runner)
        try:
            audit_negative_rosters(
                runtime_negative_sources(directory),
                directory,
                runner,
                read_runner(runner),
                2,
            )
        except QualityError as error:
            if error.code == expected_error_code:
                return
            raise QualityError(
                f"roster self-test {label} reached {error.code}, "
                f"expected {expected_error_code}: {error}",
                code="self_test.wrong_rejection",
            ) from error
        raise QualityError(
            f"roster self-test unexpectedly accepted {label}",
            code="self_test.unexpected_acceptance",
        )


def build_inventory_fixture(root: Path) -> tuple[Path, Path]:
    directory = root / "negative"
    pin_root = root / "pins"
    directory.mkdir()
    pin_root.mkdir()
    sources = {"alpha": "proof alpha\n", "beta": "proof beta\n"}
    for name, source in sources.items():
        (directory / f"{name}.rs").write_text(source, encoding="utf-8")
        digest = hashlib.sha256(source.encode()).hexdigest()
        (pin_root / f"NEGATIVE_{name.upper()}_SHA256").write_text(
            digest + "\n", encoding="utf-8"
        )
    runner = root / "verify-verus.sh"
    runner.write_text(
        "\n".join(
            [
                *RUNNER_ENVIRONMENT_BLOCK,
                *RUNNER_ROOT_BINDING_BLOCK,
                'negative_alpha="$script_dir/negative/alpha.rs"',
                'negative_beta="$script_dir/negative/beta.rs"',
                *RUNNER_TOOL_BINDING_BLOCK,
                'expected_negative_alpha=$(read_pin "$pin_dir/NEGATIVE_ALPHA_SHA256")',
                'expected_negative_beta=$(read_pin "$pin_dir/NEGATIVE_BETA_SHA256")',
                'alpha_proof="$script_dir/alpha.rs"',
                'expected_alpha=$(read_pin "$pin_dir/ALPHA_SHA256")',
                "verus_bin=${VERUS:-verus}",
                "read_pin() {",
                "    : read-pin",
                "}",
                "check_digest() {",
                "    : check-digest",
                "}",
                "check_sources() {",
                '    check_digest "$expected_negative_alpha" "$negative_alpha"',
                '    check_digest "$expected_negative_beta" "$negative_beta"',
                *RUNNER_TOOL_DIGEST_LINES,
                "}",
                "run_verus() {",
                "    : run-verus",
                "}",
                "check_positive() {",
                "    : check-positive",
                "}",
                "check_negative() {",
                "    :",
                "}",
                "seal_authority() {",
                "    : seal-authority",
                "}",
                "seal_authority",
                *RUNNER_LATE_ASSIGNMENT_LINES[:3],
                *(line for block in RUNNER_LATE_BINDING_BLOCKS for line in block),
                "check_sources",
                "check_sources",
                "check_sources",
                RUNNER_SOURCE_CHECKER_INVOCATION,
                '    "$negative_alpha" \\',
                '    "$negative_beta"',
                *RUNNER_NEGATIVE_QUALITY_INVOCATION_BLOCK,
                RUNNER_NEGATIVE_QUALITY_AUDIT,
                '"$closure_checker" "$verus_root" "$closure_manifest"',
                'check_negative "$negative_alpha" alpha_postcondition alpha-label',
                'check_negative "$negative_beta" beta_postcondition beta-label',
                "transcript='expected_negative_files=2'",
                '# negative_shadow="$script_dir/negative/shadow.rs"',
                "decoy='negative_shadow=\"$script_dir/negative/shadow.rs\" '",
                "pin_decoy='expected_negative_shadow=$(read_pin '",
                "digest_decoy='check_digest $expected_negative_shadow $negative_shadow'",
                "call_decoy='check_negative $negative_shadow shadow_postcondition shadow-label'",
                "count_decoy='expected_negative_files=999'",
            ]
        )
        + "\n",
        encoding="utf-8",
    )
    return directory, runner


def replace_runner(runner: Path, old: str, new: str) -> None:
    source = runner.read_text(encoding="utf-8")
    if source.count(old) != 1:
        raise QualityError(
            f"inventory self-test fixture expected one occurrence of {old!r}"
        )
    runner.write_text(source.replace(old, new), encoding="utf-8")


def remove_first_runner(runner: Path, text: str) -> None:
    source = runner.read_text(encoding="utf-8")
    position = source.find(text)
    if position < 0:
        raise QualityError(
            f"inventory self-test fixture expected an occurrence of {text!r}"
        )
    runner.write_text(
        source[:position] + source[position + len(text) :], encoding="utf-8"
    )


def inventory_self_test() -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-negative-quality-") as temp:
        directory, runner = build_inventory_fixture(Path(temp))
        expected_function = hashlib.sha256(
            check_negative_function_source(read_runner(runner)).encode()
        ).hexdigest()
        expected_runner_functions = {
            name: hashlib.sha256(
                runner_function_source(read_runner(runner), name).encode()
            ).hexdigest()
            for name in EXPECTED_RUNNER_FUNCTION_SHA256
        }
        expected_runner_sha256 = hashlib.sha256(
            read_runner(runner).encode()
        ).hexdigest()
        authority_source, authority_names, _, _ = preseal_authority_assignments(
            read_runner(runner)
        )
        audit_inventory(
            directory,
            runner,
            expected_count=2,
            expected_check_negative_sha256=expected_function,
            expected_runner_function_sha256=expected_runner_functions,
            expected_preseal_authority_count=len(authority_names),
            expected_preseal_authority_sha256=hashlib.sha256(
                authority_source.encode()
            ).hexdigest(),
            expected_runner_sha256=expected_runner_sha256,
        )

    cases = [
        (
            "complete runner raw-byte substitution",
            lambda _d, r: r.write_bytes(
                r.read_bytes().replace(
                    b'# negative_shadow="$script_dir/negative/shadow.rs"\n',
                    b'# negative_shadow="$script_dir/negative/shadow.rs"\r\n',
                    1,
                )
            ),
        ),
        ("extra regular source", lambda d, _r: (d / "extra.rs").write_text("x\n")),
        ("symlink source", lambda d, _r: os.symlink(d / "alpha.rs", d / "link.rs")),
        ("non-Rust source", lambda d, _r: (d / "note.txt").write_text("x\n")),
        (
            "missing root readonly",
            lambda _d, r: replace_runner(r, "\\readonly script_dir repo_root\n", ""),
        ),
        (
            "moved root readonly",
            lambda _d, r: replace_runner(
                r,
                "\\readonly script_dir repo_root\n",
                "# moved below the canonical block\n\\readonly script_dir repo_root\n",
            ),
        ),
        (
            "mutated script root",
            lambda _d, r: replace_runner(
                r,
                RUNNER_ROOT_BINDING_BLOCK[0] + "\n",
                "script_dir=/tmp/substituted\n",
            ),
        ),
        (
            "quoted script-root override",
            lambda _d, r: r.write_text(
                r.read_text() + "'script_dir'=/tmp/substituted\n"
            ),
        ),
        (
            "search-path override",
            lambda _d, r: r.write_text(r.read_text() + "PATH=/tmp/substituted\n"),
        ),
        (
            "inherited search path",
            lambda _d, r: replace_runner(
                r, "PATH=/usr/bin:/bin\n", "PATH=${PATH:-/usr/bin:/bin}\n"
            ),
        ),
        (
            "missing tool readonly",
            lambda _d, r: replace_runner(r, RUNNER_TOOL_BINDING_BLOCK[-1] + "\n", ""),
        ),
        (
            "closure-checker override after authentication",
            lambda _d, r: r.write_text(r.read_text() + "closure_checker=/bin/true\n"),
        ),
        (
            "missing closure-checker invocation",
            lambda _d, r: remove_first_runner(
                r, '"$closure_checker" "$verus_root" "$closure_manifest"\n'
            ),
        ),
        (
            "mutated source-checker binding",
            lambda _d, r: replace_runner(
                r,
                RUNNER_TOOL_BINDING_BLOCK[2] + "\n",
                "source_checker=/bin/true\n",
            ),
        ),
        (
            "source-checker override after authentication",
            lambda _d, r: r.write_text(r.read_text() + "source_checker=/bin/true\n"),
        ),
        (
            "fragment-quoted negative-quality-checker override",
            lambda _d, r: r.write_text(
                r.read_text() + 'negative_quality_"checker"=/bin/true\n'
            ),
        ),
        (
            "missing negative-quality invocation",
            lambda _d, r: remove_first_runner(
                r, RUNNER_NEGATIVE_QUALITY_INVOCATION_BLOCK[-1] + "\n"
            ),
        ),
        (
            "substituted negative-quality self-test",
            lambda _d, r: replace_runner(
                r,
                RUNNER_NEGATIVE_QUALITY_INVOCATION_BLOCK[0] + "\n",
                '"/bin/true" --self-test \\\n',
            ),
        ),
        (
            "readonly alias",
            lambda _d, r: r.write_text(r.read_text() + "alias readonly=:\n"),
        ),
        (
            "digest-check alias",
            lambda _d, r: r.write_text(r.read_text() + "alias check_digest=:\n"),
        ),
        (
            "result-check alias",
            lambda _d, r: r.write_text(r.read_text() + "alias grep=:\n"),
        ),
        (
            "result-check function",
            lambda _d, r: r.write_text(r.read_text() + "grep() {\n    :\n}\n"),
        ),
        (
            "split-line result-check function",
            lambda _d, r: r.write_text(r.read_text() + "grep()\n{\n    :\n}\n"),
        ),
        (
            "split-line authenticated function replacement",
            lambda _d, r: r.write_text(r.read_text() + "check_digest()\n{\n    :\n}\n"),
        ),
        (
            "continuation-split result-check function",
            lambda _d, r: r.write_text(
                r.read_text() + "grep \\\n( \\\n) {\n    return 0\n}\n"
            ),
        ),
        (
            "shell eval",
            lambda _d, r: r.write_text(r.read_text() + "eval 'check_digest=:'\n"),
        ),
        (
            "substituted pin-reader body",
            lambda _d, r: replace_runner(r, "    : read-pin\n", "    return 0\n"),
        ),
        (
            "substituted authority-seal body",
            lambda _d, r: replace_runner(r, "    : seal-authority\n", "    return 0\n"),
        ),
        (
            "missing authority-seal call",
            lambda _d, r: replace_runner(r, "seal_authority\n", ""),
        ),
        (
            "positive authority override before seal",
            lambda _d, r: replace_runner(
                r,
                "seal_authority\n",
                "alpha_proof=/tmp/forged.rs\n"
                "expected_alpha=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n"
                "seal_authority\n",
            ),
        ),
        (
            "quoted authority override before seal",
            lambda _d, r: replace_runner(
                r,
                "seal_authority() {\n",
                'alpha_"proof"=/tmp/forged.rs\nseal_authority() {\n',
            ),
        ),
        (
            "read authority override before seal",
            lambda _d, r: replace_runner(
                r,
                "seal_authority() {\n",
                "read alpha_proof </dev/null\nseal_authority() {\n",
            ),
        ),
        (
            "parameter authority override before seal",
            lambda _d, r: replace_runner(
                r,
                "seal_authority() {\n",
                ": ${alpha_proof:=/tmp/forged.rs}\nseal_authority() {\n",
            ),
        ),
        (
            "moved authority seal after source check",
            lambda _d, r: replace_runner(
                r,
                "seal_authority\n",
                "check_sources\nseal_authority\n",
            ),
        ),
        (
            "late Verus-path override",
            lambda _d, r: r.write_text(r.read_text() + "verus_path=/bin/true\n"),
        ),
        (
            "missing Verus-path seal",
            lambda _d, r: replace_runner(r, "\\readonly verus_path verus_root\n", ""),
        ),
        (
            "moved Verus-path seal after closure use",
            lambda _d, r: replace_runner(
                r,
                "\\readonly verus_path verus_root\n"
                '"$closure_checker" "$verus_root" "$closure_manifest"\n',
                '"$closure_checker" "$verus_root" "$closure_manifest"\n'
                "\\readonly verus_path verus_root\n",
            ),
        ),
        (
            "moved runner-environment seal",
            lambda _d, r: replace_runner(
                r,
                "\\readonly runner_home runner_path runner_rustup_home runner_cargo_home\n",
                "# moved after the canonical binding block\n"
                "\\readonly runner_home runner_path runner_rustup_home runner_cargo_home\n",
            ),
        ),
        (
            "moved timeout seal",
            lambda _d, r: replace_runner(
                r,
                "\\readonly timeout_seconds\n",
                "# moved after validation\n\\readonly timeout_seconds\n",
            ),
        ),
        (
            "moved temporary-directory seal",
            lambda _d, r: replace_runner(
                r,
                "\\readonly tmp_dir\n",
                "# moved after use\n\\readonly tmp_dir\n",
            ),
        ),
        (
            "substituted digest-check body",
            lambda _d, r: replace_runner(r, "    : check-digest\n", "    return 0\n"),
        ),
        (
            "substituted source-check body",
            lambda _d, r: replace_runner(
                r,
                '    check_digest "$expected_negative_alpha" "$negative_alpha"\n',
                "    return 0\n",
            ),
        ),
        (
            "substituted Verus-runner body",
            lambda _d, r: replace_runner(r, "    : run-verus\n", "    return 0\n"),
        ),
        (
            "substituted positive-check body",
            lambda _d, r: replace_runner(r, "    : check-positive\n", "    return 0\n"),
        ),
        (
            "missing assignment",
            lambda _d, r: replace_runner(
                r, 'negative_alpha="$script_dir/negative/alpha.rs"\n', ""
            ),
        ),
        (
            "duplicate assignment",
            lambda _d, r: r.write_text(
                r.read_text() + 'negative_alpha="$script_dir/negative/alpha.rs"\n'
            ),
        ),
        (
            "leading-whitespace assignment",
            lambda _d, r: r.write_text(
                r.read_text() + ' \tnegative_alpha = "$script_dir/negative/alpha.rs"\n'
            ),
        ),
        (
            "unparsed assignment",
            lambda _d, r: r.write_text(
                r.read_text() + "export negative_alpha=$script_dir/negative/alpha.rs\n"
            ),
        ),
        (
            "double-quoted exported assignment",
            lambda _d, r: r.write_text(
                r.read_text() + 'export "negative_alpha=$script_dir/negative/beta.rs"\n'
            ),
        ),
        (
            "single-quoted exported assignment",
            lambda _d, r: r.write_text(
                r.read_text() + "export 'negative_alpha=$script_dir/negative/beta.rs'\n"
            ),
        ),
        (
            "fragment-quoted assignment name",
            lambda _d, r: r.write_text(
                r.read_text()
                + 'export "negative_alpha"="$script_dir/negative/beta.rs"\n'
            ),
        ),
        (
            "single-fragment-quoted assignment name",
            lambda _d, r: r.write_text(
                r.read_text()
                + "export 'negative_alpha'='$script_dir/negative/beta.rs'\n"
            ),
        ),
        (
            "fragment-quoted assignment suffix",
            lambda _d, r: r.write_text(
                r.read_text() + 'export negative_alpha"=$script_dir/negative/beta.rs"\n'
            ),
        ),
        (
            "token-split source assignment",
            lambda _d, r: r.write_text(
                r.read_text() + 'negative_\\\nalpha="$script_dir/negative/alpha.rs"\n'
            ),
        ),
        (
            "missing pin assignment",
            lambda _d, r: replace_runner(
                r,
                'expected_negative_alpha=$(read_pin "$pin_dir/NEGATIVE_ALPHA_SHA256")\n',
                "",
            ),
        ),
        (
            "duplicate pin assignment",
            lambda _d, r: r.write_text(
                r.read_text()
                + 'expected_negative_alpha=$(read_pin "$pin_dir/NEGATIVE_ALPHA_SHA256")\n'
            ),
        ),
        (
            "alternate-spacing pin assignment",
            lambda _d, r: r.write_text(
                r.read_text()
                + ' \texpected_negative_alpha = $( read_pin "$pin_dir/NEGATIVE_ALPHA_SHA256" )\n'
            ),
        ),
        (
            "token-split pin assignment",
            lambda _d, r: r.write_text(
                r.read_text()
                + 'expected_negative_\\\nalpha=$(read_pin "$pin_dir/NEGATIVE_ALPHA_SHA256")\n'
            ),
        ),
        (
            "missing pin file",
            lambda _d, r: (r.parent / "pins" / "NEGATIVE_ALPHA_SHA256").unlink(),
        ),
        (
            "missing digest check",
            lambda _d, r: replace_runner(
                r, '    check_digest "$expected_negative_alpha" "$negative_alpha"\n', ""
            ),
        ),
        (
            "duplicate digest check",
            lambda _d, r: replace_runner(
                r,
                '    check_digest "$expected_negative_alpha" "$negative_alpha"\n',
                '    check_digest "$expected_negative_alpha" "$negative_alpha"\n'
                '    check_digest "$expected_negative_alpha" "$negative_alpha"\n',
            ),
        ),
        (
            "alternate-spacing digest check",
            lambda _d, r: replace_runner(
                r,
                '    check_digest "$expected_negative_alpha" "$negative_alpha"\n',
                '    check_digest "$expected_negative_alpha" "$negative_alpha"\n'
                '\tcheck_digest   "$expected_negative_alpha"    "$negative_alpha"\n',
            ),
        ),
        (
            "token-split digest check",
            lambda _d, r: r.write_text(
                r.read_text()
                + 'check_\\\ndigest "$expected_negative_alpha" "$negative_alpha"\n'
            ),
        ),
        (
            "missing source-checker argument",
            lambda _d, r: replace_runner(r, '    "$negative_alpha" \\\n', ""),
        ),
        (
            "duplicate source-checker argument",
            lambda _d, r: replace_runner(
                r,
                '    "$negative_alpha" \\\n',
                '    "$negative_alpha" \\\n    "$negative_alpha" \\\n',
            ),
        ),
        (
            "alternate-spacing source-checker argument",
            lambda _d, r: replace_runner(
                r,
                '    "$negative_alpha" \\\n',
                '    "$negative_alpha" \\\n\t  "$negative_alpha"   \\\n',
            ),
        ),
        (
            "unparsed source-checker argument",
            lambda _d, r: replace_runner(
                r, '    "$negative_alpha" \\\n', '    "${negative_alpha}" \\\n'
            ),
        ),
        (
            "unparsed source-checker invocation",
            lambda _d, r: r.write_text(
                r.read_text() + 'env "$source_checker" "$negative_alpha"\n'
            ),
        ),
        (
            "missing verification call",
            lambda _d, r: replace_runner(
                r,
                'check_negative "$negative_alpha" alpha_postcondition alpha-label\n',
                "",
            ),
        ),
        (
            "duplicate verification call",
            lambda _d, r: r.write_text(
                r.read_text()
                + 'check_negative "$negative_alpha" alpha_postcondition alpha-label\n'
            ),
        ),
        (
            "alternate-spacing verification call",
            lambda _d, r: r.write_text(
                r.read_text()
                + '\tcheck_negative   "$negative_alpha"   alpha_postcondition   alpha-label\n'
            ),
        ),
        (
            "token-split verification call",
            lambda _d, r: r.write_text(
                r.read_text()
                + 'check_\\\nnegative "$negative_alpha" alpha_postcondition alpha-label\n'
            ),
        ),
        (
            "double-quoted verification command",
            lambda _d, r: r.write_text(
                r.read_text()
                + '"check_negative" "$negative_alpha" alpha_postcondition alpha-label\n'
            ),
        ),
        (
            "single-quoted verification command",
            lambda _d, r: r.write_text(
                r.read_text()
                + "'check_negative' \"$negative_alpha\" alpha_postcondition alpha-label\n"
            ),
        ),
        (
            "fragment-double-quoted verification command",
            lambda _d, r: r.write_text(
                r.read_text()
                + 'check_"negative" "$negative_alpha" alpha_postcondition alpha-label\n'
            ),
        ),
        (
            "fragment-single-quoted verification command",
            lambda _d, r: r.write_text(
                r.read_text()
                + "check_'negative' \"$negative_alpha\" alpha_postcondition alpha-label\n"
            ),
        ),
        (
            "fragment-quoted digest command",
            lambda _d, r: r.write_text(
                r.read_text()
                + 'check_"digest" "$expected_negative_alpha" "$negative_alpha"\n'
            ),
        ),
        (
            "escaped verification command fragment",
            lambda _d, r: r.write_text(
                r.read_text()
                + 'check_\\negative "$negative_alpha" alpha_postcondition alpha-label\n'
            ),
        ),
        (
            "duplicate check-negative definition",
            lambda _d, r: r.write_text(
                r.read_text() + "check_negative() {\n    :\n}\n"
            ),
        ),
        (
            "substituted check-negative body",
            lambda _d, r: replace_runner(r, "    :\n", "    return 0\n"),
        ),
        (
            "quoted here-document omitted call",
            lambda _d, r: replace_runner(
                r,
                'check_negative "$negative_alpha" alpha_postcondition alpha-label\n',
                "cat >/dev/null <<'END'\n"
                'check_negative "$negative_alpha" alpha_postcondition alpha-label\n'
                "END\n",
            ),
        ),
        (
            "unquoted here-document omitted call",
            lambda _d, r: replace_runner(
                r,
                'check_negative "$negative_alpha" alpha_postcondition alpha-label\n',
                "cat >/dev/null <<END\n"
                'check_negative "$negative_alpha" alpha_postcondition alpha-label\n'
                "END\n",
            ),
        ),
        (
            "tab-stripping here-document omitted call",
            lambda _d, r: replace_runner(
                r,
                'check_negative "$negative_alpha" alpha_postcondition alpha-label\n',
                "cat >/dev/null <<-'END'\n"
                '\tcheck_negative "$negative_alpha" alpha_postcondition alpha-label\n'
                "\tEND\n",
            ),
        ),
        (
            "missing count",
            lambda _d, r: replace_runner(
                r, "expected_negative_files=2", "negative_count=2"
            ),
        ),
        (
            "duplicate count",
            lambda _d, r: r.write_text(
                r.read_text() + "transcript='expected_negative_files=2'\n"
            ),
        ),
        (
            "wrong count",
            lambda _d, r: replace_runner(
                r, "expected_negative_files=2", "expected_negative_files=3"
            ),
        ),
    ]
    expected_errors: dict[str, str] = {}

    def expect(code: str, *case_labels: str) -> None:
        for case_label in case_labels:
            expected_errors[case_label] = code

    expect("runner.raw_digest", "complete runner raw-byte substitution")
    expect("roster.source_count", "extra regular source")
    expect("source.unexpected_entry", "symlink source", "non-Rust source")
    expect(
        "runner.block.root_binding.count",
        "missing root readonly",
        "moved root readonly",
        "mutated script root",
    )
    expect(
        "runner.root.unparsed_assignment",
        "quoted script-root override",
        "search-path override",
    )
    expect("runner.block.environment.count", "inherited search path")
    expect(
        "runner.block.tool_binding.count",
        "missing tool readonly",
        "mutated source-checker binding",
    )
    expect(
        "runner.tool_wiring.unparsed",
        "closure-checker override after authentication",
        "source-checker override after authentication",
        "fragment-quoted negative-quality-checker override",
        "unparsed source-checker invocation",
    )
    expect(
        "runner.closure_checker.invocation_count",
        "missing closure-checker invocation",
    )
    expect("runner.quality_audit.count", "missing negative-quality invocation")
    expect(
        "runner.block.negative_quality_invocation.count",
        "substituted negative-quality self-test",
    )
    expect(
        "runner.grammar.alias_or_eval",
        "readonly alias",
        "digest-check alias",
        "result-check alias",
        "shell eval",
    )
    expect(
        "runner.grammar.continuation_split_function",
        "result-check function",
        "split-line result-check function",
        "split-line authenticated function replacement",
        "continuation-split result-check function",
    )
    expect("runner.function.read_pin.digest", "substituted pin-reader body")
    expect("runner.function.seal_authority.digest", "substituted authority-seal body")
    expect("runner.preseal.call_before_check", "missing authority-seal call")
    expect(
        "roster.duplicate.pre-seal_authority_assignment",
        "positive authority override before seal",
    )
    expect(
        "runner.preseal.unparsed_assignment",
        "quoted authority override before seal",
        "parameter authority override before seal",
        "duplicate assignment",
        "leading-whitespace assignment",
        "duplicate pin assignment",
        "alternate-spacing pin assignment",
    )
    expect("runner.preseal.unparsed_mutator", "read authority override before seal")
    expect("runner.preseal.call_adjacency", "moved authority seal after source check")
    expect("runner.late_authority.unparsed_assignment", "late Verus-path override")
    expect("runner.late_authority.seal_count", "missing Verus-path seal")
    expect(
        "runner.block.late_authority_binding.count",
        "moved Verus-path seal after closure use",
        "moved runner-environment seal",
        "moved timeout seal",
        "moved temporary-directory seal",
    )
    expect("runner.function.check_digest.digest", "substituted digest-check body")
    expect("runner.function.check_sources.digest", "substituted source-check body")
    expect("runner.function.run_verus.digest", "substituted Verus-runner body")
    expect("runner.function.check_positive.digest", "substituted positive-check body")
    expect("roster.source_assignment_mismatch", "missing assignment")
    expect(
        "runner.grammar.unparsed_source_assignment",
        "unparsed assignment",
        "fragment-quoted assignment name",
        "single-fragment-quoted assignment name",
        "fragment-quoted assignment suffix",
    )
    expect(
        "runner.grammar.quoted_negative_assignment",
        "double-quoted exported assignment",
        "single-quoted exported assignment",
    )
    expect(
        "runner.grammar.token_split",
        "token-split source assignment",
        "token-split pin assignment",
        "token-split digest check",
        "token-split verification call",
    )
    expect("roster.pin_assignment_mismatch", "missing pin assignment")
    expect("roster.pin_unavailable", "missing pin file")
    expect("roster.digest_check_mismatch", "missing digest check")
    expect(
        "roster.duplicate.negative_digest_check",
        "duplicate digest check",
        "alternate-spacing digest check",
    )
    expect("roster.source_checker_mismatch", "missing source-checker argument")
    expect(
        "roster.duplicate.negative_source-checker_argument",
        "duplicate source-checker argument",
        "alternate-spacing source-checker argument",
    )
    expect(
        "runner.source_checker.unparsed_argument", "unparsed source-checker argument"
    )
    expect("roster.verification_call_mismatch", "missing verification call")
    expect(
        "roster.duplicate.negative_verification_call",
        "duplicate verification call",
        "alternate-spacing verification call",
    )
    expect(
        "runner.grammar.quoted_negative_command",
        "double-quoted verification command",
        "single-quoted verification command",
    )
    expect(
        "runner.grammar.unparsed_verification_call",
        "fragment-double-quoted verification command",
        "fragment-single-quoted verification command",
        "escaped verification command fragment",
    )
    expect("runner.grammar.unparsed_digest_check", "fragment-quoted digest command")
    expect(
        "runner.function.check_negative.count", "duplicate check-negative definition"
    )
    expect("runner.function.check_negative.digest", "substituted check-negative body")
    expect(
        "runner.grammar.heredoc",
        "quoted here-document omitted call",
        "unquoted here-document omitted call",
        "tab-stripping here-document omitted call",
    )
    expect("roster.transcript_count", "missing count", "duplicate count", "wrong count")
    labels = [label for label, _ in cases]
    if len(labels) != len(set(labels)) or set(labels) != set(expected_errors):
        raise QualityError(
            "inventory self-test rejection roster is not exact",
            code="self_test.roster_mismatch",
        )
    reauth_preseal = {"missing assignment", "missing pin assignment"}
    reauth_check_sources = {
        "missing digest check",
        "duplicate digest check",
        "alternate-spacing digest check",
    }
    for label, build_case in cases:
        expect_inventory_rejection(
            label,
            expected_errors[label],
            build_case,
            reauth_preseal=label in reauth_preseal,
            reauth_functions=("check_sources",)
            if label in reauth_check_sources
            else (),
        )

    case_by_label = dict(cases)
    direct_roster_cases = {
        "duplicate assignment": "roster.duplicate.negative_source_assignment",
        "leading-whitespace assignment": "roster.duplicate.negative_source_assignment",
        "duplicate pin assignment": "roster.duplicate.negative_pin_assignment",
        "alternate-spacing pin assignment": "roster.duplicate.negative_pin_assignment",
    }
    for label, expected_code in direct_roster_cases.items():
        expect_roster_rejection(label, expected_code, case_by_label[label])


def self_test(reject_fixture: Path, accept_fixture: Path) -> None:
    try:
        reject_source = reject_fixture.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        raise QualityError(str(error)) from error
    literals = literal_bool_specs(reject_source)
    expected = {
        ("literal_true_direct_v1", "true"),
        ("literal_false_braced_v1", "false"),
        ("literal_true_parenthesized_v1", "true"),
        ("literal_false_equality_v1", "false"),
        ("literal_true_assert_v1", "true"),
        ("literal_false_wrapper_v1", "false"),
        ("r#literal_raw_identifier_v1", "true"),
        ("literal_true_parenthesized_return_v1", "true"),
        ("literal_false_qualified_return_v1", "false"),
        ("literal_true_absolute_qualified_return_v1", "true"),
    }
    if set(literals) != expected or len(literals) != len(expected):
        raise QualityError("reject fixture did not expose every literal-bool shape")
    try:
        scan_source(reject_source)
    except QualityError as error:
        if "zero-argument literal-bool spec constant" not in str(error):
            raise QualityError(
                f"reject fixture failed unexpectedly: {error}"
            ) from error
    else:
        raise QualityError("reject fixture unexpectedly passed")
    try:
        scan_source("spec fn \u00e9chappe_v1() -> bool { true }")
    except QualityError as error:
        if "non-ASCII Rust code" not in str(error):
            raise QualityError(
                f"Unicode-identifier fixture failed unexpectedly: {error}"
            ) from error
    else:
        raise QualityError("Unicode-identifier fixture unexpectedly passed")
    scan_source("spec fn relation_v1() -> (bool) { 1 == 1 }")
    scan_source("spec fn qualified_relation_v1() -> core::primitive::bool { 1 != 2 }")
    scan_path(accept_fixture)
    inventory_self_test()


def usage() -> int:
    print(
        f"usage: {sys.argv[0]} DIRECTORY RUNNER\n"
        f"       {sys.argv[0]} --self-test REJECT_FIXTURE ACCEPT_FIXTURE",
        file=sys.stderr,
    )
    return 2


def main() -> int:
    try:
        if len(sys.argv) == 4 and sys.argv[1] == "--self-test":
            self_test(Path(sys.argv[2]), Path(sys.argv[3]))
            print("PASS: runtime expected-negative quality and inventory self-test")
            return 0
        if len(sys.argv) != 3 or sys.argv[1] == "--self-test":
            return usage()
        sources = audit_inventory(Path(sys.argv[1]), Path(sys.argv[2]))
        for path in sources:
            try:
                scan_path(path)
            except QualityError as error:
                raise QualityError(f"{path}: {error}") from error
        print(
            f"PASS: runtime expected-negative quality and inventory checked "
            f"{len(sources)} files"
        )
        return 0
    except QualityError as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
