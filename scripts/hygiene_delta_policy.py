#!/usr/bin/env python3
"""Reject new source-shape debt in a candidate diff."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
from pathlib import Path
import re
import subprocess
import sys


REPO_ROOT = Path(__file__).resolve().parent.parent
MAX_NEW_SOURCE_LINES = 1200
MAX_LARGE_SOURCE_GROWTH_LINES = 250
DUPLICATE_MIN_BYTES = 4096
DUPLICATE_MIN_NONBLANK_LINES = 80
ALLOW_PANIC_MARKER = "fe2o3-hygiene: allow-panic"
PANIC_MACRO_RE = re.compile(r"\b(?:panic|todo|unimplemented)\s*!")
CFG_TEST_RE = re.compile(r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]")
MODULE_OPEN_RE = re.compile(r"\bmod\s+[A-Za-z_][A-Za-z0-9_]*\s*\{")
HUNK_RE = re.compile(r"^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@")


class HygieneInputError(ValueError):
    """The repository, refs, or diff cannot be inspected safely."""


@dataclass(frozen=True)
class ChangedPath:
    status: str
    path: str
    old_path: str | None = None


def run_git(repo: Path, args: list[str], *, text: bool = False) -> bytes | str:
    result = subprocess.run(
        ["git", "-C", str(repo), *args],
        check=False,
        capture_output=True,
        text=text,
    )
    if result.returncode != 0:
        stderr = result.stderr if text else result.stderr.decode("utf-8", "replace")
        raise HygieneInputError(
            f"git {' '.join(args[:2])} failed with status {result.returncode}: {stderr.strip()}"
        )
    return result.stdout


def resolve_repo(path: Path) -> Path:
    try:
        path = path.resolve(strict=True)
    except OSError as error:
        raise HygieneInputError(f"repository path cannot be resolved: {error}") from error
    output = run_git(path, ["rev-parse", "--show-toplevel"], text=True)
    return Path(str(output).strip()).resolve()


def validate_commit(repo: Path, label: str, value: str) -> None:
    if not value or value.startswith("-") or any(character.isspace() for character in value):
        raise HygieneInputError(f"{label} ref is malformed")
    run_git(repo, ["cat-file", "-e", f"{value}^{{commit}}"])


def parse_name_status(raw: bytes) -> list[ChangedPath]:
    fields = raw.decode("utf-8", "surrogateescape").split("\0")
    changes: list[ChangedPath] = []
    index = 0
    while index < len(fields) and fields[index]:
        status = fields[index]
        index += 1
        if status.startswith("R"):
            if index + 1 >= len(fields):
                raise HygieneInputError("rename record is truncated")
            old_path = fields[index]
            path = fields[index + 1]
            index += 2
            changes.append(ChangedPath(status=status, old_path=old_path, path=path))
            continue
        if index >= len(fields):
            raise HygieneInputError("name-status record is truncated")
        path = fields[index]
        index += 1
        changes.append(ChangedPath(status=status, path=path))
    return changes


def changed_paths(repo: Path, base: str, head: str) -> list[ChangedPath]:
    raw = run_git(
        repo,
        ["diff", "--name-status", "-z", "--diff-filter=AMR", base, head, "--"],
    )
    return parse_name_status(raw)


def is_production_source(path: str) -> bool:
    parts = path.split("/")
    if len(parts) < 4 or not path.endswith(".rs"):
        return False
    if parts[0] not in {"crates", "tools"} or parts[2] != "src":
        return False
    return "/tests/fixtures/" not in f"/{path}/"


def is_test_source(path: str) -> bool:
    parts = path.split("/")
    if len(parts) < 4:
        return False
    relative = parts[3:]
    name = relative[-1]
    return (
        relative[0] == "tests"
        or name == "tests.rs"
        or name.endswith("_tests.rs")
        or "/tests/fixtures/" in f"/{path}/"
    )


def git_file_bytes(repo: Path, ref: str, path: str) -> bytes | None:
    result = subprocess.run(
        ["git", "-C", str(repo), "show", f"{ref}:{path}"],
        check=False,
        capture_output=True,
    )
    if result.returncode == 0:
        return result.stdout
    return None


def physical_line_count(content: bytes) -> int:
    return len(content.splitlines())


def normalized_source_bytes(content: bytes) -> bytes:
    return content.replace(b"\r\n", b"\n").replace(b"\r", b"\n")


def nonblank_line_count(content: bytes) -> int:
    return sum(1 for line in normalized_source_bytes(content).split(b"\n") if line.strip())


def check_file_growth(
    repo: Path, base: str, head: str, changes: list[ChangedPath]
) -> list[str]:
    violations: list[str] = []
    for change in changes:
        if not is_production_source(change.path):
            continue
        head_content = git_file_bytes(repo, head, change.path)
        if head_content is None:
            continue
        head_lines = physical_line_count(head_content)
        base_path = change.old_path if change.old_path else change.path
        base_content = (
            git_file_bytes(repo, base, base_path)
            if base_path and is_production_source(base_path)
            else None
        )
        if base_content is None:
            if head_lines > MAX_NEW_SOURCE_LINES:
                violations.append(
                    f"{change.path}: new production source file has {head_lines} lines; "
                    f"limit is {MAX_NEW_SOURCE_LINES}"
                )
            continue
        base_lines = physical_line_count(base_content)
        if base_lines <= MAX_NEW_SOURCE_LINES < head_lines:
            violations.append(
                f"{change.path}: production source file crossed from {base_lines} "
                f"to {head_lines} lines; limit is {MAX_NEW_SOURCE_LINES}"
            )
        elif (
            base_lines > MAX_NEW_SOURCE_LINES
            and head_lines - base_lines > MAX_LARGE_SOURCE_GROWTH_LINES
        ):
            violations.append(
                f"{change.path}: already-large production source file grew by "
                f"{head_lines - base_lines} lines; limit is "
                f"{MAX_LARGE_SOURCE_GROWTH_LINES}"
            )
    return violations


def list_head_production_sources(repo: Path, head: str) -> list[str]:
    raw = run_git(repo, ["ls-tree", "-r", "-z", "--name-only", head, "--"])
    paths = raw.decode("utf-8", "surrogateescape").split("\0")
    return sorted(path for path in paths if path and is_production_source(path))


def check_exact_duplicates(
    repo: Path, head: str, changes: list[ChangedPath]
) -> list[str]:
    changed = sorted({change.path for change in changes if is_production_source(change.path)})
    candidate_paths: set[str] = set()
    for path in changed:
        content = git_file_bytes(repo, head, path)
        if content is None:
            continue
        normalized = normalized_source_bytes(content)
        if (
            len(normalized) >= DUPLICATE_MIN_BYTES
            and nonblank_line_count(normalized) >= DUPLICATE_MIN_NONBLANK_LINES
        ):
            candidate_paths.add(path)
    if not candidate_paths:
        return []

    by_digest: dict[str, list[str]] = {}
    for path in list_head_production_sources(repo, head):
        content = git_file_bytes(repo, head, path)
        if content is None:
            continue
        digest = hashlib.sha256(normalized_source_bytes(content)).hexdigest()
        by_digest.setdefault(digest, []).append(path)

    violations: list[str] = []
    for path in sorted(candidate_paths):
        content = git_file_bytes(repo, head, path)
        if content is None:
            continue
        digest = hashlib.sha256(normalized_source_bytes(content)).hexdigest()
        duplicates = [candidate for candidate in by_digest[digest] if candidate != path]
        if duplicates:
            violations.append(
                f"{path}: changed production source exactly duplicates {duplicates[0]}"
            )
    return violations


def raw_string_prefix(line: str, index: int) -> tuple[int, int] | None:
    if line.startswith("br", index) or line.startswith("cr", index):
        cursor = index + 2
    elif line.startswith("r", index):
        cursor = index + 1
    else:
        return None
    while cursor < len(line) and line[cursor] == "#":
        cursor += 1
    if cursor >= len(line) or line[cursor] != '"':
        return None
    return cursor - index + 1, cursor - index - (2 if line[index] in {"b", "c"} else 1)


def starts_char_literal(line: str, index: int) -> bool:
    if index + 1 >= len(line):
        return False
    next_character = line[index + 1]
    if next_character == "\\":
        return True
    if next_character.isalpha() or next_character == "_":
        return index + 2 < len(line) and line[index + 2] == "'"
    return True


def sanitize_rust_lines(lines: list[str]) -> list[str]:
    sanitized: list[str] = []
    block_depth = 0
    in_string = False
    in_char = False
    raw_hashes: int | None = None

    for line in lines:
        cursor = 0
        output: list[str] = []
        while cursor < len(line):
            if raw_hashes is not None:
                suffix = '"' + ("#" * raw_hashes)
                if line.startswith(suffix, cursor):
                    output.append(" " * len(suffix))
                    cursor += len(suffix)
                    raw_hashes = None
                else:
                    output.append(" ")
                    cursor += 1
                continue
            if block_depth:
                if line.startswith("/*", cursor):
                    block_depth += 1
                    output.append("  ")
                    cursor += 2
                elif line.startswith("*/", cursor):
                    block_depth -= 1
                    output.append("  ")
                    cursor += 2
                else:
                    output.append(" ")
                    cursor += 1
                continue
            if in_string:
                if line[cursor] == "\\" and cursor + 1 < len(line):
                    output.append("  ")
                    cursor += 2
                elif line[cursor] == '"':
                    in_string = False
                    output.append(" ")
                    cursor += 1
                else:
                    output.append(" ")
                    cursor += 1
                continue
            if in_char:
                if line[cursor] == "\\" and cursor + 1 < len(line):
                    output.append("  ")
                    cursor += 2
                elif line[cursor] == "'":
                    in_char = False
                    output.append(" ")
                    cursor += 1
                else:
                    output.append(" ")
                    cursor += 1
                continue

            raw_prefix = raw_string_prefix(line, cursor)
            if raw_prefix is not None:
                prefix_length, hashes = raw_prefix
                raw_hashes = hashes
                output.append(" " * prefix_length)
                cursor += prefix_length
            elif line.startswith("//", cursor):
                break
            elif line.startswith("/*", cursor):
                block_depth += 1
                output.append("  ")
                cursor += 2
            elif line[cursor] in {"b", "c"} and cursor + 1 < len(line) and line[cursor + 1] == '"':
                in_string = True
                output.append("  ")
                cursor += 2
            elif line[cursor] == '"':
                in_string = True
                output.append(" ")
                cursor += 1
            elif line[cursor] in {"b", "c"} and cursor + 1 < len(line) and line[cursor + 1] == "'":
                in_char = True
                output.append("  ")
                cursor += 2
            elif line[cursor] == "'" and starts_char_literal(line, cursor):
                in_char = True
                output.append(" ")
                cursor += 1
            else:
                output.append(line[cursor])
                cursor += 1
        sanitized.append("".join(output))
    return sanitized


def cfg_test_module_lines(lines: list[str]) -> set[int]:
    sanitized = sanitize_rust_lines(lines)
    pending_cfg_test = False
    depth = 0
    active_closing_depths: list[int] = []
    skipped: set[int] = set()

    for line_number, line in enumerate(sanitized, start=1):
        if active_closing_depths:
            skipped.add(line_number)
        if CFG_TEST_RE.search(line):
            pending_cfg_test = True
        starts_test_module = pending_cfg_test and MODULE_OPEN_RE.search(line) is not None
        if starts_test_module:
            skipped.add(line_number)
            active_closing_depths.append(depth)
            pending_cfg_test = False

        depth += line.count("{") - line.count("}")
        while active_closing_depths and depth <= active_closing_depths[-1]:
            active_closing_depths.pop()

        stripped = line.strip()
        if pending_cfg_test and stripped and not stripped.startswith("#"):
            pending_cfg_test = False
    return skipped


def added_lines(repo: Path, base: str, head: str, path: str) -> list[int]:
    diff = run_git(
        repo,
        ["diff", "--unified=0", "--no-color", base, head, "--", path],
        text=True,
    )
    added: list[int] = []
    next_line: int | None = None
    for line in str(diff).splitlines():
        match = HUNK_RE.match(line)
        if match:
            next_line = int(match.group(1))
            continue
        if next_line is None:
            continue
        if line.startswith("+++") or line.startswith("---"):
            continue
        if line.startswith("+"):
            added.append(next_line)
            next_line += 1
        elif line.startswith("-"):
            continue
        elif line.startswith(" "):
            next_line += 1
    return added


def check_added_panic_macros(
    repo: Path, base: str, head: str, changes: list[ChangedPath]
) -> list[str]:
    violations: list[str] = []
    for path in sorted({change.path for change in changes if is_production_source(change.path)}):
        if is_test_source(path):
            continue
        content = git_file_bytes(repo, head, path)
        if content is None:
            continue
        lines = content.decode("utf-8", "replace").splitlines()
        sanitized = sanitize_rust_lines(lines)
        cfg_test_lines = cfg_test_module_lines(lines)
        for line_number in added_lines(repo, base, head, path):
            if line_number < 1 or line_number > len(lines):
                continue
            if line_number in cfg_test_lines:
                continue
            raw = lines[line_number - 1]
            previous = lines[line_number - 2] if line_number >= 2 else ""
            if ALLOW_PANIC_MARKER in raw or ALLOW_PANIC_MARKER in previous:
                continue
            if PANIC_MACRO_RE.search(sanitized[line_number - 1]):
                violations.append(f"{path}:{line_number}: new production panic macro")
    return violations


def check_hygiene(repo: Path, base: str, head: str) -> list[str]:
    repo = resolve_repo(repo)
    validate_commit(repo, "base", base)
    validate_commit(repo, "head", head)
    changes = changed_paths(repo, base, head)
    violations: list[str] = []
    violations.extend(check_file_growth(repo, base, head, changes))
    violations.extend(check_exact_duplicates(repo, head, changes))
    violations.extend(check_added_panic_macros(repo, base, head, changes))
    return sorted(violations)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=REPO_ROOT)
    parser.add_argument("--base", required=True)
    parser.add_argument("--head", required=True)
    args = parser.parse_args(argv)

    try:
        violations = check_hygiene(args.repo, args.base, args.head)
    except HygieneInputError as error:
        print(f"hygiene delta policy input error: {error}", file=sys.stderr)
        return 2

    if violations:
        print("hygiene delta policy failed:", file=sys.stderr)
        for violation in violations:
            print(f"  - {violation}", file=sys.stderr)
        return 1
    print("hygiene delta policy: OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
