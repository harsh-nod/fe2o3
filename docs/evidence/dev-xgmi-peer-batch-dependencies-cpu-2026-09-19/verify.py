#!/usr/bin/env python3
"""Prepare, replay and seal the scoped dependency-index CPU evidence packet."""

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
PREFIX = "docs/evidence/dev-xgmi-peer-batch-dependencies-cpu-2026-09-19/"
PRIOR = "docs/evidence/dev-xgmi-peer-batch-cpu-2026-09-18/"
PRIOR_TOOLS = {
    PRIOR + "qualify.py": "8cafe8b1e1944b2a06bfd20f5a85e830f466cf0fd0246c9db2bbd507f69e25de",
    PRIOR + "verify.py": "c4f19d2bc23396dc585ebc29e391bfcc96cbabf2e54705e5bb034cf1fcf20442",
    PRIOR + "test_verify.py": "d635c53753462bd2528f6a8040a9aa0a90abe4d59a95ca7723ff9eaaec22dcf8",
}
QUALIFY = PREFIX + "qualify.py"
PROFILE_TEST = (
    "kfd_backend::xgmi_batch::tests::dependency_scaling::dependency_scaling_profile_rows"
)
PROFILE_SCHEMA = "fe2o3.xgmi-dependency-index-scaling.v1"
PROFILE_FIELD = "dependency_profiles"
PROFILE_CASES = [
    ("one-empty-64", 64, 1, 0, 63),
    ("one-four-4096", 4096, 1, 4, 4095),
    ("sixty-three-empty-4096", 4096, 63, 0, 4033),
    ("sixty-three-four-16384", 16384, 63, 252, 16321),
    ("sixty-three-four-65536", 65536, 63, 252, 65473),
]
DEPENDENCY_PREFIX = "kfd_backend::xgmi_batch::tests::dependency_scaling::"
DEPENDENCY_TESTS = frozenset(
    {
        "kfd_backend::xgmi_batch::tests::dependency_scaling::one_active_scan_counts_each_source_record_once_for_relevant_duplicates",
        "kfd_backend::xgmi_batch::tests::dependency_scaling::selected_and_unselected_duplicate_dependencies_preserve_reference_semantics",
        "kfd_backend::xgmi_batch::tests::dependency_scaling::asymmetric_dependency_predicates_and_requested_order_match_the_reference",
        "kfd_backend::xgmi_batch::tests::dependency_scaling::dependency_use_count_overflow_is_reported_without_wrapping",
        "kfd_backend::xgmi_batch::tests::dependency_scaling::late_waiter_and_count_corruptions_match_the_reference",
        "kfd_backend::xgmi_batch::tests::dependency_scaling::empty_selection_is_valid_without_scratch_reservation",
        "kfd_backend::xgmi_batch::tests::dependency_scaling::scratch_capacity_precedes_predicate_mismatch_without_mutating_inputs",
        "kfd_backend::xgmi_batch::tests::dependency_scaling::successful_scratch_reservation_is_single_and_nonmutating",
        "kfd_backend::xgmi_batch::tests::dependency_scaling::maximum_healthy_requested_dependency_breadth_is_accepted",
        "kfd_backend::xgmi_batch::tests::dependency_scaling::dependency_scaling_profile_rows",
    }
)
ADMISSION_PREFIX = "kfd_backend::xgmi_batch::tests::admission_scaling::"
ADMISSION_TESTS = frozenset(
    {
        "kfd_backend::xgmi_batch::tests::admission_scaling::bounded_ready_permutations_match_the_frozen_reference",
        "kfd_backend::xgmi_batch::tests::admission_scaling::bounded_metadata_mutations_match_the_frozen_reference",
        "kfd_backend::xgmi_batch::tests::admission_scaling::malformed_requests_are_invalid_before_any_scratch_reservation",
        "kfd_backend::xgmi_batch::tests::admission_scaling::either_scratch_reservation_failure_precedes_later_classification",
        "kfd_backend::xgmi_batch::tests::admission_scaling::injected_success_reserves_both_indexes_and_matches_the_reference",
        "kfd_backend::xgmi_batch::tests::admission_scaling::late_index_corruption_precedes_subset_or_foreign_caller_errors",
        "kfd_backend::xgmi_batch::tests::admission_scaling::sixty_five_thousand_active_blocked_records_preserve_rosters",
        "kfd_backend::xgmi_batch::tests::admission_scaling::admission_scaling_profile_rows",
    }
)


def need(value, message):
    if not value:
        raise RuntimeError(message)


def sha(path):
    need(path.is_file() and not path.is_symlink(), "ordinary file: " + str(path))
    return hashlib.sha256(path.read_bytes()).hexdigest()


for name, expected in PRIOR_TOOLS.items():
    need(sha(ROOT / name) == expected, "pinned frozen helper: " + name)
spec = importlib.util.spec_from_file_location(
    "dependencies_prior_cpu_verify", ROOT / (PRIOR + "verify.py")
)
V = importlib.util.module_from_spec(spec)
spec.loader.exec_module(V)
V.HERE = HERE
V.ROOT = ROOT
V.PREFIX = PREFIX
V.SCHEMA = "fe2o3.xgmi-peer-batch-dependencies-cpu.v1"
V.QUALIFY = QUALIFY
V.FIXED_TOOLS = dict(V.FIXED_TOOLS, **PRIOR_TOOLS)
V.LOCAL_TOOLS = tuple(
    PREFIX + name for name in ("qualify.py", "verify.py", "test_verify.py")
)
V.REQUIRED_SOURCE = V.REQUIRED_SOURCE | {
    "crates/fe2o3-runtime/src/kfd_backend/xgmi_batch/tests/admission_scaling.rs",
    "crates/fe2o3-runtime/src/kfd_backend/xgmi_batch/tests/dependency_scaling.rs",
}
read = V.read
seal = V.seal
BASE_DERIVE = V.derive


def scaling_rosters(names):
    for prefix, expected, label in (
        (DEPENDENCY_PREFIX, DEPENDENCY_TESTS, "dependency"),
        (ADMISSION_PREFIX, ADMISSION_TESTS, "admission"),
    ):
        need(
            {name for name in names if name.startswith(prefix)} == expected,
            "exact " + label + "-scaling test roster",
        )


def profile_rows(output):
    """Require exact diagnostic cases without accepting a speed threshold."""
    raw_rows = re.findall(r"schema=[^\n]*(?:\n|$)", output)
    need(len(raw_rows) == len(PROFILE_CASES), "five dependency profile rows")
    rows = []
    for raw, (case, active, requested, edges, waiters) in zip(raw_rows, PROFILE_CASES):
        pattern = (
            "schema="
            + re.escape(PROFILE_SCHEMA)
            + r" case=([a-z0-9-]+) active=([0-9]+) requested=([0-9]+)"
            + r" requested_dependency_edges=([0-9]+) blocked_waiters=([0-9]+)"
            + r" reference_ns=([0-9]+|not-run) candidate_ns=([0-9]+) result=valid\n"
        )
        match = re.fullmatch(pattern, raw)
        need(match is not None, "exact dependency profile row schema")
        label, count, selected, edge_count, waiter_count, reference, candidate = (
            match.groups()
        )
        need(
            (label, count, selected, edge_count, waiter_count)
            == (case, *map(str, (active, requested, edges, waiters))),
            "ordered dependency profile case roster",
        )
        need(
            reference == "not-run" if active == 65536 else reference != "not-run",
            "reference is omitted only at maximum active capacity",
        )
        need(
            str(int(candidate)) == candidate
            and (reference == "not-run" or str(int(reference)) == reference),
            "canonical nonnegative dependency timings",
        )
        rows.append(
            {
                "case": case,
                "active": active,
                "requested": requested,
                "requested_dependency_edges": edges,
                "blocked_waiters": waiters,
                "reference_ns": None if reference == "not-run" else int(reference),
                "candidate_ns": int(candidate),
                "result": "valid",
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
            "exact dependency binding keyset",
        )
        base_binding = {
            key: value for key, value in binding.items() if key != PROFILE_FIELD
        }
    derived = BASE_DERIVE(archive, binding=base_binding, live=live)
    names = V.roster((archive / "raw/gnu-runtime-roster/stdout").read_text())
    scaling_rosters(names)
    profiles = {}
    for target in ("gnu", "musl"):
        path = archive / "raw" / (target + "-dependency-profile") / "stdout"
        profiles[target] = {
            "stdout_sha256": sha(path),
            "rows": profile_rows(path.read_text()),
        }
    derived[PROFILE_FIELD] = profiles
    if binding is not None:
        need(
            json.dumps(binding[PROFILE_FIELD], sort_keys=True)
            == json.dumps(profiles, sort_keys=True),
            "exact frozen dependency profile JSON types and values",
        )
        need(binding == derived, "exact frozen dependency profile binding")
    return derived


# Inherited entry points resolve derive dynamically. Always pass the archive
# explicitly so the frozen module's original default path cannot leak through.
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
