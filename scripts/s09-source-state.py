#!/usr/bin/env python3
"""Require and report one exact clean Git source state for S09 evidence."""

from __future__ import annotations

import argparse
import pathlib
import re
import subprocess
import sys


GIT_OBJECT = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})")


class SourceStateError(Exception):
    pass


def git(root: pathlib.Path, *arguments: str) -> bytes:
    try:
        completed = subprocess.run(
            ["git", "-C", str(root), *arguments],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
    except OSError as error:
        raise SourceStateError(f"cannot execute git: {error}") from error
    if completed.returncode != 0:
        detail = completed.stderr[:4096].decode("utf-8", "replace").strip()
        raise SourceStateError(f"git {' '.join(arguments)} failed: {detail}")
    return completed.stdout


def decode_object(value: bytes, label: str) -> str:
    try:
        decoded = value.decode("ascii").strip()
    except UnicodeDecodeError as error:
        raise SourceStateError(f"{label} is not ASCII") from error
    if not GIT_OBJECT.fullmatch(decoded) or set(decoded) == {"0"}:
        raise SourceStateError(f"{label} is not a canonical Git object ID")
    return decoded


def inspect(root: pathlib.Path) -> tuple[str, str]:
    try:
        canonical_root = root.resolve(strict=True)
    except OSError as error:
        raise SourceStateError(f"source root cannot be resolved: {error}") from error
    top_level = pathlib.Path(
        git(canonical_root, "rev-parse", "--show-toplevel").decode("utf-8").strip()
    ).resolve(strict=True)
    if top_level != canonical_root:
        raise SourceStateError("source root is not the exact Git worktree root")
    status = git(
        canonical_root,
        "status",
        "--porcelain=v1",
        "-z",
        "--untracked-files=all",
    )
    if status:
        raise SourceStateError(
            "source worktree must be exactly clean before evidence generation"
        )
    commit = decode_object(
        git(canonical_root, "rev-parse", "--verify", "HEAD"), "source commit"
    )
    tree = decode_object(
        git(canonical_root, "rev-parse", "--verify", "HEAD^{tree}"), "source tree"
    )
    return commit, tree


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True, type=pathlib.Path)
    parser.add_argument("--expected-commit")
    parser.add_argument("--expected-tree")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if (args.expected_commit is None) != (args.expected_tree is None):
        raise SourceStateError("expected commit and tree must be supplied together")
    commit, tree = inspect(args.root)
    if args.expected_commit is not None:
        if commit != args.expected_commit or tree != args.expected_tree:
            raise SourceStateError(
                "source HEAD or tree changed during evidence generation"
            )
    print(f"source_commit\t{commit}")
    print(f"source_tree\t{tree}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except SourceStateError as error:
        print(f"s09-source-state: {error}", file=sys.stderr)
        raise SystemExit(2)
