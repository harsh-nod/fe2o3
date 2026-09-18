#!/usr/bin/env python3
"""CPU-only transcript and observer calibration; no runtime execution."""

import copy
import json
import unittest

import protocol as P


def fixture(case):
    count = P.TESTS[case]
    total = count + 2
    ids = ",".join(str(7 + 3 * index) for index in range(count))
    engines = ",".join(str(index % 2) for index in range(count))
    return (
        f"retained_combined_sdma_release=complete striped_queue_count={count} total_sdma_queue_count={total} cursor=striped-initial-0-no-advance-source-qualified primary_queue_id=0"
        f" directional_h2d_queue_id=1003 directional_d2h_queue_id=2009 striped_queue_ids={ids}"
        f" directional_engine_placement=1,0 striped_engine_placement={engines}"
        " admitted_engine_count=2 admitted_queues_per_engine=8 maximum_striped_queue_count=14"
        f" host_delta_bytes={4096 * total} host_delta_records={total}"
        f" resources_returned={11 + 3 * count} device_backing=refunded host_backing=refunded"
        " retry=rejected public_root_drop=completed packets=0 mmio_stores=0\n"
        f"profile_sha256={P.PROFILE_SHA} unique_id={P.UID[2:]} queue_id=0 event_id=1"
        " cwsr_shadow_pages=24 runtime=enabled-before-create-then-disabled ring=4096"
        " roles=ring,control,eop,cwsr,completion-signals gtt_policy=accepted"
        " doorbell_slice=8192 doorbell_byte_offset=8 dontfork=confirmed mmio_stores=0"
        f" packets=0 destroy=queue-then-event-then-runtime-confirmed resources_returned={11 + 3 * count}\n"
    )


class ProtocolTests(unittest.TestCase):
    def test_all_seven_case_fixtures_with_sparse_ids(self):
        for case in P.TESTS:
            with self.subTest(case=case):
                result = P.transcript(case, fixture(case), "")
                count = P.TESTS[case]
                self.assertEqual(result["released_resources"], 11 + 3 * count)
                self.assertEqual(result["sdma_queue_ids"], [1003, 2009] + [7 + 3 * index for index in range(count)])
                self.assertEqual(result["striped_queue_ids"], [7 + 3 * index for index in range(count)])
                self.assertEqual(result["directional_h2d_queue_id"], 1003)
                self.assertEqual(result["directional_d2h_queue_id"], 2009)
                self.assertEqual(result["striped_engine_placement"], [index % 2 for index in range(count)])
                self.assertEqual(result["directional_engine_placement"], [1, 0])
                self.assertEqual(result["admitted_engine_count"], 2)
                self.assertEqual(result["admitted_queues_per_engine"], 8)
                self.assertEqual(result["maximum_striped_queue_count"], 14)
                self.assertEqual(result["striped_queue_count"], count)
                self.assertEqual(result["total_sdma_queue_count"], count + 2)
                self.assertEqual(result["host_delta_bytes"], 4096 * (count + 2))
                self.assertEqual(result["host_delta_records"], count + 2)
                self.assertEqual(result["cursor_scope"], "striped-initial-0-no-advance-source-qualified")
                self.assertTrue(result["public_root_drop_completed"])

    def test_complete_transcript_negatives(self):
        for case in P.TESTS:
            text = fixture(case)
            count = P.TESTS[case]
            bad = [
                text + "unexpected trailing payload\n",
                "extra\n" + text,
                text + text,
                text.splitlines()[0] + "\n",
                text.splitlines()[1] + "\n",
                text.replace("striped_queue_ids=7,10", "striped_queue_ids=0,10"),
                text.replace("striped_queue_ids=7,10", "striped_queue_ids=4294967296,10"),
                text.replace("striped_queue_ids=7,10", "striped_queue_ids=10,10"),
                text.replace("striped_queue_ids=7,10", "striped_queue_ids=10"),
                text.replace("striped_queue_ids=7,10", "striped_queue_ids=4,7,10"),
                text.replace("striped_queue_ids=7,10", "striped_queue_ids=07,10"),
                text.replace("striped_engine_placement=0,1", "striped_engine_placement=1,0"),
                text.replace("striped_engine_placement=0,1", "striped_engine_placement=0,2"),
                text.replace("striped_engine_placement=0,1", "striped_engine_placement=1"),
                text.replace("striped_engine_placement=0,1", "striped_engine_placement=0,0,1"),
                text.replace("directional_engine_placement=1,0", "directional_engine_placement=0,1"),
                text.replace("directional_engine_placement=1,0", "directional_engine_placement=1,2"),
                text.replace("directional_engine_placement=1,0", "directional_engine_placement=1"),
                text.replace("directional_engine_placement=1,0", "directional_engine_placement=1,0,1"),
                text.replace("admitted_engine_count=2", "admitted_engine_count=1"),
                text.replace("admitted_queues_per_engine=8", "admitted_queues_per_engine=9"),
                text.replace("maximum_striped_queue_count=14", "maximum_striped_queue_count=16"),
                text.replace(f"host_delta_bytes={4096 * (count + 2)}", "host_delta_bytes=0"),
                text.replace(f"host_delta_records={count + 2}", "host_delta_records=0"),
                text.replace(f"striped_queue_count={count} ", "striped_queue_count=3 "),
                text.replace(f"total_sdma_queue_count={count + 2} ", "total_sdma_queue_count=3 "),
                text.replace("cursor=striped-initial-0-no-advance-source-qualified", "cursor=native-observed-1"),
                text.replace("primary_queue_id=0", "primary_queue_id=1"),
                text.replace("event_id=1", "event_id=256"),
                text.replace("event_id=1", "event_id=0"),
                text.replace("event_id=1", "event_id=01"),
                text.replace("doorbell_byte_offset=8", "doorbell_byte_offset=8192"),
                text.replace("doorbell_byte_offset=8", "doorbell_byte_offset=7"),
                text.replace("doorbell_byte_offset=8", "doorbell_byte_offset=08"),
                text.replace(f"resources_returned={11 + 3 * count}", "resources_returned=5"),
                text.replace("retry=rejected", "retry=accepted"),
                text.replace("host_backing=refunded", "host_backing=retained"),
                text.replace("device_backing=refunded", "device_backing=retained"),
                text.replace("retained_combined_sdma_release=complete", "retained_combined_sdma_release=pending"),
                text.replace("public_root_drop=completed", "public_root_drop=pending"),
                text.replace(P.PROFILE_SHA, "0" * 64),
                text.replace(P.UID[2:], "0" * 16),
                text.replace("packets=0", "packets=1"),
                text.replace("mmio_stores=0", "mmio_stores=1"),
            ]
            for key, original, duplicate in (
                ("directional_h2d_queue_id", "1003", "2009"),
                ("directional_d2h_queue_id", "2009", "1003"),
            ):
                for wrong in ("0", "4294967296", "0" + original, duplicate, "7", "10"):
                    bad.append(text.replace(key + "=" + original, key + "=" + wrong))
            for directional in ("1003", "2009"):
                bad.append(text.replace("striped_queue_ids=7,10", "striped_queue_ids=" + directional + ",10"))
            for index, value in enumerate(bad):
                with (
                    self.subTest(case=case, index=index),
                    self.assertRaises(ValueError),
                ):
                    self.assertNotEqual(value, text)
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
            P.transcript("unknown", fixture("combined-2"), "")

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
                        f"card{P.GPU}": {
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
                "stdout": "=== ROCm System Management Interface ===\n=== GPUs Indexed by PID ===\nPID 123 is using 1 DRM device(s):\n0\n===\n=== End of ROCm SMI Log ===\n",
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
        bad["sysfs"][0]["values"]["mem_busy_percent"] = "1"
        with self.assertRaises(ValueError):
            P.endpoint(bad)
        bad = copy.deepcopy(endpoint)
        bad["pids"]["stdout"] = bad["pids"]["stdout"].replace("\n0\n", f"\n{P.GPU}\n")
        with self.assertRaises(ValueError):
            P.endpoint(bad)


if __name__ == "__main__":
    unittest.main()
