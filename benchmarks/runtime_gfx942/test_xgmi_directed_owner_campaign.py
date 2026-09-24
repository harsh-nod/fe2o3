#!/usr/bin/env python3
"""CPU-only fixed-controls and collection-before-cleanup calibration."""

import importlib.util
from pathlib import Path
import unittest

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("directed_owner_campaign", HERE / "xgmi_directed_owner_campaign.py")
C = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(C)
VALUES = ["5,0000:a6:00.0,0xb7baafd0fb173d8e", "6,0000:c6:00.0,0x10a254ce4987e716"]


class OwnerCampaign(unittest.TestCase):
    def test_device_controls_are_exact_and_distinct(self):
        self.assertEqual(C.devices_from_args(VALUES), [[5, "0000:a6:00.0", "0xb7baafd0fb173d8e"], [6, "0000:c6:00.0", "0x10a254ce4987e716"]])
        for values in [[], VALUES[:1], VALUES + VALUES[:1], [VALUES[0]] * 2,
                       [VALUES[0], VALUES[1].replace("6,", "5,")],
                       [VALUES[0], VALUES[1].replace("c6", "a6")],
                       [VALUES[0], VALUES[1].replace("0x10a254ce4987e716", "0xb7baafd0fb173d8e")],
                       [VALUES[0], VALUES[1].replace("0x10a254ce4987e716", "0x0000000000000000")],
                       [VALUES[0].replace("a6", "A6"), VALUES[1]],
                       [VALUES[0].replace("5,", "8,"), VALUES[1]]]:
            with self.subTest(values=values), self.assertRaises((RuntimeError, ValueError)):
                C.devices_from_args(values)

    def test_one_copy_only_trial_and_private_controller_prefix(self):
        owned = Path(C.N.PREFIX + "0123456789abcdef")
        devices = C.devices_from_args(VALUES)
        self.assertEqual(C.N.ORDER, ("directed-owner",))
        self.assertEqual(C.N.CONTROLS, dict(allocation_bytes=65536, allocations=5, streams=4,
            directed_copies=4, copy_bytes=32768, dependency_edges=4, max_depth=3,
            checked_bytes=327680, homes=[0, 1, 0, 0, 1], offsets=[4096, 8192, 12288, 16384, 20480],
            edges=[[0, 1], [1, 2], [1, 3], [2, 4]], canaries=[161, 178, 195, 212],
            commands_per_tick=1, polls_per_tick=1, observation_timeout_seconds=60))
        self.assertEqual(C.N.trial_specs(owned, devices), [("directed-owner", [str(owned / "kfd-owner"), devices[0][2], devices[1][2]], C.H.environment(owned))])
        self.assertEqual(C.N.PAYLOAD, {"native.py", "results.py", "hot.py", "base.py", "source.tar.gz", "kfd-owner"})
        control = C.C.control_bytes(C.N.PREFIX)
        self.assertIn(("scope['PREFIX'] = " + repr(C.N.PREFIX)).encode(), control)
        self.assertNotIn(b"LD_PRELOAD", str(C.H.environment(owned)).encode())

    def test_controls_reject_bool_for_integer(self):
        for key in ("commands_per_tick", "polls_per_tick"):
            changed = {**C.N.CONTROLS, key: True}
            self.assertFalse(C.H.same_json(changed, C.N.CONTROLS))

    def test_native_attempt_requires_collection_before_cleanup(self):
        calls = []
        self.assertEqual(C.C.settle_remote(calls.append, collected=False, native_attempted=True), (False, ["remote receipts retained for recovery"]))
        self.assertEqual(calls, [])
        self.assertEqual(C.C.settle_remote(calls.append, collected=True, native_attempted=True), (True, []))
        self.assertEqual(calls, ["cleanup", "absence"])
        calls.clear()

        def fail_cleanup(name):
            calls.append(name)
            if name == "cleanup":
                raise RuntimeError("injected cleanup refusal")

        cleaned, failures = C.C.settle_remote(fail_cleanup, collected=True, native_attempted=True)
        self.assertFalse(cleaned)
        self.assertEqual(calls, ["cleanup", "absence"])
        self.assertEqual(len(failures), 1)


if __name__ == "__main__":
    unittest.main()
