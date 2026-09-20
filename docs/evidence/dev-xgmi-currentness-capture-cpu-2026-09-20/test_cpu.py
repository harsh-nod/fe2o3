#!/usr/bin/env python3
import importlib.util
from pathlib import Path
import tempfile
import unittest

path = Path(__file__).with_name("cpu.py")
spec = importlib.util.spec_from_file_location("capture_cpu", path)
Q = importlib.util.module_from_spec(spec)
spec.loader.exec_module(Q)
TARGET = "/dev/shm/fe2o3-link-parser-build-20260920.abcdefgh/target"


class Calibration(unittest.TestCase):
    def test_commands(self):
        commands = Q.commands(Q.ROOT, TARGET)
        self.assertEqual(len(commands), 22)
        self.assertEqual(len({name for name, _, _ in commands}), len(commands))
        for name, command, _ in commands:
            if name in {configuration + "-runtime" for configuration in Q.CONFIGURATIONS}:
                self.assertEqual(command[-len(Q.FILTERS):], list(Q.FILTERS))
                self.assertIn("CARGO_TARGET_DIR=" + TARGET, command)
                self.assertIn("--frozen", command)
                self.assertIn("CARGO_PROFILE_TEST_OVERFLOW_CHECKS=true", command)
                self.assertIn("CARGO_PROFILE_TEST_DEBUG_ASSERTIONS=true", command)
                self.assertEqual(command[command.index("--target") + 1],
                                 "x86_64-unknown-linux-musl" if name == "musl-runtime" else "x86_64-unknown-linux-gnu")
                self.assertIn("--no-default-features" if name == "feature-off-runtime" else "--all-features", command)
            if name.endswith("-clippy"):
                self.assertEqual(command[-3:], ["--", "-D", "warnings"])
        for target in ["/tmp/target", TARGET + "/..", TARGET.replace("abcdefgh", "abc")]:
            with self.assertRaises(RuntimeError):
                Q.commands(Q.ROOT, target)

    def test_rosters(self):
        for configuration in Q.CONFIGURATIONS:
            selected = Q.expected_runtime(configuration)
            full = selected | {"unrelated::test"}
            self.assertEqual(Q.selected_tests(full, configuration), selected)
            for changed in [full - {next(iter(selected))}, full | {"kfd_backend::xgmi_batch::unexpected"}]:
                with self.assertRaises(RuntimeError):
                    Q.selected_tests(changed, configuration)
            self.assertEqual(len(selected), 33 if configuration == "feature-off" else 58)
            self.assertEqual(len(Q.expected_example(configuration)), 12 if configuration == "feature-off" else 13)
        self.assertFalse(Q.NEW_RUNTIME & Q.expected_runtime("feature-off"))
        self.assertNotIn(Q.FORMAT_TEST, Q.expected_example("feature-off"))

    def test_test_output(self):
        output = "\nrunning 1 tests\ntest example ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.00s\n"
        Q.C.passing_tests(output, {"example"}, 2)
        for changed in [output.replace("... ok", "... ignored"), output.replace("1 passed", "0 passed"),
                        output.replace("example", "substitution"), output + "unexpected\n"]:
            with self.assertRaises(RuntimeError):
                Q.C.passing_tests(changed, {"example"}, 2)

    def test_inventory(self):
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory)
            files, directories = Q.expected_paths(TARGET)
            for name in sorted(directories):
                (archive / name).mkdir(parents=True, exist_ok=True)
            for name in files:
                (archive / name).write_text("test\n")
            Q.inventory(archive, TARGET, False)
            Q.seal(archive, TARGET, create=True)
            Q.seal(archive, TARGET)
            (archive / "unexpected").write_text("extra")
            with self.assertRaises(RuntimeError):
                Q.seal(archive, TARGET)
            (archive / "unexpected").unlink()
            (archive / "binding.json").write_text("changed")
            with self.assertRaises(RuntimeError):
                Q.seal(archive, TARGET)

    def test_typed_binding(self):
        self.assertFalse(Q.C.same_json({"exit": 0}, {"exit": False}))
        self.assertFalse(Q.C.same_json({"exit": 0}, {"exit": 0.0}))
        for text in ['{"exit":0,"exit":0}', '{"exit":NaN}']:
            with self.assertRaises(RuntimeError):
                Q.C.parse_json(text)


if __name__ == "__main__":
    unittest.main()
