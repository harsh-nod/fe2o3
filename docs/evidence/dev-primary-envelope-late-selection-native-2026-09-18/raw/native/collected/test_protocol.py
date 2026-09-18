#!/usr/bin/env python3
"""CPU-only protocol calibration. Does not execute the runtime test harness."""

import copy
import json
import unittest

import protocol as P

SUMMARY = "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1121 filtered out; finished in 0.01s\n"


def fixture(case):
    one = f"running 1 test\ntest {P.TESTS[case]} ... {P.MARKERS[case]}\nok\n\n{SUMMARY}"
    if case != "positive":
        return f"running 1 test\ntest {P.TESTS[case]} ... \n{one}ok\n\n{SUMMARY}"
    events = (
        [{"kind": "allocation_created", "allocation": str(i)} for i in range(3)]
        + [
            {"kind": "native_queue_created", "queue": "q"},
            {"kind": "dispatch_published", "queue": "q", "dispatch": "d"},
            {"kind": "dispatch_completed", "dispatch": "d"},
        ]
        + [
            {
                "kind": "host_read",
                "allocation": str(i),
                "byte_offset": 0,
                "content": {"state": "range_only", "byte_len": 4194304},
            }
            for i in range(3)
        ]
        + [{"kind": "allocation_released", "allocation": str(i)} for i in range(3)]
        + [{"kind": "native_queue_destroyed", "queue": "q"}]
    )
    profile = {
        "schema": "fe2o3-kfd-runtime-profile-v1",
        "schema_version": 1,
        "device": {"target_profile": "gfx942:xnack-", "wave_width": 64},
        "coverage": {
            "complete_runtime_operation_history": True,
            "dropped_events": 0,
            "observed_events": len(events),
        },
        "events": [
            {"sequence": i, "origin": "observed", "event": event}
            for i, event in enumerate(events)
        ],
    }
    return one.replace(
        P.MARKERS[case], "profile_json=" + json.dumps(profile) + "\n" + P.MARKERS[case]
    )


class ProtocolTests(unittest.TestCase):
    def test_three_positive_protocol_fixtures(self):
        for case in P.TESTS:
            with self.subTest(case=case):
                P.transcript(case, fixture(case), "")

    def test_parent_child_and_marker_negatives(self):
        text = fixture("error")
        bad = [
            text.replace(SUMMARY, "", 1),
            text.replace("running 1 test", "running 0 tests", 1),
            text.replace(P.TESTS["error"], P.TESTS["panic"], 1),
            text.replace(P.MARKERS["error"], P.MARKERS["panic"]),
            text.replace(P.MARKERS["error"], P.MARKERS["error"] + " extra=true"),
            text + P.MARKERS["error"] + "\n",
            text.replace("1121 filtered", "1120 filtered", 1),
            text.replace("1 passed", "0 passed", 1),
            text + "test unrelated ... ok\n",
        ]
        for index, value in enumerate(bad):
            with self.subTest(index=index), self.assertRaises(ValueError):
                P.transcript("error", value, "")
        with self.assertRaises(ValueError):
            P.transcript("panic", fixture("panic"), "unexpected panic stderr\n")

    def test_positive_profile_negatives(self):
        text = fixture("positive")
        bad = [
            text.replace(
                '"complete_runtime_operation_history": true',
                '"complete_runtime_operation_history": false',
            ),
            text.replace('"dropped_events": 0', '"dropped_events": 1'),
            text.replace('"native_queue_destroyed"', '"other_event"'),
            text.replace('"byte_len": 4194304', '"byte_len": 4', 1),
            text.replace('"dispatch": "d"', '"dispatch": "other"', 1),
            text.replace(
                "host_account_refund=complete", "host_account_refund=incomplete"
            ),
        ]
        for index, value in enumerate(bad):
            with self.subTest(index=index), self.assertRaises(ValueError):
                P.transcript("positive", value, "")

    def test_fixed_windows_and_raw_endpoint(self):
        values = {
            name: "0"
            for name in (
                "mem_info_vram_used",
                "mem_info_vis_vram_used",
                "mem_info_gtt_used",
                "gpu_busy_percent",
                "mem_busy_percent",
            )
        }
        values["unique_id"] = P.UID[2:]

        def stamp(value):
            return {"monotonic_ns": value, "utc": "2026-09-18T10:00:00.000000000Z"}

        captured = {"exit": 0, "error": None, "stderr": ""}
        endpoint = {
            "schema": "fe2o3.copy-host-observation.v1",
            "record": "observation",
            "gpu_index": P.GPU,
            "pci_bdf": P.BDF,
            "unique_id": P.UID,
            "endpoint_admitted": True,
            "reasons": [],
            "selected_pids": [],
            "vram_limit_exclusive": 512 * 1024 * 1024,
            "visibility_filters": "removed-for-cli",
            "started": stamp(21_000_000_000),
            "finished": stamp(22_000_000_000),
            "sysfs": [
                {
                    "path": "/sys/bus/pci/devices/" + P.BDF,
                    "values": values,
                    "errors": {},
                    "started": stamp(21_000_000_001 + i * 4),
                    "finished": stamp(21_000_000_002 + i * 4),
                }
                for i in range(3)
            ],
            "status": {
                **captured,
                "command": [
                    "/usr/bin/timeout",
                    "--kill-after=5s",
                    "20s",
                    "/opt/rocm/bin/rocm-smi",
                    "--showuse",
                    "--showmeminfo",
                    "vram",
                    "--showuniqueid",
                    "--showbus",
                    "--json",
                ],
                "started": stamp(21_000_000_003),
                "finished": stamp(21_000_000_004),
                "stdout": json.dumps(
                    {
                        "card4": {
                            "Unique ID": P.UID,
                            "PCI Bus": P.BDF,
                            "GPU use (%)": "0",
                            "VRAM Total Used Memory (B)": "0",
                        }
                    }
                ),
            },
            "pids": {
                **captured,
                "command": [
                    "/usr/bin/timeout",
                    "--kill-after=5s",
                    "20s",
                    "/opt/rocm/bin/rocm-smi",
                    "--showpidgpus",
                ],
                "started": stamp(21_000_000_007),
                "finished": stamp(21_000_000_008),
                "stdout": "=== ROCm System Management Interface ===\n=== GPUs Indexed by PID ===\nPID 123 is using 1 DRM device(s):\n2\n===\n=== End of ROCm SMI Log ===\n",
            },
        }
        P.endpoint(endpoint, t0=1_000_000_000, offset=20)
        for start in (20_999_999_999, 22_000_000_001):
            bad = copy.deepcopy(endpoint)
            bad["started"]["monotonic_ns"] = start
            with self.assertRaises(ValueError):
                P.endpoint(bad, t0=1_000_000_000, offset=20)
        bad = copy.deepcopy(endpoint)
        bad["sysfs"][0]["values"]["gpu_busy_percent"] = "1"
        with self.assertRaises(ValueError):
            P.endpoint(bad)
        bad = copy.deepcopy(endpoint)
        bad["pids"]["stdout"] = bad["pids"]["stdout"].replace("\n2\n", "\n4\n")
        with self.assertRaises(ValueError):
            P.endpoint(bad)


if __name__ == "__main__":
    unittest.main()
