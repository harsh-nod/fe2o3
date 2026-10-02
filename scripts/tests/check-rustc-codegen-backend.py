#!/usr/bin/env python3
"""Regression tests for the production backend build/load gate."""

import copy
import contextlib
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


REPO = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("backend_gate", REPO / "scripts/check-rustc-codegen-backend.py")
gate = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(gate)


def elf_image():
    data = bytearray(128)
    data[:7] = b"\x7fELF\x02\x01\x01"
    struct.pack_into("<HHIQQQIHHH", data, 16, 3, 62, 1, 0, 64, 0, 0, 64, 56, 1)
    return bytes(data)


class BackendGateTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="fe2o3-backend-test-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.target_dir = self.root / "custom target"
        self.target_dir.mkdir()
        self.backend = self.target_dir / "librustc_codegen_fe2o3.so"
        self.backend.write_bytes(elf_image())
        self.receipt = self.root / "cargo.jsonl"
        self.target = {"name": gate.TARGET, "kind": ["rlib", "dylib"],
                       "crate_types": ["rlib", "dylib"], "test": True,
                       "src_path": str(self.root / "crates" / gate.PACKAGE / "src/lib.rs")}
        self.metadata = {"target_directory": str(self.target_dir), "packages": [
            {"id": "path+file:///selected#rustc-codegen-fe2o3@0.1.0", "name": gate.PACKAGE,
             "manifest_path": str(self.root / "crates" / gate.PACKAGE / "Cargo.toml"),
             "targets": [self.target]}]}
        self.artifact = {"reason": "compiler-artifact", "package_id": self.metadata["packages"][0]["id"],
                         "target": self.target, "features": [], "fresh": True, "executable": None,
                         "profile": {"test": False, "debuginfo": 1, "opt_level": "0",
                                     "debug_assertions": True, "overflow_checks": True},
                         "filenames": [str(self.target_dir / "backend.rlib"), str(self.backend)]}
        self.finished = {"reason": "build-finished", "success": True}

    def write_receipt(self, records):
        self.receipt.write_text("".join(json.dumps(r) + "\n" for r in records))

    def select(self):
        return gate.cargo_artifact(self.receipt, gate.package_identity(self.metadata, self.root))

    def inspect(self):
        return gate.inspect_backend(self.backend, "rustc", self.root, dict(os.environ), self.root)

    def test_fresh_artifact_with_rlib_and_custom_target(self):
        binary = {"reason": "compiler-artifact", "target": {"name": "fe2o3-export-sim"}}
        self.write_receipt([binary, self.artifact, self.finished])
        self.assertEqual(self.select(), self.backend)

    def test_package_metadata_binding(self):
        original = copy.deepcopy(self.metadata)
        for change in ("duplicate", "manifest", "source", "missing_dylib", "relative_target"):
            with self.subTest(change=change):
                self.metadata = copy.deepcopy(original)
                package = self.metadata["packages"][0]
                if change == "duplicate":
                    self.metadata["packages"].append(package)
                elif change == "manifest":
                    package["manifest_path"] = "/other/Cargo.toml"
                elif change == "source":
                    package["targets"][0]["src_path"] = "/other/lib.rs"
                elif change == "missing_dylib":
                    package["targets"][0]["kind"] = ["rlib"]
                else:
                    self.metadata["target_directory"] = "relative"
                with self.assertRaises(gate.CheckError):
                    gate.package_identity(self.metadata, self.root)

    def test_artifact_identity_and_profile_refusals(self):
        for field, value in (("package_id", "other"), ("target", {}),
                             ("profile", {"test": True}), ("filenames", []),
                             ("filenames", [str(self.backend), str(self.backend)]),
                             ("filenames", ["/other/librustc_codegen_fe2o3.so"])):
            with self.subTest(field=field, value=value):
                artifact = copy.deepcopy(self.artifact)
                artifact[field] = value
                if field == "target":
                    artifact[field] = {**self.target, "src_path": "/other/lib.rs"}
                self.write_receipt([artifact, self.finished])
                with self.assertRaises(gate.CheckError):
                    self.select()

    def test_terminal_and_ambiguity_refusals(self):
        for records in ([], [self.artifact], [self.finished],
                        [self.artifact, self.artifact, self.finished],
                        [self.artifact, {"reason": "build-finished", "success": False}],
                        [self.artifact, self.finished, self.finished],
                        [self.finished, self.artifact]):
            with self.subTest(records=records):
                self.write_receipt(records)
                with self.assertRaises(gate.CheckError):
                    self.select()

    def test_malformed_and_bounded_receipts(self):
        self.receipt.write_text("not-json\n")
        with self.assertRaises(ValueError):
            self.select()
        self.write_receipt([self.artifact, self.finished])
        for limit in ("MAX_RECEIPT_BYTES", "MAX_RECORD_BYTES"):
            with self.subTest(limit=limit), mock.patch.object(gate, limit, 10):
                with self.assertRaises(gate.CheckError):
                    self.select()

    def test_bounded_elf_and_success_report(self):
        with mock.patch.object(gate, "load_probe") as loader:
            result = self.inspect()
        loader.assert_called_once()
        self.assertEqual(result["sha256"], hashlib.sha256(elf_image()).hexdigest())
        self.assertEqual(result["metadataLoad"], "passed")
        self.assertEqual(result["headroomBytes"], gate.MAX_BACKEND_BYTES - 128)

    def test_runtime_bound_agrees(self):
        runtime = REPO / "crates/cargo-fe2o3/src/rustc_wrapper/pinned_codegen_backend.rs"
        self.assertIn("MAX_CODEGEN_BACKEND_BYTES: u64 = 1024 * 1024 * 1024;", runtime.read_text())
        self.assertEqual(gate.MAX_BACKEND_BYTES, 1024 * 1024 * 1024)

    def test_invalid_file_shapes_never_load(self):
        with mock.patch.object(gate, "load_probe") as loader:
            for shape in ("missing", "empty", "symlink", "directory", "fifo", "oversized"):
                with self.subTest(shape=shape):
                    self.backend.unlink(missing_ok=True)
                    if shape == "empty":
                        self.backend.touch()
                    elif shape == "symlink":
                        self.backend.symlink_to(self.receipt)
                    elif shape == "directory":
                        self.backend.mkdir()
                    elif shape == "fifo":
                        os.mkfifo(self.backend)
                    elif shape == "oversized":
                        with self.backend.open("wb") as stream:
                            stream.truncate(gate.MAX_BACKEND_BYTES + 1)
                    with self.assertRaises((gate.CheckError, OSError)):
                        self.inspect()
                    if self.backend.is_dir():
                        self.backend.rmdir()
            loader.assert_not_called()

    def test_invalid_elf_never_loads(self):
        for offset, value in ((0, 0), (4, 1), (5, 2), (6, 0), (16, 2),
                              (20, 0), (32, 127), (52, 1), (54, 0), (56, 0)):
            with self.subTest(offset=offset), mock.patch.object(gate, "load_probe") as loader:
                data = bytearray(elf_image())
                data[offset] = value
                self.backend.write_bytes(data)
                with self.assertRaises(gate.CheckError):
                    self.inspect()
                loader.assert_not_called()

    def test_mutation_and_replacement_during_load(self):
        def mutate(*_):
            data = bytearray(self.backend.read_bytes())
            data[-1] ^= 1
            self.backend.write_bytes(data)

        def replace(*_):
            replacement = self.root / "replacement"
            replacement.write_bytes(elf_image())
            replacement.replace(self.backend)

        for action in (mutate, replace):
            with self.subTest(action=action), mock.patch.object(gate, "load_probe", side_effect=action):
                self.backend.write_bytes(elf_image())
                with self.assertRaisesRegex(gate.CheckError, "changed"):
                    self.inspect()

    def test_probe_descriptor_spelling_environment_and_failure(self):
        environment = {**os.environ, "RUSTFLAGS": "bad", "CARGO_ENCODED_RUSTFLAGS": "bad",
                       "RUSTC_WRAPPER": "bad", "RUSTC_WORKSPACE_WRAPPER": "bad"}
        with self.backend.open("rb") as stream:
            fd = stream.fileno()
            for status in (0, 37, 124):
                with self.subTest(status=status), mock.patch.object(gate.subprocess, "run") as run:
                    run.return_value.returncode = status
                    if status:
                        with self.assertRaisesRegex(gate.CheckError, f"status {status}"):
                            gate.load_probe(fd, "/selected/rustc", self.root, environment, self.root)
                    else:
                        gate.load_probe(fd, "/selected/rustc", self.root, environment, self.root)
                    args, kwargs = run.call_args
                    self.assertIn(f"-Zcodegen-backend=/proc/./self/fd/{fd}", args[0])
                    self.assertIn("/selected/rustc", args[0])
                    self.assertEqual(kwargs["pass_fds"], (fd,))
                    self.assertEqual(kwargs["cwd"], self.root)
                    for key in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"):
                        self.assertNotIn(key, kwargs["env"])

    def test_real_loader_rejects_shared_object_without_backend_factory(self):
        source = self.root / "not_backend.rs"
        source.write_text("pub fn not_a_backend() {}\n")
        rustc = shutil.which(os.environ.get("RUSTC", "rustc"))
        self.assertIsNotNone(rustc)
        subprocess.run([rustc, "--crate-type=cdylib", "-Cdebuginfo=0", str(source),
                        "-o", str(self.backend)], cwd=REPO, check=True, timeout=60)
        diagnostic = io.StringIO()
        with contextlib.redirect_stderr(diagnostic):
            with self.assertRaisesRegex(gate.CheckError, "load probe failed"):
                gate.inspect_backend(self.backend, rustc, REPO, dict(os.environ), self.root)
        self.assertIn("__rustc_codegen_backend", diagnostic.getvalue())

    def test_real_probe_timeout(self):
        compiler = self.root / "slow-rustc"
        compiler.write_text("#!/bin/sh\nexec sleep 30\n")
        compiler.chmod(0o700)
        with self.backend.open("rb") as stream, mock.patch.object(gate, "LOAD_TIMEOUT_SECONDS", 1):
            with self.assertRaisesRegex(gate.CheckError, "status 124"):
                gate.load_probe(stream.fileno(), str(compiler), self.root, dict(os.environ), self.root)

    def test_builds_pin_same_compiler_and_check_each_actual_artifact(self):
        def cargo(command, **kwargs):
            self.assertEqual(kwargs["cwd"], self.root)
            environment = kwargs["env"]
            self.assertEqual(environment["RUSTC"], "/selected/rustc")
            self.assertEqual(environment["RUSTC_WRAPPER"], "")
            self.assertEqual(environment["RUSTC_WORKSPACE_WRAPPER"], "")
            self.assertEqual(environment["CARGO_INCREMENTAL"], "0")
            self.assertEqual(environment["CARGO_PROFILE_DEV_DEBUG"], "1")
            if command[1] == "metadata":
                kwargs["stdout"].write(json.dumps(self.metadata).encode())
            else:
                self.assertEqual("--all-features" in command, self.all_features)
                self.assertNotIn("--lib", command)
                kwargs["stdout"].write((json.dumps(self.artifact) + "\n" +
                                        json.dumps(self.finished) + "\n").encode())
            return subprocess.CompletedProcess(command, 0)

        with mock.patch.object(gate.shutil, "which", return_value="/selected/rustc"), \
                mock.patch.object(gate.subprocess, "run", side_effect=cargo), \
                mock.patch.object(gate, "inspect_backend", return_value={}) as inspect:
            for self.all_features in (False, True):
                gate.build_and_check(self.root, self.all_features)
                self.assertEqual(inspect.call_args.args[:3], (self.backend, "/selected/rustc", self.root))
            self.assertEqual(inspect.call_count, 2)

    def test_failed_cargo_does_not_inspect_or_report_success(self):
        def failed_build(command, **kwargs):
            if command[1] == "metadata":
                kwargs["stdout"].write(json.dumps(self.metadata).encode())
                return subprocess.CompletedProcess(command, 0)
            raise subprocess.CalledProcessError(37, command)

        with mock.patch.object(gate.subprocess, "run", side_effect=failed_build) as run, \
                mock.patch.object(gate, "inspect_backend") as inspect:
            with self.assertRaises(subprocess.CalledProcessError):
                gate.build_and_check(self.root, False)
            self.assertEqual(run.call_count, 2)
            inspect.assert_not_called()


if __name__ == "__main__":
    unittest.main()
