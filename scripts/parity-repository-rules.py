#!/usr/bin/env python3
"""Render and verify the repository rules required by parity promotion."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import sys
from typing import Any


RULESET_NAME = "fe2o3-production-parity"
CHECK_CONTEXTS = ("Generic validation", "Protected signed-evidence gate")
WORKFLOW_PATHS = (
    ".github/workflows/ci.yml",
    ".github/workflows/parity-promotion.yml",
)


class RuleError(Exception):
    pass


def fail(message: str) -> None:
    raise RuleError(message)


def positive_id(value: str, label: str) -> int:
    if not re.fullmatch(r"[1-9][0-9]*", value):
        fail(f"invalid {label}")
    return int(value)


def expected_ruleset(
    repository_id: int, actions_integration_id: int, default_branch: str
) -> dict[str, Any]:
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._/-]{0,254}", default_branch):
        fail("invalid default branch")
    workflow_ref = f"refs/heads/{default_branch}"
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
            {
                "type": "merge_queue",
                "parameters": {
                    "check_response_timeout_minutes": 30,
                    "grouping_strategy": "ALLGREEN",
                    "max_entries_to_build": 5,
                    "max_entries_to_merge": 1,
                    "merge_method": "SQUASH",
                    "min_entries_to_merge": 1,
                    "min_entries_to_merge_wait_minutes": 0,
                },
            },
            {
                "type": "workflows",
                "parameters": {
                    "do_not_enforce_on_create": False,
                    "workflows": [
                        {
                            "path": path,
                            "ref": workflow_ref,
                            "repository_id": repository_id,
                        }
                        for path in WORKFLOW_PATHS
                    ],
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


def verify_ruleset(
    document: dict[str, Any],
    repository_id: int,
    actions_integration_id: int,
    default_branch: str,
) -> None:
    if document.get("name") != RULESET_NAME:
        fail("production parity ruleset name mismatch")
    if document.get("target") != "branch" or document.get("enforcement") != "active":
        fail("production parity ruleset is not active on branches")
    if document.get("bypass_actors") != []:
        fail("production parity ruleset must have no bypass actors")
    conditions = require_mapping(document.get("conditions"), "ruleset conditions")
    ref_name = require_mapping(conditions.get("ref_name"), "ref-name condition")
    if ref_name.get("include") != ["~DEFAULT_BRANCH"] or ref_name.get("exclude") != []:
        fail("production parity ruleset does not target only the default branch")

    rules = rule_map(document)
    required_types = {
        "deletion",
        "non_fast_forward",
        "pull_request",
        "required_status_checks",
        "merge_queue",
        "workflows",
    }
    if not required_types <= set(rules):
        fail("production parity ruleset is missing a required rule")

    pull_request = require_mapping(
        rules["pull_request"].get("parameters"), "pull-request parameters"
    )
    if (
        pull_request.get("allowed_merge_methods") != ["squash"]
        or pull_request.get("dismiss_stale_reviews_on_push") is not True
        or pull_request.get("require_code_owner_review") is not True
        or pull_request.get("require_last_push_approval") is not True
        or not isinstance(pull_request.get("required_approving_review_count"), int)
        or pull_request["required_approving_review_count"] < 1
        or pull_request.get("required_review_thread_resolution") is not True
    ):
        fail("pull-request review enforcement is weaker than production policy")

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
        fail("required status checks are not strict and source-pinned")

    merge_queue = require_mapping(
        rules["merge_queue"].get("parameters"), "merge-queue parameters"
    )
    if (
        merge_queue.get("grouping_strategy") != "ALLGREEN"
        or merge_queue.get("max_entries_to_merge") != 1
        or merge_queue.get("merge_method") != "SQUASH"
        or merge_queue.get("min_entries_to_merge") != 1
    ):
        fail("merge queue does not enforce one fully checked PR per group")

    workflows = require_mapping(
        rules["workflows"].get("parameters"), "required-workflow parameters"
    )
    actual_workflows = require_list(workflows.get("workflows"), "required workflows")
    workflow_ref = f"refs/heads/{default_branch}"
    expected_workflows = [
        {"path": path, "ref": workflow_ref, "repository_id": repository_id}
        for path in WORKFLOW_PATHS
    ]
    if (
        workflows.get("do_not_enforce_on_create") is not False
        or actual_workflows != expected_workflows
    ):
        fail("required workflows are not pinned to the protected default branch")


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
        command.add_argument("--repository-id", required=True)
        command.add_argument("--actions-integration-id", required=True)
        command.add_argument("--default-branch", required=True)
        if name == "verify":
            command.add_argument("ruleset", type=Path)
    return parser


def main() -> None:
    args = build_parser().parse_args()
    repository_id = positive_id(args.repository_id, "repository ID")
    integration_id = positive_id(args.actions_integration_id, "Actions integration ID")
    if args.command == "render":
        print(
            json.dumps(
                expected_ruleset(repository_id, integration_id, args.default_branch),
                sort_keys=True,
                separators=(",", ":"),
            )
        )
        return
    document = read_document(args.ruleset)
    verify_ruleset(document, repository_id, integration_id, args.default_branch)
    print("production parity repository rules are enforceable")


if __name__ == "__main__":
    try:
        main()
    except RuleError as error:
        print(f"parity repository rules: {error}", file=sys.stderr)
        raise SystemExit(2)
