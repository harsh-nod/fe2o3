#!/usr/bin/env python3
"""Record scoped CPU qualification and observational admission profiles."""

import hashlib
import importlib.util
from pathlib import Path
import signal
import sys

if not sys.flags.isolated:
    raise RuntimeError("run qualification with python3 -I")

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
PREFIX = "docs/evidence/dev-xgmi-peer-batch-admission-cpu-2026-09-19/"
PRIOR = "docs/evidence/dev-xgmi-peer-batch-cpu-2026-09-18/"
PRIOR_TOOLS = {
    PRIOR + "qualify.py": "8cafe8b1e1944b2a06bfd20f5a85e830f466cf0fd0246c9db2bbd507f69e25de",
    PRIOR + "verify.py": "c4f19d2bc23396dc585ebc29e391bfcc96cbabf2e54705e5bb034cf1fcf20442",
    PRIOR + "test_verify.py": "d635c53753462bd2528f6a8040a9aa0a90abe4d59a95ca7723ff9eaaec22dcf8",
}
PROFILE_TEST = (
    "kfd_backend::xgmi_batch::tests::admission_scaling::admission_scaling_profile_rows"
)


def sha(path):
    if not path.is_file() or path.is_symlink():
        raise RuntimeError("ordinary tool file: " + str(path))
    return hashlib.sha256(path.read_bytes()).hexdigest()


for name, expected in PRIOR_TOOLS.items():
    if sha(ROOT / name) != expected:
        raise RuntimeError("unauthenticated frozen helper: " + name)
spec = importlib.util.spec_from_file_location(
    "admission_prior_cpu_commands", ROOT / (PRIOR + "qualify.py")
)
Q = importlib.util.module_from_spec(spec)
spec.loader.exec_module(Q)
# The runtime change does not require rerunning unrelated KFD fault matrices.
# Keep all inherited Context, native XGMI, diagnostic and peer-batch filters.
Q.P.Q.KFD_FILTERS = ["sdma::tests::xgmi_batch_wait::"]
FIXED_TOOLS = dict(Q.FIXED_TOOLS, **PRIOR_TOOLS)


def configure(root, here):
    global SELECTOR
    Q.configure(root, here)
    SELECTOR = Q.P.Q.SELECTOR


configure(ROOT, HERE)


def commands():
    result = Q.commands()
    profiles = []
    for target in ("gnu", "musl"):
        target_args = (
            [] if target == "gnu" else ["--target", "x86_64-unknown-linux-musl"]
        )
        profiles.append(
            (
                target + "-admission-profile",
                Q.P.Q.ENV
                + [
                    "cargo",
                    "test",
                    "--frozen",
                    "-p",
                    "fe2o3-runtime",
                    "--all-features",
                    *target_args,
                    "--lib",
                    "--",
                    PROFILE_TEST,
                    "--exact",
                    "--nocapture",
                    "--test-threads=1",
                ],
                1800,
            )
        )
    index = next(i for i, (name, _, _) in enumerate(result) if name == "source-after")
    return result[:index] + profiles + result[index:]


def tool_manifest():
    result = dict(FIXED_TOOLS)
    for name in ("qualify.py", "verify.py", "test_verify.py"):
        result[PREFIX + name] = sha(HERE / name)
    return result


def main():
    runner = Q.P.Q.N
    for number in runner.MANAGED:
        signal.signal(number, runner.interrupted)
    recorder = runner.Recorder(HERE / "raw", ROOT)
    runner.write_json(HERE / "tools.json", tool_manifest())
    for name, command, seconds in commands():
        recorder.run(name, command, seconds)


if __name__ == "__main__":
    main()
