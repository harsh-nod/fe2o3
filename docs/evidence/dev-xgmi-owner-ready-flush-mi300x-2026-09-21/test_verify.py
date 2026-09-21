#!/usr/bin/env python3
"""Offline mutations on disposable copies; no hardware access."""

import importlib.util
import json
from pathlib import Path
import re
import shutil
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("owner_archive", HERE / "verify.py")
V = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(V)


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="ascii")


def edit(folder, name, change):
    path = folder / name
    value = V.read(path)
    change(value)
    write(path, value)


def rehash(folder):
    for path in folder.glob("*/**/receipt.json"):
        value = V.read(path)
        for stream in ("stdout", "stderr"):
            value[stream + "_sha256"] = V.sha(path.parent / stream)
        write(path, value)
    write(folder / "remote-inventory.json", V.B.inventory(folder / "remote"))
    marker = V.read(folder / "owner.json")
    marker["binding_sha256"] = V.sha(folder / "binding.json")
    write(folder / "owner.json", marker)


def change_field(folder, field, value):
    path = folder / "remote/owner/stdout"
    raw, count = re.subn(rb"\b" + field.encode() + rb"=[^ \n]+", field.encode() + b"=" + value.encode(), path.read_bytes(), count=1)
    assert count == 1
    path.write_bytes(raw)


def unreported_busy_endpoint(folder):
    path = folder / "remote/owner-before-gpu5/stdout"
    rows = path.read_bytes().splitlines()
    row = json.loads(rows[0])
    status = json.loads(row["status"]["stdout"])
    status["card5"]["GPU use (%)"] = "1"
    row["status"]["stdout"] = json.dumps(status) + "\n"
    rows[0] = json.dumps(row).encode()
    path.write_bytes(b"\n".join(rows) + b"\n")


class EvidenceTests(unittest.TestCase):
    def test_original(self):
        result = V.verify()
        self.assertEqual(result["trials"], 1)
        self.assertEqual(result["ordered_lists"], 2)
        self.assertEqual(result["endpoint_observations"], 6)
        self.assertEqual(result["runtime_passed_per_target"], 1221)
        self.assertEqual(result["pending_dataflow"], "refused_before_submission")
        self.assertFalse(result["performance_acceptance"])
        self.assertFalse(result["formal_refinement"])

    def test_semantic_mutations_after_rehash(self):
        cases = []
        for stage in ("source-signature", "rustc", "cargo", "build-kfd", "rust-tests", "upload", "native", "collect", "cleanup"):
            cases.append(("command-" + stage, lambda f, stage=stage: edit(f, "local/" + stage + "/receipt.json", lambda r: r["command"].append("--wrong"))))
        for stage in ("build-kfd", "rust-tests", "test_xgmi_segments_owner_results.py"):
            cases.append(("environment-" + stage, lambda f, stage=stage: edit(f, "local/" + stage + "/receipt.json", lambda r: r["environment"].update(RUSTFLAGS="-C opt-level=0"))))
        for field, value in (("journal", "disabled"), ("pending_dataflow", "supported"), ("dependency", "pending_producer"),
                             ("ordered_lists", "3"), ("segments", "8"), ("performance_claim", "true"),
                             ("uid0", "0000000000000001"), ("dropped_observer", "abandoned"), ("cleanup", "incomplete")):
            cases.append(("receipt-" + field, lambda f, field=field, value=value: change_field(f, field, value)))
        cases += [
            ("source-omitted", lambda f: edit(f, "binding.json", lambda r: r["local_source_files"].pop("crates/fe2o3-runtime/src/lib.rs"))),
            ("source-forged", lambda f: edit(f, "binding.json", lambda r: r["local_source_files"].update({"crates/fe2o3-runtime/src/lib.rs": "0" * 64}))),
            ("feature", lambda f: edit(f, "binding.json", lambda r: r.update(features="hardware-diagnostic"))),
            ("control-type", lambda f: edit(f, "binding.json", lambda r: r["devices"][0].__setitem__(0, True))),
            ("prefix", lambda f: edit(f, "owner.json", lambda r: r.update(path=r["path"].replace("owner-20260921", "attribution-20260921")))),
            ("control-stdin", lambda f: edit(f, "local/cleanup/receipt.json", lambda r: r.update(stdin_sha256="0" * 64))),
            ("unreaped", lambda f: edit(f, "remote/owner/receipt.json", lambda r: r.update(group_absent=False))),
            ("wrong-workload", lambda f: edit(f, "remote/owner/receipt.json", lambda r: r["command"].append("extra"))),
            ("missing-receipt", lambda f: (f / "remote/owner/stdout").write_bytes(b"")),
            ("claimed-dataflow", lambda f: edit(f, "remote/validated-results.json", lambda r: r[0]["result"].update(pending_dataflow="supported"))),
            ("unreported-busy-endpoint", unreported_busy_endpoint),
            ("short-postflight", lambda f: edit(f, "remote/owner-delayed-gpu5/receipt.json", lambda r: r.update(started_ns=V.read(f / "remote/owner-settled-gpu6/receipt.json")["finished_ns"]))),
            ("changed-ELF", lambda f: edit(f, "remote/binaries-after.json", lambda r: r.update(kfd="0" * 64))),
            ("cleanup", lambda f: write(f / "local/absence/stdout", {"path_absent": False, "processes_absent": True})),
            ("claim", lambda f: edit(f, "collection.json", lambda r: r.update(formal_refinement=True))),
            ("qualification-claim", lambda f: edit(f, "qualification-finished.json", lambda r: r.update(native_execution=True))),
            ("skipped-parser", lambda f: (f / "local/test_xgmi_segments_owner_results.py/stderr").write_text(".ss\n----------------------------------------------------------------------\nRan 3 tests in 1.000s\n\nOK (skipped=2)\n")),
            ("extra-directory", lambda f: (f / "unexpected").mkdir()),
            ("runtime-omitted", lambda f: shutil.rmtree(f / "qualification/x86_64-unknown-linux-musl")),
            ("source-bracket-omitted", lambda f: shutil.rmtree(f / "qualification/source-after")),
            ("runtime-command", lambda f: edit(f, "qualification/x86_64-unknown-linux-gnu/receipt.json", lambda r: r["command"].append("filter"))),
            ("runtime-count", lambda f: (f / "qualification/x86_64-unknown-linux-gnu/stdout").write_bytes((f / "qualification/x86_64-unknown-linux-gnu/stdout").read_bytes().replace(b"1221 passed", b"0 passed"))),
        ]
        for name, mutate in cases:
            with self.subTest(case=name), tempfile.TemporaryDirectory(prefix="fe2o3-owner-archive-test-") as tmp:
                folder = Path(tmp) / "packet"
                shutil.copytree(HERE, folder)
                mutate(folder)
                rehash(folder)
                with self.assertRaises((RuntimeError, ValueError)):
                    V.verify(folder, sealed=False)

    def test_prior_caller_driven_packet_is_not_owner_qualification(self):
        with self.assertRaises(RuntimeError):
            V.verify(HERE.parent / "dev-xgmi-ordered-host-attribution-mi300x-2026-09-21")

    def test_seal_rejects_raw_change(self):
        with tempfile.TemporaryDirectory(prefix="fe2o3-owner-archive-test-") as tmp:
            folder = Path(tmp) / "packet"
            shutil.copytree(HERE, folder)
            with (folder / "README.md").open("a") as output:
                output.write("changed\n")
            with self.assertRaisesRegex(RuntimeError, "archive seal"):
                V.verify(folder)


if __name__ == "__main__":
    unittest.main()
