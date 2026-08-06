#!/usr/bin/env python3
"""CPU-only build process used to exercise the clean snapshot runner."""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
import time
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--linked", type=Path)
    parser.add_argument("--final", type=Path)
    parser.add_argument("--target")
    parser.add_argument(
        "--mode",
        choices=(
            "stable",
            "unstable",
            "fail",
            "missing",
            "large",
            "noisy",
            "timeout",
            "descendant",
            "mutate-source",
        ),
        default="stable",
    )
    parser.add_argument("--marker", type=Path)
    parser.add_argument("--child-marker", type=Path)
    parser.add_argument("--require-clean-env", action="store_true")
    args = parser.parse_args()

    if args.child_marker is not None:
        time.sleep(0.6)
        args.child_marker.write_text("escaped cleanup", encoding="ascii")
        return 0
    if args.linked is None or args.final is None or args.target is None:
        return 18
    if args.require_clean_env and (
        "FE2O3_TEST_POISON" in os.environ or "HOME" in os.environ
    ):
        return 19
    if os.environ.get("FE2O3_TARGET") != args.target:
        return 20
    if args.mode == "fail":
        return 21
    if args.mode == "timeout":
        time.sleep(60)
        return 0
    if args.mode == "missing":
        return 0
    if args.mode == "mutate-source":
        Path(__file__).with_name("source-mutation.txt").write_text(
            "mutated", encoding="ascii"
        )
    if args.mode == "descendant":
        if args.marker is None:
            return 22
        subprocess.Popen(
            [sys.executable, __file__, "--child-marker", str(args.marker)],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
    if args.mode == "noisy":
        chunk = b"x" * (64 * 1024)
        for _ in range(20):
            os.write(sys.stdout.fileno(), chunk)

    if args.mode == "large":
        linked_payload = b"L" * (2 * 1024 * 1024)
        final_payload = b"F" * (2 * 1024 * 1024)
    else:
        payload = (
            f"target={args.target}\nepoch={os.environ.get('SOURCE_DATE_EPOCH', '')}\n"
        ).encode("ascii")
        if args.mode == "unstable":
            payload += f"cwd={Path.cwd()}\n".encode("ascii")
        linked_payload = b"linked\n" + payload
        final_payload = b"final\n" + payload

    args.linked.parent.mkdir(parents=True, exist_ok=True)
    args.final.parent.mkdir(parents=True, exist_ok=True)
    args.linked.write_bytes(linked_payload)
    args.final.write_bytes(final_payload)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
