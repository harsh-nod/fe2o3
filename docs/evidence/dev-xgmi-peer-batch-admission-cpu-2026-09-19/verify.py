#!/usr/bin/env python3
"""Prepare, replay and seal the scoped admission CPU evidence packet."""

import sys

if not sys.flags.isolated:
    raise RuntimeError("run verification with python3 -I")

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re

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
QUALIFY = PREFIX + "qualify.py"
PROFILE_TEST = (
    "kfd_backend::xgmi_batch::tests::admission_scaling::admission_scaling_profile_rows"
)
PROFILE_SCHEMA = "fe2o3.xgmi-batch-admission-scaling.v1"
PROFILE_FIELD = "admission_profiles"
PROFILE_CASES = [(count, 1, count - 1) for count in (64, 256, 1024, 4096, 16384)] + [
    (65536, 63, 65473)
]


def need(value, message):
    if not value:
        raise RuntimeError(message)


def sha(path):
    need(path.is_file() and not path.is_symlink(), "ordinary file: " + str(path))
    return hashlib.sha256(path.read_bytes()).hexdigest()


for name, expected in PRIOR_TOOLS.items():
    need(sha(ROOT / name) == expected, "pinned frozen helper: " + name)
spec = importlib.util.spec_from_file_location(
    "admission_prior_cpu_verify", ROOT / (PRIOR + "verify.py")
)
V = importlib.util.module_from_spec(spec)
spec.loader.exec_module(V)
V.HERE = HERE
V.ROOT = ROOT
V.PREFIX = PREFIX
V.SCHEMA = "fe2o3.xgmi-peer-batch-admission-cpu.v1"
V.QUALIFY = QUALIFY
V.FIXED_TOOLS = dict(V.FIXED_TOOLS, **PRIOR_TOOLS)
V.LOCAL_TOOLS = tuple(
    PREFIX + name for name in ("qualify.py", "verify.py", "test_verify.py")
)
V.REQUIRED_SOURCE = V.REQUIRED_SOURCE | {
    "crates/fe2o3-runtime/src/kfd_backend/xgmi_batch/tests/admission_scaling.rs"
}
read = V.read
seal = V.seal
BASE_DERIVE = V.derive


def profile_rows(output):
    """Require the exact diagnostic cases without accepting a speed threshold."""
    raw_rows = re.findall(r"schema=[^\n]*(?:\n|$)", output)
    need(len(raw_rows) == len(PROFILE_CASES), "six admission profile rows")
    rows = []
    for raw, (active, requested, opposite) in zip(raw_rows, PROFILE_CASES):
        pattern = (
            "schema="
            + re.escape(PROFILE_SCHEMA)
            + r" active=([0-9]+) requested=([0-9]+) opposite_ready=([0-9]+)"
            + r" blocked=0 reference_ns=([0-9]+|not-run) candidate_ns=([0-9]+)"
            + r" accepted=true\n"
        )
        match = re.fullmatch(pattern, raw)
        need(match is not None, "exact admission profile row schema")
        count, selected, ready, reference, candidate = match.groups()
        need(
            (count, selected, ready) == tuple(map(str, (active, requested, opposite))),
            "ordered admission profile case roster",
        )
        need(
            reference == "not-run" if active == 65536 else reference != "not-run",
            "reference is omitted only at maximum active capacity",
        )
        need(
            str(int(candidate)) == candidate
            and (reference == "not-run" or str(int(reference)) == reference),
            "canonical nonnegative admission timings",
        )
        rows.append(
            {
                "active": active,
                "requested": requested,
                "opposite_ready": opposite,
                "blocked": 0,
                "reference_ns": None if reference == "not-run" else int(reference),
                "candidate_ns": int(candidate),
                "accepted": True,
            }
        )
    normalized = output
    for raw in raw_rows:
        normalized = normalized.replace(raw, "", 1)
    V.A.passing_tests(normalized, [PROFILE_TEST])
    return rows


def derive(archive, *, binding=None, live=False):
    base_binding = None
    if binding is not None:
        need(
            set(binding) == V.BINDING_KEYS | {PROFILE_FIELD},
            "exact admission binding keyset",
        )
        base_binding = {
            key: value for key, value in binding.items() if key != PROFILE_FIELD
        }
    derived = BASE_DERIVE(archive, binding=base_binding, live=live)
    names = V.roster((archive / "raw/gnu-runtime-roster/stdout").read_text())
    need(
        PROFILE_TEST in names
        and any(
            name.startswith("kfd_backend::xgmi_batch::tests::admission_scaling::")
            and name != PROFILE_TEST
            for name in names
        ),
        "admission correctness and profile test family selected",
    )
    profiles = {}
    for target in ("gnu", "musl"):
        path = archive / "raw" / (target + "-admission-profile") / "stdout"
        profiles[target] = {
            "stdout_sha256": sha(path),
            "rows": profile_rows(path.read_text()),
        }
    derived[PROFILE_FIELD] = profiles
    if binding is not None:
        need(
            json.dumps(binding[PROFILE_FIELD], sort_keys=True)
            == json.dumps(profiles, sort_keys=True),
            "exact frozen admission profile JSON types and values",
        )
        need(binding == derived, "exact frozen admission profile binding")
    return derived


# Inherited entry points resolve derive dynamically. Explicit archive arguments
# prevent the frozen module's definition-time HERE defaults from leaking through.
V.derive = derive


def prepare_binding(archive=HERE):
    return V.prepare_binding(archive)


def verify_bundle(archive=HERE, live=False):
    report = V.verify_bundle(archive, live=live)
    report[PROFILE_FIELD] = read(archive / "binding.json")[PROFILE_FIELD]
    return report


def verify_archive(archive=HERE, *, live=False, allow_unsealed=False):
    report = verify_bundle(archive, live=live)
    if not allow_unsealed or (archive / "SHA256SUMS").exists():
        seal(archive, False)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--prepare-binding", action="store_true")
    mode.add_argument("--seal", action="store_true")
    mode.add_argument("--allow-unsealed", action="store_true")
    parser.add_argument("--live", action="store_true")
    args = parser.parse_args()
    if args.prepare_binding:
        need(not args.live, "binding preparation already requires live equality")
        report = prepare_binding(HERE)
    else:
        report = verify_bundle(HERE, live=args.live)
        if args.seal:
            seal(HERE, True)
        elif not args.allow_unsealed or (HERE / "SHA256SUMS").exists():
            seal(HERE, False)
    print(json.dumps(report, sort_keys=True))


if __name__ == "__main__":
    main()
