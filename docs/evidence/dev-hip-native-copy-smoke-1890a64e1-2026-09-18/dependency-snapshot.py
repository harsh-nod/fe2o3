#!/usr/bin/env python3
"""Hash the actual compiler dependency closure and real dynamic dependencies."""

import hashlib
import json
from pathlib import Path
import re
import shlex

ROOT = Path(__file__).resolve().parent


def main():
    dependencies = shlex.split((ROOT / "results/hip.d").read_text().replace("\\\n", ""))
    assert dependencies.pop(0) == str(ROOT / "async-copy-hip") + ":"
    paths = set()
    for name in dependencies:
        path = Path(name)
        if not path.is_absolute():
            path = ROOT / "source" / path
        paths.add(path.resolve(strict=True))
    linked = (ROOT / "results/hip.ldd").read_text()
    assert "not found" not in linked and "libamdhip64.so" in linked
    paths.update(Path(value).resolve(strict=True) for value in re.findall(r"(?:=>\s+|^\s*)(/\S+)", linked, re.MULTILINE))
    for name in [
        "/opt/rocm/lib/libamdhip64.so", "/opt/rocm/lib/libhsa-runtime64.so",
        "/opt/rocm/bin/hipcc", "/opt/rocm/bin/hipconfig", "/opt/rocm/lib/llvm/bin/clang++",
        "/opt/rocm/lib/llvm/bin/ld.lld", "/opt/rocm/bin/rocm-smi",
        "/usr/bin/numactl", "/usr/bin/timeout", "/usr/bin/prlimit", "/usr/bin/python3",
        "/usr/bin/taskset", "/usr/bin/ld", "/usr/bin/g++",
    ]:
        paths.add(Path(name).resolve(strict=True))
    assert any(str(path).endswith("/hip/hip_runtime.h") for path in paths)
    assert not any("mock" in str(path).lower() for path in paths)
    records = [{"path": str(path), "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                "bytes": path.stat().st_size} for path in sorted(paths)]
    (ROOT / "results/dependencies.json").write_text(json.dumps(records, indent=2) + "\n")
    (ROOT / "results/platform.sha256").write_text(
        "".join(f"{row['sha256']}  {row['path']}\n" for row in records)
    )
    print(json.dumps({"actual_dependency_files": len(records), "mock_linked": False}))


if __name__ == "__main__":
    main()
