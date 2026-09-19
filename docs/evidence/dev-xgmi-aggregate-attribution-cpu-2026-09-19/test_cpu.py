#!/usr/bin/env python3
"""Adversarial, synthetic CPU evidence calibration; never invokes Cargo."""

import importlib.util
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("run with python3 -I -B")

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("aggregate_cpu", HERE / "cpu.py")
C = importlib.util.module_from_spec(spec)
spec.loader.exec_module(C)


def write_json(path, value):
    path.write_text(json.dumps(value, sort_keys=True) + "\n")


def listing(names):
    return "".join(name + ": test\n" for name in sorted(names)) + f"\n{len(names)} tests, 0 benchmarks\n"


def passing(names, filtered=0, ignored=None):
    ignored = {} if ignored is None else ignored
    rows = [f"test {name} ... ok" for name in sorted(names)]
    rows += [f"test {name} ... ignored, {reason}" for name, reason in ignored.items()]
    return (f"\nrunning {len(rows)} tests\n" + "\n".join(rows)
            + f"\n\ntest result: ok. {len(names)} passed; 0 failed; {len(ignored)} ignored; "
              f"0 measured; {filtered} filtered out; finished in 0.01s\n\n")


def python_output():
    return "".join(f"{name} (__main__.CpuEvidenceTests.{name}) ... ok\n"
                   for name in sorted(C.PYTHON_TESTS)) + "\nRan 7 tests in 0.001s\n\nOK\n"


def fixture(folder):
    archive = Path(folder) / "archive"
    archive.mkdir()
    for name in C.STATIC:
        shutil.copyfile(HERE / name, archive / name)
    write_json(archive / "tools.json", C.tool_manifest(archive))
    source = {"base": "a" * 40, "files": dict.fromkeys(C.REQUIRED_SOURCE, "b" * 64)}
    runtime = C.BATCH_TESTS | C.DIAGNOSTIC_TESTS | {"unrelated::control"}
    outputs = {
        "source-before": json.dumps(source, sort_keys=True),
        "source-after": json.dumps(source, sort_keys=True),
        "rustc": "rustc 1.95.0\nbinary: rustc\n", "cargo": "cargo 1.95.0\n",
        "safety-inventory-roster": listing(C.SAFETY_TESTS | set(C.SAFETY_IGNORED)),
        "safety-inventory": passing(C.SAFETY_TESTS, ignored=C.SAFETY_IGNORED),
    }
    for target in ("gnu", "musl"):
        outputs[target + "-runtime-roster"] = listing(runtime)
        outputs[target + "-batch"] = passing(C.BATCH_TESTS, len(runtime) - len(C.BATCH_TESTS))
        outputs[target + "-diagnostic"] = passing(C.DIAGNOSTIC_TESTS, len(runtime) - len(C.DIAGNOSTIC_TESTS))
    for target in ("gnu", "musl", "feature-off"):
        outputs[target + "-example-roster"] = listing(C.EXAMPLE_TESTS)
        outputs[target + "-example"] = passing(C.EXAMPLE_TESTS)
    for index, (stage, command, timeout) in enumerate(C.commands(C.ROOT)):
        path = archive / "raw" / stage
        path.mkdir(parents=True)
        (path / "stdout").write_text(outputs.get(stage, ""))
        (path / "stderr").write_text(python_output() if stage == "verifier-tests" else "")
        write_json(path / "receipt.json", {
            "command": command, "cwd": str(C.ROOT), "started_ns": 1000 + index * 10,
            "finished_ns": 1001 + index * 10, "timeout_seconds": timeout,
            "pid": 42, "exit": 0, "error": None, "group_absent": True,
            "environment": None, "stdin_sha256": None,
            "stdout_sha256": C.sha(path / "stdout"), "stderr_sha256": C.sha(path / "stderr"),
        })
    write_json(archive / "binding.json", C.derive(archive))
    C.verify(archive, allow_unsealed=True)
    return archive


def change_output(archive, stage, stream, transform):
    path = archive / "raw" / stage
    target = path / stream
    target.write_text(transform(target.read_text()))
    row = C.read(path / "receipt.json")
    row[stream + "_sha256"] = C.sha(target)
    write_json(path / "receipt.json", row)


class CpuEvidenceTests(unittest.TestCase):
    def test_exact_commands_and_profile(self):
        commands = C.commands(C.ROOT)
        self.assertEqual(tuple(row[0] for row in commands), C.STAGES)
        self.assertEqual(len(commands), 21)
        self.assertEqual((len(C.BATCH_TESTS), len(C.DIAGNOSTIC_TESTS), len(C.EXAMPLE_TESTS)), (38, 9, 10))
        for name, command, timeout in commands:
            self.assertGreater(timeout, 0)
            self.assertLessEqual(timeout, 1800)
            if "cargo" in command:
                if name == "cargo":
                    continue
                self.assertEqual(command[:len(C.ENV)], C.ENV)
                self.assertIn("--frozen", command)
            if name.startswith("musl-"):
                self.assertIn("x86_64-unknown-linux-musl", command)
            if name.startswith("feature-off-"):
                self.assertIn("--no-default-features", command)
        for setting in ("CARGO_BUILD_JOBS=2", "CARGO_INCREMENTAL=0",
                        "CARGO_PROFILE_DEV_OPT_LEVEL=1", "CARGO_PROFILE_TEST_OPT_LEVEL=1",
                        "CARGO_PROFILE_DEV_DEBUG=0", "CARGO_PROFILE_TEST_DEBUG=0",
                        "CARGO_PROFILE_DEV_DEBUG_ASSERTIONS=true", "CARGO_PROFILE_TEST_DEBUG_ASSERTIONS=true",
                        "CARGO_PROFILE_DEV_OVERFLOW_CHECKS=true", "CARGO_PROFILE_TEST_OVERFLOW_CHECKS=true"):
            self.assertIn(setting, C.ENV)
        table = {name: command for name, command, _ in commands}
        self.assertEqual(table["gnu-batch"][-1], "kfd_backend::xgmi_batch")
        self.assertEqual(table["gnu-diagnostic"][-1], "kfd_backend::xgmi_diagnostic")
        self.assertEqual(table["clippy"][-3:], ["--", "-D", "warnings"])
        self.assertIn("skip_children=true", table["fmt"])
        self.assertNotIn("--ignored", table["safety-inventory"])

    def test_list_rosters_reject_omission_substitution_and_duplicates(self):
        good = listing(C.BATCH_TESTS)
        self.assertEqual(C.roster(good), C.BATCH_TESTS)
        first = sorted(C.BATCH_TESTS)[0]
        for bad in (good + first + ": test\n", good.replace("38 tests", "038 tests"),
                    good.replace(first + ": test", first + ": benchmark")):
            with self.subTest(bad=bad[:80]), self.assertRaises(RuntimeError):
                C.roster(bad)
        for mutation in ("substitute", "omit"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as folder:
                archive = fixture(folder)
                replacement = first + "_replacement"
                for target in ("gnu", "musl"):
                    for suffix in ("-runtime-roster", "-batch"):
                        stage = target + suffix
                        if mutation == "substitute":
                            change_output(archive, stage, "stdout", lambda text: text.replace(first, replacement))
                        elif suffix.endswith("roster"):
                            names = C.roster((archive / "raw" / stage / "stdout").read_text()) - {first}
                            change_output(archive, stage, "stdout", lambda _: listing(names))
                        else:
                            change_output(archive, stage, "stdout", lambda _: passing(C.BATCH_TESTS - {first}, 10))
                # Re-derivation itself must reject jointly forged list/run rows,
                # even without depending on the original binding or seal.
                with self.assertRaises(RuntimeError):
                    C.derive(archive)

    def test_success_output_rejects_false_success_and_wrong_counts(self):
        good = passing(C.EXAMPLE_TESTS)
        C.passing_tests(good, C.EXAMPLE_TESTS)
        first = sorted(C.EXAMPLE_TESTS)[0]
        for bad in (good.replace(" ... ok", " ... FAILED", 1),
                    good.replace(" ... ok", " ... ignored", 1),
                    good.replace("0 filtered out", "1 filtered out"),
                    good.replace("10 passed", "010 passed"),
                    good.replace(first, sorted(C.EXAMPLE_TESTS)[1]),
                    good + "test unexpected ... ok\n"):
            with self.subTest(bad=bad[:80]), self.assertRaises(RuntimeError):
                C.passing_tests(bad, C.EXAMPLE_TESTS)
        safety = passing(C.SAFETY_TESTS, ignored=C.SAFETY_IGNORED)
        C.passing_tests(safety, C.SAFETY_TESTS, ignored=C.SAFETY_IGNORED)
        with self.assertRaises(RuntimeError):
            C.passing_tests(safety.replace("ignored, explicit", "ignored, other"), C.SAFETY_TESTS, ignored=C.SAFETY_IGNORED)
        C.python_tests(python_output())
        for bad in (python_output().replace(" ... ok", " ... skipped 'x'", 1),
                    python_output().replace("Ran 7 tests", "Ran 6 tests")):
            with self.assertRaises(RuntimeError):
                C.python_tests(bad)

    def test_receipts_reject_type_confusion_tampering_and_reordering(self):
        changes = {"exit": False, "pid": True, "timeout_seconds": 30.0,
                   "started_ns": True, "finished_ns": 1, "group_absent": 1,
                   "error": "timeout", "command": ["cargo", "--version"],
                   "environment": {}, "stdin_sha256": "0" * 64, "cwd": "/other"}
        for key, value in changes.items():
            with self.subTest(key=key), tempfile.TemporaryDirectory() as folder:
                archive = fixture(folder)
                path = archive / "raw/cargo/receipt.json"
                row = C.read(path)
                row[key] = value
                write_json(path, row)
                with self.assertRaises(RuntimeError):
                    C.derive(archive)
        with tempfile.TemporaryDirectory() as folder:
            archive = fixture(folder)
            path = archive / "raw/cargo/stdout"
            path.write_text(path.read_text() + "tampered\n")
            with self.assertRaises(RuntimeError):
                C.derive(archive)

    def test_binding_and_source_reject_rebased_mutations(self):
        with tempfile.TemporaryDirectory() as folder:
            archive = fixture(folder)
            binding = C.read(archive / "binding.json")
            binding["native_execution"] = 0
            with self.assertRaises(RuntimeError):
                C.derive(archive, binding)
            binding = C.read(archive / "binding.json")
            binding["commands"] = float(binding["commands"])
            with self.assertRaises(RuntimeError):
                C.derive(archive, binding)
            def omit(text):
                source = C.parse_json(text)
                del source["files"][C.BACKEND + "/xgmi_batch_diagnostic.rs"]
                return json.dumps(source)
            for stage in ("source-before", "source-after"):
                change_output(archive, stage, "stdout", omit)
            with self.assertRaises(RuntimeError):
                C.derive(archive)
        with tempfile.TemporaryDirectory() as folder:
            archive = fixture(folder)
            change_output(archive, "source-after", "stdout", lambda text: text.replace("b" * 64, "c" * 64, 1))
            with self.assertRaises(RuntimeError):
                C.derive(archive)
        with tempfile.TemporaryDirectory() as folder:
            archive = fixture(folder)
            path = archive / "PROTOCOL.md"
            path.write_text(path.read_text() + "changed\n")
            with self.assertRaises(RuntimeError):
                C.derive(archive)

    def test_inventory_and_seal_reject_unaccounted_files(self):
        for mutation in ("extra", "missing", "symlink", "digest", "duplicate-seal", "extra-directory"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as folder:
                archive = fixture(folder)
                C.seal(archive, create=True)
                C.verify(archive)
                target = archive / "raw/fmt/stderr"
                if mutation == "extra":
                    (archive / "extra").write_text("x")
                elif mutation == "extra-directory":
                    (archive / "extra").mkdir()
                elif mutation == "missing":
                    target.unlink()
                elif mutation == "symlink":
                    target.unlink()
                    target.symlink_to("stdout")
                elif mutation == "digest":
                    change_output(archive, "fmt", "stderr", lambda _: "altered output\n")
                else:
                    path = archive / "SHA256SUMS"
                    text = path.read_text()
                    path.write_text(text + text.splitlines()[0] + "\n")
                with self.assertRaises((RuntimeError, FileNotFoundError)):
                    C.verify(archive)

    def test_json_rejects_duplicates_and_nonfinite_values(self):
        for text in ('{"exit":0,"exit":0}', '{"a":{"b":1,"b":1}}',
                     '{"a":NaN}', '{"a":Infinity}', '{"a":-Infinity}'):
            with self.subTest(text=text), self.assertRaises(RuntimeError):
                C.parse_json(text)


if __name__ == "__main__":
    unittest.main()
