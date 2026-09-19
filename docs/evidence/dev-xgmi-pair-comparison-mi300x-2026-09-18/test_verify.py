#!/usr/bin/env python3
"""Postcollection artifact calibration on disposable copies; no device access."""

import sys

if not sys.flags.isolated:
    raise RuntimeError("run with python3 -I")

import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("artifact_verifier", HERE / "verify.py")
V = importlib.util.module_from_spec(spec)
spec.loader.exec_module(V)
P, B = V.P, V.B


class ArtifactCalibration(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="fe2o3-pair-verifier-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "packet"
        shutil.copytree(HERE, self.root)
        seal = self.root / "SHA256SUMS"
        if seal.exists():
            seal.unlink()

    def write(self, relative, value):
        (self.root / relative).write_text(json.dumps(value, sort_keys=True) + "\n")

    def mutate_json(self, relative, change):
        value = P.read(self.root / relative)
        change(value)
        self.write(relative, value)

    def refresh_receipt(self, relative):
        folder = self.root / relative
        self.mutate_json(
            relative + "/receipt.json",
            lambda r: r.update(
                {s + "_sha256": P.sha(folder / s) for s in ("stdout", "stderr")}
            ),
        )

    def refresh_collection(self):
        manifest = B.inventory(self.root / "remote")
        self.write("remote-inventory.json", manifest)
        self.write("local/remote-inventory/stdout", manifest)
        self.refresh_receipt("local/remote-inventory")

    def reject(self):
        with self.assertRaises((RuntimeError, ValueError, KeyError, OSError)):
            V.verify(self.root)

    def test_valid_collection(self):
        result = V.verify(self.root)
        self.assertTrue(result["native_execution"])
        self.assertFalse(result["performance_acceptance"])
        self.assertFalse(result["formal_refinement"])
        self.assertEqual(result["remote_commands"], 62)

    def test_source_substitution_after_collection(self):
        self.mutate_json(
            "remote/source-after.json",
            lambda r: r["candidate"]["source_files"].update({"Cargo.toml": "0" * 64}),
        )
        self.refresh_collection()
        self.reject()

    def test_source_mode_substitution(self):
        self.mutate_json(
            "remote/source-before.json",
            lambda r: r["baseline"]["source_modes"].update({"Cargo.toml": "100755"}),
        )
        self.refresh_collection()
        self.reject()

    def test_binary_cohort_roster(self):
        self.mutate_json("remote/binaries.json", lambda r: r.update(extra="0" * 64))
        self.refresh_collection()
        self.reject()

    def test_wrong_cohort_cwd(self):
        self.mutate_json(
            "remote/candidate-on1/receipt.json",
            lambda r: r.update(
                cwd=r["cwd"].replace("source-candidate", "source-baseline")
            ),
        )
        self.refresh_collection()
        self.reject()

    def test_missing_endpoint_guard(self):
        device = P.read(self.root / "binding.json")["devices"][0][0]
        shutil.rmtree(self.root / ("remote/baseline-on1-before-gpu" + str(device)))
        self.refresh_collection()
        self.reject()

    def test_postflight_lower_bound(self):
        device = P.read(self.root / "binding.json")["devices"][0][0]
        finish = P.read(self.root / "remote/baseline-on1/receipt.json")["finished_ns"]
        self.mutate_json(
            "remote/baseline-on1-settled-gpu" + str(device) + "/receipt.json",
            lambda r: r.update(started_ns=finish + 1),
        )
        self.refresh_collection()
        self.reject()

    def test_phase_chronology(self):
        self.mutate_json(
            "remote/candidate-on1/receipt.json", lambda r: r.update(started_ns=1)
        )
        self.refresh_collection()
        self.reject()

    def test_transcript_submission_identity(self):
        path = self.root / "remote/baseline-on1/stdout"
        before = path.read_bytes()
        after = before.replace(b"backend_submission=7 ", b"backend_submission=8 ", 1)
        self.assertNotEqual(before, after)
        path.write_bytes(after)
        self.refresh_receipt("remote/baseline-on1")
        self.refresh_collection()
        self.reject()

    def test_diagnostic_mode_substitution(self):
        path = self.root / "remote/candidate-off1/stdout"
        path.write_bytes((self.root / "remote/candidate-on1/stdout").read_bytes())
        self.refresh_receipt("remote/candidate-off1")
        self.refresh_collection()
        self.reject()

    def test_acceptance_flag_escalation(self):
        self.mutate_json(
            "remote/finished.json", lambda r: r.update(performance_acceptance=True)
        )
        self.refresh_collection()
        self.reject()

    def test_cleanup_absence_not_boolean(self):
        self.mutate_json("controller-state.json", lambda r: r.update(absence=1))
        self.reject()

    def test_extra_archive_node(self):
        (self.root / "unexpected").mkdir()
        self.reject()

    def test_seal_detects_document_mutation(self):
        V.seal(self.root, create=True)
        V.seal(self.root)
        path = self.root / "README.md"
        path.write_bytes(path.read_bytes() + b"changed\n")
        with self.assertRaises(RuntimeError):
            V.seal(self.root)


if __name__ == "__main__":
    unittest.main()
