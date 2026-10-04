#!/usr/bin/env python3
"""Verify Cargo and libtest JSON for one exact integration test execution."""

from __future__ import annotations

import argparse
import json
import pathlib
import sys
from typing import Any


def fail(message: str) -> None:
    raise ValueError(message)


def load_events(path: pathlib.Path) -> list[dict[str, Any]]:
    events: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as stream:
        for line_number, line in enumerate(stream, 1):
            if not line.strip():
                continue
            try:
                event = json.loads(line)
            except json.JSONDecodeError as error:
                fail(f"line {line_number} is not JSON: {error}")
            if not isinstance(event, dict):
                fail(f"line {line_number} is not a JSON object")
            events.append(event)
    if not events:
        fail("Cargo JSON evidence is empty")
    return events


def one_index(events: list[dict[str, Any]], predicate: Any, label: str) -> int:
    matches = [index for index, event in enumerate(events) if predicate(event)]
    if len(matches) != 1:
        fail(f"expected exactly one {label}, found {len(matches)}")
    return matches[0]


def verify(path: pathlib.Path, test_target: str, test_name: str) -> None:
    events = load_events(path)
    artifact_index = one_index(
        events,
        lambda event: event.get("reason") == "compiler-artifact"
        and event.get("target", {}).get("name") == test_target
        and event.get("target", {}).get("kind") == ["test"]
        and isinstance(event.get("executable"), str)
        and bool(event["executable"]),
        f"Cargo artifact for test target {test_target!r}",
    )
    build_index = one_index(
        events,
        lambda event: event.get("reason") == "build-finished"
        and event.get("success") is True,
        "successful Cargo build-finished event",
    )
    suite_start_index = one_index(
        events,
        lambda event: event.get("type") == "suite"
        and event.get("event") == "started"
        and event.get("test_count") == 1,
        "single-test suite start",
    )
    test_start_index = one_index(
        events,
        lambda event: event.get("type") == "test"
        and event.get("event") == "started"
        and event.get("name") == test_name,
        f"start event for {test_name!r}",
    )
    test_ok_index = one_index(
        events,
        lambda event: event.get("type") == "test"
        and event.get("event") == "ok"
        and event.get("name") == test_name,
        f"success event for {test_name!r}",
    )
    suite_ok_index = one_index(
        events,
        lambda event: event.get("type") == "suite"
        and event.get("event") == "ok"
        and event.get("passed") == 1
        and event.get("failed") == 0
        and event.get("ignored") == 0
        and event.get("measured") == 0
        and event.get("filtered_out") == 0,
        "unfiltered single-test suite success",
    )

    for event in events:
        if event.get("type") == "test" and event.get("name") != test_name:
            fail(f"unexpected test event for {event.get('name')!r}")
        if event.get("type") in {"test", "suite"} and event.get("event") in {
            "failed",
            "ignored",
        }:
            fail("test evidence contains a failed or ignored event")
        if event.get("reason") == "build-finished" and event.get("success") is not True:
            fail("Cargo reported an unsuccessful build")

    if not (
        artifact_index
        < build_index
        < suite_start_index
        < test_start_index
        < test_ok_index
        < suite_ok_index
    ):
        fail("Cargo and libtest evidence events are out of order")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("evidence", type=pathlib.Path)
    parser.add_argument("--test-target", required=True)
    parser.add_argument("--test-name", required=True)
    arguments = parser.parse_args()
    try:
        verify(arguments.evidence, arguments.test_target, arguments.test_name)
    except (OSError, ValueError) as error:
        print(f"invalid Cargo test evidence: {error}", file=sys.stderr)
        return 1
    print(
        f"verified Cargo/libtest JSON: {arguments.test_target}::{arguments.test_name} passed"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
