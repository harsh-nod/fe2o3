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
    path = folder / "remote/directed-owner/stdout"
    raw, count = re.subn(rb"\b" + field.encode() + rb"=[^ \n]+", field.encode() + b"=" + value.encode(), path.read_bytes(), count=1)
    assert count == 1
    path.write_bytes(raw)


def unreported_busy_endpoint(folder):
    index = V.DEVICES[0][0]
    path = folder / f"remote/directed-owner-before-gpu{index}/stdout"
    rows = path.read_bytes().splitlines()
    row = json.loads(rows[0])
    status = json.loads(row["status"]["stdout"])
    status["card" + str(index)]["GPU use (%)"] = "1"
    row["status"]["stdout"] = json.dumps(status) + "\n"
    rows[0] = json.dumps(row).encode()
    path.write_bytes(b"\n".join(rows) + b"\n")


def stale_endpoint_utc(folder):
    for path in (folder / "remote").glob("directed-owner-*-gpu*/stdout"):
        rows = [json.loads(line) for line in path.read_bytes().splitlines()]

        def age(value):
            if isinstance(value, dict):
                for key, child in value.items():
                    if key == "utc":
                        value[key] = "2025" + child[4:]
                    else:
                        age(child)
            elif isinstance(value, list):
                for child in value:
                    age(child)

        age(rows)
        path.write_text("".join(json.dumps(row) + "\n" for row in rows), encoding="ascii")


class EvidenceTests(unittest.TestCase):
    def test_utc_parser_preserves_nanoseconds(self):
        self.assertEqual(V.utc_ns("1970-01-01T00:00:00.000000001Z"), 1)
        self.assertEqual(V.utc_ns("1970-01-02T00:00:00.999999999Z"), 86400999999999)
        for value in (None, True, "1970-01-01T00:00:00Z", "1970-01-01T00:00:00.000001Z",
                      "2026-02-31T00:00:00.000000000Z"):
            with self.subTest(value=value), self.assertRaises((RuntimeError, ValueError)):
                V.utc_ns(value)

    def test_original(self):
        result = V.verify()
        self.assertEqual(result["trials"], 1)
        self.assertEqual(result["runtime_tests_each"], 1341)
        self.assertEqual(result["runtime_ignored_each"], 20)
        self.assertEqual(result["doctests"], 46)
        self.assertEqual(result["directed_copies"], 4)
        self.assertEqual(result["endpoint_observations"], 6)
        self.assertEqual(result["example_tests"], 6)
        self.assertEqual(result["python_tests"], 14)
        self.assertEqual(result["checked_bytes"], 327680)
        self.assertEqual(result["pending_dataflow"], "supported")
        self.assertFalse(result["exclusive_reservation"])
        self.assertFalse(result["performance_acceptance"])
        self.assertFalse(result["formal_refinement"])

    def test_semantic_mutations_after_rehash(self):
        cases = []
        for stage in ("source-signature", "rustc", "cargo", "build-kfd", "rust-tests", "upload", "native", "collect", "cleanup"):
            cases.append(("command-" + stage, lambda f, stage=stage: edit(f, "local/" + stage + "/receipt.json", lambda r: r["command"].append("--wrong"))))
        for stage in ("build-kfd", "rust-tests", "test_xgmi_directed_owner_results.py"):
            cases.append(("environment-" + stage, lambda f, stage=stage: edit(f, "local/" + stage + "/receipt.json", lambda r: r["environment"].update(RUSTFLAGS="-C opt-level=0"))))
        for field, value in (("journal", "disabled"), ("pending_dataflow", "refused"), ("dependency_edges", "3"),
                             ("directed_copies", "3"), ("checked_bytes", "65536"), ("performance_claim", "true"),
                             ("uid0", "0000000000000001"), ("events", "retained"), ("cleanup", "incomplete"),
                             ("statuses", "3-succeeded"), ("rejected", "1"), ("exclusive_reservation", "true"),
                             ("formal_refinement", "true")):
            cases.append(("receipt-" + field, lambda f, field=field, value=value: change_field(f, field, value)))
        cases += [
            ("source-omitted", lambda f: edit(f, "binding.json", lambda r: r["local_source_files"].pop("crates/fe2o3-runtime/src/lib.rs"))),
            ("source-forged", lambda f: edit(f, "binding.json", lambda r: r["local_source_files"].update({"crates/fe2o3-runtime/src/lib.rs": "0" * 64}))),
            ("feature", lambda f: edit(f, "binding.json", lambda r: r.update(features="hardware-diagnostic"))),
            ("control-type", lambda f: edit(f, "binding.json", lambda r: r["controls"].update(commands_per_tick=True))),
            ("device-swap", lambda f: edit(f, "binding.json", lambda r: r["devices"].reverse())),
            ("prefix", lambda f: edit(f, "owner.json", lambda r: r.update(path=r["path"].replace("directed-owner", "segments-owner")))),
            ("control-stdin", lambda f: edit(f, "local/cleanup/receipt.json", lambda r: r.update(stdin_sha256="0" * 64))),
            ("unreaped", lambda f: edit(f, "remote/directed-owner/receipt.json", lambda r: r.update(group_absent=False))),
            ("wrong-workload", lambda f: edit(f, "remote/directed-owner/receipt.json", lambda r: r["command"].append("extra"))),
            ("missing-receipt", lambda f: (f / "remote/directed-owner/stdout").write_bytes(b"")),
            ("claimed-dataflow", lambda f: edit(f, "remote/validated-results.json", lambda r: r[0]["result"].update(pending_dataflow="refused"))),
            ("unreported-busy-endpoint", unreported_busy_endpoint),
            ("stale-observer-transcripts", stale_endpoint_utc),
            ("short-postflight", lambda f: edit(f, f"remote/directed-owner-delayed-gpu{V.DEVICES[0][0]}/receipt.json", lambda r: r.update(started_ns=V.read(f / f"remote/directed-owner-settled-gpu{V.DEVICES[-1][0]}/receipt.json")["finished_ns"]))),
            ("changed-ELF", lambda f: edit(f, "remote/binaries-after.json", lambda r: r.update(kfd="0" * 64))),
            ("cleanup", lambda f: write(f / "local/absence/stdout", {"path_absent": False, "processes_absent": True})),
            ("claim", lambda f: edit(f, "collection.json", lambda r: r.update(formal_refinement=True))),
            ("payload-ELF", lambda f: (f / "payload/kfd-owner").write_bytes(b"different ELF")),
            ("payload-archive", lambda f: (f / "payload/source.tar.gz").write_bytes(b"different archive")),
            ("skipped-parser", lambda f: (f / "local/test_xgmi_directed_owner_results.py/stderr").write_text("....s\n----------------------------------------------------------------------\nRan 5 tests in 1.000s\n\nOK (skipped=1)\n")),
            ("extra-directory", lambda f: (f / "unexpected").mkdir()),
            ("CPU-omitted", lambda f: shutil.rmtree(f / "qualification/musl")),
            ("source-bracket-omitted", lambda f: (f / "qualification/inputs-after.json").unlink()),
            ("CPU-command", lambda f: edit(f, "qualification/gnu/record.json", lambda r: r["command"].append("filter"))),
            ("CPU-count", lambda f: (f / "qualification/gnu/stdout.log").write_bytes((f / "qualification/gnu/stdout.log").read_bytes().replace(b"1341 passed", b"0 passed"))),
            ("CPU-boolean", lambda f: edit(f, "qualification/gnu/record.json", lambda r: r.update(status=False))),
            ("CPU-unreaped", lambda f: edit(f, "qualification/gnu/record.json", lambda r: r.update(group_absent=False))),
            ("CPU-source-omitted", lambda f: edit(f, "qualification/inputs-before.json", lambda r: r["source"].pop("crates/fe2o3-runtime/src/lib.rs"))),
            ("CPU-runner", lambda f: edit(f, "qualification/inputs-before.json", lambda r: r.update(runner="0" * 64))),
        ]
        for name, mutate in cases:
            with self.subTest(case=name), tempfile.TemporaryDirectory(prefix="fe2o3-owner-archive-test-") as tmp:
                folder = Path(tmp) / "packet"
                shutil.copytree(HERE, folder)
                mutate(folder)
                rehash(folder)
                with self.assertRaises((RuntimeError, ValueError)):
                    V.verify(folder, sealed=False)

    def test_prior_ordered_owner_packet_is_not_directed_qualification(self):
        with self.assertRaises(RuntimeError):
            V.verify(HERE.parent / "dev-xgmi-owner-ready-flush-mi300x-2026-09-21")

    def test_cpu_content_checks_independent_of_signed_copy_gate(self):
        # The production verifier always authenticates the copy first. Call the
        # content layer directly here so mutations reach its individual checks.
        cases = [
            ("status-bool", lambda f: edit(f, "gnu/record.json", lambda r: r.update(status=False))),
            ("unreaped", lambda f: edit(f, "gnu/record.json", lambda r: r.update(group_absent=False))),
            ("command", lambda f: edit(f, "gnu/record.json", lambda r: r["command"].append("filter"))),
            ("missing-stage", lambda f: shutil.rmtree(f / "musl")),
            ("time-order", lambda f: edit(f, "gnu/record.json", lambda r: r.update(started_ns=1))),
            ("count", lambda f: (f / "gnu/stdout.log").write_bytes((f / "gnu/stdout.log").read_bytes().replace(b"1341 passed", b"0 passed"))),
            ("name", lambda f: (f / "gnu-example/stdout.log").write_bytes((f / "gnu-example/stdout.log").read_bytes().replace(b"tests::dropped_gate_sender", b"tests::different_gate_sender"))),
            ("docs", lambda f: (f / "docs/stdout.log").write_bytes((f / "docs/stdout.log").read_bytes().replace(b"42 passed", b"0 passed"))),
            ("shared-regression", lambda f: (f / "gnu/stdout.log").write_bytes((f / "gnu/stdout.log").read_bytes().replace(b"production_owner_gate_admits_the_unchanged_diamond_without_sibling_dependency", b"unrelated_test"))),
            ("child-banner-result", lambda f: (f / "musl/stdout.log").write_bytes((f / "musl/stdout.log").read_bytes().replace(b"... \nrunning 1 test\nok\n", b"... \nrunning 1 test\nFAILED\n"))),
            ("skipped-python", lambda f: (f / "results-tests/stderr.log").write_text("....s\n----------------------------------------------------------------------\nRan 5 tests in 1.000s\n\nOK (skipped=1)\n")),
            ("source-bracket", lambda f: edit(f, "inputs-after.json", lambda r: r["source"].pop("crates/fe2o3-runtime/src/lib.rs"))),
            ("runner", lambda f: (f / "runner.py").write_bytes(b"different runner\n")),
            ("tool", lambda f: (f / "after-rustc/stdout.log").write_bytes(b"different rustc\n")),
        ]
        started = V.read(HERE / "local/source-signature/receipt.json")["started_ns"]
        for name, mutate in cases:
            with self.subTest(case=name), tempfile.TemporaryDirectory(prefix="fe2o3-directed-cpu-check-") as temp:
                folder = Path(temp) / "qualification"
                shutil.copytree(HERE / "qualification", folder)
                mutate(folder)
                with self.assertRaises((RuntimeError, ValueError)):
                    V.verify_cpu_contents(folder, HERE / "local", started)

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
