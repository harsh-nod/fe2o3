#!/usr/bin/env python3
"""Synthetic parser/custody controls, not production compiler evidence."""
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("production_census", ROOT / "scripts/tutorial_production_census_v91.py")
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)


def row():
    values = {name: [i+1]*32 for i, name in enumerate(m.HASH_FIELDS)}
    values.update(schema=m.KIND, session=[31]*16, generation=3, target="gfx942",
                  descriptor_version=89, typed_receipt_version=90, policy=11,
                  compiler=[[41]*32]*6, graphs=[[[42]*32, 53]]*4,
                  proof_tools=[[43]*32]*5, artifact_bytes=70,
                  kernels=[dict(logical_name="kernel", entry_name="kernel.entry", root=0,
                                output_function=1, contract=[44]*32)], authority=False)
    return {name: values[name] for name in m.FIELDS}


def expected(value):
    return dict(config=value["config"], units=[value["unit"]], compiler=value["compiler"],
                target=value["target"], symbols=["kernel"], pins=[])


def log(rows):
    return (m.PREFIX + f"frames={len(rows)} missing=0 failure=0 encoding=hex:".encode()
            + m.canonical(rows).hex().encode() + b" authority=observation-only\n")


class ProductionCensusTests(unittest.TestCase):
    def test_complete_positive_remains_observation_only(self):
        value = row()
        result = m.check_log(log([value]), expected(value))
        self.assertEqual(result["rows"], [value])
        self.assertEqual(result["status"], "production-compile-census-pass")
        self.assertFalse(result["authority"])

    def test_each_identity_census_and_schema_substitution_refuses(self):
        original = row()
        mutations = [lambda x: x.update(schema="source-isa-summary-v1"),
                     lambda x: x.update(descriptor_version=53),
                     lambda x: x.update(typed_receipt_version=50),
                     lambda x: x.update(policy=10), lambda x: x.update(authority=True),
                     lambda x: x.update(config=[61]*32), lambda x: x.update(unit=[62]*32),
                     lambda x: x.update(target="gfx950"), lambda x: x["compiler"][3].__setitem__(0, 9),
                     lambda x: x.update(session=[0]*16), lambda x: x.update(generation=True),
                     lambda x: x.update(graphs=x["graphs"][:3]),
                     lambda x: x.update(proof_tools=x["proof_tools"][:4]),
                     lambda x: x["kernels"][0].update(logical_name="another"),
                     lambda x: x["kernels"][0].update(root=1),
                     lambda x: x["kernels"].append(copy.deepcopy(x["kernels"][0])),
                     lambda x: x.update(artifact_bytes=0), lambda x: x.update(artifact_bytes=m.MAX_INPUT+1),
                     lambda x: x.update(unknown=0)]
        for ordinal, change in enumerate(mutations):
            value = copy.deepcopy(original)
            change(value)
            with self.subTest(ordinal=ordinal), self.assertRaises(ValueError):
                m.check_log(log([value]), expected(original))
        for field in m.HASH_FIELDS:
            value = copy.deepcopy(original)
            value[field] = [0]*32
            with self.subTest(field=field), self.assertRaises(ValueError):
                m.check_log(log([value]), expected(original))

    def test_missing_duplicate_truncated_extra_and_noncanonical_log_refuse(self):
        value = row()
        good = log([value])
        for changed in [b"", good+good, good[:-1][:-30], good.replace(b"missing=0", b"missing=1"),
                        good.replace(b"failure=0", b"failure=1"), log([value, value]),
                        good.replace(b"frames=1", b"frames=2"),
                        good.replace(m.canonical([value]).hex().encode(),
                                     json.dumps([value], indent=2).encode().hex().encode())]:
            with self.subTest(changed=changed[:50]), self.assertRaises(ValueError):
                m.check_log(changed, expected(value))

    def test_fd_hashing_refuses_symlink_substitution_and_exact_pin_changes(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "file"
            path.write_bytes(b"original")
            _, pin = m.read_file(path)
            m.check_pins({"pins": [pin]})
            path.write_bytes(b"modified")
            with self.assertRaises(ValueError):
                m.check_pins({"pins": [pin]})
            link = Path(directory) / "link"
            link.symlink_to(path)
            with self.assertRaises(OSError):
                m.read_file(link)
            with self.assertRaises(ValueError):
                m.read_file(path, 1)

    def test_success_requires_exact_recorded_log_and_reaped_command(self):
        value = row()
        data = log([value])
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "log"
            path.write_bytes(data)
            outcome = dict(exitCode=0, logComplete=True, directChildReaped=True,
                           logSha256=hashlib.sha256(data).hexdigest(), logBytes=len(data))
            m.reconcile(path, outcome, expected(value))
            for key, bad in [("exitCode", 1), ("logComplete", False), ("directChildReaped", False),
                             ("logSha256", "0"*64), ("logBytes", len(data)-1)]:
                with self.subTest(key=key), self.assertRaises(ValueError):
                    m.reconcile(path, {**outcome, key: bad}, expected(value))

    def test_config_and_compiler_pins_are_derived_from_actual_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            outputs = root / "out"
            outputs.mkdir()
            package = root / "example"
            (package / "src").mkdir(parents=True)
            (package / "src/lib.rs").write_bytes(b"source")
            executable = root / "tool"
            executable.write_bytes(b"test-only-image")
            executable.chmod(0o700)
            digest = hashlib.sha256(executable.read_bytes()).hexdigest()
            template = dict(format="fe2o3-production-build-config-v1", units=[], providers=[],
                            worker=dict(path=str(executable), sha256=digest, byte_len=15,
                                        worker_build_identity="11"*32, llvm_build_identity="22"*32),
                            candidate_output_max_bytes=1000, limits={}, link_options=[])
            template_path = root / "template.json"
            template_path.write_bytes(m.canonical(template))
            env = {"FE2O3_PRODUCTION_BUILD_CONFIG_V1": str(template_path),
                   "CARGO": str(executable), "FE2O3_BACKEND": str(executable),
                   "FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_PATH_V1": str(executable),
                   "FE2O3_AUTHORITY_RUSTC_PATH_V1": str(executable),
                   "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1": "33"*32}
            for name in ["CARGO", "BACKEND", "RUSTC", "CARGO_BINDING_TRAMPOLINE"]:
                env[f"FE2O3_AUTHORITY_{name}_SHA256_V1"] = digest
            fixture = dict(target="gfx942", compilerInput=dict(packageManifest="example/Cargo.toml",
                cargoLockPath="Cargo.lock", cargoTarget=dict(name="example", sourcePath="src/lib.rs"), kernelSymbols=["kernel"]))
            child, pins = m.prepare(root, fixture, executable, env, outputs, 0)
            self.assertNotIn("FE2O3_PRODUCTION_BUILD_CONFIG_V1", child)
            self.assertEqual(pins["selector"], dict(crate_name="example", source="example/src/lib.rs", working_directory=str(root)))
            actual = m.decode(Path(child[m.CONFIGS[1]]).read_bytes())
            self.assertEqual(actual["observation"], {"kind": m.KIND})
            for key in ("worker", "providers", "limits", "link_options", "candidate_output_max_bytes"):
                self.assertEqual(actual[key], template[key])
            m.check_pins(pins)
            self.assertEqual(child["FE2O3_PRODUCTION_BUILD_EXPECTED_ID_V2"], bytes(pins["config"]).hex())
            executable.write_bytes(b"changed")
            with self.assertRaises(ValueError):
                m.check_pins(pins)


if __name__ == "__main__":
    unittest.main()
