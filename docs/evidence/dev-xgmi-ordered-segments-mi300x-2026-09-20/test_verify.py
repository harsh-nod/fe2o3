#!/usr/bin/env python3
"""Adversarial offline replay; mutations never alter the original archive."""

import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("segments_archive", HERE / "verify.py")
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


class EvidenceTests(unittest.TestCase):
    def test_original(self):
        result = V.verify()
        self.assertEqual(result["trials"], 6)
        self.assertEqual(result["timed_samples"], 120)
        self.assertEqual(result["endpoint_observations"], 36)

    def test_semantic_mutations_after_rehash(self):
        cases = []
        for stage in ("source-signature", "rustc", "cargo", "build-kfd", "rust-tests", "upload", "native", "collect", "cleanup"):
            cases.append(("command-" + stage, lambda f, stage=stage: edit(f, "local/" + stage + "/receipt.json", lambda r: r["command"].append("--wrong"))))
        for stage in ("build-kfd", "rust-tests", "test_xgmi_peer_segments.py"):
            cases.append(("environment-" + stage, lambda f, stage=stage: edit(f, "local/" + stage + "/receipt.json", lambda r: r["environment"].update(RUSTFLAGS="-C opt-level=0"))))
        cases += [
            ("source-omitted", lambda f: edit(f, "binding.json", lambda r: r["local_source_files"].pop("crates/fe2o3-runtime/src/lib.rs"))),
            ("stdin", lambda f: edit(f, "local/build-kfd/receipt.json", lambda r: r.update(stdin_sha256="0" * 64))),
            ("unreaped", lambda f: edit(f, "remote/1-kfd/receipt.json", lambda r: r.update(group_absent=False))),
            ("remote-command", lambda f: edit(f, "remote/1-kfd/receipt.json", lambda r: r["command"].append("--wrong"))),
            ("missing-row", lambda f: (f / "remote/1-kfd/stdout").write_bytes((f / "remote/1-kfd/stdout").read_bytes().split(b"\n", 1)[1])),
            ("claimed-median", lambda f: edit(f, "remote/validated-results.json", lambda r: r[0]["result"]["directions"][0].update(p50_ns_median=1))),
            ("changed-ELF", lambda f: edit(f, "remote/binaries-after.json", lambda r: r.update(kfd="0" * 64))),
            ("cleanup", lambda f: write(f / "local/absence/stdout", {"path_absent": False, "processes_absent": True})),
            ("skipped-differential", lambda f: (f / "local/test_xgmi_peer_segments.py/stderr").write_text(".s\n----------------------------------------------------------------------\nRan 2 tests in 1.000s\n\nOK (skipped=1)\n")),
            ("extra-packet-directory", lambda f: (f / "unexpected").mkdir()),
        ]
        for name, mutate in cases:
            with self.subTest(case=name), tempfile.TemporaryDirectory(prefix="fe2o3-segments-evidence-") as tmp:
                folder = Path(tmp) / "packet"
                shutil.copytree(HERE, folder)
                mutate(folder)
                rehash(folder)
                with self.assertRaises((RuntimeError, ValueError)):
                    V.verify(folder, sealed=False)

    def test_seal_rejects_raw_change(self):
        with tempfile.TemporaryDirectory(prefix="fe2o3-segments-evidence-") as tmp:
            folder = Path(tmp) / "packet"
            shutil.copytree(HERE, folder)
            with (folder / "README.md").open("a") as output:
                output.write("changed\n")
            with self.assertRaisesRegex(RuntimeError, "archive seal"):
                V.verify(folder)


if __name__ == "__main__":
    unittest.main()
