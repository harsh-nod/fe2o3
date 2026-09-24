#!/usr/bin/env python3
"""CPU-only matrix/parser/custody calibration; never starts GPU work."""

import copy
import hashlib
import json
from pathlib import Path
import re
import shutil
import tempfile
from types import ModuleType, SimpleNamespace
import unittest
from unittest import mock

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]


def module(name):
    path = HERE / (name + ".py")
    value = ModuleType("matrix_test_" + name)
    value.__file__ = str(path)
    exec(compile(path.read_bytes(), str(path), "exec"), value.__dict__)
    return value


P, N, M, C = [module(name) for name in ("protocol", "native", "prepare", "campaign")]
OBSERVER = P.load_module(REPO / "benchmarks/runtime_gfx942/copy-host-observe.py", P.OBSERVER_SHA, "matrix_test_observer")


def profile(streams, pending):
    events = []
    def event(kind, **fields):
        events.append({"origin": "observed", "sequence": len(events), "event": {"kind": kind, **fields}})
    count = streams * 3 + (2 if pending else 0)
    for index in range(count):
        event("allocation_created", allocation=index + 100)
    for index in range(streams):
        event("native_queue_created", queue=index + 10)
        event("dispatch_published", queue=index + 10, dispatch=index + 20)
        event("dispatch_completed", dispatch=index + 20)
    for index in range(count):
        event("host_read", allocation=index + 100, byte_offset=0,
              content={"state": "range_only", "byte_len": 4194304 if index < streams * 3 else 4096})
        event("allocation_released", allocation=index + 100)
    for index in range(streams):
        event("native_queue_destroyed", queue=index + 10)
    return {"schema": "fe2o3-kfd-runtime-profile-v1", "schema_version": 1,
            "device": {"target_profile": "gfx942:xnack-", "wave_width": 64}, "events": events,
            "coverage": {"complete_runtime_operation_history": True, "observed_events": len(events), "dropped_events": 0}}


def transcript(case):
    summary = "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1421 filtered out; finished in 0.01s\n"
    frame = f"running 1 test\ntest {P.TESTS[case]} ... ok\n{summary}"
    stdout, stderr = frame * (2 if case.startswith("primary-") else 1), ""
    marker = P.marker(case)
    if case.startswith("generated-"):
        stderr = marker + "\n"
    elif marker is not None:
        stdout += marker + "\n"
    else:
        zero = case == "zero-cache"
        for size in (17, 4097):
            digest = hashlib.sha256(bytes((index * 17 + 3) % 251 for index in range(size))).hexdigest()
            stdout += f"native_device_promotion logical_bytes={size} physical_bytes={((size + 4095) // 4096) * 4096} readback_sha256={digest}\n"
        pool = ("Gfx942SdmaMemoryPoolObservationV1 { checked_out_buffers: 0, retained_free_buffers: "
                + ("0, retained_free_bytes: 0, reuse_count: 0" if zero else "4, retained_free_bytes: 16402, reuse_count: 8") + " }")
        stdout += (f"native_device_promotion=complete zero_cache={str(zero).lower()} pool_before_trim={pool} device_before_trim="
                   + P.account("Device", 1048576, 8, 0 if zero else 12288, 0 if zero else 2)
                   + " device_before=" + P.account("Device", 1048576, 8, 12288, 2)
                   + " device_after=" + P.account("Device", 1048576, 8) + " host_usage=" + P.shutdown(1048576, 8)
                   + " profile_events=8\n")
    if case in P.TYPED:
        streams, pending = P.TYPED[case]
        stdout += "profile_json=" + json.dumps(profile(streams, pending)) + "\n"
        if pending:
            stdout += "native_pending_receipts=fixture new_allocations=2 physical_overlap=not_measured\n"
    return stdout, stderr


def endpoint():
    clock = iter(range(30_000_000_000, 30_000_000_100))
    def stamp():
        return {"utc": "2026-09-24T00:00:00.000000001Z", "monotonic_ns": next(clock)}
    def snapshot():
        return {"started": stamp(), "finished": stamp(), "path": "/sys/bus/pci/devices/" + P.BDF,
                "values": {key: P.UID[2:] if key == "unique_id" else "0" for key in OBSERVER.METRICS}, "errors": {}}
    def captured(arguments, stdout):
        return {"command": ["/usr/bin/timeout", "--kill-after=5s", "20s", "/opt/rocm/bin/rocm-smi", *arguments],
                "started": stamp(), "finished": stamp(), "exit": 0, "error": None, "stdout": stdout, "stderr": ""}
    value = {"schema": OBSERVER.SCHEMA, "record": "observation", "index": 0, "gpu_index": P.GPU,
             "pci_bdf": P.BDF, "unique_id": P.UID, "started": stamp()}
    before = snapshot()
    value["status"] = captured(["--showuse", "--showmeminfo", "vram", "--showuniqueid", "--showbus", "--json"],
                                json.dumps({"card1": {"Unique ID": P.UID, "PCI Bus": P.BDF, "GPU use (%)": "0",
                                                       "VRAM Total Used Memory (B)": "0"}}))
    between = snapshot()
    value["pids"] = captured(["--showpidgpus"], "=== ROCm System Management Interface ===\n=== GPUs Indexed by PID ===\n"
                             "PID 9999 is using 0 DRM device(s)\n===\n=== End of ROCm SMI Log ===\n")
    value.update(sysfs=[before, between, snapshot()], finished=stamp(), endpoint_admitted=True, reasons=[], selected_pids=[],
                 scope="sequential-endpoint-observations-not-continuous-monitoring-or-reservation",
                 visibility_filters="removed-for-cli", vram_limit_exclusive=512 * 1024 * 1024)
    complete = {"schema": OBSERVER.SCHEMA, "record": "complete", "observations": 1, "refused": 0,
                "all_endpoints_admitted": True, "performance_accepted": False}
    return value, complete


def encoded(value, complete):
    return (json.dumps(value) + "\n" + json.dumps(complete) + "\n").encode()


class ProtocolTests(unittest.TestCase):
    def test_protocol_capture_precedes_helper_execution(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "helper.py"
            path.write_bytes(b"value = 1\n")
            captured = C.capture(path)
            path.write_bytes(b"raise AssertionError('reread mutated helper')\n")
            self.assertEqual(C.module(path, "captured_only", captured).value, 1)
            alias = Path(temporary) / "alias.py"
            alias.symlink_to(path)
            with self.assertRaisesRegex(RuntimeError, "ordinary protocol source"):
                C.capture(alias)

    def test_suffix_is_only_the_two_producer_cases(self):
        self.assertEqual(N.selected_cases(P), P.CASE_NAMES[22:])
        self.assertEqual(len(N.selected_cases(P)), 2)
        self.assertEqual([name for name, _ in N.selected_cases(P)], ["queued-producer", "published-producer"])

    def test_helper_pins_match_current_source(self):
        for path, digest in ((REPO / "benchmarks/runtime_gfx942/copy-host-observe.py", P.OBSERVER_SHA),
                             (REPO / "benchmarks/runtime_gfx942/r26-host-guard.py", N.TOPOLOGY_SHA),
                             (REPO / C.RECORDER, P.RECORDER_SHA), (REPO / M.HELPER, P.BASE_SHA)):
            self.assertEqual(P.sha(path), digest)

    def test_exact_roster_and_case_environments(self):
        self.assertEqual((len(P.CASE_NAMES), len(P.TESTS), len(set(P.TESTS.values()))), (24, 24, 22))
        source = (REPO / M.CPU / "raw/cpu4/commands/musl-runtime/stdout").read_text()
        self.assertEqual({name for name, result in M.roster(source).items() if result == "ignored"}, set(P.TESTS.values()))
        for case in P.TESTS:
            env = P.environment(Path("/owned"), case)
            self.assertEqual("FE2O3_TEST_NATIVE_ISOLATED" in env,
                             case.startswith(("generated-", "cold-")) or case.endswith("-producer"))
            self.assertEqual("FE2O3_TEST_NATIVE_ACCOUNTING" in env, case == "copy-256mib")
            self.assertEqual("FE2O3_TEST_NATIVE_INITIALIZED_PREFIX" in env, case.startswith("prefix-"))
            self.assertNotIn("FE2O3_TEST_PRIMARY_RELEASE_ENVELOPE_CHILD", env)
            self.assertEqual(P.command(Path("/owned"), case)[-6:],
                             ["--exact", P.TESTS[case], "--ignored", "--nocapture", "--test-threads=1", "--color=never"])

    def test_all_24_parser_calibrations(self):
        self.assertEqual(sum(P.transcript(case, *transcript(case))["harness_passes"] for case in P.TESTS), 26)

    def test_transcript_mutations(self):
        for case in P.TESTS:
            stdout, stderr = transcript(case)
            for text, err in ((stdout.replace("1421 filtered", "1420 filtered"), stderr),
                              (stdout.replace("0.01s", "1..2s"), stderr), (stdout.replace("... ok", "..."), stderr),
                              (stdout.replace(P.TESTS[case], "foreign-test"), stderr), (stdout, stderr + "unexpected\n")):
                with self.subTest(case=case, text=text[:30]), self.assertRaises((ValueError, KeyError)):
                    P.transcript(case, text, err)

    def test_producer_markers_require_exact_mode_custody_and_complete_bytes(self):
        for case in ("queued-producer", "published-producer"):
            stdout, stderr = transcript(case)
            marker = P.marker(case)
            mutations = [stdout.replace(marker, ""), stdout + marker + "\n"]
            for field, replacement in (("authority_calls=2", "authority_calls=1"),
                                       ("checked_bytes=1048576", "checked_bytes=4"),
                                       ("peak_readers=4", "peak_readers=3"),
                                       ("terminal_readers=0", "terminal_readers=1"),
                                       ("terminal_writers=0", "terminal_writers=1"),
                                       ("public_event=released", "public_event=retained"),
                                       ("observation=consumer_first", "observation=producer_first"),
                                       ("materializations=0", "materializations=1"),
                                       ("cleanup=complete", "cleanup=unknown")):
                mutations.append(stdout.replace(field, replacement))
            mutations.append(stdout.replace("published=" + str(case == "published-producer").lower(),
                                             "published=" + str(case != "published-producer").lower()))
            mutations.append(stdout.replace("a_sha256=1202", "a_sha256=0000"))
            for text in mutations:
                with self.subTest(case=case), self.assertRaises(ValueError):
                    P.transcript(case, text, stderr)

    def test_cold_promotion_accounts_cannot_be_spliced_or_corrupted(self):
        for case in ("cold-device", "cold-host", "device-promotion", "zero-cache"):
            stdout, stderr = transcript(case)
            for text in (stdout.replace("poisoned: false", "poisoned: true", 1),
                         stdout.replace("used_backing_bytes: 0", "used_backing_bytes: 1", 1),
                         stdout + P.marker_lines(stdout, "native_")[0] + "\n",
                         stdout.replace("profile_events=8", "profile_events=9") if "promotion" in case or case == "zero-cache"
                         else stdout.replace("kind: Capacity", "kind: Terminal")):
                with self.subTest(case=case), self.assertRaises(ValueError):
                    P.transcript(case, text, stderr)

    def test_profiler_rejects_boolean_and_wrong_readback(self):
        for mutate in (lambda v: v.update(schema_version=True), lambda v: v["coverage"].update(dropped_events=False),
                       lambda v: v["events"][0].update(sequence=False),
                       lambda v: next(row["event"] for row in v["events"] if row["event"]["kind"] == "host_read").update(byte_offset=1)):
            value = profile(2, True)
            mutate(value)
            with self.assertRaises(ValueError):
                P.profile("profile_json=" + json.dumps(value), 2, True)

    def test_endpoint_policy_and_fixed_windows(self):
        value, complete = endpoint()
        P.endpoint(encoded(value, complete), OBSERVER, t0=value["started"]["monotonic_ns"] - 20 * 10**9, offset=20)
        with self.assertRaises(ValueError):
            P.endpoint(encoded(value, complete), OBSERVER, t0=value["started"]["monotonic_ns"] - 22 * 10**9, offset=20)
        for field, content in (("gpu_busy_percent", "1"), ("mem_busy_percent", "1"),
                               ("mem_info_vram_used", str(512 * 1024 * 1024)), ("unique_id", "0" * 16)):
            changed = copy.deepcopy(value)
            changed["sysfs"][0]["values"][field] = content
            with self.subTest(field=field), self.assertRaises(ValueError):
                P.endpoint(encoded(changed, complete), OBSERVER)
        for number in (True, 1.0):
            with self.assertRaises(ValueError):
                P.endpoint(encoded(value, {**complete, "observations": number}), OBSERVER)
        changed = copy.deepcopy(value)
        changed["pids"]["stdout"] = changed["pids"]["stdout"].replace("using 0 DRM device(s)", "using 1 DRM device(s):\n1")
        with self.assertRaises(ValueError):
            P.endpoint(encoded(changed, complete), OBSERVER)

    def test_historical_output_as_parser_calibration_only(self):
        historical = REPO / "docs/evidence/dev-r126-native-closure-2026-09-17/native"
        for case, number in (("cold-device", 0), ("cold-host", 1), ("device-promotion", 6), ("zero-cache", 7)):
            text = (historical / f"probe-{number:02}.log").read_text()
            text = re.sub(r"[0-9]+ filtered out", "1421 filtered out", text)
            P.transcript(case, text, "")

    def test_captured_loader_rejects_wrong_digest(self):
        with self.assertRaises(ValueError):
            P.load_module(HERE / "native.py", "0" * 64, "must_not_execute")

    def test_build_artifact_rejects_multiple_and_foreign_paths(self):
        with tempfile.TemporaryDirectory() as raw:
            target = Path(raw)
            directory = target / "x86_64-unknown-linux-musl/debug/deps"
            directory.mkdir(parents=True)
            binary = directory / "runtime"
            binary.write_bytes(b"\x7fELFfixture")
            artifact = {"reason": "compiler-artifact", "target": {"name": "fe2o3_runtime", "kind": ["lib"]},
                        "profile": {"test": True}, "executable": str(binary)}
            data = json.dumps(artifact).encode()
            self.assertEqual(M.executable(data, target), binary)
            for changed in (data + b"\n" + data, json.dumps({**artifact, "executable": "/bin/true"}).encode()):
                with self.assertRaises(RuntimeError):
                    M.executable(changed, target)

    def test_recorded_spawn_persistence_failure_keeps_custody_and_t0(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            shutil.copy2(REPO / C.RECORDER, root / "recorder.py")
            r, rec = N.recorder(P, root)
            original = r.write
            def fail_active(path, value):
                if path.name == "receipt.json":
                    raise OSError("injected persistence failure")
                return original(path, value)
            with mock.patch.object(r, "write", side_effect=fail_active):
                row, _, _ = rec.run("cpu-only", ["/bin/true"], 5)
            self.assertTrue(row["group_absent"])
            self.assertIsNotNone(row["t0"])
            self.assertEqual(len(rec.spawn_failures), 1)
            self.assertIsNone(r.ACTIVE)

    def test_recorded_spawn_does_not_overwrite_live_custody(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            shutil.copy2(REPO / C.RECORDER, root / "recorder.py")
            r, rec = N.recorder(P, root)
            active = SimpleNamespace(pid=123456)
            r.ACTIVE = active
            with mock.patch.object(r, "group_exists", return_value=True):
                row, _, _ = rec.run("not-spawned", ["/bin/true"], 5)
            self.assertIs(r.ACTIVE, active)
            self.assertIsNone(row["process_group"])
            self.assertFalse(row["group_absent"])
            self.assertFalse(list(rec.output.glob("active-*")))

    def test_spawn_and_cleanup_failure_preserves_handle(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            shutil.copy2(REPO / C.RECORDER, root / "recorder.py")
            child = SimpleNamespace(pid=123456)
            with mock.patch.object(N.subprocess, "Popen", return_value=child):
                r, rec = N.recorder(P, root)
            with mock.patch.object(r, "write", side_effect=OSError("disk")), \
                 mock.patch.object(r, "stop_owned", side_effect=RuntimeError("cleanup")):
                self.assertIs(r.subprocess.Popen(["not-executed"]), child)
            self.assertIs(r.ACTIVE, child)
            self.assertEqual(len(rec.spawn_failures), 2)

    def test_failed_case_attempts_both_postflights_and_stops(self):
        for refusal in ("test", "immediate", "delayed", "persistence"):
            with self.subTest(refusal=refusal), tempfile.TemporaryDirectory() as raw:
                root = Path(raw)
                calls = []
                rec = SimpleNamespace(output=root, spawn_failures=[])
                def run(name, command, seconds, fresh=None):
                    calls.append(name)
                    if refusal == "persistence" and name.endswith("-test"):
                        rec.spawn_failures.append("failed")
                    directory = root / name
                    directory.mkdir()
                    (directory / "stdout").write_text("fixture")
                    (directory / "stderr").write_text("")
                    return ({"status": 1 if name.endswith("-" + refusal) else 0, "group_absent": True,
                             "spawned": {"monotonic_ns": 1}, "t0": {"monotonic_ns": 10**9}}, directory / "stdout", directory / "stderr")
                rec.run = run
                r = SimpleNamespace(passed=lambda row: row["status"] == 0, write=lambda *_: None)
                value = {"started": {"monotonic_ns": 2}, "finished": {"monotonic_ns": 3}}
                cases = []
                with mock.patch.object(P, "CASE_NAMES", P.CASE_NAMES[:2]), mock.patch.object(P, "endpoint", return_value=value), \
                     mock.patch.object(P, "stamp", side_effect=lambda value: value["monotonic_ns"]), \
                     mock.patch.object(P, "transcript", return_value={}), mock.patch.object(N, "resources", return_value={}), \
                     mock.patch.object(N.time, "monotonic_ns", return_value=30 * 10**9):
                    with self.assertRaises(ValueError):
                        N.run_cases(P, r, rec, root, None, lambda: None, cases)
                self.assertEqual(calls, ["01-two-stream-" + part for part in ("before", "test", "immediate", "delayed")])
                self.assertEqual(len(cases), 1)

    def test_unrecorded_prelaunch_spawn_never_starts_test(self):
        for when in ("prior", "preflight"):
            with self.subTest(when=when), tempfile.TemporaryDirectory() as raw:
                root = Path(raw)
                calls, cases = [], []
                rec = SimpleNamespace(output=root, spawn_failures=["failed"] if when == "prior" else [])
                def run(name, *_):
                    calls.append(name)
                    rec.spawn_failures.append("failed")
                    (root / "stdout").write_text("fixture")
                    (root / "stderr").write_text("")
                    return ({"spawned": {"monotonic_ns": 1}, "t0": {"monotonic_ns": 4}}, root / "stdout", root / "stderr")
                rec.run = run
                r = SimpleNamespace(passed=lambda _: True, write=lambda *_: None)
                value = {"started": {"monotonic_ns": 2}, "finished": {"monotonic_ns": 3}}
                with mock.patch.object(P, "endpoint", return_value=value), \
                     mock.patch.object(P, "stamp", side_effect=lambda value: value["monotonic_ns"]), \
                     mock.patch.object(N, "resources", return_value={}):
                    with self.assertRaises(ValueError):
                        N.run_cases(P, r, rec, root, None, lambda: None, cases)
                self.assertEqual(cases, [])
                self.assertEqual(calls, [] if when == "prior" else ["01-two-stream-before"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
