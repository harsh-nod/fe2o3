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


def verify(
    path: pathlib.Path, test_target: str, test_name: str, *, allow_filtered: bool = False,
    library_kinds: list[str] | None = None,
) -> None:
    if library_kinds is not None and (
        not library_kinds or len(set(library_kinds)) != len(library_kinds)
        or any(kind not in ("lib", "rlib", "dylib", "cdylib", "staticlib", "proc-macro")
               for kind in library_kinds)
    ):
        fail("expected a nonempty exact library-kind roster without duplicates")
    events = load_events(path)
    artifact_index = one_index(
        events,
        lambda event: event.get("reason") == "compiler-artifact"
        and isinstance(event.get("target"), dict)
        and event.get("target", {}).get("name") == test_target
        and event.get("target", {}).get("kind") == (library_kinds or ["test"])
        and (library_kinds is None or (
            event["target"].get("crate_types") == library_kinds
            and isinstance(event.get("profile"), dict)
            and event["profile"].get("test") is True
        ))
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
        and type(event.get("test_count")) is int
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
        and all(type(event.get(key)) is int for key in
                ("passed", "failed", "ignored", "measured", "filtered_out"))
        and event.get("passed") == 1
        and event.get("failed") == 0
        and event.get("ignored") == 0
        and event.get("measured") == 0
        and event["filtered_out"] >= 0
        and (allow_filtered or event["filtered_out"] == 0),
        "single-test suite success with admitted filtering",
    )

    # Libtest may emit one informational long-running notice, not a timeout exit.
    advisory_indices = [index for index, event in enumerate(events)
                        if event.get("type") == "test" and event.get("event") == "timeout"]
    if len(advisory_indices) > 1 or any(
        events[index].get("name") != test_name or not test_start_index < index < test_ok_index
        for index in advisory_indices
    ):
        fail("unexpected, duplicate, or misplaced long-running test notification")
    libtest_indices = [index for index, event in enumerate(events)
                       if event.get("type") in ("test", "suite")
                       and index not in advisory_indices]
    if libtest_indices != [suite_start_index, test_start_index, test_ok_index, suite_ok_index]:
        fail("expected exactly one suite and one named test, without extra events")
    test_artifacts = [index for index, event in enumerate(events)
                      if event.get("reason") == "compiler-artifact"
                      and isinstance(event.get("target"), dict)
                      and (event["target"].get("kind") == ["test"]
                           or (library_kinds is not None
                               and isinstance(event.get("profile"), dict)
                               and event["profile"].get("test") is True))]
    if test_artifacts != [artifact_index]:
        fail("expected exactly one selected test artifact")
    for event in events:
        if "reason" in event:
            if "type" in event or event["reason"] not in (
                "compiler-artifact", "compiler-message", "build-script-executed", "build-finished"
            ):
                fail("unexpected or mixed Cargo/libtest event")
        elif event.get("type") not in ("test", "suite"):
            fail("unexpected Cargo/libtest event")
        if event.get("type") == "test" and event.get("name") != test_name:
            fail(f"unexpected test event for {event.get('name')!r}")
        if event.get("type") in ("test", "suite") and event.get("event") in (
            "failed",
            "ignored",
        ):
            fail("test evidence contains a failed or ignored event")
        if event.get("reason") == "compiler-artifact" and not isinstance(event.get("target"), dict):
            fail("Cargo emitted a malformed artifact target")
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
    parser.add_argument("--allow-filtered", action="store_true",
                        help="allow nonnegative filtered-out count for one exact selected test")
    parser.add_argument("--lib", metavar="KINDS",
                        help="require this exact comma-separated library kind/crate_types roster")
    arguments = parser.parse_args()
    try:
        verify(arguments.evidence, arguments.test_target, arguments.test_name,
               allow_filtered=arguments.allow_filtered,
               library_kinds=None if arguments.lib is None else arguments.lib.split(","))
    except (OSError, ValueError) as error:
        print(f"invalid Cargo test evidence: {error}", file=sys.stderr)
        return 1
    print(
        f"verified Cargo/libtest JSON: {arguments.test_target}::{arguments.test_name} passed"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
