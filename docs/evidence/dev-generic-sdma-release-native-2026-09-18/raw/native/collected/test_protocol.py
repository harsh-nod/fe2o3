#!/usr/bin/env python3
"""CPU-only transcript and observer calibration; no runtime execution."""

import copy
import json
import unittest

import protocol as P


def fixture(case):
    return (
        f"retained_single_sdma_release=complete engine={P.TESTS[case]} primary_queue_id=0"
        " sdma_queue_id=1 resources_returned=8 device_backing=refunded host_backing=refunded"
        " retry=rejected public_root_drop=completed packets=0 mmio_stores=0\n"
        f"profile_sha256={P.PROFILE_SHA} unique_id={P.UID[2:]} queue_id=0 event_id=1"
        " cwsr_shadow_pages=24 runtime=enabled-before-create-then-disabled ring=4096"
        " roles=ring,control,eop,cwsr,completion-signals gtt_policy=accepted"
        " doorbell_slice=8192 doorbell_byte_offset=8 dontfork=confirmed mmio_stores=0"
        " packets=0 destroy=queue-then-event-then-runtime-confirmed resources_returned=8\n"
    )


class ProtocolTests(unittest.TestCase):
    def test_all_three_case_fixtures(self):
        for case in P.TESTS:
            with self.subTest(case=case):
                result = P.transcript(case, fixture(case), "")
                self.assertEqual(result["released_resources"], 8)
                self.assertEqual(result["engine"], P.TESTS[case])
                self.assertTrue(result["public_root_drop_completed"])

    def test_complete_transcript_negatives(self):
        for case in P.TESTS:
            text = fixture(case)
            bad = [
                text + "unexpected trailing payload\n",
                "extra\n" + text,
                text + text,
                text.splitlines()[0] + "\n",
                text.splitlines()[1] + "\n",
                text.replace("sdma_queue_id=1", "sdma_queue_id=0"),
                text.replace("sdma_queue_id=1", "sdma_queue_id=4294967296"),
                text.replace("primary_queue_id=0", "primary_queue_id=1"),
                text.replace("event_id=1", "event_id=256"),
                text.replace("event_id=1", "event_id=0"),
                text.replace("doorbell_byte_offset=8", "doorbell_byte_offset=8192"),
                text.replace("doorbell_byte_offset=8", "doorbell_byte_offset=7"),
                text.replace("resources_returned=8", "resources_returned=5"),
                text.replace("retry=rejected", "retry=accepted"),
                text.replace("host_backing=refunded", "host_backing=retained"),
                text.replace("public_root_drop=completed", "public_root_drop=pending"),
                text.replace(P.PROFILE_SHA, "0" * 64),
                text.replace(P.UID[2:], "0" * 16),
                text.replace("packets=0", "packets=1"),
            ]
            for index, value in enumerate(bad):
                with (
                    self.subTest(case=case, index=index),
                    self.assertRaises(ValueError),
                ):
                    P.transcript(case, value, "")
            for error in (" ", "unexpected stderr\n"):
                with self.assertRaises(ValueError):
                    P.transcript(case, text, error)

    def test_cross_case_and_unknown_names_reject(self):
        for case in P.TESTS:
            for other in P.TESTS:
                if case != other:
                    with self.assertRaises(ValueError):
                        P.transcript(case, fixture(other), "")
        with self.assertRaises(ValueError):
            P.transcript("unknown", fixture("generic"), "")

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
