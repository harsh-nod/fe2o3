#!/usr/bin/env python3
"""Require exact DCO sign-offs across a bounded Git commit range."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from typing import Any


COMMIT_PATTERN = re.compile(r"[0-9a-f]{40}")
REPOSITORY_PATTERN = re.compile(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+")
DEPENDABOT_NAME = "dependabot[bot]"
DEPENDABOT_EMAIL = "49699333+dependabot[bot]@users.noreply.github.com"
DEPENDABOT_SIGNOFF = "Signed-off-by: dependabot[bot] <support@github.com>"
MAX_COMMITS = 1024


class DcoError(Exception):
    pass


def run(*arguments: str) -> bytes:
    try:
        return subprocess.check_output(arguments, stderr=subprocess.PIPE)
    except (OSError, subprocess.CalledProcessError) as error:
        raise DcoError(f"command failed: {' '.join(arguments)}: {error}") from error


def mapping(value: object, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise DcoError(f"Dependabot API response lacks {label}")
    return value


def verified_dependabot(commit: str, author: str, email: str, body: str, repo: str) -> bool:
    if (
        author != DEPENDABOT_NAME
        or email != DEPENDABOT_EMAIL
        or DEPENDABOT_SIGNOFF not in body.splitlines()
    ):
        return False
    try:
        document = json.loads(
            run(
                "gh",
                "api",
                "-H",
                "Accept: application/vnd.github+json",
                "-H",
                "X-GitHub-Api-Version: 2026-03-10",
                f"repos/{repo}/commits/{commit}",
            )
        )
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        raise DcoError(f"{commit}: invalid Dependabot API response: {error}") from error
    document = mapping(document, "document")
    commit_record = mapping(document.get("commit"), "commit record")
    api_author = mapping(document.get("author"), "author account")
    api_committer = mapping(document.get("committer"), "committer account")
    git_author = mapping(commit_record.get("author"), "Git author")
    git_committer = mapping(commit_record.get("committer"), "Git committer")
    verification = mapping(commit_record.get("verification"), "verification")
    return (
        document.get("sha") == commit
        and api_author.get("login") == DEPENDABOT_NAME
        and api_committer.get("login") == "web-flow"
        and git_author.get("name") == DEPENDABOT_NAME
        and git_author.get("email") == DEPENDABOT_EMAIL
        and git_committer.get("name") == "GitHub"
        and git_committer.get("email") == "noreply@github.com"
        and verification.get("verified") is True
        and verification.get("reason") == "valid"
    )


def commits_in_range(base: str, head: str) -> list[str]:
    raw = run("git", "rev-list", "--reverse", f"{base}..{head}")
    try:
        commits = raw.decode("ascii").splitlines()
    except UnicodeDecodeError as error:
        raise DcoError("commit range is not ASCII") from error
    if not commits:
        raise DcoError("pull request has an empty commit range")
    if len(commits) > MAX_COMMITS:
        raise DcoError(f"pull request exceeds the {MAX_COMMITS}-commit bound")
    if any(COMMIT_PATTERN.fullmatch(commit) is None for commit in commits):
        raise DcoError("commit range contains a noncanonical identity")
    return commits


def commit_identity(commit: str) -> tuple[str, str, str]:
    raw = run("git", "show", "-s", "--format=%an%x00%ae%x00%B", commit)
    try:
        return tuple(raw.decode("utf-8").split("\0", 2))  # type: ignore[return-value]
    except (UnicodeDecodeError, ValueError) as error:
        raise DcoError(f"{commit}: commit identity is not canonical UTF-8") from error


def check(base: str, head: str, repo: str) -> int:
    if COMMIT_PATTERN.fullmatch(base) is None or COMMIT_PATTERN.fullmatch(head) is None:
        raise DcoError("invalid pull-request commit identity")
    if REPOSITORY_PATTERN.fullmatch(repo) is None:
        raise DcoError("invalid GitHub repository identity")
    missing = []
    commits = commits_in_range(base, head)
    for commit in commits:
        author, email, body = commit_identity(commit)
        expected = f"Signed-off-by: {author} <{email}>"
        if expected in body.splitlines():
            continue
        if verified_dependabot(commit, author, email, body, repo):
            continue
        missing.append(f"{commit}: missing exact trailer {expected}")
    if missing:
        raise DcoError("\n".join(missing))
    print(f"DCO sign-off present on {len(commits)} commit(s)")
    return len(commits)


def main() -> None:
    if not sys.flags.isolated:
        raise DcoError("checker must run in Python isolated mode (-I)")
    parser = argparse.ArgumentParser()
    parser.add_argument("--base", required=True)
    parser.add_argument("--head", required=True)
    parser.add_argument("--repo", required=True)
    args = parser.parse_args()
    check(args.base, args.head, args.repo)


if __name__ == "__main__":
    try:
        main()
    except DcoError as error:
        print(f"DCO check: {error}", file=sys.stderr)
        raise SystemExit(1)
