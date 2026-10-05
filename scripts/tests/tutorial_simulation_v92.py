#!/usr/bin/env python3
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("simulation_v92", ROOT / "scripts/tutorial_simulation_v92.py")
sim = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sim)
spec = importlib.util.spec_from_file_location("qualification_v92", ROOT / "scripts/qualify-tutorial-current-simulation-v92.py")
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


def fixture():
    request = {"schema": "fe2o3-simulation-request-v1", "kernel": "fill", "grid": [1, 1, 1],
               "workgroup": [1, 1, 1], "arguments": [{"kind": "buffer", "element": "u32",
               "access": "read_write", "alignment": 4, "bytes": "0x00000000"},
               {"kind": "scalar", "type": "u32", "bits": "0x00000025"}]}
    raw = json.dumps(request).encode()
    reference = {"schema": "fe2o3-tutorial-reference-result-v92", "kernel": "fill",
                 "request_sha256": list(hashlib.sha256(raw).digest()), "authority": False,
                 "outputs": [{"argument": 0, "element": "u32", "bytes": "0x25000000",
                 "absolute_tolerance": 0, "relative_tolerance": 0}]}
    graph = {"sha256": "a" * 64, "canonical_bytes": 42}
    result = {"schema": "fe2o3-simulation-result-v1", "status": "ok", "authority": "observation_only",
              "simulated": True, "hardware_observed": False, "hardware_validation": False,
              "performance_prediction": False, "kir": graph, "shared_buffers": [],
              "target_profile": {"identity": "amdgpu_gfx942_little_endian_v2", "index_bits": 64,
                                 "max_workgroup_invocations": 1024},
              "conflict_assessment": {"status": "no_conflicts_observed"},
              "schedule": {"coverage": {"complete": True}}, "arguments": [
              {"kind": "buffer", "value": {"element": "u32", "access": "read_write", "alignment": 4,
                                            "bytes": "0x25000000", "initialized": "0x0f"}},
              request["arguments"][1]]}
    return raw, reference, result, graph


class CurrentSimulation(unittest.TestCase):
    def test_actual_command_binds_cpu_to_existing_complete_production_target(self):
        import re
        source = (ROOT / "crates/fe2o3-amd-target/src/lib.rs").read_text()
        for cpu in ["gfx942", "gfx950"]:
            name = f"PRODUCTION_{cpu.upper()}_DEVICE_TARGET_V1"
            target, = re.findall(rf'pub const {name}: &str = "([^"]+)";', source)
            command = sim.simulation_command("sim", "graph", "request", "output", cpu)
            self.assertEqual(command, ["sim", "--diagnostic-kir-v18", "graph",
                                      "--diagnostic-target", target, "--request", "request",
                                      "--output", "output"])
        for cpu in ["gfx999", "gfx942:xnack+", "gfx942:xnack-", "gfx950:xnack-", "gfx942 ", None]:
            with self.assertRaises(ValueError):
                sim.simulation_command("sim", "graph", "request", "output", cpu)

    def test_same_request_complete_bytes_and_target_pass(self):
        raw, reference, result, graph = fixture()
        checked = sim.compare(raw, reference, result, graph, "gfx942")
        self.assertEqual(checked["checkedBufferBytes"], 4)
        self.assertFalse(checked["authority"])
        self.assertFalse(checked["hardwareExecuted"])

    def test_reference_and_simulator_substitutions_refuse(self):
        mutations = [lambda q, r: q.update(request_sha256=[0]*32),
            lambda q, r: q.update(kernel="other"), lambda q, r: q.update(authority=True),
            lambda q, r: q["outputs"].clear(),
            lambda q, r: q["outputs"].append(copy.deepcopy(q["outputs"][0])),
            lambda q, r: q["outputs"][0].update(absolute_tolerance=1),
            lambda q, r: r.update(hardware_observed=True),
            lambda q, r: r["kir"].update(sha256="b"*64),
            lambda q, r: r["target_profile"].update(identity="amdgpu_gfx950_little_endian_v2"),
            lambda q, r: r["schedule"]["coverage"].update(complete=False),
            lambda q, r: r["conflict_assessment"].update(status="conflict"),
            lambda q, r: r["arguments"][0]["value"].update(bytes="0x26000000"),
            lambda q, r: r["arguments"][0]["value"].update(initialized="0x07"),
            lambda q, r: r["arguments"][0]["value"].update(alignment=8),
            lambda q, r: r["arguments"][1].update(bits="0x00000026")]
        for mutate in mutations:
            raw, reference, result, graph = fixture()
            graph = copy.deepcopy(graph)
            mutate(reference, result)
            with self.assertRaises(ValueError):
                sim.compare(raw, reference, result, graph, "gfx942")

    def test_readonly_and_output_padding_remain_observed(self):
        raw, reference, result, graph = fixture()
        request = json.loads(raw)
        request["arguments"][0]["bytes"] += "efbeadde"
        result["arguments"][0]["value"].update(bytes="0x25000000efbeadde", initialized="0xff")
        reference["outputs"][0]["bytes"] += "efbeadde"
        raw = json.dumps(request).encode()
        reference["request_sha256"] = list(hashlib.sha256(raw).digest())
        sim.compare(raw, reference, result, graph, "gfx942")
        result["arguments"][0]["value"]["bytes"] = "0x2500000000000000"
        with self.assertRaises(ValueError):
            sim.compare(raw, reference, result, graph, "gfx942")

    def test_command_requires_eof_reap_and_actual_zero(self):
        success = {"status": "cargo-completed-unqualified", "exitCode": 0,
                   "logComplete": True, "directChildReaped": True}
        self.assertTrue(sim.command_passed(success))
        for key, value in [("status", "timeout"), ("exitCode", 1), ("logComplete", False),
                           ("directChildReaped", False)]:
            self.assertFalse(sim.command_passed(dict(success, **{key: value})))

    def test_special_files_and_artifact_census_refuse(self):
        import os
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            os.mkfifo(root / "fifo")
            with self.assertRaises(ValueError):
                sim.read_file(root / "fifo")
            with self.assertRaises(ValueError):
                gate.check_no_artifacts(root)
            (root / "fifo").unlink()
            (root / "ordinary.rlib").write_bytes(b"fixture")
            self.assertEqual(gate.check_no_artifacts(root)["entriesInspected"], 1)
            (root / "actual.hsaco").write_bytes(b"fixture")
            with self.assertRaises(ValueError):
                gate.check_no_artifacts(root)

    def test_current_manifest_corpus_keeps_all_invocations_and_negatives(self):
        census = gate.gate.load_module("census_v92_test", ROOT / "scripts/tutorial_source_census.py")
        manifest = json.loads((ROOT / "config/tutorial-kernel-manifest-v1.json").read_bytes())
        rows = census.registered_invocations(manifest)
        self.assertEqual(len(rows), 64)
        self.assertEqual(sum(row[2] for row in rows.values()), 3)
        self.assertEqual(len({name for fixture, _, neg in rows.values() if not neg
                              for name in fixture["compilerInput"]["kernelSymbols"]}), 45)
        for fixture, refs, neg in rows.values():
            if neg:
                self.assertEqual(gate.expectation(manifest, {"references": refs})["outputArtifact"], "absent")


if __name__ == "__main__":
    unittest.main()
