#!/usr/bin/env python3
"""CPU-only transcript and observer calibration; no runtime execution."""

import copy
import json
from pathlib import Path
import re
import unittest

import protocol as P


def fixture(case):
    lanes = P.TESTS[case]
    return (
        "retained_logical_mux_sdma_release=complete"
        f" logical_lane_count={lanes} native_queue_count=2"
        " cursor=initial-0-no-advance-source-qualified primary_queue_id=0"
        " sdma_queue_ids=1003,2009 engine_placement=0,1"
        " host_delta_bytes=8192 host_delta_records=2 resources_returned=11"
        " device_backing=refunded host_backing=refunded retry=rejected"
        " public_root_drop=completed packets=0 mmio_stores=0\n"
        f"profile_sha256={P.PROFILE_SHA} unique_id={P.UID[2:]} queue_id=0 event_id=1"
        " cwsr_shadow_pages=24 runtime=enabled-before-create-then-disabled ring=4096"
        " roles=ring,control,eop,cwsr,completion-signals gtt_policy=accepted"
        " doorbell_slice=8192 doorbell_byte_offset=8 dontfork=confirmed"
        " mmio_stores=0 packets=0 destroy=queue-then-event-then-runtime-confirmed"
        " resources_returned=11\n"
    )


class ProtocolTests(unittest.TestCase):
    def test_all_five_case_fixtures_with_sparse_ids(self):
        self.assertEqual(list(P.TESTS.values()), [2, 4, 8, 14, 16])
        self.assertEqual(list(P.TESTS), [f"logical-mux-{n}" for n in (2, 4, 8, 14, 16)])
        for case, lanes in P.TESTS.items():
            with self.subTest(case=case):
                result = P.transcript(case, fixture(case), "")
                self.assertEqual(result["logical_lane_count"], lanes)
                self.assertEqual(result["native_queue_count"], 2)
                self.assertEqual(result["released_resources"], 11)
                self.assertEqual(result["sdma_queue_ids"], [1003, 2009])
                self.assertEqual(result["engine_placement"], [0, 1])
                self.assertEqual(result["host_delta_bytes"], 8192)
                self.assertEqual(result["host_delta_records"], 2)
                self.assertEqual(result["cursor_scope"], "initial-0-no-advance-source-qualified")
                self.assertTrue(result["public_root_drop_completed"])
                swapped = fixture(case).replace("1003,2009", "2009,1003")
                self.assertEqual(P.transcript(case, swapped, "")["sdma_queue_ids"], [2009, 1003])

    def test_complete_transcript_negatives(self):
        for case, lanes in P.TESTS.items():
            text = fixture(case)
            bad = [
                text + "unexpected trailing payload\n", "extra\n" + text, text + text,
                text.splitlines()[0] + "\n", text.splitlines()[1] + "\n",
                text.replace(f"logical_lane_count={lanes} ", "logical_lane_count=3 "),
                text.replace("native_queue_count=2", "native_queue_count=16"),
                text.replace("cursor=initial-0-no-advance-source-qualified", "cursor=native-observed-0"),
                text.replace("primary_queue_id=0", "primary_queue_id=1"),
                text.replace(" queue_id=0 ", " queue_id=1 "),
                text.replace("event_id=1", "event_id=256"),
                text.replace("event_id=1", "event_id=0"),
                text.replace("event_id=1", "event_id=01"),
                text.replace("doorbell_byte_offset=8", "doorbell_byte_offset=8192"),
                text.replace("doorbell_byte_offset=8", "doorbell_byte_offset=7"),
                text.replace("doorbell_byte_offset=8", "doorbell_byte_offset=08"),
                text.replace("host_delta_bytes=8192", "host_delta_bytes=65536"),
                text.replace("host_delta_records=2", "host_delta_records=16"),
                text.replace("retry=rejected", "retry=accepted"),
                text.replace("host_backing=refunded", "host_backing=retained"),
                text.replace("device_backing=refunded", "device_backing=retained"),
                text.replace("retained_logical_mux_sdma_release=complete", "retained_logical_mux_sdma_release=pending"),
                text.replace("public_root_drop=completed", "public_root_drop=pending"),
                text.replace(P.PROFILE_SHA, "0" * 64),
                text.replace(P.UID[2:], "0" * 16),
            ]
            for wrong in ("0,2009", "4294967296,2009", "01003,2009", "1003,0", "1003,4294967296", "1003,02009", "1003,1003", "2009,2009", "1003", "1003,2009,3001"):
                bad.append(text.replace("1003,2009", wrong))
            for wrong in ("1,0", "0,2", "0", "0,1,0"):
                bad.append(text.replace("engine_placement=0,1", "engine_placement=" + wrong))
            for original, replacement in (("resources_returned=11", "resources_returned=5"), ("packets=0", "packets=1"), ("mmio_stores=0", "mmio_stores=1")):
                occurrences = list(re.finditer(re.escape(original), text))
                self.assertEqual(len(occurrences), 2)
                for match in occurrences:
                    bad.append(text[:match.start()] + replacement + text[match.end():])
            for index, value in enumerate(bad):
                with self.subTest(case=case, index=index), self.assertRaises(ValueError):
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
            P.transcript("unknown", fixture("logical-mux-2"), "")

    def test_old_combined_protocol_is_not_admitted(self):
        old = (
            "retained_combined_sdma_release=complete striped_queue_count=2 total_sdma_queue_count=4"
            " cursor=striped-initial-0-no-advance-source-qualified primary_queue_id=0"
            " directional_h2d_queue_id=1 directional_d2h_queue_id=2 striped_queue_ids=3,4"
            " directional_engine_placement=1,0 striped_engine_placement=0,1"
            " admitted_engine_count=2 admitted_queues_per_engine=8 maximum_striped_queue_count=14"
            " host_delta_bytes=16384 host_delta_records=4 resources_returned=17"
            " device_backing=refunded host_backing=refunded retry=rejected"
            " public_root_drop=completed packets=0 mmio_stores=0\n"
        ) + fixture("logical-mux-2").splitlines(keepends=True)[1].replace("resources_returned=11", "resources_returned=17")
        for case in P.TESTS:
            with self.assertRaises(ValueError):
                P.transcript(case, old, "")
        forbidden = ("--retained-release-combined-sdma", "striped_queue_count", "total_sdma_queue_count", "directional_h2d_queue_id", "maximum_striped_queue_count", "11 + 3 *", "count + 2")
        for name in ("protocol.py", "run.py", "PLAN.md"):
            source = (Path(__file__).resolve().parent / name).read_text()
            for token in forbidden:
                with self.subTest(name=name, token=token):
                    self.assertNotIn(token, source)

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
