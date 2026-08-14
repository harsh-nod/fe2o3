#!/usr/bin/env python3
"""Fail-closed lexer for the row-softmax Verus proof corpus."""

from __future__ import annotations

import dataclasses
import hashlib
import pathlib
import re
import sys
import unicodedata


VOCABULARY_FORMAT = "FE2O3-ROW-SOFTMAX-VERUS-TRUST-VOCABULARY-V1"
DEFAULT_VOCABULARY = pathlib.Path(__file__).parent / "verus/VERUS_TRUST_VOCABULARY"
REQUIRED_TRUST_TOKENS = {
    "admit",
    "assume",
    "assume_specification",
    "assume_termination",
    "axiom",
    "exec_allows_no_decreases_clause",
    "external",
    "external_body",
    "external_derive",
    "external_fn_specification",
    "external_trait_blanket",
    "external_trait_extension",
    "external_trait_private_bound",
    "external_trait_specification",
    "external_type_specification",
    "externals_available_without_declaration",
    "trusted",
    "uninterp",
}
CLOSED_SOURCE_PREFIX = [
    "use",
    "vstd",
    "::",
    "prelude",
    "::",
    "*",
    ";",
    "verus",
    "!",
    "{",
]
SOURCE_INJECTION_IDENTIFIERS = {
    "extern",
    "include",
    "include_bytes",
    "include_str",
    "macro",
    "macro_rules",
    "mod",
}
APPROVED_EXP_DECLARATION = [
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


@dataclasses.dataclass(frozen=True)
class Vocabulary:
    version: str
    upstream_commit: str
    parser_tokens: frozenset[str]
    parser_decisions: frozenset[str]
    trust_tokens: frozenset[str]
    defensive_tokens: frozenset[str]
    release_roots: tuple[str, ...]
    release_sources: tuple[tuple[str, int, str], ...]
    upstream_source: tuple[str, int, str]


def one_value(fields: dict[str, list[str]], name: str) -> str:
    values = fields.get(name, [])
    if len(values) != 1:
        raise ScanError(f"trust vocabulary requires exactly one {name} field")
    return values[0]


def sorted_unique_values(fields: dict[str, list[str]], name: str) -> tuple[str, ...]:
    values = fields.get(name, [])
    if not values or values != sorted(set(values)):
        raise ScanError(f"trust vocabulary {name} fields must be nonempty, sorted, and unique")
    return tuple(values)


def load_vocabulary(path: pathlib.Path = DEFAULT_VOCABULARY) -> Vocabulary:
    try:
        lines = path.read_text(encoding="ascii", errors="strict").splitlines()
    except (OSError, UnicodeError) as error:
        raise ScanError(f"cannot read trust vocabulary: {error}") from error
    fields: dict[str, list[str]] = {}
    for line_number, line in enumerate(lines, 1):
        if not line or line.startswith("#"):
            continue
        if "=" not in line:
            raise ScanError(f"malformed trust vocabulary line {line_number}")
        name, value = line.split("=", 1)
        normalized_name = name.replace("-", "")
        if (
            not name
            or not value
            or not normalized_name.isascii()
            or not normalized_name.isalnum()
        ):
            raise ScanError(f"malformed trust vocabulary field at line {line_number}")
        fields.setdefault(name, []).append(value)

    if one_value(fields, "format") != VOCABULARY_FORMAT:
        raise ScanError("unsupported trust vocabulary format")
    version = one_value(fields, "version")
    upstream_commit = one_value(fields, "upstream-commit")
    if len(upstream_commit) != 40 or any(
        character not in "0123456789abcdef" for character in upstream_commit
    ):
        raise ScanError("trust vocabulary has an invalid upstream commit")
    parser_tokens = frozenset(sorted_unique_values(fields, "parser-token"))
    parser_decisions = frozenset(sorted_unique_values(fields, "parser-decision"))
    trust_tokens = frozenset(sorted_unique_values(fields, "trust-token"))
    defensive_tokens = frozenset(sorted_unique_values(fields, "defensive-token"))
    if not parser_decisions <= parser_tokens:
        raise ScanError("parser decisions are not covered by the conservative parser vocabulary")
    if trust_tokens != REQUIRED_TRUST_TOKENS:
        raise ScanError("trust vocabulary does not match the reviewed trust-token set")
    if defensive_tokens & trust_tokens:
        raise ScanError("defensive and trust-token vocabularies must be disjoint")
    non_parser_trust_tokens = {"admit", "assume_specification", "axiom", "uninterp"}
    if not trust_tokens <= parser_tokens | non_parser_trust_tokens:
        raise ScanError("trust vocabulary contains a token outside the audited parser vocabulary")

    release_roots = sorted_unique_values(fields, "release-root")
    for relative in release_roots:
        pure_relative = pathlib.PurePosixPath(relative)
        if pure_relative.is_absolute() or pure_relative == pathlib.PurePosixPath("."):
            raise ScanError("unsafe release-root trust vocabulary path")
        if ".." in pure_relative.parts:
            raise ScanError("unsafe release-root trust vocabulary path")
    release_sources = []
    for record in sorted_unique_values(fields, "release-source"):
        parts = record.split("|")
        if len(parts) != 3:
            raise ScanError("malformed release-source trust vocabulary record")
        relative, byte_length, digest = parts
        pure_relative = pathlib.PurePosixPath(relative)
        if (
            pure_relative.is_absolute()
            or pure_relative == pathlib.PurePosixPath(".")
            or ".." in pure_relative.parts
        ):
            raise ScanError("unsafe release-source trust vocabulary path")
        if not byte_length.isdigit() or int(byte_length) <= 0:
            raise ScanError("invalid release-source byte length")
        if len(digest) != 64 or any(character not in "0123456789abcdef" for character in digest):
            raise ScanError("invalid release-source SHA-256")
        release_sources.append((relative, int(byte_length), digest))

    allowed_fields = {
        "defensive-token",
        "format",
        "parser-token",
        "parser-decision",
        "release-root",
        "release-source",
        "trust-token",
        "upstream-commit",
        "upstream-source",
        "version",
    }
    unexpected = set(fields) - allowed_fields
    if unexpected:
        raise ScanError(f"unknown trust vocabulary fields: {sorted(unexpected)!r}")
    upstream_source = one_value(fields, "upstream-source").split("|")
    if (
        len(upstream_source) != 3
        or upstream_source[0] != "source/rust_verify/src/attributes.rs"
        or not upstream_source[1].isdigit()
        or len(upstream_source[2]) != 64
        or any(character not in "0123456789abcdef" for character in upstream_source[2])
    ):
        raise ScanError("invalid audited upstream attribute-parser source record")
    return Vocabulary(
        version,
        upstream_commit,
        parser_tokens,
        parser_decisions,
        trust_tokens,
        defensive_tokens,
        release_roots,
        tuple(release_sources),
        (upstream_source[0], int(upstream_source[1]), upstream_source[2]),
    )


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


def validate_balanced_closed_source(tokens: list[Token]) -> None:
    values = [token.value for token in tokens]
    for token in tokens:
        if token.value == "#":
            raise ScanError(f"source attributes are forbidden at {token.line}:{token.column}")
        if token.value in SOURCE_INJECTION_IDENTIFIERS:
            raise ScanError(
                f"source-injection token {token.value!r} is forbidden "
                f"at {token.line}:{token.column}"
            )

    macro_invocations = []
    for index in range(len(tokens) - 2):
        if (
            identifier_start(tokens[index].value[0])
            and tokens[index + 1].value == "!"
            and tokens[index + 2].value in {"(", "[", "{"}
        ):
            macro_invocations.append((tokens[index].value, index))
    if macro_invocations != [("verus", 7)]:
        raise ScanError("the only allowed code-generating macro is one enclosing verus! block")

    if values[: len(CLOSED_SOURCE_PREFIX)] != CLOSED_SOURCE_PREFIX:
        raise ScanError("proof must start with the exact vstd prelude import and one verus! block")
    if values.count("use") != 1:
        raise ScanError("proof may contain only the exact vstd prelude import")

    opening_index = len(CLOSED_SOURCE_PREFIX) - 1
    pairs = {")": "(", "]": "[", "}": "{"}
    stack: list[tuple[str, int]] = []
    outer_closing = None
    for index in range(opening_index, len(tokens)):
        value = tokens[index].value
        if value in {"(", "[", "{"}:
            stack.append((value, index))
        elif value in pairs:
            if not stack or stack[-1][0] != pairs[value]:
                raise ScanError(
                    f"unbalanced delimiter {value!r} at "
                    f"{tokens[index].line}:{tokens[index].column}"
                )
            _, matched_index = stack.pop()
            if matched_index == opening_index:
                outer_closing = index
    if stack or outer_closing != len(tokens) - 1:
        raise ScanError("the enclosing verus! block must be balanced and end the source")


def validate_exp_declaration(tokens: list[Token], exp_policy: str) -> None:
    values = [token.value for token in tokens]
    uninterp_indices = [index for index, value in enumerate(values) if value == "uninterp"]
    exp_fn_indices = [
        index
        for index in range(len(values) - 1)
        if values[index] == "fn" and values[index + 1] == "exp_real_v1"
    ]
    if exp_policy == "forbid":
        if uninterp_indices or exp_fn_indices:
            raise ScanError("this proof mutation may not declare an uninterpreted function")
        return

    if len(uninterp_indices) != 1:
        raise ScanError("exactly one approved exp_real_v1 uninterpreted declaration is required")
    index = uninterp_indices[0]
    start = index - 1
    declaration = values[start : start + len(APPROVED_EXP_DECLARATION)]
    if start < 0 or declaration != APPROVED_EXP_DECLARATION:
        token = tokens[index]
        raise ScanError(
            f"unapproved uninterpreted declaration at {token.line}:{token.column}"
        )
    depth = 0
    for value in values[:start]:
        if value in {"(", "[", "{"}:
            depth += 1
        elif value in {")", "]", "}"}:
            depth -= 1
    if depth != 1:
        raise ScanError("exp_real_v1 must be a direct item in the enclosing verus! block")
    if len(exp_fn_indices) != 1 or exp_fn_indices[0] != index + 2:
        raise ScanError("exp_real_v1 must have exactly one function declaration")


def validate(tokens: list[Token], vocabulary: Vocabulary, exp_policy: str) -> None:
    for token in tokens:
        if token.value in (vocabulary.trust_tokens | vocabulary.defensive_tokens) - {"uninterp"}:
            raise ScanError(
                f"forbidden proof token {token.value!r} at {token.line}:{token.column}"
            )
    validate_balanced_closed_source(tokens)
    validate_exp_declaration(tokens, exp_policy)


def scan(path: pathlib.Path, vocabulary: Vocabulary, exp_policy: str) -> None:
    if path.is_symlink() or not path.is_file():
        raise ScanError("proof source is not a direct regular file")
    try:
        source = path.read_text(encoding="utf-8", errors="strict")
    except (OSError, UnicodeError) as error:
        raise ScanError(f"cannot read UTF-8 proof source: {error}") from error
    validate(lex(source), vocabulary, exp_policy)


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def audit_verus_root(root: pathlib.Path, vocabulary: Vocabulary) -> None:
    if root.is_symlink() or not root.is_dir():
        raise ScanError("Verus audit root is not a direct directory")
    version_file = root / "version.txt"
    try:
        version = version_file.read_text(encoding="ascii", errors="strict").splitlines()[0]
    except (OSError, UnicodeError, IndexError) as error:
        raise ScanError(f"cannot read audited Verus version: {error}") from error
    if version != vocabulary.version:
        raise ScanError("audited Verus version does not match the trust vocabulary")

    for relative, byte_length, expected_digest in vocabulary.release_sources:
        path = root / relative
        if path.is_symlink() or not path.is_file():
            raise ScanError(f"audited Verus source is not a direct regular file: {relative}")
        if path.stat().st_size != byte_length or sha256(path) != expected_digest:
            raise ScanError(f"audited Verus source drifted: {relative}")

    identifiers = set()
    observed_attributes = set()
    observed_trust_tokens = set()
    for relative_root in vocabulary.release_roots:
        source_root = root / relative_root
        if source_root.is_symlink() or not source_root.is_dir():
            raise ScanError(f"audited Verus source root is invalid: {relative_root}")
        for path in sorted(source_root.rglob("*.rs")):
            if path.is_symlink() or not path.is_file():
                raise ScanError(f"audited Verus source entry is invalid: {path}")
            try:
                source = path.read_text(encoding="utf-8", errors="strict")
                tokens = lex(source)
            except (OSError, UnicodeError) as error:
                raise ScanError(f"cannot read audited Verus source {path}: {error}") from error
            observed_trust_tokens.update(
                token
                for token in vocabulary.trust_tokens
                if re.search(rf"\b{re.escape(token)}\b", source)
            )
            values = [token.value for token in tokens]
            identifiers.update(value for value in values if value and identifier_start(value[0]))
            for index in range(len(values) - 2):
                candidate = values[index + 2]
                if (
                    values[index] == "verifier"
                    and values[index + 1] == "::"
                    and candidate
                    and identifier_start(candidate[0])
                ):
                    observed_attributes.add(candidate)

    unknown = observed_attributes - vocabulary.parser_tokens
    if unknown:
        raise ScanError(f"pinned Verus exposes unaudited verifier attributes: {sorted(unknown)!r}")
    rust_verify = root / "rust_verify"
    if rust_verify.is_symlink() or not rust_verify.is_file():
        raise ScanError("audited rust_verify is not a direct regular file")
    try:
        rust_verify_bytes = rust_verify.read_bytes()
    except OSError as error:
        raise ScanError(f"cannot read audited rust_verify: {error}") from error
    absent = {
        token
        for token in vocabulary.trust_tokens
        if token not in identifiers
        and token not in observed_trust_tokens
        and token.encode("ascii") not in rust_verify_bytes
    }
    if absent:
        raise ScanError(
            f"trust tokens are absent from the pinned Verus closure: {sorted(absent)!r}"
        )


def audit_parser_source(path: pathlib.Path, vocabulary: Vocabulary) -> None:
    relative, byte_length, expected_digest = vocabulary.upstream_source
    if path.is_symlink() or not path.is_file():
        raise ScanError("audited upstream Verus parser source is not a direct regular file")
    if path.stat().st_size != byte_length or sha256(path) != expected_digest:
        raise ScanError(f"audited upstream Verus parser source drifted: {relative}")
    try:
        source = path.read_text(encoding="utf-8", errors="strict")
        start = source.index("AttrPrefix::Verifier =>")
        end = source.index("AttrPrefix::Verus(verus_prefix)", start)
    except (OSError, UnicodeError, ValueError) as error:
        raise ScanError(f"cannot inspect audited upstream Verus parser source: {error}") from error
    decisions = frozenset(
        re.findall(r'(?:name|arg) == "([a-z_][a-z0-9_]*)"', source[start:end])
    )
    if decisions != vocabulary.parser_decisions:
        missing = sorted(vocabulary.parser_decisions - decisions)
        unknown = sorted(decisions - vocabulary.parser_decisions)
        raise ScanError(
            f"upstream Verus parser decision vocabulary drifted: "
            f"missing={missing!r}, unknown={unknown!r}"
        )


def main(arguments: list[str]) -> int:
    usage = (
        f"usage: {pathlib.Path(sys.argv[0]).name} "
        "[--require-exp-real|--forbid-uninterp] PROOF [PROOF ...]\n"
        f"       {pathlib.Path(sys.argv[0]).name} --audit-verus-root ROOT\n"
        f"       {pathlib.Path(sys.argv[0]).name} --audit-parser-source SOURCE"
    )
    try:
        vocabulary = load_vocabulary()
    except ScanError as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1
    if len(arguments) == 2 and arguments[0] == "--audit-verus-root":
        try:
            audit_verus_root(pathlib.Path(arguments[1]), vocabulary)
        except ScanError as error:
            print(f"FAIL: {error}", file=sys.stderr)
            return 1
        return 0
    if len(arguments) == 2 and arguments[0] == "--audit-parser-source":
        try:
            audit_parser_source(pathlib.Path(arguments[1]), vocabulary)
        except ScanError as error:
            print(f"FAIL: {error}", file=sys.stderr)
            return 1
        return 0
    exp_policy = "require"
    if arguments and arguments[0] in {"--require-exp-real", "--forbid-uninterp"}:
        exp_policy = "require" if arguments.pop(0) == "--require-exp-real" else "forbid"
    if not arguments or arguments[0].startswith("--"):
        print(usage, file=sys.stderr)
        return 2
    for argument in arguments:
        path = pathlib.Path(argument)
        try:
            scan(path, vocabulary, exp_policy)
        except ScanError as error:
            print(f"FAIL: {path}: {error}", file=sys.stderr)
            return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
