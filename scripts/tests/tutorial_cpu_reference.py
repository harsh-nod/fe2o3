#!/usr/bin/env python3
"""Component-only CPU adapter tests; doubles never qualify a tutorial suite."""

from __future__ import annotations

import copy
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import sys
import tempfile
import time
from types import SimpleNamespace
import unittest
from unittest.mock import patch


HELPER = Path(__file__).resolve().parents[1] / "tutorial_cpu_reference.py"
SPEC = importlib.util.spec_from_file_location("tutorial_cpu_reference", HELPER)
assert SPEC is not None and SPEC.loader is not None
adapter = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(adapter)


def source_manifest(arguments: list[str]) -> dict:
    return {
        "qualification": {"suites": [{
            "suiteId": "cpu-reference-fixture", "gate": "cpu-reference",
            "command": {"executable": adapter.WRAPPER, "arguments": arguments,
                        "workingDirectory": ".", "environment": [], "timeoutSeconds": 1200},
            "coverage": [{"lessonId": "fixture", "fixtureIds": ["fixture-a", "fixture-b"]}],
        }]},
        "compilerFixtures": [
            {"fixtureId": "fixture-a", "compilerInput": {"packageManifest": arguments[0], "features": ["kernel-a"]}},
            {"fixtureId": "fixture-b", "compilerInput": {"packageManifest": arguments[0], "features": ["kernel-b"]}},
        ],
    }


class AdapterComponents(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="cpu-reference-components-")
        self.root = Path(self.temporary.name).resolve()
        self.target = self.root / "build"
        self.target.mkdir()
        self.executable = self.target / "fixture"
        self.executable.write_bytes(b"component artifact; not executed")
        self.executable.chmod(0o700)
        self.source = self.root / "src/lib.rs"
        self.source.parent.mkdir()
        self.source.write_text("#[test] fn component() {}\n")
        self.manifest = self.root / "Cargo.toml"
        self.manifest.write_text('[package]\nname="fixture"\nversion="0.1.0"\n')
        self.expected = {"packageId": "fixture#0.1.0", "name": "fixture", "kind": ["lib"],
                         "crateTypes": ["lib"], "features": [], "source": str(self.source)}

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def events(self, states: list[str] | None = None) -> list[dict]:
        states = ["ok"] if states is None else states
        events = [{"reason": "compiler-artifact", "package_id": self.expected["packageId"],
                   "target": {"name": "fixture", "kind": ["lib"], "crate_types": ["lib"],
                              "src_path": str(self.source)}, "features": [], "profile": {"test": True},
                   "executable": str(self.executable)},
                  {"reason": "build-finished", "success": True},
                  {"type": "suite", "event": "started", "test_count": len(states)}]
        for index, state in enumerate(states):
            events.append({"type": "test", "event": "started", "name": f"test_{index}"})
            events.append({"type": "test", "event": state, "name": f"test_{index}"})
        events.append({"type": "suite", "event": "failed" if "failed" in states else "ok",
                       "passed": states.count("ok"), "failed": states.count("failed"),
                       "ignored": states.count("ignored"), "measured": 0, "filtered_out": 0})
        return events

    def observe(self, events: list[dict]) -> dict:
        payload = b"\n".join(json.dumps(event).encode() for event in events) + b"\n"
        return adapter.test_observation(payload, self.expected, self.target)

    def metadata(self, kind: str = "lib") -> dict:
        return {"packages": [{"id": self.expected["packageId"], "manifest_path": str(self.manifest),
                              "targets": [{"name": "fixture", "kind": [kind], "crate_types": [kind],
                                           "src_path": str(self.source), "test": True}]}],
                "resolve": {"nodes": [{"id": self.expected["packageId"], "features": []}]}}

    def test_literal_argument_planning_preserves_default_host_features(self) -> None:
        for arguments in (["examples/fixture/Cargo.toml", "lib"],
                          ["examples/fixture/Cargo.toml", "test", "reference"]):
            manifest = source_manifest(arguments)
            plan = adapter.select_suite(manifest, arguments)
            self.assertEqual(plan["hostConfiguration"]["features"], [])
            self.assertTrue(plan["hostConfiguration"]["defaultFeatures"])
            self.assertEqual(plan["hostConfiguration"]["selector"], arguments[1:])
            self.assertEqual(len(plan["fixtureContracts"]), 2)
            self.assertEqual(manifest, source_manifest(arguments))

    def test_all_declared_cpu_suites_have_exact_component_plans(self) -> None:
        source = Path(__file__).resolve().parents[2] / "config/tutorial-kernel-manifest-v1.json"
        payload = source.read_bytes()
        manifest = adapter.decode_json(payload)
        suites = [suite for suite in manifest["qualification"]["suites"] if suite["gate"] == "cpu-reference"]
        self.assertEqual(len(suites), 12)
        for suite in suites:
            plan = adapter.select_suite(manifest, suite["command"]["arguments"])
            self.assertEqual(plan["suiteId"], suite["suiteId"])
            self.assertEqual(plan["hostConfiguration"]["features"], [])
        self.assertEqual(source.read_bytes(), payload)

    def test_source_loader_uses_the_existing_validator_before_selecting(self) -> None:
        scripts = self.root / "scripts"
        scripts.mkdir()
        config = self.root / "config"
        config.mkdir()
        arguments = ["Cargo.toml", "lib"]
        (config / "tutorial-kernel-manifest-v1.json").write_text(json.dumps(source_manifest(arguments)))
        validator = scripts / "validate-tutorial-kernel-manifest.py"
        validator.write_text("def validate_manifest(root, manifest):\n    raise SystemExit('fixture refuses')\n")
        with self.assertRaisesRegex(adapter.ObservationError, "fixture refuses"):
            adapter.load_plan(self.root, arguments)
        validator.write_text("def validate_manifest(root, manifest):\n    assert manifest['qualification']['suites']\n")
        plan, _ = adapter.load_plan(self.root, arguments)
        self.assertEqual(plan["suiteId"], "cpu-reference-fixture")
        self.assertIn("validatorSha256", plan)

    def test_wrong_ambiguous_injected_and_foreign_suite_refuse(self) -> None:
        arguments = ["examples/fixture/Cargo.toml", "lib"]
        for mutation in ("duplicate", "executable", "gate", "environment", "directory", "timeout", "foreign"):
            manifest = source_manifest(arguments)
            suite = manifest["qualification"]["suites"][0]
            if mutation == "duplicate":
                manifest["qualification"]["suites"].append(copy.deepcopy(suite))
            elif mutation == "foreign":
                manifest["compilerFixtures"][0]["compilerInput"]["packageManifest"] = "other/Cargo.toml"
            elif mutation == "gate":
                suite["gate"] = "semantic-simulation"
            else:
                key, value = {"executable": ("executable", "sh"), "environment": ("environment", ["RUSTFLAGS=hostile"]),
                              "directory": ("workingDirectory", ".."), "timeout": ("timeoutSeconds", True)}[mutation]
                suite["command"][key] = value
            with self.subTest(mutation=mutation), self.assertRaises(adapter.ObservationError):
                adapter.select_suite(manifest, arguments)
        for extra in (["--ignored"], [";", "touch", "marker"], ["test", "*"]):
            with self.assertRaises(adapter.ObservationError):
                adapter.select_suite(source_manifest(arguments), arguments + extra)

    def test_standard_library_and_auto_integration_harness_identity(self) -> None:
        selected = adapter.selected_target(self.root, self.metadata(), self.manifest, ["lib"])
        self.assertEqual(selected["source"], str(self.source))
        self.source = self.root / "tests/fixture.rs"
        self.source.parent.mkdir()
        self.source.write_text("#[test] fn reference() {}")
        selected = adapter.selected_target(self.root, self.metadata("test"), self.manifest, ["test", "fixture"])
        self.assertEqual(selected["kind"], ["test"])

    def test_custom_disabled_wrong_and_unbound_target_harnesses_refuse(self) -> None:
        for text in ('[lib]\nharness=false\n', '[lib]\ntest=false\n'):
            self.manifest.write_text('[package]\nname="fixture"\nversion="0.1.0"\n' + text)
            with self.assertRaises(adapter.ObservationError):
                adapter.selected_target(self.root, self.metadata(), self.manifest, ["lib"])
        self.manifest.write_text('[package]\nname="fixture"\nversion="0.1.0"\n')
        for field, value in (("src_path", "/proc/self/fd/7/lib.rs"), ("test", False), ("kind", ["bin"])):
            metadata = self.metadata()
            metadata["packages"][0]["targets"][0][field] = value
            with self.assertRaises((adapter.ObservationError, OSError)):
                adapter.selected_target(self.root, metadata, self.manifest, ["lib"])

    def test_pass_failure_ignore_and_empty_remain_distinct(self) -> None:
        for states, result in ((["ok", "ok"], "passed"), (["failed"], "failed"),
                               (["ok", "ignored"], "partial"), ([], "empty")):
            self.assertEqual(self.observe(self.events(states))["outcome"], result)
        self.assertTrue(all(value is False for value in adapter.NO_AUTHORITY.values()))
        observed = self.observe(self.events())["executable"]
        self.assertEqual(observed["digestScope"], "post-execution-on-disk-artifact-only")
        self.assertEqual(set(observed), {"path", "observedSha256", "digestScope"})

    def test_artifact_observation_uses_existing_runner_bound_not_data_bounds(self) -> None:
        self.assertEqual(adapter.MAX_EXECUTABLE_BYTES, 512 * 1024 * 1024)
        self.assertEqual(adapter.ARTIFACT_HASH_CHUNK, 64 * 1024)
        self.assertEqual(adapter.MAX_FILE, 16 * 1024 * 1024)
        self.assertEqual(adapter.MAX_INPUT_BYTES, 256 * 1024 * 1024)
        self.assertEqual(adapter.MAX_STREAM, 16 * 1024 * 1024)
        # Scale only the constants in this component: no large allocation or
        # real test executable is needed to exercise the distinct bound.
        payload = b"artifact above both data limits"
        self.executable.write_bytes(payload)
        with patch.object(adapter, "MAX_EXECUTABLE_BYTES", 32), \
                patch.object(adapter, "ARTIFACT_HASH_CHUNK", 7), \
                patch.object(adapter, "MAX_INPUT_BYTES", 16), \
                patch.object(adapter, "MAX_FILE", 8):
            for limit in (adapter.MAX_FILE, adapter.MAX_INPUT_BYTES):
                with self.assertRaisesRegex(adapter.ObservationError, "invalid/big file"):
                    adapter.read_regular(self.executable, limit)
            observed = self.observe(self.events())
        self.assertEqual(observed["outcome"], "passed")
        self.assertEqual(observed["executable"]["observedSha256"], adapter.digest(payload))
        self.assertEqual(observed["executable"]["digestScope"], "post-execution-on-disk-artifact-only")
        self.assertTrue(all(value is False for value in adapter.NO_AUTHORITY.values()))

    def test_artifact_hash_empty_exact_and_over_limit_are_bounded(self) -> None:
        real_open = Path.open
        requests = []

        @contextlib.contextmanager
        def opened(path, *arguments, **keywords):
            with real_open(path, *arguments, **keywords) as stream:
                def read(size):
                    requests.append(size)
                    return stream.read(size)
                yield SimpleNamespace(read=read, fileno=stream.fileno)

        with patch.object(adapter, "MAX_EXECUTABLE_BYTES", 32), \
                patch.object(adapter, "ARTIFACT_HASH_CHUNK", 7):
            for size in (1, 31, 32):
                payload = b"x" * size
                self.executable.write_bytes(payload)
                requests.clear()
                with patch.object(Path, "open", opened):
                    observed = adapter.artifact_digest(self.executable)
                self.assertEqual(observed, adapter.digest(payload))
                self.assertTrue(requests)
                self.assertTrue(all(0 < size <= 7 for size in requests))
                self.assertLessEqual(sum(requests), len(payload) + 14)
            for size in (0, 33):
                self.executable.write_bytes(b"x" * size)
                with patch.object(Path, "open", side_effect=AssertionError("invalid file opened")), \
                        self.assertRaisesRegex(adapter.ObservationError, "invalid/big artifact"):
                    adapter.artifact_digest(self.executable)

    def test_artifact_hash_refuses_noncanonical_nonregular_and_substituted_open(self) -> None:
        alias = self.target / "alias"
        alias.symlink_to(self.executable)
        for path in (alias, Path("relative"), Path("/proc/self/fd/9"), self.target):
            with self.subTest(path=path), self.assertRaises(adapter.ObservationError):
                adapter.artifact_digest(path)
        other = self.target / "other"
        other.write_bytes(self.executable.read_bytes())
        real_open = Path.open
        opened = real_open(other, "rb")
        with patch.object(Path, "open", return_value=opened), \
                self.assertRaisesRegex(adapter.ObservationError, "changed while opening"):
            adapter.artifact_digest(self.executable)
        self.assertTrue(opened.closed)

    def test_artifact_hash_checks_every_opened_and_final_descriptor_field(self) -> None:
        real_fstat = os.fstat
        fields = ("st_dev", "st_ino", "st_mode", "st_size", "st_mtime_ns", "st_ctime_ns")
        for changed_call in (1, 2):
            for field in fields:
                calls = 0

                def altered(fd):
                    nonlocal calls
                    calls += 1
                    actual = real_fstat(fd)
                    values = {name: getattr(actual, name) for name in fields}
                    if calls == changed_call:
                        values[field] += 1
                    return SimpleNamespace(**values)

                with self.subTest(call=changed_call, field=field), \
                        patch.object(adapter.os, "fstat", side_effect=altered), \
                        self.assertRaisesRegex(adapter.ObservationError, "artifact changed"):
                    adapter.artifact_digest(self.executable)

    def test_artifact_hash_rejects_short_read_growth_mutation_and_path_drift(self) -> None:
        real_open = Path.open
        for mode in ("short", "growth", "overwrite", "replace", "read-error"):
            self.executable.write_bytes(b"abcdefgh")
            retained = []

            @contextlib.contextmanager
            def opened(path, *arguments, **keywords):
                with real_open(path, *arguments, **keywords) as stream:
                    retained.append(stream)
                    first = True

                    def read(size):
                        nonlocal first
                        if not first:
                            return stream.read(size)
                        first = False
                        if mode == "short":
                            return b""
                        if mode == "read-error":
                            raise OSError("injected artifact read failure")
                        data = stream.read(size)
                        if mode == "growth":
                            with real_open(path, "ab") as writer:
                                writer.write(b"extra")
                        elif mode == "overwrite":
                            with real_open(path, "r+b") as writer:
                                writer.write(b"X")
                            current = path.stat()
                            os.utime(path, ns=(current.st_atime_ns, current.st_mtime_ns + 1_000_000))
                        else:
                            path.unlink()
                            with real_open(path, "wb") as writer:
                                writer.write(b"abcdefgh")
                        return data

                    yield SimpleNamespace(read=read, fileno=stream.fileno)

            with self.subTest(mode=mode), patch.object(adapter, "MAX_EXECUTABLE_BYTES", 8), \
                    patch.object(adapter, "ARTIFACT_HASH_CHUNK", 3), \
                    patch.object(Path, "open", opened), \
                    self.assertRaises((adapter.ObservationError, OSError)):
                adapter.artifact_digest(self.executable)
            self.assertTrue(retained and all(stream.closed for stream in retained))

    def test_named_integration_accepts_only_exact_metadata_companion_bins(self) -> None:
        self.source = self.root / "tests/fixture.rs"
        self.source.parent.mkdir()
        self.source.write_text("#[test] fn reference() {}")
        companion_source = self.root / "src/main.rs"
        companion_source.write_text("fn main() {}")
        metadata = self.metadata("test")
        metadata["packages"][0]["targets"][0]["crate_types"] = ["bin"]
        target = {"name": "fixture-tool", "kind": ["bin"], "crate_types": ["bin"],
                  "src_path": str(companion_source), "test": True}
        metadata["packages"][0]["targets"].append(target)
        self.expected = adapter.selected_target(self.root, metadata, self.manifest, ["test", "fixture"])
        binary = self.target / "fixture-tool"
        binary.write_bytes(b"unexecuted companion artifact")
        binary.chmod(0o700)
        companion = {"reason": "compiler-artifact", "package_id": self.expected["packageId"],
                     "target": target, "features": [], "profile": {"test": False}, "executable": str(binary)}

        def events_with_companion():
            events = self.events()
            events[0]["target"].update(kind=["test"], crate_types=["bin"])
            events.insert(1, copy.deepcopy(companion))
            return events

        observed = self.observe(events_with_companion())
        self.assertEqual(observed["outcome"], "passed")
        self.assertEqual(observed["counts"]["passed"], 1)
        self.assertEqual(observed["companionArtifacts"]["fixture-tool"]["digestScope"],
                         "post-execution-on-disk-artifact-only")
        for mutation in ("package", "source", "name", "kind", "crate", "features", "profile", "duplicate", "overlap"):
            events = events_with_companion()
            item = events[1]
            if mutation == "package": item["package_id"] = "unrelated"
            elif mutation == "source": item["target"]["src_path"] = str(self.source)
            elif mutation == "name": item["target"]["name"] = "unrelated"
            elif mutation == "kind": item["target"]["kind"] = ["example"]
            elif mutation == "crate": item["target"]["crate_types"] = ["lib"]
            elif mutation == "features": item["features"] = ["extra"]
            elif mutation == "profile": item["profile"]["test"] = True
            elif mutation == "overlap": item["executable"] = str(self.executable)
            else: events.insert(2, copy.deepcopy(item))
            with self.subTest(mutation=mutation), self.assertRaises(adapter.ObservationError):
                self.observe(events)

    def test_all_artifact_identity_fields_and_extra_harnesses_are_checked(self) -> None:
        for mutation in ("package", "source", "kind", "crate", "name", "features", "profile", "duplicate", "alias"):
            events = self.events()
            artifact = events[0]
            if mutation == "package": artifact["package_id"] = "other"
            elif mutation == "source": artifact["target"]["src_path"] = str(self.root / "other.rs")
            elif mutation == "kind": artifact["target"]["kind"] = ["test"]
            elif mutation == "crate": artifact["target"]["crate_types"] = ["bin"]
            elif mutation == "name": artifact["target"]["name"] = "other"
            elif mutation == "features": artifact["features"] = ["unexpected"]
            elif mutation == "profile": artifact["profile"]["test"] = False
            elif mutation == "alias": artifact["executable"] = "/proc/self/fd/8/test"
            else: events.insert(1, copy.deepcopy(artifact))
            with self.subTest(mutation=mutation), self.assertRaises(adapter.ObservationError):
                self.observe(events)

    def test_event_completion_counts_and_selection_are_not_optional(self) -> None:
        for mutation in ("start", "terminal", "bool", "ignored", "filtered", "measured", "trailing", "extra-suite", "duplicate-test", "build"):
            events = self.events()
            if mutation == "start": del events[2]
            elif mutation == "terminal": events.pop()
            elif mutation == "bool": events[-1]["passed"] = True
            elif mutation == "ignored": events[-1]["ignored"] = 1
            elif mutation == "filtered": events[-1]["filtered_out"] = 1
            elif mutation == "measured": events[-1]["measured"] = 1
            elif mutation == "trailing": events.append(copy.deepcopy(events[-1]))
            elif mutation == "extra-suite": events.insert(3, copy.deepcopy(events[2]))
            elif mutation == "duplicate-test": events.insert(4, copy.deepcopy(events[3]))
            else: events[1]["success"] = False
            with self.subTest(mutation=mutation), self.assertRaises(adapter.ObservationError):
                self.observe(events)

    def test_strict_json_and_exact_stream_event_limits(self) -> None:
        for data in (b'{"a":1,"a":2}', b'{"a":NaN}', b'{"a":Infinity}', b'not json', b'[]'):
            with self.assertRaises(adapter.ObservationError):
                adapter.test_observation(data, self.expected, self.target)
        payload = b"\n".join(json.dumps(event).encode() for event in self.events()) + b"\n"
        with patch.object(adapter, "MAX_STREAM", len(payload)):
            self.assertEqual(adapter.test_observation(payload, self.expected, self.target)["outcome"], "passed")
        with patch.object(adapter, "MAX_STREAM", len(payload) - 1), self.assertRaises(adapter.ObservationError):
            adapter.test_observation(payload, self.expected, self.target)
        with patch.object(adapter, "MAX_EVENTS", len(self.events()) - 1), self.assertRaises(adapter.ObservationError):
            self.observe(self.events())

    def test_input_identity_change_symlink_and_exact_file_bound(self) -> None:
        before = adapter.footprint([], [self.source], time.monotonic() + 5)
        self.source.write_text("changed")
        self.assertNotEqual(before, adapter.footprint([], [self.source], time.monotonic() + 5))
        with patch.object(adapter, "MAX_FILES", 1):
            self.assertEqual(len(adapter.footprint([], [self.source], time.monotonic() + 5)), 1)
        with patch.object(adapter, "MAX_FILES", 0), self.assertRaises(adapter.ObservationError):
            adapter.footprint([], [self.source], time.monotonic() + 5)
        alias = self.root / "alias.rs"
        alias.symlink_to(self.source)
        with self.assertRaises(adapter.ObservationError):
            adapter.read_regular(alias)
        with self.assertRaises(adapter.ObservationError):
            adapter.relative_file(self.root, "../outside")

    def test_controlled_environment_does_not_accept_inherited_selection(self) -> None:
        for name in ("RUSTC", "RUSTC_BOOTSTRAP", "CARGO_PROFILE_TEST_OPT_LEVEL", "CARGO_TARGET_X_RUNNER",
                     "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER", "FE2O3_EXAMPLE_CARGO_ARGS", "LD_LIBRARY_PATH"):
            with patch.dict(os.environ, {name: "hostile"}, clear=True), self.assertRaises(adapter.ObservationError):
                adapter.environment()
        with patch.dict(os.environ, {"PATH": "/usr/bin", "FE2O3_TUTORIAL_CPU_OUTPUT_ROOT": str(self.root)}, clear=True):
            env = adapter.environment()
            self.assertNotIn("FE2O3_TUTORIAL_CPU_OUTPUT_ROOT", env)
            self.assertEqual(env["CARGO_NET_OFFLINE"], "true")

    def test_component_child_status_cap_deadline_and_cleanup(self) -> None:
        env = {"PATH": os.environ.get("PATH", "")}
        phases: list[dict] = []
        output = adapter.child([sys.executable, "-c", "print('component'); raise SystemExit(7)"],
                               self.root, env, self.root, "exit", time.monotonic() + 5, phases)
        self.assertEqual(output, b"component\n")
        self.assertEqual(phases[-1]["returncode"], 7)
        with patch.object(adapter, "MAX_STREAM", 3), self.assertRaises(adapter.ObservationError):
            adapter.child([sys.executable, "-c", "print('too much')"], self.root, env,
                          self.root, "cap", time.monotonic() + 5, phases)
        with self.assertRaises(adapter.ObservationError):
            adapter.child([sys.executable, "-c", "import time; time.sleep(10)"], self.root, env,
                          self.root, "deadline", time.monotonic() + 0.1, phases)
        self.assertIsNotNone(phases[-1]["returncode"])
        with self.assertRaises(FileExistsError):
            adapter.child([sys.executable, "-c", "print('no overwrite')"], self.root, env,
                          self.root, "exit", time.monotonic() + 5, phases)

    def run_orchestration_double(self, mutate_source: bool = False, interval: str | None = None,
                                 config_change: tuple[str, str] | None = None) -> tuple[dict, list[list[str]]]:
        # Every tool response here is a protocol double, never compiler evidence.
        def write(relative: str, value: str = "fixture") -> Path:
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(value)
            return path

        app_manifest = write("examples/fixture/Cargo.toml", '[package]\nname="fixture"\nversion="0.1.0"\n')
        app_source = write("examples/fixture/src/lib.rs", "#[test] fn component() {}")
        driver_manifest = write("crates/cargo-fe2o3/Cargo.toml")
        driver_source = write("crates/cargo-fe2o3/src/main.rs")
        for relative in ("config/tutorial-kernel-manifest-v1.json", "scripts/validate-tutorial-kernel-manifest.py",
                         adapter.WRAPPER, "scripts/tutorial_cpu_reference.py", "Cargo.lock"):
            write(relative)
        write("rust-toolchain.toml", '[toolchain]\nchannel="nightly-2026-04-03"\n')
        (self.root / ".cargo").mkdir(exist_ok=True)
        cargo_home = self.root / "cargo-home"
        cargo_home.mkdir(exist_ok=True)
        for base in (self.root / ".cargo", cargo_home):
            for name in ("config", "config.toml"):
                (base / name).unlink(missing_ok=True)
        config_path = None
        if config_change is not None:
            location, change = config_change
            config_path = (self.root / ".cargo" if location == "ancestor" else cargo_home) / "config.toml"
            if change == "remove":
                config_path.write_text("# original configuration\n")
            elif change == "precedence":
                config_path.write_text("# original config.toml\n")
                config_path = config_path.with_name("config")
        toolpaths = {name: write("tools/" + name) for name in ("cargo", "rustc", "rustup")}
        output = Path(tempfile.mkdtemp(prefix="observation-", dir=self.root))
        arguments = ["examples/fixture/Cargo.toml", "lib"]
        declaration = source_manifest(arguments)
        tutorial_manifest = write("config/tutorial-kernel-manifest-v1.json", json.dumps(declaration))
        plan = adapter.select_suite(declaration, arguments)
        plan["manifestSha256"] = adapter.digest(tutorial_manifest.read_bytes())
        validator_path = self.root / "scripts/validate-tutorial-kernel-manifest.py"
        plan["validatorSha256"] = adapter.digest(validator_path.read_bytes())
        contracts = {path: adapter.digest(path.read_bytes()) for path in
                     (app_manifest, app_source, self.root / "Cargo.lock")}
        observation = {**adapter.NO_AUTHORITY, "phases": [], "errors": []}
        target = {"name": "fixture", "kind": ["lib"], "crate_types": ["lib"],
                  "src_path": str(app_source), "test": True}
        metadata = {"packages": [{"id": "app", "manifest_path": str(app_manifest), "source": None,
                                  "targets": [target]}], "resolve": {"nodes": [{"id": "app", "features": []}]}}
        driver_metadata = {"packages": [{"id": "driver", "manifest_path": str(driver_manifest), "source": None}]}
        commands: list[list[str]] = []

        def child(argv, cwd, env, directory, label, deadline, phases):
            commands.append(argv)
            phases.append({"label": label, "argv": argv, "returncode": 0})
            if label.startswith("locate-"):
                return (str(toolpaths[label[7:]]) + "\n").encode()
            if label == "rustc-version":
                return b"rustc fixture\nhost: x86_64-unknown-linux-gnu\n"
            if label in {"driver-metadata", "suite-metadata"}:
                if label == "suite-metadata" and interval not in {None, "baseline"}:
                    changed = {"source": app_source, "manifest": app_manifest, "lock": self.root / "Cargo.lock",
                               "tutorial": tutorial_manifest, "validator": validator_path,
                               "cargo": toolpaths["cargo"], "rustc": toolpaths["rustc"],
                               "config": cargo_home / "config"}[interval]
                    with changed.open("a") as stream:
                        stream.write("\n# metadata interval mutation\n")
                return json.dumps(driver_metadata if label == "driver-metadata" else metadata).encode()
            if label == "driver-build":
                binary = output / "build/driver"
                binary.write_bytes(b"unexecuted driver double")
                binary.chmod(0o700)
                self.assertEqual(env["FE2O3_HIP_SYS_DISABLE"], "1")
                return (json.dumps({"reason": "compiler-artifact", "package_id": "driver",
                                    "target": {"name": "cargo-fe2o3", "kind": ["bin"], "crate_types": ["bin"],
                                               "src_path": str(driver_source)}, "profile": {"test": False, "opt_level": "0"},
                                    "executable": str(binary)}) + "\n" +
                        json.dumps({"reason": "build-finished", "success": True}) + "\n").encode()
            self.assertEqual(label, "suite")
            self.assertEqual(argv[argv.index("--manifest-path") + 1], str(app_manifest))
            self.assertIn("--lib", argv)
            self.assertNotIn("--all-targets", argv)
            self.assertNotIn("RUSTC", env)
            self.assertNotIn("FE2O3_HIP_SYS_DISABLE", env)
            executable = output / "build/test"
            executable.write_bytes(b"unexecuted test double")
            executable.chmod(0o700)
            events = self.events(["failed"] if mutate_source else ["ok"])
            events[0].update(package_id="app", target=target, executable=str(executable))
            if mutate_source:
                app_source.write_text("changed after fake execution")
                phases[-1]["returncode"] = 101
            if config_change is not None:
                assert config_path is not None
                if config_change[1] == "remove":
                    config_path.unlink()
                else:
                    config_path.write_text("# appeared while suite ran\n")
            return b"\n".join(json.dumps(event).encode() for event in events) + b"\n"

        validations = []

        def validate_manifest(root, manifest):
            # A contract-checking double tests the adapter's temporal binding,
            # not the real validator or a compiler result.
            self.assertEqual(manifest, declaration)
            validations.append(True)
            for path, expected in contracts.items():
                if adapter.digest(path.read_bytes()) != expected:
                    raise SystemExit("fixture source/manifest/lock contract is stale")
            if interval == "baseline":
                app_source.write_text("changed during contract validation")

        validator = SimpleNamespace(effective_cargo_lock=lambda root, package, label: root / "Cargo.lock",
                                    validate_manifest=validate_manifest)
        with patch.dict(os.environ, {"HOME": str(self.root), "PATH": "/usr/bin", "CARGO_HOME": str(cargo_home)}, clear=True), \
                patch.object(adapter, "load_plan", return_value=(plan, validator)), \
                patch.object(adapter.shutil, "which", return_value=str(toolpaths["rustup"])), \
                patch.object(adapter, "child", side_effect=child):
            if mutate_source or interval is not None or config_change is not None:
                expected_error = ("input drift" if mutate_source else
                                  "configuration input drift" if config_change is not None else
                                  {"source": "bound source contract refused", "manifest": "bound source contract refused",
                                   "lock": "bound source contract refused", "tutorial": "original tutorial manifest changed",
                                   "validator": "source validator changed", "cargo": "tool changed before baseline",
                                   "rustc": "tool changed before baseline", "config": "configuration drift during metadata",
                                   "baseline": "input drift during baseline validation"}[interval])
                with self.assertRaisesRegex(adapter.ObservationError, expected_error):
                    adapter.execute(self.root, arguments, output, time.monotonic(), observation)
            else:
                adapter.execute(self.root, arguments, output, time.monotonic(), observation)
                self.assertEqual(len(validations), 2)
        if interval is not None:
            self.assertNotIn("driver-build", [phase["label"] for phase in observation["phases"]])
        return observation, commands

    def test_orchestration_double_binds_absolute_selected_source_and_keeps_authority_false(self) -> None:
        observation, commands = self.run_orchestration_double(False)
        self.assertEqual(observation["outcome"], "passed")
        self.assertTrue(observation["inputsUnchanged"])
        self.assertTrue(all(observation[key] is False for key in adapter.NO_AUTHORITY))
        self.assertEqual(len(commands), 7)

    def test_postflight_drift_does_not_erase_failed_test_or_process_history(self) -> None:
        observation, _ = self.run_orchestration_double(True)
        self.assertFalse(observation["inputsUnchanged"])
        self.assertEqual(observation["tests"]["outcome"], "failed")
        self.assertEqual(observation["phases"][-1]["returncode"], 101)

    def test_metadata_cannot_refresh_source_contract_or_tool_baselines(self) -> None:
        for interval in ("source", "manifest", "lock", "tutorial", "validator", "cargo", "rustc", "config", "baseline"):
            with self.subTest(interval=interval):
                observation, _ = self.run_orchestration_double(interval=interval)
                self.assertNotEqual(observation.get("outcome"), "passed")

    def test_config_presence_and_precedence_are_rescanned_outside_local_roots(self) -> None:
        for location in ("ancestor", "cargo-home"):
            for change in ("create", "remove", "precedence"):
                with self.subTest(location=location, change=change):
                    observation, _ = self.run_orchestration_double(config_change=(location, change))
                    self.assertFalse(observation["configurationInputsUnchanged"])
                    self.assertEqual(observation["tests"]["outcome"], "passed")
                    self.assertTrue(all(observation[key] is False for key in adapter.NO_AUTHORITY))

    def test_main_signal_and_deadline_cleanup_are_nonpassing_component_records(self) -> None:
        for mode in ("terminate", "deadline"):
            def execute(root, arguments, output, started, observation):
                code = ("import os,signal,time; os.kill(os.getppid(),signal.SIGTERM); time.sleep(5)"
                        if mode == "terminate" else "import time; time.sleep(5)")
                adapter.child([sys.executable, "-c", code], self.root, {}, output, "component-signal",
                              time.monotonic() + 10, observation["phases"])
            captured = io.StringIO()
            with patch.dict(os.environ, {"FE2O3_TUTORIAL_CPU_OUTPUT_ROOT": str(self.root)}, clear=True), \
                    patch.object(adapter, "execute", side_effect=execute), \
                    patch.object(adapter, "MAX_SECONDS", 0.1 if mode == "deadline" else 5), \
                    contextlib.redirect_stdout(captured):
                result = adapter.main(["fixture", "lib"])
            observation = json.loads(captured.getvalue())
            self.assertNotEqual(result, 0)
            self.assertNotEqual(observation["outcome"], "passed")
            self.assertIsNotNone(observation["phases"][-1]["returncode"])
            self.assertTrue(all(observation[key] is False for key in adapter.NO_AUTHORITY))


if __name__ == "__main__":
    unittest.main()
