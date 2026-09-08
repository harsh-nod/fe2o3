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


EXPECTED_RUNTIME_NEGATIVE_COUNT = 535
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


def literal_bool_specs(source: str) -> list[tuple[str, str]]:
    tokens = TOKEN.findall(code_only(source))
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
        if scan + 1 >= len(tokens) or tokens[scan : scan + 2] != ["->", "bool"]:
            cursor = scan
            continue
        scan += 2
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
        raise QualityError(f"negative source directory is unavailable: {directory}")
    entries = sorted(directory.iterdir())
    unexpected = [
        path
        for path in entries
        if path.is_symlink() or not path.is_file() or path.suffix != ".rs"
    ]
    if unexpected:
        raise QualityError(f"unexpected negative source entry: {unexpected[0]}")
    if not entries:
        raise QualityError(f"negative source directory is empty: {directory}")
    return entries


def require_unique(stage: str, values: list[str]) -> None:
    duplicates = sorted(value for value, count in Counter(values).items() if count != 1)
    if duplicates:
        raise QualityError(f"duplicate {stage}: {duplicates[0]}")


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
        raise QualityError("unterminated shell quote in verification runner")
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
                        f"token-splitting shell continuation at runner line {line}"
                    )
                cursor += 1 + newline_length
                continue
            cursor += 2
            continue
        cursor += 1


def reject_unparsed_runner_forms(runner_source: str) -> None:
    for line_number, original_line in enumerate(runner_source.splitlines(), start=1):
        line = shell_line_code(original_line)
        if not line.strip():
            continue
        projection = shell_unquoted_projection(line)
        source_assignment = SOURCE_ASSIGNMENT.fullmatch(line)
        pin_assignment = PIN_ASSIGNMENT.fullmatch(line)
        digest_check = DIGEST_CHECK.fullmatch(line)
        negative_call = NEGATIVE_CALL.fullmatch(line)
        if re.search(r"\bnegative_(?!quality_)[A-Za-z0-9_]*[ \t]*=", projection):
            if source_assignment is None:
                raise QualityError(
                    f"unparsed negative source assignment at runner line {line_number}"
                )
        if re.search(
            r"\bexpected_negative_(?!quality_)[A-Za-z0-9_]*[ \t]*=", projection
        ):
            if pin_assignment is None:
                raise QualityError(
                    f"unparsed negative pin assignment at runner line {line_number}"
                )
        if re.search(r"\bcheck_digest\b", projection) and "negative_" in line:
            if digest_check is None:
                raise QualityError(
                    f"unparsed negative digest check at runner line {line_number}"
                )
        if re.search(r"\bcheck_negative\b", projection):
            if re.fullmatch(
                r"[ \t]*check_negative[ \t]*\([ \t]*\)[ \t]*\{[ \t]*", line
            ):
                continue
            if negative_call is None:
                raise QualityError(
                    f"unparsed negative verification call at runner line {line_number}"
                )


def source_checker_arguments(runner_source: str) -> list[str]:
    lines = runner_source.splitlines()
    start_pattern = re.compile(r'^[ \t]*"\$source_checker"[ \t]+\\[ \t]*$')
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
                f"unparsed source-checker invocation at runner line {index + 1}"
            )
    if len(starts) != 1:
        raise QualityError("runner must contain exactly one source-checker invocation")
    arguments: list[str] = []
    cursor = starts[0] + 1
    while cursor < len(lines):
        line = shell_line_code(lines[cursor])
        match = re.fullmatch(
            r'[ \t]*"\$([A-Za-z_][A-Za-z0-9_]*)"(?:[ \t]+\\)?[ \t]*', line
        )
        if match is None:
            raise QualityError(
                f"unparsed source-checker argument at runner line {cursor + 1}"
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
        raise QualityError(f"verification runner is unavailable: {path}")
    try:
        return path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        raise QualityError(str(error)) from error


def audit_inventory(
    directory: Path,
    runner: Path,
    expected_count: int = EXPECTED_RUNTIME_NEGATIVE_COUNT,
) -> list[Path]:
    sources = runtime_negative_sources(directory)
    runner_source = read_runner(runner)
    reject_token_split_continuations(runner_source)
    normalized_runner = normalized_runner_source(runner_source)
    reject_unparsed_runner_forms(runner_source)
    assignments = SOURCE_ASSIGNMENT.findall(normalized_runner)
    variables = [variable for variable, _ in assignments]
    filenames = [filename for _, filename in assignments]
    require_unique("negative source assignment", variables)
    require_unique("negative source filename assignment", filenames)
    disk_names = [path.name for path in sources]
    if len(sources) != expected_count:
        raise QualityError(
            f"negative source count is {len(sources)}, expected {expected_count}"
        )
    if set(filenames) != set(disk_names):
        missing = sorted(set(disk_names) - set(filenames))
        extra = sorted(set(filenames) - set(disk_names))
        detail = (missing or extra)[0]
        raise QualityError(f"negative source assignment inventory mismatch: {detail}")

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
            f"unexpected negative pin assignment: {sorted(unexpected_pin_variables)[0]}"
        )
    require_unique("negative pin assignment", relevant_pin_variables)
    require_unique("negative pin file", pin_names)
    if set(relevant_pin_variables) != expected_variables:
        detail = sorted(
            expected_variables.symmetric_difference(relevant_pin_variables)
        )[0]
        raise QualityError(f"negative pin assignment inventory mismatch: {detail}")
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
        raise QualityError(f"negative digest-check inventory mismatch: {difference}")

    checked_arguments = source_checker_arguments(runner_source)
    require_unique("negative source-checker argument", checked_arguments)
    if set(checked_arguments) != variable_set:
        detail = sorted(variable_set.symmetric_difference(checked_arguments))[0]
        raise QualityError(f"negative source-checker inventory mismatch: {detail}")

    calls = NEGATIVE_CALL.findall(normalized_runner)
    called_variables = [variable for variable, _, _ in calls]
    labels = [label for _, _, label in calls]
    require_unique("negative verification call", called_variables)
    require_unique("negative verification label", labels)
    if set(called_variables) != variable_set:
        detail = sorted(variable_set.symmetric_difference(called_variables))[0]
        raise QualityError(f"negative verification-call inventory mismatch: {detail}")

    counts = [int(value) for value in TRANSCRIPT_COUNT.findall(normalized_runner)]
    if counts != [expected_count]:
        raise QualityError(
            f"runner must contain one expected-negative count equal to {expected_count}"
        )

    assignment_by_variable = dict(assignments)
    pin_root = runner.parent / "pins"
    for variable in variables:
        expected_variable = f"expected_{variable}"
        pin_path = pin_root / pin_by_variable[expected_variable]
        if pin_path.is_symlink() or not pin_path.is_file():
            raise QualityError(f"negative source pin is unavailable: {pin_path}")
        try:
            pin_lines = pin_path.read_text(encoding="utf-8").splitlines()
        except (OSError, UnicodeError) as error:
            raise QualityError(str(error)) from error
        if len(pin_lines) != 1 or HEX_SHA256.fullmatch(pin_lines[0]) is None:
            raise QualityError(f"invalid negative source pin: {pin_path}")
        source_path = directory / assignment_by_variable[variable]
        digest = hashlib.sha256(source_path.read_bytes()).hexdigest()
        if digest != pin_lines[0]:
            raise QualityError(f"negative source digest mismatch: {source_path}")
    return sources


def expect_inventory_rejection(label: str, build_case) -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-negative-quality-") as temp:
        root = Path(temp)
        directory, runner = build_inventory_fixture(root)
        build_case(directory, runner)
        try:
            audit_inventory(directory, runner, expected_count=2)
        except QualityError:
            return
        raise QualityError(f"inventory self-test unexpectedly accepted {label}")


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
                'negative_alpha="$script_dir/negative/alpha.rs"',
                'negative_beta="$script_dir/negative/beta.rs"',
                'expected_negative_alpha=$(read_pin "$pin_dir/NEGATIVE_ALPHA_SHA256")',
                'expected_negative_beta=$(read_pin "$pin_dir/NEGATIVE_BETA_SHA256")',
                '    check_digest "$expected_negative_alpha" "$negative_alpha"',
                '    check_digest "$expected_negative_beta" "$negative_beta"',
                '"$source_checker" \\',
                '    "$negative_alpha" \\',
                '    "$negative_beta"',
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


def inventory_self_test() -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-negative-quality-") as temp:
        directory, runner = build_inventory_fixture(Path(temp))
        audit_inventory(directory, runner, expected_count=2)

    cases = [
        ("extra regular source", lambda d, _r: (d / "extra.rs").write_text("x\n")),
        ("symlink source", lambda d, _r: os.symlink(d / "alpha.rs", d / "link.rs")),
        ("non-Rust source", lambda d, _r: (d / "note.txt").write_text("x\n")),
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
            lambda _d, r: r.write_text(
                r.read_text()
                + '    check_digest "$expected_negative_alpha" "$negative_alpha"\n'
            ),
        ),
        (
            "alternate-spacing digest check",
            lambda _d, r: r.write_text(
                r.read_text()
                + '\tcheck_digest   "$expected_negative_alpha"    "$negative_alpha"\n'
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
    for label, build_case in cases:
        expect_inventory_rejection(label, build_case)


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
