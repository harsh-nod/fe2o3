#!/usr/bin/env python3
"""CPU-only mock used to exercise the clean-build runner."""

from __future__ import annotations

import argparse
import os
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--target", required=True)
    parser.add_argument(
        "--mode", choices=("stable", "unstable", "fail", "missing"), default="stable"
    )
    parser.add_argument("--require-clean-env", action="store_true")
    args = parser.parse_args()

    if args.require_clean_env and "FE2O3_TEST_POISON" in os.environ:
        return 19
    if os.environ.get("FE2O3_TARGET") != args.target:
        return 20
    if args.mode == "fail":
        return 21
    if args.mode == "missing":
        return 0

    payload = (
        f"target={args.target}\nepoch={os.environ.get('SOURCE_DATE_EPOCH', '')}\n"
    ).encode("ascii")
    if args.mode == "unstable":
        payload += f"cwd={Path.cwd()}\n".encode("ascii")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(payload)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
