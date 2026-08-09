#!/usr/bin/env python3
"""Installed fixed-path entrypoint for the production OCI evidence operator."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import sys


EXECUTOR = Path("/usr/libexec/fe2o3-oci-executor.py")
LAUNCHER = Path("/usr/libexec/fe2o3-oci-operator")


def main() -> int:
    if Path(__file__).absolute() != LAUNCHER:
        print(
            "fe2o3-oci-operator: production operator must run from fixed installed path",
            file=sys.stderr,
        )
        return 2
    try:
        spec = importlib.util.spec_from_file_location("fe2o3_oci_executor", EXECUTOR)
        if spec is None or spec.loader is None:
            raise OSError("fixed executor module is unavailable")
        module = importlib.util.module_from_spec(spec)
        sys.modules[spec.name] = module
        spec.loader.exec_module(module)
    except (ImportError, OSError) as error:
        print(
            f"fe2o3-oci-operator: cannot load fixed executor: {error}", file=sys.stderr
        )
        return 2
    return module.operator_main()


if __name__ == "__main__":
    raise SystemExit(main())
