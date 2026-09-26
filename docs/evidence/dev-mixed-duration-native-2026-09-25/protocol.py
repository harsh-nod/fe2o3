#!/usr/bin/env python3
"""Fixed mixed-duration correctness protocol; not a performance benchmark."""
import hashlib
from pathlib import Path

HERE = Path(__file__).resolve().parent
raw = (HERE / "protocol-base.py").read_bytes()
if hashlib.sha256(raw).hexdigest() != "384cf35d5f3471c4fe0b2228e39963f647383fbbb63e2475d64d643be2d40be1":
    raise RuntimeError("pinned endpoint protocol")
exec(compile(raw, str(HERE / "protocol-base.py"), "exec"), globals())

COMMIT = "ca1edd1423b6f094c75bc363b6d6f02004f1fa0e"
PREFIX = "/home/harsh/fe2o3-mixed-duration-20260925."
CASE_NAMES = [
    ("profiles", BASE + "mixed_duration::native_mixed_duration_profiles_preserve_full_output_and_refund_backing"),
    ("owner", BASE + "mixed_duration::native_owned_later_short_completes_while_earlier_long_signal_is_pending"),
]
TESTS = dict(CASE_NAMES)
ORACLE_SHA = "67846f1e7332f04f0dcafcefa4da5bfad5a769501dbb62b900053be675830625"
ORACLE = load_module(HERE / "oracle.py", ORACLE_SHA, "mixed_duration_oracle")
ORACLE.self_test()


def outputs(case, stdout):
    prefix = "mixed_duration_owner" if case == "owner" else "mixed_duration"
    rows = re.findall(re.escape(prefix) + r" variant=(Short|Long) observed_hex=([0-9a-f]+)(?:\n|$)", stdout)
    need([name for name, _ in rows] == (["Long", "Short"] if case == "owner" else ["Short", "Long"]),
         "exact ordered full-output roster")
    for name, encoded in rows:
        need(len(encoded) == 768 and ORACLE.validate_observed(name.lower(), bytes.fromhex(encoded)),
             "independent full 384-byte oracle including both guards")
    return {name: hashlib.sha256(bytes.fromhex(encoded)).hexdigest() for name, encoded in rows}


def owner_profile(stdout):
    lines = marker_lines(stdout, "mixed_duration_owner_profile_json=")
    need(len(lines) == 1, "one complete owner profiler")
    value = parse(lines[0].split("=", 1)[1])
    need(value["schema"] == "fe2o3-kfd-runtime-profile-v1" and same(value["schema_version"], 1), "profile schema")
    need(value["device"]["target_profile"] == "gfx942:xnack-" and same(value["device"]["wave_width"], 64), "profile target")
    events, coverage = value["events"], value["coverage"]
    need(coverage["complete_runtime_operation_history"] is True and same(coverage["dropped_events"], 0)
         and same(coverage["observed_events"], len(events)), "complete observed history")
    need(same([event["sequence"] for event in events], list(range(len(events)))), "contiguous profile")
    groups = {}
    for event in events:
        need(event["origin"] == "observed", "observed profile origin")
        groups.setdefault(event["event"]["kind"], []).append(event)
    for kind in ("native_queue_created", "native_queue_destroyed", "dispatch_published", "dispatch_completed",
                 "allocation_created", "allocation_released", "host_read"):
        need(len(groups.get(kind, [])) == 2, "exact two-operation lifecycle: " + kind)
    created = {row["event"]["queue"]: row["sequence"] for row in groups["native_queue_created"]}
    destroyed = {row["event"]["queue"]: row["sequence"] for row in groups["native_queue_destroyed"]}
    published, completed = groups["dispatch_published"], groups["dispatch_completed"]
    ids = [row["event"]["dispatch"] for row in published]
    need(len(created) == len(destroyed) == len(set(ids)) == 2 and created.keys() == destroyed.keys(), "distinct queue closure")
    need([row["event"]["dispatch"] for row in completed] == ids[::-1], "observed out-of-order completion")
    need(len({row["event"]["queue"] for row in published}) == 2, "distinct published physical queues")
    need(published[1]["sequence"] < completed[0]["sequence"], "both published before short completion")
    for row, done in zip(published, reversed(completed)):
        queue = row["event"]["queue"]
        need(created[queue] < row["sequence"] < done["sequence"] < destroyed[queue], "queue lifecycle ordering")
    allocations = [row["event"]["allocation"] for row in groups["allocation_created"]]
    need(len(set(allocations)) == 2
         and sorted(allocations) == sorted(row["event"]["allocation"] for row in groups["allocation_released"]),
         "exact allocation closure")
    need([row["event"]["allocation"] for row in groups["host_read"]] == allocations, "ordered full readback roster")
    for row in groups["host_read"]:
        need(same(row["event"]["byte_offset"], 0)
             and same(row["event"]["content"], {"state": "range_only", "byte_len": 384}), "full readback extent")
    return {"events": len(events), "queues": 2, "observed_completion_order": "short-before-long",
            "physical_overlap_measured": False}


def transcript(case, stdout, stderr):
    need(case in TESTS and stdout.isascii() and stderr == "", "known ASCII test with empty stderr")
    need(re.findall(r"^running (\d+) test.*$", stdout, re.M) == ["1"], "exact single test")
    need(re.findall(r"^test (\S+) \.\.\. ", stdout, re.M) == [TESTS[case]], "exact test identity")
    need(len(re.findall(r"(?:^| \.\.\. )ok$", stdout, re.M)) == 1, "one successful test status")
    need(re.findall(r"^test result:.*$", stdout, re.M) and len(re.findall(r"^test result:.*$", stdout, re.M)) == 1,
         "one complete summary")
    need(re.fullmatch(r"test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; 1446 filtered out; finished in [0-9]+\.[0-9]+s",
                      re.findall(r"^test result:.*$", stdout, re.M)[0]), "exact successful summary")
    need("FAILED" not in stdout and "panicked at" not in stdout, "no failure markers")
    result = {"case": case, "outputs": outputs(case, stdout), "harness_passes": 1}
    if case == "owner":
        lines = marker_lines(stdout, "mixed_duration_owner ids=")
        need(len(lines) == 1 and "Some(Ok(Pending))" in lines[0]
             and lines[0].endswith("physical_overlap=not_measured"), "native pending-signal diagnostic")
        result["profile"] = owner_profile(stdout)
    else:
        need("mixed_duration_owner" not in stdout, "no owner claim in artifact correctness")
        need(stdout.count("physical_overlap=not_measured") == 2, "two separately drained profiles")
    return result
