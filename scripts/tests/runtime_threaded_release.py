#!/usr/bin/env python3
"""CPU calibration for the real threaded release qualification path."""

import importlib.util
import hashlib
import os
from pathlib import Path
import signal
import struct
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("threaded_release", ROOT / "scripts/runtime_threaded_release.py")
CHECK = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = CHECK
SPEC.loader.exec_module(CHECK)


def layout():
    header = b"\x7fELF\x02\x01\x01" + bytes(9)
    header += struct.pack("<HHIQQQIHHHHHH", 3, 62, 1, 0x1000, 64, 0, 0, 64, 56, 4, 64, 0, 0)
    entries = [(1, 5, 0, 0x1000, 0, 0x100, 0x100, 4096),
               (2, 6, 0, 0, 0, 0, 0, 8),
               (0x6474e552, 4, 0, 0, 0, 0, 0, 8),
               (0x6474e551, 6, 0, 0, 0, 0, 0, 8)]
    return bytearray(header + b"".join(struct.pack("<IIQQQQQQ", *entry) for entry in entries))


class ReleaseTests(unittest.TestCase):
    def test_records_the_executing_interpreter(self):
        with tempfile.TemporaryDirectory() as name:
            qualifier = CHECK.Qualifier(Path(name), Path.home())
            with mock.patch.object(CHECK.sys, "executable", "/selected/python3"):
                with mock.patch.object(qualifier, "run", return_value=("/usr/bin/cc\n", "")):
                    with mock.patch.object(CHECK, "identities", return_value={}) as measured:
                        qualifier.tool_identity()
            self.assertIn(Path("/selected/python3"), measured.call_args.args[0])
            self.assertNotIn(Path("/usr/bin/python3"), measured.call_args.args[0])

    def test_capture_rejects_nonordinary_or_mismatched_files(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            path = root / "source.py"
            data = b"VALUE = 17\n"
            path.write_bytes(data)
            expected = hashlib.sha256(data).hexdigest()
            self.assertEqual(CHECK.capture_pinned(path, expected), data)
            with self.assertRaises(ValueError):
                CHECK.capture_pinned(path, "0" * 64)
            link = root / "link.py"
            link.symlink_to(path)
            with self.assertRaises(OSError):
                CHECK.capture_pinned(link, expected)
            fifo = root / "fifo"
            os.mkfifo(fifo)
            with self.assertRaises(ValueError):
                CHECK.capture_pinned(fifo, expected)
            with mock.patch.dict(CHECK.CAPTURED, {path: data}):
                path.write_text("raise AssertionError('reopened mutable helper')\n")
                module = CHECK.load("threaded_capture_regression", path)
                self.assertEqual(module.VALUE, 17)
            self.assertIs(importlib.util.spec_from_file_location, CHECK.ORIGINAL_SPEC)

    def test_exact_enabled_usage_not_stub(self):
        CHECK.validate_usage(1, "", CHECK.USAGE)
        for value in [(0, "", CHECK.USAGE), (2, "", CHECK.USAGE),
                      (1, "unexpected", CHECK.USAGE), (1, "", CHECK.USAGE + "extra\n"),
                      (2, "", "enable the fe2o3-runtime `hardware-qualification` feature\n")]:
            with self.assertRaises(CHECK.Error):
                CHECK.validate_usage(*value)

    def test_executable_thread_paths_not_strings(self):
        names = [f"_RINvNtNtCsTest_{prefix}NCNCINvNtCsTest_9fe2o3_kfd13shared_memory{name}"
                 for name in CHECK.THREAD_PATHS
                 for prefix in ("3std3sys9backtrace28___rust_begin_short_backtrace", "3std6thread6scoped5scope",
                                "3std6thread9lifecycle15spawn_unchecked")]
        lines = [f"{name} t {0x1000 + index * 0x100:x} 80" for index, name in enumerate(names)]
        symbols = "\n".join([*lines, "pthread_create T 2000 100"])
        self.assertEqual(len(CHECK.validate_thread_paths(symbols)), 9)
        for bad in [symbols.replace(" t ", " r "), symbols.replace(" 80", " 0"),
                    symbols.replace(" 1000", " 0"), "\n".join(lines),
                    "\n".join(lines[1:] + ["pthread_create T 2000 100"]),
                    symbols.replace("NCNCINv", "drop_in_placeNCNCINv"),
                    symbols.replace("NCNCINv", "call_once6vtableNCNCINv"),
                    symbols.replace("9fe2o3_kfd", "9lookalike"),
                    symbols.replace(" 1100 ", " 1000 "),
                    symbols.replace("3std3sys9backtrace28", "3std6thread9lifecycle15spawn_unchecked28")]:
            with self.assertRaises(CHECK.Error):
                CHECK.validate_thread_paths(bad)

    def test_exact_gnu_failure_not_blanket_expected_failure(self):
        expected = ["prohibited dynamic symbol: dlsym (exact)"]
        CHECK.validate_gnu_rejection(expected)
        for bad in [[], ["input error"], expected * 2, expected + ["prohibited dynamic symbol: fork (exact)"]]:
            with self.assertRaises(CHECK.Error):
                CHECK.validate_gnu_rejection(bad)

    def test_static_pie_layout_rejects_weaker_profiles(self):
        CHECK.validate_static_layout(layout())
        for offset, fmt, value in [(16, "H", 2), (18, "H", 183), (24, "Q", 0),
                                   (54, "H", 48), (56, "H", 0), (68, "I", 7),
                                   (64 + 56, "I", 3), (64 + 2 * 56, "I", 0),
                                   (64 + 3 * 56 + 4, "I", 7)]:
            bad = layout()
            struct.pack_into("<" + fmt, bad, offset, value)
            with self.assertRaises(CHECK.Error):
                CHECK.validate_static_layout(bad)

    def test_exact_resolved_crt_and_archive_roster(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            paths = [root / item for item in ("rcrt1.o", "crti.o", "crtbeginS.o", "crtendS.o", "crtn.o", "libc.a", "libunwind.a")]
            for path in paths:
                path.write_bytes(b"fixture")
            libraries = CHECK.identities(paths)
            link = "/usr/bin/cc " + " ".join(map(str, paths[:5]))
            mapping = "\n".join(f"LOAD {path}" for path in paths)
            result = CHECK.validate_link_inputs(link, mapping, libraries, root / "target", root / "tmp")
            self.assertEqual(result, libraries)
            for bad_link, bad_map in [(link.replace("rcrt1.o", "crt1.o"), mapping),
                                      (link, mapping.replace("libc.a", "foreign.a")),
                                      (link, ""), (link, mapping.replace(f"LOAD {paths[-1]}", "")),
                                      (link, mapping + "\nLOAD relative.o")]:
                with self.assertRaises(CHECK.Error):
                    CHECK.validate_link_inputs(bad_link, bad_map, libraries, root / "target", root / "tmp")
            paths[0].write_bytes(b"changed")
            with self.assertRaises(CHECK.Error):
                CHECK.validate_link_inputs(link, mapping, libraries, root / "target", root / "tmp")

    def test_ambient_cargo_config_rejected(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            home = root / "home"
            repo = root / "repo"
            home.mkdir()
            repo.mkdir()
            CHECK.reject_cargo_config(repo, home)
            cargo = root / ".cargo"
            cargo.mkdir()
            config = cargo / "config.toml"
            config.write_text("[build]\n")
            with self.assertRaises(CHECK.Error):
                CHECK.reject_cargo_config(repo, home)
            config.unlink()
            config.symlink_to(root / "missing")
            with self.assertRaises(CHECK.Error):
                CHECK.reject_cargo_config(repo, home)

    def test_ci_routes_real_qualifier_once(self):
        script = 'source scripts/ci-local.sh; run_step() { printf "%s\\n" "$*"; }; run_runtime_threaded_release_policy'
        result = subprocess.run(["bash", "-c", script], cwd=ROOT, capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr)
        lines = result.stdout.splitlines()
        self.assertEqual(len(lines), 2)
        self.assertTrue(lines[0].startswith("runtime-threaded-release-tests python3 "))
        self.assertTrue(lines[1].startswith("runtime-threaded-release-qualification python3 "))
        self.assertIn("scripts/runtime_threaded_release.py --output-root ", lines[1])
        ci = (ROOT / "scripts/ci-local.sh").read_text()
        runtime_body = ci.split("run_runtime_pure_rust_policy() {", 1)[1].split("\n}\n", 1)[0]
        self.assertEqual(runtime_body.splitlines().count("  run_runtime_threaded_release_policy"), 1)
        generic = ci.split("run_generic_core() {", 1)[1].split("\n}\n", 1)[0]
        self.assertEqual(generic.splitlines().count("  run_runtime_pure_rust_policy"), 1)
        self.assertIn("run: scripts/ci-local.sh generic-core", (ROOT / ".github/workflows/ci.yml").read_text())
        with tempfile.TemporaryDirectory() as name:
            result = subprocess.run(["bash", "scripts/ci-local.sh", "runtime-threaded-release", "extra"],
                                    cwd=ROOT, env={**CHECK.BASE.CLEAN_ENV, "CI_LOG_DIR": name},
                                    capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 2)
            self.assertIn("accepts no arguments", result.stderr)

    def test_both_builds_and_metadata_share_explicit_feature(self):
        with tempfile.TemporaryDirectory() as name:
            qualifier = CHECK.Qualifier(Path(name), Path.home())
            commands = []
            def stop_at_build(label, command, **_kwargs):
                commands.append((label, list(map(str, command))))
                if label.startswith("build-"):
                    raise RuntimeError("stop before compilation")
                return "{}", ""
            with mock.patch.object(CHECK.AUDIT, "audit_metadata", return_value=([], {})):
                with mock.patch.object(qualifier, "run", side_effect=stop_at_build):
                    for target in (CHECK.GNU, CHECK.MUSL):
                        with self.assertRaisesRegex(RuntimeError, "stop before compilation"):
                            qualifier.build(target, {}, {})
            for _label, command in commands:
                self.assertIn("--no-default-features", command)
                self.assertEqual(command[command.index("--features") + 1], "fe2o3-runtime/hardware-qualification")
                if "rustc" in command:
                    self.assertIn("--release", command)
                    self.assertEqual(command[command.index("--example") + 1], CHECK.EXAMPLE)
                    self.assertIn("--target", command)
                else:
                    self.assertIn("--filter-platform", command)
                    self.assertNotIn("--target", command)

    def test_inherited_process_cleanup_calibration(self):
        handlers = {number: signal.getsignal(number) for number in CHECK.OWNED.SIGNALS}
        try:
            for number in handlers:
                signal.signal(number, CHECK.OWNED.interrupted)
            CHECK.OWNED.process_self_test()
        finally:
            for number, handler in handlers.items():
                signal.signal(number, handler)


if __name__ == "__main__":
    unittest.main()
