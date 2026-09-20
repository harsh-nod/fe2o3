#!/usr/bin/env python3
"""Adverse receipt tests; no SSH, builds, or GPU execution."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("ordered_peer_receipt_verifier", HERE / "verify.py")
V = importlib.util.module_from_spec(spec)
spec.loader.exec_module(V)


class ReplayTests(unittest.TestCase):
    def test_complete_receipt(self):
        self.assertEqual(V.verify(sealed=False)["native_cases"], 8)

    def test_altered_receipts_fail_after_rehashing(self):
        cases = ["commit", "performance", "exclusivity", "boolean_type", "native_case",
                 "native_command", "exit", "delay", "busy_endpoint", "payload",
                 "cleanup", "cpu_source", "cpu_count", "cpu_binary", "extra_file", "rustc", "cargo"]
        for case in cases:
            with self.subTest(case=case), tempfile.TemporaryDirectory(prefix="fe2o3-ordered-replay-test-") as tmp:
                folder = Path(tmp) / "docs/evidence/packet"
                folder.mkdir(parents=True)
                for name in ["raw", "local"]:
                    shutil.copytree(HERE / name, folder / name)
                for name in ["controller.py", "qualify.py"]:
                    shutil.copyfile(HERE / name, folder / name)
                raw = folder / "raw"
                local = folder / "local"
                report = V.read(raw / "result.json")
                cpu = V.read(local / "result.json")

                def write(path, value):
                    path.write_text(json.dumps(value) + "\n", encoding="ascii")

                def record(label):
                    return next(r for r in report["records"] if r["label"] == label)

                if case == "commit":
                    report["source_commit"] = "0" * 40
                elif case == "performance":
                    report["performance_accepted"] = True
                elif case == "exclusivity":
                    report["exclusive_reservation"] = True
                elif case == "boolean_type":
                    report["owned_cleanup"] = 1
                elif case == "native_case":
                    path = raw / "native.stdout"
                    path.write_bytes(path.read_bytes().replace(b"segments=4096", b"segments=4095", 1))
                elif case == "native_command":
                    record("native")["command"][-1] = record("native")["command"][-1].replace("180s", "360s")
                elif case == "exit":
                    record("native")["exit"] = 1
                elif case == "delay":
                    record("settled-gpu1")["started_unix_ns"] = record("native")["finished_unix_ns"]
                elif case == "busy_endpoint":
                    path = raw / "preflight-gpu1.stdout"
                    rows = [json.loads(row) for row in path.read_text().splitlines()]
                    rows[0]["sysfs"][0]["values"]["mem_busy_percent"] = "1"
                    path.write_text("\n".join(json.dumps(row) for row in rows) + "\n", encoding="ascii")
                elif case == "payload":
                    path = raw / "payload-after.stdout"
                    path.write_bytes(b"0" * 64 + path.read_bytes()[64:])
                elif case == "cleanup":
                    (raw / "cleanup.stdout").write_bytes(b"owned_cleanup=failed\n")
                elif case == "cpu_source":
                    (local / "source-after.stdout").write_bytes(b"source differs\n")
                elif case == "cpu_count":
                    path = local / "gnu-runtime.stdout"
                    path.write_bytes(path.read_bytes().replace(b"1190 passed", b"1189 passed"))
                elif case == "cpu_binary":
                    cpu["binary_sha256"] = "0" * 64
                elif case == "extra_file":
                    (raw / "extra").write_bytes(b"unexpected\n")
                elif case in ["rustc", "cargo"]:
                    (local / (case + ".stdout")).write_bytes(b"unqualified toolchain\n")
                for item in report["records"]:
                    write(raw / (item["label"] + ".json"), item)
                report["files"] = V.inventory(raw, ["result.json"])
                cpu["files"] = V.inventory(local, ["result.json"])
                write(raw / "result.json", report)
                write(local / "result.json", cpu)
                with self.assertRaises(RuntimeError):
                    V.verify(folder, sealed=False)

    def test_sealed_inventory_detects_modified_member(self):
        with tempfile.TemporaryDirectory(prefix="fe2o3-ordered-replay-test-") as tmp:
            folder = Path(tmp) / "docs/evidence/packet"
            shutil.copytree(HERE, folder)
            (folder / "inventory.json").write_text(
                json.dumps(V.inventory(folder, ["inventory.json"])) + "\n", encoding="ascii"
            )
            self.assertEqual(V.verify(folder)["native_cases"], 8)
            (folder / "README.md").write_bytes(b"altered\n")
            with self.assertRaisesRegex(RuntimeError, "archive seal"):
                V.verify(folder)

    def test_json_rejects_duplicate_keys_and_nonfinite_numbers(self):
        for text in ['{"a": 1, "a": 2}', '{"a": NaN}', '{"a": Infinity}']:
            with self.subTest(text=text), self.assertRaises((ValueError, RuntimeError)):
                V.H.parse_json(text)


if __name__ == "__main__":
    unittest.main()
