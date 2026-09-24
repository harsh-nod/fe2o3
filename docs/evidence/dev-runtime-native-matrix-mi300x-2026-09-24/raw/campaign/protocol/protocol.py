#!/usr/bin/env python3
"""Fixed current-source native matrix and strict, offline transcript checks."""

import hashlib
import json
import re
from types import ModuleType

COMMIT = "3540325123fa30b5208d57ba950922a707eb07b8"
GPU, UID, BDF = 1, "0xab83d2ffef0d3cdf", "0000:26:00.0"
PREFIX = "/home/harsh/fe2o3-runtime-native-20260924."
OBSERVER_SHA = "5d37b09bf0823854c6a45b914d866c042c1e65486cd96c6301285a0147ae6c51"
RECORDER_SHA = "3960b587fb80c45af05f8c906ef7067ccc8b3a28d4ad161e8ff7a10ccd4f354c"
BASE_SHA = "df740a80c83c94af02f7e7dfb1203dcf7c1eafbde0010e41219e4aac40b34bb7"
BASE = "kfd_backend::retained_release_tests::"
GENERATED = "kfd_backend::generated_adoption::tests::native::"
CASE_NAMES = [
    ("two-stream", BASE + "native_runtime_two_stream_dispatch_uses_primary_and_auxiliary_then_refunds"),
    *[("prefix-" + str(prefix), BASE + "native_runtime_auxiliary_budget_failure_retains_initialized_prefix") for prefix in range(3)],
    ("primary-error", BASE + "primary_envelope::native_runtime_primary_release_error_retains_installed_root"),
    ("primary-panic", BASE + "primary_envelope::native_runtime_primary_release_panic_retains_installed_root_and_payload"),
    *[("generated-" + start + "-" + action, GENERATED + "generated_native_" + start + "_" + suffix)
      for action, suffix in (("abort", "primary_auxiliary_rebound_abort_and_shutdown"),
                             ("issue", "issue_complete_readback_retire"), ("roster", "full_roster_readback"))
      for start in ("cold", "bootstrap")],
    ("typed", BASE + "native_runtime_typed_dispatch_shutdown_refunds_and_profiles_retained_primary"),
    ("pending-primary", BASE + "native_runtime_allocates_while_primary_compute_is_pending"),
    ("pending-both", BASE + "native_runtime_allocates_while_primary_and_auxiliary_compute_are_pending"),
    ("auxiliary-retry", BASE + "native_runtime_auxiliary_shutdown_retries_after_primary_capacity_rejection"),
    ("cold-device", BASE + "cold_allocation::native_runtime_cold_device_capacity_refunds_context_and_retries"),
    ("cold-host", BASE + "cold_allocation::native_runtime_cold_host_capacity_refunds_context_and_retries"),
    ("allocation-release", BASE + "native_runtime_allocation_shutdown_selects_retained_directional_release"),
    ("device-promotion", BASE + "native_runtime_device_promotion_roundtrip_and_retained_shutdown"),
    ("zero-cache", BASE + "native_runtime_zero_capacity_recycle_disposes_before_trim"),
    ("copy-256mib", BASE + "copy_accounting::native_runtime_directional_256_mib_copy_shutdown_refunds_exact_backing"),
]
TESTS = dict(CASE_NAMES)
TYPED = {"two-stream": (2, False), "typed": (1, False), "pending-primary": (1, True),
         "pending-both": (2, True), "auxiliary-retry": (2, False)}
ENV = {"HOME": "/home/harsh", "PATH": "/usr/bin:/bin:/opt/rocm/bin", "LANG": "C", "LC_ALL": "C",
       "PYTHONDONTWRITEBYTECODE": "1", "HSA_XNACK": "0", "FE2O3_TEST_NATIVE_UNIQUE_ID": UID}


def need(condition, message):
    if not condition:
        raise ValueError(message)


def sha(path):
    need(path.is_file() and not path.is_symlink(), "ordinary file: " + str(path))
    return hashlib.sha256(path.read_bytes()).hexdigest()


def pairs(items):
    result = {}
    for key, value in items:
        need(key not in result, "duplicate JSON key")
        result[key] = value
    return result


def parse(text):
    return json.loads(text, object_pairs_hook=pairs, parse_constant=lambda value: need(False, "nonfinite JSON"))


def same(left, right):
    return json.dumps(left, sort_keys=True) == json.dumps(right, sort_keys=True)


def account(kind, budget, records, used=0, retained=0):
    return (f"Gfx942{kind}BackingUsageV1 {{ budget: Gfx942{kind}BackingBudgetV1 {{ max_backing_bytes: {budget}, "
            f"max_allocations: {records} }}, used_backing_bytes: {used}, used_allocation_records: {retained}, "
            f"reserved_records: 0, retained_records: {retained}, quarantined_records: 0, poisoned: false }}")


def shutdown(device_budget, device_records):
    host = account("HostVisible", 16777216, 32, 532480, 3)
    empty = account("HostVisible", 16777216, 32)
    device = account("Device", device_budget, device_records)
    return (f"PrimaryHostUsage {{ before: Some({host}), completed: Some({empty}), device_before: Some({device}), "
            f"device_completed: Some({device}), observations: [1, 1] }}")


def marker_lines(text, prefix):
    return [line[line.index(prefix):] for line in text.splitlines() if prefix in line]


def load_module(path, digest, name):
    need(path.is_file() and not path.is_symlink(), "ordinary module")
    raw = path.read_bytes()
    need(hashlib.sha256(raw).hexdigest() == digest, "authenticated module")
    module = ModuleType(name)
    module.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), module.__dict__)
    return module


def environment(root, case=None):
    value = {**ENV, "TMPDIR": str(root / "tmp")}
    if case is not None and case.startswith(("generated-", "cold-")):
        value["FE2O3_TEST_NATIVE_ISOLATED"] = "1"
    if case is not None and case.startswith("prefix-"):
        value["FE2O3_TEST_NATIVE_INITIALIZED_PREFIX"] = case[-1]
    if case == "copy-256mib":
        value["FE2O3_TEST_NATIVE_ACCOUNTING"] = "1"
    return value


def command(root, case):
    return ["/usr/bin/timeout", "--signal=TERM", "--kill-after=5s", "180s", "/usr/bin/prlimit",
            "--core=0:0", "--fsize=16777216:16777216", "--", "/usr/bin/numactl", "--physcpubind=0-47",
            "--membind=0", str(root / "runtime-tests"), "--exact", TESTS[case], "--ignored", "--nocapture",
            "--test-threads=1", "--color=never"]


def marker(case):
    if case in TYPED:
        streams, pending = TYPED[case]
        return ("native_runtime_typed_dispatch_retained_release=complete selector=retained kernel=vecadd "
                f"compute_ordinals={list(range(streams))} primary_execution=confirmed pending_allocations={str(pending).lower()} "
                f"packets={streams} readbacks={3 * streams + (2 if pending else 0)} "
                "output_sha256=79fd0768604fe9de0ced87297f7d653343e998926b59e2cce7df5e38194c52b3 "
                "host_account_refund=complete queue_profile=matched completed_primary_root_drop=confirmed backend_drop=completed")
    if case.startswith("prefix-"):
        return (f"native_auxiliary_budget_failure=retained initialized_prefix={case[-1]} "
                "unpublished_auxiliary=confirmed retry=inert reclamation=process_exit_only")
    if case.startswith("primary-"):
        mode = case.removeprefix("primary-")
        return (f"native_runtime_primary_release_envelope={mode} retained_original_root=true terminal=true "
                "destroyed_observation=false retry_entered=false "
                + ("original_payload=true " if mode == "panic" else "") + "disposition=process_exit")
    if case.startswith("generated-"):
        _, start, action = case.split("-")
        bootstrap = str(start == "bootstrap").lower()
        if action == "abort":
            return f"N5 DATA native mechanics: bootstrap={bootstrap}, primary/AUX/rebound, 9 DATA releases, zero publication, shutdown complete"
        full = action == "roster"
        return (f"generated native fixture: bootstrap={bootstrap}, full_roster={str(full).lower()}, primary/AUX/rebound, "
                f"{12 if full else 9} DATA disposals, shutdown complete; no protected Worker/carrier or typed reply claim")
    if case == "allocation-release":
        return ("native_runtime_retained_directional_release=complete allocation_workflow=public pooled_buffers=1 "
                "pooled_bytes=4096 selector=retained completed_root_drop=confirmed backend_drop=completed packets=0")
    if case == "copy-256mib":
        return ("native_runtime_directional_copy_accounting=complete bytes_per_allocation=268435456 allocations=3 "
                "checked_bytes=268435456 roundtrips=1 default_cache=true host_backing_after=0 device_backing_after=0 "
                "external_vram_baseline=not_asserted performance=not_measured")
    if case.startswith("cold-"):
        kind = "DeviceLocal" if case == "cold-device" else "HostVisible"
        rejected = 4097 if case == "cold-device" else 16781312
        credits = "Device" if case == "cold-device" else "HostVisible"
        digest = hashlib.sha256(bytes(((index * 17 + 3) % 251) ^ 0x5a for index in range(4096))).hexdigest()
        return (f"native_cold_allocation_settlement=complete kind={kind} rejected_bytes={rejected} retry_bytes=4096 "
                + 'cold=KfdRuntimeBackendErrorV1 { kind: Capacity, detail: "KFD persistent SDMA allocation: Sdma(Memory('
                + credits + 'BackingCredits(Capacity)))" } host_baseline='
                + account("HostVisible", 16777216, 32, 532480, 3) + " device_baseline=" + account("Device", 4096, 1)
                + " shutdown=" + shutdown(4096, 1) + " native_readback_sha256=" + digest)
    return None


def profile(text, streams, pending):
    lines = [line.split("profile_json=", 1)[1] for line in text.splitlines() if "profile_json=" in line]
    need(len(lines) == 1, "one full profiler document")
    value = parse(lines[0])
    need(value["schema"] == "fe2o3-kfd-runtime-profile-v1" and same(value["schema_version"], 1), "profiler schema")
    need(value["device"]["target_profile"] == "gfx942:xnack-" and same(value["device"]["wave_width"], 64), "profile target")
    events, coverage = value["events"], value["coverage"]
    need(coverage["complete_runtime_operation_history"] is True and same(coverage["dropped_events"], 0)
         and same(coverage["observed_events"], len(events)), "complete profiler coverage")
    need(same([row["sequence"] for row in events], list(range(len(events)))), "contiguous profile sequence")
    groups = {}
    for row in events:
        need(row["origin"] == "observed", "observed profile origin")
        groups.setdefault(row["event"]["kind"], []).append(row)
    for kind in ("native_queue_created", "native_queue_destroyed", "dispatch_published", "dispatch_completed"):
        need(len(groups.get(kind, [])) == streams, "exact compute queue/dispatch count")
    created = {row["event"]["queue"]: row["sequence"] for row in groups["native_queue_created"]}
    destroyed = {row["event"]["queue"]: row["sequence"] for row in groups["native_queue_destroyed"]}
    completed = {row["event"]["dispatch"]: row["sequence"] for row in groups["dispatch_completed"]}
    published = groups["dispatch_published"]
    need(len(created) == len(destroyed) == len(completed) == streams and created.keys() == destroyed.keys(), "distinct queue closure")
    need({row["event"]["dispatch"] for row in published} == completed.keys(), "exact dispatch closure")
    for row in published:
        queue, dispatch = row["event"]["queue"], row["event"]["dispatch"]
        need(created[queue] < row["sequence"] < completed[dispatch] < destroyed[queue], "queue dispatch completion order")
    allocations, releases, reads = [groups.get(kind, []) for kind in ("allocation_created", "allocation_released", "host_read")]
    need(len(allocations) == len(releases) == streams * 3 + (2 if pending else 0), "allocation lifecycle count")
    ids = [row["event"]["allocation"] for row in allocations]
    need(len(set(ids)) == len(ids) and sorted(ids) == sorted(row["event"]["allocation"] for row in releases), "exact allocation closure")
    need([row["event"]["allocation"] for row in reads] == ids, "complete ordered readback roster")
    for index, row in enumerate(reads):
        event = row["event"]
        need(event["allocation"] in ids and same(event["byte_offset"], 0)
             and same(event["content"], {"state": "range_only", "byte_len": 4194304 if index < streams * 3 else 4096}),
             "complete vecadd/additional allocation readback extent")


def transcript(case, stdout, stderr):
    need(case in TESTS and stdout.isascii() and stderr.isascii(), "known ASCII transcript")
    count = 2 if case.startswith("primary-") else 1
    need(len(re.findall(r"^running 1 test$", stdout, re.MULTILINE)) == count, "parent/child harness count")
    summaries = re.findall(r"^test result:.*$", stdout, re.MULTILINE)
    need(len(summaries) == count and all(re.fullmatch(
        r"test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; 1384 filtered out; finished in [0-9]+\.[0-9]+s", line)
        for line in summaries), "exact successful test summaries")
    need(re.findall(r"^test (\S+) \.\.\. ", stdout, re.MULTILINE) == [TESTS[case]] * count, "exact native test identity")
    need(len(re.findall(r"(?:^| \.\.\. )ok$", stdout, re.MULTILINE)) == count, "one ok status per harness frame")
    need("FAILED" not in stdout + stderr and "panicked at" not in stdout + stderr, "no failed test transcript")
    expected = marker(case)
    if case.startswith("generated-"):
        need(stderr == expected + "\n" and expected not in stdout, "exact generated stderr marker")
    else:
        need(stderr == "", "empty native test stderr")
        if expected is not None:
            prefix = expected.split(" ")[0].split("=")[0] + "="
            observed = marker_lines(stdout, prefix)
            need(observed == [expected], "exact native case marker")
        else:
            zero = str(case == "zero-cache").lower()
            rows = marker_lines(stdout, "native_device_promotion=complete")
            need(len(rows) == 1, "one complete promotion marker")
            trim = account("Device", 1048576, 8, 0 if case == "zero-cache" else 12288, 0 if case == "zero-cache" else 2)
            pattern = (re.escape("native_device_promotion=complete zero_cache=" + zero + " pool_before_trim="
                                 + "Gfx942SdmaMemoryPoolObservationV1 { checked_out_buffers: 0, retained_free_buffers: ")
                       + r"(0|[1-9][0-9]*), retained_free_bytes: (0|[1-9][0-9]*), reuse_count: (0|[1-9][0-9]*)"
                       + re.escape(" } device_before_trim=" + trim + " device_before=" + account("Device", 1048576, 8, 12288, 2)
                                   + " device_after=" + account("Device", 1048576, 8) + " host_usage=" + shutdown(1048576, 8)
                                   + " profile_events=8"))
            match = re.fullmatch(pattern, rows[0])
            need(match is not None, "promotion/refund/unpoisoned accounts")
            free, byte_count, reuse = map(int, match.groups())
            need((free, byte_count, reuse) == (0, 0, 0) if case == "zero-cache" else free > 0 and byte_count > 0,
                 "pool before trim")
            expected_reads = []
            for size in (17, 4097):
                expected_bytes = bytes((index * 17 + 3) % 251 for index in range(size))
                line = (f"native_device_promotion logical_bytes={size} physical_bytes={((size + 4095) // 4096) * 4096} "
                        + "readback_sha256=" + hashlib.sha256(expected_bytes).hexdigest())
                expected_reads.append(line)
            need(marker_lines(stdout, "native_device_promotion logical_bytes=") == expected_reads, "exact independent promotion digests")
    if case in TYPED:
        profile(stdout, *TYPED[case])
        if TYPED[case][1]:
            need(stdout.count("physical_overlap=not_measured") == 1, "pending custody is not physical overlap")
    else:
        need("profile_json=" not in stdout, "no unexpected profiler document")
    return {"case": case, "harness_passes": count, "marker_count": 1}


def stamp(value):
    need(type(value) is dict and set(value) == {"utc", "monotonic_ns"}
         and type(value["monotonic_ns"]) is int and value["monotonic_ns"] > 0
         and type(value["utc"]) is str
         and re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{9}Z", value["utc"]), "complete timestamp")
    return value["monotonic_ns"]


def endpoint(data, observer, *, t0=None, offset=None):
    need(type(data) is bytes and len(data) <= 4 * 1024 * 1024 and data.endswith(b"\n")
         and b"\r" not in data and b"\0" not in data, "bounded complete endpoint transcript")
    lines = data.decode("ascii").splitlines()
    need(len(lines) == 2, "two-row endpoint transcript")
    value, complete = map(parse, lines)
    need(same(complete, {"schema": "fe2o3.copy-host-observation.v1", "record": "complete", "observations": 1,
                        "refused": 0, "all_endpoints_admitted": True, "performance_accepted": False}), "strict observer completion")
    fixed = {"schema": "fe2o3.copy-host-observation.v1", "record": "observation", "index": 0,
             "gpu_index": GPU, "pci_bdf": BDF, "unique_id": UID, "endpoint_admitted": True, "reasons": [],
             "selected_pids": [], "scope": "sequential-endpoint-observations-not-continuous-monitoring-or-reservation",
             "visibility_filters": "removed-for-cli", "vram_limit_exclusive": 512 * 1024 * 1024}
    need(set(value) == set(fixed) | {"started", "finished", "status", "pids", "sysfs"}, "exact endpoint schema")
    need(json.dumps({key: value[key] for key in fixed}, sort_keys=True) == json.dumps(fixed, sort_keys=True), "fixed endpoint policy")
    snapshots = value["sysfs"]
    need(type(snapshots) is list and len(snapshots) == 3, "three sysfs snapshots")
    for snapshot in snapshots:
        need(set(snapshot) == {"started", "finished", "path", "values", "errors"}
             and snapshot["path"] == "/sys/bus/pci/devices/" + BDF and snapshot["errors"] == {}
             and set(snapshot["values"]) == set(observer.METRICS), "complete raw sysfs capture")
        values = snapshot["values"]
        need(values["unique_id"].lower() == UID[2:], "raw sysfs identity")
        for name in observer.METRICS:
            if name != "unique_id":
                number = observer.decimal(values[name], 100 if name.endswith("percent") else (1 << 64) - 1)
                if name.endswith("percent"):
                    need(number == 0, "raw GPU and memory engines idle")
                elif name == "mem_info_vram_used":
                    need(number < 512 * 1024 * 1024, "raw VRAM below limit")
    prefix = ["/usr/bin/timeout", "--kill-after=5s", "20s", "/opt/rocm/bin/rocm-smi"]
    for name, arguments in (("status", ["--showuse", "--showmeminfo", "vram", "--showuniqueid", "--showbus", "--json"]),
                            ("pids", ["--showpidgpus"])):
        captured = value[name]
        need(set(captured) == {"command", "started", "finished", "exit", "error", "stdout", "stderr"}
             and captured["command"] == prefix + arguments and type(captured["exit"]) is int and captured["exit"] == 0
             and captured["error"] is None and not captured["stderr"].strip(), "complete raw SMI capture")
    status = observer.parse_status(value["status"]["stdout"], GPU, BDF, UID)
    need(status["busy_percent"] == 0 and status["vram_bytes"] < 512 * 1024 * 1024, "raw SMI idle and VRAM")
    need(not any(GPU in devices for devices in observer.parse_pids(value["pids"]["stdout"]).values()), "no selected GPU PID")
    previous = stamp(value["started"])
    for captured in (snapshots[0], value["status"], snapshots[1], value["pids"], snapshots[2]):
        started, finished = stamp(captured["started"]), stamp(captured["finished"])
        need(previous <= started <= finished, "ordered observation captures")
        previous = finished
    need(previous <= stamp(value["finished"]), "closed observation timeline")
    if t0 is not None:
        actual = stamp(value["started"]) - t0
        need(offset * 10**9 <= actual <= (offset + 1) * 10**9, "fixed actual observer-start window")
    return value
