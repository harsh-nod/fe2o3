#!/usr/bin/env python3
"""Render and verify the user-owned-compatible canonical branch ruleset."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import sys
from typing import Any


RULESET_NAME = "fe2o3-canonical-default-branch"
GITHUB_ACTIONS_INTEGRATION_ID = 15368
CHECK_CONTEXTS = (
    "Fork-safe preflight",
    "Generic parity policy gate",
    "Generic validation",
)


class RuleError(Exception):
    pass


def fail(message: str) -> None:
    raise RuleError(message)


def positive_id(value: str, label: str) -> int:
    if not re.fullmatch(r"[1-9][0-9]*", value):
        fail(f"invalid {label}")
    return int(value)


def expected_ruleset(actions_integration_id: int) -> dict[str, Any]:
    return {
        "name": RULESET_NAME,
        "target": "branch",
        "enforcement": "active",
        "bypass_actors": [],
        "conditions": {
            "ref_name": {"include": ["~DEFAULT_BRANCH"], "exclude": []}
        },
        "rules": [
            {"type": "deletion"},
            {"type": "non_fast_forward"},
            {
                "type": "pull_request",
                "parameters": {
                    "allowed_merge_methods": ["squash"],
                    "dismiss_stale_reviews_on_push": True,
                    "require_code_owner_review": True,
                    "require_last_push_approval": True,
                    "required_approving_review_count": 1,
                    "required_review_thread_resolution": True,
                },
            },
            {
                "type": "required_status_checks",
                "parameters": {
                    "do_not_enforce_on_create": False,
                    "required_status_checks": [
                        {
                            "context": context,
                            "integration_id": actions_integration_id,
                        }
                        for context in CHECK_CONTEXTS
                    ],
                    "strict_required_status_checks_policy": True,
                },
            },
        ],
    }


def require_mapping(value: object, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        fail(f"{label} is not an object")
    return value


def require_list(value: object, label: str) -> list[Any]:
    if not isinstance(value, list):
        fail(f"{label} is not an array")
    return value


def rule_map(document: dict[str, Any]) -> dict[str, dict[str, Any]]:
    output: dict[str, dict[str, Any]] = {}
    for value in require_list(document.get("rules"), "ruleset rules"):
        rule = require_mapping(value, "ruleset rule")
        kind = rule.get("type")
        if not isinstance(kind, str) or kind in output:
            fail("ruleset has a missing or duplicate rule type")
        output[kind] = rule
    return output


def verify_ruleset(document: dict[str, Any], actions_integration_id: int) -> None:
    if document.get("name") != RULESET_NAME:
        fail("canonical ruleset name mismatch")
    if document.get("target") != "branch" or document.get("enforcement") != "active":
        fail("canonical ruleset is not active on branches")
    if document.get("bypass_actors") != []:
        fail("canonical ruleset must have no bypass actors")

    expected_conditions = {
        "ref_name": {"include": ["~DEFAULT_BRANCH"], "exclude": []}
    }
    if document.get("conditions") != expected_conditions:
        fail("canonical ruleset does not target only the default branch")

    rules = rule_map(document)
    required_types = {
        "deletion",
        "non_fast_forward",
        "pull_request",
        "required_status_checks",
    }
    if set(rules) != required_types:
        fail("canonical ruleset does not have the exact admitted rule types")

    pull_request = require_mapping(
        rules["pull_request"].get("parameters"), "pull-request parameters"
    )
    approval_count = pull_request.get("required_approving_review_count")
    if (
        pull_request.get("allowed_merge_methods") != ["squash"]
        or pull_request.get("dismiss_stale_reviews_on_push") is not True
        or pull_request.get("require_code_owner_review") is not True
        or pull_request.get("require_last_push_approval") is not True
        or type(approval_count) is not int
        or approval_count != 1
        or pull_request.get("required_review_thread_resolution") is not True
    ):
        fail("pull-request review enforcement does not match canonical policy")

    statuses = require_mapping(
        rules["required_status_checks"].get("parameters"),
        "required-status parameters",
    )
    actual_statuses = require_list(
        statuses.get("required_status_checks"), "required status checks"
    )
    expected_statuses = [
        {"context": context, "integration_id": actions_integration_id}
        for context in CHECK_CONTEXTS
    ]
    if (
        statuses.get("strict_required_status_checks_policy") is not True
        or statuses.get("do_not_enforce_on_create") is not False
        or actual_statuses != expected_statuses
    ):
        fail("required status checks are not strict and GitHub-Actions-pinned")


def read_document(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        fail(f"cannot read ruleset JSON: {error}")
    return require_mapping(value, "ruleset")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    for name in ("render", "verify"):
        command = subparsers.add_parser(name)
        command.add_argument("--actions-integration-id", required=True)
        if name == "verify":
            command.add_argument("ruleset", type=Path)
    return parser


def main() -> None:
    args = build_parser().parse_args()
    integration_id = positive_id(args.actions_integration_id, "Actions integration ID")
    if integration_id != GITHUB_ACTIONS_INTEGRATION_ID:
        fail("Actions integration ID does not identify GitHub Actions on github.com")
    if args.command == "render":
        print(
            json.dumps(
                expected_ruleset(integration_id),
                sort_keys=True,
                separators=(",", ":"),
            )
        )
        return
    document = read_document(args.ruleset)
    verify_ruleset(document, integration_id)
    print("canonical repository rules are enforceable")


if __name__ == "__main__":
    try:
        main()
    except RuleError as error:
        print(f"canonical repository rules: {error}", file=sys.stderr)
        raise SystemExit(2)
