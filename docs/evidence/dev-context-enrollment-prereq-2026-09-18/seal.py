#!/usr/bin/env python3
"""Run portable audits/calibrations and seal this new packet exactly once."""

import argparse
import importlib.util
from pathlib import Path
import sys

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location(
    "enrollment_archive_seal", HERE / "verify.py"
)
V = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = V
exec(
    compile((HERE / "verify.py").read_bytes(), str(HERE / "verify.py"), "exec"),
    V.__dict__,
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", required=True, type=Path)
    args = parser.parse_args()
    V.need(HERE == V.ORIGINAL / V.ARCHIVE_RELATIVE, "original packaging location")
    V.need(args.source_root.resolve() == V.ORIGINAL, "original source location")
    V.need(not (HERE / "SHA256SUMS").exists(), "archive not already sealed")
    V.need(not (HERE / "packaging").exists(), "fresh packaging receipts")
    V.source_inputs(HERE)
    _, j4 = V.checker(HERE)
    environment = {
        "HOME": "/home/harsh",
        "PATH": "/usr/bin:/bin",
        "PYTHONDONTWRITEBYTECODE": "1",
    }
    recipes = (
        ("portable-audit", ["verify.py", "--unsealed"]),
        ("calibration", ["test_verifier.py", "--unsealed"]),
        ("source-match", ["verify.py", "--unsealed", "--source-root", str(V.ORIGINAL)]),
    )
    (HERE / "packaging").mkdir()
    for name, arguments in recipes:
        command = ["/usr/bin/python3", "-B", str(HERE / arguments[0]), *arguments[1:]]
        status, _, _ = j4.BASE.run_owned(
            command, 180, HERE / "packaging" / name, environment
        )
        V.need(status == 0, f"packaging command passed: {name}")
        print(f"{name}: PASS", flush=True)
    result = V.audit(HERE, require_manifest=False)
    V.packaging(HERE, result)
    names = sorted(V.files(HERE))
    (HERE / "SHA256SUMS").write_text(
        "".join(f"{V.sha(HERE / name)}  {name}\n" for name in names)
    )
    V.audit(HERE)
    print(f"sealed {len(names)} files; manifest sha256={V.sha(HERE / 'SHA256SUMS')}")


if __name__ == "__main__":
    main()
