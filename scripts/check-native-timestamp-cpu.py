#!/usr/bin/env python3
"""Require the three CPU-only timestamp cases; this never launches a process."""

import re
import sys
from pathlib import Path

PREFIX = "memory_linux::dispatch_timestamps::program::tests::"
NAMES = tuple(PREFIX + name for name in (
    "native_timestamp_clear_preserves_all_other_bytes",
    "native_timestamp_invalid_extent_rejects_before_mutation",
    "native_timestamp_observe_requires_completed_in_range_signal",
))
LIMIT = 64 * 1024 * 1024


def validate(mode, raw):
    lines = raw.splitlines()
    if mode == "list":
        cases = [line for line in lines if re.fullmatch(r".+: (?:test|benchmark)", line)]
        summaries = [line for line in lines if re.fullmatch(r"\d+ tests?, \d+ benchmarks?", line)]
        if sorted(cases) != sorted(name + ": test" for name in NAMES) or summaries != ["3 tests, 0 benchmarks"]:
            raise ValueError("exact three timestamp CPU tests must be discovered")
    elif mode == "run":
        cases = [line for line in lines if line.startswith("test ") and not line.startswith("test result:")]
        summaries = [line for line in lines if line.startswith("test result:")]
        banners = [line for line in lines if re.fullmatch(r"running \d+ tests?", line)]
        if sorted(cases) != sorted("test " + name + " ... ok" for name in NAMES):
            raise ValueError("each original timestamp CPU test must pass once")
        if banners != ["running 3 tests"] or len(summaries) != 1 or not re.fullmatch(
            r"test result: ok\. 3 passed; 0 failed; 0 ignored; 0 measured; "
            r"\d+ filtered out; finished in [0-9.]+s", summaries[0]
        ):
            raise ValueError("one successful three-test summary with no ignored cases required")
    else:
        raise ValueError("mode must be list or run")


def main():
    if len(sys.argv) != 3:
        raise ValueError("expected list|run and original step log")
    with Path(sys.argv[2]).open("rb") as source:
        raw = source.read(LIMIT + 1)
    if len(raw) > LIMIT:
        raise ValueError("timestamp CPU log exceeds 64 MiB")
    validate(sys.argv[1], raw.decode("utf-8"))
    print("NATIVE_TIMESTAMP_CPU_" + sys.argv[1].upper() + "_OK cases=3 native_execution=false")


if __name__ == "__main__":
    main()
