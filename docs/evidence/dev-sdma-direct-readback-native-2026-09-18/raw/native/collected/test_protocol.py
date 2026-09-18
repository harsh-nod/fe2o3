#!/usr/bin/env python3
"""CPU-only transcript and observer calibration; no runtime execution."""

import copy
import json
import unittest

import protocol as P

SUMMARY = "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1124 filtered out; finished in 0.01s\n"


def fixture(case):
    marker = (
        P.MARKERS[case]
        + " cold=error host_baseline=host device_baseline=device shutdown=settled"
        + " native_readback_sha256="
        + P.READBACK_SHA
    )
    return f"running 1 test\ntest {P.TESTS[case]} ... {marker}\nok\n\n{SUMMARY}"


class ProtocolTests(unittest.TestCase):
    def test_both_native_case_fixtures(self):
        for case in P.TESTS:
            with self.subTest(case=case):
                result = P.transcript(case, fixture(case), "")
                self.assertEqual(result["harness_passes"], 1)
                self.assertEqual(result["readback_sha256"], P.READBACK_SHA)

    def test_harness_and_marker_negatives(self):
        for case in P.TESTS:
            text = fixture(case)
            other = "host" if case == "device" else "device"
            bad = [
                text.replace(SUMMARY, "", 1),
                text.replace("running 1 test", "running 0 tests", 1),
                text.replace(P.TESTS[case], P.TESTS[other], 1),
                text.replace(P.MARKERS[case], P.MARKERS[other]),
                text.replace("1124 filtered", "1121 filtered"),
                text.replace("1 passed", "0 passed"),
                text + "test unrelated ... ok\n",
                text + text,
                text.replace(P.READBACK_SHA, "0" * 64),
                text.replace("retry_bytes=4096", "retry_bytes=4095"),
                text.replace("rejected_bytes=", "wrong_bytes="),
                text.replace(" host_baseline=", " wrong_baseline="),
                text.replace(" device_baseline=", " wrong_baseline="),
                text.replace(" shutdown=", " wrong_shutdown="),
                text.replace(P.READBACK_SHA, P.READBACK_SHA + " extra=true"),
                text + "native_cold_allocation_settlement=extra\n",
                text + "panicked at unexpected\n",
            ]
            for index, value in enumerate(bad):
                with (
                    self.subTest(case=case, index=index),
                    self.assertRaises(ValueError),
                ):
                    P.transcript(case, value, "")
            with self.assertRaises(ValueError):
                P.transcript(case, text, "unexpected stderr\n")

    def test_cross_case_and_unknown_names_reject(self):
        with self.assertRaises(ValueError):
            P.transcript("host", fixture("device"), "")
        with self.assertRaises(ValueError):
            P.transcript("unknown", fixture("host"), "")

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
