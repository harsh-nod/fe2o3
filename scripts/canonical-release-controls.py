#!/usr/bin/env python3
"""Render and verify canonical release tag and environment controls."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import sys
from typing import Any


TAG_CREATION_NAME = "fe2o3-release-tag-creation"
TAG_IMMUTABILITY_NAME = "fe2o3-release-tag-immutability"
RELEASE_ENVIRONMENT = "release"
RELEASE_TAG_PATTERN = "refs/tags/v*"
RELEASE_USER_ID = 3144552
REVIEWER_USER_ID = 74956
CONTROLS = (
    "tag-creation",
    "tag-guard",
    "tag-immutability",
    "environment",
    "environment-policy",
)


class ControlError(Exception):
    pass


def fail(message: str) -> None:
    raise ControlError(message)


def positive_id(value: str, label: str) -> int:
    if not re.fullmatch(r"[1-9][0-9]*", value):
        fail(f"invalid {label}")
    return int(value)


def validate_identity_ids(release_user_id: int, reviewer_user_id: int) -> None:
    if release_user_id != RELEASE_USER_ID:
        fail("release user ID does not identify harsh-nod on github.com")
    if reviewer_user_id != REVIEWER_USER_ID:
        fail("reviewer user ID does not identify powderluv on github.com")


def tag_conditions() -> dict[str, Any]:
    return {
        "ref_name": {"include": [RELEASE_TAG_PATTERN], "exclude": []}
    }


def tag_creation(release_user_id: int) -> dict[str, Any]:
    return {
        "name": TAG_CREATION_NAME,
        "target": "tag",
        "enforcement": "active",
        "bypass_actors": [
            {
                "actor_id": release_user_id,
                "actor_type": "User",
                "bypass_mode": "always",
            }
        ],
        "conditions": tag_conditions(),
        "rules": [{"type": "creation"}],
    }


def tag_immutability() -> dict[str, Any]:
    return {
        "name": TAG_IMMUTABILITY_NAME,
        "target": "tag",
        "enforcement": "active",
        "bypass_actors": [],
        "conditions": tag_conditions(),
        "rules": [
            {
                "type": "update",
                "parameters": {"update_allows_fetch_and_merge": False},
            },
            {"type": "deletion"},
        ],
    }


def tag_guard() -> dict[str, Any]:
    document = tag_immutability()
    document["rules"] = [{"type": "creation"}, *document["rules"]]
    return document


def environment(reviewer_user_id: int) -> dict[str, Any]:
    return {
        "wait_timer": 0,
        "prevent_self_review": True,
        "can_admins_bypass": False,
        "reviewers": [{"type": "User", "id": reviewer_user_id}],
        "deployment_branch_policy": {
            "protected_branches": False,
            "custom_branch_policies": True,
        },
    }


def environment_policy() -> dict[str, Any]:
    return {"name": "main", "type": "branch"}


def require_mapping(value: object, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        fail(f"{label} is not an object")
    return value


def require_list(value: object, label: str) -> list[Any]:
    if not isinstance(value, list):
        fail(f"{label} is not an array")
    return value


def verify_tag_common(
    document: dict[str, Any], name: str, expected_bypass: list[dict[str, Any]]
) -> dict[str, dict[str, Any]]:
    if document.get("name") != name:
        fail("release tag ruleset name mismatch")
    if document.get("target") != "tag" or document.get("enforcement") != "active":
        fail("release tag ruleset is not active on tags")
    if document.get("conditions") != tag_conditions():
        fail("release tag ruleset does not target refs/tags/v*")
    if document.get("bypass_actors") != expected_bypass:
        fail("release tag ruleset bypass actors do not match policy")

    rules: dict[str, dict[str, Any]] = {}
    for value in require_list(document.get("rules"), "release tag rules"):
        rule = require_mapping(value, "release tag rule")
        kind = rule.get("type")
        if not isinstance(kind, str) or kind in rules:
            fail("release tag ruleset has a missing or duplicate rule type")
        rules[kind] = rule
    return rules


def verify_tag_creation(document: dict[str, Any], release_user_id: int) -> None:
    expected_bypass = [
        {
            "actor_id": release_user_id,
            "actor_type": "User",
            "bypass_mode": "always",
        }
    ]
    rules = verify_tag_common(document, TAG_CREATION_NAME, expected_bypass)
    if rules != {"creation": {"type": "creation"}}:
        fail("release tag creation ruleset must contain only creation")


def verify_tag_immutability(document: dict[str, Any]) -> None:
    rules = verify_tag_common(document, TAG_IMMUTABILITY_NAME, [])
    if set(rules) != {"update", "deletion"}:
        fail("release tag immutability ruleset must contain update and deletion")
    if rules["deletion"] != {"type": "deletion"}:
        fail("release tag deletion rule has unexpected parameters")
    if rules["update"] != {
        "type": "update",
        "parameters": {"update_allows_fetch_and_merge": False},
    }:
        fail("release tag update rule does not prohibit all updates")


def verify_tag_guard(document: dict[str, Any]) -> None:
    rules = verify_tag_common(document, TAG_IMMUTABILITY_NAME, [])
    if set(rules) != {"creation", "update", "deletion"}:
        fail("release tag bootstrap guard must lock creation, update, and deletion")
    if rules["creation"] != {"type": "creation"}:
        fail("release tag bootstrap creation guard has unexpected parameters")
    if rules["deletion"] != {"type": "deletion"}:
        fail("release tag deletion rule has unexpected parameters")
    if rules["update"] != {
        "type": "update",
        "parameters": {"update_allows_fetch_and_merge": False},
    }:
        fail("release tag update rule does not prohibit all updates")


def reviewer_identity(value: object) -> tuple[object, object]:
    reviewer = require_mapping(value, "release environment reviewer")
    if "reviewer" in reviewer:
        identity = require_mapping(reviewer.get("reviewer"), "reviewer identity")
        if identity.get("login") != "powderluv":
            fail("release environment reviewer login does not match powderluv")
        return reviewer.get("type"), identity.get("id")
    return reviewer.get("type"), reviewer.get("id")


def verify_environment(document: dict[str, Any], reviewer_user_id: int) -> None:
    expected_branch_policy = {
        "protected_branches": False,
        "custom_branch_policies": True,
    }
    if document.get("deployment_branch_policy") != expected_branch_policy:
        fail("release environment must use an exact custom main-branch policy")
    if document.get("can_admins_bypass") is not False:
        fail("release environment must prohibit administrator bypass")

    if "protection_rules" not in document:
        if document.get("wait_timer") != 0:
            fail("release environment wait timer must be zero")
        if document.get("prevent_self_review") is not True:
            fail("release environment must prevent self review")
        reviewers = require_list(document.get("reviewers"), "environment reviewers")
    else:
        if document.get("name") != RELEASE_ENVIRONMENT:
            fail("release environment name mismatch")
        protection_rules = require_list(
            document.get("protection_rules"), "environment protection rules"
        )
        types = [
            require_mapping(rule, "environment protection rule").get("type")
            for rule in protection_rules
        ]
        if (
            types.count("required_reviewers") != 1
            or types.count("branch_policy") != 1
            or types.count("wait_timer") > 1
            or any(
                kind not in {"required_reviewers", "branch_policy", "wait_timer"}
                for kind in types
            )
        ):
            fail("release environment protection rules do not match policy")
        for rule in protection_rules:
            mapped = require_mapping(rule, "environment protection rule")
            if mapped.get("type") == "wait_timer" and mapped.get("wait_timer") != 0:
                fail("release environment wait timer must be zero")
        required = next(
            require_mapping(rule, "required-reviewers rule")
            for rule in protection_rules
            if require_mapping(rule, "environment protection rule").get("type")
            == "required_reviewers"
        )
        if required.get("prevent_self_review") is not True:
            fail("release environment must prevent self review")
        reviewers = require_list(required.get("reviewers"), "environment reviewers")

    identities = [reviewer_identity(value) for value in reviewers]
    if identities != [("User", reviewer_user_id)]:
        fail("release environment reviewer does not match powderluv")


def verify_environment_policy(document: dict[str, Any]) -> None:
    if document.get("name") != "main":
        fail("release environment deployment policy must match only main")
    if "type" in document and document.get("type") != "branch":
        fail("release environment deployment policy must target a branch")


def read_document(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        fail(f"cannot read control JSON: {error}")
    return require_mapping(value, "release control")


def expected_control(
    control: str, release_user_id: int, reviewer_user_id: int
) -> dict[str, Any]:
    if control == "tag-creation":
        return tag_creation(release_user_id)
    if control == "tag-guard":
        return tag_guard()
    if control == "tag-immutability":
        return tag_immutability()
    if control == "environment":
        return environment(reviewer_user_id)
    if control == "environment-policy":
        return environment_policy()
    fail("unknown release control")


def verify_control(
    control: str,
    document: dict[str, Any],
    release_user_id: int,
    reviewer_user_id: int,
) -> None:
    if control == "tag-creation":
        verify_tag_creation(document, release_user_id)
    elif control == "tag-guard":
        verify_tag_guard(document)
    elif control == "tag-immutability":
        verify_tag_immutability(document)
    elif control == "environment":
        verify_environment(document, reviewer_user_id)
    elif control == "environment-policy":
        verify_environment_policy(document)
    else:
        fail("unknown release control")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    for name in ("render", "verify"):
        command = subparsers.add_parser(name)
        command.add_argument("--control", choices=CONTROLS, required=True)
        command.add_argument("--release-user-id", required=True)
        command.add_argument("--reviewer-user-id", required=True)
        if name == "verify":
            command.add_argument("document", type=Path)
    return parser


def main() -> None:
    if not sys.flags.isolated:
        fail("must run with isolated Python (-I)")
    args = build_parser().parse_args()
    release_user_id = positive_id(args.release_user_id, "release user ID")
    reviewer_user_id = positive_id(args.reviewer_user_id, "reviewer user ID")
    validate_identity_ids(release_user_id, reviewer_user_id)
    if args.command == "render":
        print(
            json.dumps(
                expected_control(args.control, release_user_id, reviewer_user_id),
                sort_keys=True,
                separators=(",", ":"),
            )
        )
        return
    verify_control(
        args.control,
        read_document(args.document),
        release_user_id,
        reviewer_user_id,
    )
    print(f"canonical release {args.control} control is enforceable")


if __name__ == "__main__":
    try:
        main()
    except ControlError as error:
        print(f"canonical release controls: {error}", file=sys.stderr)
        raise SystemExit(2)
