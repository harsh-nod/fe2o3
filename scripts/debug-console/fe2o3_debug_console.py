#!/usr/bin/env python3
"""Bounded interactive adapter over the existing CPU debugger JSONL protocol."""
import argparse
import os
from pathlib import Path
import stat
import sys

from debug_console_process import run_console

KINDS = ("kir-v7", "bundle", "bundle-v2", "bundle-v3", "bundle-v4",
         "bundle-v5", "bundle-v6", "diagnostic-kir-v16", "diagnostic-kir-v17")

def regular_path(value, cap, executable=False):
    path = Path(value)
    if not path.is_absolute() or len(os.fsencode(value)) > 4096 or any(ord(c) < 32 for c in value):
        raise ValueError("canonical absolute bounded path required")
    if str(path.resolve(strict=True)) != value:
        raise ValueError("symlink/redirected input path refused")
    info = path.stat(follow_symlinks=False)
    if not stat.S_ISREG(info.st_mode) or not 0 < info.st_size <= cap:
        raise ValueError("regular nonempty file within cap required")
    if executable and not os.access(value, os.X_OK):
        raise ValueError("debugger executable required")
    return value

def launch_arguments(args):
    binary = regular_path(args.binary, 512 * 1024 * 1024, executable=True)
    kernel = regular_path(args.input, 64 * 1024 * 1024)
    request = regular_path(args.request, 1024 * 1024)
    if args.kind not in KINDS or args.wave_width not in (32, 64):
        raise ValueError("unsupported CPU input profile")
    return [binary, "sim", "--" + args.kind, kernel, "--request", request,
            "--wave-width", str(args.wave_width), "--protocol", "jsonl"]

def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, help="absolute trusted fe2o3-debug executable")
    parser.add_argument("--kind", choices=KINDS, required=True)
    parser.add_argument("--input", required=True, help="absolute already-exported KIR/bundle")
    parser.add_argument("--request", required=True, help="absolute simulation request")
    parser.add_argument("--wave-width", type=int, choices=(32, 64), default=64)
    args = parser.parse_args(argv)
    try:
        command = launch_arguments(args)
    except (OSError, ValueError) as error:
        parser.error(str(error))
    return run_console(command)

if __name__ == "__main__":
    sys.exit(main())
