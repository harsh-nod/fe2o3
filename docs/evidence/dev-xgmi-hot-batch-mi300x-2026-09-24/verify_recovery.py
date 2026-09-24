#!/usr/bin/env python3
"""Replay recovered native records without accepting the failed campaign."""

import importlib.util
from pathlib import Path

ROOT = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("hot_batch_recovered_records", ROOT / "verify.py")
V = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(V)

if __name__ == "__main__":
    V.main(recovered=True)
