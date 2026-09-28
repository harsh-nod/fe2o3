#!/usr/bin/env python3
"""Seven isolated native correctness cells; no performance or full-refinement claim."""
import hashlib
from pathlib import Path

HERE = Path(__file__).resolve().parent
raw = (HERE / "protocol-base.py").read_bytes()
if hashlib.sha256(raw).hexdigest() != "384cf35d5f3471c4fe0b2228e39963f647383fbbb63e2475d64d643be2d40be1":
    raise RuntimeError("pinned endpoint protocol")
exec(compile(raw, str(HERE / "protocol-base.py"), "exec"), globals())

COMMIT = "dda812658d8827e7d508389c9650228844d0f503"
PREFIX = "/home/harsh/fe2o3-native-isolated-20260928."
FILTERED = 1791
CASE_NAMES = [
    ("profiles-short", BASE + "mixed_duration::native_mixed_short_profile_preserves_full_output_and_refunds_backing"),
    ("profiles-long", BASE + "mixed_duration::native_mixed_long_profile_preserves_full_output_and_refunds_backing"),
    ("owner", BASE + "mixed_duration::native_owned_later_short_completes_while_earlier_long_signal_is_pending"),
    ("timeout", BASE + "mixed_duration::observers::native_owned_timeout_recovers_exact_published_operation_and_full_output"),
    ("drop", BASE + "mixed_duration::observers::native_owned_dropped_observer_keeps_autonomous_progress_and_full_output"),
    ("backpressure", BASE + "mixed_duration::observers::native_owned_command_backpressure_refunds_rejected_launch_and_recovers"),
    ("depth", "kfd_backend::scale_capacity::native_depth::native_scaled_two_lane_2048_retained_receipts_and_cleanup"),
]
TESTS = dict(CASE_NAMES)
ORACLE_SHA = "67846f1e7332f04f0dcafcefa4da5bfad5a769501dbb62b900053be675830625"
PROFILE_SHA = "3d925910f084b860235b81d58401541782036ba5c690aceda98f197edb78093c"
SCALE_SHA = "bb75f0f77f63bed37a07d6c306fef39396c2d442d3b64c615c95f0179d3b2363"
ORACLE = load_module(HERE / "oracle.py", ORACLE_SHA, "native_async_oracle")
PROFILE = load_module(HERE / "profile.py", PROFILE_SHA, "native_async_profile")
SCALE = load_module(HERE / "scale.py", SCALE_SHA, "native_async_scale")
ORACLE.self_test()
base_command = command


def command(root, case):
    need(case in TESTS, "known native case")
    value = base_command(root, case)
    if case == "depth":
        need(value[2:4] == ["--kill-after=5s", "180s"], "pinned timeout arguments")
        value[2:4] = ["--kill-after=10s", "600s"]
    return value


def test_seconds(case):
    need(case in TESTS, "known timeout case")
    return 620 if case == "depth" else 200


def one(text, prefix):
    lines = marker_lines(text, prefix)
    need(len(lines) == 1, "one complete marker: " + prefix)
    return lines[0][len(prefix):]


def check_shutdown(text, budget=67108864, records=128):
    header = (f"Gfx942HostVisibleBackingUsageV1 {{ budget: Gfx942HostVisibleBackingBudgetV1 {{ "
              f"max_backing_bytes: {budget}, max_allocations: {records} }}, ")
    pattern = (r"before: Some\(" + re.escape(header)
               + r"used_backing_bytes: ([0-9]+), used_allocation_records: ([0-9]+), reserved_records: 0, "
               + r"retained_records: ([0-9]+), quarantined_records: 0, poisoned: false \}\)")
    before = re.findall(pattern, text)
    need(len(before) == 1, "one unpoisoned configured account before shutdown")
    used, allocations, retained = map(int, before[0])
    need(0 < used <= budget and 0 < allocations == retained <= records, "positive bounded retained account")
    need(text.count("completed: Some(" + account("HostVisible", budget, records) + ")") == 1
         and text.count("observations: [1, 1]") == 1, "exact completed account refund")


def transcript(case, stdout, stderr):
    need(case in TESTS and stdout.isascii() and stderr == "", "known ASCII test and empty stderr")
    need(re.findall(r"^running (\d+) test.*$", stdout, re.M) == ["1"], "exact single test")
    need(re.findall(r"^test (\S+) \.\.\. ", stdout, re.M) == [TESTS[case]], "exact selected test identity")
    need(len(re.findall(r"(?:^| \.\.\. )ok$", stdout, re.M)) == 1, "one successful test status")
    summaries = re.findall(r"^test result:.*$", stdout, re.M)
    need(len(summaries) == 1 and re.fullmatch(
        rf"test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; {FILTERED} filtered out; finished in [0-9]+\.[0-9]+s",
        summaries[0]), "successful current-ELF summary")
    need("FAILED" not in stdout and "panicked at" not in stdout, "no failure marker")
    result = {"case": case, "harness_passes": 1}
    if case == "depth":
        receipt = parse(one(stdout, "scale_depth_receipts_json="))
        profile = parse(one(stdout, "profile_json="))
        result["depth"] = SCALE.check(receipt, profile, PROFILE)
        check_shutdown(one(stdout, "scale_host_shutdown="), 134217728, 512)
        return result
    profiles = case in ("profiles-short", "profiles-long")
    prefix = "mixed_duration" if profiles else "mixed_duration_owner" if case == "owner" else "mixed_observer"
    variants = {"profiles-short": ["Short"], "profiles-long": ["Long"], "backpressure": ["Short", "Long"],
                "owner": ["Long", "Short"], "timeout": ["Long"], "drop": ["Long"]}[case]
    observed = re.findall(re.escape(prefix) + r" variant=(Short|Long) observed_hex=([0-9a-f]+)(?:\n|$)", stdout)
    need([name for name, _ in observed] == variants, "exact full-output order")
    for name, encoded in observed:
        need(len(encoded) == 768 and ORACLE.validate_observed(name.lower(), bytes.fromhex(encoded)), "independent 384-byte oracle")
    result["outputs"] = {name: hashlib.sha256(bytes.fromhex(encoded)).hexdigest() for name, encoded in observed}
    if profiles:
        need("profile_json=" not in stdout and stdout.count("physical_overlap=not_measured") == 1, "one isolated artifact diagnostic")
        other = "Long" if variants == ["Short"] else "Short"
        need("variant=" + other not in stdout, "no opposite-variant output")
        for variant in variants:
            check_shutdown(one(stdout, prefix + " variant=" + variant + " shutdown="))
        return result
    summary = one(stdout, prefix + " ids=")
    need(summary.endswith("physical_overlap=not_measured"), "no overlap claim")
    check_shutdown(summary)
    profile = parse(one(stdout, prefix + "_profile_json="))
    result["profile"] = PROFILE.mixed(profile, case)
    ids = re.match(r"\[([0-9]+(?:, [0-9]+)*)\] ", summary)
    need(ids is not None, "exact numeric operation roster")
    ids = list(map(int, ids[1].split(", ")))
    need(len(ids) == len(set(ids)) == len(variants) and all(0 < ref < 2**64 for ref in ids), "distinct positive operation IDs")
    publications = [row["event"]["dispatch"] for row in profile["events"] if row["event"]["kind"] == "dispatch_published"]
    need(publications == [PROFILE.resource(profile["capture_scope"], 6, ref) for ref in ids], "operation/profile identity join")
    if case == "owner":
        need("Some(Ok(Pending))" in summary, "actual native Pending diagnostic")
    else:
        expected = {"timeout": "timeout recovered_same_operation=true", "drop": "drop autonomous_completion=true",
                    "backpressure": "backpressure command_queue_full=1 rejected_launch_issued=false recovered=true native_slot_saturation=not_measured"}
        need(one(stdout, "mixed_observer_case=") == expected[case], "exact successful observer marker")
    return result
